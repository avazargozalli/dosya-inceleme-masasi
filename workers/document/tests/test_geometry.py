"""F06-T08: segment/blok koordinatları, ortak koordinat uzayı ve döndürme.

Kutuların **metinle aynı çıkarımdan** geldiğini, döndürme ve kırpma kutusunun
koordinatları kaydırmadığını ve çerçeve bütçesi dolduğunda geometrinin
sessizce değil uyarıyla düştüğünü doğrular.
"""

from __future__ import annotations

import io
import shutil

import pytest

from dim_worker.engines import (
    COORDINATE_SPACE_IMAGE,
    COORDINATE_SPACE_PDF,
    COORDINATE_SPACE_STRUCTURE,
    extract,
    extract_docx,
    extract_pdf,
)
from dim_worker.worker import GEOMETRY_FRAME_BUDGET, _encoded_size, _page_payload, _shrink_geometry


def _pdf(pages: list[dict]) -> bytes:
    pymupdf = pytest.importorskip("pymupdf")
    doc = pymupdf.open()
    for spec in pages:
        page = doc.new_page(width=spec.get("width", 300), height=spec.get("height", 400))
        for x, y, text in spec.get("texts", []):
            page.insert_text((x, y), text, fontsize=spec.get("fontsize", 14))
        if spec.get("rotation"):
            page.set_rotation(spec["rotation"])
        if spec.get("cropbox"):
            page.set_cropbox(pymupdf.Rect(*spec["cropbox"]))
    data = doc.tobytes()
    doc.close()
    return data


def _blocks_fit(blocks, page_text: str) -> None:
    previous_end = 0
    for block in blocks:
        assert 0 <= block.start < block.end <= len(page_text)
        assert block.start >= previous_end, "blok aralıkları çakışıyor"
        previous_end = block.end
        assert page_text[block.start : block.end].strip(), "blok aralığı boş metne düşüyor"


def test_pdf_blocks_cover_text_and_boxes_stay_inside_page():
    data = _pdf([{"texts": [(40, 80, "KIRA SOZLESMESI"), (40, 140, "Ikinci paragraf")]}])
    page = extract_pdf(data).pages[0]

    assert page.coordinate_space == COORDINATE_SPACE_PDF
    assert page.rotation == 0
    assert page.blocks, "PDF metin bloğu üretmedi"
    _blocks_fit(page.blocks, page.text)
    for block in page.blocks:
        assert block.bbox is not None
        x0, y0, x1, y1 = block.bbox
        assert 0 <= x0 < x1 <= page.width
        assert 0 <= y0 < y1 <= page.height
        for line in block.lines:
            assert x0 - 0.5 <= line[0] and line[2] <= x1 + 0.5
            assert y0 - 0.5 <= line[1] and line[3] <= y1 + 0.5


def test_rotated_page_keeps_unrotated_coordinates_and_reports_rotation():
    data = _pdf([{"texts": [(50, 100, "DONDURULMUS SAYFA")], "rotation": 90}])
    page = extract_pdf(data).pages[0]

    # Genişlik/yükseklik döndürme öncesi kırpma kutusudur; görünen boyut tersidir.
    assert page.rotation == 90
    assert (page.width, page.height) == (300.0, 400.0)
    assert page.blocks
    x0, y0, x1, y1 = page.blocks[0].bbox
    # Metin (50, 100) noktasına kondu; döndürme koordinatı değiştirmemeli.
    assert 40 <= x0 <= 60
    assert 70 <= y0 <= 100
    assert x1 <= page.width and y1 <= page.height


def test_cropbox_origin_is_the_coordinate_origin():
    data = _pdf(
        [
            {
                "texts": [(50, 100, "KIRPILMIS")],
                "cropbox": (20, 30, 280, 380),
            }
        ]
    )
    page = extract_pdf(data).pages[0]

    assert (page.width, page.height) == (260.0, 350.0)
    x0, y0, x1, y1 = page.blocks[0].bbox
    # (50, 100) - (20, 30) = (30, 70); punto toleransı bırakılır.
    assert 25 <= x0 <= 35
    assert 45 <= y0 <= 70
    assert x1 <= page.width and y1 <= page.height


