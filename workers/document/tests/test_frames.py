"""Parçalı worker yanıtı: 1 MiB çerçeve sınırı aşılmadan sayfa kaybı olmadan.

Küçük yanıt tek çerçeve kalır (geriye dönük uyum). Büyük yanıt sayfalara
bölünür; her çerçeve sınırın altındadır, sayfa sırası korunur ve zarf
(motor/uyarılar) her parçada tekrarlanır.
"""

from __future__ import annotations

import pytest

from dim_worker.engines import Extraction, Page, TextBlock
from dim_worker.protocol import MAX_FRAME_BYTES, ProtocolError
from dim_worker.worker import (
    RESPONSE_FRAME_BUDGET,
    _encoded_size,
    _extract_frames,
)


def _extraction(page_count: int, chars_per_page: int) -> Extraction:
    pages = []
    for index in range(page_count):
        text = ("Sayfa " + str(index) + " ") * max(1, chars_per_page // 10)
        pages.append(
            Page(
                index=index,
                text=text,
                width=595.0,
                height=842.0,
                coordinate_space="unrotated_cropbox_points",
                blocks=[TextBlock(start=0, end=len(text), bbox=[10.0, 10.0, 200.0, 24.0])],
            )
        )
    return Extraction(engine="pymupdf", engine_version="2.0", pages=pages, warnings=["genel uyarı"])


def test_small_response_stays_in_one_frame():
    frames = _extract_frames("r1", _extraction(page_count=2, chars_per_page=100))
    assert len(frames) == 1
    result = frames[0]["result"]
    assert result["engine"] == "pymupdf"
    assert "part" not in result and "parts" not in result
    assert [page["index"] for page in result["pages"]] == [0, 1]


def test_page_label_is_carried_only_when_defined():
    """Etiket alanı yalnız gerçekten tanımlıysa taşınır; boş sayfa etiket
    uydurmaz (arayüz "fiziksel sayfa" demek zorunda kalır)."""
    labeled = _extraction(page_count=1, chars_per_page=100)
    labeled.pages[0].label = "iv"
    plain = _extraction(page_count=1, chars_per_page=100)

    labeled_page = _extract_frames("r3", labeled)[0]["result"]["pages"][0]
    plain_page = _extract_frames("r4", plain)[0]["result"]["pages"][0]
    assert labeled_page["pageLabel"] == "iv"
    assert "pageLabel" not in plain_page


def test_large_response_is_split_without_losing_pages():
    extraction = _extraction(page_count=400, chars_per_page=4000)
    frames = _extract_frames("r2", extraction)
    assert len(frames) > 1, "büyük yanıt tek çerçevede kalmamalı"

    parts = [frame["result"]["parts"] for frame in frames]
    assert len(set(parts)) == 1
    assert parts[0] == len(frames)
    assert [frame["result"]["part"] for frame in frames] == list(range(1, len(frames) + 1))

    for frame in frames:
        assert frame["id"] == "r2"
        assert _encoded_size(frame) <= MAX_FRAME_BYTES
        assert _encoded_size(frame["result"]) <= RESPONSE_FRAME_BUDGET
        # Zarf her parçada aynı; istemci uyarıları tekilleştirir.
        assert frame["result"]["engine"] == "pymupdf"
        assert frame["result"]["engineVersion"] == "2.0"
        assert "genel uyarı" in frame["result"]["warnings"]
        assert frame["result"]["warnings"] == frames[0]["result"]["warnings"]

    # Bu boyutta geometri bütçesi aşılır: kutular düşer ama durum sessiz kalmaz.
    assert any("geometri" in warning for warning in frames[0]["result"]["warnings"])

    indexes = [page["index"] for frame in frames for page in frame["result"]["pages"]]
    assert indexes == list(range(400)), "sayfa sırası korunmalı, kayıp olmamalı"
    # Metin parçalama sonrası birebir korunur.
    texts = [page["text"] for frame in frames for page in frame["result"]["pages"]]
    assert texts == [page.text for page in extraction.pages]


def test_single_oversized_page_is_rejected_not_truncated():
    extraction = _extraction(page_count=1, chars_per_page=1_200_000)
    with pytest.raises(ProtocolError) as exc:
        _extract_frames("r3", extraction)
    assert exc.value.code == "E_FRAME_TOO_LARGE"
