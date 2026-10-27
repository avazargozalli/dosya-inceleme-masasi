"""F01-T05 — karartmanın bağımsız okuyucuyla doğrulanması.

Üç eksen ölçülür ve her test **iki yönlü** kanıt üretir:

1. **Metin katmanı** — terim gerçekten vardı (karartma öncesi okunur), sonra yok.
2. **Görüntü** — raster metin terim aramasıyla bulunamaz; ya OCR ile
   reddedilir ya da bölge karartması pikselleri temizler ve OCR doğrular.
3. **Gizli nesne** — annotation `/Contents`/`/T`, sayfa dışı metin, gömülü
   JavaScript, ek dosya, metadata/XMP. Bunlar `get_text`'te görünmez; ham bayt
   taraması olmadan "temiz" sanılır.

Bağımsız kontrol hiçbir PDF kütüphanesine dayanmaz: ham baytlar + Flate
akışlarının kendi açıcımızla açılması + pypdf. Dedektörün kör olmadığı,
temizlenmemiş bir dosyada terimi **bulmasıyla** kanıtlanır.
"""

from __future__ import annotations

import shutil

import pytest

from dim_worker import engines
from dim_worker.engines import EngineError, redact_pdf, redact_pdf_regions

MARKER = "ZZKARARTMA7412"
OTHER = "ZZBASKAMETIN5531"


def _doc():
    return pytest.importorskip("pymupdf")


def _carries(data: bytes, term: str = MARKER) -> bool:
    """Dosya terimi taşıyor mu? (ham bayt + onaltılık + açılmış akışlar)"""
    found, _streams, _binary, _unreadable = engines._scan_terms(data, [term])
    return found == [term]


def _pdf(build) -> bytes:
    pymupdf = _doc()
    doc = build(pymupdf)
    data = doc.tobytes()
    doc.close()
    return data


def _text_pdf(text: str = MARKER, extra: str | None = None) -> bytes:
    def build(pymupdf):
        doc = pymupdf.open()
        page = doc.new_page()
        page.insert_text((72, 100), text, fontsize=12)
        if extra:
            page.insert_text((72, 300), extra, fontsize=12)
        return doc

    return _pdf(build)


def _png_words(data: bytes) -> str:
    """Hiçbir PDF kütüphanesi kullanmadan: baytlarda ve açılmış akışlarda ara."""
    found, _streams, _binary, _unreadable = engines._scan_terms(data, [MARKER])
    return ",".join(found)


# --- 1. Metin katmanı -------------------------------------------------------


def test_text_layer_term_is_gone_from_bytes_and_readers():
    source = _text_pdf()
    assert _carries(source)  # kaynak gerçekten taşıyor

    result = redact_pdf(source, [MARKER])
    assert result.hits, "terim metin katmanında bulunmalıydı"
    assert result.remaining == []
    assert result.annotations_removed == 0
    assert "raw-scan" in result.engines
    assert _png_words(result.pdf) == ""

    pypdf = pytest.importorskip("pypdf")
    import io

    reader = pypdf.PdfReader(io.BytesIO(result.pdf))
    text = "".join((page.extract_text() or "") for page in reader.pages)
    assert MARKER not in text


def test_scanner_detects_known_leak_not_blind():
    """Negatif kontrol: dedektör, temizlenmemiş dosyada terimi buluyor."""
    source = _pdf(lambda p: _annot_doc(p, MARKER))
    assert _png_words(source) == MARKER
    result = redact_pdf(source, [MARKER])
    assert _png_words(result.pdf) == ""


def test_compressed_stream_is_inflated_before_searching():
    """Terim ham baytlarda yok, sıkıştırılmış akışta var: açıcı şart."""
    pymupdf = _doc()
    doc = pymupdf.open()
    page = doc.new_page()
    page.insert_text((72, 100), MARKER, fontsize=12)
    source = doc.tobytes(deflate=True)
    doc.close()
    assert MARKER.encode() not in source, "test kurgusu: terim ham baytlarda olmamalı"
    assert _carries(source), "sıkıştırılmış akışta olmalı"

    found, streams, _binary, _unreadable = engines._scan_terms(source, [MARKER])
    assert found == [MARKER]
    assert streams >= 1


