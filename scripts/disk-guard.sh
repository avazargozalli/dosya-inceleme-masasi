#!/usr/bin/env bash
# Disk koruması: her commit / iş periyodu sonunda çalışır.
#
# Kural: yalnız **yeniden üretilebilir** çıktılar silinir (cargo target,
# PyInstaller ara çıktıları, paketlenmiş worker, gerekiyorsa node_modules).
# Kaynak, docs, .git ve kullanıcı verisine dokunulmaz.
#
# Eşikler (ortam değişkeniyle değiştirilebilir):
#   DIM_DISK_MIN_GB  = 5   → boş alan bunun altındaysa temizle
#   DIM_REPO_MAX_GB  = 2   → repo bunun üstündeyse temizle
#   DIM_HARD_MIN_GB  = 4   → bunun altındaysa node_modules de silinir
#
# Kullanım: bash scripts/disk-guard.sh [--quiet] [--force]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
QUIET=0
FORCE=0
for arg in "$@"; do
  case "$arg" in
    --quiet) QUIET=1 ;;
    --force) FORCE=1 ;;
  esac
done

MIN_FREE_GB="${DIM_DISK_MIN_GB:-5}"
MAX_REPO_GB="${DIM_REPO_MAX_GB:-2}"
HARD_MIN_GB="${DIM_HARD_MIN_GB:-4}"

free_kb="$(df -Pk / | awk 'NR==2 {print $4}')"
free_gb=$((free_kb / 1024 / 1024))
repo_kb="$(du -sk "$ROOT" | awk '{print $1}')"
repo_gb=$((repo_kb / 1024 / 1024))

log() { [ "$QUIET" -eq 1 ] || echo "disk-guard: $*"; }

if [ "$FORCE" -eq 0 ] && [ "$free_gb" -ge "$MIN_FREE_GB" ] && [ "$repo_gb" -lt "$MAX_REPO_GB" ]; then
  log "temiz (boş ${free_gb}G ≥ ${MIN_FREE_GB}G, repo ${repo_gb}G < ${MAX_REPO_GB}G)"
  exit 0
fi

before_gb="$repo_gb"

# 1) Rust derleme çıktıları — kök workspace ve Tauri ayrı workspace.
if command -v cargo >/dev/null 2>&1; then
  cargo clean --manifest-path "$ROOT/Cargo.toml" >/dev/null 2>&1 || true
  cargo clean --manifest-path "$ROOT/apps/desktop/src-tauri/Cargo.toml" >/dev/null 2>&1 || true
fi

# 2) PyInstaller / paket çıktıları (gitignore'da, CI üretir).
rm -rf "$ROOT/workers/document/build" "$ROOT/workers/document/dist" "$ROOT/.worker-venv"
if [ -d "$ROOT/apps/desktop/src-tauri/resources/worker" ]; then
  find "$ROOT/apps/desktop/src-tauri/resources/worker" -mindepth 1 -maxdepth 1 \
    ! -name '.gitkeep' ! -name 'README.md' -exec rm -rf {} + 2>/dev/null || true
fi

# 3) Çok sıkışırsa node_modules (pnpm store'dan geri gelir).
if [ "$free_gb" -lt "$HARD_MIN_GB" ] || [ "$FORCE" -eq 1 ]; then
  rm -rf "$ROOT/node_modules" "$ROOT/apps/desktop/node_modules"
fi

# 4) Hermes scratch: eski venv'ler, loglar, indirilen artefaktlar.
SCRATCH="${HOME}/.hermes/cache/scratch"
if [ -d "$SCRATCH" ]; then
  rm -rf "$SCRATCH"/dimvenv "$SCRATCH"/dim-f02 "$SCRATCH"/fmt* 2>/dev/null || true
  find "$SCRATCH" -maxdepth 1 -type f -name 'f*.log' -delete 2>/dev/null || true
fi

after_kb="$(du -sk "$ROOT" | awk '{print $1}')"
after_gb=$((after_kb / 1024 / 1024))
free_after_gb=$(( $(df -Pk / | awk 'NR==2 {print $4}') / 1024 / 1024 ))
log "temizlendi: repo ${before_gb}G → ${after_gb}G · boş ${free_gb}G → ${free_after_gb}G"
