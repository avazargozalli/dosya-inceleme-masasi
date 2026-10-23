"""Görüntü/TIFF ayrıştırma testleri: kare sayısı, piksel bütçesi ve dürüst OCR."""

from __future__ import annotations

import io

import pytest

from dim_worker.engines import (
    MAX_IMAGE_PIXELS,
    EngineError,
    _check_pixel_budget,
    extract,
    extract_image,
)

Image = pytest.importorskip("PIL.Image")


def _png(color: tuple[int, int, int] = (255, 255, 255), size: tuple[int, int] = (8, 6)) -> bytes:
    buffer = io.BytesIO()
    Image.new("RGB", size, color).save(buffer, format="PNG")
    return buffer.getvalue()


def _tiff(frames: int) -> bytes:
    images = [Image.new("RGB", (10, 7), (255, 255, 255)) for _ in range(frames)]
    buffer = io.BytesIO()
    images[0].save(buffer, format="TIFF", save_all=True, append_images=images[1:])
    return buffer.getvalue()


def test_image_without_ocr_does_not_invent_text():
    result = extract_image(_png())
    assert result.engine == "image"
    assert len(result.pages) == 1
    assert result.pages[0].text == ""
    assert any("OCR gerekli" in warning for warning in result.warnings)
    assert result.pages[0].width == 8.0
    assert result.pages[0].height == 6.0


def test_multi_frame_tiff_becomes_one_page_per_frame():
    result = extract_image(_tiff(3))
    assert len(result.pages) == 3
    assert [page.index for page in result.pages] == [0, 1, 2]
    assert all("metin katmanı yok" in page.warnings[0] for page in result.pages)


def test_image_dispatch_by_mime_and_content():
    data = _png()
    assert extract(data, "image/png").engine == "image"
    # MIME genel olsa da sihirli bayt görüntüyü tanıtır.
    assert extract(data, "application/octet-stream").engine == "image"


def test_pixel_budget_rejects_oversized_frame():
    with pytest.raises(EngineError) as exc:
        _check_pixel_budget(MAX_IMAGE_PIXELS, 2)
    assert exc.value.code == "E_IMAGE_TOO_LARGE"
    with pytest.raises(EngineError) as exc:
        _check_pixel_budget(0, 10)
    assert exc.value.code == "E_IMAGE_INVALID"


def test_ocr_request_without_tesseract_is_error_not_silent_text():
    import shutil

    if shutil.which("tesseract") is not None:
        pytest.skip("tesseract kurulu; eksik bağımlılık yolu denenemez")
    with pytest.raises(EngineError) as exc:
        extract_image(_png(), ocr=True, ocr_lang="tur")
    assert exc.value.code == "E_OCR_MISSING"


def test_broken_image_is_engine_error():
    with pytest.raises(EngineError) as exc:
        extract_image(b"\x89PNG\r\n\x1a\nbozuk")
    assert exc.value.code == "E_IMAGE_OPEN"
