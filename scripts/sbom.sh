#!/usr/bin/env bash
# Yazılım malzeme listesi (SBOM). Üç ekosistemi tek JSON'da birleştirir:
# Rust (cargo metadata), Node (pnpm list), Python (workers/document/pyproject.toml).
# Sürümler sabittir; "latest" yazılmaz.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-$ROOT/sbom.json}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

cd "$ROOT"
cargo metadata --format-version 1 --all-features > "$TMP/cargo.json"
pnpm list -r --depth 0 --json > "$TMP/pnpm.json"

python3 - "$TMP" "$OUT" <<'PY'
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

tmp = Path(sys.argv[1])
out = Path(sys.argv[2])

cargo = json.loads((tmp / "cargo.json").read_text())
rust = sorted(
    {
        f"{pkg['name']}@{pkg['version']}"
        for pkg in cargo.get("packages", [])
        if pkg.get("source") is None or "registry+https://github.com/rust-lang/crates.io" in pkg.get("source", "")
    }
)

node_raw = json.loads((tmp / "pnpm.json").read_text())
node = []
for project in node_raw if isinstance(node_raw, list) else [node_raw]:
    for name, info in (project.get("dependencies") or {}).items():
        version = (info or {}).get("version")
        if version:
            node.append(f"{name}@{version}")
node = sorted(set(node))

import re

pyproject = Path("workers/document/pyproject.toml").read_text()
python = sorted(set(re.findall(r'"([A-Za-z0-9_.\-]+==[0-9][^"]*)"', pyproject)))

payload = {
    "bomFormat": "dim-sbom",
    "specVersion": 1,
    "generatedAt": datetime.now(timezone.utc).isoformat(),
    "components": {
        "rust": rust,
        "node": node,
        "python": python,
    },
    "counts": {"rust": len(rust), "node": len(node), "python": len(python)},
}
out.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n")
print(f"SBOM yazıldı: {out} ({payload['counts']})")
PY
