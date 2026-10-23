"""DOCX envanteri: dipnot/sonnot/yorum ve izlenen değişiklik (F06-T02).

python-docx gövde metnine bu içerikleri **katmaz**: izlenen ekleme/silme
metni `w:ins`/`w:del` içinde, not ve yorumlar ayrı XML parçalarındadır
(`word/footnotes.xml`, `word/endnotes.xml`, `word/comments.xml`). Worker
bunları ayrı blok olarak ekler; silinen metin güncel metin sayılmaz ve bu
durum uyarıda açıkça yazılır.

Testler gerçek OOXML üretir: gövde python-docx ile yazılır, arşive gerçek
parçalar ve ilişkiler eklenir. Böylece kod yolu taklit edilmez.
"""

from __future__ import annotations

import io
import zipfile

import pytest

from dim_worker import engines
from dim_worker.engines import EngineError, extract_docx

W_NS = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
REL_NS = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"

_CONTENT_TYPES = {
    "footnotes": "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml",
    "endnotes": "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml",
    "comments": "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml",
}

_BODY_XML = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    f'<w:document xmlns:w="{W_NS}"><w:body>'
    "<w:p><w:r><w:t>Govde metni.</w:t></w:r></w:p>"
    "</w:body></w:document>"
).encode("utf-8")


def _zip_bytes(parts: dict[str, bytes]) -> bytes:
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, blob in parts.items():
            archive.writestr(name, blob)
    return buffer.getvalue()


def _notes_xml(kind: str, notes: list[tuple[str, str]]) -> bytes:
    """`kind` = footnote|endnote. Word'ün yazdığı -1/0 ayırıcı kayıtları
    her zaman eklenir: bunlar içerik taşımaz ve sayıma girmemelidir."""
    tag = "footnote" if kind == "footnote" else "endnote"
    chunks = [
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>',
        f'<w:{tag}s xmlns:w="{W_NS}">',
        f'<w:{tag} w:id="-1" w:type="separator"><w:p><w:r><w:separator/></w:r></w:p></w:{tag}>',
        f'<w:{tag} w:id="0" w:type="continuationSeparator">'
        f"<w:p><w:r><w:continuationSeparator/></w:r></w:p></w:{tag}>",
    ]
    for note_id, text in notes:
        chunks.append(
            f'<w:{tag} w:id="{note_id}"><w:p><w:r>'
            f'<w:t xml:space="preserve">{text}</w:t></w:r></w:p></w:{tag}>'
        )
    chunks.append(f"</w:{tag}s>")
    return "".join(chunks).encode("utf-8")


def _comments_xml(comments: list[tuple[str, str, str]]) -> bytes:
    chunks = [
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>',
        f'<w:comments xmlns:w="{W_NS}">',
    ]
    for comment_id, author, text in comments:
        chunks.append(
            f'<w:comment w:id="{comment_id}" w:author="{author}" w:initials="AD">'
            f'<w:p><w:r><w:t xml:space="preserve">{text}</w:t></w:r></w:p></w:comment>'
        )
    chunks.append("</w:comments>")
    return "".join(chunks).encode("utf-8")


def _tracked_paragraph(inserted: str | None, deleted: str | None) -> str:
    inner = ""
    if inserted:
        inner += (
            '<w:ins w:id="101" w:author="Av. Deneme" w:date="2026-10-01T09:00:00Z">'
            f'<w:r><w:t xml:space="preserve">{inserted}</w:t></w:r></w:ins>'
        )
    if deleted:
        inner += (
            '<w:del w:id="102" w:author="Karsi Taraf" w:date="2026-10-02T09:00:00Z">'
            f'<w:r><w:delText xml:space="preserve">{deleted}</w:delText></w:r></w:del>'
        )
    return f"<w:p>{inner}</w:p>"


def _word_docx(
    *,
    inserted: str | None = None,
    deleted: str | None = None,
    footnotes: list[tuple[str, str]] | None = None,
    endnotes: list[tuple[str, str]] | None = None,
    comments: list[tuple[str, str, str]] | None = None,
) -> bytes:
    """python-docx gövdesi + gerçek ek parçalar ve ilişkiler."""
    docx = pytest.importorskip("docx")
    document = docx.Document()
    document.add_paragraph("Kira sozlesmesi 2026.")
    buffer = io.BytesIO()
    document.save(buffer)
    raw = buffer.getvalue()

    extra_parts: dict[str, bytes] = {}
    relationships: list[str] = []
    overrides: list[str] = []
    for name, blob in (
        ("footnotes", _notes_xml("footnote", footnotes) if footnotes is not None else None),
        ("endnotes", _notes_xml("endnote", endnotes) if endnotes is not None else None),
        ("comments", _comments_xml(comments) if comments is not None else None),
    ):
        if blob is None:
            continue
        extra_parts[f"word/{name}.xml"] = blob
        relationships.append(
            f'<Relationship Id="rId9{len(relationships)}" '
            f'Type="{REL_NS}/{name}" Target="{name}.xml"/>'
        )
        overrides.append(
            f'<Override PartName="/word/{name}.xml" ContentType="{_CONTENT_TYPES[name]}"/>'
        )

    body_extra = _tracked_paragraph(inserted, deleted)

    def patch_document(blob: bytes) -> bytes:
        text = blob.decode("utf-8")
        marker = text.find("<w:sectPr")
        if marker == -1:
            marker = text.find("</w:body>")
        return (text[:marker] + body_extra + text[marker:]).encode("utf-8")

    with zipfile.ZipFile(io.BytesIO(raw)) as source:
        out = io.BytesIO()
        with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as target:
            for item in source.infolist():
                blob = source.read(item.filename)
                if item.filename == "word/document.xml":
                    blob = patch_document(blob)
                elif item.filename == "[Content_Types].xml" and overrides:
                    blob = blob.decode("utf-8").replace(
                        "</Types>", "".join(overrides) + "</Types>"
                    ).encode("utf-8")
                elif item.filename == "word/_rels/document.xml.rels" and relationships:
                    blob = blob.decode("utf-8").replace(
                        "</Relationships>", "".join(relationships) + "</Relationships>"
                    ).encode("utf-8")
                target.writestr(item.filename, blob)
            for name, blob in extra_parts.items():
                target.writestr(name, blob)
    return out.getvalue()


