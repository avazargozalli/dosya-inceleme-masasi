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


#: Fixture metinleri: envanterin **içeriği** dışarı vermediğini test eden
#: işaretçiler. Gerçek kişi/kurum verisi değil, kurgu.
INVENTORY_MARKERS = {
    "body": "Kira sozlesmesi 2026 - ornek envanter belgesi.",
    "inserted": "Yillik %12 faiz isletilecektir.",
    "deleted": "Yillik %30 faiz isletilecektir.",
    "footnote": "Damga vergisi kiraciya aittir.",
    "comment": "Bu hukum riskli.",
    "textbox": "Metin kutusu icerigi ayrica cizilir.",
    "ole": "ZZ-OLE-ICERIK-GIZLI",
    "image": "ZZ-GORSEL-ICERIK",
    "altchunk": "ZZ-ALTCHUNK-ICERIK",
    "signature": "ZZ-IMZA-PARCASI",
    "udfImage": "ZZ-UDF-GORSEL",
    "udfSignature": "ZZ-UDF-IMZA",
    "udfEmbedded": "ZZ-UDF-PDF",
}

W_NS = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
REL_NS = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
PKG_REL_NS = "http://schemas.openxmlformats.org/package/2006/relationships"
CT_NS = "http://schemas.openxmlformats.org/package/2006/content-types"
CP_NS = "http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
DC_NS = "http://purl.org/dc/elements/1.1/"
DCTERMS_NS = "http://purl.org/dc/terms/"


def _docx_zip(parts: dict[str, bytes]) -> bytes:
    import io as _io
    import zipfile

    buffer = _io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, blob in parts.items():
            archive.writestr(name, blob)
    return buffer.getvalue()


