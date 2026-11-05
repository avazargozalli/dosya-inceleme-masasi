#!/usr/bin/env python3
"""AGPL kaynak arşivi — üret, ürüne bağla, bağımsız doğrula.

Neden: F16-T01 "kaynak etiketi, kilit dosyaları, SBOM ve üçüncü taraf
bildirimlerini **aynı sürüme bağla**" maddesinin tek eksik parçası kaynak
etiketi/arşiviydi. Depoda hiç git etiketi yok ve kaynak arşivi üreten bir araç
yoktu: ikili paketle birlikte verilecek "buna karşılık gelen kaynak" hiçbir
zaman üretilmiyordu (`git archive` hiç çağrılmıyor). Bu betik o boşluğu
kapatır ve **kendi kendini kandırmaz**:

1. Arşiv **commit ağacından** üretilir (`git archive`), çalışma ağacından
   değil → commit'lenmemiş/yerel dosya arşive sızmaz.
2. Üretim deterministiktir: aynı commit iki kez arşivlenince **bayt bayt
   aynı** çıktı ve aynı sha256 (gzip başlığı `mtime=0`, sıra commit'ten).
3. Manifest **ürünü bağlar**: sürüm (üç kaynakta aynı olmalı), tam commit,
   etiket (varsa ve etiket gerçekten o commit'e bakıyorsa), arşivdeki kilit
   dosyalarının sha256'sı, `THIRD_PARTY_NOTICES.md` özeti ve verilmişse
   SBOM'un subject'i (commit + sürüm + kilit özetleri birebir).
4. `check` arşivi **bağımsız** doğrular: sha256, dosya kümesi = commit'in
   ağacı (`git ls-tree -r`), kilit özetleri (arşivin **içinden** okunur),
   zorunlu kaynak/lisans dosyaları, etiket bağı, sır/çöp yolu reddi.
5. Sır sızıntısı kapısı: arşivde `.env`, `id_rsa`, `*.pem`, `*.key`, `.venv`,
   `node_modules` gibi yollar varsa üretim **durur** — "kaynak teslimi"
   nedeniyle yayımlanacak ağaçta sır olmadığı ölçülür, varsayılmaz.

Kullanım:
    # yayın anında (etiket zorunlu, SBOM ve bildirim bağı doğrulanır)
    bash scripts/sbom.sh
    git tag -a v0.1.0 -m "kaynak sürümü 0.1.0"
    python3 scripts/source_release.py build --tag v0.1.0 --sbom sbom.json --out dist
    python3 scripts/source_release.py check dist/dim-source-0.1.0-<kısa>.tar.gz --sbom sbom.json

    # CI her push'ta (etiket yok): üretimi ve doğrulamayı gerçek repoda koşar
    python3 scripts/source_release.py build --ref HEAD --out "$RUNNER_TEMP/kaynak" --allow-untagged
    python3 scripts/source_release.py check "$RUNNER_TEMP/kaynak"/*.tar.gz

    # kapının kendisi (fixture deposu; gerçek repoya dokunmaz)
    python3 scripts/source_release.py --self-test
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import zlib
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Literal, overload

ROOT = Path(__file__).resolve().parent.parent
SCHEMA = "dim-source-release"
SCHEMA_VERSION = 1

# Arşivde **bulunması zorunlu** kaynak/lisans dosyaları (kaynak teslimi ve
# yeniden derleme için). Liste bilinçli olarak "en az bunlar": commit ağacının
# tamamı zaten dosya kümesi karşılaştırmasıyla doğrulanıyor.
REQUIRED_FILES = (
    "LICENSE",
    "NOTICE",
    "THIRD_PARTY_NOTICES.md",
    "README.md",
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "pnpm-lock.yaml",
    "pnpm-workspace.yaml",
    "apps/desktop/package.json",
    "apps/desktop/src-tauri/Cargo.toml",
    "apps/desktop/src-tauri/tauri.conf.json",
    "packages/contracts/package.json",
    "workers/document/pyproject.toml",
    "docs/spec/DOSYA_INCELEME_MASASI_TAM_PROJE_PLANI.md",
    "scripts/sbom.sh",
    "scripts/sbom_check.py",
    "scripts/source_release.py",
    "scripts/third_party_notices.py",
)

# Arşivde asla bulunmaması gereken yollar: sır dosyaları ve yeniden üretilebilir
# çıktılar. `git archive` yalnız izlenen dosyaları alır; yine de izlenen bir sır
# dosyası gerçek bir sızıntıdır → üretim durur.
DENY_NAMES = frozenset(
    {
        ".env",
        ".env.local",
        ".env.production",
        "id_rsa",
        "id_dsa",
        "id_ecdsa",
        "id_ed25519",
        "credentials.json",
        "secrets.toml",
        ".netrc",
    }
)
DENY_SUFFIXES = (".pem", ".key", ".p12", ".pfx", ".keystore", ".jks", ".mobileprovision")
DENY_SEGMENTS = frozenset({".venv", "node_modules", "target", ".git", "__pycache__"})

# Sürümün okunduğu üç uygulama kaynağı. Kök `package.json` monorepo sürümüdür
# (0.2.0) ve bilinçli olarak bu bağa girmez — dürüst sınırlar belgede yazılı.
VERSION_SOURCES = (
    "apps/desktop/src-tauri/tauri.conf.json",
    "apps/desktop/package.json",
    "packages/contracts/package.json",
)

# Arşivdeki kilit dosyaları (iki cargo çalışma alanı + node + python).
LOCKFILES = (
    "Cargo.lock",
    "apps/desktop/src-tauri/Cargo.lock",
    "pnpm-lock.yaml",
    "workers/document/pyproject.toml",
)


class ReleaseError(Exception):
    """Üretim/doğrulama durduruldu; mesaj kullanıcıya olduğu gibi gösterilir."""


@dataclass
class Repo:
    """Bir git deposu; `root` fixture testlerinde geçici depoyu gösterir."""

    root: Path

    @overload
    def git(self, *args: str, binary: Literal[False] = False) -> str: ...

    @overload
    def git(self, *args: str, binary: Literal[True]) -> bytes: ...

    def git(self, *args: str, binary: bool = False) -> str | bytes:
        proc = subprocess.run(
            ["git", *args],
            cwd=self.root,
            capture_output=True,
            check=False,
        )
        if proc.returncode != 0:
            stderr = proc.stderr.decode("utf-8", "replace").strip()
            raise ReleaseError(f"git {' '.join(args)} düştü: {stderr}")
        return proc.stdout if binary else proc.stdout.decode("utf-8", "replace").strip()

    def commit_of(self, ref: str) -> str:
        return self.git("rev-parse", "--verify", f"{ref}^{{commit}}")

    def tags_at(self, commit: str) -> list[str]:
        out = self.git("tag", "--points-at", commit)
        return sorted(line for line in out.splitlines() if line.strip())

    def tag_commit(self, tag: str) -> str:
        return self.git("rev-parse", "--verify", f"refs/tags/{tag}^{{commit}}")

    def show(self, commit: str, path: str) -> bytes:
        return self.git("show", f"{commit}:{path}", binary=True)

    def tree(self, commit: str) -> list[str]:
        # `-z`: git, ASCII dışı yolları varsayılan olarak C tarzı tırnaklar
        # (`"docs/adr/\303\256…"`); tar üye adları ise ham UTF-8'dir. Türkçe
        # dosya adları bu yüzden "fazla/eksik" gibi görünür → NUL ayırıcı.
        out = self.git("ls-tree", "-r", "-z", "--name-only", commit, binary=True)
        return sorted(part.decode("utf-8") for part in out.split(b"\0") if part)


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def gzip_deterministic(raw: bytes) -> bytes:
    """Sabit başlıklı gzip: `mtime=0`, OS baytı zlib'in sabit değeri.

    `gzip.compress(..., mtime=0)` de deterministik ama sürümler arasında başlık
    baytı değişebiliyor; ham zlib sarmalayıcısı tek davranış verir.
    """
    engine = zlib.compressobj(9, zlib.DEFLATED, 16 + zlib.MAX_WBITS)
    return engine.compress(raw) + engine.flush()


def denied_reason(path: str) -> str | None:
    """Arşiv yolunda sır/çöp izi varsa nedenini döndürür."""
    segments = [part for part in path.split("/") if part]
    if not segments:
        return None
    name = segments[-1]
    if name in DENY_NAMES:
        return "sır dosyası adı"
    if name.lower().endswith(DENY_SUFFIXES):
        return "sır dosyası uzantısı"
    for part in segments[:-1]:
        if part in DENY_SEGMENTS:
            return f"yeniden üretilebilir/çalışma dizini ({part})"
    return None


def versions_from(read) -> dict[str, str]:
    """Sürümü üç uygulama kaynağından okur; `read(path) -> bytes` enjekte edilir."""
    found: dict[str, str] = {}
    for path in VERSION_SOURCES:
        raw = read(path)
        try:
            if path.endswith(".json"):
                data = json.loads(raw.decode("utf-8"))
                version = data.get("version")
            else:  # pragma: no cover - bugün yalnız JSON kaynaklar var
                version = tomllib.loads(raw.decode("utf-8"))["package"]["version"]
        except (UnicodeDecodeError, json.JSONDecodeError, KeyError) as exc:
            raise ReleaseError(f"{path}: sürüm okunamadı ({exc})") from exc
        if not isinstance(version, str) or not version.strip():
            raise ReleaseError(f"{path}: sürüm alanı yok")
        found[path] = version.strip()
    cargo = tomllib.loads(read("Cargo.toml").decode("utf-8"))
    workspace_version = (cargo.get("workspace") or {}).get("package", {}).get("version")
    if not isinstance(workspace_version, str) or not workspace_version.strip():
        raise ReleaseError("Cargo.toml: [workspace.package].version yok")
    found["Cargo.toml"] = workspace_version.strip()
    return found


def require_single_version(found: dict[str, str]) -> str:
    unique = sorted(set(found.values()))
    if len(unique) != 1:
        lines = ", ".join(f"{path}={version}" for path, version in sorted(found.items()))
        raise ReleaseError(f"sürüm kaynakları ayrışıyor ({lines}); tek sürüm gerekir")
    return unique[0]


def tar_members(raw: bytes) -> dict[str, bytes]:
    """Arşivdeki **dosyalar** (dizinler hariç) yol → bayt."""
    members: dict[str, bytes] = {}
    with tarfile.open(fileobj=io.BytesIO(raw), mode="r:") as archive:
        for entry in archive.getmembers():
            if entry.isdir():
                continue
            if not entry.isfile():
                raise ReleaseError(f"arşivde dosya olmayan öğe: {entry.name}")
            handle = archive.extractfile(entry)
            if handle is None:  # pragma: no cover - tarfile sözleşmesi
                raise ReleaseError(f"arşiv öğesi okunamadı: {entry.name}")
            members[entry.name] = handle.read()
    return members


def strip_prefix(members: dict[str, bytes], prefix: str) -> dict[str, bytes]:
    stripped: dict[str, bytes] = {}
    for name, data in members.items():
        if not name.startswith(prefix):
            raise ReleaseError(f"arşivde beklenen kök yok: {name!r} (beklenen {prefix!r})")
        stripped[name[len(prefix) :]] = data
    return stripped


def sbom_binding(sbom: dict, commit: str, version: str, lockfiles: dict[str, str]) -> list[str]:
    """SBOM `subject` bloğunu arşive bağlar; sorun listesi döner (boş = bağlı)."""
    problems: list[str] = []
    subject = sbom.get("subject")
    if not isinstance(subject, dict):
        return ["SBOM'da subject bloğu yok"]
    if subject.get("commit") != commit:
        problems.append(f"SBOM commit {subject.get('commit')!r} ≠ arşiv commit {commit}")
    for field in ("version", "bundleVersion", "cargoVersion"):
        if subject.get(field) != version:
            problems.append(f"SBOM subject.{field} {subject.get(field)!r} ≠ sürüm {version}")
    listed = subject.get("lockfiles")
    if not isinstance(listed, dict) or not listed:
        problems.append("SBOM subject.lockfiles boş")
        return problems
    for path, digest in sorted(listed.items()):
        if path not in lockfiles:
            problems.append(f"SBOM'da kilit dosyası arşivde yok: {path}")
        elif lockfiles[path] != digest:
            problems.append(f"SBOM kilit özeti uyuşmuyor: {path}")
    for path in sorted(lockfiles):
        if path not in listed:
            problems.append(f"arşivdeki kilit dosyası SBOM'da yok: {path}")
    return problems


def build(
    repo: Repo,
    ref: str,
    out_dir: Path,
    tag: str | None = None,
    sbom_path: Path | None = None,
    allow_untagged: bool = False,
) -> tuple[Path, dict]:
    """Arşivi commit ağacından üretir, manifesti yazar; (yol, manifest) döner."""
    commit = repo.commit_of(ref)
    short = commit[:8]
    versions = versions_from(lambda path: repo.show(commit, path))
    version = require_single_version(versions)

    if tag is not None:
        if repo.tag_commit(tag) != commit:
            raise ReleaseError(
                f"etiket {tag!r} bu commit'e bakmıyor (etiket {repo.tag_commit(tag)[:12]}, "
                f"arşiv {commit[:12]})"
            )
        chosen_tag: str | None = tag
    else:
        tags = repo.tags_at(commit)
        chosen_tag = tags[0] if tags else None
        if chosen_tag is None and not allow_untagged:
            raise ReleaseError(
                "commit etiketsiz: yayın için `git tag -a v<ürün> -m …` ile etiketleyip "
                "`--tag <ad>` verin (CI gibi geçici üretimde `--allow-untagged`)"
            )

    prefix = f"dim-source-{version}-{short}/"
    raw = repo.git("archive", "--format=tar", f"--prefix={prefix}", commit, binary=True)
    members = tar_members(raw)

    for name in sorted(members):
        reason = denied_reason(name[len(prefix) :] if name.startswith(prefix) else name)
        if reason:
            raise ReleaseError(f"arşivde yayımlanmaması gereken yol: {name} ({reason})")

    inside = strip_prefix(members, prefix)
    missing = sorted(set(REQUIRED_FILES) - set(inside))
    if missing:
        raise ReleaseError(f"arşivde zorunlu kaynak dosyası yok: {', '.join(missing)}")

    lockfiles = {path: sha256_bytes(inside[path]) for path in LOCKFILES if path in inside}
    absent_locks = sorted(set(LOCKFILES) - set(lockfiles))
    if absent_locks:
        raise ReleaseError(f"arşivde kilit dosyası yok: {', '.join(absent_locks)}")

    notices = inside.get("THIRD_PARTY_NOTICES.md")
    if notices is None:  # REQUIRED_FILES zaten reddeder; tip daraltması
        raise ReleaseError("THIRD_PARTY_NOTICES.md arşivde yok")

    artifacts: dict[str, object] = {
        "notices": {"path": "THIRD_PARTY_NOTICES.md", "sha256": sha256_bytes(notices)},
        "sbom": None,
    }
    if sbom_path is not None:
        try:
            sbom = json.loads(sbom_path.read_text())
        except (OSError, json.JSONDecodeError) as exc:
            raise ReleaseError(f"SBOM okunamadı ({sbom_path}): {exc}") from exc
        problems = sbom_binding(sbom, commit, version, lockfiles)
        if problems:
            raise ReleaseError("SBOM arşive bağlanmıyor: " + "; ".join(problems))
        artifacts["sbom"] = {
            "path": sbom_path.name,
            "sha256": sha256_file(sbom_path),
            "subjectCommit": sbom["subject"]["commit"],
        }

    packed = gzip_deterministic(raw)
    archive_name = f"dim-source-{version}-{short}.tar.gz"
    manifest = {
        "schema": SCHEMA,
        "schemaVersion": SCHEMA_VERSION,
        "name": "@dim/desktop",
        "version": version,
        "versionSources": versions,
        "commit": commit,
        "shortCommit": short,
        "tag": chosen_tag,
        "tagsAtCommit": repo.tags_at(commit),
        "archive": {
            "name": archive_name,
            "prefix": prefix,
            "sha256": sha256_bytes(packed),
            "sizeBytes": len(packed),
            "uncompressedBytes": len(raw),
            "fileCount": len(inside),
        },
        "lockfiles": lockfiles,
        "artifacts": artifacts,
        "generatedAt": datetime.now(timezone.utc).isoformat(timespec="seconds"),
    }

    out_dir.mkdir(parents=True, exist_ok=True)
    archive_path = out_dir / archive_name
    if archive_path.exists():
        previous = sha256_file(archive_path)
        if previous != manifest["archive"]["sha256"]:
            raise ReleaseError(
                f"aynı adda arşiv var ama özeti farklı ({previous[:12]} ≠ "
                f"{manifest['archive']['sha256'][:12]}): üretim deterministik değil"
            )
    tmp = archive_path.with_suffix(archive_path.suffix + ".tmp")
    tmp.write_bytes(packed)
    tmp.replace(archive_path)
    (out_dir / f"{archive_name}.sha256").write_text(
        f"{manifest['archive']['sha256']}  {archive_name}\n"
    )
    (out_dir / f"{archive_name}.manifest.json").write_text(
        json.dumps(manifest, indent=1, sort_keys=True, ensure_ascii=False) + "\n"
    )
    return archive_path, manifest


def verify(
    repo: Repo,
    archive_path: Path,
    manifest_path: Path | None = None,
    sbom_path: Path | None = None,
) -> list[str]:
    """Arşivi bağımsız doğrular; sorun listesi döner (boş = geçti)."""
    problems: list[str] = []
    manifest_path = manifest_path or archive_path.with_suffix(archive_path.suffix + ".manifest.json")
    sidecar_path = archive_path.with_suffix(archive_path.suffix + ".sha256")

    if not archive_path.is_file():
        return [f"arşiv yok: {archive_path}"]
    if not manifest_path.is_file():
        return [f"manifest yok: {manifest_path}"]
    try:
        manifest = json.loads(manifest_path.read_text())
    except json.JSONDecodeError as exc:
        return [f"manifest bozuk: {exc}"]

    if manifest.get("schema") != SCHEMA:
        problems.append(f"manifest şeması {manifest.get('schema')!r} ≠ {SCHEMA!r}")

    digest = sha256_file(archive_path)
    archive = manifest.get("archive") or {}
    if digest != archive.get("sha256"):
        problems.append(f"arşiv sha256 {digest[:12]} ≠ manifest {str(archive.get('sha256'))[:12]}")
    if archive_path.name != archive.get("name"):
        problems.append(f"arşiv adı {archive_path.name!r} ≠ manifest {archive.get('name')!r}")
    if archive_path.stat().st_size != archive.get("sizeBytes"):
        problems.append(
            f"arşiv boyutu {archive_path.stat().st_size} ≠ manifest {archive.get('sizeBytes')}"
        )
    if sidecar_path.is_file():
        sidecar = sidecar_path.read_text().split()
        if not sidecar or sidecar[0] != digest:
            problems.append("yan dosya (.sha256) arşiv özetiyle uyuşmuyor")
    else:
        problems.append(f"özet yan dosyası yok: {sidecar_path.name}")

    commit = str(manifest.get("commit") or "")
    version = str(manifest.get("version") or "")
    try:
        tree = repo.tree(commit)
    except ReleaseError:
        return problems + [f"commit depoda bulunamadı: {commit or '(boş)'}"]

    try:
        versions = versions_from(lambda path: repo.show(commit, path))
    except ReleaseError as exc:
        problems.append(f"commit ağacından sürüm okunamadı: {exc}")
        versions = {}
    if versions:
        found = require_single_version(versions)
        if found != version:
            problems.append(f"commit sürümü {found} ≠ manifest sürümü {version}")
        if manifest.get("versionSources") not in (None, versions):
            problems.append("manifest versionSources commit ile uyuşmuyor")

    tag = manifest.get("tag")
    if tag:
        try:
            if repo.tag_commit(tag) != commit:
                problems.append(f"etiket {tag!r} arşiv commit'ine bakmıyor")
        except ReleaseError:
            problems.append(f"etiket depoda yok: {tag!r}")

    prefix = str(archive.get("prefix") or "")
    try:
        with tarfile.open(archive_path, "r:gz") as archive_file:
            names = [entry.name for entry in archive_file.getmembers() if not entry.isdir()]
            packed_members = {}
            for entry in archive_file.getmembers():
                if entry.isdir():
                    continue
                handle = archive_file.extractfile(entry)
                if handle is None:
                    raise ReleaseError(f"arşiv öğesi okunamadı: {entry.name}")
                packed_members[entry.name] = handle.read()
    except (tarfile.TarError, OSError, ReleaseError) as exc:
        return problems + [f"arşiv açılamadı: {exc}"]

    try:
        inside = strip_prefix(packed_members, prefix)
    except ReleaseError as exc:
        return problems + [str(exc)]

    if sorted(inside) != tree:
        extra = sorted(set(inside) - set(tree))
        missing = sorted(set(tree) - set(inside))
        detail = []
        if extra:
            detail.append(f"fazla: {', '.join(extra[:5])}")
        if missing:
            detail.append(f"eksik: {', '.join(missing[:5])}")
        problems.append(f"arşiv dosya kümesi commit ağacına eşit değil ({'; '.join(detail)})")
    if len(inside) != archive.get("fileCount"):
        problems.append(f"dosya sayısı {len(inside)} ≠ manifest {archive.get('fileCount')}")

    missing_required = sorted(set(REQUIRED_FILES) - set(inside))
    if missing_required:
        problems.append(f"zorunlu kaynak dosyası yok: {', '.join(missing_required)}")

    for name in sorted(inside):
        reason = denied_reason(name)
        if reason:
            problems.append(f"yayımlanmaması gereken yol: {name} ({reason})")

    lockfiles = manifest.get("lockfiles") or {}
    for path in LOCKFILES:
        if path not in inside:
            problems.append(f"kilit dosyası arşivde yok: {path}")
            continue
        digest_inside = sha256_bytes(inside[path])
        if path not in lockfiles:
            problems.append(f"manifest kilit özeti taşımıyor: {path}")
        elif lockfiles[path] != digest_inside:
            problems.append(f"kilit dosyası özeti uyuşmuyor: {path}")

    notices = (manifest.get("artifacts") or {}).get("notices") or {}
    if inside.get("THIRD_PARTY_NOTICES.md") is not None:
        if notices.get("sha256") != sha256_bytes(inside["THIRD_PARTY_NOTICES.md"]):
            problems.append("THIRD_PARTY_NOTICES.md özeti manifest ile uyuşmuyor")
    else:
        problems.append("THIRD_PARTY_NOTICES.md arşivde yok")

    sbom_entry = (manifest.get("artifacts") or {}).get("sbom")
    if sbom_path is not None:
        try:
            sbom = json.loads(sbom_path.read_text())
        except (OSError, json.JSONDecodeError) as exc:
            problems.append(f"SBOM okunamadı ({sbom_path}): {exc}")
        else:
            problems.extend(sbom_binding(sbom, commit, version, lockfiles))
    elif sbom_entry:
        problems.append("manifest SBOM'a bağlı ama doğrulama için SBOM verilmedi (--sbom)")

    return problems


# --- kendi kendini sınama --------------------------------------------------


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text if text.endswith("\n") else text + "\n")


def _fixture_repo(base: Path) -> Repo:
    """Zorunlu dosyaları taşıyan, tek commit + etiketli geçici depo kurar."""
    for rel in REQUIRED_FILES:
        _write(base / rel, f"# fixture {rel}")
    _write(
        base / "Cargo.toml",
        '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "0.1.0"\nedition = "2021"\n',
    )
    _write(base / "Cargo.lock", "# fixture lock\n")
    _write(base / "apps/desktop/src-tauri/Cargo.lock", "# fixture shell lock\n")
    _write(base / "pnpm-lock.yaml", "lockfileVersion: '9.0'\n")
    for rel in VERSION_SOURCES:
        if rel.endswith("tauri.conf.json"):
            _write(base / rel, json.dumps({"version": "0.1.0"}) )
        else:
            name = "@dim/desktop" if "apps/desktop" in rel else "@dim/contracts"
            _write(base / rel, json.dumps({"name": name, "version": "0.1.0"}))
    _write(base / "THIRD_PARTY_NOTICES.md", "# Üçüncü taraf bildirimleri (fixture)\n")
    # ASCII dışı yol: `git ls-tree` tırnaklama tuzağını (Türkçe dosya adları)
    # fixture'ın kendisi ölçer.
    _write(base / "docs/adr/türkçe-ad.md", "# fixture ADR\n")

    repo = Repo(base)
    repo.git("init", "-q", "-b", "main")
    repo.git("config", "user.email", "fixture@dim.invalid")
    repo.git("config", "user.name", "DIM fixture")
    repo.git("add", "-A")
    repo.git("commit", "-q", "-m", "fixture: kaynak ağacı")
    repo.git("tag", "-a", "v0.1.0", "-m", "fixture sürümü")
    return repo


def _copy_archive(source: Path, target: Path) -> Path:
    shutil.copyfile(source, target)
    return target


def _rewrite_archive(source: Path, target: Path, drop: tuple[str, ...] = (), add: dict[str, bytes] | None = None) -> Path:
    """Arşivi yeniden paketler (içerik düzeyi mutasyonları için)."""
    with tarfile.open(source, "r:gz") as archive:
        entries = []
        for entry in archive.getmembers():
            if entry.isdir():
                continue
            handle = archive.extractfile(entry)
            if handle is None:
                raise ReleaseError(f"fixture arşiv öğesi okunamadı: {entry.name}")
            entries.append((entry, handle.read()))
        prefix = entries[0][0].name.rsplit("/", 1)[0] + "/"
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w") as out:
        for entry, data in entries:
            if entry.name in drop:
                continue
            info = tarfile.TarInfo(entry.name)
            info.size = len(data)
            info.mode = entry.mode
            info.mtime = 0
            out.addfile(info, io.BytesIO(data))
        for name, data in (add or {}).items():
            info = tarfile.TarInfo(prefix + name)
            info.size = len(data)
            info.mode = 0o644
            info.mtime = 0
            out.addfile(info, io.BytesIO(data))
    target.write_bytes(gzip_deterministic(raw.getvalue()))
    return target


def self_test() -> int:
    failures = 0

    def check(label: str, ok: bool, detail: str = "") -> None:
        nonlocal failures
        if ok:
            print(f"[ok] {label}")
        else:
            failures += 1
            print(f"[FAIL] {label}{' — ' + detail if detail else ''}")

    with tempfile.TemporaryDirectory(prefix="dim-source-selftest-") as tmp:
        base = Path(tmp)
        repo = _fixture_repo(base / "repo")
        out = base / "out"
        archive, manifest = build(repo, "HEAD", out, sbom_path=None)
        problems = verify(repo, archive)
        check("temel arşiv üretilir ve doğrulanır", not problems, "; ".join(problems))
        check(
            "manifest etiketi ve commit'i taşır",
            manifest["tag"] == "v0.1.0" and manifest["commit"] == repo.commit_of("HEAD"),
        )
        check(
            "manifest arşivdeki kilit özetlerini taşır",
            manifest["lockfiles"]["Cargo.lock"] == sha256_bytes(b"# fixture lock\n"),
        )

        block = tomllib.loads((repo.root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
        check("fixture sürümü 0.1.0", block == "0.1.0")

        second, manifest2 = build(repo, "HEAD", base / "out2")
        check(
            "üretim deterministik (iki koşu bayt bayt aynı)",
            manifest2["archive"]["sha256"] == manifest["archive"]["sha256"],
            f"{manifest2['archive']['sha256'][:12]} ≠ {manifest['archive']['sha256'][:12]}",
        )
        # M1: manifest'teki kilit özeti bozulunca doğrulama kırmızı
        tampered = json.loads((out / f"{archive.name}.manifest.json").read_text())
        tampered["lockfiles"]["pnpm-lock.yaml"] = "0" * 64
        bad_manifest = base / "bad-lock.manifest.json"
        bad_manifest.write_text(json.dumps(tampered))
        problems = verify(repo, archive, manifest_path=bad_manifest)
        check(
            "M1 kilit özeti uyuşmazlığı yakalanır",
            any("kilit dosyası özeti uyuşmuyor" in problem for problem in problems),
            "; ".join(problems),
        )

        # M2: arşivden zorunlu dosya düşünce (özet yan dosyası güncel olsa bile) kırmızı
        dropped = _rewrite_archive(archive, base / "no-license.tar.gz", drop=("dim-source-0.1.0-" + manifest["shortCommit"] + "/LICENSE",))
        patched = json.loads(json.dumps(manifest))
        patched["archive"]["name"] = dropped.name
        patched["archive"]["sha256"] = sha256_file(dropped)
        patched["archive"]["sizeBytes"] = dropped.stat().st_size
        patched["archive"]["fileCount"] = patched["archive"]["fileCount"] - 1
        patched_manifest = base / f"{dropped.name}.manifest.json"
        patched_manifest.write_text(json.dumps(patched))
        (base / f"{dropped.name}.sha256").write_text(f"{patched['archive']['sha256']}  {dropped.name}\n")
        problems = verify(repo, dropped, manifest_path=patched_manifest)
        check(
            "M2 eksik zorunlu dosya yakalanır",
            any("zorunlu kaynak dosyası yok" in problem for problem in problems),
            "; ".join(problems),
        )

        # M3: arşive fazladan dosya eklenince ağaç karşılaştırması kırmızı
        extra = _rewrite_archive(archive, base / "extra.tar.gz", add={"tools/sizinti.txt": b"x"})
        patched = json.loads(json.dumps(manifest))
        patched["archive"]["name"] = extra.name
        patched["archive"]["sha256"] = sha256_file(extra)
        patched["archive"]["sizeBytes"] = extra.stat().st_size
        patched["archive"]["fileCount"] = patched["archive"]["fileCount"] + 1
        extra_manifest = base / f"{extra.name}.manifest.json"
        extra_manifest.write_text(json.dumps(patched))
        (base / f"{extra.name}.sha256").write_text(f"{patched['archive']['sha256']}  {extra.name}\n")
        problems = verify(repo, extra, manifest_path=extra_manifest)
        check(
            "M3 fazla dosya ağaç karşılaştırmasında yakalanır",
            any("dosya kümesi commit ağacına eşit değil" in problem for problem in problems),
            "; ".join(problems),
        )

        # M4: bozuk arşiv (özet uyuşmazlığı)
        corrupt = base / "corrupt.tar.gz"
        corrupt.write_bytes(archive.read_bytes()[:-32] + b"\x00" * 32)
        problems = verify(repo, corrupt, manifest_path=out / f"{archive.name}.manifest.json")
        check(
            "M4 bozuk arşiv özet kapısında durur",
            any("arşiv sha256" in problem for problem in problems),
            "; ".join(problems),
        )

        # M5: etiket başka commit'e bakınca kırmızı
        (repo.root / "README.md").write_text("# değişti\n")
        repo.git("add", "-A")
        repo.git("commit", "-q", "-m", "fixture: ikinci commit")
        head2 = repo.commit_of("HEAD")
        repo.git("tag", "-f", "-a", "v0.1.0", "-m", "fixture: etiket taşındı", head2)
        moved = json.loads(json.dumps(manifest))
        moved["tag"] = "v0.1.0"
        moved_manifest = base / "tag-drift.manifest.json"
        moved_manifest.write_text(json.dumps(moved))
        problems = verify(repo, archive, manifest_path=moved_manifest)
        check(
            "M5 etiket commit'i kayınca yakalanır",
            any("etiket" in problem and "bakmıyor" in problem for problem in problems),
            "; ".join(problems),
        )
        check("ikinci fixture commit üretildi", head2 != manifest["commit"])

        # M6: sürüm kaynakları ayrışınca üretim durur
        _write(repo.root / "apps/desktop/src-tauri/tauri.conf.json", json.dumps({"version": "0.2.0"}))
        repo.git("add", "-A")
        repo.git("commit", "-q", "-m", "fixture: sürüm kayması")
        try:
            build(repo, "HEAD", base / "out3", allow_untagged=True)
        except ReleaseError as exc:
            check("M6 sürüm ayrışması üretimi durdurur", "ayrışıyor" in str(exc), str(exc))
        else:
            check("M6 sürüm ayrışması üretimi durdurur", False, "hata beklenirdi")

        # M7: izlenen sır dosyası üretimi durdurur
        _write(repo.root / "apps/desktop/src-tauri/tauri.conf.json", json.dumps({"version": "0.1.0"}))
        _write(repo.root / "deploy/id_rsa", "PRIVATE KEY")
        repo.git("add", "-A")
        repo.git("commit", "-q", "-m", "fixture: sır dosyası")
        try:
            build(repo, "HEAD", base / "out4", allow_untagged=True)
        except ReleaseError as exc:
            check("M7 izlenen sır dosyası üretimi durdurur", "id_rsa" in str(exc), str(exc))
        else:
            check("M7 izlenen sır dosyası üretimi durdurur", False, "hata beklenirdi")

        # M8: SBOM commit'i kayınca bağ kurulmaz
        sbom = {
            "bomFormat": "dim-sbom",
            "specVersion": 1,
            "subject": {
                "name": "@dim/desktop",
                "version": "0.1.0",
                "bundleVersion": "0.1.0",
                "cargoVersion": "0.1.0",
                "commit": "0" * 40,
                "lockfiles": manifest["lockfiles"],
            },
        }
        sbom_file = base / "sbom.json"
        sbom_file.write_text(json.dumps(sbom))
        try:
            build(repo, manifest["commit"], base / "out5", sbom_path=sbom_file, allow_untagged=True)
        except ReleaseError as exc:
            check("M8 SBOM commit uyuşmazlığı üretimi durdurur", "bağlanmıyor" in str(exc), str(exc))
        else:
            check("M8 SBOM commit uyuşmazlığı üretimi durdurur", False, "hata beklenirdi")

        sbom["subject"]["commit"] = manifest["commit"]
        sbom_file.write_text(json.dumps(sbom))
        archive2, manifest2 = build(
            repo, manifest["commit"], base / "out6", sbom_path=sbom_file, allow_untagged=True
        )
        problems = verify(repo, archive2, sbom_path=sbom_file)
        check("bağlı SBOM ile arşiv doğrulanır", not problems, "; ".join(problems))
        check(
            "manifest SBOM özetini taşır",
            (manifest2["artifacts"]["sbom"] or {}).get("sha256") == sha256_file(sbom_file),
        )

        # M9: commit ağacında zorunlu dosya hiç yoksa üretim durur (ağaç eşitliği
        # bunu yakalayamaz: arşiv commit ile birebir aynıdır; kapı REQUIRED_FILES)
        (repo.root / "deploy/id_rsa").unlink()
        (repo.root / "LICENSE").unlink()
        repo.git("add", "-A")
        repo.git("commit", "-q", "-m", "fixture: lisanssız ağaç")
        try:
            build(repo, "HEAD", base / "out7", allow_untagged=True)
        except ReleaseError as exc:
            check(
                "M9 lisanssız ağaç üretimi durdurur",
                "zorunlu kaynak dosyası yok" in str(exc) and "LICENSE" in str(exc),
                str(exc),
            )
        else:
            check("M9 lisanssız ağaç üretimi durdurur", False, "hata beklenirdi")

        # M10: eski/hatalı bir araçla üretilmiş (ağacı lisanssız) arşivi doğrulama
        # da yakalar: ağaç eşitliği geçer, kapı REQUIRED_FILES'tır.
        head3 = repo.commit_of("HEAD")
        prefix3 = f"dim-source-0.1.0-{head3[:8]}/"
        raw3 = repo.git("archive", "--format=tar", f"--prefix={prefix3}", head3, binary=True)
        packed3 = gzip_deterministic(raw3)
        name3 = f"dim-source-0.1.0-{head3[:8]}.tar.gz"
        old = base / "old"
        old.mkdir()
        (old / name3).write_bytes(packed3)
        manifest3 = json.loads(json.dumps(manifest))
        manifest3.update({"commit": head3, "shortCommit": head3[:8], "tag": None, "tagsAtCommit": []})
        manifest3["archive"] = {
            "name": name3,
            "prefix": prefix3,
            "sha256": sha256_bytes(packed3),
            "sizeBytes": len(packed3),
            "uncompressedBytes": len(raw3),
            "fileCount": len(tar_members(raw3)),
        }
        manifest3["lockfiles"] = {
            path: sha256_bytes(tar_members(raw3)[prefix3 + path]) for path in LOCKFILES
        }
        (old / f"{name3}.sha256").write_text(f"{sha256_bytes(packed3)}  {name3}\n")
        (old / f"{name3}.manifest.json").write_text(json.dumps(manifest3))
        problems = verify(repo, old / name3)
        check(
            "M10 lisanssız arşiv doğrulamada durur",
            any("zorunlu kaynak dosyası yok" in problem for problem in problems),
            "; ".join(problems),
        )

    print()
    if failures:
        print(f"self-test: {failures} kural KIRMIZI")
        return 1
    print("self-test: tüm kurallar yeşil")
    return 0


# --- komut satırı ----------------------------------------------------------


def main(argv: list[str]) -> int:
    if "--self-test" in argv[1:]:
        return self_test()

    parser = argparse.ArgumentParser(description="AGPL kaynak arşivi üret/doğrula")
    sub = parser.add_subparsers(dest="command", required=True)

    build_parser = sub.add_parser("build", help="commit ağacından deterministik kaynak arşivi üret")
    build_parser.add_argument("--ref", default="HEAD", help="commit/ref (varsayılan HEAD)")
    build_parser.add_argument("--out", default="dist", help="çıktı dizini (varsayılan dist)")
    build_parser.add_argument("--tag", default=None, help="yayın etiketi; commit'e baktığı doğrulanır")
    build_parser.add_argument("--sbom", default=None, help="SBOM dosyası; subject arşive bağlanır")
    build_parser.add_argument(
        "--allow-untagged",
        action="store_true",
        help="etiketsiz üretime izin ver (CI); yayında kullanılmaz",
    )

    check_parser = sub.add_parser("check", help="üretilmiş arşivi bağımsız doğrula")
    check_parser.add_argument("archive", help=".tar.gz yolu")
    check_parser.add_argument("--manifest", default=None)
    check_parser.add_argument("--sbom", default=None)

    args = parser.parse_args(argv[1:])
    repo = Repo(ROOT)

    try:
        if args.command == "build":
            archive, manifest = build(
                repo,
                args.ref,
                Path(args.out),
                tag=args.tag,
                sbom_path=Path(args.sbom) if args.sbom else None,
                allow_untagged=args.allow_untagged,
            )
            print(f"arşiv: {archive}")
            print(f"sha256: {manifest['archive']['sha256']}")
            print(
                f"sürüm: {manifest['version']} · commit: {manifest['commit']} · "
                f"etiket: {manifest['tag'] or '(yok)'} · dosya: {manifest['archive']['fileCount']} · "
                f"boyut: {manifest['archive']['sizeBytes']} bayt"
            )
            return 0

        problems = verify(
            repo,
            Path(args.archive),
            manifest_path=Path(args.manifest) if args.manifest else None,
            sbom_path=Path(args.sbom) if args.sbom else None,
        )
        if problems:
            print(f"doğrulama KIRMIZI ({len(problems)} neden):")
            for problem in problems:
                print(f"  - {problem}")
            return 1
        print(f"doğrulama yeşil: {args.archive}")
        return 0
    except ReleaseError as exc:
        print(f"hata: {exc}")
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