def test_docx_tracked_changes_and_notes_are_in_text():
    data = _word_docx(
        inserted="Yillik %12 faiz isletilecektir.",
        deleted="Yillik %30 faiz isletilecektir.",
        footnotes=[("1", "Damga vergisi kiraciya aittir.")],
        endnotes=[("2", "Ek protokol 2026.")],
        comments=[("3", "Av. Deneme", "Bu hukum riskli.")],
    )

    # Özelliğin nedeni: python-docx gövdesi bu metni katmaz.
    docx = pytest.importorskip("docx")
    body = "\n".join(p.text for p in docx.Document(io.BytesIO(data)).paragraphs)
    assert "Yillik %12" not in body
    assert "Yillik %30" not in body

    result = extract_docx(data)
    page = result.pages[0]
    text = page.text
    assert "Kira sozlesmesi 2026." in text
    assert "[dipnot 1] Damga vergisi kiraciya aittir." in text
    assert "[sonnot 2] Ek protokol 2026." in text
    assert "[yorum 3 — Av. Deneme] Bu hukum riskli." in text
    assert "Yillik %12 faiz isletilecektir." in text
    assert "Yillik %30 faiz isletilecektir." in text
    # Gövde önce, envanter sonra: gövde metni değişmez.
    assert text.index("Kira sozlesmesi") < text.index("[dipnot 1]")

    inventory = [w for w in result.warnings if w.startswith("DOCX envanteri:")]
    assert len(inventory) == 1
    assert "1 dipnot, 1 sonnot, 1 yorum, 1 ekleme, 1 silme" in inventory[0]
    assert "Av. Deneme" in inventory[0] and "Karsi Taraf" in inventory[0]
    assert any("varsayılan metin" in w for w in result.warnings)
    assert any("silinen metin güncel belge metni sayılmaz" in w for w in result.warnings)
    # Sayfa düzeni uyarısı korunur (DOCX yapısal).
    assert any("sayfa düzeni" in w for w in result.warnings)

    # Envanter blokları ayrı segmenttir: arama/alıntı bunları görür.
    blocks = [text[block.start : block.end] for block in page.blocks]
    assert any(block.startswith("[dipnot 1]") for block in blocks)
    assert any(block.startswith("[değişiklik silme") for block in blocks)
    assert page.coordinate_space == engines.COORDINATE_SPACE_STRUCTURE


def test_docx_separator_notes_are_not_inventoried():
    data = _zip_bytes(
        {
            "word/document.xml": _BODY_XML,
            "word/footnotes.xml": _notes_xml("footnote", []),
            "word/endnotes.xml": _notes_xml("endnote", []),
        }
    )
    sections, counts, warnings = engines._docx_annotations(data)
    assert sections == []
    assert counts == {
        "footnotes": 0,
        "endnotes": 0,
        "comments": 0,
        "insertions": 0,
        "deletions": 0,
    }
    assert warnings == []


def test_docx_inventory_cap_is_reported():
    notes = [(str(index), f"not {index}") for index in range(1, 601)]
    data = _zip_bytes(
        {
            "word/document.xml": _BODY_XML,
            "word/footnotes.xml": _notes_xml("footnote", notes),
        }
    )
    sections, counts, warnings = engines._docx_annotations(data)
    assert len(sections) == engines.MAX_DOCX_ANNOTATIONS
    assert counts["footnotes"] == 600
    assert any("sınırlandı" in warning for warning in warnings)


def test_docx_dtd_is_rejected():
    hostile = (
        '<?xml version="1.0"?>'
        '<!DOCTYPE lolz [<!ENTITY lol "lol"><!ENTITY lol2 "&lol;&lol;">]>'
        f'<w:document xmlns:w="{W_NS}"><w:body><w:p><w:r>'
        "<w:t>&lol2;</w:t></w:r></w:p></w:body></w:document>"
    ).encode("utf-8")
    data = _zip_bytes({"word/document.xml": hostile})
    with pytest.raises(EngineError) as excinfo:
        engines._docx_annotations(data)
    assert excinfo.value.code == "E_DOCX_XML"
    assert "DTD" in excinfo.value.message


def test_docx_part_size_limit(monkeypatch):
    data = _zip_bytes(
        {
            "word/document.xml": _BODY_XML,
            "word/comments.xml": _comments_xml([("1", "Av. Deneme", "x" * 400)]),
        }
    )
    monkeypatch.setattr(engines, "MAX_DOCX_PART_BYTES", 64)
    with pytest.raises(EngineError) as excinfo:
        engines._docx_annotations(data)
    assert excinfo.value.code == "E_DOCX_PART_TOO_LARGE"


def test_docx_without_body_part_is_rejected():
    data = _zip_bytes({"word/footnotes.xml": _notes_xml("footnote", [("1", "x")])})
    with pytest.raises(EngineError) as excinfo:
        engines._docx_annotations(data)
    assert excinfo.value.code == "E_DOCX_XML"
    assert "document.xml" in excinfo.value.message


def test_docx_broken_archive_is_rejected():
    with pytest.raises(EngineError) as excinfo:
        engines._docx_annotations(b"PK\x03\x04 bu bir zip degil")
    assert excinfo.value.code == "E_DOCX_OPEN"
