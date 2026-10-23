"""Worker giriş noktası: `python -m dim_worker.worker`.

Protokol: NDJSON, tek satır = tek çerçeve, en fazla 1 MiB.
Yöntemler: ping, capabilities, extract_text, shutdown.
"""

from __future__ import annotations

import base64
import sys
from typing import Any

from . import PROTOCOL_VERSION
from .engines import EngineError, capabilities, decode_content, extract, redact_pdf, render_report
from .protocol import ProtocolError, err, ok, read_frame, write_frame

METHODS = {"ping", "capabilities", "extract_text", "redact", "render_report", "shutdown"}


def _require_version(frame: dict[str, Any]) -> None:
    if frame.get("v") != PROTOCOL_VERSION:
        raise ProtocolError("E_VERSION", f"desteklenmeyen protokol sürümü: {frame.get('v')!r}")


def _request_id(frame: dict[str, Any]) -> str | None:
    rid = frame.get("id")
    if rid is None:
        return None
    if not isinstance(rid, str) or not rid or len(rid) > 64:
        raise ProtocolError("E_REQUEST_ID", "id 1-64 karakter metin olmalı")
    return rid


def handle(frame: dict[str, Any]) -> tuple[dict[str, Any], bool]:
    """Tek isteği işler. (yanıt, kapanmalı_mı) döner."""
    rid = _request_id(frame)
    _require_version(frame)
    method = frame.get("method")
    if method not in METHODS:
        raise ProtocolError("E_METHOD", "bilinmeyen yöntem")
    params = frame.get("params") or {}
    if not isinstance(params, dict):
        raise ProtocolError("E_PARAMS", "params nesne olmalı")

    if method == "ping":
        return ok(rid, {"pong": True, "workerVersion": "0.1.0"}), False
    if method == "capabilities":
        return ok(rid, capabilities()), False
    if method == "shutdown":
        return ok(rid, {"bye": True}), True

    data = decode_content(params)

    if method == "render_report":
        payload = params.get("payload")
        if not isinstance(payload, dict):
            raise ProtocolError("E_PARAMS", "payload nesnesi zorunlu")
        data = render_report(payload)
        import base64 as _b64

        return ok(rid, {"docxBase64": _b64.b64encode(data).decode(), "bytes": len(data)}), False

    if method == "redact":
        terms = params.get("terms")
        if not isinstance(terms, list) or not terms:
            raise ProtocolError("E_PARAMS", "terms listesi zorunlu")
        result = redact_pdf(data, [str(t) for t in terms])
        import base64 as _b64

        return ok(
            rid,
            {
                "pdfBase64": _b64.b64encode(result.pdf).decode(),
                "hits": [{"page": h.page, "term": h.term, "rect": h.rect} for h in result.hits],
                "remaining": result.remaining,
                "engine": result.engine,
                "engineVersion": result.engine_version,
            },
        ), False

    mime = params.get("mime")
    if not isinstance(mime, str) or not mime:
        raise ProtocolError("E_PARAMS", "mime zorunlu")
    ocr = bool(params.get("ocr", False))
    ocr_lang = str(params.get("ocrLang") or "eng")
    encoding = str(params.get("encoding") or "utf-8")
    extraction = extract(data, mime, ocr=ocr, ocr_lang=ocr_lang, encoding=encoding)
    result = {
        "engine": extraction.engine,
        "engineVersion": extraction.engine_version,
        "pages": [
            {
                "index": p.index,
                "text": p.text,
                "width": p.width,
                "height": p.height,
                "warnings": p.warnings,
            }
            for p in extraction.pages
        ],
        "warnings": extraction.warnings,
    }
    return ok(rid, result), False


def serve(stdin, stdout) -> int:
    while True:
        try:
            frame = read_frame(stdin)
        except ProtocolError as exc:
            write_frame(stdout, err(None, exc.code, exc.message, exc.retryable))
            return 2
        if frame is None:
            return 0
        rid: str | None = frame.get("id") if isinstance(frame.get("id"), str) else None
        try:
            response, should_close = handle(frame)
        except ProtocolError as exc:
            response, should_close = err(rid, exc.code, exc.message, exc.retryable), False
        except EngineError as exc:
            response, should_close = err(rid, exc.code, exc.message, exc.retryable), False
        except Exception:  # noqa: BLE001 - iç hata ayrıntısı sızdırılmaz
            response, should_close = err(rid, "E_WORKER_INTERNAL", "worker iç hatası"), False
        try:
            write_frame(stdout, response)
        except ProtocolError as exc:
            write_frame(stdout, err(rid, exc.code, exc.message))
            return 3
        if should_close:
            return 0


def main() -> None:
    sys.exit(serve(sys.stdin.buffer, sys.stdout.buffer))


if __name__ == "__main__":
    main()
