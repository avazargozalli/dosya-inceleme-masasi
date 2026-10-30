"""Worker giriş noktası: `python -m dim_worker.worker`.

Protokol: NDJSON, tek satır = tek çerçeve, en fazla 1 MiB.
Yöntemler: ping, capabilities, extract_text, redact, shutdown.

`extract_text` yanıtı 900 KiB bütçesini aşarsa sayfalar `part`/`parts`
alanlarıyla birden çok çerçevede gönderilir; istemci sırayı doğrulayıp
birleştirir. Tek bir sayfa bile sığmıyorsa `E_FRAME_TOO_LARGE` döner, metin
sessizce kırpılmaz.

**İlerleme çerçeveleri (F04-T07):** çok sayfalı çıkarımda worker, sonuç
çerçevesinden **önce** `{"id":…,"progress":{"stage":"pages","done":n,
"total":m}}` satırları yazar. Bunlar yanıt değildir (`ok` yok); istemci
bunları ayrı okur ve sonucu beklemezden önce kullanıcıya aşama/sayım gösterir.
"""

from __future__ import annotations

import base64
import json
import sys
from collections.abc import Callable
from typing import Any

from . import PROTOCOL_VERSION
from .engines import (
    MAX_BLOCKS_PER_PAGE,
    DEFAULT_OCR_LANG,
    EngineError,
    capabilities,
    decode_content,
    extract,
    redact_pdf,
    render_page_png,
    render_report,
    render_report_pdf,
)
from .protocol import ProtocolError, err, ok, progress_frame, read_frame, write_frame
from .inventory import InventoryError, inventory as build_inventory

METHODS = {"ping", "capabilities", "extract_text", "inventory", "redact", "redact_regions", "render_report", "render_report_pdf", "render_page", "shutdown"}

# İlerleme geri çağrısı: (aşama, sayılan, toplam). Toplam bilinmiyorsa `None`.
Progress = Callable[[str, int, int | None], None]

# Geometri (blok/satır kutuları) çerçevenin bir parçasıdır. 1 MiB sert sınırına
# yaklaşmamak için bütçe ayrılır; aşılırsa kutular kademeli atılır ve durum
# `warnings` içinde açıkça yazılır — sessiz öğe kaybı olmaz.
GEOMETRY_FRAME_BUDGET = 700 * 1024

# Metin yanıtı tek çerçeveye sığmazsa sayfalar parçalara bölünür. Sert sınır
# 1 MiB'tir; zarf ve JSON kaçışları için pay bırakılır.
RESPONSE_FRAME_BUDGET = 900 * 1024
MAX_RESPONSE_PARTS = 5000


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


def _page_payload(page: Any) -> dict[str, Any]:
    """Sayfayı protokole çevirir. Geometri kutuları `start`/`end` (Unicode
    scalar) ile birlikte taşınır; kutu yoksa alan yazılmaz."""
    blocks: list[dict[str, Any]] = []
    for block in page.blocks[:MAX_BLOCKS_PER_PAGE]:
        entry: dict[str, Any] = {"start": block.start, "end": block.end}
        if block.bbox:
            entry["bbox"] = block.bbox
        if block.lines:
            entry["lines"] = block.lines
        # Yapısal motorlarda paragraf/bölüm bağlantısı; yoksa alan yazılmaz.
        path = getattr(block, "path", None)
        if path:
            entry["path"] = path
        kind = getattr(block, "kind", None)
        if kind:
            entry["kind"] = kind
        blocks.append(entry)
    payload: dict[str, Any] = {
        "index": page.index,
        "text": page.text,
        "width": page.width,
        "height": page.height,
        "rotation": page.rotation,
        "coordinateSpace": page.coordinate_space,
        "blocks": blocks,
        "warnings": page.warnings,
    }
    # Etiket yalnız PDF'te gerçekten tanımlıysa taşınır; yoksa alan yazılmaz
    # (istemci `null` ile aynı şeyi anlar, arayüz etiket uydurmaz).
    label = getattr(page, "label", None)
    if label:
        payload["pageLabel"] = label
    return payload


