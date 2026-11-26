#!/usr/bin/env python3
"""`pnpm bench:core` — yerel, hassas veri içermeyen çekirdek benchmark koşucusu.

**Neden var:** Şartname §21.1 geliştirici komutları arasında `pnpm bench:core`
komutunu ve çıktısını ("donanım etiketli sonuç") **vaat ediyordu**; depoda böyle
bir komut yoktu. B-01 ölçümü bugüne dek bu belgedeki üç satırlık `cargo test`
komutunun ve beş `DIM_BENCH_*` ortam değişkeninin elle kopyalanmasıyla
koşuluyordu. 2026-11-14'te göreli `DIM_BENCH_OUT` yüzünden 254 saniyelik koşunun
raporu kayboldu (kusur, düzeltildi) — yani komutun kendisi bir hata yüzeyiydi.

Bu betik tek komutu gerçek koşucuya bağlar ve **raporu yayımlamadan önce
kapıdan geçirir**:

1. **Profil:** `smoke` (20 belge), `quick` (100 belge, varsayılan), `b01`
   (1.000 belge / 100.000 segment / 200 sorgu — şartname profili). Boyutlar
   `--docs/--pages/--queries-per-group` ile değiştirilirse profil `custom`
   olur ve B-01 hükmü **verilmez**.
2. **Disk kapısı:** depo dosya sisteminde yeterli boş alan yoksa koşu
   **başlatılmaz** (varsayılan 5 GiB; depo kuralı). Ölçüm öncesi/sonrası boş
   alan rapora yazılır.
3. **Gerçek koşucu:** `cargo test --release --locked -p dim-storage --test
   bench_scale -- --ignored --nocapture`. Çıktı `target/bench/*.log`a yazılır;
   `cargo` PATH'te yoksa `~/.cargo/bin/cargo` aranır (cron kabuğu dersi).
4. **Rapor kapısı (ölçülen kurallar):** makine bloğu (os/arch/cpus/cpuModel),
   korpus sayıları profil ile **birebir**, sorgu listesi uzunluğu/biçimi,
   `queryListSha256` 64 hex, `perQuery` uzunluğu ve sayısal alanları,
   `p50 ≤ p95 ≤ max`, **yayımlanan yüzdeliklerin `perQuery`den doğrusal ara
   değerle yeniden üretilebilmesi** (en yakın sıraya yuvarlayan bir koşucu
   burada kırmızı olur), yükleme/depolama pozitif, ertelenmiş sayım
   tutarlılığı, B-01 hükmünün profil ile tutarlılığı, hassas anahtar ve
   ev-dizini yolu yokluğu, `generatedAt` biçimi.
5. **Donanım etiketi:** rapora `harness` bloğu eklenir — komut, profil,
   `DIM_BENCH_*` ortamı, commit + kirli ağaç işareti, betik sha256, süre,
   disk alanı, log yolu. Rapor **atomik** yazılır (`tmp` + `os.replace`).

Kullanım:
    pnpm bench:core                       # quick (100 belge)
    pnpm bench:core -- --profile b01      # şartname profili (dakikalar)
    pnpm bench:core -- --profile smoke    # duman (saniyeler, derleme sonrası)
    pnpm bench:core -- --out docs/releases/F15/bench-core.json
    python3 scripts/bench_core.py --self-test

`--self-test` gerçek ölçüm koşmadan bütün kapı kurallarını fixture raporla
sınar: temiz rapor **yeşil**, her kusurlu rapor **kırmızı**. Ölçüm katmanı
**çekirdektir** (Tauri IPC ve arayüz yok); 100.000 **belge** kapasite testi
(F15-T04) bu betiğin kapsamı dışındadır — ayrı, uzun donanım koşusu ister.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import os
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass, replace
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Sequence
import tempfile

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = Path(__file__).resolve()
PACKAGE_JSON = ROOT / "package.json"
DEFAULT_OUT_DIR = ROOT / "target" / "bench"
RUNNER_TEST = "bench_scale"
RUNNER_CRATE = "dim-storage"
B01_SEGMENTS = 100_000
B01_QUERIES = 200
B01_P95_MS = 2_000.0
PERCENTILE_TOLERANCE_MS = 0.02  # perQuery alanları 2 ondalığa yuvarlanır


# --- profiller -------------------------------------------------------------


@dataclass(frozen=True)
class Profile:
    """Bir ölçüm profili: korpus boyu + operasyonel sınırlar."""

    name: str
    docs: int
    pages_per_doc: int
    queries_per_group: int
    page_only_every: int
    min_free_gb: float
    timeout_s: int

    @property
    def pages(self) -> int:
        return self.docs * self.pages_per_doc

    @property
    def queries(self) -> int:
        return 4 * self.queries_per_group

    @property
    def is_b01(self) -> bool:
        return (
            self.docs == 1_000
            and self.pages_per_doc == 100
            and self.queries_per_group == 50
        )


PROFILES: dict[str, Profile] = {
    # Duman: derleme sonrası saniyeler; yalnız "boru hattı çalışıyor mu" der.
    "smoke": Profile("smoke", docs=20, pages_per_doc=5, queries_per_group=1,
                     page_only_every=1, min_free_gb=5.0, timeout_s=1_800),
    # Varsayılan: geliştirici makinesinde ~1 dakikalık ölçüm + derleme.
    "quick": Profile("quick", docs=100, pages_per_doc=10, queries_per_group=5,
                     page_only_every=5, min_free_gb=5.0, timeout_s=3_600),
    # Şartname B-01 profili: 1.000 belge / 100.000 segment / 200 sorgu.
    "b01": Profile("b01", docs=1_000, pages_per_doc=100, queries_per_group=50,
                   page_only_every=10, min_free_gb=5.0, timeout_s=7_200),
}


# --- saf yardımcılar (--self-test ile sınanır) -----------------------------


def percentile(sorted_values: Sequence[float], p: float) -> float:
    """Doğrusal ara değer — `bench_scale`/`bench_ocr` ile **aynı** tanım.

    `(n-1)*p` konumu + floor/ceil ağırlığı. En yakın sıraya yuvarlama küçük
    örnekte p50'yi yukarı kaydırır ve aynı "p95" adını iki tanımla hesaplar;
    ölçüm belgesi tek tanım diyor, bu yardımcı o tanımı uygular.
    """

    if not sorted_values:
        return 0.0
    position = (len(sorted_values) - 1) * min(max(p, 0.0), 1.0)
    lower = math.floor(position)
    upper = math.ceil(position)
    weight = position - lower
    low = float(sorted_values[lower])
    high = float(sorted_values[upper])
    return low + (high - low) * weight


def profile_for(
    name: str,
    docs: int | None,
    pages_per_doc: int | None,
    queries_per_group: int | None,
) -> Profile:
    """Adlandırılmış profili ya da açık boyutlardan `custom` profilini çözer.

    Açık boyut verilirse adlandırılmış profil **kullanılmaz** (karışık niyet
    sessizce yanlış korpus ölçmesin); üç boyut da pozitif olmalı.
    """

    overrides = {
        "docs": docs,
        "pages": pages_per_doc,
        "queries": queries_per_group,
    }
    given = [key for key, value in overrides.items() if value is not None]
    if given and len(given) != 3:
        raise ValueError(
            "açık boyutların üçü birlikte verilmeli: --docs, --pages, "
            "--queries-per-group"
        )
    if given:
        if docs is None or pages_per_doc is None or queries_per_group is None:
            raise ValueError(
                "açık boyutların üçü birlikte verilmeli: --docs, --pages, "
                "--queries-per-group"
            )
        if docs <= 0 or pages_per_doc <= 0 or queries_per_group <= 0:
            raise ValueError("--docs, --pages ve --queries-per-group pozitif olmalı")
        return Profile(
            "custom",
            docs=docs,
            pages_per_doc=pages_per_doc,
            queries_per_group=queries_per_group,
            page_only_every=5,
            min_free_gb=5.0,
            timeout_s=7_200,
        )
    try:
        return PROFILES[name]
    except KeyError as exc:
        raise ValueError(
            f"bilinmeyen profil: {name} (seçenekler: {', '.join(PROFILES)})"
        ) from exc


def runner_command(cargo: str) -> list[str]:
    """Gerçek ölçüm koşucusunun komutu (tek kaynak)."""

    return [
        cargo,
        "test",
        "--release",
        "--locked",
        "-p",
        RUNNER_CRATE,
        "--test",
        RUNNER_TEST,
        "--",
        "--ignored",
        "--nocapture",
    ]


def runner_env(profile: Profile, report_path: Path, base: dict[str, str] | None = None) -> dict[str, str]:
    """Koşucunun okuduğu `DIM_BENCH_*` sözleşmesi (tek kaynak)."""

    env = dict(os.environ if base is None else base)
    env.update(
        {
            "DIM_BENCH_DOCS": str(profile.docs),
            "DIM_BENCH_PAGES": str(profile.pages_per_doc),
            "DIM_BENCH_QUERIES_PER_GROUP": str(profile.queries_per_group),
            "DIM_BENCH_PAGE_ONLY_EVERY": str(profile.page_only_every),
            "DIM_BENCH_OUT": str(report_path),
            "CARGO_TERM_COLOR": "never",
        }
    )
    return env


def find_cargo(explicit: str | None) -> str | None:
    """`cargo` yolunu çözer: açık yol → PATH → `~/.cargo/bin/cargo`.

    Cron kabuğunda `~/.cargo/bin` PATH'te olmayabiliyor (ölçüldü); bu yüzden
    son çare ev dizinindeki standart kurulum aranır. Bulunamazsa `None`.
    """

    if explicit:
        path = Path(explicit).expanduser()
        return str(path) if path.is_file() else None
    found = shutil.which("cargo")
    if found:
        return found
    fallback = Path.home() / ".cargo" / "bin" / "cargo"
    return str(fallback) if fallback.is_file() else None


def cargo_source(explicit: str | None) -> str:
    """Cargo'nun nereden bulunduğu — **yol yazılmaz** (rapor paylaşılabilir kalır)."""

    if explicit:
        return "explicit"
    return "path" if shutil.which("cargo") else "home"


