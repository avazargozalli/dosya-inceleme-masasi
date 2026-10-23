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


def main() -> None:
    parser = argparse.ArgumentParser(description="Kurgu test belgesi üret")
    parser.add_argument("--out", default="fixtures/generated", help="çıktı klasörü")
    args = parser.parse_args()
    out = Path(args.out)
    pdf = generate_pdf(out / "ornek-kira.pdf")
    txt = generate_txt(out / "ornek-kira.txt")
    print(f"üretildi: {pdf} ({os.path.getsize(pdf)} bayt)")
    print(f"üretildi: {txt} ({os.path.getsize(txt)} bayt)")


if __name__ == "__main__":
    main()