def build_docx_inventory() -> bytes:
    """Kurgu DOCX: yapı + gömülü nesne + altChunk + izlenen değişiklik.

    Bağımlılıksız üretilir (elle OOXML); python-docx gerektirmez ve
    `python-docx`in gövdeye katmadığı parçaları **gerçekten** taşır.
    """
    markers = INVENTORY_MARKERS
    document = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<w:document xmlns:w="{W_NS}" xmlns:r="{REL_NS}"><w:body>'
        f"<w:p><w:r><w:t>{markers['body']}</w:t></w:r></w:p>"
        f'<w:p><w:ins w:id="101" w:author="Av. Deneme" w:date="2026-10-01T09:00:00Z">'
        f"<w:r><w:t>{markers['inserted']}</w:t></w:r></w:ins></w:p>"
        f'<w:p><w:del w:id="102" w:author="Karsi Taraf" w:date="2026-10-02T09:00:00Z">'
        f"<w:r><w:delText>{markers['deleted']}</w:delText></w:r></w:del></w:p>"
        "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Kalem</w:t></w:r></w:p></w:tc>"
        "<w:tc><w:p><w:r><w:t>Tutar</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"
        "<w:p><w:r><w:pict><w:txbxContent><w:p><w:r><w:t>"
        f"{markers['textbox']}"
        "</w:t></w:r></w:p></w:txbxContent></w:pict></w:r>"
        '<w:r><w:drawing><wp:inline xmlns:wp="http://schemas.openxmlformats.org/drawingml/'
        '2006/wordprocessingDrawing"><wp:extent cx="100" cy="100"/></wp:inline></w:drawing></w:r></w:p>'
        "<w:sdt><w:sdtPr/><w:sdtContent><w:p><w:r><w:t>Kontrollu icerik.</w:t></w:r></w:p>"
        "</w:sdtContent></w:sdt>"
        "<w:p><w:r><w:object><o:OLEObject xmlns:o=\"urn:schemas-microsoft-com:office:office\" "
        'ProgID="Excel.Sheet.12" r:id="rIdOle1"/></w:object></w:r></w:p>'
        "<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/></w:sectPr>"
        "</w:body></w:document>"
    ).encode("utf-8")

    footnotes = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<w:footnotes xmlns:w="{W_NS}">'
        '<w:footnote w:id="-1" w:type="separator"><w:p><w:r><w:separator/></w:r></w:p></w:footnote>'
        '<w:footnote w:id="0" w:type="continuationSeparator">'
        "<w:p><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>"
        '<w:footnote w:id="1"><w:p><w:r><w:t>'
        f"{markers['footnote']}"
        "</w:t></w:r></w:p></w:footnote></w:footnotes>"
    ).encode("utf-8")

    comments = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<w:comments xmlns:w="{W_NS}">'
        '<w:comment w:id="1" w:author="Av. Deneme" w:initials="AD"><w:p><w:r><w:t>'
        f"{markers['comment']}"
        "</w:t></w:r></w:p></w:comment></w:comments>"
    ).encode("utf-8")

    settings = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<w:settings xmlns:w="{W_NS}"><w:trackRevisions/>'
        '<w:documentProtection w:edit="readOnly" w:enforcement="1"/>'
        "</w:settings>"
    ).encode("utf-8")

    styles = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<w:styles xmlns:w="{W_NS}">'
        '<w:style w:type="paragraph" w:styleId="Normal"><w:name w:val="Normal"/></w:style>'
        '<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="Heading 1"/></w:style>'
        "</w:styles>"
    ).encode("utf-8")

    header = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<w:hdr xmlns:w="{W_NS}"><w:p><w:r><w:t>Ustbilgi metni.</w:t></w:r></w:p></w:hdr>'
    ).encode("utf-8")

    core = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<cp:coreProperties xmlns:cp="{CP_NS}" xmlns:dc="{DC_NS}" xmlns:dcterms="{DCTERMS_NS}">'
        "<dc:creator>Av. Deneme</dc:creator>"
        "<cp:lastModifiedBy>Av. Karsi</cp:lastModifiedBy>"
        "<cp:revision>12</cp:revision>"
        "<dcterms:created>2026-09-01T09:00:00Z</dcterms:created>"
        "<dcterms:modified>2026-10-02T10:00:00Z</dcterms:modified>"
        "</cp:coreProperties>"
    ).encode("utf-8")

    rels = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<Relationships xmlns="{PKG_REL_NS}">'
        f'<Relationship Id="rId1" Type="{REL_NS}/footnotes" Target="footnotes.xml"/>'
        f'<Relationship Id="rId2" Type="{REL_NS}/comments" Target="comments.xml"/>'
        f'<Relationship Id="rId3" Type="{REL_NS}/header" Target="header1.xml"/>'
        f'<Relationship Id="rId4" Type="{REL_NS}/oleObject" Target="embeddings/oleObject1.bin"/>'
        f'<Relationship Id="rId5" Type="{REL_NS}/aFChunk" Target="afchunk1.mht"/>'
        f'<Relationship Id="rId9" Type="{REL_NS}/hyperlink" Target="https://ornek.test/emsal" '
        'TargetMode="External"/>'
        "</Relationships>"
    ).encode("utf-8")

    content_types = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<Types xmlns="{CT_NS}">'
        '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
        '<Default Extension="xml" ContentType="application/xml"/>'
        '<Default Extension="png" ContentType="image/png"/>'
        '<Default Extension="bin" ContentType="application/vnd.openxmlformats-officedocument.'
        'oleObject"/>'
        '<Default Extension="mht" ContentType="message/rfc822"/>'
        '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-'
        'officedocument.wordprocessingml.document.main+xml"/>'
        '<Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-'
        'officedocument.wordprocessingml.footnotes+xml"/>'
        '<Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-'
        'officedocument.wordprocessingml.comments+xml"/>'
        "</Types>"
    ).encode("utf-8")

    package_rels = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<Relationships xmlns="{PKG_REL_NS}">'
        f'<Relationship Id="rId1" Type="{REL_NS}/officeDocument" Target="word/document.xml"/>'
        '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/'
        'relationships/metadata/core-properties" Target="docProps/core.xml"/>'
        "</Relationships>"
    ).encode("utf-8")

    return _docx_zip(
        {
            "[Content_Types].xml": content_types,
            "_rels/.rels": package_rels,
            "word/document.xml": document,
            "word/_rels/document.xml.rels": rels,
            "word/footnotes.xml": footnotes,
            "word/comments.xml": comments,
            "word/settings.xml": settings,
            "word/styles.xml": styles,
            "word/header1.xml": header,
            "word/afchunk1.mht": (
                f"<html><body>{markers['altchunk']}</body></html>"
            ).encode("utf-8"),
            "word/embeddings/oleObject1.bin": (
                b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1" + markers["ole"].encode("utf-8") * 4
            ),
            # 1x1 PNG (gerçek baytlar) + kurgu işaretçi kuyruğu.
            "word/media/image1.png": bytes.fromhex(
                "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4"
                "890000000a49444154789c6360000002000154a24f640000000049454e44ae42"
                "6082"
            )
            + markers["image"].encode("utf-8"),
            "_xmlsignatures/sig1.xml": (
                f"<Signature>{markers['signature']}</Signature>"
            ).encode("utf-8"),
            "docProps/core.xml": core,
        }
    )


