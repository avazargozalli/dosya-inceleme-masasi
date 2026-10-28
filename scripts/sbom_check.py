#!/usr/bin/env python3
"""SBOM bağlayıcı kapısı.

`scripts/sbom.sh` çıktısını **bağımsız** kaynaklarla doğrular; betiğin kendi
mantığını tekrar etmez:

1. Sayaçlar listelerle birebir (eksik/fazla sayaç sessizce geçmez).
2. Her bileşen **sabit sürüm** taşır: `latest`, `*`, `^`, `~`, aralık yok.
3. Çalışma alanındaki her `package.json`'un bildirdiği bağımlılık (üretim +
   geliştirme) SBOM'da **adıyla** bulunur → `devDependencies` atlanamaz.
4. Geçişli bağımlılık kanıtı: bileşen sayısı bildirilen bağımlılık sayısından
   **büyük** olmalı (eski `--depth 0` kusurunu yakalar).
5. Ürüne bağlama: `subject.version` = `bundleVersion` = `cargoVersion` (semver),
   commit 40 haneli, üç kilit dosyasının sha256 özeti dolu.
6. Python pinleri `ad==sürüm` biçiminde.

`--self-test` gerçek SBOM'a dokunmadan bu kuralları fixture'larla sınar:
kusurlu SBOM'lar **kırmızı** olmalı, tam olan **yeşil**.
"""

from __future__ import annotations

import json
import re
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

SEMVER = re.compile(r"^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")
FLOATING_HINTS = ("*", "^", "~", ">", "<", "latest", "workspace:", "link:", "file:")
WORKSPACE_MANIFESTS = (
    "package.json",
    "apps/desktop/package.json",
    "packages/contracts/package.json",
)
COMPONENT_LISTS = (
    "rust",
    "rustWorkspace",
    "node",
    "nodeDev",
    "nodeWorkspace",
    "python",
    "pythonDev",
)


def split_entry(entry: str) -> tuple[str, str]:
    """`ad@sürüm` → (ad, sürüm); sürüm yoksa ("", "")."""
    name, sep, version = entry.rpartition("@")
    if sep and name:
        return name, version
    return entry, ""


def declared_dependencies(root: Path) -> dict[str, list[str]]:
    """Çalışma alanındaki package.json dosyalarından bildirilen bağımlılıklar."""
    declared: dict[str, list[str]] = {}
    for rel in WORKSPACE_MANIFESTS:
        path = root / rel
        if not path.exists():
            continue
        data = json.loads(path.read_text())
        for section in ("dependencies", "devDependencies", "optionalDependencies"):
            for name in data.get(section) or {}:
                declared.setdefault(name, []).append(f"{rel}:{section}")
    return declared


