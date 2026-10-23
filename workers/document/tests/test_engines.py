"""Motor testleri: PDF metni, DOCX, düz metin ve gerçek OCR.

Ağır bağımlılık yoksa test atlanır; CI'da `[all]` kurulur ve OCR dahil koşar.
"""

from __future__ import annotations

import io
import os
import shutil

import pytest

from dim_worker.engines import EngineError, capabilities, extract, extract_docx, extract_pdf


def _pdf_with_text(text: str) -> bytes:
    pymupdf = pytest.importorskip("pymupdf")
    doc = pymupdf.open()
    page = doc.new_page()
    page.insert_text((72, 72), text)
    data = doc.tobytes()
    doc.close()
    return data


def test_pdf_text_layer_is_extracted():
    pytest.importorskip("pymupdf")
    data = _pdf_with_text("Kira sozlesmesi 2026")
    result = extract_pdf(data)
    assert result.engine == "pymupdf"
    assert len(result.pages) == 1
    assert "Kira sozlesmesi 2026" in result.pages[0].text
    assert result.pages[0].warnings == []


def test_pdf_page_labels_are_read_from_page_labels_tree():
    """`/PageLabels` ağacı varsa basılı etiket okunur (iv, A-1…)."""
    pymupdf = pytest.importorskip("pymupdf")
    doc = pymupdf.open()
    for _ in range(3):
        doc.new_page()
    doc.set_page_labels([{"startpage": 0, "prefix": "", "style": "r", "firstpagenum": 1}])
    data = doc.tobytes()
    doc.close()

    result = extract_pdf(data)
    assert [page.label for page in result.pages] == ["i", "ii", "iii"]


def test_pdf_without_page_labels_has_no_label():
    """Ağaç yoksa etiket `None`: fiziksel numara etiket diye uydurulmaz."""
    data = _pdf_with_text("Etiketsiz belge")
    result = extract_pdf(data)
    assert result.pages[0].label is None


def test_pdf_without_text_layer_warns_when_ocr_off():
    pymupdf = pytest.importorskip("pymupdf")
    doc = pymupdf.open()
    doc.new_page()
    data = doc.tobytes()
    doc.close()

    result = extract_pdf(data, ocr=False)
    assert result.pages[0].text.strip() == ""
    assert any("OCR gerekli" in w for w in result.warnings)


def test_docx_heading_stack_gives_section_paths_not_fabricated():
    """Başlık yığını gerçek DOCX stillerinden gelir; paragraf yolu p/N ile
    fiziksel paragraf sırasını (türev sayfadan bağımsız) taşır."""
    docx = pytest.importorskip("docx")
    document = docx.Document()
    document.add_heading("Kira Sözleşmesi", level=1)
    document.add_paragraph("Birinci paragraf")
    document.add_heading("Ödeme", level=2)
    document.add_paragraph("İkinci paragraf")
    buffer = io.BytesIO()
    document.save(buffer)

    result = extract_docx(buffer.getvalue())
    blocks = result.pages[0].blocks
    assert [block.kind for block in blocks] == ["heading", "paragraph", "heading", "paragraph"]
    assert blocks[0].path == "Kira Sözleşmesi"
    assert blocks[1].path == "Kira Sözleşmesi · p/2"
    assert blocks[2].path == "Kira Sözleşmesi › Ödeme"
    assert blocks[3].path == "Kira Sözleşmesi › Ödeme · p/4"
    # Metin offsetleri blok sınırlarıyla tutarlı.
    for block in blocks:
        assert result.pages[0].text[block.start : block.end].strip()


def test_docx_without_heading_style_does_not_invent_section():
    docx = pytest.importorskip("docx")
    document = docx.Document()
    document.add_paragraph("Başlıksız paragraf")
    buffer = io.BytesIO()
    document.save(buffer)

    result = extract_docx(buffer.getvalue())
    block = result.pages[0].blocks[0]
    assert block.path == "p/1"
    assert block.kind == "paragraph"


def test_docx_paragraphs_and_tables():
    docx = pytest.importorskip("docx")
    document = docx.Document()
    document.add_paragraph("Kira bedeli odenmedi.")
    table = document.add_table(rows=1, cols=2)
    table.rows[0].cells[0].text = "Tutar"
    table.rows[0].cells[1].text = "12.000 TL"
    buffer = io.BytesIO()
    document.save(buffer)

    result = extract_docx(buffer.getvalue())
    assert result.engine == "python-docx"
    assert "Kira bedeli odenmedi." in result.pages[0].text
    assert "12.000 TL" in result.pages[0].text
    assert any("sayfa düzeni" in w for w in result.warnings)


def test_plain_text_dispatch():
    result = extract(b"duz metin", "text/plain")
    assert result.engine == "plain"
    assert result.pages[0].text == "duz metin"


