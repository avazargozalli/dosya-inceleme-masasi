#!/usr/bin/env python3
"""Kurgu test belgeleri üretir. Gerçek kişi/kurum verisi içermez.

PDF bağımlılıksız üretilir (PDF 1.4, tek sayfa, Helvetica). Türkçe karakterler
WinAnsi'de bulunmadığından örnek metin ASCII tutulur; Türkçe offset testleri
Rust tarafında ayrı yapılır.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path

TEXT = "Kira sozlesmesi 2026 - ornek belge. Odeme yapilmadi."


def _build_pdf(text: str) -> bytes:
    content = f"BT /F1 12 Tf 72 760 Td ({text}) Tj ET".encode("latin-1")
    objects: list[bytes] = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] "
        b"/Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        b"<< /Length " + str(len(content)).encode() + b" >>\nstream\n" + content + b"\nendstream",
    ]
    out = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
    offsets = [0]
    for i, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += f"{i} 0 obj\n".encode() + body + b"\nendobj\n"
    xref_at = len(out)
    out += f"xref\n0 {len(objects) + 1}\n".encode()
    out += b"0000000000 65535 f \n"
    for off in offsets[1:]:
        out += f"{off:010d} 00000 n \n".encode()
    out += f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n".encode()
    return bytes(out)


def generate_pdf(path: Path) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        path.unlink()
    path.write_bytes(_build_pdf(TEXT))
    return path


def generate_txt(path: Path) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(TEXT + "\n", encoding="utf-8")
    return path


# --- F01-T05 karartma fikstürü ---------------------------------------------
# Gizli nesne vektörleri tek dosyada: metin katmanı, sayfa dışı metin
# (MediaBox dışında x=900), yapışkan not (`/Contents` + `/T`) ve katalog içine
# gömülü JavaScript (`/OpenAction`). Hepsi kurgudur.
REDACTION_MARKERS = {
    "text": "ZZREDTEXT3141",
    "annot": "ZZREDANNOT2718",
    "offpage": "ZZREDOFFPAGE1618",
    "script": "ZZREDSCRIPT1123",
}


def _pdf_string(value: str) -> bytes:
    escaped = value.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")
    return b"(" + escaped.encode("latin-1") + b")"


def _assemble_pdf(objects: list[bytes]) -> bytes:
    out = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
    offsets = [0]
    for index, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += f"{index} 0 obj\n".encode() + body + b"\nendobj\n"
    xref_at = len(out)
    out += f"xref\n0 {len(objects) + 1}\n".encode()
    out += b"0000000000 65535 f \n"
    for offset in offsets[1:]:
        out += f"{offset:010d} 00000 n \n".encode()
    out += (
        f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n"
    ).encode()
    return bytes(out)


def build_redaction_pdf() -> bytes:
    content = (
        b"BT /F1 12 Tf 72 760 Td " + _pdf_string("Kira sozlesmesi - gorunur metin") + b" Tj ET\n"
        b"BT /F1 12 Tf 72 700 Td " + _pdf_string(REDACTION_MARKERS["text"]) + b" Tj ET\n"
        b"BT /F1 12 Tf 900 640 Td " + _pdf_string(REDACTION_MARKERS["offpage"]) + b" Tj ET"
    )
    annot = _pdf_string(REDACTION_MARKERS["annot"])
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R /OpenAction << /S /JavaScript /JS "
        + _pdf_string(REDACTION_MARKERS["script"])
        + b" >> >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] "
        b"/Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R /Annots [6 0 R] >>",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        b"<< /Length "
        + str(len(content)).encode()
        + b" >>\nstream\n"
        + content
        + b"\nendstream",
        b"<< /Type /Annot /Subtype /Text /Rect [300 500 320 520] /Contents "
        + annot
        + b" /T "
        + annot
        + b" /P 3 0 R >>",
    ]
    return _assemble_pdf(objects)


def generate_redaction_pdf(path: Path) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        path.unlink()
    path.write_bytes(build_redaction_pdf())
    return path


def main() -> None:
    parser = argparse.ArgumentParser(description="Kurgu test belgesi üret")
    parser.add_argument("--out", default="fixtures/generated", help="çıktı klasörü")
    args = parser.parse_args()
    out = Path(args.out)
    pdf = generate_pdf(out / "ornek-kira.pdf")
    txt = generate_txt(out / "ornek-kira.txt")
    redaction = generate_redaction_pdf(Path("fixtures/synthetic/redaction-hidden.pdf"))
    print(f"üretildi: {pdf} ({os.path.getsize(pdf)} bayt)")
    print(f"üretildi: {txt} ({os.path.getsize(txt)} bayt)")
    print(f"üretildi: {redaction} ({os.path.getsize(redaction)} bayt)")


if __name__ == "__main__":
    main()