def check_disk(free_bytes: int, min_free_gb: float) -> str | None:
    """Disk kapısı: yeterli alan yoksa hata metni, varsa `None`."""

    required = int(min_free_gb * (1024**3))
    if free_bytes < required:
        return (
            f"disk yetersiz: {free_bytes / 1024**3:.1f} GiB boş, "
            f"en az {min_free_gb:.1f} GiB gerekli — ağır koşu başlatılmadı"
        )
    return None


def resolve_out_path(value: str | None, stamp: str) -> Path:
    """Rapor yolu: göreli yol **depo köküne** çözülür (2026-11-14 kusuru)."""

    if not value:
        return DEFAULT_OUT_DIR / f"bench-core-{stamp}.json"
    path = Path(value).expanduser()
    return path if path.is_absolute() else ROOT / path


def canonical(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _numbers(entry: dict[str, Any], key: str) -> bool:
    value = entry.get(key)
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def _non_empty(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _as_number(value: Any) -> float | None:
    """JSON sayısı → float; bool ve diğer tipler `None` (JSON'da bool sayı değil)."""

    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return float(value)
    return None


SENSITIVE_KEYS = ("rawtext", "normalizedtext", "documenttext", "apikey", "api_key",
                  "secret", "password", "authorization", "prompt")
SENSITIVE_PATH_MARKERS = ("/Users/", "/home/", "\\Users\\")


def _walk(value: Any, path: str = ""):
    if isinstance(value, dict):
        for key, item in value.items():
            yield from _walk(item, f"{path}/{key}")
    elif isinstance(value, list):
        for index, item in enumerate(value):
            yield from _walk(item, f"{path}[{index}]")
    else:
        yield path, value


def validate_report(report: Any, profile: Profile) -> list[str]:
    """Rapor kapısı. Dönen liste boşsa rapor yayımlanabilir.

    Her kural `Rxx` kodu taşır; `--self-test` mutasyonları bu kodlara bakar.
    """

    errors: list[str] = []
    if not isinstance(report, dict):
        return ["R01 rapor bir JSON nesnesi değil"]

    version = report.get("schemaVersion")
    if not isinstance(version, int) or isinstance(version, bool) or version < 1:
        errors.append("R02 schemaVersion pozitif tam sayı değil")

    machine = report.get("machine")
    if not isinstance(machine, dict):
        errors.append("R03 machine bloğu yok — donanım etiketi üretilemez")
    else:
        for key in ("os", "arch", "cpuModel"):
            if not _non_empty(machine.get(key)):
                errors.append(f"R03 machine.{key} boş — donanım etiketi eksik")
        cpus = machine.get("cpus")
        if not isinstance(cpus, int) or isinstance(cpus, bool) or cpus <= 0:
            errors.append("R03 machine.cpus pozitif tam sayı değil")
        memory = machine.get("memoryBytes")
        if memory is not None and (
            not isinstance(memory, int) or isinstance(memory, bool) or memory <= 0
        ):
            errors.append("R03 machine.memoryBytes sayı ya da null olmalı")

    corpus = report.get("corpus")
    if not isinstance(corpus, dict):
        errors.append("R04 corpus bloğu yok")
        corpus = {}
    if corpus.get("documents") != profile.docs:
        errors.append(
            f"R04 corpus.documents {corpus.get('documents')!r} — profilde "
            f"{profile.docs} bekleniyor (bayat/yanlış rapor)"
        )
    if corpus.get("pages") != profile.pages:
        errors.append(
            f"R05 corpus.pages {corpus.get('pages')!r} — profilde "
            f"{profile.pages} bekleniyor"
        )
    pages = corpus.get("pages")
    segments = corpus.get("segments")
    if not isinstance(segments, int) or isinstance(segments, bool) or segments <= 0:
        errors.append("R06 corpus.segments pozitif tam sayı değil")
    elif isinstance(pages, int) and segments < pages:
        errors.append("R06 corpus.segments sayfa sayısından küçük olamaz")

    query_list = report.get("queryList")
    if not isinstance(query_list, list):
        errors.append("R07 queryList liste değil")
        query_list = []
    elif len(query_list) != profile.queries:
        errors.append(
            f"R07 queryList {len(query_list)} sorgu — profilde "
            f"{profile.queries} bekleniyor"
        )
    allowed_modes = {"Docket", "Flexible", "Phrase", "Exact"}
    allowed_groups = {"word", "phrase", "folded", "docket"}
    for index, entry in enumerate(query_list):
        if not isinstance(entry, dict):
            errors.append(f"R08 queryList[{index}] nesne değil")
            continue
        if not _non_empty(entry.get("text")):
            errors.append(f"R08 queryList[{index}].text boş")
        if entry.get("mode") not in allowed_modes:
            errors.append(f"R08 queryList[{index}].mode tanınmıyor: {entry.get('mode')!r}")
        if entry.get("group") not in allowed_groups:
            errors.append(f"R08 queryList[{index}].group tanınmıyor: {entry.get('group')!r}")

    digest = report.get("queryListSha256")
    if not isinstance(digest, str) or len(digest) != 64 or any(
        char not in "0123456789abcdef" for char in digest
    ):
        errors.append("R09 queryListSha256 64 küçük hex değil")

    per_query = report.get("perQuery")
    if not isinstance(per_query, list):
        errors.append("R10 perQuery liste değil")
        per_query = []
    elif len(per_query) != len(query_list):
        errors.append(
            f"R10 perQuery {len(per_query)} satır — sorgu listesi "
            f"{len(query_list)} (kesik/eksik rapor)"
        )
    timings: list[float] = []
    for index, entry in enumerate(per_query):
        if not isinstance(entry, dict):
            errors.append(f"R11 perQuery[{index}] nesne değil")
            continue
        if not _non_empty(entry.get("text")):
            errors.append(f"R11 perQuery[{index}].text boş")
        if not _numbers(entry, "withTotalMs") or entry["withTotalMs"] < 0:
            errors.append(f"R11 perQuery[{index}].withTotalMs sayı değil")
        else:
            timings.append(float(entry["withTotalMs"]))
        if entry.get("pageOnlyMs") is not None and not _numbers(entry, "pageOnlyMs"):
            errors.append(f"R11 perQuery[{index}].pageOnlyMs sayı ya da null olmalı")
        for key in ("hits", "total"):
            value = entry.get(key)
            if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                errors.append(f"R11 perQuery[{index}].{key} negatif olmayan tam sayı değil")

    search = report.get("search")
    if not isinstance(search, dict):
        errors.append("R12 search bloğu yok")
        search = {}
    if search.get("queries") != len(per_query):
        errors.append("R12 search.queries perQuery uzunluğuyla aynı değil")
    p50 = _as_number(search.get("p50Ms"))
    p95 = _as_number(search.get("p95Ms"))
    max_ms = _as_number(search.get("maxMs"))
    for key, value in (("p50Ms", p50), ("p95Ms", p95), ("maxMs", max_ms)):
        if value is None or value < 0:
            errors.append(f"R12 search.{key} sayı değil")
    if p50 is not None and p95 is not None and max_ms is not None:
        if not (p50 <= p95 <= max_ms):
            errors.append(
                f"R13 yüzdelikler sıralı değil: p50 {p50} · p95 {p95} · max {max_ms}"
            )
    if timings:
        ordered = sorted(timings)
        expected = {
            "p50Ms": percentile(ordered, 0.50),
            "p95Ms": percentile(ordered, 0.95),
            "maxMs": percentile(ordered, 1.0),
        }
        for key, want in expected.items():
            got = search.get(key)
            if not _numbers(search, key):
                continue
            if abs(float(got) - want) > PERCENTILE_TOLERANCE_MS:
                errors.append(
                    f"R14 search.{key} perQuery'den yeniden üretilemiyor: "
                    f"yayımlanan {float(got):.3f} ms, doğrusal ara değer "
                    f"{want:.3f} ms (en yakın sıra mı kullanıldı?)"
                )

    ingest = report.get("ingest")
    ingest_seconds = _as_number(ingest.get("seconds")) if isinstance(ingest, dict) else None
    if ingest_seconds is None or ingest_seconds <= 0:
        errors.append("R15 ingest.seconds pozitif sayı değil")

    storage = report.get("storage")
    db_bytes = _as_number(storage.get("dbBytes")) if isinstance(storage, dict) else None
    per_segment = (
        _as_number(storage.get("bytesPerSegment")) if isinstance(storage, dict) else None
    )
    if db_bytes is None or db_bytes <= 0:
        errors.append("R16 storage.dbBytes pozitif sayı değil")
    elif per_segment is None or per_segment <= 0:
        errors.append("R16 storage.bytesPerSegment pozitif sayı değil")

    deferred = report.get("searchTotalDeferred")
    if isinstance(deferred, dict):
        deferred_queries = deferred.get("queries")
        if not isinstance(deferred_queries, int) or isinstance(deferred_queries, bool):
            errors.append("R17 searchTotalDeferred.queries tam sayı değil")
        elif deferred_queries > 0:
            if deferred.get("matchesPageTotal") is not True:
                errors.append(
                    "R17 searchTotalDeferred.matchesPageTotal true değil — "
                    "ertelenmiş sayım sayfa toplamıyla aynı değil"
                )
            if deferred.get("identicalTotalsChecked") != deferred_queries:
                errors.append(
                    "R17 searchTotalDeferred.identicalTotalsChecked örnek sayısıyla aynı değil"
                )
    else:
        errors.append("R17 searchTotalDeferred bloğu yok")

    b01 = report.get("b01")
    if not isinstance(b01, dict):
        errors.append("R18 b01 bloğu yok")
    else:
        applies = b01.get("applies")
        if profile.is_b01 and applies is not True:
            errors.append(
                "R18 B-01 profili koşuldu ama b01.applies true değil — "
                "korpus/sorgu sayısı hedefin altında kalmış"
            )
        if applies is True:
            if not isinstance(b01.get("segments"), int) or b01["segments"] < B01_SEGMENTS:
                errors.append("R18 b01.applies true ama segment sayısı B-01 eşiğinin altında")
            if not isinstance(b01.get("queries"), int) or b01["queries"] < B01_QUERIES:
                errors.append("R18 b01.applies true ama sorgu sayısı B-01 eşiğinin altında")
            met = b01.get("coreP95Met")
            if not isinstance(met, bool):
                errors.append("R18 b01.applies true iken coreP95Met bool değil")
            elif p95 is not None and met != (p95 <= B01_P95_MS):
                errors.append(
                    "R18 b01.coreP95Met p95 ile tutarsız "
                    f"(p95 {p95} ms, hedef {B01_P95_MS:.0f} ms)"
                )
            core_p95 = _as_number(b01.get("coreP95Ms"))
            if core_p95 is not None and p95 is not None and (
                abs(core_p95 - p95) > PERCENTILE_TOLERANCE_MS
            ):
                errors.append("R18 b01.coreP95Ms search.p95Ms ile aynı değil")
        elif applies is False:
            if b01.get("coreP95Met") is not None:
                errors.append("R18 b01.applies false iken coreP95Met null olmalı")
        else:
            errors.append("R18 b01.applies bool değil")

    generated = report.get("generatedAt")
    if not isinstance(generated, str) or len(generated) < 19 or generated[10] != "T":
        errors.append("R21 generatedAt ISO-8601 görünmüyor")
    else:
        try:
            datetime.fromisoformat(generated.replace("Z", "+00:00"))
        except ValueError:
            errors.append("R21 generatedAt ISO-8601 olarak çözülemiyor")

    for path, value in _walk(report):
        leaf = path.rsplit("/", 1)[-1].lower()
        lowered = path.lower()
        if any(marker == leaf or marker in lowered for marker in SENSITIVE_KEYS):
            errors.append(f"R19 hassas anahtar raporda: {path}")
            break
    for path, value in _walk(report):
        if isinstance(value, str) and any(
            marker in value for marker in SENSITIVE_PATH_MARKERS
        ):
            errors.append(f"R20 hassas yol raporda: {path} (ev dizini sızıntısı)")
            break

    return errors


def wiring_errors(scripts: dict[str, Any] | None) -> list[str]:
    """`pnpm bench:core` komutu package.json'da doğru betiğe bağlı mı?"""

    errors: list[str] = []
    if not isinstance(scripts, dict):
        return ["W01 package.json scripts bloğu okunamadı"]
    command = scripts.get("bench:core")
    if command != "python3 scripts/bench_core.py":
        errors.append(
            "W01 scripts['bench:core'] 'python3 scripts/bench_core.py' değil: "
            f"{command!r}"
        )
    return errors


def tag_report(report: dict[str, Any], meta: dict[str, Any]) -> dict[str, Any]:
    """Raporun kopyasına `harness` bloğunu ekler (girdi değişmez)."""

    tagged = copy.deepcopy(report)
    tagged["harness"] = meta
    return tagged


def write_json_atomic(path: Path, payload: dict[str, Any]) -> None:
    """Önce `tmp`e yaz, sonra `os.replace` — yarım rapor yayımlanmaz."""

    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    os.replace(tmp, path)


def git_field(args: Sequence[str]) -> str:
    try:
        out = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True, check=False)
    except OSError:
        return ""
    return (out.stdout or "").strip() if out.returncode == 0 else ""


def script_sha256() -> str:
    return hashlib.sha256(SCRIPT.read_bytes()).hexdigest()


def summarize(report: dict[str, Any]) -> str:
    machine = report.get("machine") or {}
    corpus = report.get("corpus") or {}
    search = report.get("search") or {}
    ingest = report.get("ingest") or {}
    memory = machine.get("memoryBytes")
    memory_text = f"{memory / 1024**3:.1f} GiB" if isinstance(memory, int) and memory > 0 else "RAM ?"
    return (
        f"  {machine.get('cpuModel') or 'CPU ?'} · {machine.get('cpus', '?')} CPU · "
        f"{memory_text} · {machine.get('os', '?')}/{machine.get('arch', '?')}\n"
        f"  korpus: {corpus.get('documents')} belge · {corpus.get('pages')} sayfa · "
        f"{corpus.get('segments')} segment · yükleme {float(ingest.get('seconds', 0)):.2f} s\n"
        f"  arama: p50 {float(search.get('p50Ms', 0)):.2f} ms · "
        f"p95 {float(search.get('p95Ms', 0)):.2f} ms · "
        f"max {float(search.get('maxMs', 0)):.2f} ms · {search.get('queries')} sorgu"
    )


# --- koşu ------------------------------------------------------------------


def run_measurement(profile: Profile, out_path: Path, cargo: str) -> tuple[int, dict[str, Any] | None, Path]:
    """Gerçek koşucuyu çalıştırır; (çıkış kodu, rapor ya da None, log yolu)."""

    start_stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    log_path = out_path.with_suffix(".log")
    log_path.parent.mkdir(parents=True, exist_ok=True)
    report_path = out_path.with_suffix(".raw.json")
    command = runner_command(cargo)
    env = runner_env(profile, report_path)
    started = time.monotonic()
    with log_path.open("w", encoding="utf-8") as log:
        log.write("$ " + " ".join(command) + "\n")
        log.write("# " + " ".join(f"{k}={env[k]}" for k in sorted(env) if k.startswith("DIM_BENCH_")) + "\n")
        log.flush()
        try:
            completed = subprocess.run(
                command,
                cwd=ROOT,
                env=env,
                stdout=log,
                stderr=subprocess.STDOUT,
                timeout=profile.timeout_s,
                check=False,
            )
            code = completed.returncode
        except subprocess.TimeoutExpired:
            log.write(f"\n[zaman aşımı] {profile.timeout_s} s\n")
            code = 124
        except OSError as exc:
            log.write(f"\n[başlatılamadı] {exc}\n")
            code = 127
    elapsed = time.monotonic() - started
    print(f"  koşucu: {elapsed:.1f} s · çıkış {code} · log {log_path.relative_to(ROOT)}")
    if code != 0:
        return code, None, log_path
    if not report_path.is_file():
        print(f"  rapor yok: {report_path}")
        return 5, None, log_path
    try:
        report = json.loads(report_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"  rapor okunamadı: {exc}")
        return 5, None, log_path
    return 0, report, log_path


def self_test() -> int:
    """Kapı kurallarını fixture raporlarla sınar: her mutasyon kırmızı olmalı."""

    checks: list[tuple[str, bool, str]] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        checks.append((name, ok, detail))

    # Yüzdelik tanımı: doğrusal ara değer (en yakın sıra değil).
    micros = [10.0, 20.0, 30.0, 40.0]
    check("p50 doğrusal ara değer (25 ms)", abs(percentile(micros, 0.50) - 25.0) < 1e-9,
          f"{percentile(micros, 0.50)}")
    check("p95 doğrusal ara değer (38,5 ms)", abs(percentile(micros, 0.95) - 38.5) < 1e-9,
          f"{percentile(micros, 0.95)}")
    check("uçlar kaymaz", percentile(micros, 0.0) == 10.0 and percentile(micros, 1.0) == 40.0)
    check("boş örnek 0", percentile([], 0.95) == 0.0)

    # Profil çözümü.
    try:
        profile_for("yok", None, None, None)
        check("bilinmeyen profil reddedilir", False)
    except ValueError:
        check("bilinmeyen profil reddedilir", True)
    try:
        profile_for("quick", 50, None, None)
        check("eksik açık boyut reddedilir", False)
    except ValueError:
        check("eksik açık boyut reddedilir", True)
    custom = profile_for("quick", 50, 2, 3)
    check("açık boyutlar custom profili verir",
          custom.name == "custom" and custom.pages == 100 and custom.queries == 12)
    check("b01 profili şartname eşiğinde",
          PROFILES["b01"].is_b01 and PROFILES["b01"].docs == 1_000
          and PROFILES["b01"].pages == 100_000 and PROFILES["b01"].queries == 200)

    env = runner_env(PROFILES["smoke"], Path("/tmp/rapor.json"), base={})
    check("DIM_BENCH_* sözleşmesi",
          env["DIM_BENCH_DOCS"] == "20" and env["DIM_BENCH_PAGES"] == "5"
          and env["DIM_BENCH_QUERIES_PER_GROUP"] == "1" and env["DIM_BENCH_OUT"] == "/tmp/rapor.json")

    command = runner_command("/usr/bin/cargo")
    check("koşucu komutu gerçek test ikilisi",
          command[:4] == ["/usr/bin/cargo", "test", "--release", "--locked"]
          and "--ignored" in command and RUNNER_TEST in command)

    # Disk kapısı.
    check("disk eşiği: 4,9 GiB kırmızı", check_disk(int(4.9 * 1024**3), 5.0) is not None)
    check("disk eşiği: 5,1 GiB yeşil", check_disk(int(5.1 * 1024**3), 5.0) is None)

    # Yol çözümü + atomik yazım.
    check("göreli rapor yolu depo köküne çözülür",
          resolve_out_path("docs/x.json", "s") == ROOT / "docs" / "x.json")
    check("mutlak rapor yolu korunur",
          resolve_out_path("/tmp/x.json", "s") == Path("/tmp/x.json"))
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        target = Path(tmp) / "alt" / "rapor.json"
        write_json_atomic(target, {"a": 1})
        leftovers = list(target.parent.glob("*.tmp"))
        check("atomik yazım geçerli JSON + artık yok",
              json.loads(target.read_text(encoding="utf-8")) == {"a": 1} and not leftovers)

    # Cargo keşfi.
    check("cargo keşfi: açık yol korunur", find_cargo(sys.executable) == sys.executable)
    check("olmayan cargo yolu None", find_cargo("/olmayan/cargo") is None)

    # pnpm 10 `--` ayırıcısını aynen geçiriyor; argparse onu yememeli.
    check("pnpm '--' ayırıcısı soyulur",
          strip_leading_separator(["--", "--profile", "b01"]) == ["--profile", "b01"]
          and strip_leading_separator(["--profile", "b01"]) == ["--profile", "b01"])

    # package.json bağlantısı.
    real_scripts = None
    try:
        real_scripts = json.loads(PACKAGE_JSON.read_text(encoding="utf-8")).get("scripts")
    except (OSError, json.JSONDecodeError):
        pass
    check("gerçek package.json bench:core'a bağlı", wiring_errors(real_scripts) == [],
          str(wiring_errors(real_scripts)))
    check("bağsız package.json kırmızı", wiring_errors({"test": "x"}) != [])

    # Temiz fixture raporu.
    profile = profile_for("quick", 40, 4, 3)
    fixture = sample_report(profile)
    base_errors = validate_report(fixture, profile)
    check("temiz fixture raporu YEŞİL", base_errors == [], "; ".join(base_errors))

    # `harness` etiketi girdiyi değiştirmez.
    tagged = tag_report(fixture, {"tool": "pnpm bench:core"})
    check("harness etiketi eklenir, girdi korunur",
          tagged["harness"]["tool"] == "pnpm bench:core" and "harness" not in fixture)

    # Etiket de kapıdan geçer: ev dizini yolu `harness` bloğundan sızamaz.
    leaky = tag_report(fixture, {"tool": "pnpm bench:core",
                                 "command": ["/Users/biri/.cargo/bin/cargo", "test"]})
    check("harness bloğundaki ev dizini yolu KIRMIZI (R20)",
          any(err.startswith("R20") for err in validate_report(leaky, profile)))
    check("harness komutu yol yazmaz",
          runner_command("cargo")[0] == "cargo" and cargo_source("x") == "explicit"
          and cargo_source(None) in {"path", "home"})

    # Mutasyonlar: her biri hedef kuralı kırmızıya çevirmeli.
    def mutate(fn):
        broken = copy.deepcopy(fixture)
        fn(broken)
        return validate_report(broken, profile)

    mutations: list[tuple[str, Any, str]] = [
        ("machine bloğu yok", lambda r: r.pop("machine"), "R03"),
        ("cpus 0", lambda r: r["machine"].update(cpus=0), "R03"),
        ("cpuModel boş", lambda r: r["machine"].update(cpuModel=" "), "R03"),
        ("belge sayısı profile uymuyor", lambda r: r["corpus"].update(documents=39), "R04"),
        ("sayfa sayısı profile uymuyor", lambda r: r["corpus"].update(pages=159), "R05"),
        ("segment < sayfa", lambda r: r["corpus"].update(segments=10), "R06"),
        ("sorgu listesi kesik", lambda r: r["queryList"].pop(), "R07"),
        ("sorgu metni boş", lambda r: r["queryList"][0].update(text=""), "R08"),
        ("sha hex değil", lambda r: r.update(queryListSha256="Z" * 64), "R09"),
        ("perQuery kesik", lambda r: r["perQuery"].pop(), "R10"),
        ("perQuery süresi metin", lambda r: r["perQuery"][0].update(withTotalMs="12"), "R11"),
        ("p50 > p95", lambda r: r["search"].update(p50Ms=r["search"]["p95Ms"] + 1), "R13"),
        ("p95 en yakın sırayla (tanım kayması)",
         lambda r: r["search"].update(p95Ms=nearest_rank([q["withTotalMs"] for q in r["perQuery"]], 0.95)),
         "R14"),
        ("yükleme 0", lambda r: r["ingest"].update(seconds=0), "R15"),
        ("dbBytes 0", lambda r: r["storage"].update(dbBytes=0), "R16"),
        ("sayım sayfayla uyuşmuyor",
         lambda r: r["searchTotalDeferred"].update(matchesPageTotal=False), "R17"),
        ("b01 applies true ama eşik altı",
         lambda r: r["b01"].update(applies=True, coreP95Met=False), "R18"),
        ("b01 coreP95Met p95 ile tutarsız",
         lambda r: (r["b01"].update(applies=True, segments=100_000, queries=200, coreP95Ms=r["search"]["p95Ms"],
                                    coreP95Met=not (r["search"]["p95Ms"] <= 2000.0))), "R18"),
        ("hassas anahtar rawText",
         lambda r: r["perQuery"][0].update(rawText="gizli belge"), "R19"),
        ("ev dizini yolu", lambda r: r.update(note="/Users/biri/kasa.db"), "R20"),
        ("generatedAt bozuk", lambda r: r.update(generatedAt="26.11.2026"), "R21"),
    ]
    red = 0
    for label, fn, code in mutations:
        errors = mutate(fn)
        hit = any(err.startswith(code) for err in errors)
        red += 1 if hit else 0
        check(f"mutasyon kırmızı: {label} ({code})", hit, "; ".join(errors) or "KURAL TUTMADI")

    # B-01 profili kendi raporunda yeşil olmalı.
    b01_profile = PROFILES["b01"]
    b01_fixture = sample_report(b01_profile, b01=True)
    b01_errors = validate_report(b01_fixture, b01_profile)
    check("B-01 fixture raporu YEŞİL", b01_errors == [], "; ".join(b01_errors))

    failed = [item for item in checks if not item[1]]
    for name, ok, detail in checks:
        print(f"  {'YEŞİL ' if ok else 'KIRMIZI'} {name}" + (f" — {detail}" if detail and not ok else ""))
    print(
        f"\n{len(checks) - len(failed)}/{len(checks)} KANIT · "
        f"{len(failed)} KIRMIZI · {red}/{len(mutations)} mutasyon hedef kurala çarptı"
    )
    return 1 if failed else 0


def nearest_rank(values: list[float], p: float) -> float:
    """Eski (yanlış) tanım — self-test'te mutasyon üretmek için."""

    ordered = sorted(values)
    index = round((len(ordered) - 1) * p)
    return ordered[index]


def sample_report(profile: Profile, b01: bool = False) -> dict[str, Any]:
    """Kapıyı geçen sentetik rapor (self-test fixture'ı)."""

    groups = ["word", "phrase", "folded", "docket"]
    modes = ["Flexible", "Phrase", "Flexible", "Docket"]
    queries: list[dict[str, Any]] = []
    per_query: list[dict[str, Any]] = []
    timings: list[float] = []
    for group, mode in zip(groups, modes):
        for index in range(profile.queries_per_group):
            text = f"sorgu-{group}-{index}"
            entry = {"text": text, "mode": mode, "group": group, "filter": ""}
            queries.append(entry)
            timing = 10.0 + len(timings) * 1.0
            timings.append(timing)
            per_query.append(
                {
                    **entry,
                    "withTotalMs": timing,
                    "pageOnlyMs": timing * 0.6,
                    "deferredTotalMs": timing * 0.4,
                    "hits": 50,
                    "total": 120,
                    "hasMore": True,
                }
            )
    ordered = sorted(timings)
    p50 = percentile(ordered, 0.50)
    p95 = percentile(ordered, 0.95)
    max_ms = percentile(ordered, 1.0)
    page_only_queries = len(range(0, len(queries), profile.page_only_every))
    deferred_queries = page_only_queries
    b01_segments = B01_SEGMENTS if b01 else 160
    b01_queries = B01_QUERIES if b01 else len(queries)
    return {
        "schemaVersion": 2,
        "generatedAt": "2026-11-26T12:00:00.000000Z",
        "machine": {
            "os": "macos",
            "arch": "aarch64",
            "cpus": 8,
            "cpuModel": "Apple M2",
            "memoryBytes": 17_179_869_184,
            "storage": "Mac14,7",
        },
        "corpus": {
            "documents": profile.docs,
            "pages": profile.pages,
            "segments": max(b01_segments, profile.pages),
            "pagesPerDocument": profile.pages_per_doc,
        },
        "queryList": queries,
        "queryListSha256": "a" * 64,
        "perQuery": per_query,
        "ingest": {"seconds": 1.5},
        "search": {"queries": len(per_query), "p50Ms": p50, "p95Ms": p95, "maxMs": max_ms},
        "searchTotalDeferred": {
            "queries": deferred_queries,
            "identicalTotalsChecked": deferred_queries,
            "matchesPageTotal": True,
        },
        "b01": {
            "applies": b01,
            "segments": b01_segments,
            "queries": b01_queries,
            "coreP50Ms": p50,
            "coreP95Ms": p95,
            "coreP95Met": (p95 <= B01_P95_MS) if b01 else None,
        },
        "storage": {"dbBytes": 1_000_000, "bytesPerSegment": 1_000_000 // max(profile.pages, 1)},
    }


# --- giriş -----------------------------------------------------------------


def strip_leading_separator(argv: Sequence[str]) -> list[str]:
    """`pnpm bench:core -- --profile b01` biçimindeki ayırıcıyı soyar.

    pnpm 10 argümanları betiğe **aynen** geçiriyor (ayırıcı `--` dahil); argparse
    onu tanımadığı için komut kullanılamaz hâle geliyordu (ölçüldü).
    """

    items = list(argv)
    return items[1:] if items[:1] == ["--"] else items


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="pnpm bench:core — çekirdek benchmark koşucusu")
    parser.add_argument("--profile", default="quick", choices=sorted(PROFILES), help="ölçüm profili")
    parser.add_argument("--docs", type=int, default=None, help="açık belge sayısı (custom profil)")
    parser.add_argument("--pages", type=int, default=None, help="belge başına sayfa (custom)")
    parser.add_argument("--queries-per-group", type=int, default=None, help="grup başına sorgu (custom)")
    parser.add_argument("--out", default="", help="rapor yolu (göreli: depo kökü)")
    parser.add_argument("--min-free-gb", type=float, default=None, help="disk kapısı eşiği")
    parser.add_argument("--cargo", default="", help="cargo yolu (varsayılan: PATH → ~/.cargo/bin)")
    parser.add_argument("--dry-run", action="store_true", help="komutu yaz, koşma")
    parser.add_argument("--self-test", action="store_true", help="yalnız kapı kurallarını sına")
    args = parser.parse_args(strip_leading_separator(sys.argv[1:] if argv is None else argv))

    if args.self_test:
        return self_test()

    try:
        profile = profile_for(args.profile, args.docs, args.pages, args.queries_per_group)
    except ValueError as exc:
        print(f"profil hatası: {exc}")
        return 2
    if args.min_free_gb is not None:
        if args.min_free_gb <= 0:
            print("--min-free-gb pozitif olmalı")
            return 2
        profile = replace(profile, min_free_gb=args.min_free_gb)

    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    out_path = resolve_out_path(args.out or None, stamp)
    cargo = find_cargo(args.cargo or None)
    if cargo is None:
        print(
            "cargo bulunamadı (PATH ve ~/.cargo/bin). Rust toolchain kurulu mu? "
            "Cron kabuğunda `export PATH=\"$HOME/.cargo/bin:$PATH\"` gerekebilir."
        )
        return 2

    free_before = shutil.disk_usage(ROOT).free
    disk_error = check_disk(free_before, profile.min_free_gb)
    if disk_error:
        print(disk_error)
        return 3

    print(
        f"bench:core · profil {profile.name} · {profile.docs} belge · "
        f"{profile.pages} sayfa · {profile.queries} sorgu · disk "
        f"{free_before / 1024**3:.1f} GiB"
    )
    if args.dry_run:
        print("  $ " + " ".join(runner_command(cargo)))
        for key in ("DIM_BENCH_DOCS", "DIM_BENCH_PAGES", "DIM_BENCH_QUERIES_PER_GROUP",
                    "DIM_BENCH_PAGE_ONLY_EVERY", "DIM_BENCH_OUT"):
            print(f"  {key}={runner_env(profile, out_path.with_suffix('.raw.json'))[key]}")
        print(f"  rapor: {out_path}")
        return 0

    started_iso = datetime.now(timezone.utc).isoformat()
    started = time.monotonic()
    code, report, log_path = run_measurement(profile, out_path, cargo)
    duration = time.monotonic() - started
    if code != 0 or report is None:
        tail = ""
        try:
            lines = log_path.read_text(encoding="utf-8", errors="replace").splitlines()
            tail = "\n".join(lines[-25:])
        except OSError:
            pass
        print(f"koşu başarısız (çıkış {code}). Son satırlar:\n{tail}")
        return 4 if code else 5

    free_after = shutil.disk_usage(ROOT).free
    harness = {
        "tool": "pnpm bench:core",
        "profile": profile.name,
        "profileSettings": {
            "documents": profile.docs,
            "pagesPerDocument": profile.pages_per_doc,
            "queriesPerGroup": profile.queries_per_group,
            "pageOnlyEvery": profile.page_only_every,
            "minFreeGb": profile.min_free_gb,
        },
        # Yol **yazılmaz**: rapor paylaşılabilir kalmalı (ev dizini sızıntısı yok).
        "command": runner_command("cargo"),
        "cargoSource": cargo_source(args.cargo or None),
        "runnerEnv": {
            "DIM_BENCH_DOCS": str(profile.docs),
            "DIM_BENCH_PAGES": str(profile.pages_per_doc),
            "DIM_BENCH_QUERIES_PER_GROUP": str(profile.queries_per_group),
            "DIM_BENCH_PAGE_ONLY_EVERY": str(profile.page_only_every),
        },
        "repoCommit": git_field(["rev-parse", "HEAD"]) or None,
        "repoDirty": bool(git_field(["status", "--porcelain"])),
        "scriptSha256": script_sha256(),
        "startedAt": started_iso,
        "finishedAt": datetime.now(timezone.utc).isoformat(),
        "durationSeconds": round(duration, 1),
        "diskFreeGbBefore": round(free_before / 1024**3, 2),
        "diskFreeGbAfter": round(free_after / 1024**3, 2),
        "log": str(log_path.relative_to(ROOT)) if str(log_path).startswith(str(ROOT)) else None,
        "gateRules": "scripts/bench_core.py validate_report (R01–R21)",
    }
    tagged = tag_report(report, harness)
    # Kapı **etiketten sonra** koşar: `harness` bloğu da taranır, ev dizini
    # yolu ya da hassas anahtar oradan sızamaz (ilk yazımda sızıyordu — ölçüldü).
    errors = validate_report(tagged, profile)
    if errors:
        print(f"rapor kapısı KIRMIZI ({len(errors)} neden) — rapor yayımlanmadı:")
        for error in errors:
            print(f"  - {error}")
        return 6
    write_json_atomic(out_path, tagged)
    print(summarize(tagged))
    print(f"  rapor: {out_path.relative_to(ROOT) if str(out_path).startswith(str(ROOT)) else out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
