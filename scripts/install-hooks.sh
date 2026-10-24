#!/usr/bin/env bash
# Git hook'larını etkinleştirir. `core.hooksPath` .git/config'e yazıldığı için
# her klonda bir kez çalıştırılır (README'de not edilir).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
git -C "$ROOT" config core.hooksPath scripts/hooks
chmod +x "$ROOT"/scripts/hooks/* "$ROOT"/scripts/*.sh 2>/dev/null || true
echo "hooks etkin: core.hooksPath=scripts/hooks"
echo "commit sonrası: scripts/disk-guard.sh --quiet (arka plan)"