def check(sbom: dict, root: Path) -> list[str]:
    errors: list[str] = []

    if sbom.get("bomFormat") != "dim-sbom":
        errors.append(f"bomFormat 'dim-sbom' değil: {sbom.get('bomFormat')!r}")
    if sbom.get("specVersion") != 1:
        errors.append(f"specVersion 1 değil: {sbom.get('specVersion')!r}")

    components = sbom.get("components") or {}
    counts = sbom.get("counts") or {}
    lists: dict[str, list[str]] = {}
    for key in COMPONENT_LISTS:
        value = components.get(key)
        if not isinstance(value, list):
            errors.append(f"components.{key} liste değil")
            lists[key] = []
            continue
        lists[key] = value
        if counts.get(key) != len(value):
            errors.append(
                f"counts.{key}={counts.get(key)!r} ama listede {len(value)} bileşen var"
            )

    # 2) Sabit sürümler ------------------------------------------------
    for key in ("rust", "node", "nodeDev"):
        for entry in lists.get(key, []):
            name, version = split_entry(entry)
            if not name or not version:
                errors.append(f"{key}: sürüm yok → {entry!r}")
                continue
            if any(hint in version.lower() for hint in FLOATING_HINTS):
                errors.append(f"{key}: sabit olmayan sürüm → {entry!r}")
            elif not SEMVER.match(version):
                errors.append(f"{key}: semver değil → {entry!r}")
    for entry in lists.get("nodeWorkspace", []):
        if "@" in entry.lstrip("@") and split_entry(entry)[1]:
            errors.append(f"nodeWorkspace: sürüm taşımamalı → {entry!r}")
    for key in ("python", "pythonDev"):
        for entry in lists.get(key, []):
            name, sep, version = entry.partition("==")
            if not sep or not name or not re.match(r"^\d", version):
                errors.append(f"{key}: 'ad==sürüm' değil → {entry!r}")

    # 3) Bildirilen her bağımlılık listede ------------------------------
    declared = declared_dependencies(root)
    present: set[str] = set()
    for key in ("node", "nodeDev", "nodeWorkspace"):
        for entry in lists.get(key, []):
            present.add(split_entry(entry)[0])
    missing = sorted(name for name in declared if name not in present)
    if missing:
        errors.append(
            "bildirilen ama SBOM'da olmayan bağımlılık: " + ", ".join(missing)
        )

    # 4) Geçişli bağımlılık kanıtı --------------------------------------
    node_total = len(lists.get("node", [])) + len(lists.get("nodeDev", []))
    if declared and node_total <= len(declared):
        errors.append(
            f"Node listesi geçişli bağımlılık taşımıyor: {node_total} bileşen, "
            f"{len(declared)} bildirilen bağımlılık (pnpm --depth düşürülmüş olabilir)"
        )
    if not lists.get("rust"):
        errors.append("rust bileşeni boş")
    if not lists.get("node"):
        errors.append("node bileşeni boş")

    # 5) Ürüne bağlama ---------------------------------------------------
    subject = sbom.get("subject") or {}
    versions = {
        field: subject.get(field)
        for field in ("version", "bundleVersion", "cargoVersion")
    }
    for field, value in versions.items():
        if not value:
            errors.append(f"subject.{field} boş")
        elif not SEMVER.match(str(value)):
            errors.append(f"subject.{field} semver değil: {value!r}")
    distinct = {value for value in versions.values() if value}
    if len(distinct) > 1:
        errors.append(
            "sürüm hizasız: " + ", ".join(f"{k}={v}" for k, v in versions.items())
        )
    commit = subject.get("commit") or ""
    if not COMMIT.match(commit):
        errors.append(f"subject.commit 40 haneli git özeti değil: {commit!r}")
    lockfiles = subject.get("lockfiles") or {}
    for rel in ("Cargo.lock", "pnpm-lock.yaml", "workers/document/pyproject.toml"):
        digest = lockfiles.get(rel)
        if not digest:
            errors.append(f"subject.lockfiles.{rel} yok")
        elif not SHA256.match(str(digest)):
            errors.append(f"subject.lockfiles.{rel} sha256 değil: {digest!r}")

    return errors


# --- öz test ---------------------------------------------------------------

PYPROJECT = """\
[project]
name = "dim-document-worker"
version = "1.2.3"
dependencies = []

[project.optional-dependencies]
pdf = ["pymupdf==1.24.14"]
dev = ["pytest==8.3.4"]
"""


def _write_fixture_root(base: Path) -> None:
    (base / "apps/desktop/src-tauri").mkdir(parents=True)
    (base / "packages/contracts").mkdir(parents=True)
    (base / "workers/document").mkdir(parents=True)
    (base / "package.json").write_text(json.dumps({"name": "kök", "version": "1.2.3"}))
    (base / "apps/desktop/package.json").write_text(
        json.dumps(
            {
                "name": "@dim/desktop",
                "version": "1.2.3",
                "dependencies": {"react": "=18.3.1", "@dim/contracts": "workspace:*"},
                "devDependencies": {"vite": "^6.0.3"},
            }
        )
    )
    (base / "packages/contracts/package.json").write_text(json.dumps({"name": "@dim/contracts"}))
    (base / "Cargo.toml").write_text('[workspace]\nversion = "1.2.3"\n')
    (base / "apps/desktop/src-tauri/tauri.conf.json").write_text(json.dumps({"version": "1.2.3"}))
    (base / "workers/document/pyproject.toml").write_text(PYPROJECT)


