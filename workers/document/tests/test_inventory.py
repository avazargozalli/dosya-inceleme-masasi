"""Belge envanteri testleri (F01-T06): yapı, ekler, değişiklik izi.

Paketler gerçek OOXML/UDF olarak üretilir (python-docx gövdesi + gerçek ek
parçalar); kod yolu taklit edilmez. İki yön aranır: sayı **gerçekten** pakette
duruyor (parça/varlık doğrulanır) ve envanter **gövde metnini dışarı vermiyor**.
"""

from __future__ import annotations

import io
import json
import zipfile

import pytest

from dim_worker import inventory
from dim_worker.inventory import InventoryError, docx_inventory, inventory as build_inventory, udf_inventory

W_NS = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
REL_NS = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
PKG_REL_NS = "http://schemas.openxmlformats.org/package/2006/relationships"
CP_NS = "http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
DC_NS = "http://purl.org/dc/elements/1.1/"
DCTERMS_NS = "http://purl.org/dc/terms/"

DOCX_MIME = "application/vnd.openxmlformats-officedocument.wordprocessingml.document"


def _zip(parts: dict[str, bytes]) -> bytes:
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, blob in parts.items():
            archive.writestr(name, blob)
    return buffer.getvalue()


def _core_xml(revision: str = "7", last_by: str = "Av. Deneme") -> bytes:
    return (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<cp:coreProperties xmlns:cp="{CP_NS}" xmlns:dc="{DC_NS}" xmlns:dcterms="{DCTERMS_NS}">'
        f"<dc:creator>Av. Deneme</dc:creator><cp:lastModifiedBy>{last_by}</cp:lastModifiedBy>"
        f"<cp:revision>{revision}</cp:revision>"
        '<dcterms:created>2026-09-01T09:00:00Z</dcterms:created>'
        '<dcterms:modified>2026-10-01T10:00:00Z</dcterms:modified>'
        "</cp:coreProperties>"
    ).encode("utf-8")


def _settings_xml(*, track: bool, protection: str | None = None) -> bytes:
    inner = "<w:trackRevisions/>" if track else ""
    if protection:
        inner += f'<w:documentProtection w:edit="{protection}" w:enforcement="1"/>'
    return (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<w:settings xmlns:w="{W_NS}">{inner}</w:settings>'
    ).encode("utf-8")


def _docx(
    *,
    body_extra: str = "",
    footnotes: bool = True,
    comments: bool = True,
    settings: bytes | None = None,
    core: bytes | None = None,
    extra_parts: dict[str, bytes] | None = None,
    relationships: list[str] | None = None,
    marker: str = "ZZ-ENVANTER-MARKER-9C1",
) -> bytes:
    docx = pytest.importorskip("docx")
    document = docx.Document()
    document.add_paragraph(marker)
    document.add_paragraph("Kira sozlesmesi 2026.")
    table = document.add_table(rows=2, cols=2)
    table.cell(0, 0).text = "Kalem"
    table.cell(0, 1).text = "Tutar"
    table.cell(1, 0).text = "Kira"
    table.cell(1, 1).text = "1000"
    buffer = io.BytesIO()
    document.save(buffer)
    raw = buffer.getvalue()

    parts: dict[str, bytes] = {}
    rels: list[str] = list(relationships or [])
    overrides: list[str] = []
    if footnotes:
        parts["word/footnotes.xml"] = (
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            f'<w:footnotes xmlns:w="{W_NS}">'
            '<w:footnote w:id="-1" w:type="separator"><w:p><w:r><w:separator/></w:r></w:p></w:footnote>'
            '<w:footnote w:id="0" w:type="continuationSeparator">'
            "<w:p><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>"
            '<w:footnote w:id="1"><w:p><w:r><w:t>Damga vergisi kiraciya aittir.</w:t></w:r></w:p></w:footnote>'
            "</w:footnotes>"
        ).encode("utf-8")
        rels.append(f'<Relationship Id="rIdF1" Type="{REL_NS}/footnotes" Target="footnotes.xml"/>')
        overrides.append(
            '<Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-'
            'officedocument.wordprocessingml.footnotes+xml"/>'
        )
    if comments:
        parts["word/comments.xml"] = (
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            f'<w:comments xmlns:w="{W_NS}">'
            '<w:comment w:id="1" w:author="Av. Deneme" w:initials="AD">'
            "<w:p><w:r><w:t>Bu hukum riskli.</w:t></w:r></w:p></w:comment>"
            "</w:comments>"
        ).encode("utf-8")
        rels.append(f'<Relationship Id="rIdC1" Type="{REL_NS}/comments" Target="comments.xml"/>')
        overrides.append(
            '<Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-'
            'officedocument.wordprocessingml.comments+xml"/>'
        )
    parts.update(extra_parts or {})

    def patch_document(blob: bytes) -> bytes:
        text = blob.decode("utf-8")
        marker_at = text.find("<w:sectPr")
        if marker_at == -1:
            marker_at = text.find("</w:body>")
        return (text[:marker_at] + body_extra + text[marker_at:]).encode("utf-8")

    replacements = dict(parts)
    if settings is not None:
        replacements["word/settings.xml"] = settings
    if core is not None:
        replacements["docProps/core.xml"] = core

    with zipfile.ZipFile(io.BytesIO(raw)) as source:
        out = io.BytesIO()
        with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as target:
            names = set()
            for item in source.infolist():
                names.add(item.filename)
                if item.filename in replacements:
                    target.writestr(item.filename, replacements.pop(item.filename))
                    continue
                blob = source.read(item.filename)
                if item.filename == "word/document.xml":
                    blob = patch_document(blob)
                elif item.filename == "[Content_Types].xml" and overrides:
                    blob = blob.decode("utf-8").replace("</Types>", "".join(overrides) + "</Types>").encode(
                        "utf-8"
                    )
                elif item.filename == "word/_rels/document.xml.rels" and rels:
                    blob = blob.decode("utf-8").replace(
                        "</Relationships>", "".join(rels) + "</Relationships>"
                    ).encode("utf-8")
                target.writestr(item.filename, blob)
            for name, blob in replacements.items():
                if name not in names:
                    target.writestr(name, blob)
    return out.getvalue()


