"""F04-T07 — canlı aşama/sayım: ilerleme çerçeveleri.

Ölçülen sözleşme: worker çok sayfalı çıkarımda sonuç çerçevesinden **önce**
`progress` çerçeveleri yazar; bu çerçeveler yanıt değildir (`ok` yok), istek
kimliğini taşır ve yalnız aşama + sayaç içerir.
"""

import base64
import io
import json

import pytest

from dim_worker.protocol import MAX_FRAME_BYTES, progress_frame
from dim_worker.worker import handle, serve


def _pdf_b64(pages: int = 3) -> str:
    pymupdf = pytest.importorskip("pymupdf")
    doc = pymupdf.open()
    for index in range(pages):
        doc.new_page().insert_text((72, 72), f"Sayfa {index + 1}")
    data = doc.tobytes()
    doc.close()
    return base64.b64encode(data).decode()


def _pdf_bytes(pages: int = 3) -> bytes:
    return base64.b64decode(_pdf_b64(pages))


def _request(**kw) -> dict:
    base = {
        "v": 1,
        "id": "req-1",
        "method": "extract_text",
        "params": {"contentBase64": _pdf_b64(), "mime": "application/pdf"},
    }
    if "params" in kw:
        base["params"] = {**base["params"], **kw.pop("params")}
    base.update(kw)
    return base


def _single(response):
    """`handle` parçalı yanıtta liste dönebilir; testler tek çerçeve bekler."""
    return response[0] if isinstance(response, list) else response


def test_progress_frame_is_not_a_response():
    frame = progress_frame("req-1", "pages", 3, 9)
    assert frame == {
        "v": 1,
        "id": "req-1",
        "progress": {"stage": "pages", "done": 3, "total": 9},
    }
    # Yanıt alanları **yok**: istemci bunu sonuç sanmamalı.
    assert "ok" not in frame
    assert "result" not in frame
    # Toplam bilinmiyorsa alan yazılmaz (sıfır uydurulmaz).
    open_frame = progress_frame("req-2", "start", 0)
    assert open_frame["progress"] == {"stage": "start", "done": 0}
    assert len(json.dumps(frame).encode()) < MAX_FRAME_BYTES


def test_pdf_reports_open_then_each_page():
    from dim_worker.engines import extract_pdf

    seen: list[tuple[str, int, int | None]] = []
    extraction = extract_pdf(
        _pdf_bytes(3), progress=lambda stage, done, total: seen.append((stage, done, total))
    )

    assert len(extraction.pages) == 3
    assert seen[0] == ("open", 0, 3)
    assert seen[1:] == [("pages", 1, 3), ("pages", 2, 3), ("pages", 3, 3)]


def test_extraction_is_identical_with_and_without_progress():
    """İlerleme bilgilendiricidir: sonucu değiştirmez."""
    from dim_worker.engines import extract_pdf

    data = _pdf_bytes(2)
    silent = extract_pdf(data)
    seen: list[tuple[str, int, int | None]] = []
    noisy = extract_pdf(data, progress=lambda *item: seen.append(item))
    assert [page.text for page in noisy.pages] == [page.text for page in silent.pages]
    assert noisy.warnings == silent.warnings
    assert seen  # gerçekten çağrıldı


def test_handle_forwards_progress_for_extract_text():
    seen: list[tuple[str, int, int | None]] = []
    response = _single(
        handle(
            _request(), progress=lambda stage, done, total: seen.append((stage, done, total))
        )[0]
    )
    assert response["ok"] is True
    assert len(response["result"]["pages"]) == 3
    assert seen[0] == ("open", 0, 3)
    assert seen[-1] == ("pages", 3, 3)


def test_progress_frames_precede_the_response_on_the_wire():
    stdin = io.BytesIO(
        json.dumps(_request()).encode()
        + b"\n"
        + json.dumps({"v": 1, "id": "req-2", "method": "shutdown"}).encode()
        + b"\n"
    )
    stdout = io.BytesIO()
    assert serve(stdin, stdout) == 0

    lines = [json.loads(line) for line in stdout.getvalue().splitlines()]
    progress = [line for line in lines if "progress" in line]
    responses = [index for index, line in enumerate(lines) if line.get("ok") is True]

    assert progress, "ilerleme çerçevesi yazılmalı"
    assert all(line["id"] == "req-1" for line in progress)
    assert [line["progress"]["stage"] for line in progress] == [
        "open",
        "pages",
        "pages",
        "pages",
    ]
    assert [line["progress"]["done"] for line in progress] == [0, 1, 2, 3]
    first_response = min(responses)
    assert all(lines.index(line) < first_response for line in progress)


