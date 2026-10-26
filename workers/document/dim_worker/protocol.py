"""NDJSON çerçeve protokolü.

Her satır tek bir JSON nesnesidir. Çerçeve boyutu sınırlıdır; sınır aşılırsa
okuma durur ve bağlantı kapanır (senkron kaybı sessizce yutulmaz).
"""

from __future__ import annotations

import json
from typing import Any, BinaryIO

PROTOCOL_VERSION = 1
MAX_FRAME_BYTES = 1024 * 1024  # 1 MiB
MAX_LINE_BYTES = MAX_FRAME_BYTES + 1


class ProtocolError(Exception):
    def __init__(self, code: str, message: str, retryable: bool = False) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.retryable = retryable


def read_frame(stream: BinaryIO) -> dict[str, Any] | None:
    """Bir satır okur. EOF'ta None. Bozuk/oversize çerçevede ProtocolError."""
    line = stream.readline(MAX_LINE_BYTES)
    if not line:
        return None
    if len(line) > MAX_FRAME_BYTES:
        raise ProtocolError("E_FRAME_TOO_LARGE", "çerçeve 1 MiB sınırını aşıyor")
    try:
        text = line.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise ProtocolError("E_FRAME_ENCODING", "çerçeve UTF-8 değil") from exc
    try:
        obj = json.loads(text)
    except json.JSONDecodeError as exc:
        raise ProtocolError("E_FRAME_JSON", "çerçeve geçerli JSON değil") from exc
    if not isinstance(obj, dict):
        raise ProtocolError("E_FRAME_SHAPE", "çerçeve nesne olmalı")
    return obj


def write_frame(stream: BinaryIO, payload: dict[str, Any]) -> None:
    data = json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    if len(data) > MAX_FRAME_BYTES:
        raise ProtocolError("E_FRAME_TOO_LARGE", "yanıt 1 MiB sınırını aşıyor")
    stream.write(data + b"\n")
    stream.flush()


def progress_frame(
    request_id: str | None, stage: str, done: int, total: int | None = None
) -> dict[str, Any]:
    """İlerleme çerçevesi (F04-T07).

    İlerleme **yanıt değildir**: `ok` alanı yoktur, istek kimliğini taşır ve
    isteğe ait sonuç çerçevesinden **önce** yazılır. Belge metni, dosya adı
    veya yol taşımaz; yalnız aşama ve sayaç.
    """
    entry: dict[str, Any] = {"stage": stage, "done": done}
    if total is not None:
        entry["total"] = total
    return {"v": PROTOCOL_VERSION, "id": request_id, "progress": entry}


def ok(request_id: str | None, result: dict[str, Any]) -> dict[str, Any]:
    return {"v": PROTOCOL_VERSION, "id": request_id, "ok": True, "result": result}


def err(request_id: str | None, code: str, message: str, retryable: bool = False) -> dict[str, Any]:
    return {
        "v": PROTOCOL_VERSION,
        "id": request_id,
        "ok": False,
        "error": {"code": code, "message": message, "retryable": retryable},
    }