TRACKED = (
    '<w:p><w:ins w:id="101" w:author="Av. Deneme" w:date="2026-10-01T09:00:00Z">'
    '<w:r><w:t>Yillik %12 faiz.</w:t></w:r></w:ins></w:p>'
    '<w:p><w:del w:id="102" w:author="Karsi Taraf" w:date="2026-10-02T09:00:00Z">'
    '<w:r><w:delText>Yillik %30 faiz.</w:delText></w:r></w:del></w:p>'
)


def test_docx_structure_counts_come_from_the_package():
    data = _docx(body_extra=TRACKED, settings=_settings_xml(track=True), core=_core_xml())

    result = docx_inventory(data)

    # İki yön: belge gerçekten bu parçaları taşıyor.
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        assert "word/document.xml" in archive.namelist()
        body = archive.read("word/document.xml").decode("utf-8")
    assert body.count("<w:tbl>") == 1
    assert "<w:ins " in body and "<w:del " in body

    assert result.kind == "docx"
    assert result.structure["paragraphs"] >= 4
    assert result.structure["tables"] == 1
    assert result.structure["rows"] == 2
    assert result.structure["cells"] == 4
    assert result.structure["footnotes"] == 1  # -1/0 ayırıcı sayılmaz
    assert result.structure["comments"] == 1
    assert result.structure["styles"] >= 1
    assert result.structure["sections"] >= 1

    changes = result.changes
    assert changes["insertions"] == 1
    assert changes["deletions"] == 1
    assert changes["comments"] == 1
    assert changes["authors"] == ["Av. Deneme", "Karsi Taraf"]
    assert changes["firstDate"] == "2026-10-01T09:00:00Z"
    assert changes["lastDate"] == "2026-10-02T09:00:00Z"
    assert changes["trackRevisions"] is True
    assert changes["revision"] == "7"
    assert changes["lastModifiedBy"] == "Av. Deneme"
    assert changes["created"] == "2026-09-01T09:00:00Z"


def test_docx_attachments_are_listed_without_their_content():
    ole_bytes = b"\xd0\xcf\x11\xe0" + b"ZZ-EK-ICERIK-GIZLI" * 50
    image_bytes = b"\x89PNG\r\n\x1a\n" + b"ZZ-GORSEL" * 20
    alt_bytes = b"<html><body>ZZ-ALTCHUNK</body></html>"
    data = _docx(
        body_extra='<w:p><w:r><w:t>Ekler asagida.</w:t></w:r></w:p>',
        extra_parts={
            "word/embeddings/oleObject1.bin": ole_bytes,
            "word/media/image1.png": image_bytes,
            "word/afchunk1.mht": alt_bytes,
            "word/embeddings/ek.xlsx": b"PK\x03\x04ZZ-EK-XLSX",
            "_xmlsignatures/sig1.xml": b"<Signature>ZZ-IMZA</Signature>",
        },
        relationships=[
            f'<Relationship Id="rIdA1" Type="{REL_NS}/aFChunk" Target="afchunk1.mht"/>',
            f'<Relationship Id="rIdE1" Type="{REL_NS}/hyperlink" Target="https://ornek.test/karar" '
            'TargetMode="External"/>',
        ],
        core=_core_xml(),
    )

    result = docx_inventory(data)

    by_name = {item.name: item for item in result.attachments}
    assert by_name["word/embeddings/oleObject1.bin"].kind == "ole_object"
    assert by_name["word/embeddings/oleObject1.bin"].size == len(ole_bytes)
    assert by_name["word/embeddings/ek.xlsx"].kind == "embedded_package"
    assert by_name["word/afchunk1.mht"].kind == "alt_chunk"
    assert by_name["word/media/image1.png"].kind == "image"
    assert result.attachment_count == 4
    assert result.attachment_bytes == len(ole_bytes) + len(image_bytes) + len(alt_bytes) + len(
        b"PK\x03\x04ZZ-EK-XLSX"
    )
    assert result.external_links == 1
    assert result.changes["signatures"] == 1
    assert result.structure["mediaParts"] == 1

    # İçerik dışarı verilmez: eklerin baytları yanıtta yok.
    payload = json.dumps(result.to_dict(), ensure_ascii=False)
    for marker in ("ZZ-EK-ICERIK-GIZLI", "ZZ-GORSEL", "ZZ-ALTCHUNK", "ZZ-IMZA", "ornek.test"):
        assert marker not in payload

    assert any("gömülü nesne" in note for note in result.notes)
    assert any("altChunk" in note for note in result.notes)
    assert any("imza" in note for note in result.notes)
    assert any("dış bağlantı" in note for note in result.notes)


