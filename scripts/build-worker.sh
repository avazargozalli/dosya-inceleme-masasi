#!/usr/bin/env bash
# Belge worker'ını PyInstaller onedir olarak paketler ve Tauri kaynaklarına koyar.
# Tesseract ikilisi/dil verisi paketlenmez; OCR için sistemde kurulu olmalıdır.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENV="${DIM_WORKER_VENV:-$ROOT/.worker-venv}"
PYTHON="${PYTHON:-python3}"

"$PYTHON" -m venv "$VENV"
"$VENV/bin/python" -m pip install --quiet --upgrade pip
"$VENV/bin/python" -m pip install --quiet -e "$ROOT/workers/document[all]" "pyinstaller==6.11.1"

cd "$ROOT/workers/document"
"$VENV/bin/pyinstaller" --noconfirm --clean dim-worker.spec

DEST="$ROOT/apps/desktop/src-tauri/resources/worker"
rm -rf "$DEST"
mkdir -p "$DEST"
cp -R "$ROOT/workers/document/dist/dim-worker/." "$DEST/"

if [[ ! -x "$DEST/dim-worker" ]]; then
  echo "HATA: paketlenmiş worker ikilisi üretilmedi" >&2
  exit 1
fi
echo "Worker paketlendi: $DEST/dim-worker"
