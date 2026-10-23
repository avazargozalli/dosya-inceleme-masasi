"""DOCX dipnot/sonnot/yorum ve izlenen değişiklik envanteri.

python-docx bu katmanları gövde metnine katmaz; envanterin görünür olması
gerekir, aksi hâlde "belgede yok" sanılır.
"""

from __future__ import annotations

import io
import zipfile

import pytest

from dim_worker import engines

W_NS = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"

FOOTNOTES = f"""<?xml version="1.0" encoding="UTF-8"?>
<w:footnotes xmlns:w="{W_NS}">
  <w:footnote w:id="-1" w:type="separator"><w:p><w:r><w:separator/></w:r></w:p></w:footnote>
  <w:footnote w:id="1"><w:p><w:r><w:t>Dipnot metni</w:t></w:r></w:p></w:footnote>
</w:footnotes>
"""

COMMENTS = f"""<?xml version="1.0" encoding="UTF-8"?>
<w:comments xmlns:w="{W_NS}">
  <w:comment w:id="0" w:author="Avukat"><w:p><w:r><w:t>Yorum metni</w:t></w:r></w:p></w:comment>
</w:comments>
"""


def _docx_with_annotations() -> bytes:
    docx = pytest.importorskip("docx")
    document = docx.Document()
    document.add_paragraph("Gövde metni")
    buffer = io.BytesIO()
    document.save(buffer)
    buffer.seek(0)

    source = zipfile.ZipFile(buffer)
    out = io.BytesIO()
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as target:
        for item in source.infolist():
            data = source.read(item.filename)
            if item.filename == "word/document.xml":
                inserted = (
                    f'<w:p><w:ins w:id="1" w:author="Avukat"><w:r><w:t>Eklenen madde</w:t>'
                    f"</w:r></w:ins></w:p>"
                ).encode()
                data = data.replace(b"</w:body>", inserted + b"</w:body>")
            target.writestr(item, data)
        target.writestr("word/footnotes.xml", FOOTNOTES)
        target.writestr("word/comments.xml", COMMENTS)
    source.close()
    return out.getvalue()


def test_inventory_counts_footnotes_comments_and_tracked_changes():
    pytest.importorskip("docx")
    data = _docx_with_annotations()

    blocks, counts, warnings = engines._docx_annotations(data)
    assert counts["footnotes"] == 1, counts
    assert counts["comments"] == 1, counts
    assert counts["insertions"] == 1, counts
    joined = "\n".join(blocks)
    assert "Dipnot metni" in joined
    assert "Yorum metni" in joined
    assert "Eklenen madde" in joined
    assert warnings, "envanter uyarısı üretilmeliydi"


def test_annotations_appear_in_extracted_text():
    pytest.importorskip("docx")
    data = _docx_with_annotations()
    extraction = engines.extract_docx(data)
    text = "\n".join(page.text for page in extraction.pages)
    assert "Gövde metni" in text
    assert "Dipnot metni" in text
    assert "Yorum metni" in text
    assert "Eklenen madde" in text