def test_docx_hidden_structure_is_counted_not_silently_dropped():
    text_box = (
        "<w:p><w:r><w:t>Govde.</w:t></w:r></w:p>"
        "<w:p><w:r><w:pict><w:txbxContent><w:p><w:r><w:t>Kutu metni.</w:t></w:r></w:p>"
        "</w:txbxContent></w:pict></w:r></w:p>"
        "<w:sdt><w:sdtPr/><w:sdtContent><w:p><w:r><w:t>Kontrollu icerik.</w:t></w:r></w:p>"
        "</w:sdtContent></w:sdt>"
    )
    result = docx_inventory(_docx(body_extra=text_box))

    assert result.structure["textBoxes"] == 1
    assert result.structure["contentControls"] == 1
    assert result.structure["pictures"] >= 1
    assert result.structure["drawings"] == 0
    assert any("metin kutusu" in note for note in result.notes)
    # Metin kutusunun metni sayıma girmez, dışarı da verilmez.
    assert "Kutu metni" not in json.dumps(result.to_dict(), ensure_ascii=False)


def test_docx_protection_and_absent_core_properties_are_reported_honestly():
    protected = docx_inventory(_docx(settings=_settings_xml(track=False, protection="readOnly")))
    assert protected.changes["trackRevisions"] is False
    assert protected.changes["protection"] == "readOnly"
    assert any("belge koruması" in note for note in protected.notes)

    # python-docx şablonu kendi core.xml/settings.xml'ini taşır; "yok" durumu
    # ancak parçalar paketten çıkarılarak kurulur. Alanlar uydurulmaz.
    plain = _docx(settings=None, core=None, footnotes=False, comments=False)
    with zipfile.ZipFile(io.BytesIO(plain)) as source:
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as target:
            for item in source.infolist():
                if item.filename in {"docProps/core.xml", "word/settings.xml"}:
                    continue
                target.writestr(item.filename, source.read(item.filename))
    result = docx_inventory(buffer.getvalue())
    assert result.changes["revision"] is None
    assert result.changes["lastModifiedBy"] is None
    assert result.changes["created"] is None
    assert result.changes["trackRevisions"] is False
    assert result.structure["footnotes"] == 0
    assert result.structure["comments"] == 0


def test_docx_entry_list_is_capped_and_reported():
    extra = {f"word/media/image{index}.png": b"x" * 8 for index in range(1, 8)}
    data = _docx(extra_parts=extra)
    monkey = inventory.MAX_LISTED_ATTACHMENTS
    inventory.MAX_LISTED_ATTACHMENTS = 3
    try:
        result = docx_inventory(data)
    finally:
        inventory.MAX_LISTED_ATTACHMENTS = monkey

    assert len(result.attachments) == 3
    assert result.attachment_count == 7
    assert result.truncated is True
    assert any("sınırlandı" in note for note in result.notes)


def test_docx_zip_escape_is_rejected():
    evil = _zip(
        {
            "word/document.xml": f'<w:document xmlns:w="{W_NS}"><w:body/></w:document>'.encode(),
            "../evil.xml": b"x",
        }
    )
    with pytest.raises(InventoryError) as excinfo:
        docx_inventory(evil)
    assert excinfo.value.code == "E_INVENTORY_ZIP_UNSAFE"


