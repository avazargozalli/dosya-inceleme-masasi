"""Gerçek karartma testleri: içerik metin katmanından çıkmalı, doğrulama geçmeli."""

from __future__ import annotations

import pytest

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
