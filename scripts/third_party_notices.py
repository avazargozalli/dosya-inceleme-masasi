#!/usr/bin/env python3
"""Üçüncü taraf lisans bildirimleri (NOTICE) üreteci ve kapısı.

F16-T01'in "üçüncü taraf lisans bildirimleri" maddesini **ölçülebilir** kılar:
bildirim dosyası elle yazılmaz, üç ekosistemin gerçek üstverisinden **üretilir** ve
bağımsız kaynaklarla **denetlenir**.

Kullanım:

    python3 scripts/third_party_notices.py build                      # üret
    python3 scripts/third_party_notices.py --check                    # denetle
    python3 scripts/third_party_notices.py --check --sbom sbom.json   # + SBOM eşleşmesi
    python3 scripts/third_party_notices.py --check --strict-texts     # metni diskten doğrula
    python3 scripts/third_party_notices.py --self-test                # kapı kurallarını sına

Neden gerekli (ölçülen kusurlar — ayrıntı `docs/releases/F15/README.md`):

* `MIT/Apache-2.0` (5 crate) **geçerli SPDX değildir**; SPDX'te `/` operatör değildir.
  Bir SPDX tüketicisi bu ifadeyi tek bilinmeyen lisans sanar. Normalleştirme tablosu
  (`LEGACY_EXPRESSIONS`) olmadan ifade bildirime olduğu gibi giriyordu.
* `pypdf==5.1.0` üstverisinde **hiç** `License`/`License-Expression` alanı yok
  (yalnız sınıflandırıcı + LICENSE dosyası) → yalnız üstveri okuyan üretici bu pini
  **sessizce atlar**. Kimlik LICENSE metninden çıkarılır ve `kaynak: çıkarıldı` yazılır.
* `stackback@0.0.2` (node) ve `r-efi@6.0.0` (rust) **lisans metni taşımıyor**;
  "metin var" varsayımı sessiz boşluk üretir → ayrı "metin yok" defteri.
* Node tarafında SBOM **çözümlenmiş** ağacı (192) yazar, `pnpm licenses list` ise
  yalnız **kurulu** paketleri bilir (bu makinede 105); aradaki 87 platforma özgü
  isteğe bağlı paket hiçbir yerde görünmezse kapsam sessizce daralır. Bu yüzden
  kapsam **kilit dosyasından çözümlenen ağaç**la kurulur, kurulu olanlarla değil.

İki katman:

* **dağıtılan** (`shipped`): rust crate'leri, python pinleri, node üretim ağacı →
  lisans **metni** gömülür.
* **geliştirme** (`dev`): yalnız geliştirme için kurulu node paketleri → kimlik yeter,
  metin gömülmez (dağıtılmıyor). Bildirim bu yüzden ~0,5 MB, 1,4 MB değil.

Kapı kuralları (`--check`):

1. Biçim sürümü + ürün sürümü `tauri.conf.json` ile aynı.
2. Her üçüncü taraf crate (kaynak taşıyan) **tam bir kez** ve doğru lisansla.
3. Node: kilit dosyasından çözümlenen her paket ya lisanslı listede ya "kurulu değil"
   defterinde (platformdan bağımsız kapsam); yerelde kurulu olanın lisansı üstveriyle
   karşılaştırılır, kurulu olmayan için uyarı (hata değil).
4. Her Python pini bildirimde, sürümü pinlendiği gibi.
5. Her lisans ifadesi geçerli SPDX (`scripts/spdx-ids.json`, SPDX 3.29.0).
6. Yasak lisans yok (`SSPL`, `BUSL`, `Commons-Clause`, `CC-BY-NC`, `Elastic`) ve
   AGPL-3.0 ile bağdaşmayan `GPL-2.0-only` yok.
7. Özet sayaçları bölüm içerikleriyle birebir.
8. Gömülü her metnin sha256'sı içerikle tutuyor; bileşenin andığı özet var; "metin yok"
   diyen her bileşen defterde; metni paylaşan bileşen sayısı başlıktaki sayıya eşit.
9. `--sbom` verilirse bildirim kümeleri SBOM bileşenleriyle **iki yönlü** eşleşir.
10. Okunamayan bileşen satırı **sessizce atlanmaz** (kırmızı).

`--strict-texts` ayrıca her metni **diskten yeniden okur** (cargo registry `src`,
`.crate` arşivi, `node_modules`, Python `dist-info`) ve özeti karşılaştırır — bildirim
metninin gerçekten bağımlılığın kendi dosyası olduğunu kanıtlayan tek mod budur.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tarfile
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
NOTICE_PATH = ROOT / "THIRD_PARTY_NOTICES.md"
SPDX_PATH = ROOT / "scripts" / "spdx-ids.json"
FORMAT_VERSION = 1
PYPROJECT = ROOT / "workers" / "document" / "pyproject.toml"
TAURI_CONF = ROOT / "apps" / "desktop" / "src-tauri" / "tauri.conf.json"
VENV_GLOB = "workers/document/.venv/lib/python*/site-packages/*.dist-info"
CARGO_CACHE_GLOB = "~/.cargo/registry/cache/*/*.crate"

TEXT_FILE = re.compile(r"(?i)^(licen[cs]e|copying|unlicense|notice)")

# Eski/serbest biçimler → SPDX. Her satır bir **yargı**; gerekçesi yanında yazılı.
LEGACY_EXPRESSIONS = {
    # cargo'nun eski çift lisans biçimi; SPDX'te `/` operatör değildir (5 crate).
    "MIT/Apache-2.0": (
        "MIT OR Apache-2.0",
        "cargo eski çift lisans biçimi; SPDX'te `/` ayırıcı değil, `OR` yazılır",
    ),
    # Python üstverisi serbest metin taşıyor (SPDX kimliği değil).
    "GNU AFFERO GPL 3.0": (
        "AGPL-3.0-only",
        "PyMuPDF üstverisi serbest metin; COPYING'de 'any later version' yok → only",
    ),
    "Apache License 2.0": ("Apache-2.0", "pytesseract üstverisi serbest metin"),
    "BSD License": (None, "pypdf sınıflandırıcısı; kimlik LICENSE metninden çıkarılır"),
}
DENY_PREFIXES = ("SSPL", "BUSL", "Commons-Clause", "CC-BY-NC", "Elastic")
# AGPL-3.0-or-later ile dağıtılamaz: yalnız GPL-2.0 "only" biçimleri.
PROHIBITED = ("GPL-2.0-only", "GPL-2.0")
COPYLEFT_PREFIXES = ("AGPL", "GPL", "LGPL", "MPL", "EUPL", "OSL")

GROUPS = (
    ("rust", "shipped", "Rust (dağıtılan)"),
    ("node", "shipped", "Node (dağıtılan)"),
    ("node", "dev", "Node (geliştirme)"),
    ("python", "shipped", "Python (dağıtılan)"),
)
GROUP_LABEL = {(eco, tier): label for eco, tier, label in GROUPS}
DEV_TOKEN = "—"


# --------------------------------------------------------------------------- model


@dataclass
class Component:
    ecosystem: str
    name: str
    version: str
    license: str
    declared: str
    origin: str
    tier: str = "shipped"
    text_sha: str | None = None
    text_bytes: int = 0
    text_file: str = ""
    text_body: str = ""
    license_measured: bool = True
    text_measured: bool = True

    @property
    def key(self) -> str:
        return f"{self.name} {self.version}"

    @property
    def group(self) -> str:
        return GROUP_LABEL.get((self.ecosystem, self.tier), self.ecosystem)


@dataclass
class Notices:
    version: str
    components: list[Component] = field(default_factory=list)
    texts: dict[str, str] = field(default_factory=dict)  # sha16 → metin
    missing_text: list[Component] = field(default_factory=list)
    not_installed: list[tuple[str, str]] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)


class NoticesError(RuntimeError):
    pass


class ParseError(NoticesError):
    pass


def sha16(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()[:16]


def normalize_whitespace(text: str) -> str:
    """Metni karşılaştırma için sabitle: satır sonları `\\n`, sonda boşluk yok."""
    return text.replace("\r\n", "\n").replace("\r", "\n").rstrip("\n")


# ------------------------------------------------------------------ SPDX denetimi


def load_spdx() -> tuple[set[str], set[str], str]:
    data = json.loads(SPDX_PATH.read_text())
    return set(data["licenses"]), set(data["exceptions"]), data["licenseListVersion"]


def spdx_tokens(expr: str) -> list[str]:
    return [t for t in re.split(r"[\s()]+", expr) if t]


def spdx_problems(expr: str, licenses: set[str], exceptions: set[str]) -> list[str]:
    """İfadeyi SPDX sözdizimine göre denetler; sorun listesi döner (boş = geçerli)."""
    problems: list[str] = []
    tokens = spdx_tokens(expr)
    if not tokens:
        return [f"boş lisans ifadesi: {expr!r}"]
    expect_license = True
    i = 0
    while i < len(tokens):
        token = tokens[i]
        if token in ("AND", "OR"):
            if expect_license:
                problems.append(f"`{token}` beklenmedik yerde: {expr!r}")
            expect_license = True
            i += 1
            continue
        if token == "WITH":
            if expect_license:
                problems.append(f"`WITH` beklenmedik yerde: {expr!r}")
            i += 1
            if i >= len(tokens) or tokens[i] not in exceptions:
                problems.append(f"bilinmeyen SPDX istisnası: {tokens[i] if i < len(tokens) else '(yok)'}")
            i += 1
            expect_license = False
            continue
        if not expect_license:
            problems.append(f"iki lisans yan yana (operatör yok): {expr!r}")
        if not token.startswith("LicenseRef-") and token not in licenses:
            problems.append(f"bilinmeyen SPDX kimliği: {token}")
        expect_license = False
        i += 1
    if expect_license:
        problems.append(f"ifade lisansla bitmiyor: {expr!r}")
    return problems


def denied_license(expr: str) -> list[str]:
    hits = [t for t in spdx_tokens(expr) if any(t.startswith(p) for p in DENY_PREFIXES)]
    hits += [t for t in spdx_tokens(expr) if t in PROHIBITED]
    return hits


def is_copyleft(expr: str) -> bool:
    return any(t.startswith(COPYLEFT_PREFIXES) for t in spdx_tokens(expr))


# ------------------------------------------------------------------- veri toplama


def run(cmd: list[str]) -> str:
    try:
        return subprocess.run(cmd, cwd=ROOT, check=True, capture_output=True, text=True).stdout
    except FileNotFoundError as exc:  # pragma: no cover - ortam hatası
        raise NoticesError(f"komut yok: {cmd[0]} ({exc})") from exc
    except subprocess.CalledProcessError as exc:
        raise NoticesError(f"{cmd[0]} başarısız: {exc.stderr.strip()[:400]}") from exc


def license_texts_in(directory: Path | None) -> list[Path]:
    if directory is None or not directory.is_dir():
        return []
    found = [f for f in sorted(directory.iterdir()) if f.is_file() and TEXT_FILE.match(f.name)]
    if found:
        return found[:4]
    # Bazı paketler metni alt dizinde tutar (`LICENSES/`, `legal/`).
    return [f for f in sorted(directory.rglob("*")) if f.is_file() and TEXT_FILE.match(f.name)][:4]


def crate_archive_texts(name: str, version: str) -> list[tuple[str, str]]:
    """`.crate` arşivinden lisans metinleri (registry `src/` yoksa yedek yol)."""
    root = Path(CARGO_CACHE_GLOB).expanduser().parent
    for archive in sorted(root.glob(f"*/{name}-{version}.crate")):
        try:
            with tarfile.open(archive, "r:gz") as tar:
                out: list[tuple[str, str]] = []
                for member in tar.getmembers():
                    if not member.isfile() or not TEXT_FILE.match(Path(member.name).name):
                        continue
                    handle = tar.extractfile(member)
                    if handle is not None:
                        out.append((member.name, handle.read().decode("utf-8", "replace")))
                return out[:4]
        except (tarfile.TarError, OSError):
            continue
    return []


def _license_id_set() -> set[str]:
    return load_spdx()[0]


def classify(declared: str) -> tuple[str, str]:
    """Ham lisans ifadesini SPDX'e çevirir: (spdx, kaynak etiketi).

    İki eski biçim var: (a) tabloda yazılı özel eşlemeler, (b) cargo'nun `/` ile
    yazdığı çift lisans (`MIT/Apache-2.0`, `Apache-2.0 / MIT`, `Unlicense/MIT`…).
    `/` biçimi **yalnız her parça geçerli SPDX kimliğiyse** `OR`'a çevrilir; değilse
    olduğu gibi bırakılır ve SPDX kapısı kırmızı olur (uydurma normalleştirme yok).
    """
    raw = (declared or "").strip()
    if raw in LEGACY_EXPRESSIONS:
        mapped, _note = LEGACY_EXPRESSIONS[raw]
        return (mapped or ""), ("normalleştirildi" if mapped else "çıkarıldı")
    if "/" in raw:
        parts = [part.strip() for part in raw.split("/") if part.strip()]
        ids = _license_id_set()
        if len(parts) > 1 and all(part in ids for part in parts):
            return " OR ".join(parts), "normalleştirildi"
    return raw, "bildirildi"


def legacy_reason(declared: str) -> str:
    """Normalleştirme gerekçesi (bildirimde insan okunur satır)."""
    if declared in LEGACY_EXPRESSIONS:
        return LEGACY_EXPRESSIONS[declared][1]
    if "/" in declared:
        return "cargo eski `/` çift lisans biçimi; parçalar SPDX kimliği → `OR` ile yazıldı"
    return "kaynaktan çıkarıldı"


def _merge_texts(texts: list[tuple[str, str]]) -> str:
    """Birden çok lisans dosyasını tek gövdede birleştirir (dosya adı başlıkta kalır)."""
    return "\n\n".join(f"--- {name} ---\n{normalize_whitespace(body)}".strip()
                       for name, body in texts if normalize_whitespace(body))


def _attach_text(comp: Component, texts: list[tuple[str, str]]) -> Component:
    if comp.tier != "shipped" or not texts:
        return comp  # geliştirme ağacı: metin gömülmez (dağıtılmıyor)
    merged = _merge_texts(texts)
    if not merged:
        return comp
    comp.text_sha = sha16(merged)
    comp.text_bytes = len(merged.encode("utf-8"))
    comp.text_file = ", ".join(name for name, _ in texts)
    comp.text_body = merged
    return comp


def collect_rust() -> tuple[list[Component], list[str]]:
    # İki ayrı cargo çalışma alanı var: kök ve Tauri kabuğu (`apps/desktop/src-tauri`).
    # Yalnız kök tararsa kabuk bağımlılıkları (tauri/wry/tao…) SBOM'da olur, bildirimde
    # olmaz — `--check --sbom` bunu kırmızı gösterir (2026-10-28'de ölçüldü).
    manifests = (None, "apps/desktop/src-tauri/Cargo.toml")
    components: list[Component] = []
    notes: list[str] = []
    seen: set[tuple[str, str]] = set()
    for manifest in manifests:
        cmd = ["cargo", "metadata", "--format-version", "1", "--all-features"]
        if manifest:
            cmd += ["--manifest-path", manifest]
        meta = json.loads(run(cmd))
        for pkg in meta["packages"]:
            if not pkg.get("source"):
                continue  # çalışma alanı: ürünün kendi kodu
            if (pkg["name"], pkg["version"]) in seen:
                continue  # iki çalışma alanında ortak crate: tek kez yazılır
            seen.add((pkg["name"], pkg["version"]))
            declared = pkg.get("license") or ""
            spdx, origin = classify(declared)
            if not declared and pkg.get("license_file"):
                spdx, origin = "", "çıkarıldı"
                notes.append(f"{pkg['name']}: yalnız `license-file` bildiriyor ({pkg['license_file']})")
            comp = Component("rust", pkg["name"], pkg["version"], spdx, declared, origin, "shipped")
            source_dir = Path(pkg["manifest_path"]).parent
            texts = [(f.name, f.read_text(encoding="utf-8", errors="replace"))
                     for f in license_texts_in(source_dir)] if source_dir.is_dir() else []
            if not texts:
                texts = crate_archive_texts(pkg["name"], pkg["version"])
            if not texts and not source_dir.is_dir():
                # Kaynak paketi bu ortamda yok (CI'da registry `src/` indirilmemiş):
                # "metin yok" ile "metin okunamadı" aynı şey değildir.
                comp.text_measured = False
            components.append(_attach_text(comp, texts))
    return components, notes


def resolved_node_tree() -> dict[tuple[str, str], str]:
    """Kilit dosyasından çözümlenen ağaç: (ad, sürüm) → tier (platformdan bağımsız)."""
    raw = json.loads(run(["pnpm", "list", "-r", "--depth", "Infinity", "--json"]))
    projects = raw if isinstance(raw, list) else [raw]
    resolved: dict[tuple[str, str], str] = {}

    def walk(deps, tier: str) -> None:
        for name, info in (deps or {}).items():
            info = info or {}
            version = info.get("version")
            if version and not str(version).startswith(("link:", "file:", "workspace:")):
                key = (name, version)
                resolved[key] = "shipped" if tier == "shipped" or resolved.get(key) == "shipped" else "dev"
            walk(info.get("dependencies"), tier)

    for project in projects:
        walk(project.get("dependencies"), "shipped")
        walk(project.get("devDependencies"), "dev")
    return resolved


def installed_node_licenses() -> dict[tuple[str, str], tuple[str, Path | None]]:
    """Kurulu paketlerin üstverisinden lisans + dizin (yalnız bu platformda kurulu olanlar)."""
    found: dict[tuple[str, str], tuple[str, Path | None]] = {}
    for scope in ("--prod", "--dev"):
        raw = json.loads(run(["pnpm", "licenses", "list", "--json", scope]))
        for expr, entries in raw.items():
            for entry in entries:
                for version in entry.get("versions") or []:
                    key = (entry["name"], version)
                    if key in found:
                        continue
                    directory = None
                    for candidate in entry.get("paths") or []:
                        match = re.search(r"\.pnpm/[^/]+@([^/]+)/", candidate)
                        if match and match.group(1) == version:
                            directory = Path(candidate)
                            break
                    found[key] = ((expr or "").strip(), directory)
    return found


def collect_node() -> tuple[list[Component], list[tuple[str, str]], list[str]]:
    resolved = resolved_node_tree()
    installed = installed_node_licenses()
    components: list[Component] = []
    notes: list[str] = []
    not_installed: list[tuple[str, str]] = []
    for (name, version), tier in sorted(resolved.items()):
        comp = Component("node", name, version, "", "", "bildirildi", tier)
        if (name, version) in installed:
            license_expr, directory = installed[(name, version)]
            comp.license = license_expr
            comp.declared = license_expr
            texts = [(f.name, f.read_text(encoding="utf-8", errors="replace"))
                     for f in license_texts_in(directory)]
            _attach_text(comp, texts)
        else:
            # Bu makinede kurulu değil: lisans üstverisi okunamaz → kimlik **uydurulmaz**.
            comp.license = ""
            comp.declared = ""
            comp.origin = "ölçülmedi"
            comp.license_measured = False
            comp.text_measured = False
            not_installed.append((name, version))
        components.append(comp)
    if not_installed:
        notes.append(f"{len(not_installed)} node bileşeni bu makinede kurulu değil "
                     f"(platforma özgü isteğe bağlı paketler) → lisansı ölçülmedi")
    return components, not_installed, notes


def declared_python_pins() -> list[tuple[str, str]]:
    text = PYPROJECT.read_text(encoding="utf-8")
    match = re.search(r"^all\s*=\s*\[(.*?)\]", text, re.S | re.M)
    if not match:
        raise NoticesError("pyproject.toml içinde `all` bağımlılık listesi bulunamadı")
    pins = []
    for item in re.findall(r'"([^"]+)"', match.group(1)):
        name, sep, version = item.partition("==")
        if not sep:
            raise NoticesError(f"Python bağımlılığı pinsiz: {item}")
        pins.append((name.strip(), version.strip()))
    return pins


def normalize_dist(name: str) -> str:
    return re.sub(r"[-_.]+", "-", name).lower()


def dist_name(dist_dir: Path) -> str:
    """`dist-info` dizininden paket adı: METADATA `Name:` alanı (sürüm adla karışmaz)."""
    meta = dist_dir / "METADATA"
    if meta.is_file():
        found = re.search(r"^Name:\s*(.+)$", meta.read_text(errors="replace"), re.M)
        if found:
            return normalize_dist(found.group(1).strip())
    return normalize_dist(dist_dir.name.removesuffix(".dist-info").rsplit("-", 1)[0])


def python_license_from_metadata(meta_text: str, dist_dir: Path) -> tuple[str, str, str, str]:
    """(spdx, kaynak, gerekçe, ham değer) — sıra: License-Expression → License → metinden çıkarım."""
    expr = re.search(r"^License-Expression:\s*(.+)$", meta_text, re.M)
    if expr:
        value = expr.group(1).strip()
        return value, "bildirildi", "", value
    simple = re.search(r"^License:\s*(.+)$", meta_text, re.M)
    if simple:
        value = simple.group(1).strip()
        spdx, origin = classify(value)
        return spdx, origin, LEGACY_EXPRESSIONS.get(value, (None, ""))[1], value
    classifiers = re.findall(r"^Classifier:\s*License\s*::.*$", meta_text, re.M)
    for candidate in ("LICENSE", "LICENSE.txt", "COPYING", "LICENSE.md"):
        path = dist_dir / candidate
        if not path.is_file():
            continue
        raw = path.read_text(encoding="utf-8", errors="replace")
        # Dizgi iki yana yaslı: cümleler satır ortasında bölünür → düzleştirerek ara.
        body = " ".join(raw.split())
        if "Redistribution and use in source and binary forms" in body:
            if "endorse or promote products derived from this software" in body:
                return ("BSD-3-Clause", "çıkarıldı",
                        "üstveri lisans bildirmiyor; LICENSE metni 3 madde", "BSD License (sınıflandırıcı)")
            return ("BSD-2-Clause", "çıkarıldı",
                    "üstveri lisans bildirmiyor; LICENSE metni 2 madde", "BSD License (sınıflandırıcı)")
        if "Permission is hereby granted, free of charge" in body:
            return ("MIT", "çıkarıldı",
                    "üstveri lisans bildirmiyor; LICENSE metni MIT", "MIT (LICENSE metninden)")
    raise NoticesError(
        f"{dist_dir.name}: lisans üstveride yok ve LICENSE metninden çıkarılamadı "
        f"(sınıflandırıcı: {classifiers[:2]})"
    )


def venv_dist_dirs() -> dict[str, Path]:
    return {dist_name(Path(p)): Path(p)
            for p in Path(ROOT).glob(VENV_GLOB) if not Path(p).name.startswith("dim_")}


def collect_python(*, require_env: bool = True) -> tuple[list[Component], list[str]]:
    """Python pinleri. `require_env=False` iken venv yoksa lisans **ölçülmez** (uydurulmaz)."""
    components: list[Component] = []
    notes: list[str] = []
    dist_dirs = venv_dist_dirs()
    if not dist_dirs:
        if require_env:
            raise NoticesError(
                "Python kurulu ortamı yok (`workers/document/.venv`): pinlerin lisansı "
                "uydurulmaz. Kurulum: `python3 -m venv workers/document/.venv && "
                "workers/document/.venv/bin/pip install -e \"workers/document[all]\"`"
            )
        for name, version in declared_python_pins():
            comp = Component("python", name, version, "", "", "bildirildi", "shipped")
            comp.license_measured = False
            comp.text_measured = False
            components.append(comp)
        notes.append("Python kurulu ortamı yok: pin lisansları bu ortamda ölçülmedi "
                     "(bildirimdeki değerler yalnız yapı/SPDX açısından denetlendi)")
        return components, notes
    for name, version in declared_python_pins():
        dist = dist_dirs.get(normalize_dist(name))
        if dist is None:
            raise NoticesError(f"{name}=={version} kurulu ortamda yok; lisansı ölçülemez")
        meta_text = (dist / "METADATA").read_text(errors="replace")
        installed = re.search(r"^Version:\s*(.+)$", meta_text, re.M)
        got = installed.group(1).strip() if installed else "?"
        if got != version:
            raise NoticesError(f"{name}: kurulu sürüm {got} ≠ pin {version}")
        spdx, origin, note, declared = python_license_from_metadata(meta_text, dist)
        comp = Component("python", name, version, spdx, declared, origin, "shipped")
        _attach_text(comp, [(f.name, f.read_text(encoding="utf-8", errors="replace"))
                            for f in license_texts_in(dist)])
        components.append(comp)
        if note:
            notes.append(f"{name}=={version}: {note}")
    return components, notes


# ------------------------------------------------------------------------ üretim


def product_version() -> str:
    return json.loads(TAURI_CONF.read_text())["version"]


def build_notices() -> Notices:
    notices = Notices(version=product_version())
    rust, rust_notes = collect_rust()
    node, not_installed, node_notes = collect_node()
    python, python_notes = collect_python()
    everything = rust + node + python
    # Lisansı **ölçülemeyen** bileşen (bu makinede kurulu değil) bileşen listesine girmez:
    # kimlik uydurulmaz, yalnız ayrı defterde görünür. Kapsam sayısı ikisini toplar.
    notices.components = sorted(
        (c for c in everything if c.license_measured),
        key=lambda c: (c.group, c.license or "zzz", c.name, c.version),
    )
    notices.not_installed = sorted(not_installed)
    notices.notes = rust_notes + node_notes + python_notes
    for comp in notices.components:
        if comp.text_sha:
            notices.texts.setdefault(comp.text_sha, comp.text_body)
        elif comp.tier == "shipped":
            if not comp.text_measured:
                raise NoticesError(
                    f"{comp.name} {comp.version}: lisans metni okunamadı (kaynak paket yok). "
                    "Bildirimi eksik üretmek yerine durur: `cargo fetch` ile kaynakları indir "
                    "ya da paketi kur."
                )
            notices.missing_text.append(comp)
    return notices


def render(notices: Notices) -> str:
    _licenses, _exceptions, spdx_version = load_spdx()
    grouped: dict[str, list[Component]] = {label: [] for _eco, _tier, label in GROUPS}
    for comp in notices.components:
        grouped[comp.group].append(comp)
    distinct = sorted({c.license for c in notices.components if c.license})
    use_count: dict[str, int] = {}
    for comp in notices.components:
        if comp.text_sha:
            use_count[comp.text_sha] = use_count.get(comp.text_sha, 0) + 1
    out: list[str] = [
        "# Üçüncü Taraf Lisans Bildirimleri",
        "",
        "> Bu dosya **üretilir**, elle düzenlenmez:",
        "> `python3 scripts/third_party_notices.py build`",
        ">",
        f"> Biçim: {FORMAT_VERSION} · Ürün sürümü: {notices.version} · "
        f"SPDX kimlik listesi: {spdx_version}",
        "",
        "Dağıtılan uygulama aşağıdaki üçüncü taraf bileşenleri içerir. Her bileşenin "
        "lisansı, bildirildiği biçim ve lisans metninin özeti yazılıdır. Metni "
        "paketlemeyen bileşenler ayrı defterde açıkça listelenir.",
        "",
        "Hukuki kararlar (kullanılabilir/koşullu, AGPL uyumu, sistem ikilileri) "
        "`docs/legal/license-registry.md` içinde tutulur; bu dosya yalnız **envanteri ve "
        "lisans metinlerini** taşır. Yeni bağımlılık eklendiğinde tablo kendiliğinden "
        "güncellenir; `--check` kapısı eksik bileşeni kırmızı gösterir.",
        "",
        "## Özet",
        "",
        "| Grup | Bileşen | Metin gömülü | Metin yok |",
        "|---|---|---|---|",
    ]
    for _eco, _tier, label in GROUPS:
        items = grouped[label]
        unique_texts = len({c.text_sha for c in items if c.text_sha})
        missing = len([c for c in items if not c.text_sha]) if _tier == "shipped" else 0
        out.append(f"| {label} | {len(items)} | {unique_texts} | {missing} |")
    out.append(f"| **Toplam** | **{len(notices.components)}** | **{len(notices.texts)}** | "
               f"**{len(notices.missing_text)}** |")
    out.append("")
    out.append(f"> Kapsam: {len(notices.components) + len(notices.not_installed)} · "
               f"ölçülen {len(notices.components)} · ölçülemeyen {len(notices.not_installed)}")
    out.append("")
    out.append("`Metin gömülü` **benzersiz** lisans metni sayısıdır (aynı metni paylaşan "
               "bileşenler tek gövde altında toplanır). **Geliştirme** grubu dağıtıma "
               "girmez: kimliği yazılır, metni gömülmez. Yalnız geliştirme için olan "
               "Python pini (`pythonDev`, pytest) ve çalışma alanı bağları bildirime girmez.")
    out.append("")
    out.append(f"- Farklı lisans ifadesi: **{len(distinct)}**")
    out.append(f"- Lisansı ölçülemeyen (bu makinede kurulu olmayan) node bileşeni: "
               f"**{len(notices.not_installed)}**")
    copyleft = [expr for expr in distinct if is_copyleft(expr)]
    if copyleft:
        out.append("- Copyleft içeren ifadeler: " + ", ".join(f"`{e}`" for e in copyleft))
    out.append("")
    out.append("## Bileşenler")
    out.append("")
    for _eco, _tier, label in GROUPS:
        items = grouped[label]
        out.append(f"### {label} — {len(items)} bileşen")
        out.append("")
        if not items:
            out.append("Yok.")
            out.append("")
            continue
        for comp in items:
            if comp.tier == "dev":
                token = DEV_TOKEN
            else:
                token = f"`{comp.text_sha}`" if comp.text_sha else "yok"
            licence = comp.license or "(ölçülmedi)"
            out.append(f"- {comp.name} {comp.version} — {licence} · "
                       f"kaynak: {comp.origin} · metin: {token}")
        out.append("")
    out.append("## Metni paketlemeyen bileşenler")
    out.append("")
    if not notices.missing_text:
        out.append("Yok.")
    else:
        out.append("Bu bileşenler lisans **ifadesini** bildiriyor ama kaynak paketinde "
                   "lisans metni taşımıyor. Metin dağıtıma giremez; kimlik bildirime göre "
                   "yazıldı ve elle doğrulanmalı.")
        out.append("")
        for comp in notices.missing_text:
            out.append(f"- {comp.name} {comp.version} — {comp.license} · paket lisans metni taşımıyor")
    out.append("")
    out.append("## Lisansı ölçülemeyen node bileşenleri")
    out.append("")
    if not notices.not_installed:
        out.append("Yok.")
    else:
        out.append("Bunlar kilit dosyasından **çözümlenen** ağaçta var ama bu makinede "
                   "kurulu değiller (platforma özgü isteğe bağlı paketler); lisans "
                   "üstverisi okunamadığı için kimlik **uydurulmadı**. Kapsam dışı değil, "
                   "**ölçülmedi**.")
        out.append("")
        for name, version in notices.not_installed:
            out.append(f"- {name} {version} — kurulu değil (platforma özgü isteğe bağlı paket)")
    out.append("")
    out.append("## Normalleştirilen ve çıkarılan lisans ifadeleri")
    out.append("")
    normalized = [c for c in notices.components if c.origin in ("normalleştirildi", "çıkarıldı")]
    if not normalized:
        out.append("Yok.")
    else:
        seen: dict[tuple[str, str], list[str]] = {}
        for comp in normalized:
            seen.setdefault((comp.declared, comp.license), []).append(comp.key)
        for (declared, spdx), names in sorted(seen.items()):
            reason = legacy_reason(declared)
            out.append(f"- `{declared or '(boş)'}` → `{spdx}` · {len(names)} bileşen · {reason}")
            out.append(f"  - {', '.join(sorted(names))}")
    out.append("")
    out.append("## Lisans metinleri")
    out.append("")
    lic_of = {c.text_sha: c.license for c in notices.components if c.text_sha}
    for sha in sorted(notices.texts, key=lambda s: (lic_of.get(s, ""), s)):
        body = notices.texts[sha]
        if not body:
            continue
        out.append(f"#### `{lic_of.get(sha, '?')}` · `{sha}` · {use_count.get(sha, 0)} bileşen")
        out.append("")
        out.append(f"<!-- lisans-metni:{sha} -->")
        out.append("~~~text")
        out.append(body)
        out.append("~~~")
        out.append("")
    if notices.notes:
        out.append("## Üretim notları")
        out.append("")
        for note in notices.notes:
            out.append(f"- {note}")
        out.append("")
    return "\n".join(out).rstrip("\n") + "\n"


# ---------------------------------------------------------------- dosya okuma


@dataclass
class ParsedNotices:
    format_version: int
    version: str
    components: dict[str, Component] = field(default_factory=dict)
    texts: dict[str, str] = field(default_factory=dict)
    missing_text: list[Component] = field(default_factory=list)
    not_installed: list[tuple[str, str]] = field(default_factory=list)
    summary: dict[str, tuple[int, int, int]] = field(default_factory=dict)
    scope: tuple[int, int, int] | None = None
    unparsed: list[str] = field(default_factory=list)


COMPONENT_LINE = re.compile(
    r"^- (\S+) (\S+) — (.+?) · kaynak: (bildirildi|normalleştirildi|çıkarıldı|ölçülmedi) · "
    r"metin: (?:`([0-9a-f]{16})`|(yok)|(—))$"
)
MISSING_LINE = re.compile(r"^- (\S+) (\S+) — (.+?) · paket lisans metni taşımıyor$")
NOTINSTALLED_LINE = re.compile(r"^- (\S+) (\S+) — kurulu değil")
SUMMARY_ROW = re.compile(
    r"^\| \*{0,2}([^|*]+?)\*{0,2} \| \*{0,2}(\d+)\*{0,2} \| \*{0,2}(\d+)\*{0,2} \| "
    r"\*{0,2}(\d+)\*{0,2} \|$"
)
HEADER_LINE = re.compile(r"^> Biçim: (\d+) · Ürün sürümü: (\S+) · SPDX kimlik listesi: (\S+)$", re.M)
SCOPE_LINE = re.compile(r"^> Kapsam: (\d+) · ölçülen (\d+) · ölçülemeyen (\d+)$", re.M)
TEXT_BLOCK = re.compile(r"^<!-- lisans-metni:([0-9a-f]{16}) -->\n~~~text\n(.*?)\n~~~$", re.M | re.S)


def parse_notices(text: str) -> ParsedNotices:
    header = HEADER_LINE.search(text)
    if not header:
        raise ParseError("bildirim başlığı okunamadı (biçim/ürün sürümü satırı yok)")
    parsed = ParsedNotices(format_version=int(header.group(1)), version=header.group(2))
    for block in TEXT_BLOCK.finditer(text):
        parsed.texts[block.group(1)] = block.group(2)
    if scope := SCOPE_LINE.search(text):
        parsed.scope = (int(scope.group(1)), int(scope.group(2)), int(scope.group(3)))
    # Lisans metinleri `## …` başlıkları taşıyabilir (ör. CC veri lisansı); bölüm izleme
    # metin gövdelerini görmezse belge yapısı bozulmaz ve satır yanlış bölüme düşmez.
    body_only = TEXT_BLOCK.sub("", text)
    section = None
    subsection: tuple[str, str] | None = None
    group_of = {label: (eco, tier) for eco, tier, label in GROUPS}
    for line in body_only.splitlines():
        if line.startswith("### "):
            # `### Rust (dağıtılan) — 159 bileşen` → grup etiketi
            subsection = group_of.get(line[4:].split(" — ")[0].strip())
            continue
        if line.startswith("## "):
            section = line[3:].strip()
            subsection = None
            continue
        if line.startswith("|") and (row := SUMMARY_ROW.match(line)):
            parsed.summary[row.group(1).strip()] = (int(row.group(2)), int(row.group(3)), int(row.group(4)))
            continue
        if section == "Bileşenler":
            if line.startswith("- "):
                match = COMPONENT_LINE.match(line)
                if not match:
                    parsed.unparsed.append(line)
                    continue
                name, version, lic, origin, text_hash, no_text, dev = match.groups()
                license_value = "" if lic.strip() == "(ölçülmedi)" else lic.strip()
                eco, tier = subsection if subsection else ("", "dev" if dev else "shipped")
                parsed.components[f"{name} {version}"] = Component(
                    eco,
                    name,
                    version,
                    license_value,
                    license_value,
                    origin,
                    "dev" if dev else tier,
                    text_sha=text_hash or None,
                )
        elif section == "Metni paketlemeyen bileşenler":
            if (match := MISSING_LINE.match(line)):
                parsed.missing_text.append(
                    Component("", match.group(1), match.group(2), match.group(3).strip(), "", "bildirildi")
                )
            elif line.startswith("- "):
                parsed.unparsed.append(line)
        elif section == "Lisansı ölçülemeyen node bileşenleri":
            if (match := NOTINSTALLED_LINE.match(line)):
                parsed.not_installed.append((match.group(1), match.group(2)))
            elif line.startswith("- "):
                parsed.unparsed.append(line)
    return parsed


# ------------------------------------------------------------------- kapı


@dataclass
class Findings:
    errors: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)

    def err(self, message: str) -> None:
        self.errors.append(message)

    def warn(self, message: str) -> None:
        self.warnings.append(message)


def expected_components(*, require_python_env: bool) -> tuple[dict[str, Component], list[tuple[str, str]], list[str]]:
    rust, rust_notes = collect_rust()
    node, not_installed, node_notes = collect_node()
    python, python_notes = collect_python(require_env=require_python_env)
    items = {c.key: c for c in rust + node + python}
    return items, not_installed, rust_notes + node_notes + python_notes


def check(notices_text: str, *, sbom_path: Path | None, strict_texts: bool) -> Findings:
    findings = Findings()
    notices = parse_notices(notices_text)
    licenses, exceptions, _spdx_version = load_spdx()
    expected, _ledger, _notes = expected_components(require_python_env=False)
    ledger = {f"{name} {version}" for name, version in notices.not_installed}

    # 1 — biçim + ürün sürümü
    if notices.format_version != FORMAT_VERSION:
        findings.err(f"biçim sürümü {notices.format_version} ≠ {FORMAT_VERSION}")
    expected_version = product_version()
    if notices.version != expected_version:
        findings.err(f"ürün sürümü {notices.version} ≠ tauri.conf.json {expected_version}")
    for line in notices.unparsed[:5]:
        findings.err(f"okunamayan bileşen satırı (sessizce atlanmaz): {line}")

    # 2-4 — kapsam: **kilit dosyasından çözümlenen** küme platformdan bağımsızdır
    covered = set(notices.components) | ledger
    for key in sorted(set(expected) - covered)[:8]:
        findings.err(f"kapsam dışı kalmış bileşen (ne lisanslı ne defterde): {key}")
    for key in sorted(covered - set(expected))[:8]:
        findings.err(f"bildirimde fazla bileşen (kaynakta yok): {key}")

    for key, comp in expected.items():
        got = notices.components.get(key)
        if got is None:
            if key in ledger:
                if comp.license_measured:
                    findings.warn(f"{key}: bildirimde 'kurulu değil' defterinde ama bu makinede "
                                  f"kurulu — platform farkı, lisans yeniden üretilmeli")
            continue
        if not comp.license_measured:
            # Bu ortamda ölçülemedi (ör. başka platformda kurulu): değer **doğrulanamaz**.
            if not got.license:
                findings.err(f"{key}: lisans boş")
            else:
                findings.warn(f"{key}: lisans bu ortamda ölçülemedi, bildirimdeki değer "
                              f"doğrulanmadı (`{got.license}`)")
            continue
        if comp.license and got.license != comp.license:
            findings.err(f"{key}: bildirim `{got.license}` ≠ kaynak `{comp.license}`")
        if not comp.license and got.license:
            findings.err(f"{key}: kaynak lisans ölçülemedi ama bildirim `{got.license}` yazıyor")
        if comp.origin and got.origin != comp.origin:
            findings.err(f"{key}: kaynak etiketi `{got.origin}` ≠ `{comp.origin}`")
        if got.tier != comp.tier:
            findings.err(f"{key}: katman `{got.tier}` ≠ `{comp.tier}`")
        if comp.tier == "dev":
            if got.text_sha:
                findings.err(f"{key}: geliştirme bileşeni metin gömüyor (gömülmemeli)")
        elif not comp.text_measured:
            if got.text_sha:
                findings.warn(f"{key}: lisans metni bu ortamda okunamadı, bildirimdeki özet "
                              f"doğrulanmadı")
        elif comp.text_sha and got.text_sha != comp.text_sha:
            findings.err(f"{key}: metin özeti bildirimde {got.text_sha} ≠ kaynak {comp.text_sha}")
        elif not comp.text_sha and got.text_sha:
            findings.err(f"{key}: bildirim metin gömüyor ama kaynakta lisans metni yok")

    missing_keys = {c.key for c in notices.missing_text}
    measurable = [c for c in expected.values() if c.tier == "shipped" and c.text_measured]
    if not measurable:
        findings.warn("lisans metinleri bu ortamda okunamadı (kaynak paketler yok); "
                      "'metin yok' defteri doğrulanmadı")
    else:
        expected_missing = {c.key for c in measurable if not c.text_sha}
        for key in sorted(expected_missing - missing_keys)[:5]:
            findings.err(f"metin yok defterinde eksik: {key}")
        for key in sorted(missing_keys - expected_missing)[:5]:
            comp = expected.get(key)
            if comp is None or not comp.text_measured:
                findings.warn(f"{key}: 'metin yok' bu ortamda doğrulanamadı")
            elif comp.text_sha:
                findings.err(f"{key}: defterde 'metin yok' diyor ama kaynakta lisans metni var")

    # 5-6 — SPDX geçerliliği ve yasaklı lisanslar
    copyleft_seen: set[str] = set()
    for key, comp in notices.components.items():
        if not comp.license:
            continue
        for problem in spdx_problems(comp.license, licenses, exceptions):
            findings.err(f"{key}: {problem}")
        for bad in denied_license(comp.license):
            findings.err(f"{key}: yasaklı lisans `{bad}`")
        if is_copyleft(comp.license) and comp.license not in copyleft_seen:
            copyleft_seen.add(comp.license)
            findings.warn(f"copyleft lisans: `{comp.license}` (ürün AGPL-3.0-or-later)")

    # 7 — özet sayaçları: tablo **dosyanın kendi içeriğiyle** birebir (platformdan
    # bağımsız). Ölçülemeyen bileşenler yalnız defterde olduğu için tabloya girmez.
    for _eco, _tier, label in GROUPS:
        items = [c for c in notices.components.values() if c.group == label]
        unique_texts = len({c.text_sha for c in items if c.text_sha})
        # Geliştirme grubunda metin **bilinçli** gömülmez: "metin yok" sayılmaz.
        missing = len([c for c in items if not c.text_sha]) if _tier == "shipped" else 0
        want = (len(items), unique_texts, missing)
        got = notices.summary.get(label)
        if got is None:
            findings.err(f"özet satırı yok: {label}")
        elif got != want:
            findings.err(f"özet `{label}` {got} ≠ dosyadaki içerik {want}")
    total = notices.summary.get("Toplam")
    if total is None:
        findings.err("özet `Toplam` satırı yok")
    else:
        if total[0] != len(notices.components):
            findings.err(f"özet toplam bileşen {total[0]} ≠ {len(notices.components)}")
        if total[1] != len(notices.texts):
            findings.err(f"özet toplam metin {total[1]} ≠ {len(notices.texts)}")
        if total[2] != len(notices.missing_text):
            findings.err(f"özet 'metin yok' {total[2]} ≠ {len(notices.missing_text)}")
    if notices.scope is None:
        findings.err("kapsam satırı yok (`> Kapsam: …`)")
    else:
        declared_total, declared_measured, declared_ledger = notices.scope
        real_total = len(notices.components) + len(notices.not_installed)
        if declared_total != real_total:
            findings.err(f"kapsam {declared_total} ≠ listede {real_total}")
        if declared_measured != len(notices.components):
            findings.err(f"kapsam 'ölçülen' {declared_measured} ≠ {len(notices.components)}")
        if declared_ledger != len(notices.not_installed):
            findings.err(f"kapsam 'ölçülemeyen' {declared_ledger} ≠ {len(notices.not_installed)}")
        if declared_total != len(expected):
            findings.err(f"kapsam {declared_total} ≠ kilit dosyasından çözümlenen {len(expected)}")

    # 8 — metin özetleri
    for sha, body in notices.texts.items():
        if sha16(body) != sha:
            findings.err(f"gömülü metin özeti tutmuyor: başlık {sha}, içerik {sha16(body)}")
    referenced = {c.text_sha for c in notices.components.values() if c.text_sha}
    for sha in sorted(referenced - set(notices.texts)):
        findings.err(f"bileşenin andığı metin dosyada yok: {sha}")
    for sha in sorted(set(notices.texts) - referenced):
        findings.err(f"hiçbir bileşenin anmadığı metin (ölü blok): {sha}")
    for sha, count in _usage_counts(notices_text).items():
        actual = sum(1 for c in notices.components.values() if c.text_sha == sha)
        if count != actual:
            findings.err(f"metin `{sha}` başlıkta {count} bileşen diyor, listede {actual}")

    # 9 — SBOM eşleşmesi
    if sbom_path is not None:
        if sbom_path.is_file():
            findings.errors.extend(compare_with_sbom(json.loads(sbom_path.read_text()), expected))
        else:
            findings.err(f"SBOM dosyası yok: {sbom_path}")

    # 10 — metinleri diskten doğrula
    if strict_texts:
        for key, comp in expected.items():
            if not comp.text_sha or not comp.text_body:
                continue
            got = notices.components.get(key)
            if not got or got.text_sha != comp.text_sha:
                continue
            body = notices.texts.get(comp.text_sha, "")
            if normalize_whitespace(body) != normalize_whitespace(comp.text_body):
                findings.err(f"{key}: gömülü metin kaynaktaki lisans dosyasıyla birebir değil")
    return findings


def _usage_counts(text: str) -> dict[str, int]:
    return {m.group(1): int(m.group(2))
            for m in re.finditer(r"^#### `.+?` · `([0-9a-f]{16})` · (\d+) bileşen$", text, re.M)}


def compare_with_sbom(sbom: dict, expected: dict[str, Component]) -> list[str]:
    errors: list[str] = []
    components = sbom.get("components", {})
    sbom_keys: set[str] = set()
    for list_name in ("rust", "node", "nodeDev", "python"):
        for entry in components.get(list_name, []):
            # Rust/node: `ad@sürüm` · Python: `ad==sürüm` (SBOM'un iki biçimi).
            if "==" in entry:
                name, version = entry.split("==", 1)
            else:
                name, sep, version = entry.rpartition("@")
                if not sep:
                    errors.append(f"SBOM bileşeni ayrıştırılamadı: {entry}")
                    continue
            key = f"{name} {version}"
            sbom_keys.add(key)
            if key not in expected:
                errors.append(f"SBOM'da olup bildirimde olmayan bileşen: {entry}")
    for key in sorted(set(expected) - sbom_keys)[:5]:
        errors.append(f"bildirimde olup SBOM'da olmayan bileşen: {key}")
    return errors


# ------------------------------------------------------------------ self-test


def _fixture() -> tuple[str, dict[str, Component]]:
    body = {
        sha16("MIT TEXT BODY"): "MIT TEXT BODY",
        sha16("DUAL TEXT"): "DUAL TEXT",
        sha16("BSD TEXT"): "BSD TEXT",
    }
    mit, dual, bsd = sha16("MIT TEXT BODY"), sha16("DUAL TEXT"), sha16("BSD TEXT")
    comps = [
        Component("rust", "alpha", "1.0.0", "MIT", "MIT", "bildirildi", "shipped", mit, 13, "LICENSE"),
        Component("rust", "beta", "2.0.0", "MIT OR Apache-2.0", "MIT/Apache-2.0", "normalleştirildi", "shipped", dual, 9, "LICENSE"),
        Component("node", "gamma", "3.1.4", "ISC", "ISC", "bildirildi", "shipped", None, 0, ""),
        Component("node", "delta", "0.9.0", "MIT", "MIT", "bildirildi", "dev", None, 0, ""),
        Component("python", "epsilon", "0.9", "BSD-3-Clause", "BSD-3-Clause", "çıkarıldı", "shipped", bsd, 8, "LICENSE"),
    ]
    notices = Notices(version="9.9.9", components=comps, texts=dict(body),
                      missing_text=[comps[2]], not_installed=[("@opt/pkg", "1.2.3")])
    use_count = {mit: 1, dual: 1, bsd: 1}
    lines = [
        "# Üçüncü Taraf Lisans Bildirimleri", "",
        f"> Biçim: {FORMAT_VERSION} · Ürün sürümü: 9.9.9 · SPDX kimlik listesi: 3.29.0", "",
        "## Özet", "", "| Grup | Bileşen | Metin gömülü | Metin yok |", "|---|---|---|---|",
        "| Rust (dağıtılan) | 2 | 2 | 0 |", "| Node (dağıtılan) | 1 | 0 | 1 |",
        "| Node (geliştirme) | 1 | 0 | 0 |", "| Python (dağıtılan) | 1 | 1 | 0 |",
        "| **Toplam** | **5** | **3** | **1** |", "",
        "## Bileşenler", "",
    ]
    for comp in comps:
        if comp.tier == "dev":
            token = "—"
        else:
            token = f"`{comp.text_sha}`" if comp.text_sha else "yok"
        lines.append(f"- {comp.name} {comp.version} — {comp.license} · kaynak: {comp.origin} · metin: {token}")
    lines += [
        "", "## Metni paketlemeyen bileşenler", "",
        "- gamma 3.1.4 — ISC · paket lisans metni taşımıyor", "",
        "## Lisansı ölçülemeyen node bileşenleri", "",
        "- @opt/pkg 1.2.3 — kurulu değil (platforma özgü isteğe bağlı paket)", "",
        "## Lisans metinleri", "",
    ]
    for sha in (mit, dual, bsd):
        lic = next(c.license for c in comps if c.text_sha == sha)
        lines += [f"#### `{lic}` · `{sha}` · {use_count[sha]} bileşen", "",
                  f"<!-- lisans-metni:{sha} -->", "~~~text", body[sha], "~~~", ""]
    return "\n".join(lines) + "\n", {c.key: c for c in comps}


def _self_check(notices: ParsedNotices, expected: dict[str, Component]) -> list[str]:
    """`check`in saf çekirdeği: fixture verisiyle, disk/araç çağırmadan."""
    licenses, exceptions, _ = load_spdx()
    errors: list[str] = []
    if notices.format_version != FORMAT_VERSION:
        errors.append("biçim sürümü")
    if notices.unparsed:
        errors.append(f"okunamayan satır: {notices.unparsed[:2]}")
    if set(notices.components) != set(expected):
        errors.append(f"küme farkı: eksik {sorted(set(expected) - set(notices.components))[:2]} "
                      f"fazla {sorted(set(notices.components) - set(expected))[:2]}")
    for key, comp in expected.items():
        got = notices.components.get(key)
        if got is None:
            continue
        if got.license != comp.license:
            errors.append(f"{key} lisans {got.license} ≠ {comp.license}")
        if got.origin != comp.origin:
            errors.append(f"{key} kaynak {got.origin} ≠ {comp.origin}")
        if got.tier != comp.tier:
            errors.append(f"{key} katman {got.tier} ≠ {comp.tier}")
        if got.text_sha != comp.text_sha:
            errors.append(f"{key} metin {got.text_sha} ≠ {comp.text_sha}")
        for problem in spdx_problems(got.license, licenses, exceptions):
            errors.append(f"{key}: {problem}")
        if denied_license(got.license):
            errors.append(f"{key}: yasaklı lisans")
    missing_expected = {c.key for c in expected.values() if c.tier == "shipped" and not c.text_sha}
    if {c.key for c in notices.missing_text} != missing_expected:
        errors.append("metin yok defteri")
    if not notices.not_installed:
        errors.append("lisansı ölçülemeyen defteri boş")
    summary_expect = {"Rust (dağıtılan)": (2, 2, 0), "Node (dağıtılan)": (1, 0, 1),
                      "Node (geliştirme)": (1, 0, 0), "Python (dağıtılan)": (1, 1, 0)}
    for label, counts in summary_expect.items():
        if notices.summary.get(label) != counts:
            errors.append(f"özet {label}")
    if notices.summary.get("Toplam") != (5, 3, 1):
        errors.append("özet toplam")
    for sha, body in notices.texts.items():
        if sha16(body) != sha:
            errors.append("metin özeti")
    referenced = {c.text_sha for c in notices.components.values() if c.text_sha}
    if referenced != set(notices.texts):
        errors.append("metin kümesi")
    return errors


def self_test() -> int:
    failures = 0

    def expect(label: str, text: str, expected: dict[str, Component], should_pass: bool) -> None:
        nonlocal failures
        try:
            notices = parse_notices(text)
            errors = _self_check(notices, expected)
        except ParseError as exc:
            errors = [str(exc)]
        ok = not errors
        if ok != should_pass:
            failures += 1
            print(f"  KIRMIZI  {label}: beklenen {'yeşil' if should_pass else 'kırmızı'}, "
                  f"bulunan {'yeşil' if ok else 'kırmızı'} · {errors[:2]}")

    good, expected = _fixture()

    def drop(text: str, prefix: str) -> str:
        return "\n".join(line for line in text.splitlines() if not line.startswith(prefix)) + "\n"

    expect("tam bildirim yeşil", good, expected, True)
    expect("eksik bileşen kırmızı", drop(good, "- beta 2.0.0"), expected, False)
    expect("yanlış lisans kırmızı", good.replace("— ISC · kaynak", "— MIT · kaynak"), expected, False)
    expect("kimlik sapması kırmızı", good.replace("kaynak: normalleştirildi", "kaynak: bildirildi"), expected, False)
    expect("bozuk SPDX kırmızı", good.replace("MIT OR Apache-2.0", "MIT/Apache-2.0"), expected, False)
    expect("yasaklı lisans kırmızı", good.replace("— ISC ·", "— SSPL-1.0 ·"), expected, False)
    expect("sayaç uyuşmazlığı kırmızı", good.replace("| Rust (dağıtılan) | 2 | 2 | 0 |",
                                                     "| Rust (dağıtılan) | 3 | 2 | 0 |"), expected, False)
    expect("metin özeti kırmızı", good.replace("MIT TEXT BODY", "MIT TEXT BODX"), expected, False)
    expect("ölü metin bloğu kırmızı",
           good + "\n<!-- lisans-metni:3333333333333333 -->\n~~~text\nFAZLA\n~~~\n", expected, False)
    expect("metin yok defteri eksik kırmızı",
           good.replace("- gamma 3.1.4 — ISC · paket lisans metni taşımıyor\n", ""), expected, False)
    expect("lisansı ölçülemeyen defteri eksik kırmızı",
           good.replace("- @opt/pkg 1.2.3 — kurulu değil (platforma özgü isteğe bağlı paket)\n", ""),
           expected, False)
    expect("okunamayan satır kırmızı",
           good.replace("- gamma 3.1.4 — ISC · kaynak: bildirildi · metin: yok",
                        "- gamma 3.1.4 ISC metin yok"), expected, False)
    expect("geliştirme bileşenine metin kırmızı",
           good.replace("- delta 0.9.0 — MIT · kaynak: bildirildi · metin: —",
                        f"- delta 0.9.0 — MIT · kaynak: bildirildi · metin: `{sha16('BSD TEXT')}`"),
           expected, False)

    # SPDX ayrıştırıcısı
    licenses, exceptions, _ = load_spdx()
    cases = ["MIT", "MIT OR Apache-2.0", "Apache-2.0 WITH LLVM-exception", "LicenseRef-Beni",
             "(MIT OR Apache-2.0) AND Unicode-3.0"]
    bad_cases = ["MIT/Apache-2.0", "MIT AND", "MIT ISC", "", "Apache-2.0 WITH Nope-exception"]
    for expr in cases:
        if spdx_problems(expr, licenses, exceptions):
            failures += 1
            print(f"  KIRMIZI  SPDX {expr!r}: geçerli olmalıydı, {spdx_problems(expr, licenses, exceptions)}")
    for expr in bad_cases:
        if not spdx_problems(expr, licenses, exceptions):
            failures += 1
            print(f"  KIRMIZI  SPDX {expr!r}: geçersiz olmalıydı")
    if not denied_license("SSPL-1.0") or not denied_license("GPL-2.0-only") or denied_license("MIT OR Apache-2.0"):
        failures += 1
        print("  KIRMIZI  yasaklı lisans listesi")
    if not failures:
        print("  self-test: 13 kapı kuralı + 10 SPDX ifadesi + yasak listesi geçti")
    return 1 if failures else 0


# ----------------------------------------------------------------------- CLI


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Üçüncü taraf lisans bildirimleri (üret + denetle)")
    parser.add_argument("command", nargs="?", default="build", choices=["build"],
                        help="build: bildirimi üret (varsayılan)")
    parser.add_argument("--check", action="store_true", help="depodaki bildirimi denetle")
    parser.add_argument("--sbom", type=Path, default=None, help="SBOM ile iki yönlü eşleşme")
    parser.add_argument("--strict-texts", action="store_true", help="metinleri diskten doğrula")
    parser.add_argument("--self-test", action="store_true", help="kapı kurallarını fixture'larla sına")
    parser.add_argument("--out", type=Path, default=NOTICE_PATH, help="çıktı yolu (build)")
    args = parser.parse_args(argv)

    if args.self_test:
        print("self-test:")
        return self_test()

    try:
        if args.check:
            if not args.out.is_file():
                print(f"KIRMIZI: bildirim dosyası yok: {args.out}")
                return 1
            findings = check(args.out.read_text(encoding="utf-8"), sbom_path=args.sbom,
                             strict_texts=args.strict_texts)
            for warning in findings.warnings:
                print(f"UYARI: {warning}")
            if findings.errors:
                for error in findings.errors:
                    print(f"KIRMIZI: {error}")
                print(f"\n{len(findings.errors)} kusur — bildirim güncellenene kadar kapı kapalı.")
                return 1
            print("yeşil: bildirim üç ekosistemin gerçek üstverisiyle birebir"
                  + (" (metinler diskten doğrulandı)" if args.strict_texts else ""))
            return 0
        notices = build_notices()
        args.out.write_text(render(notices), encoding="utf-8")
        print(f"yazıldı: {args.out} · {len(notices.components)} bileşen · "
              f"{len(notices.texts)} metin · {len(notices.not_installed)} lisansı ölçülmedi · "
              f"{len(notices.missing_text)} metin yok")
        return 0
    except NoticesError as exc:
        print(f"KIRMIZI: {exc}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
