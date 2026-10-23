# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller onedir spec'i. Worker tek ikili olarak paketlenir; Tesseract
ikilisi ve dil verisi **paketlenmez** (sistem bağımlılığı, OCR için gerekir).
"""

import sys

block_cipher = None

hidden = [
    "pymupdf",
    "fitz",
    "docx",
    "PIL",
    "PIL.Image",
    "pytesseract",
]

a = Analysis(
    ["dim_worker/__main__.py"],
    pathex=["."],
    binaries=[],
    datas=[],
    hiddenimports=hidden,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=["tkinter", "matplotlib", "IPython", "pytest"],
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name="dim-worker",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)

coll = COLLECT(
    exe,
    a.binaries,
    a.zipfiles,
    a.datas,
    strip=False,
    upx=False,
    upx_exclude=[],
    name="dim-worker",
)
