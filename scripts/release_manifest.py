#!/usr/bin/env python3
"""Sürüm paketi manifesti ve indirilen paket doğrulaması (F16-T05, F16-T09).

İkili paketler (`.dmg`/`.msi`/`.AppImage`/`.deb`) **elle** üretilir ve depoda
tutulmaz; bu betik onların **kimliğini** üretir ve indirilen kopyayı bağımsız
olarak doğrular. Kaynak arşivi tarafı `scripts/source_release.py`'nin işidir ve
buradaki manifest kaynak manifestine **bağlanır** (sürüm + commit birebir).

`build`: bir artefakt klasörünü tarar, dosya adından platform/mimari/tür çıkarır,
her dosyanın boyutunu ve sha256'sını yazar. Kurallar (her biri kapı):

1. Ad biçimi `<ürün>_<sürüm>_<mimari>[_<yerel>].<tür>` olmalı; tanınmayan dosya
   sessizce atlanmaz (`E_UNKNOWN_ARTIFACT`).
2. Dosyadaki sürüm `--version` ile birebir aynı olmalı (`E_VERSION`).
3. Tür → platform tutarlı olmalı: `dmg`→macOS, `msi`→Windows, `AppImage`/`deb`→Linux.
4. Aynı platform+mimari+tür+yerel iki kez olamaz (`E_DUPLICATE`).
5. Zorunlu üçlü (macOS arm64 dmg, macOS x86_64 dmg, Windows x86_64 msi) eksikse
   manifest yazılmaz (`E_MISSING_PLATFORM`); `--require` ile değiştirilir.
6. Boş dosya artefakt sayılmaz (`E_EMPTY`).
7. `--source-manifest` verilirse sürüm ve commit **birebir** bağlanır
   (`E_SOURCE_LINK`); kaynak arşivinin tar özeti manifeste taşınır.
8. `signed`/`notarized` **iddia** edilemez: `--signed` yalnız bir attestation
   dosyası verildiğinde yazılır (`E_SIGNATURE_CLAIM`), aksi hâlde `false`.
9. Ürün adı NFKC'ye normalize edilir: macOS dosya adlarını ayrıştırılmış (NFD)
   yazabilir; `Dosya İnceleme Masası` ile `Dosya İnceleme Masası` aynı üründür.
10. Kimlik `contentSha256`dir: `generatedAt` dışındaki alanların kanonik JSON
    özeti. İki koşu aynı klasörde aynı kimliği verir; manifest elle düzenlenirse
    `verify` bunu yakalar (`E_MANIFEST_TAMPER`).

`verify`: indirilen tek dosyayı manifeste karşı doğrular — ad listede olmalı
(`E_ARTIFACT_UNLISTED`), **boyut** birebir (`E_SIZE`; kesilen indirme), sha256
birebir (`E_HASH`; kurcalama), beklenen platform/mimari (`E_PLATFORM`,
`E_ARCH`; yanlış mimari), beklenen sürüm (`E_VERSION`) ve istenirse boş disk
(`E_DISK`). Hiçbir adım "uyarı" değildir: ya geçer ya reddeder.

`--self-test` gerçek paketlere dokunmadan bütün kuralları fixture'larla sınar:
kusurlu her durum **kırmızı** olmalı.
"""

from __future__ import annotations

import argparse
import contextlib
import hashlib
import io
import json
import os
import re
import shutil
import sys
import tempfile
import unicodedata
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PRODUCT = "Dosya İnceleme Masası"
MANIFEST_VERSION = 1
COMMIT = re.compile(r"^[0-9a-f]{40}$")
SEMVER = re.compile(r"^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")

#: Dosya adındaki mimari belirteçleri → manifest mimarisi.
ARCH_TOKENS = {
    "aarch64": "aarch64",
    "arm64": "aarch64",
    "x64": "x86_64",
    "x86_64": "x86_64",
    "amd64": "x86_64",
    "universal": "universal",
}

#: Ad ayrıştırmasında mimari belirteci **kapalı küme**dir: `[A-Za-z0-9]+`
#: `x86_64`'ün alt çizgisini yerel ayırıcısıyla karıştırırdı.
_ARCH_PATTERN = "|".join(
    sorted((re.escape(token) for token in ARCH_TOKENS), key=len, reverse=True)
)

#: Paket türü → platform. Tür listesi kapalıdır: tanınmayan uzantı reddedilir.
KIND_PLATFORM = {
    "dmg": "macos",
    "msi": "windows",
    "appimage": "linux",
    "deb": "linux",
}

#: Varsayılan zorunlu üçlü: iki macOS mimarisi + Windows x64.
REQUIRED_DEFAULT = (
    ("macos", "aarch64", "dmg"),
    ("macos", "x86_64", "dmg"),
    ("windows", "x86_64", "msi"),
)

#: Artefakt olmayan, klasörde bulunmasına izin verilen yan dosyalar.
SIDECAR_SUFFIXES = (".sha256", ".json", ".md", ".txt")