def _fixture_sbom(**overrides) -> dict:
    sbom = {
        "bomFormat": "dim-sbom",
        "specVersion": 1,
        "generatedAt": "2026-01-01T00:00:00+00:00",
        "subject": {
            "name": "@dim/desktop",
            "version": "1.2.3",
            "bundleVersion": "1.2.3",
            "cargoVersion": "1.2.3",
            "commit": "a" * 40,
            "lockfiles": {
                "Cargo.lock": "b" * 64,
                "pnpm-lock.yaml": "c" * 64,
                "workers/document/pyproject.toml": "d" * 64,
            },
        },
        "components": {
            "rust": ["serde@1.0.0", "sqlx@0.8.2"],
            "rustWorkspace": ["dim-storage"],
            "node": ["js-tokens@4.0.0", "react@18.3.1"],
            "nodeDev": ["@babel/core@7.29.7", "vite@6.0.3"],
            "nodeWorkspace": ["@dim/contracts"],
            "python": ["pymupdf==1.24.14"],
            "pythonDev": ["pytest==8.3.4"],
        },
    }
    sbom["counts"] = {
        key: len(value) for key, value in sbom["components"].items()
    }
    for path, value in overrides.items():
        cursor = sbom
        parts = path.split(".")
        for part in parts[:-1]:
            cursor = cursor[part]
        cursor[parts[-1]] = value
    return sbom


def self_test() -> int:
    failures: list[str] = []
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        _write_fixture_root(root)

        good = _fixture_sbom()
        if (errors := check(good, root)):
            failures.append(f"tam SBOM yeşil olmalıydı: {errors}")

        missing_dev = _fixture_sbom(**{"components.nodeDev": ["@babel/core@7.29.7"]})
        missing_dev["counts"]["nodeDev"] = 1
        errors = check(missing_dev, root)
        if not any("vite" in error for error in errors):
            failures.append("eksik devDependencies yakalanmadı")

        floating = _fixture_sbom(**{"components.nodeDev": ["vite@latest"]})
        errors = check(floating, root)
        if not any("sabit olmayan sürüm" in error for error in errors):
            failures.append("sabit olmayan sürüm yakalanmadı")

        shallow = _fixture_sbom(
            **{
                "components.node": ["react@18.3.1"],
                "components.nodeDev": ["vite@6.0.3"],
            }
        )
        shallow["counts"]["node"] = 1
        shallow["counts"]["nodeDev"] = 1
        errors = check(shallow, root)
        if not any("geçişli bağımlılık" in error for error in errors):
            failures.append("--depth 0 kusuru yakalanmadı")

        drift = _fixture_sbom(**{"subject.bundleVersion": "9.9.9"})
        errors = check(drift, root)
        if not any("sürüm hizasız" in error for error in errors):
            failures.append("sürüm uyuşmazlığı yakalanmadı")

        bad_count = _fixture_sbom(**{"counts.rust": 99})
        errors = check(bad_count, root)
        if not any("counts.rust" in error for error in errors):
            failures.append("sayaç uyuşmazlığı yakalanmadı")

    if failures:
        print("SBOM öz testi KIRMIZI:")
        for failure in failures:
            print(" -", failure)
        return 1
    print("SBOM öz testi yeşil (6 kural: tam/eksik-dev/sabit-sürüm/geçişli/hiza/sayaç)")
    return 0


def main(argv: list[str]) -> int:
    if "--self-test" in argv[1:]:
        return self_test()
    path = Path(argv[1]) if len(argv) > 1 else ROOT / "sbom.json"
    if not path.exists():
        print(f"SBOM bulunamadı: {path}")
        return 1
    sbom = json.loads(path.read_text())
    errors = check(sbom, ROOT)
    if errors:
        print("SBOM kapısı KIRMIZI:")
        for error in errors:
            print(" -", error)
        return 1
    counts = sbom.get("counts", {})
    print(
        "SBOM kapısı yeşil · "
        f"rust {counts.get('rust')} (+{counts.get('rustWorkspace')} çalışma alanı) · "
        f"node {counts.get('node')} üretim + {counts.get('nodeDev')} geliştirme "
        f"(+{counts.get('nodeWorkspace')} bağ) · "
        f"python {counts.get('python')} (+{counts.get('pythonDev')} geliştirme) · "
        f"sürüm {sbom.get('subject', {}).get('version')}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
