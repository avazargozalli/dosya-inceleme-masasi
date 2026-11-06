#!/usr/bin/env bash
# Belge worker'ını PyInstaller onedir olarak paketler, Tauri kaynaklarına koyar
# ve paketin manifestini (dosya + sha256 + rol) üretip doğrular.
# Tesseract ikilisi/dil verisi paketlenmez; OCR için sistemde kurulu olmalıdır.
# Windows'ta da koşar (git-bash): venv düzeni `Scripts/`, çıktı `dim-worker.exe`.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENV="${DIM_WORKER_VENV:-$ROOT/.worker-venv}"
PYTHON="${PYTHON:-python3}"
PYINSTALLER_VERSION="6.11.1"
DEST="$ROOT/apps/desktop/src-tauri/resources/worker"
DIST="$ROOT/workers/document/dist/dim-worker"
MANIFEST_TOOL="$ROOT/scripts/worker_manifest.py"

# git-bash (MSYS) yolları `/d/a/...` biçimindedir; **yerel** Python bunları
# çözemez. Python'a argüman olarak geçen her yol `cygpath -w` ile Windows
# biçimine çevrilir. Windows dışında `cygpath` yoktur → yol kimlik döner.
to_native() {
  if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s' "$1"; fi
}

case "$("$PYTHON" -c 'import sys; print(sys.platform)')" in
  win32) EXE=".exe" ;;
  *) EXE="" ;;
esac

"$PYTHON" -m venv "$(to_native "$VENV")"
# Windows venv'i `bin/` değil `Scripts/` kullanır; tek yol seçilir, ikisi de yoksa durulur.
if [[ -f "$VENV/Scripts/python.exe" ]]; then
  VENV_PY="$VENV/Scripts/python.exe"
else
  VENV_PY="$VENV/bin/python"
fi
if [[ ! -f "$VENV_PY" ]]; then
  echo "HATA: venv yorumlayıcısı bulunamadı ($VENV_PY)" >&2
  exit 1
fi

"$VENV_PY" -m pip install --quiet --upgrade pip
"$VENV_PY" -m pip install --quiet -e "$(to_native "$ROOT/workers/document")[all]" "pyinstaller==$PYINSTALLER_VERSION"

cd "$ROOT/workers/document"
# `-m PyInstaller` iki platformda da aynı yolu kullanır (`bin/pyinstaller` ↔
# `Scripts/pyinstaller.exe` ayrımı ortadan kalkar).
"$VENV_PY" -m PyInstaller --noconfirm --clean dim-worker.spec

BIN="$DIST/dim-worker$EXE"
if [[ ! -f "$BIN" ]] || [[ ! -s "$BIN" ]]; then
  echo "HATA: paketlenmiş worker ikilisi üretilmedi ($BIN)" >&2
  exit 1
fi

rm -rf "$DEST"
mkdir -p "$DEST"
cp -R "$DIST/." "$DEST/"

# Depoda izlenen iki dosya silinmez: `.gitkeep` Tauri glob'unun hedefi,
# `README.md` dizinin açıklaması. Manifest ikisini de listelemez.
git -C "$ROOT" checkout -- apps/desktop/src-tauri/resources/worker/.gitkeep \
  apps/desktop/src-tauri/resources/worker/README.md 2>/dev/null || true

DEST_BIN="$DEST/dim-worker$EXE"
if [[ ! -f "$DEST_BIN" ]] || [[ ! -s "$DEST_BIN" ]]; then
  echo "HATA: kopyalanan worker ikilisi yok ($DEST_BIN)" >&2
  exit 1
fi
if [[ -z "$EXE" && ! -x "$DEST_BIN" ]]; then
  echo "HATA: kopyalanan worker ikilisi çalıştırılabilir değil" >&2
  exit 1
fi

# "Üretildi" ile "çalışıyor" aynı şey değil: dizin dolu görünse de ikili
# konuşmayabilir (eksik DLL, yanlış hidden import). Tek satırlık gerçek tur.
PING_REPLY="$(printf '{"v":1,"id":"1","method":"ping","params":{}}\n' | "$DEST_BIN")"
if [[ "$PING_REPLY" != *'"pong":true'* ]]; then
  echo "HATA: paketlenmiş worker ping yanıtı vermedi: $PING_REPLY" >&2
  exit 1
fi

WORKER_VERSION="$("$VENV_PY" -c 'import dim_worker; print(getattr(dim_worker, "__version__", "0.1.0"))' 2>/dev/null || echo "0.1.0")"
PYTHON_VERSION="$("$VENV_PY" -c 'import platform; print(platform.python_version())')"

# Platform etiketi tek yerden gelir (`worker_manifest._platform_tag`, `amd64`
# → `x86_64` normalizasyonu orada): burada ikinci bir kural tutulmaz.
"$PYTHON" "$(to_native "$MANIFEST_TOOL")" generate \
  --dir "$(to_native "$DEST")" \
  --version "$WORKER_VERSION" \
  --python "$PYTHON_VERSION" \
  --pyinstaller "$PYINSTALLER_VERSION"

"$PYTHON" "$(to_native "$MANIFEST_TOOL")" verify --dir "$(to_native "$DEST")"

echo "Worker paketlendi: $DEST_BIN"
