"""Gerçek karartma testleri: içerik metin katmanından çıkmalı, doğrulama geçmeli."""

from __future__ import annotations

import pytest

from dim_worker import engines

from dim_worker.engines import EngineError, redact_pdf


def _pdf_with_text(lines: list[str]) -> bytes:
    pymupdf = pytest.importorskip("pymupdf")
    doc = pymupdf.open()
    page = doc.new_page()
    y = 72
    for line in lines:
        page.insert_text((72, y), line)
        y += 24
    data = doc.tobytes()
    doc.close()
    return data


def test_redaction_removes_text_from_text_layer():
    pytest.importorskip("pymupdf")
    data = _pdf_with_text(["Ahmet Yilmaz", "Kira sozlesmesi", "TCKN 00000000000"])

    result = redact_pdf(data, ["Ahmet Yilmaz", "00000000000"])
    assert len(result.hits) >= 2
    assert result.remaining == []

    pymupdf = pytest.importorskip("pymupdf")
    out = pymupdf.open(stream=result.pdf, filetype="pdf")
    text = "\n".join(p.get_text("text") for p in out)
    out.close()
    assert "Ahmet Yilmaz" not in text
    assert "00000000000" not in text
    assert "Kira sozlesmesi" in text


def test_empty_terms_rejected():
    pytest.importorskip("pymupdf")
    with pytest.raises(EngineError) as exc:
        redact_pdf(b"%PDF-1.4", [])
    assert exc.value.code == "E_REDACT_TERMS"


def test_redaction_reports_missing_term_as_no_hit():
    pytest.importorskip("pymupdf")
    data = _pdf_with_text(["Kira sozlesmesi"])
    result = redact_pdf(data, ["Bu metin yok"])
    assert result.hits == []
    assert result.remaining == []


def test_independent_reader_confirms_removal():
    """Çıktı, üreten kütüphaneden başka bir okuyucuyla da temiz görünmeli."""
    pypdf = pytest.importorskip("pypdf")
    pymupdf = pytest.importorskip("pymupdf")
    import io as _io

    doc = pymupdf.open()
    page = doc.new_page()
    page.insert_text((72, 72), "Ahmet Yilmaz 00000000000")
    source = doc.tobytes()
    doc.close()

    result = redact_pdf(source, ["Ahmet Yilmaz"])
    assert "pypdf" in result.engines
    reader = pypdf.PdfReader(_io.BytesIO(result.pdf))
    text = "".join((p.extract_text() or "") for p in reader.pages)
    assert "Ahmet Yilmaz" not in text


def test_ocr_verification_flag_is_honest():
    """Tesseract yoksa 'görüntü doğrulandı' denmez."""
    pymupdf = pytest.importorskip("pymupdf")
    doc = pymupdf.open()
    page = doc.new_page()
    page.insert_text((72, 72), "gizli metin")
    source = doc.tobytes()
    doc.close()

    result = redact_pdf(source, ["gizli"])
    import shutil

    if shutil.which("tesseract") is None:
        assert result.ocr_verified is False
        assert "tesseract" not in result.engines
    else:
        assert result.ocr_verified is True
        assert "tesseract" in result.engines


def test_region_redaction_removes_selected_area():
    pymupdf = pytest.importorskip("pymupdf")
    doc = pymupdf.open()
    page = doc.new_page()
    page.insert_text((72, 72), "Gizli Isim 12345678901")
    data = doc.tobytes()
    doc.close()

    result = engines.redact_pdf_regions(data, [{"page": 0, "rect": [60, 55, 240, 95]}])
    assert result.hits
    assert result.remaining == []
    check = pymupdf.open(stream=result.pdf, filetype="pdf")
    try:
        assert "Gizli Isim" not in check[0].get_text("text")
    finally:
        check.close()


def test_region_redaction_rejects_bad_input():
    pymupdf = pytest.importorskip("pymupdf")
    doc = pymupdf.open()
    doc.new_page()
    data = doc.tobytes()
    doc.close()

    with pytest.raises(EngineError) as tiny:
        engines.redact_pdf_regions(data, [{"page": 0, "rect": [10, 10, 10.5, 10.5]}])
    assert tiny.value.code == "E_REDACT_REGIONS"

    with pytest.raises(EngineError) as outside:
        engines.redact_pdf_regions(data, [{"page": 3, "rect": [10, 10, 100, 100]}])
    assert outside.value.code == "E_REDACT_REGIONS"

    with pytest.raises(EngineError) as empty:
        engines.redact_pdf_regions(data, [])
    assert empty.value.code == "E_REDACT_REGIONS"