def test_progress_does_not_leak_text_or_paths():
    """Çerçeve yalnız aşama/sayaç taşır; belge metni ve dosya adı yok."""
    stdin = io.BytesIO(
        json.dumps(_request()).encode()
        + b"\n"
        + json.dumps({"v": 1, "id": "req-2", "method": "shutdown"}).encode()
        + b"\n"
    )
    stdout = io.BytesIO()
    serve(stdin, stdout)
    for line in stdout.getvalue().splitlines():
        frame = json.loads(line)
        if "progress" not in frame:
            continue
        assert set(frame) == {"v", "id", "progress"}
        assert set(frame["progress"]) <= {"stage", "done", "total"}
        assert "Sayfa" not in line.decode()


def test_single_page_engines_emit_no_progress_frames():
    """Düz metin/DOCX tek sayfa: ilerleme çerçevesi yok (sahte sayaç üretilmez)."""
    seen: list[tuple[str, int, int | None]] = []
    params = {"contentBase64": base64.b64encode("kira sözleşmesi".encode()).decode(), "mime": "text/plain"}
    response = _single(
        handle(_request(params=params), progress=lambda *item: seen.append(item))[0]
    )
    assert response["ok"] is True
    assert seen == []


def test_redaction_reports_page_counts_and_phases():
    """Maskeleme artık gerçek sayfa sayacı verir: redact → pack → verify → scan.

    Ölçülen sözleşme: sayaç uydurulmaz — `redact` sayfa sayısı kadar artar,
    `verify` ikinci okuma turunda aynı sayıya ulaşır, `scan` tek birimdir.
    """
    from dim_worker.engines import redact_pdf

    data = _pdf_bytes(3)
    seen: list[tuple[str, int, int | None]] = []
    result = redact_pdf(data, ["Sayfa 2"], progress=lambda *item: seen.append(item))

    assert result.remaining == []
    redact_frames = [(done, total) for stage, done, total in seen if stage == "redact"]
    assert redact_frames[0] == (0, 3)
    assert redact_frames[1:] == [(1, 3), (2, 3), (3, 3)]
    verify_frames = [(done, total) for stage, done, total in seen if stage == "verify"]
    assert verify_frames == [(0, 3), (1, 3), (2, 3), (3, 3)]
    assert ("pack", 0, 1) in seen and ("pack", 1, 1) in seen
    assert ("scan", 0, 1) in seen and ("scan", 1, 1) in seen
    # Aşama sırası gerçek iş sırasıdır: doğrulama paketlemeden sonra gelir.
    stages = [stage for stage, _, _ in seen]
    assert stages.index("pack") < stages.index("verify") < stages.index("scan")


def test_redaction_result_is_identical_with_and_without_progress():
    """İlerleme bilgilendiricidir: maskeli çıktının **metni** değişmez.

    Bayt eşitliği beklenmez: PyMuPDF çıktıya her koşuda farklı belge kimliği
    yazar (ölçüldü). Anlamlı değişmez, maskeli belgenin metni ve bulgularıdır.
    """
    import pymupdf

    from dim_worker.engines import redact_pdf

    data = _pdf_bytes(2)
    silent = redact_pdf(data, ["Sayfa 1"])
    seen: list[tuple[str, int, int | None]] = []
    noisy = redact_pdf(data, ["Sayfa 1"], progress=lambda *item: seen.append(item))

    def texts(pdf: bytes) -> list[str]:
        doc = pymupdf.open(stream=pdf, filetype="pdf")
        try:
            return [page.get_text("text") for page in doc]
        finally:
            doc.close()

    assert texts(noisy.pdf) == texts(silent.pdf)
    assert noisy.hits == silent.hits
    assert noisy.remaining == silent.remaining == []
    assert seen


def test_region_redaction_reports_progress_per_page():
    from dim_worker.engines import redact_pdf_regions

    data = _pdf_bytes(2)
    seen: list[tuple[str, int, int | None]] = []
    redact_pdf_regions(
        data,
        [{"page": 0, "rect": [60, 60, 220, 100]}, {"page": 1, "rect": [60, 60, 220, 100]}],
        progress=lambda *item: seen.append(item),
    )
    redact_frames = [(done, total) for stage, done, total in seen if stage == "redact"]
    assert redact_frames == [(0, 2), (1, 2), (2, 2)]
    assert ("scan", 1, 1) in seen


def test_handle_forwards_progress_for_redact():
    seen: list[tuple[str, int, int | None]] = []
    params = {"contentBase64": _pdf_b64(2), "terms": ["Sayfa 1"]}
    response = _single(
        handle(
            _request(method="redact", params=params),
            progress=lambda stage, done, total: seen.append((stage, done, total)),
        )[0]
    )
    assert response["ok"] is True
    assert response["result"]["remaining"] == []
    assert seen[0] == ("redact", 0, 2)
    assert seen[-1] == ("scan", 1, 1)
