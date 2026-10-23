import io
import json

import pytest

from dim_worker.protocol import MAX_FRAME_BYTES, ProtocolError, read_frame, write_frame
from dim_worker.worker import handle, serve


def frame(**kw):
    base = {"v": 1, "id": "req-1", "method": "ping", "params": {}}
    base.update(kw)
    return base


def test_ping_roundtrip():
    response, close = handle(frame())
    assert response["ok"] is True
    assert response["result"]["pong"] is True
    assert close is False


def test_unknown_method_rejected():
    with pytest.raises(ProtocolError) as exc:
        handle(frame(method="rm_rf"))
    assert exc.value.code == "E_METHOD"


def test_wrong_protocol_version_rejected():
    with pytest.raises(ProtocolError) as exc:
        handle(frame(v=2))
    assert exc.value.code == "E_VERSION"


def test_missing_content_returns_input_error():
    with pytest.raises(Exception) as exc:
        handle(frame(method="extract_text"))
    assert getattr(exc.value, "code", "") == "E_INPUT"


def test_invalid_base64_rejected():
    with pytest.raises(Exception) as exc:
        handle(frame(method="extract_text", params={"contentBase64": "%%%"}))
    assert getattr(exc.value, "code", "") == "E_INPUT_BASE64"


def test_content_without_mime_is_rejected():
    import base64

    with pytest.raises(ProtocolError) as exc:
        handle(
            frame(
                method="extract_text",
                params={"contentBase64": base64.b64encode(b"abc").decode()},
            )
        )
    assert exc.value.code == "E_PARAMS"


def test_oversize_frame_rejected():
    stream = io.BytesIO(b"x" * (MAX_FRAME_BYTES + 10) + b"\n")
    with pytest.raises(ProtocolError) as exc:
        read_frame(stream)
    assert exc.value.code == "E_FRAME_TOO_LARGE"


def test_broken_json_rejected():
    with pytest.raises(ProtocolError) as exc:
        read_frame(io.BytesIO(b"{not json}\n"))
    assert exc.value.code == "E_FRAME_JSON"


def test_serve_survives_bad_frame_then_answers():
    stdin = io.BytesIO(b"{bad}\n" + json.dumps(frame(id="a")).encode() + b"\n")
    stdout = io.BytesIO()
    code = serve(stdin, stdout)
    assert code == 2  # senkron kaybolduğu için kapanır
    assert b"E_FRAME_JSON" in stdout.getvalue()


def test_serve_shutdown_closes_cleanly():
    stdin = io.BytesIO(json.dumps(frame(method="shutdown")).encode() + b"\n")
    stdout = io.BytesIO()
    assert serve(stdin, stdout) == 0
    assert json.loads(stdout.getvalue().splitlines()[0])["result"]["bye"] is True


def test_write_frame_rejects_oversize():
    with pytest.raises(ProtocolError):
        write_frame(io.BytesIO(), {"x": "y" * (MAX_FRAME_BYTES + 1)})


def _two_page_pdf() -> str:
    import base64

    pymupdf = pytest.importorskip("pymupdf")
    doc = pymupdf.open()
    doc.new_page().insert_text((72, 72), "Ek-1")
    doc.new_page().insert_text((72, 72), "Ek-2")
    data = doc.tobytes()
    doc.close()
    return base64.b64encode(data).decode()


def test_render_page_returns_png_over_protocol():
    import base64

    response, close = handle(
        frame(method="render_page", params={"contentBase64": _two_page_pdf(), "pageIndex": 1, "dpi": 72})
    )
    assert close is False
    assert response["ok"] is True
    assert response["result"]["pageCount"] == 2
    png = base64.b64decode(response["result"]["pngBase64"])
    assert png.startswith(b"\x89PNG\r\n\x1a\n")


def test_render_page_requires_integer_page_index():
    with pytest.raises(ProtocolError) as exc:
        handle(frame(method="render_page", params={"contentBase64": _two_page_pdf(), "pageIndex": "0"}))
    assert exc.value.code == "E_PARAMS"


def test_render_page_out_of_range_is_engine_error():
    with pytest.raises(Exception) as exc:
        handle(frame(method="render_page", params={"contentBase64": _two_page_pdf(), "pageIndex": 5}))
    assert getattr(exc.value, "code", "") == "E_PAGE_RANGE"