def _encoded_size(payload: dict[str, Any]) -> int:
    return len(json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8"))


def _shrink_geometry(result: dict[str, Any]) -> None:
    """Çerçeve sınırı için geometriyi kademeli küçültür.

    Sessizce atmaz: her adım `warnings` listesine yazılır, ayrıştırma durumu
    `partial` olur. Önce satır kutuları, sonra blok kutuları düşer; metin ve
    offsetler korunur.
    """
    if _encoded_size(result) <= GEOMETRY_FRAME_BUDGET:
        return
    pages = [page for page in result["pages"] if page.get("blocks")]
    for page in pages:
        for block in page["blocks"]:
            block.pop("lines", None)
    if _encoded_size(result) <= GEOMETRY_FRAME_BUDGET:
        result["warnings"].append(
            "geometri: satır kutuları çerçeve sınırı için atıldı (blok kutuları korundu)"
        )
        return
    for page in pages:
        page["blocks"] = []
    result["warnings"].append(
        f"geometri: {len(pages)} sayfada blok kutuları çerçeve sınırı için atıldı; metin korundu"
    )


def _geometry_warnings(extraction: Any, warnings: list[str]) -> None:
    for page in extraction.pages:
        if len(page.blocks) > MAX_BLOCKS_PER_PAGE:
            warnings.append(
                f"geometri: sayfa {page.index + 1} ilk {MAX_BLOCKS_PER_PAGE} blokla sınırlandı"
            )


def _split_pages(result: dict[str, Any]) -> list[dict[str, Any]]:
    """Sayfaları çerçeve bütçesine sığacak parçalara böler.

    Zarf (motor, sürüm, uyarılar) her parçada tekrarlanır; sayfa sırası korunur.
    Tek bir sayfa bütçeyi aşıyorsa parçalamak offsetsiz metin üretirdi: açık
    hata döner, sessiz kırpma yapılmaz.
    """
    envelope = {key: value for key, value in result.items() if key != "pages"}
    base_size = _encoded_size(dict(envelope, pages=[]))
    pages = result["pages"]
    parts: list[list[dict[str, Any]]] = []
    current: list[dict[str, Any]] = []
    used = base_size
    for page in pages:
        size = _encoded_size(page) + 1  # ayırıcı virgül
        if base_size + size > RESPONSE_FRAME_BUDGET:
            raise ProtocolError(
                "E_FRAME_TOO_LARGE",
                f"sayfa {page.get('index', '?')} tek çerçeve sınırını aşıyor",
            )
        if current and used + size > RESPONSE_FRAME_BUDGET:
            parts.append(current)
            current = []
            used = base_size
        current.append(page)
        used += size
    if current:
        parts.append(current)
    if len(parts) > MAX_RESPONSE_PARTS:
        raise ProtocolError("E_FRAME_TOO_MANY_PARTS", "yanıt çok fazla parçaya bölündü")
    return [
        dict(result, pages=chunk, part=index, parts=len(parts))
        for index, chunk in enumerate(parts, start=1)
    ]


def _extract_frames(rid: str | None, extraction: Any) -> list[dict[str, Any]]:
    """Metin çıkarımını çerçevelere çevirir. Sığarsa tek çerçeve; sığmazsa
    sayfalar `part`/`parts` alanlarıyla bölünür (istemci birleştirir)."""
    warnings = list(extraction.warnings)
    _geometry_warnings(extraction, warnings)
    result: dict[str, Any] = {
        "engine": extraction.engine,
        "engineVersion": extraction.engine_version,
        "pages": [_page_payload(page) for page in extraction.pages],
        "warnings": warnings,
    }
    _shrink_geometry(result)
    if _encoded_size(result) <= RESPONSE_FRAME_BUDGET:
        return [ok(rid, result)]
    return [ok(rid, chunk) for chunk in _split_pages(result)]


def handle(
    frame: dict[str, Any], progress: Progress | None = None
) -> tuple[dict[str, Any] | list[dict[str, Any]], bool]:
    """Tek isteği işler. (yanıt, kapanmalı_mı) döner. `progress` verilirse
    çok sayfalı çıkarımda aşama/sayım bildirir (F04-T07)."""
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

    if method == "inventory":
        # Envanter paket düzeyinde çıkarılır: yapı sayıları, ekler, değişiklik
        # izi. Gövde metni yanıta konmaz; MIME zorunludur (DOCX/UDF).
        mime = params.get("mime")
        if not isinstance(mime, str) or not mime:
            raise ProtocolError("E_PARAMS", "mime zorunlu")
        result = build_inventory(data, mime)
        return ok(rid, result.to_dict()), False

    if method == "render_page":
        page_index = params.get("pageIndex")
        dpi = params.get("dpi", 96)
        if isinstance(page_index, bool) or not isinstance(page_index, int):
            raise ProtocolError("E_PARAMS", "pageIndex tam sayı olmalı")
        if isinstance(dpi, bool) or not isinstance(dpi, int):
            raise ProtocolError("E_PARAMS", "dpi tam sayı olmalı")
        png, total = render_page_png(data, page_index, dpi)
        import base64 as _b64

        return ok(
            rid,
            {
                "pngBase64": _b64.b64encode(png).decode(),
                "pageCount": total,
                "bytes": len(png),
            },
        ), False

    if method == "render_report":
        payload = params.get("payload")
        if not isinstance(payload, dict):
            raise ProtocolError("E_PARAMS", "payload nesnesi zorunlu")
        data = render_report(payload)
        import base64 as _b64

        return ok(rid, {"docxBase64": _b64.b64encode(data).decode(), "bytes": len(data)}), False

    if method == "render_report_pdf":
        payload = params.get("payload")
        if not isinstance(payload, dict):
            raise ProtocolError("E_PARAMS", "payload nesnesi zorunlu")
        data = render_report_pdf(payload)
        import base64 as _b64

        return ok(rid, {"pdfBase64": _b64.b64encode(data).decode(), "bytes": len(data)}), False

    if method == "redact_regions":
        regions = params.get("regions")
        if not isinstance(regions, list) or not regions:
            raise ProtocolError("E_PARAMS", "regions listesi zorunlu")
        result = redact_pdf_regions(data, regions, progress=progress)
        import base64 as _b64

        return ok(
            rid,
            {
                "pdfBase64": _b64.b64encode(result.pdf).decode(),
                "hits": [{"page": h.page, "term": h.term, "rect": h.rect} for h in result.hits],
                "remaining": result.remaining,
                "engine": result.engine,
                "engineVersion": result.engine_version,
                "ocrVerified": result.ocr_verified,
                "engines": result.engines,
                "streamsScanned": result.streams_scanned,
                "streamsBinary": result.streams_binary,
                "streamsUnreadable": result.streams_unreadable,
                "annotationsRemoved": result.annotations_removed,
                "offpageSpansRemoved": result.offpage_spans_removed,
                "scriptsRemoved": result.scripts_removed,
                "warnings": result.warnings,
            },
        ), False

    if method == "redact":
        terms = params.get("terms")
        if not isinstance(terms, list) or not terms:
            raise ProtocolError("E_PARAMS", "terms listesi zorunlu")
        result = redact_pdf(data, [str(t) for t in terms], progress=progress)
        import base64 as _b64

        return ok(
            rid,
            {
                "pdfBase64": _b64.b64encode(result.pdf).decode(),
                "hits": [{"page": h.page, "term": h.term, "rect": h.rect} for h in result.hits],
                "remaining": result.remaining,
                "engine": result.engine,
                "engineVersion": result.engine_version,
                "ocrVerified": result.ocr_verified,
                "engines": result.engines,
                "streamsScanned": result.streams_scanned,
                "streamsBinary": result.streams_binary,
                "streamsUnreadable": result.streams_unreadable,
                "annotationsRemoved": result.annotations_removed,
                "offpageSpansRemoved": result.offpage_spans_removed,
                "scriptsRemoved": result.scripts_removed,
                "warnings": result.warnings,
            },
        ), False

    mime = params.get("mime")
    if not isinstance(mime, str) or not mime:
        raise ProtocolError("E_PARAMS", "mime zorunlu")
    ocr = bool(params.get("ocr", False))
    ocr_lang = str(params.get("ocrLang") or DEFAULT_OCR_LANG)
    password = params.get("password")
    if password is not None and not isinstance(password, str):
        raise ProtocolError("E_PARAMS", "password metin olmalı")
    encoding = str(params.get("encoding") or "auto")
    extraction = extract(
        data,
        mime,
        ocr=ocr,
        ocr_lang=ocr_lang,
        encoding=encoding,
        password=password,
        progress=progress,
    )
    return _extract_frames(rid, extraction), False


def progress_writer(stream, request_id: str | None) -> Progress:
    """İlerlemeyi çerçeve olarak yazan geri çağrı.

    Yazma düşerse **ayrıştırma düşmez**: ilerleme bilgilendiricidir, sonuç
    çerçevesinin yazımı ayrıca denenir ve hata orada raporlanır.
    """

    def emit(stage: str, done: int, total: int | None = None) -> None:
        try:
            write_frame(stream, progress_frame(request_id, stage, done, total))
        except ProtocolError:
            pass

    return emit


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
        progress = progress_writer(stdout, rid)
        try:
            response, should_close = handle(frame, progress=progress)
        except ProtocolError as exc:
            response, should_close = err(rid, exc.code, exc.message, exc.retryable), False
        except EngineError as exc:
            response, should_close = err(rid, exc.code, exc.message, exc.retryable), False
        except InventoryError as exc:
            response, should_close = err(rid, exc.code, exc.message), False
        except Exception:  # noqa: BLE001 - iç hata ayrıntısı sızdırılmaz
            response, should_close = err(rid, "E_WORKER_INTERNAL", "worker iç hatası"), False
        # Tek yanıt veya parçalı yanıt: sıra korunur, her çerçeve 1 MiB altındadır.
        frames = response if isinstance(response, list) else [response]
        try:
            for item in frames:
                write_frame(stdout, item)
        except ProtocolError as exc:
            write_frame(stdout, err(rid, exc.code, exc.message))
            return 3
        if should_close:
            return 0


def main() -> None:
    sys.exit(serve(sys.stdin.buffer, sys.stdout.buffer))


if __name__ == "__main__":
    main()
