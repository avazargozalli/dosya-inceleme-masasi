"""`scripts/worker_manifest.py` testleri (F01-T02).

Manifest üretimi/doğrulaması stdlib'dir; PyInstaller gerekmez. Testler sentetik
ağaçlar kurar: gerçek paket üretmek dakikalar alır, kural aynıdır.
"""

from __future__ import annotations

import importlib.util
import json
import os
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "scripts" / "worker_manifest.py"


def _load():
    spec = importlib.util.spec_from_file_location("worker_manifest", SCRIPT)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules["worker_manifest"] = module
    spec.loader.exec_module(module)
    return module


wm = _load()


def _tree(tmp_path: Path) -> Path:
    package = tmp_path / "worker"
    (package / "_internal" / "lib").mkdir(parents=True)
    (package / "dim-worker").write_bytes(b"ikili")
    (package / "_internal" / "base_library.zip").write_bytes(b"arsiv")
    (package / "_internal" / "lib" / "libpython3.12.dylib").write_bytes(b"dylib")
    # Depoda izlenen dosyalar paketin parçası değildir.
    (package / ".gitkeep").write_bytes(b"")
    (package / "README.md").write_text("aciklama", encoding="utf-8")
    return package


def _manifest(package: Path) -> dict:
    return wm.build_manifest(
        package,
        version="0.1.0",
        platform_tag="test-arm64",
        python_version="3.12.0",
        pyinstaller_version="6.11.1",
        tesseract=None,
    )


def test_manifest_lists_files_with_hashes_and_roles(tmp_path: Path) -> None:
    package = _tree(tmp_path)
    manifest = _manifest(package)

    assert manifest["manifestVersion"] == 1
    assert manifest["mode"] == "onedir"
    assert manifest["entryPoint"] == ("dim-worker.exe" if os.name == "nt" else "dim-worker")
    assert manifest["totals"]["files"] == 3
    assert manifest["totals"]["bytes"] == len(b"ikili") + len(b"arsiv") + len(b"dylib")
    assert manifest["totals"]["truncated"] is False
    assert manifest["tesseract"]["bundled"] is False

    by_path = {entry["path"]: entry for entry in manifest["files"]}
    assert set(by_path) == {"dim-worker", "_internal/base_library.zip", "_internal/lib/libpython3.12.dylib"}
    assert by_path["dim-worker"]["role"] == "bootloader"
    assert by_path["_internal/base_library.zip"]["role"] == "stdlib_archive"
    assert by_path["_internal/lib/libpython3.12.dylib"]["role"] == "python_runtime"
    # sha256 gerçekten hesaplanır (boş string değil).
    assert len(by_path["dim-worker"]["sha256"]) == 64


def test_repository_placeholders_are_not_part_of_the_package(tmp_path: Path) -> None:
    package = _tree(tmp_path)
    manifest = _manifest(package)
    paths = {entry["path"] for entry in manifest["files"]}
    assert ".gitkeep" not in paths
    assert "README.md" not in paths


def test_round_trip_verify_is_clean(tmp_path: Path) -> None:
    package = _tree(tmp_path)
    manifest = _manifest(package)
    result = wm.verify(package, manifest)
    assert result["ok"] is True
    assert result["problems"] == []
    assert result["extra"] == []
    assert result["verified"] == 3
    assert result["totalBytes"] == manifest["totals"]["bytes"]


def test_changed_bytes_are_reported_not_ignored(tmp_path: Path) -> None:
    package = _tree(tmp_path)
    manifest = _manifest(package)
    (package / "dim-worker").write_bytes(b"BASKA-UZUN")  # boyut değişti
    result = wm.verify(package, manifest)
    assert result["ok"] is False
    assert [item["problem"] for item in result["problems"]] == ["size_changed"]

    # Aynı boyutta değişen bayt hash ile yakalanır.
    package2 = _tree(tmp_path / "ikinci")
    manifest2 = _manifest(package2)
    (package2 / "_internal" / "base_library.zip").write_bytes(b"ARSHV")
    result2 = wm.verify(package2, manifest2)
    assert [item["problem"] for item in result2["problems"]] == ["hash_changed"]


def test_missing_and_extra_are_separate_counts(tmp_path: Path) -> None:
    package = _tree(tmp_path)
    manifest = _manifest(package)
    (package / "_internal" / "base_library.zip").unlink()
    (package / "fazla.txt").write_bytes(b"x")
    result = wm.verify(package, manifest)
    assert result["verified"] == 2
    assert [item["problem"] for item in result["problems"]] == ["missing"]
    assert result["extra"] == ["fazla.txt"]
    assert result["ok"] is False


def test_directory_symlinks_are_recorded_and_checked(tmp_path: Path) -> None:
    package = _tree(tmp_path)
    target = package / "_internal" / "lib"
    link = package / "_internal" / "Current"
    try:
        link.symlink_to("lib", target_is_directory=True)
    except (OSError, NotImplementedError):  # Windows'ta ayrıcalık gerekir
        pytest.skip("sembolik bağ oluşturulamadı")

    manifest = _manifest(package)
    entry = next(item for item in manifest["files"] if item["path"] == "_internal/Current")
    assert entry["kind"] == "symlink"
    assert entry["linkTarget"] == "lib"
    assert entry["unsafeLink"] is False
    assert manifest["totals"]["symlinks"] == 1

    result = wm.verify(package, manifest)
    assert result["ok"] is True

    link.unlink()
    link.symlink_to("baska", target_is_directory=True)
    broken = wm.verify(package, manifest)
    assert broken["ok"] is False
    assert [item["problem"] for item in broken["problems"]] == ["link_target_changed"]


def test_symlink_escaping_the_package_is_flagged(tmp_path: Path) -> None:
    package = _tree(tmp_path)
    outside = tmp_path / "disarida.txt"
    outside.write_bytes(b"gizli")
    link = package / "_internal" / "kacis"
    try:
        link.symlink_to(outside)
    except (OSError, NotImplementedError):
        pytest.skip("sembolik bağ oluşturulamadı")

    manifest = _manifest(package)
    entry = next(item for item in manifest["files"] if item["path"] == "_internal/kacis")
    assert entry["unsafeLink"] is True
    assert manifest["totals"]["unsafeLinks"] == 1
    # Güvensiz bağ "temiz" sayılmaz.
    result = wm.verify(package, manifest)
    assert result["ok"] is False
    assert [item["problem"] for item in result["problems"]] == ["unsafe_link"]


def test_verify_rejects_paths_that_escape_the_package(tmp_path: Path) -> None:
    package = _tree(tmp_path)
    manifest = _manifest(package)
    manifest["files"].append({"path": "../disari", "role": "other", "kind": "file"})
    result = wm.verify(package, manifest)
    assert result["ok"] is False
    assert [item["problem"] for item in result["problems"]] == ["unsafe_path"]


def test_unsupported_manifest_version_is_refused(tmp_path: Path) -> None:
    package = _tree(tmp_path)
    manifest = _manifest(package)
    manifest["manifestVersion"] = 99
    (package / wm.MANIFEST_NAME).write_text(json.dumps(manifest), encoding="utf-8")
    with pytest.raises(wm.ManifestError):
        wm._read_manifest(package)


def test_missing_manifest_is_an_error_not_an_empty_pass(tmp_path: Path) -> None:
    package = _tree(tmp_path)
    with pytest.raises(wm.ManifestError):
        wm._read_manifest(package)
    assert wm.main(["verify", "--dir", str(package), "--json"]) == 2
