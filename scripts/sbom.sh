#!/usr/bin/env bash
# Yazılım malzeme listesi (SBOM). Üç ekosistemi tek JSON'da birleştirir:
# Rust (cargo metadata), Node (pnpm list) ve Python (workers/document/pyproject.toml).
#
# Kurallar:
# - Sürümler **sabittir**: her bileşen `ad@sürüm` yazar; "latest"/"*"/aralık yazılmaz.
# - Node listesi **tam ağaçtır** (`--depth Infinity`): yalnız doğrudan bağımlılıklar
#   değil geçişli bağımlılıklar da girer; üretim ve geliştirme ağaçları ayrı yazılır.
#   (Eski betik `--depth 0` ile yalnız 5 üretim bağımlılığı yazıyordu — ölçüm:
#   docs/releases/F15/README.md → "SBOM kapsamı" bölümü.)
# - Çıktı deterministiktir: listeler sıralıdır, tek değişken alan `generatedAt`tir.
# - SBOM ürüne **bağlanır**: `subject` bloğu uygulama/paket sürümünü, commit'i ve
#   kilit dosyalarının sha256 özetini taşır (F16-T01 için zemin).
# - Kapı: `scripts/sbom_check.py` (CI'da koşar) listeyi bağımsız kaynaklarla doğrular.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-$ROOT/sbom.json}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

cd "$ROOT"
cargo metadata --format-version 1 --all-features > "$TMP/cargo.json"
# `--depth Infinity` geçişli bağımlılıkları da verir; `-r` bütün çalışma alanını gezer.
pnpm list -r --depth Infinity --json > "$TMP/pnpm.json"

python3 - "$TMP" "$OUT" "$ROOT" <<'PY'
import hashlib
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

tmp = Path(sys.argv[1])
out = Path(sys.argv[2])
root = Path(sys.argv[3])


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


# --- Rust ---------------------------------------------------------------
cargo = json.loads((tmp / "cargo.json").read_text())
rust, rust_workspace = set(), set()
for pkg in cargo.get("packages", []):
    source = pkg.get("source") or ""
    if not source:
        # Yol/çalışma alanı üyesi: ürünün kendi kodu, üçüncü taraf bileşen değil.
        rust_workspace.add(pkg["name"])
    elif "registry+https://github.com/rust-lang/crates.io" in source:
        rust.add(f"{pkg['name']}@{pkg['version']}")
rust = sorted(rust)
rust_workspace = sorted(rust_workspace)

# --- Node ---------------------------------------------------------------
# pnpm JSON'u proje başına iç içe ağaç verir: {ad: {version, dependencies: {...}}}.
# Geçişli düğümler yalnız `dependencies` altında görünür, bu yüzden iki gövde de
# özyinelemeli yürünür. `dev-` öneki taşıyan sürümler (workspace bağı) sürüm değildir.
node_raw = json.loads((tmp / "pnpm.json").read_text())
projects = node_raw if isinstance(node_raw, list) else [node_raw]


def walk(deps, acc):
    for name, info in (deps or {}).items():
        info = info or {}
        version = info.get("version")
        if version:
            acc.add((name, version))
        walk(info.get("dependencies"), acc)


prod, dev, linked = set(), set(), set()
for project in projects:
    walk(project.get("dependencies"), prod)
    walk(project.get("devDependencies"), dev)

# Çalışma alanı bağları (`link:`/`file:`) sürüm taşımaz → ayrı liste.
for bucket in (prod, dev):
    for name, version in list(bucket):
        if version.startswith("link:") or version.startswith("file:"):
            bucket.discard((name, version))
            linked.add(name)

# Geliştirme ağacında olup üretimde de bulunan paket üretim tarafında kalır.
dev = {pair for pair in dev if pair not in prod}

# Aynı ad birden çok sürümle geçebilir; ikisi de listede kalır (SBOM ayrıntısı).
node = sorted(f"{name}@{version}" for name, version in prod)
node_dev = sorted(f"{name}@{version}" for name, version in dev)
node_workspace = sorted(linked)

# --- Python -------------------------------------------------------------
# `pyproject.toml` yalnız **bildirilen** pinleri taşır (geçişli Python bağımlılıkları
# burada yoktur; paketlenmiş worker'ın içeriği `scripts/worker_manifest.py` ile ayrı
# doğrulanır — dürüst sınır olarak belgede yazılı).
pyproject_text = (root / "workers/document/pyproject.toml").read_text()
groups = {}
for match in re.finditer(r"^([A-Za-z0-9_-]+)\s*=\s*\[(.*?)\]", pyproject_text, re.M | re.S):
    pins = re.findall(r'"([A-Za-z0-9_.\-]+==[0-9][^"]*)"', match.group(2))
    if pins:
        groups[match.group(1)] = set(pins)
python_runtime = set().union(*(pins for name, pins in groups.items() if name != "dev"))
python = sorted(python_runtime)
python_dev = sorted(groups.get("dev", set()) - python_runtime)

# --- Ürüne bağlama (subject) -------------------------------------------
app_pkg = json.loads((root / "apps/desktop/package.json").read_text())
bundle = json.loads((root / "apps/desktop/src-tauri/tauri.conf.json").read_text())
cargo_toml = (root / "Cargo.toml").read_text()
cargo_version = re.search(r'^version\s*=\s*"([^"]+)"', cargo_toml, re.M)
commit = ""
try:
    commit = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, text=True, check=False
    ).stdout.strip()
except OSError:
    commit = ""

lockfiles = {}
for rel in ("Cargo.lock", "pnpm-lock.yaml", "workers/document/pyproject.toml"):
    path = root / rel
    if path.exists():
        lockfiles[rel] = sha256(path)

payload = {
    "bomFormat": "dim-sbom",
    "specVersion": 1,
    "generatedAt": datetime.now(timezone.utc).isoformat(),
    "subject": {
        "name": app_pkg.get("name"),
        "version": app_pkg.get("version"),
        "bundleVersion": bundle.get("version"),
        "cargoVersion": cargo_version.group(1) if cargo_version else None,
        "commit": commit,
        "lockfiles": lockfiles,
    },
    "components": {
        "rust": rust,
        "rustWorkspace": rust_workspace,
        "node": node,
        "nodeDev": node_dev,
        "nodeWorkspace": node_workspace,
        "python": python,
        "pythonDev": python_dev,
    },
    "counts": {
        "rust": len(rust),
        "rustWorkspace": len(rust_workspace),
        "node": len(node),
        "nodeDev": len(node_dev),
        "nodeWorkspace": len(node_workspace),
        "python": len(python),
        "pythonDev": len(python_dev),
    },
}
out.write_text(json.dumps(payload, indent=2, ensure_ascii=False, sort_keys=True) + "\n")
print(f"SBOM yazıldı: {out} ({payload['counts']})")
PY