def test_black_box_over_text_is_not_redaction():
    """Klasik yanlış yöntem: metnin üstüne siyah dikdörtgen çizmek.

    Metin dosyada kalır, kopyalanabilir. Tarayıcı bunu görmeli; ürünün gerçek
    karartması aynı belgede metni silmeli.
    """
    pymupdf = _doc()
    doc = pymupdf.open()
    page = doc.new_page()
    page.insert_text((72, 100), MARKER, fontsize=12)
    page.draw_rect(pymupdf.Rect(60, 85, 400, 115), color=(0, 0, 0), fill=(0, 0, 0))
    naive = doc.tobytes()
    doc.close()

    assert _carries(naive), "kara kutu metni gizlemez; tarayıcı görmeliydi"

    result = redact_pdf(naive, [MARKER])
    assert result.remaining == []
    assert not _carries(result.pdf)


def test_short_terms_are_not_byte_scanned():
    pymupdf = _doc()
    doc = pymupdf.open()
    page = doc.new_page()
    page.insert_text((72, 100), "ab cd", fontsize=12)
    source = doc.tobytes()
    doc.close()

    result = redact_pdf(source, ["ab"])
    assert result.remaining == []
    assert any("kısa" in note for note in result.warnings)


# --- 2. Görüntü -------------------------------------------------------------


def _raster_pdf(pymupdf, marker: str = MARKER, rect=(60, 60, 540, 200)):
    doc = pymupdf.open()
    page = doc.new_page()
    tmp = pymupdf.open()
    tp = tmp.new_page(width=600, height=120)
    tp.insert_text((20, 70), marker, fontsize=30)
    pixmap = tp.get_pixmap(dpi=150)
    tmp.close()
    page.insert_image(pymupdf.Rect(*rect), pixmap=pixmap)
    return doc


def test_rasterized_term_is_refused_not_silently_ignored():
    """Terim araması raster metni bulamaz: sonuç 'temiz' diye sunulmamalı."""
    if shutil.which("tesseract") is None:
        pytest.skip("tesseract yok; görüntü doğrulaması bu makinede yapılamaz")
    source = _pdf(lambda p: _raster_pdf(p))
    assert not _carries(source), "rastgele bayt eşleşmesi olmamalı; metin pikselde"

    result = redact_pdf(source, [MARKER])
    assert result.hits == []
    assert result.remaining == [MARKER], "görüntüde kalan terim raporlanmalıydı"
    assert "tesseract" in result.engines


def test_region_redaction_clears_pixels_and_ocr_confirms():
    if shutil.which("tesseract") is None:
        pytest.skip("tesseract yok")
    pymupdf = _doc()
    source = _pdf(lambda p: _raster_pdf(p))

    result = redact_pdf_regions(source, [{"page": 0, "rect": [55, 55, 545, 205]}])
    assert result.hits
    assert result.remaining == []
    assert result.ocr_verified is True
    assert "tesseract" in result.engines
    assert any("metin katmanı yoktu" in note for note in result.warnings)
    assert _png_words(result.pdf) == ""


def test_region_redaction_keeps_words_that_appear_elsewhere():
    """Bölge dışında da geçen kelime yanlış 'sızıntı' sayılmamalı."""
    pymupdf = _doc()
    doc = pymupdf.open()
    page = doc.new_page()
    page.insert_text((72, 100), f"Ahmet Yilmaz {MARKER}", fontsize=12)
    page.insert_text((72, 300), "Ahmet burada tekrar geciyor", fontsize=12)
    source = doc.tobytes()
    doc.close()

    result = redact_pdf_regions(source, [{"page": 0, "rect": [60, 85, 400, 115]}])
    assert result.remaining == []

    check = pymupdf.open(stream=result.pdf, filetype="pdf")
    try:
        text = check[0].get_text("text")
    finally:
        check.close()
    assert "Ahmet burada tekrar geciyor" in text
    assert MARKER not in text


# --- 3. Gizli nesneler ------------------------------------------------------