def test_docx_blocks_are_structure_ranges_without_boxes():
    docx = pytest.importorskip("docx")
    document = docx.Document()
    document.add_paragraph("Birinci paragraf")
    document.add_paragraph("Ikinci paragraf")
    buffer = io.BytesIO()
    document.save(buffer)

    page = extract_docx(buffer.getvalue()).pages[0]
    assert page.coordinate_space == COORDINATE_SPACE_STRUCTURE
    assert len(page.blocks) >= 2
    _blocks_fit(page.blocks, page.text)
    assert all(block.bbox is None for block in page.blocks)
    assert page.text[page.blocks[0].start : page.blocks[0].end] == "Birinci paragraf"


def test_image_page_uses_pixel_space():
    PIL = pytest.importorskip("PIL")
    from PIL import Image

    image = Image.new("RGB", (120, 80), "white")
    buffer = io.BytesIO()
    image.save(buffer, format="PNG")

    page = extract(buffer.getvalue(), "image/png").pages[0]
    assert page.coordinate_space == COORDINATE_SPACE_IMAGE
    assert (page.width, page.height) == (120.0, 80.0)
    assert page.rotation == 0
    assert page.blocks == []


@pytest.mark.skipif(shutil.which("tesseract") is None, reason="tesseract kurulu değil")
def test_ocr_boxes_are_derotated_into_page_space():
    pytest.importorskip("pytesseract")
    pytest.importorskip("PIL")
    PIL = pytest.importorskip("PIL")
    from PIL import Image, ImageDraw

    font = None
    for path in (
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    ):
        try:
            font = PIL.ImageFont.truetype(path, 60)
            break
        except OSError:
            continue
    if font is None:
        pytest.skip("uygun yazı tipi bulunamadı")

    image = Image.new("RGB", (900, 300), "white")
    ImageDraw.Draw(image).text((40, 90), "KIRA ODEMESI", fill="black", font=font)
    buffer = io.BytesIO()
    image.save(buffer, format="PNG")

    pymupdf = pytest.importorskip("pymupdf")
    doc = pymupdf.open()
    page = doc.new_page(width=595, height=842)
    page.insert_image(pymupdf.Rect(40, 60, 555, 260), stream=buffer.getvalue())
    page.set_rotation(90)
    data = doc.tobytes()
    doc.close()

    result = extract_pdf(data, ocr=True, ocr_lang="eng")
    page_result = result.pages[0]
    assert page_result.coordinate_space == COORDINATE_SPACE_PDF
    assert page_result.rotation == 90
    assert page_result.text.strip()
    assert page_result.blocks, "OCR kutuları üretilmedi"
    _blocks_fit(page_result.blocks, page_result.text)
    for block in page_result.blocks:
        x0, y0, x1, y1 = block.bbox
        # Döndürme geri alınmazsa kutular sayfa dışına (400..842) taşardı.
        assert 0 <= x0 < x1 <= page_result.width + 1
        assert 0 <= y0 < y1 <= page_result.height + 1


def _fake_result(pages: int, blocks_per_page: int) -> dict:
    block = {"start": 0, "end": 10, "bbox": [0.0, 0.0, 10.0, 10.0], "lines": [[0.0, 0.0, 10.0, 5.0]]}
    return {
        "engine": "pymupdf",
        "engineVersion": "test",
        "pages": [
            {"index": index, "text": "x" * 10, "width": 300.0, "height": 400.0, "blocks": [dict(block) for _ in range(blocks_per_page)]}
            for index in range(pages)
        ],
        "warnings": [],
    }


def test_page_payload_carries_geometry_and_space():
    data = _pdf([{"texts": [(40, 80, "GEOMETRI")]}])
    page = extract_pdf(data).pages[0]
    payload = _page_payload(page)

    assert payload["coordinateSpace"] == COORDINATE_SPACE_PDF
    assert payload["rotation"] == 0
    assert payload["blocks"][0]["start"] == 0
    assert len(payload["blocks"][0]["bbox"]) == 4
    assert payload["blocks"][0]["lines"]


def test_frame_budget_drops_lines_then_boxes_and_reports_it():
    result = _fake_result(pages=40, blocks_per_page=260)
    assert _encoded_size(result) > GEOMETRY_FRAME_BUDGET

    _shrink_geometry(result)
    assert result["warnings"], "geometri kırpılması sessiz kalmamalı"
    assert "lines" not in result["pages"][0]["blocks"][0] or result["pages"][0]["blocks"] == []
    assert any("geometri" in warning for warning in result["warnings"])
    assert _encoded_size(result) <= GEOMETRY_FRAME_BUDGET