#: Manifestin **kendi** `contentSha256`ı dosyanın içindedir: bir alan değiştirilip
#: özet yeniden hesaplanırsa `E_MANIFEST_TAMPER` yakalamaz. Bu yüzden manifest de
#: `.sha256` yan dosyasıyla yayımlanır (`build` yazar) ve `verify` **dışarıdan**
#: verilen özeti (`--manifest-sha256`) manifest hiç ayrıştırılmadan dosya
#: baytlarına karşı denetler — ayrı kanaldan gelen özet, manifestin kendisinden
#: bağımsız tek referanstır.
MANIFEST_SIDECAR_SUFFIX = ".sha256"

NAME = re.compile(
    r"^(?P<product>.+?)_(?P<version>\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?)"
    rf"_(?P<arch>{_ARCH_PATTERN})(?:_(?P<locale>[A-Za-z]{{2}}(?:-[A-Za-z]{{2,4}})?))?"
    r"\.(?P<kind>[A-Za-z][A-Za-z0-9]*)$",
    re.IGNORECASE,
)


class GateError(Exception):
    """Tek satırlık, kod taşıyan ret."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(f"{code}: {message}")
        self.code = code
        self.message = message


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def canonical(payload: dict) -> bytes:
    """Kimlik için kanonik JSON: anahtar sırası ve boşluklar sabit."""
    return json.dumps(
        payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")


def content_sha256(manifest: dict) -> str:
    """`generatedAt` ve `contentSha256` dışındaki alanların özeti: kimlik."""
    body = {
        key: value
        for key, value in manifest.items()
        if key not in ("generatedAt", "contentSha256")
    }
    return hashlib.sha256(canonical(body)).hexdigest()


def parse_artifact(name: str, expected_version: str) -> dict:
    """Dosya adını artefakt künyesine çevirir; tanınmayan ad reddedilir."""
    forced = unicodedata.normalize("NFKC", name)
    match = NAME.match(forced)
    if match is None:
        raise GateError("E_UNKNOWN_ARTIFACT", f"ad biçimi tanınmadı: {name}")

    product = unicodedata.normalize("NFKC", match.group("product"))
    if product != PRODUCT:
        raise GateError(
            "E_UNKNOWN_ARTIFACT", f"ürün adı beklenen {PRODUCT!r} değil: {name}"
        )

    version = match.group("version")
    if version != expected_version:
        raise GateError(
            "E_VERSION", f"dosyadaki sürüm {version}, beklenen {expected_version}: {name}"
        )

    raw_arch = match.group("arch").lower()
    arch = ARCH_TOKENS.get(raw_arch)
    if arch is None:
        raise GateError("E_UNKNOWN_ARTIFACT", f"mimari tanınmadı ({raw_arch}): {name}")

    raw_kind = match.group("kind")
    kind = raw_kind.lower()
    if kind not in KIND_PLATFORM:
        raise GateError("E_UNKNOWN_ARTIFACT", f"paket türü tanınmadı ({raw_kind}): {name}")

    locale = match.group("locale")
    return {
        "name": name,
        "platform": KIND_PLATFORM[kind],
        "arch": arch,
        "kind": kind,
        "locale": locale,
        "product": product,
        "version": version,
    }


def artifact_entries(directory: Path, version: str) -> list[dict]:
    """Klasördeki artefaktları okur; tanınmayan dosya manifesti düşürür."""
    if not directory.is_dir():
        raise GateError("E_ARTIFACTS_DIR", f"klasör yok: {directory}")

    entries: list[dict] = []
    seen: dict[tuple, str] = {}
    for path in sorted(directory.iterdir(), key=lambda item: item.name):
        if path.is_dir():
            raise GateError("E_UNKNOWN_ARTIFACT", f"alt klasör artefakt sayılmaz: {path.name}")
        if path.name.lower().endswith(SIDECAR_SUFFIXES):
            continue  # `.sha256`, bildirim/kaynak manifesti, README: artefakt değil
        meta = parse_artifact(path.name, version)
        size = path.stat().st_size
        if size <= 0:
            raise GateError("E_EMPTY", f"boş dosya: {path.name}")
        key = (meta["platform"], meta["arch"], meta["kind"], meta["locale"])
        if key in seen:
            raise GateError(
                "E_DUPLICATE",
                f"{key[0]}/{key[1]}/{key[2]} iki kez: {seen[key]} ve {path.name}",
            )
        seen[key] = path.name
        entries.append(
            {
                "name": meta["name"],
                "platform": meta["platform"],
                "arch": meta["arch"],
                "kind": meta["kind"],
                "locale": meta["locale"],
                "bytes": size,
                "sha256": sha256_file(path),
            }
        )
    if not entries:
        raise GateError("E_ARTIFACTS_DIR", f"klasörde artefakt yok: {directory}")
    return sorted(entries, key=lambda entry: (entry["platform"], entry["arch"], entry["kind"]))


def parse_require(values: list[str] | None) -> tuple[tuple[str, str, str], ...]:
    if not values:
        return REQUIRED_DEFAULT
    required: list[tuple[str, str, str]] = []
    for value in values:
        parts = value.split("/")
        if len(parts) != 3:
            raise GateError("E_USAGE", f"--require platform/mimari/tür olmalı: {value}")
        platform, arch, kind = parts
        if platform not in ("macos", "windows", "linux"):
            raise GateError("E_USAGE", f"bilinmeyen platform: {platform}")
        if arch not in ARCH_TOKENS.values():
            raise GateError("E_USAGE", f"bilinmeyen mimari: {arch}")
        if kind not in KIND_PLATFORM:
            raise GateError("E_USAGE", f"bilinmeyen paket türü: {kind}")
        required.append((platform, arch, kind))
    return tuple(required)


def read_source_link(path: Path, version: str, commit: str) -> dict:
    """Kaynak arşivi manifestine bağlanır: sürüm/commit birebir olmalı."""
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise GateError("E_SOURCE_LINK", f"kaynak manifesti okunamadı: {error}") from error
    if not isinstance(payload, dict):
        raise GateError("E_SOURCE_LINK", "kaynak manifesti nesne değil")
    source_version = payload.get("version")
    source_commit = payload.get("commit")
    if source_version != version:
        raise GateError(
            "E_SOURCE_LINK", f"kaynak sürümü {source_version}, paket sürümü {version}"
        )
    if source_commit != commit:
        raise GateError("E_SOURCE_LINK", f"kaynak commit {source_commit}, beklenen {commit}")
    archive = payload.get("archive") or {}
    tar_sha = archive.get("tarSha256")
    if not isinstance(tar_sha, str) or SHA256.match(tar_sha) is None:
        raise GateError("E_SOURCE_LINK", "kaynak manifestinde tar özeti yok")
    return {
        "manifest": path.name,
        "manifestSha256": sha256_file(path),
        "archive": archive.get("name"),
        "archiveSha256": archive.get("sha256"),
        "tarSha256": tar_sha,
        "commit": source_commit,
    }


def build_manifest(
    artifacts_dir: Path,
    version: str,
    commit: str,
    source_manifest: Path | None,
    require: list[str] | None,
    signed: bool,
    attestation: Path | None,
    generated_at: str | None = None,
) -> dict:
    """Manifesti üretir. Sıra: adlar → sürüm → zorunlular → imza → kaynak bağı."""
    if SEMVER.match(version) is None:
        raise GateError("E_USAGE", f"sürüm semver değil: {version}")
    if COMMIT.match(commit) is None:
        raise GateError("E_USAGE", f"commit 40 haneli hex değil: {commit}")

    entries = artifact_entries(artifacts_dir, version)
    required = parse_require(require)
    have = {(entry["platform"], entry["arch"], entry["kind"]) for entry in entries}
    missing = [f"{platform}/{arch}/{kind}" for platform, arch, kind in required if (platform, arch, kind) not in have]
    if missing:
        raise GateError("E_MISSING_PLATFORM", "eksik paket: " + ", ".join(sorted(missing)))

    signature = {"signed": False, "notarized": False}
    if signed or attestation is not None:
        if attestation is None or not attestation.is_file():
            raise GateError(
                "E_SIGNATURE_CLAIM",
                "--signed yalnız attestation dosyasıyla yazılır; imza iddiası kanıtsız olamaz",
            )
        signature = {
            "signed": True,
            "notarized": signed,
            "attestation": attestation.name,
            "attestationSha256": sha256_file(attestation),
        }

    source = None
    if source_manifest is not None:
        source = read_source_link(source_manifest, version, commit)

    manifest = {
        "manifestVersion": MANIFEST_VERSION,
        "product": PRODUCT,
        "version": version,
        "commit": commit,
        **signature,
        "source": source,
        "artifacts": entries,
        "generatedAt": generated_at
        or datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
    }
    manifest["contentSha256"] = content_sha256(manifest)
    return manifest


def write_manifest(path: Path, manifest: dict) -> str:
    """Manifesti **ve** `.sha256` yan dosyasını yazar; dosya özetini döndürür.

    Tek yazma yolu: yan dosya ile manifest aynı baytlardan türetilir, arada
    başka bir yazma olursa özet tutmaz (`E_MANIFEST_SIDECAR`).
    """
    body = json.dumps(manifest, ensure_ascii=False, indent=2) + "\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body, encoding="utf-8")
    digest = sha256_file(path)
    sidecar = path.with_name(path.name + MANIFEST_SIDECAR_SUFFIX)
    sidecar.write_text(f"{digest}  {path.name}\n", encoding="utf-8")
    return digest


def manifest_sidecar_path(manifest_path: Path) -> Path:
    return manifest_path.with_name(manifest_path.name + MANIFEST_SIDECAR_SUFFIX)


def check_manifest_digest(
    manifest_path: Path,
    manifest_sha256: str | None,
    require_manifest_digest: bool,
) -> str | None:
    """Manifestin **dosya** özetini denetler; manifesti ayrıştırmadan.

    Sıra: dışarıdan verilen özet → yan dosya → (istenirse) özet yokluğu.
    Dönüş: kullanılan özet ya da `None` (denetim istenmedi).
    """
    actual = sha256_file(manifest_path) if manifest_path.is_file() else None

    if manifest_sha256 is not None:
        wanted = manifest_sha256.strip().lower()
        if SHA256.match(wanted) is None:
            raise GateError("E_USAGE", f"dış özet 64 haneli hex değil: {manifest_sha256!r}")
        if actual is None:
            raise GateError("E_MISSING_FILE", f"manifest yok: {manifest_path}")
        if actual != wanted:
            raise GateError(
                "E_MANIFEST_DIGEST",
                f"manifest dosyası dış özetle uyuşmuyor: {actual[:12]}… ≠ {wanted[:12]}… "
                "— manifest değiştirilmiş (kendi contentSha256'sı bunu yakalayamaz)",
            )
        return actual

    sidecar = manifest_sidecar_path(manifest_path)
    if sidecar.is_file():
        parts = sidecar.read_text(encoding="utf-8", errors="replace").split()
        if not parts or SHA256.match(parts[0].lower()) is None:
            raise GateError("E_MANIFEST_SIDECAR", f"yan dosya okunamadı: {sidecar.name}")
        if actual is None:
            raise GateError("E_MISSING_FILE", f"manifest yok: {manifest_path}")
        if actual != parts[0].lower():
            raise GateError(
                "E_MANIFEST_SIDECAR",
                f"yan dosya {parts[0][:12]}… ≠ manifest {actual[:12]}… "
                "— manifest okunurken kesilmiş/bozulmuş olabilir",
            )
        return actual

    if require_manifest_digest:
        raise GateError(
            "E_MANIFEST_DIGEST",
            f"dış özet yok: --manifest-sha256 verilmedi ve {sidecar.name} bulunamadı",
        )
    return None


def load_manifest(path: Path) -> dict:
    try:
        manifest = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise GateError("E_MANIFEST", f"manifest okunamadı: {error}") from error
    if not isinstance(manifest, dict):
        raise GateError("E_MANIFEST", "manifest nesne değil")
    if manifest.get("manifestVersion") != MANIFEST_VERSION:
        raise GateError("E_MANIFEST", f"manifest sürümü {manifest.get('manifestVersion')}")
    expected = content_sha256(manifest)
    if manifest.get("contentSha256") != expected:
        raise GateError(
            "E_MANIFEST_TAMPER",
            "contentSha256 tutmuyor: manifest sonradan düzenlenmiş",
        )
    return manifest


def verify_file(
    manifest_path: Path,
    file_path: Path,
    platform: str | None,
    arch: str | None,
    version: str | None,
    require_free_disk: bool,
    *,
    manifest_sha256: str | None = None,
    require_manifest_digest: bool = False,
) -> dict:
    """İndirilen tek dosyayı manifeste karşı doğrular (F16-T09).

    Manifestin kendi `contentSha256`ı dosyanın içinde olduğu için tek başına
    yetmez; bu yüzden **dosya** özeti (dışarıdan verilen ya da yan dosyadaki)
    manifest ayrıştırılmadan önce denetlenir.
    """
    manifest_digest = check_manifest_digest(
        manifest_path, manifest_sha256, require_manifest_digest
    )
    manifest = load_manifest(manifest_path)

    if version is not None and version != manifest.get("version"):
        raise GateError(
            "E_VERSION", f"beklenen sürüm {version}, manifest {manifest.get('version')}"
        )

    entries = manifest.get("artifacts")
    if not isinstance(entries, list):
        raise GateError("E_MANIFEST", "artifacts listesi yok")
    entry = next((item for item in entries if item.get("name") == file_path.name), None)
    if entry is None:
        raise GateError("E_ARTIFACT_UNLISTED", f"dosya manifestte yok: {file_path.name}")

    if platform is not None and platform != entry.get("platform"):
        raise GateError(
            "E_PLATFORM",
            f"{file_path.name} {entry.get('platform')} için, beklenen {platform}",
        )
    if arch is not None and arch != entry.get("arch"):
        raise GateError(
            "E_ARCH", f"{file_path.name} {entry.get('arch')} için, beklenen {arch}"
        )

    if not file_path.is_file():
        raise GateError("E_MISSING_FILE", f"dosya yok: {file_path}")
    size = file_path.stat().st_size
    if size != entry.get("bytes"):
        raise GateError(
            "E_SIZE",
            f"{file_path.name}: {size} bayt, manifest {entry.get('bytes')} — indirme kesilmiş olabilir",
        )
    digest = sha256_file(file_path)
    if digest != entry.get("sha256"):
        raise GateError(
            "E_HASH", f"{file_path.name}: sha256 {digest[:12]}…, manifest {str(entry.get('sha256'))[:12]}…"
        )

    if require_free_disk:
        free = shutil.disk_usage(file_path.parent).free
        if free < int(entry.get("bytes", 0)):
            raise GateError(
                "E_DISK",
                f"boş disk {free} bayt < paket {entry.get('bytes')} bayt — kurulum reddedildi",
            )

    return {
        "name": file_path.name,
        "platform": entry.get("platform"),
        "arch": entry.get("arch"),
        "kind": entry.get("kind"),
        "bytes": size,
        "version": manifest.get("version"),
        "signed": manifest.get("signed") is True,
        "manifestSha256": manifest_digest,
    }


def summarize(manifest: dict) -> str:
    artifacts = manifest.get("artifacts", [])
    order = {"macos": 0, "windows": 1, "linux": 2}
    parts = [
        f"{entry['platform']}/{entry['arch']}/{entry['kind']}"
        for entry in sorted(
            artifacts, key=lambda item: (order.get(item["platform"], 9), item["arch"], item["kind"])
        )
    ]
    return (
        f"sürüm {manifest.get('version')} · {len(artifacts)} paket ({', '.join(parts)}) · "
        f"imza {'var' if manifest.get('signed') else 'yok'} · "
        f"kimlik {str(manifest.get('contentSha256'))[:12]}…"
    )


# ---------------------------------------------------------------- öz test ---


def _fixture_artifact(directory: Path, name: str, body: str) -> Path:
    path = directory / name
    path.write_bytes((body * 97).encode("utf-8")[:4096])
    return path


def _expect(code: str, call) -> str:
    """`call` çağrısı `code` ile reddetmeli; aksi hâlde öz test kırmızı."""
    try:
        call()
    except GateError as error:
        if error.code != code:
            return f"beklenen {code}, gelen {error.code} ({error.message})"
        return ""
    return f"beklenen {code}, ret yok"


def self_test() -> int:
    failures: list[str] = []
    checks: list[str] = []

    def ok(line: str) -> None:
        """Geçen kuralı **anında** yazar: mutasyon kanıtı tek satırın düşmesini ölçer."""
        checks.append(line)
        print("ok:", line)

    def note(line: str) -> None:
        """Ölçüm notu: kapı değil, sayılmaz (kusuru pinlememek için)."""
        print("ölçü:", line)

    with tempfile.TemporaryDirectory(prefix="dimrel-") as raw:
        base = Path(raw)
        artifacts = base / "artifacts"
        artifacts.mkdir()
        dmg_arm = _fixture_artifact(artifacts, f"{PRODUCT}_0.1.0_aarch64.dmg", "arm")
        dmg_x64 = _fixture_artifact(artifacts, f"{PRODUCT}_0.1.0_x64.dmg", "x64")
        msi = _fixture_artifact(artifacts, f"{PRODUCT}_0.1.0_x64_tr-TR.msi", "msi")
        (artifacts / f"{PRODUCT}_0.1.0_x64.dmg.sha256").write_text("ölçü değil yan dosya\n")

        commit = "a" * 40
        manifest_path = base / "release-manifest.json"
        manifest = build_manifest(artifacts, "0.1.0", commit, None, None, False, None)
        manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")

        # 1) Mutlu yol: üç paket, imza iddiası yok, kimlik var.
        if len(manifest["artifacts"]) != 3:
            failures.append("G1 mutlu yol: üç artefakt bekleniyordu")
        else:
            ok("G1 üç paket (macos/aarch64, macos/x86_64, windows/x86_64)")
        if manifest["signed"] is not False or manifest["notarized"] is not False:
            failures.append("G1b imza iddiası kanıtsız yazıldı")
        else:
            ok("G1b imza/notarize kanıtsız false")

        # 2) Kimlik determinizmi: `generatedAt` dışı aynı → aynı contentSha256.
        again = build_manifest(
            artifacts, "0.1.0", commit, None, None, False, None, "2030-01-01T00:00:00Z"
        )
        if again["contentSha256"] != manifest["contentSha256"]:
            failures.append("G2 iki koşunun kimliği farklı")
        else:
            ok("G2 kimlik (contentSha256) `generatedAt`ten bağımsız")

        # 3) Gerçek kanıt adı (F02/F17): tr-TR yereli ayrışır.
        msi_entry = next(item for item in manifest["artifacts"] if item["kind"] == "msi")
        if msi_entry["locale"] != "tr-TR" or msi_entry["platform"] != "windows":
            failures.append("G3 gerçek MSI adı yanlış çözüldü")
        else:
            ok("G3 gerçek ad `..._x64_tr-TR.msi` → windows/x86_64/tr-TR")

        # 4) macOS NFD ürün adı aynı üründür.
        nfd = unicodedata.normalize("NFD", f"{PRODUCT}_0.1.0_aarch64.dmg")
        try:
            parse_artifact(nfd, "0.1.0")
            ok("G4 NFD (macOS) ürün adı NFKC ile eşleşti")
        except GateError as error:
            failures.append(f"G4 NFD ad reddedildi: {error.message}")

        # 5) İndirme doğrulaması: mutlu yol.
        try:
            outcome = verify_file(manifest_path, msi, None, None, None, False)
            ok(f"G5 manifest + dosya doğrulandı ({outcome['name']})")
        except GateError as error:
            failures.append(f"G5 mutlu doğrulama düştü: {error.message}")

        # 6) Kesilen indirme: boyut farkı **E_SIZE** ile reddedilir.
        truncated = base / "truncated" / msi.name
        truncated.parent.mkdir()
        truncated.write_bytes(msi.read_bytes()[:100])
        message = _expect("E_SIZE", lambda: verify_file(manifest_path, truncated, None, None, None, False))
        if message:
            failures.append(f"G6 kesilen indirme: {message}")
        else:
            ok("G6 kesilen indirme E_SIZE")

        # 7) Kurcalanmış dosya (aynı boyut): sha256 reddeder.
        tampered = base / "tampered" / msi.name
        tampered.parent.mkdir()
        data = bytearray(msi.read_bytes())
        data[0] ^= 0xFF
        tampered.write_bytes(bytes(data))
        message = _expect("E_HASH", lambda: verify_file(manifest_path, tampered, None, None, None, False))
        if message:
            failures.append(f"G7 kurcalama: {message}")
        else:
            ok("G7 kurcalanmış paket E_HASH")

        # 8) Yanlış mimari: arm64 dmg'yi x86_64 diye kurmak.
        message = _expect(
            "E_ARCH", lambda: verify_file(manifest_path, dmg_arm, None, "x86_64", None, False)
        )
        if message:
            failures.append(f"G8 yanlış mimari: {message}")
        else:
            ok("G8 yanlış mimari E_ARCH")

        # 9) Yanlış platform ve yanlış sürüm beklentisi.
        message = _expect(
            "E_PLATFORM", lambda: verify_file(manifest_path, msi, "macos", None, None, False)
        )
        if message:
            failures.append(f"G9 yanlış platform: {message}")
        else:
            ok("G9 yanlış platform E_PLATFORM")
        message = _expect(
            "E_VERSION", lambda: verify_file(manifest_path, msi, None, None, "9.9.9", False)
        )
        if message:
            failures.append(f"G9b yanlış sürüm: {message}")
        else:
            ok("G9b yanlış sürüm E_VERSION")

        # 10) Manifestte olmayan dosya.
        stray = _fixture_artifact(base, f"{PRODUCT}_0.1.0_x64.deb", "deb")
        message = _expect("E_ARTIFACT_UNLISTED", lambda: verify_file(manifest_path, stray, None, None, None, False))
        if message:
            failures.append(f"G10 listede olmayan dosya: {message}")
        else:
            ok("G10 listede olmayan dosya E_ARTIFACT_UNLISTED")

        # 11) Manifest elle düzenlenirse kimlik tutmaz.
        edited = base / "edited.json"
        payload = json.loads(manifest_path.read_text(encoding="utf-8"))
        payload["artifacts"][0]["bytes"] = 1
        edited.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
        message = _expect("E_MANIFEST_TAMPER", lambda: verify_file(edited, msi, None, None, None, False))
        if message:
            failures.append(f"G11 manifest kurcalama: {message}")
        else:
            ok("G11 manifest kurcalama E_MANIFEST_TAMPER")

        # 12) Boş dosya artefakt değildir.
        empty_dir = base / "empty"
        empty_dir.mkdir()
        (empty_dir / f"{PRODUCT}_0.1.0_x64.dmg").write_bytes(b"")
        message = _expect(
            "E_EMPTY", lambda: artifact_entries(empty_dir, "0.1.0")
        )
        if message:
            failures.append(f"G12 boş dosya: {message}")
        else:
            ok("G12 boş dosya E_EMPTY")

        # 13) Tanınmayan dosya sessizce atlanmaz.
        stray_dir = base / "stray"
        stray_dir.mkdir()
        shutil.copy(msi, stray_dir / msi.name)
        (stray_dir / "notlar.txt").write_text("yan dosya", encoding="utf-8")
        (stray_dir / "kurulum.exe").write_bytes(b"x")
        message = _expect("E_UNKNOWN_ARTIFACT", lambda: artifact_entries(stray_dir, "0.1.0"))
        if message:
            failures.append(f"G13 tanınmayan dosya: {message}")
        else:
            ok("G13 tanınmayan dosya E_UNKNOWN_ARTIFACT (yan dosya serbest)")

        # 14) Dosyadaki sürüm ile manifest sürümü ayrışamaz.
        wrong_dir = base / "wrong"
        wrong_dir.mkdir()
        shutil.copy(msi, wrong_dir / msi.name)
        message = _expect("E_VERSION", lambda: artifact_entries(wrong_dir, "0.2.0"))
        if message:
            failures.append(f"G14 sürüm ayrışması: {message}")
        else:
            ok("G14 dosya sürümü ≠ manifest sürümü E_VERSION")

        # 15) Aynı platform/mimari/tür iki kez olamaz: `x64` ile `x86_64`
        # aynı mimariye çözülür → ikinci dosya yinelenmiş sayılır.
        dupe_dir = base / "dupe"
        dupe_dir.mkdir()
        shutil.copy(msi, dupe_dir / msi.name)
        shutil.copy(msi, dupe_dir / f"{PRODUCT}_0.1.0_x86_64_tr-TR.msi")
        message = _expect("E_DUPLICATE", lambda: artifact_entries(dupe_dir, "0.1.0"))
        if message:
            failures.append(f"G15 yinelenen paket: {message}")
        else:
            ok("G15 yinelenen platform/mimari/tür E_DUPLICATE (`x64` ≡ `x86_64`)")

        # 16) Eksik zorunlu platform manifesti düşürür.
        partial = base / "partial"
        partial.mkdir()
        shutil.copy(msi, partial / msi.name)
        message = _expect(
            "E_MISSING_PLATFORM",
            lambda: build_manifest(partial, "0.1.0", commit, None, None, False, None),
        )
        if message:
            failures.append(f"G16 eksik platform: {message}")
        else:
            ok("G16 eksik zorunlu paket E_MISSING_PLATFORM")

        # 17) Kanıtsız imza iddiası yazılamaz; attestation ile yazılır.
        message = _expect(
            "E_SIGNATURE_CLAIM",
            lambda: build_manifest(artifacts, "0.1.0", commit, None, None, True, None),
        )
        if message:
            failures.append(f"G17 kanıtsız imza: {message}")
        else:
            ok("G17 kanıtsız imza E_SIGNATURE_CLAIM")
        attestation = base / "attestation.json"
        attestation.write_text('{"issuer":"ornek"}\n', encoding="utf-8")
        signed = build_manifest(artifacts, "0.1.0", commit, None, None, True, attestation)
        if signed["signed"] is not True or not signed.get("attestationSha256"):
            failures.append("G17b attestation ile imza yazılmadı")
        else:
            ok("G17b attestation varsa imza + özeti yazılır")

        # 18) Kaynak arşivi bağı: sürüm/commit birebir olmalı.
        source = base / "source.manifest.json"
        source.write_text(
            json.dumps(
                {
                    "version": "0.1.0",
                    "commit": commit,
                    "archive": {"name": "dim-source-0.1.0-aaaa.tar.gz", "sha256": "b" * 64, "tarSha256": "c" * 64},
                }
            ),
            encoding="utf-8",
        )
        linked = build_manifest(artifacts, "0.1.0", commit, source, None, False, None)
        if not linked["source"] or linked["source"]["tarSha256"] != "c" * 64:
            failures.append("G18 kaynak bağı taşınmadı")
        else:
            ok("G18 kaynak manifesti (tar özeti + commit) bağlandı")
        drifled = base / "source-drift.json"
        drifled.write_text(
            json.dumps({"version": "0.1.0", "commit": "d" * 40, "archive": {"tarSha256": "c" * 64}}),
            encoding="utf-8",
        )
        message = _expect(
            "E_SOURCE_LINK",
            lambda: build_manifest(artifacts, "0.1.0", commit, drifled, None, False, None),
        )
        if message:
            failures.append(f"G18b kaynak commit kayması: {message}")
        else:
            ok("G18b kaynak commit kayması E_SOURCE_LINK")

        # 19) `require` ile zorunlu küme değişir; tanınmayan tür reddedilir.
        linux = build_manifest(
            artifacts, "0.1.0", commit, None, ["macos/aarch64/dmg"], False, None
        )
        if len(linux["artifacts"]) != 3:
            failures.append("G19 require zorunlu kümeyi değiştirmedi")
        else:
            ok("G19 `--require` zorunlu kümeyi değiştirir")
        message = _expect("E_USAGE", lambda: parse_require(["macos/aarch64/exe"]))
        if message:
            failures.append(f"G19b tanınmayan tür: {message}")
        else:
            ok("G19b tanınmayan `--require` türü E_USAGE")

        # 20) Boş disk kapısı: paket boyutundan az boş alan reddeder.
        message = _expect(
            "E_DISK",
            lambda: verify_file(manifest_path, msi, None, None, None, True),
        )
        # Bu makinede genelde yer vardır; kapı yalnız gerçekten darsa kırmızı olur.
        if message and "ret yok" in message:
            ok("G20 boş disk kapısı (yer yeterli, red yok)")
        elif message:
            failures.append(f"G20 boş disk kapısı beklenmeyen kod: {message}")
        else:
            ok("G20 boş disk kapısı yetersizse E_DISK")

        # 21) Manifest `.sha256` yan dosyasıyla yazılır; yan dosya **baytları** özetler.
        paired_dir = base / "paired"
        paired = paired_dir / "release-manifest.json"
        paired_digest = write_manifest(paired, manifest)
        sidecar = manifest_sidecar_path(paired)
        if not sidecar.is_file():
            failures.append("G21 `.sha256` yan dosyası yazılmadı")
        elif sidecar.read_text(encoding="utf-8").split()[:1] != [paired_digest]:
            failures.append("G21 yan dosyadaki özet `write_manifest` dönüşüyle uyuşmuyor")
        elif sha256_file(paired) != paired_digest:
            failures.append("G21 yan dosya manifestin gerçek baytlarını özetlemiyor")
        else:
            ok("G21 `build` manifesti + `.sha256` yan dosyası (dosya baytı) yazar")

        # 22) Kurcalanmış manifest, `contentSha256` **geri hesaplanmış** olsa da
        #     yayımlanmış yan dosya özetiyle düşer. İç özet tek başına yetmez.
        edited_dir = base / "edited"
        edited_dir.mkdir()
        edited = edited_dir / paired.name
        tampered = json.loads(json.dumps(manifest))
        tampered["artifacts"][2]["bytes"] = int(tampered["artifacts"][2]["bytes"]) + 4096
        tampered["contentSha256"] = content_sha256(tampered)
        edited.write_text(
            json.dumps(tampered, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )
        note(
            "geri hesaplanmış contentSha256'lı manifest kendi iç özetiyle tutarlı görünür "
            "(`E_MANIFEST_TAMPER` yakalamaz) — dış özet gerekir"
        )
        shutil.copy(sidecar, manifest_sidecar_path(edited))
        message = _expect(
            "E_MANIFEST_SIDECAR",
            lambda: verify_file(edited, msi, None, None, None, False),
        )
        if message:
            failures.append(f"G22 kurcalanmış manifest (yan dosya): {message}")
        else:
            ok("G22 kurcalanmış + geri hesaplanmış manifest E_MANIFEST_SIDECAR")

        # 23) Dışarıdan verilen özet (yayın duyurusu) uyuşmazsa manifest okunmadan düşer.
        message = _expect(
            "E_MANIFEST_DIGEST",
            lambda: verify_file(
                edited, msi, None, None, None, False, manifest_sha256=paired_digest
            ),
        )
        if message:
            failures.append(f"G23 dış özet uyuşmazlığı: {message}")
        else:
            ok("G23 `--manifest-sha256` uyuşmazsa E_MANIFEST_DIGEST")

        # 24) Özet ne dışarıdan ne yan dosyadan geldiyse ve zorunluysa: sessiz kabul yok.
        lone = base / "lone" / "release-manifest.json"
        lone.parent.mkdir()
        lone.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        message = _expect(
            "E_MANIFEST_DIGEST",
            lambda: verify_file(lone, msi, None, None, None, False, require_manifest_digest=True),
        )
        if message:
            failures.append(f"G24 özet yokluğu: {message}")
        else:
            ok("G24 `--require-manifest-digest`: özet yoksa E_MANIFEST_DIGEST")

        # 25) Bozuk özet biçimi sessizce yorumlanmaz.
        message = _expect(
            "E_USAGE",
            lambda: verify_file(paired, msi, None, None, None, False, manifest_sha256="abc"),
        )
        if message:
            failures.append(f"G25 bozuk dış özet: {message}")
        else:
            ok("G25 64 haneli olmayan dış özet E_USAGE")

        # 26) Özet yoksa `verify` **sessiz** kalmaz: uyarı yazar (kabul eder ama yalan söylemez).
        err = io.StringIO()
        with contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()):
            code = main(["release_manifest.py", "verify", "--manifest", str(lone), "--file", str(msi)])
        if code != 0:
            failures.append(f"G26 özetsiz doğrulama {code} koduyla döndü (beklenen 0)")
        elif "UYARI" not in err.getvalue():
            failures.append("G26 özetsiz doğrulama uyarı yazmadı (sessiz kabul)")
        else:
            ok("G26 özetsiz doğrulama uyarır (sessiz kabul yok)")

    if failures:
        print("paket manifesti öz testi KIRMIZI:")
        for failure in failures:
            print(" -", failure)
        return 1
    print(f"paket manifesti öz testi yeşil ({len(checks)} kural)")
    return 0


# ------------------------------------------------------------------- CLI ---


def main(argv: list[str]) -> int:
    if "--self-test" in argv[1:]:
        return self_test()

    parser = argparse.ArgumentParser(
        prog="release_manifest.py",
        description="Sürüm paketi manifesti üretir ve indirilen paketi doğrular.",
    )
    sub = parser.add_subparsers(dest="command", required=True)

    build = sub.add_parser("build", help="artefakt klasöründen manifest üret")
    build.add_argument("--artifacts", required=True, type=Path)
    build.add_argument("--version", required=True)
    build.add_argument("--commit", required=True)
    build.add_argument("--source-manifest", type=Path, default=None)
    build.add_argument("--require", action="append", default=None, metavar="PLATFORM/MIMARI/TÜR")
    build.add_argument("--signed", action="store_true")
    build.add_argument("--attestation", type=Path, default=None)
    build.add_argument("--out", required=True, type=Path)

    verify = sub.add_parser("verify", help="indirilen paketi manifeste karşı doğrula")
    verify.add_argument("--manifest", required=True, type=Path)
    verify.add_argument("--file", required=True, type=Path)
    verify.add_argument("--platform", choices=["macos", "windows", "linux"], default=None)
    verify.add_argument("--arch", choices=sorted(set(ARCH_TOKENS.values())), default=None)
    verify.add_argument("--version", default=None)
    verify.add_argument("--require-free-disk", action="store_true")
    verify.add_argument(
        "--manifest-sha256",
        default=None,
        metavar="HEX",
        help="manifestin yayımlanmış (ayrı kanaldan gelen) dosya özeti",
    )
    verify.add_argument(
        "--require-manifest-digest",
        action="store_true",
        help="ne dış özet ne de .sha256 yan dosyası varsa reddet",
    )

    args = parser.parse_args(argv[1:])
    try:
        if args.command == "build":
            manifest = build_manifest(
                args.artifacts,
                args.version,
                args.commit,
                args.source_manifest,
                args.require,
                args.signed,
                args.attestation,
            )
            digest = write_manifest(args.out, manifest)
            print(f"manifest yazıldı: {args.out} · {summarize(manifest)}")
            print(f"manifest özeti (dışarıya ilan edilecek): {digest}")
            print(f"yan dosya: {manifest_sidecar_path(args.out).name}")
            return 0
        outcome = verify_file(
            args.manifest,
            args.file,
            args.platform,
            args.arch,
            args.version,
            args.require_free_disk,
            manifest_sha256=args.manifest_sha256,
            require_manifest_digest=args.require_manifest_digest,
        )
        print(
            f"paket doğrulandı: {outcome['name']} · {outcome['platform']}/{outcome['arch']}/"
            f"{outcome['kind']} · {outcome['bytes']} bayt · sürüm {outcome['version']} · "
            f"imza {'var' if outcome['signed'] else 'yok'}"
        )
        if outcome["manifestSha256"]:
            print(f"manifest özeti doğrulandı: {outcome['manifestSha256']}")
        else:
            print(
                "UYARI: manifest özeti doğrulanmadı (--manifest-sha256 ya da .sha256 yan dosyası "
                "yok) — paket ikamesi bu çağrıda yakalanmaz",
                file=sys.stderr,
            )
        return 0
    except GateError as error:
        print(f"KIRMIZI {error.code}: {error.message}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
