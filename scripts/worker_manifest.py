#!/usr/bin/env python3
"""Paketlenmiş belge worker'ı için manifest üretir ve doğrular.

Neden: PyInstaller `onedir` çıktısı tek dosya değil, yüzlerce dosyalık bir
ağaçtır. "Paketlendi" demek "paket eksiksiz" demek değildir; manifest her
dosyanın rolünü, boyutunu ve sha256'sını taşır — böylece sonradan (uygulama
içinde veya CI'da) hangi dosyanın eksildiği/değiştiği gösterilebilir.

Bağımlılık yoktur (yalnız stdlib); worker venv'i olmadan da koşar.

Kullanım:
    python3 scripts/worker_manifest.py generate --dir <paket-dizini> \
        --version 0.1.0 --platform macos-aarch64 \
        --python 3.12.8 --pyinstaller 6.11.1
    python3 scripts/worker_manifest.py verify --dir <paket-dizini>
    python3 scripts/worker_manifest.py summary --dir <paket-dizini>

Sınırlar (sessiz kesme yok, hepsi raporda):
    - en çok 20.000 dosya, 8 GiB toplam
    - sembolik bağlar **izlenmez** (hedef metni kaydedilir, içerik hash'lenmez)
    - dizin dışına çıkan bağ (`..`/mutlak hedef) `unsafe_link` olarak raporlanır
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import stat
import sys
from pathlib import Path

MANIFEST_NAME = "WORKER-MANIFEST.json"
MANIFEST_VERSION = 1
# Paket dizininde depoda izlenen iki dosya durur (Tauri glob'u hedef bulsun
# diye); paketin parçası değildirler. Manifest onları listelemez, doğrulama da
# "fazla dosya" saymaz.
NON_PACKAGE_FILES = frozenset({".gitkeep", "README.md"})
MAX_FILES = 20_000
MAX_TOTAL_BYTES = 8 * 1024 * 1024 * 1024
CHUNK = 1024 * 1024


class ManifestError(Exception):
    """Manifest üretimi/doğrulaması düşerse."""


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            block = handle.read(CHUNK)
            if not block:
                break
            digest.update(block)
    return digest.hexdigest()


def classify(rel: str) -> str:
    """Dosyanın paketteki rolü. Ad desenine göre; içerik okunmaz."""
    name = rel.rsplit("/", 1)[-1]
    lower = rel.lower()
    if name in ("dim-worker", "dim-worker.exe"):
        return "bootloader"
    if (
        "python.framework" in lower
        or name.startswith("libpython")
        or name == "Python"
    ):
        return "python_runtime"
    if name == "base_library.zip":
        return "stdlib_archive"
    if "pymupdf" in lower or "fitz" in lower or name.startswith("_fitz"):
        return "pdf_engine"
    if lower.startswith("_internal/pil") or "/pil/" in lower or name.startswith("libjpeg"):
        return "image_lib"
    if "pytesseract" in lower or "tesseract" in lower:
        return "ocr_bridge"
    if "docx" in lower:
        return "docx_engine"
    if lower.endswith(".dist-info") or "/.dist-info/" in lower or name.endswith("METADATA"):
        return "package_metadata"
    if name.endswith((".so", ".dylib", ".dll")):
        return "native_library"
    if name.endswith((".pyc", ".py")) or lower.startswith("_internal/pyz"):
        return "python_module"
    if name.endswith((".ttf", ".otf", ".ttc")):
        return "font"
    if name.endswith((".rng", ".xsl", ".xslt", ".jpeg", ".xml", ".rels")):
        return "data_file"
    return "other"


def _platform_tag() -> str:
    system = platform.system().lower()
    machine = platform.machine().lower()
    if system == "darwin":
        system = "macos"
    if machine in ("aarch64", "arm64"):
        machine = "arm64"
    return f"{system}-{machine}"


def collect(root: Path) -> tuple[list[dict], dict]:
    """Dizin ağacını tarar; dosya listesi + sınır bilgisi döner.

    `os.walk` kullanılmaz: dizin sembolik bağlarını da kaydetmek gerekir
    (macOS paketinde `Python.framework/Versions/Current` gibi). Bağlar
    izlenmez, hedef metni yazılır.
    """
    if not root.is_dir():
        raise ManifestError(f"paket dizini yok: {root}")
    entries: list[dict] = []
    total = 0
    truncated = False
    unsafe_links: list[str] = []
    stack: list[Path] = [root]
    while stack:
        directory = stack.pop()
        try:
            children = sorted(os.scandir(directory), key=lambda e: e.name)
        except OSError as exc:
            entries.append(
                {
                    "path": directory.relative_to(root).as_posix(),
                    "role": "unreadable",
                    "error": str(exc),
                }
            )
            continue
        for child in children:
            full = Path(child.path)
            rel = full.relative_to(root).as_posix()
            if rel == MANIFEST_NAME or rel in NON_PACKAGE_FILES:
                continue
            try:
                info = child.stat(follow_symlinks=False)
            except OSError as exc:  # okunamayan girdi gizlenmez
                entries.append({"path": rel, "role": "unreadable", "error": str(exc)})
                continue
            if stat.S_ISLNK(info.st_mode):
                target = os.readlink(full)
                unsafe = os.path.isabs(target) or ".." in Path(target).parts
                if unsafe:
                    unsafe_links.append(rel)
                entries.append(
                    {
                        "path": rel,
                        "role": classify(rel),
                        "kind": "symlink",
                        "linkTarget": target,
                        "unsafeLink": unsafe,
                    }
                )
                continue
            if stat.S_ISDIR(info.st_mode):
                stack.append(full)
                continue
            if not stat.S_ISREG(info.st_mode):
                continue
            if len(entries) >= MAX_FILES or total + info.st_size > MAX_TOTAL_BYTES:
                truncated = True
                continue
            try:
                digest = _sha256(full)
            except OSError as exc:
                entries.append({"path": rel, "role": "unreadable", "error": str(exc)})
                continue
            entries.append(
                {
                    "path": rel,
                    "role": classify(rel),
                    "kind": "file",
                    "bytes": info.st_size,
                    "sha256": digest,
                }
            )
            total += info.st_size
    entries.sort(key=lambda entry: entry["path"])
    limits = {"truncated": truncated, "unsafeLinks": unsafe_links}
    return entries, {
        "files": len([e for e in entries if e.get("kind") == "file"]),
        "symlinks": len([e for e in entries if e.get("kind") == "symlink"]),
        "bytes": total,
        "truncated": truncated,
        "unsafeLinks": len(unsafe_links),
    }


def _non_directory_paths(root: Path) -> list[str]:
    """Dizin olmayan girdilerin göreli yolları (dosya + sembolik bağ).

    Dizin sembolik bağları da birer girdidir; ağaçlarına **inilmez**.
    """
    found: list[str] = []
    stack: list[Path] = [root]
    while stack:
        directory = stack.pop()
        try:
            children = os.scandir(directory)
        except OSError:
            continue
        for child in children:
            full = Path(child.path)
            rel = full.relative_to(root).as_posix()
            try:
                info = child.stat(follow_symlinks=False)
            except OSError:
                found.append(rel)
                continue
            if stat.S_ISDIR(info.st_mode):
                stack.append(full)
            else:
                found.append(rel)
    found.sort()
    return found


def build_manifest(
    root: Path,
    version: str,
    platform_tag: str,
    python_version: str,
    pyinstaller_version: str,
    tesseract: dict | None,
) -> dict:
    entries, totals = collect(root)
    roles: dict[str, int] = {}
    role_bytes: dict[str, int] = {}
    for entry in entries:
        role = entry["role"]
        roles[role] = roles.get(role, 0) + 1
        if entry.get("kind") == "file":
            role_bytes[role] = role_bytes.get(role, 0) + int(entry.get("bytes", 0))
    exe = "dim-worker.exe" if os.name == "nt" else "dim-worker"
    return {
        "manifestVersion": MANIFEST_VERSION,
        "workerVersion": version,
        "mode": "onedir",
        "buildPlatform": platform_tag,
        "pythonVersion": python_version,
        "pyinstallerVersion": pyinstaller_version,
        "entryPoint": exe,
        "totals": totals,
        "roles": dict(sorted(roles.items())),
        "roleBytes": dict(sorted(role_bytes.items())),
        "tesseract": tesseract
        or {
            "bundled": False,
            "note": "Tesseract ikilisi ve dil verisi pakete gömülmez; OCR için sistemde kurulu olmalı.",
        },
        "files": entries,
    }


def verify(root: Path, manifest: dict) -> dict:
    """Manifesti dizine karşı doğrular. Eksik/fazla/değişmiş ayrı raporlanır."""
    problems: list[dict] = []
    listed: set[str] = set()
    verified = 0
    total_bytes = 0
    for entry in manifest.get("files", []):
        rel = str(entry.get("path", ""))
        if not rel or rel in listed:
            problems.append({"path": rel, "problem": "duplicate_or_empty_path"})
            continue
        parts = Path(rel).parts
        if Path(rel).is_absolute() or ".." in parts or "\\" in rel:
            problems.append({"path": rel, "problem": "unsafe_path"})
            continue
        listed.add(rel)
        full = root / rel
        if entry.get("kind") == "symlink":
            if entry.get("unsafeLink"):
                # Hedefi paket dışına çıkan bağ "temiz" sayılmaz: paket
                # kendi sınırının dışındaki bir dosyaya bağlı demektir.
                problems.append({"path": rel, "problem": "unsafe_link"})
                continue
            if not full.is_symlink():
                problems.append({"path": rel, "problem": "missing"})
                continue
            target = os.readlink(full)
            if target != entry.get("linkTarget"):
                problems.append({"path": rel, "problem": "link_target_changed"})
                continue
            verified += 1
            continue
        if not full.is_file():
            problems.append({"path": rel, "problem": "missing"})
            continue
        info = full.lstat()
        if info.st_size != entry.get("bytes"):
            problems.append(
                {
                    "path": rel,
                    "problem": "size_changed",
                    "expected": entry.get("bytes"),
                    "actual": info.st_size,
                }
            )
            continue
        try:
            digest = _sha256(full)
        except OSError as exc:
            problems.append({"path": rel, "problem": "unreadable", "detail": str(exc)})
            continue
        if digest != entry.get("sha256"):
            problems.append({"path": rel, "problem": "hash_changed"})
            continue
        verified += 1
        total_bytes += info.st_size
    extra: list[str] = []
    for rel in _non_directory_paths(root):
        if rel == MANIFEST_NAME or rel in NON_PACKAGE_FILES:
            continue
        if rel not in listed:
            extra.append(rel)
    return {
        "manifestVersion": manifest.get("manifestVersion"),
        "workerVersion": manifest.get("workerVersion"),
        "buildPlatform": manifest.get("buildPlatform"),
        "pythonVersion": manifest.get("pythonVersion"),
        "pyinstallerVersion": manifest.get("pyinstallerVersion"),
        "verified": verified,
        "problems": problems,
        "extra": sorted(extra),
        "totalBytes": total_bytes,
        "ok": not problems and not extra,
    }


def _read_manifest(root: Path) -> dict:
    path = root / MANIFEST_NAME
    if not path.is_file():
        raise ManifestError(f"manifest yok: {path}")
    with path.open("r", encoding="utf-8") as handle:
        data = json.load(handle)
    if not isinstance(data, dict):
        raise ManifestError("manifest bir nesne değil")
    if data.get("manifestVersion") != MANIFEST_VERSION:
        raise ManifestError(f"desteklenmeyen manifest sürümü: {data.get('manifestVersion')!r}")
    return data


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Worker paketi manifesti")
    sub = parser.add_subparsers(dest="command", required=True)

    gen = sub.add_parser("generate", help="manifest üret")
    gen.add_argument("--dir", required=True, type=Path)
    gen.add_argument("--version", default="0.1.0")
    gen.add_argument("--platform", dest="platform_tag", default=None)
    gen.add_argument("--python", dest="python_version", default=platform.python_version())
    gen.add_argument("--pyinstaller", dest="pyinstaller_version", default="unknown")

    ver = sub.add_parser("verify", help="manifesti dizine karşı doğrula")
    ver.add_argument("--dir", required=True, type=Path)
    ver.add_argument("--json", action="store_true", help="makine okunur çıktı")

    summ = sub.add_parser("summary", help="manifest özeti")
    summ.add_argument("--dir", required=True, type=Path)

    args = parser.parse_args(argv)

    if args.command == "generate":
        manifest = build_manifest(
            args.dir,
            args.version,
            args.platform_tag or _platform_tag(),
            args.python_version,
            args.pyinstaller_version,
            None,
        )
        out = args.dir / MANIFEST_NAME
        with out.open("w", encoding="utf-8") as handle:
            json.dump(manifest, handle, ensure_ascii=False, indent=2, sort_keys=False)
            handle.write("\n")
        totals = manifest["totals"]
        print(
            f"manifest: {out} — {totals['files']} dosya / {totals['bytes']} bayt / "
            f"{totals['symlinks']} bağ"
        )
        if totals["truncated"]:
            print("UYARI: dosya sınırı aşıldı, manifest eksik (truncated=true)", file=sys.stderr)
        if totals["unsafeLinks"]:
            print(f"UYARI: {totals['unsafeLinks']} güvensiz bağ", file=sys.stderr)
        return 0

    if args.command == "verify":
        try:
            manifest = _read_manifest(args.dir)
        except ManifestError as exc:
            if args.json:
                print(json.dumps({"ok": False, "error": str(exc)}, ensure_ascii=False, indent=2))
            else:
                print(f"HATA: {exc}", file=sys.stderr)
            return 2
        result = verify(args.dir, manifest)
        if args.json:
            print(json.dumps(result, ensure_ascii=False, indent=2))
        else:
            print(
                f"doğrulanan {result['verified']} · sorun {len(result['problems'])} · "
                f"fazla {len(result['extra'])} · {result['totalBytes']} bayt · "
                f"{'TAMAM' if result['ok'] else 'KUSURLU'}"
            )
            for item in result["problems"][:20]:
                print(f"  sorun: {item['path']} → {item['problem']}")
            for rel in result["extra"][:20]:
                print(f"  fazla: {rel}")
        return 0 if result["ok"] else 1

    manifest = _read_manifest(args.dir)
    print(json.dumps(
        {
            "workerVersion": manifest.get("workerVersion"),
            "mode": manifest.get("mode"),
            "buildPlatform": manifest.get("buildPlatform"),
            "pythonVersion": manifest.get("pythonVersion"),
            "pyinstallerVersion": manifest.get("pyinstallerVersion"),
            "totals": manifest.get("totals"),
            "roles": manifest.get("roles"),
            "tesseract": manifest.get("tesseract"),
        },
        ensure_ascii=False,
        indent=2,
    ))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ManifestError as exc:
        print(f"HATA: {exc}", file=sys.stderr)
        raise SystemExit(2) from exc
