"""Rapor üretimi: DOCX ve PDF; kullanıcı verisi HTML olarak çalıştırılmaz."""

from __future__ import annotations

import io

import pytest

from dim_worker.engines import render_report, render_report_pdf

PAYLOAD = {
    "title": "Kira dosyası — rapor",
    "meta": {"Dosya": "Kira", "Esas": "2026/0000"},
    "sections": [
        {"heading": "Kronoloji", "rows": [["Tarih", "Başlık"], ["2026-01-01", "Ödeme yapılmadı"]]},
        {"heading": "Notlar", "lines": ["<script>alert(1)</script>"]},
    ],
}


def test_docx_report_contains_sections():
    pytest.importorskip("docx")
    data = render_report(PAYLOAD)
    document = pytest.importorskip("docx").Document(io.BytesIO(data))
    text = "\n".join(p.text for p in document.paragraphs)
    assert "Kira dosyası" in text
    assert "Kronoloji" in text
    assert any(table.rows[1].cells[1].text == "Ödeme yapılmadı" for table in document.tables)


def test_pdf_report_contains_text_and_escapes_html():
    pytest.importorskip("pymupdf")
    data = render_report_pdf(PAYLOAD)
    assert data.startswith(b"%PDF-")
    pymupdf = pytest.importorskip("pymupdf")
    doc = pymupdf.open(stream=data, filetype="pdf")
    text = "\n".join(page.get_text("text") for page in doc)
    doc.close()
    assert "Kira dosyası" in text
    assert "Ödeme yapılmadı" in text
    # HTML kaçışlandığı için etiket metin olarak görünür, çalışmaz.
    assert "<script>" in text