def _annot_doc(pymupdf, marker: str):
    doc = pymupdf.open()
    page = doc.new_page()
    page.insert_text((72, 100), "sozlesme")
    note = page.add_text_annot((300, 300), f"gizli not {marker}")
    note.set_info(title=marker)
    page.add_text_annot((400, 300), "zararsiz not")
    return doc


def test_annotation_text_is_removed_and_harmless_note_kept():
    source = _pdf(lambda p: _annot_doc(p, MARKER))
    assert _carries(source)

    result = redact_pdf(source, [MARKER])
    assert result.annotations_removed == 1
    assert result.remaining == []
    assert _png_words(result.pdf) == ""
    assert any("açıklama silindi" in note for note in result.warnings)

    pymupdf = _doc()
    check = pymupdf.open(stream=result.pdf, filetype="pdf")
    try:
        contents = [a.info.get("content", "") for a in (check[0].annots() or [])]
    finally:
        check.close()
    assert contents == ["zararsiz not"]


def test_annotation_without_marker_does_not_block_redaction():
    """Eskiden herhangi bir açıklama karartmayı çökertiyordu (PyMuPDF 1.24.14)."""
    source = _pdf(lambda p: _annot_doc(p, MARKER))
    result = redact_pdf(source, ["sozlesme"])
    assert result.remaining == []
    assert result.annotations_removed == 0


def test_offpage_text_is_removed_and_visible_text_survives():
    def build(pymupdf):
        doc = pymupdf.open()
        page = doc.new_page()
        page.insert_text((900, 100), MARKER, fontsize=12)  # sayfa dışı
        page.insert_text((72, 100), OTHER, fontsize=12)  # görünür
        return doc

    source = _pdf(build)
    assert _carries(source)

    result = redact_pdf(source, [MARKER])
    assert result.offpage_spans_removed == 1
    assert result.remaining == []
    assert _png_words(result.pdf) == ""

    pymupdf = _doc()
    check = pymupdf.open(stream=result.pdf, filetype="pdf")
    try:
        text = check[0].get_text("text")
    finally:
        check.close()
    assert OTHER in text, "görünen metin silinmemeliydi"


def test_rotated_page_visible_text_is_not_mistaken_for_offpage():
    def build(pymupdf):
        doc = pymupdf.open()
        page = doc.new_page()
        page.insert_text((72, 100), OTHER, fontsize=12)
        page.set_rotation(90)
        return doc

    source = _pdf(build)
    result = redact_pdf(source, [MARKER])
    assert result.offpage_spans_removed == 0

    pymupdf = _doc()
    check = pymupdf.open(stream=result.pdf, filetype="pdf")
    try:
        assert OTHER in check[0].get_text("text")
    finally:
        check.close()


def test_inline_javascript_is_removed():
    def build(pymupdf):
        doc = pymupdf.open()
        page = doc.new_page()
        page.insert_text((72, 100), "metin")
        doc.xref_set_key(
            doc.pdf_catalog(), "OpenAction", f"<</S/JavaScript/JS({MARKER})>>"
        )
        return doc

    source = _pdf(build)
    assert _carries(source)

    result = redact_pdf(source, [MARKER])
    assert result.scripts_removed == 1
    assert result.remaining == []
    assert _png_words(result.pdf) == ""
    assert any("JavaScript" in note for note in result.warnings)


def test_attachment_metadata_and_xmp_are_scrubbed():
    def build(pymupdf):
        doc = pymupdf.open()
        page = doc.new_page()
        page.insert_text((72, 100), "metin")
        doc.set_metadata({"title": MARKER, "author": MARKER, "keywords": MARKER})
        doc.set_xml_metadata(
            f"<x:xmpmeta xmlns:x='adobe:ns:meta/'><t>{MARKER}</t></x:xmpmeta>"
        )
        doc.embfile_add("ek.txt", MARKER.encode())
        return doc

    source = _pdf(build)
    result = redact_pdf(source, [MARKER])
    assert result.remaining == []
    assert _png_words(result.pdf) == ""


# --- Doğrulayıcı dürüstlüğü -------------------------------------------------