def generate_docx_inventory(path: Path) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        path.unlink()
    path.write_bytes(build_docx_inventory())
    return path


def build_udf_inventory() -> bytes:
    """Kurgu UDF: paragraf/tablo/üstbilgi + gömülü dosya + imza + bilinmeyen öğe."""
    markers = INVENTORY_MARKERS
    pool = "Birinci paragraf metni. Ikinci paragraf metni. Hucre metni."
    content = (
        '<?xml version="1.0" encoding="UTF-8"?>'
        "<template>"
        f"<content>{pool}</content>"
        "<elements>"
        '<page><paragraph startOffset="0" length="22"/>'
        '<paragraph startOffset="23" length="21"/>'
        '<table><row><cell><paragraph startOffset="45" length="11"/></cell></row></table>'
        '<image startOffset="0" length="0"/></page>'
        "</elements>"
        '<header><paragraph startOffset="0" length="7"/></header>'
        '<footer><paragraph startOffset="23" length="21"/></footer>'
        '<bilinmeyenOge startOffset="0" length="0"/>'
        "</template>"
    ).encode("utf-8")
    return _docx_zip(
        {
            "content.xml": content,
            "ekler/resim1.jpg": b"\xff\xd8\xff\xe0" + markers["udfImage"].encode("utf-8") * 8,
            "ekler/karar.pdf": b"%PDF-1.4 " + markers["udfEmbedded"].encode("utf-8"),
            "signature.p7s": markers["udfSignature"].encode("utf-8") * 6,
        }
    )


def generate_udf_inventory(path: Path) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        path.unlink()
    path.write_bytes(build_udf_inventory())
    return path


def main() -> None:
    parser = argparse.ArgumentParser(description="Kurgu test belgesi üret")
    parser.add_argument("--out", default="fixtures/generated", help="çıktı klasörü")
    args = parser.parse_args()
    out = Path(args.out)
    pdf = generate_pdf(out / "ornek-kira.pdf")
    txt = generate_txt(out / "ornek-kira.txt")
    redaction = generate_redaction_pdf(Path("fixtures/synthetic/redaction-hidden.pdf"))
    docx = generate_docx_inventory(Path("fixtures/synthetic/docx-inventory.docx"))
    udf = generate_udf_inventory(Path("fixtures/synthetic/udf-inventory.udf"))
    print(f"üretildi: {pdf} ({os.path.getsize(pdf)} bayt)")
    print(f"üretildi: {txt} ({os.path.getsize(txt)} bayt)")
    print(f"üretildi: {redaction} ({os.path.getsize(redaction)} bayt)")
    print(f"üretildi: {docx} ({os.path.getsize(docx)} bayt)")
    print(f"üretildi: {udf} ({os.path.getsize(udf)} bayt)")


if __name__ == "__main__":
    main()