def test_docx_encrypted_entry_is_rejected():
    """Şifreli giriş işareti bayt düzeyinde kurulur: `writestr` bu bayrağı
    sıfırlar, bu yüzden baytlar sonradan işaretlenir (gerçek arşiv davranışı)."""
    plain = _zip({"word/document.xml": f'<w:document xmlns:w="{W_NS}"><w:body/></w:document>'.encode()})
    raw = bytearray(plain)
    local = raw.find(b"PK\x03\x04")
    central = raw.find(b"PK\x01\x02")
    assert local != -1 and central != -1
    raw[local + 6] |= 0x1
    raw[central + 8] |= 0x1
    with pytest.raises(InventoryError) as excinfo:
        docx_inventory(bytes(raw))
    assert excinfo.value.code == "E_INVENTORY_ZIP_UNSAFE"
    assert "şifreli" in excinfo.value.message


def test_docx_dtd_in_part_is_rejected():
    data = _docx(
        extra_parts={
            "word/comments.xml": (
                b'<?xml version="1.0"?><!DOCTYPE lolz [<!ENTITY lol "lol">]>'
                + f'<w:comments xmlns:w="{W_NS}"><w:comment w:id="1"><w:p><w:r><w:t>&lol;</w:t>'
                .encode()
                + b"</w:r></w:p></w:comment></w:comments>"
            )
        },
        comments=False,
    )
    with pytest.raises(Exception) as excinfo:
        docx_inventory(data)
    assert getattr(excinfo.value, "code", "") == "E_DOCX_XML"


def test_docx_without_body_part_is_rejected():
    data = _zip({"word/styles.xml": b"<w:styles/>"})
    with pytest.raises(InventoryError) as excinfo:
        docx_inventory(data)
    assert excinfo.value.code == "E_DOCX_XML"


def test_docx_broken_archive_is_rejected():
    with pytest.raises(InventoryError) as excinfo:
        docx_inventory(b"PK\x03\x04 bu zip degil")
    assert excinfo.value.code == "E_DOCX_OPEN"


def _udf_parts(*, embed: bool = True) -> dict[str, bytes]:
    pool = "Birinci paragraf. Ikinci paragraf."
    content = (
        '<?xml version="1.0" encoding="UTF-8"?>'
        '<template><content>'
        + pool
        + "</content>"
        "<elements>"
        '<paragraph startOffset="0" length="17"/>'
        '<paragraph startOffset="18" length="17"/>'
        '<table><row><cell><paragraph startOffset="0" length="17"/></cell></row></table>'
        '<image startOffset="0" length="0"/>'
        "</elements>"
        "<header><paragraph startOffset=\"0\" length=\"8\"/></header>"
        '<bilinmeyenOgе startOffset="0" length="0"/>'
        "</template>"
    ).encode("utf-8")
    parts = {"content.xml": content}
    if embed:
        parts["ekler/resim1.jpg"] = b"\xff\xd8\xff" + b"ZZ-UDF-GORSEL" * 10
        parts["signature.p7s"] = b"ZZ-UDF-IMZA" * 5
        parts["ekler/karar.pdf"] = b"%PDF-1.4 ZZ-UDF-PDF"
    return parts


def test_udf_inventory_counts_elements_and_lists_entries():
    data = _zip(_udf_parts())
    result = udf_inventory(data)

    assert result.kind == "udf"
    assert result.structure["pages"] == 0
    assert result.structure["paragraphs"] == 4  # 2 gövde + 1 tablo hücresi + 1 üstbilgi
    assert result.structure["tables"] == 1
    assert result.structure["rows"] == 1
    assert result.structure["cells"] == 1
    assert result.structure["images"] == 1
    assert result.structure["headers"] == 1
    assert result.structure["unknown"] == 1
    assert result.changes["signatures"] == 1
    assert result.attachment_count == 3
    kinds = {item.name: item.kind for item in result.attachments}
    assert kinds["ekler/resim1.jpg"] == "image"
    assert kinds["ekler/karar.pdf"] == "embedded_package"
    assert kinds["signature.p7s"] == "signature"

    payload = json.dumps(result.to_dict(), ensure_ascii=False)
    for marker in ("ZZ-UDF-GORSEL", "ZZ-UDF-IMZA", "ZZ-UDF-PDF", "Birinci paragraf"):
        assert marker not in payload
    assert any("imza" in note for note in result.notes)
    assert any("sayfa ayrımı" in note for note in result.notes)


def test_udf_missing_content_pool_is_an_error():
    with pytest.raises(InventoryError) as excinfo:
        udf_inventory(_zip({"ekler/resim1.jpg": b"x"}))
    assert excinfo.value.code == "E_UDF_STRUCTURE"


def test_unsupported_mime_is_rejected():
    with pytest.raises(InventoryError) as excinfo:
        build_inventory(b"%PDF-1.4", "application/pdf")
    assert excinfo.value.code == "E_INVENTORY_UNSUPPORTED"


def test_dispatch_picks_the_engine_from_mime():
    assert build_inventory(_docx(), DOCX_MIME).kind == "docx"
    assert build_inventory(_zip(_udf_parts()), "application/udf").kind == "udf"