def test_missing_pypdf_is_not_claimed_as_verifier(monkeypatch):
    monkeypatch.setattr(engines, "_pypdf_module", lambda: None)
    source = _text_pdf()

    result = redact_pdf(source, [MARKER])
    assert "pypdf" not in result.engines
    assert any("pypdf kurulu değil" in note for note in result.warnings)


def test_unreadable_stream_is_reported(monkeypatch):
    """Filtreli bir akış açılamazsa 'temiz' denmez; sayı ve uyarı yazılır."""

    class FakeZlib:
        error = __import__("zlib").error

        @staticmethod
        def decompress(_body):
            raise __import__("zlib").error("bozuk")

        @staticmethod
        def decompressobj():
            raise __import__("zlib").error("bozuk")

    source = _text_pdf()
    monkeypatch.setattr(engines, "zlib", FakeZlib)

    result = redact_pdf(source, [MARKER])
    assert result.streams_unreadable >= 1
    assert any("akış açılamadı" in note for note in result.warnings)


def test_terms_required():
    with pytest.raises(EngineError) as exc:
        redact_pdf(_text_pdf(), ["   "])
    assert exc.value.code == "E_REDACT_TERMS"


def test_reports_are_not_in_engine_list_when_not_run(monkeypatch):
    monkeypatch.setattr(engines.shutil, "which", lambda _name: None)
    source = _text_pdf()
    result = redact_pdf(source, [MARKER])
    assert "tesseract" not in result.engines
    assert result.ocr_verified is False


def test_pypdf_crash_is_reported_as_unverified_not_as_leak(monkeypatch):
    """Doğrulayıcı çökerse dosya 'sızıntı' diye reddedilmez (ölçülen kusur).

    Eskiden yardımcı çökme yolunda `list(terms)` dönüyordu: `remaining` doluyor,
    ürün `E_REDACT_VERIFY` veriyor ve **maskeli dosya hiç kaydedilmiyordu**.
    Doğru davranış: çökme "doğrulanamadı"dır (uyarı), sızıntı değildir.
    """

    class BoomPypdf:
        class PdfReader:  # noqa: D401 - taklit
            def __init__(self, *_args, **_kwargs):
                raise RuntimeError("okuyucu çöktü")

    monkeypatch.setattr(engines, "_pypdf_module", lambda: BoomPypdf)
    source = _text_pdf()

    result = redact_pdf(source, [MARKER])
    assert result.remaining == [], "çökme sızıntı sayılmamalı"
    assert "pypdf" not in result.engines
    assert any("pypdf doğrulaması yapılamadı" in note for note in result.warnings)


def test_ocr_crash_is_reported_as_unverified_not_as_leak(monkeypatch):
    """Aynı sınıf OCR tarafında: tesseract hatası maskeli dosyayı düşürmez."""
    pytesseract = pytest.importorskip("pytesseract")
    monkeypatch.setattr(engines.shutil, "which", lambda _name: "/usr/bin/tesseract")

    def boom(*_args, **_kwargs):
        raise RuntimeError("tesseract çöktü")

    monkeypatch.setattr(pytesseract, "image_to_string", boom)
    source = _text_pdf()

    result = redact_pdf(source, [MARKER])
    assert result.remaining == [], "OCR çökmesi sızıntı sayılmamalı"
    assert result.ocr_verified is False
    assert "tesseract" not in result.engines
    assert any("OCR doğrulaması yapılamadı" in note for note in result.warnings)


def test_region_ocr_crash_is_reported_as_unverified(monkeypatch):
    """Bölge maskesinde de çökme 'doğrulanamadı'dır; 'bölgede metin kaldı' değil."""
    pytesseract = pytest.importorskip("pytesseract")
    monkeypatch.setattr(engines.shutil, "which", lambda _name: "/usr/bin/tesseract")

    def boom(*_args, **_kwargs):
        raise RuntimeError("tesseract çöktü")

    monkeypatch.setattr(pytesseract, "image_to_string", boom)
    source = _text_pdf()

    result = redact_pdf_regions(source, [{"page": 0, "rect": [0, 0, 400, 200]}])
    assert not any("bölgede metin kaldı" in note for note in result.warnings)
    assert "tesseract" not in result.engines
    assert any("OCR doğrulaması yapılamadı" in note for note in result.warnings)
