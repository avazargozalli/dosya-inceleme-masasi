#!/usr/bin/env bash
# Belge worker'ını PyInstaller onedir olarak paketler, Tauri kaynaklarına koyar
# ve paketin manifestini (dosya + sha256 + rol) üretip doğrular.
# Tesseract ikilisi/dil verisi paketlenmez; OCR için sistemde kurulu olmalıdır.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENV="${DIM_WORKER_VENV:-$ROOT/.worker-venv}"
PYTHON="${PYTHON:-python3}"
PYINSTALLER_VERSION="6.11.1"
DEST="$ROOT/apps/desktop/src-tauri/resources/worker"
DIST="$ROOT/workers/document/dist/dim-worker"
MANIFEST_TOOL="$ROOT/scripts/worker_manifest.py"

"$PYTHON" -m venv "$VENV"
"$VENV/bin/python" -m pip install --quiet --upgrade pip
"$VENV/bin/python" -m pip install --quiet -e "$ROOT/workers/document[all]" "pyinstaller==$PYINSTALLER_VERSION"

cd "$ROOT/workers/document"
"$VENV/bin/pyinstaller" --noconfirm --clean dim-worker.spec

if [[ ! -x "$DIST/dim-worker" ]]; then
  echo "HATA: paketlenmiş worker ikilisi üretilmedi ($DIST/dim-worker)" >&2
  exit 1
fi

rm -rf "$DEST"
mkdir -p "$DEST"
cp -R "$DIST/." "$DEST/"

# Depoda izlenen iki dosya silinmez: `.gitkeep` Tauri glob'unun hedefi,
# `README.md` dizinin açıklaması. Manifest ikisini de listelemez.
git -C "$ROOT" checkout -- apps/desktop/src-tauri/resources/worker/.gitkeep \
  apps/desktop/src-tauri/resources/worker/README.md 2>/dev/null || true

if [[ ! -x "$DEST/dim-worker" ]]; then
  echo "HATA: kopyalanan worker ikilisi çalıştırılabilir değil" >&2
  exit 1
fi

WORKER_VERSION="$("$VENV/bin/python" -c 'import dim_worker; print(getattr(dim_worker, "__version__", "0.1.0"))' 2>/dev/null || echo "0.1.0")"
PYTHON_VERSION="$("$VENV/bin/python" -c 'import platform; print(platform.python_version())')"
PLATFORM_TAG="$("$VENV/bin/python" -c 'import platform; s=platform.system().lower(); s="macos" if s=="darwin" else s; m=platform.machine().lower(); m="arm64" if m in ("aarch64","arm64") else m; print(f"{s}-{m}")')"

"$PYTHON" "$MANIFEST_TOOL" generate \
  --dir "$DEST" \
  --version "$WORKER_VERSION" \
  --platform "$PLATFORM_TAG" \
  --python "$PYTHON_VERSION" \
  --pyinstaller "$PYINSTALLER_VERSION"

"$PYTHON" "$MANIFEST_TOOL" verify --dir "$DEST"

echo "Worker paketlendi: $DEST/dim-worker"
