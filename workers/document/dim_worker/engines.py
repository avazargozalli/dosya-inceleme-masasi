"""Belge motorları. Ağır bağımlılıklar isteğe bağlıdır; yoksa açık hata döner.

Worker'a dosya yolu değil **bayt** verilir (ADR-03): çekirdek yalnız iş için
gereken belgeyi gönderir, worker başka klasörleri göremez.
"""

from __future__ import annotations

import base64
import binascii
from dataclasses import dataclass, field
from typing import Any

MAX_PAGES = 2000
MAX_INPUT_BYTES = 250 * 1024 * 1024


class EngineError(Exception):
    def __init__(self, code: str, message: str, retryable: bool = False) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.retryable = retryable


@dataclass
class Page:
    index: int
    text: str
    width: float = 0.0
    height: float = 0.0
    warnings: list[str] = field(default_factory=list)


@dataclass
class Extraction:
    engine: str
    engine_version: str
    pages: list[Page]
    warnings: list[str] = field(default_factory=list)


def decode_content(params: dict[str, Any]) -> bytes:
    encoded = params.get("contentBase64")
    if not isinstance(encoded, str) or not encoded:
        raise EngineError("E_INPUT", "contentBase64 zorunlu")
    try:
        data = base64.b64decode(encoded, validate=True)
    except (binascii.Error, ValueError) as exc:
        raise EngineError("E_INPUT_BASE64", "içerik base64 değil") from exc
    if len(data) > MAX_INPUT_BYTES:
        raise EngineError("E_INPUT_TOO_LARGE", "belge 250 MiB sınırını aşıyor")
    return data


def extract_pdf(data: bytes) -> Extraction:
    try:
        import pymupdf  # type: ignore
    except ImportError as exc:  # pragma: no cover - paket yoksa
        raise EngineError("E_ENGINE_MISSING", "PyMuPDF bu worker paketinde yok", retryable=False) from exc

    try:
        doc = pymupdf.open(stream=data, filetype="pdf")
    except Exception as exc:  # noqa: BLE001 - motor hatası normalize edilir
        raise EngineError("E_PDF_OPEN", "PDF açılamadı (bozuk veya parola korumalı)") from exc

    if doc.needs_pass:
        raise EngineError("E_PDF_PASSWORD", "belge parola korumalı; parola desteği F06'da")

    pages: list[Page] = []
    warnings: list[str] = []
    try:
        count = doc.page_count
        if count > MAX_PAGES:
            raise EngineError("E_PDF_TOO_MANY_PAGES", f"belge {MAX_PAGES} sayfa sınırını aşıyor")
        for index in range(count):
            page = doc.load_page(index)
            text = page.get_text("text") or ""
            if not text.strip():
                warnings.append(f"sayfa {index + 1}: metin katmanı yok (OCR gerekli)")
            rect = page.rect
            pages.append(Page(index=index, text=text, width=float(rect.width), height=float(rect.height)))
    finally:
        doc.close()

    version = getattr(getattr(pymupdf, "__version__", None), "__str__", lambda: "unknown")()
    return Extraction(engine="pymupdf", engine_version=str(version), pages=pages, warnings=warnings)


def capabilities() -> dict[str, Any]:
    pdf = False
    version = None
    try:
        import pymupdf  # type: ignore

        pdf = True
        version = str(pymupdf.__version__)
    except ImportError:
        pass
    return {
        "protocolVersion": 1,
        "engines": {"pdf": {"available": pdf, "version": version}},
        "limits": {"maxPages": MAX_PAGES, "maxInputBytes": MAX_INPUT_BYTES},
    }