def test_pdf_signature_routes_to_pdf_engine_even_with_generic_mime():
    pytest.importorskip("pymupdf")
    with pytest.raises(EngineError) as exc:
        extract(b"%PDF-1.4 bozuk", "application/octet-stream")
    assert exc.value.code == "E_PDF_OPEN"


def test_unknown_mime_rejected():
    with pytest.raises(EngineError) as exc:
        extract(b"abc", "application/zip")
    assert exc.value.code == "E_MIME_UNSUPPORTED"


def test_capabilities_reports_optional_engines():
    caps = capabilities()
    assert caps["protocolVersion"] == 1
    assert "pdf" in caps["engines"] and "ocr" in caps["engines"]
    assert isinstance(caps["engines"]["ocr"]["languages"], list)


def _find_font():
    from PIL import ImageFont

    for path in (
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    ):
        if os.path.exists(path):
            return ImageFont.truetype(path, 120)
    return None


def test_ocr_reads_image_only_pdf_when_tesseract_available():
    if shutil.which("tesseract") is None:
        pytest.skip("tesseract kurulu değil")
    pytest.importorskip("pytesseract")
    pytest.importorskip("PIL")
    pymupdf = pytest.importorskip("pymupdf")
    from PIL import Image, ImageDraw

    font = _find_font()
    if font is None:
        pytest.skip("uygun yazı tipi bulunamadı")

    image = Image.new("RGB", (1400, 400), "white")
    draw = ImageDraw.Draw(image)
    draw.text((60, 120), "KIRA ODEMESI", fill="black", font=font)
    buffer = io.BytesIO()
    image.save(buffer, format="PNG")

    doc = pymupdf.open()
    page = doc.new_page(width=595, height=842)
    page.insert_image(pymupdf.Rect(40, 60, 555, 220), stream=buffer.getvalue())
    pdf_bytes = doc.tobytes()
    doc.close()

    result = extract_pdf(pdf_bytes, ocr=True, ocr_lang="eng")
    text = result.pages[0].text.strip()
    assert text != "", "OCR boş sonuç döndü"
    assert any("OCR ile okundu" in w for w in result.pages[0].warnings)
    # Tanıma kalitesi motora bağlı; burada yalnız okunabilir metin üretildiği doğrulanır.
    assert len(text) >= 3


def test_password_protected_pdf_needs_correct_password():
    pymupdf = pytest.importorskip("pymupdf")
    doc = pymupdf.open()
    page = doc.new_page()
    page.insert_text((72, 72), "gizli icerik")
    protected = doc.tobytes(encryption=pymupdf.PDF_ENCRYPT_AES_256, owner_pw="sahip", user_pw="acil")
    doc.close()

    with pytest.raises(EngineError) as missing:
        extract(protected, "application/pdf")
    assert missing.value.code == "E_PDF_PASSWORD"

    with pytest.raises(EngineError) as wrong:
        extract(protected, "application/pdf", password="yanlis")
    assert wrong.value.code == "E_PDF_PASSWORD"

    result = extract(protected, "application/pdf", password="acil")
    text = "\n".join(page.text for page in result.pages)
    assert "gizli icerik" in text


def test_mixed_pdf_ocrs_only_pages_without_text_layer(monkeypatch):
    """Karma PDF'te metin katmanı olan sayfa yeniden OCR edilmez; yalnız
    görüntüden oluşan sayfa OCR'a gider. Çağrı sayısı ölçülür."""
    pymupdf = pytest.importorskip("pymupdf")
    pytest.importorskip("pytesseract")
    import shutil

    if shutil.which("tesseract") is None:
        pytest.skip("tesseract yok")

    doc = pymupdf.open()
    first = doc.new_page()
    first.insert_text((72, 72), "metin katmani burada")
    doc.new_page()  # metin katmanı yok: OCR'a gitmeli
    data = doc.tobytes()
    doc.close()

    import dim_worker.engines as engines

    calls: list[int] = []
    real = engines._ocr_page

    def counting(page, lang):
        calls.append(int(page.number))
        return real(page, lang)

    monkeypatch.setattr(engines, "_ocr_page", counting)
    result = engines.extract_pdf(data, ocr=True, ocr_lang="eng")

    assert calls == [1], f"yalnız metinsiz sayfa OCR edilmeliydi, çağrılar: {calls}"
    assert "metin katmani burada" in result.pages[0].text
    assert result.pages[0].coordinate_space == "unrotated_cropbox_points"


def test_capabilities_reports_tessdata_fingerprint_honestly():
    """Dil paketi parmak izi varsa 64 hex; yoksa None. Uydurma sürüm yok."""
    caps = capabilities()
    digest = caps["ocr"]["tessdataHash"]
    if digest is not None:
        assert len(digest) == 64
        int(digest, 16)
    assert isinstance(caps["ocr"]["langs"], list)
