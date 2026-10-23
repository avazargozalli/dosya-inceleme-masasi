"""Belge motorları: PDF, DOCX, düz metin ve isteğe bağlı OCR.

Worker'a dosya yolu değil **bayt** verilir (ADR-03). Ağır bağımlılıklar
isteğe bağlıdır; yoksa açık hata kodu döner, sessizce boş metin üretilmez.
"""

from __future__ import annotations

import base64
import binascii
import io
import shutil
from dataclasses import dataclass, field
from typing import Any

MAX_PAGES = 2000
MAX_INPUT_BYTES = 250 * 1024 * 1024
OCR_DPI = 300


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


def _pymupdf():
    try:
        import pymupdf  # type: ignore
    except ImportError as exc:  # pragma: no cover - paket yoksa
        raise EngineError("E_ENGINE_MISSING", "PyMuPDF bu worker paketinde yok") from exc
    return pymupdf


def _tesseract_available() -> bool:
    try:
        import pytesseract  # type: ignore  # noqa: F401
    except ImportError:
        return False
    return shutil.which("tesseract") is not None


def _ocr_page(page, lang: str) -> str:
    try:
        import pytesseract  # type: ignore
        from PIL import Image  # type: ignore
    except ImportError as exc:
        raise EngineError("E_OCR_MISSING", "OCR bağımlılıkları (pytesseract/Pillow) kurulu değil") from exc
    if shutil.which("tesseract") is None:
        raise EngineError("E_OCR_MISSING", "Tesseract yürütülebilir dosyası bulunamadı")

    pix = page.get_pixmap(dpi=OCR_DPI, alpha=False)
    image = Image.open(io.BytesIO(pix.tobytes("png")))
    try:
        return pytesseract.image_to_string(image, lang=lang) or ""
    except Exception as exc:  # noqa: BLE001 - motor hatası normalize edilir
        raise EngineError("E_OCR_FAILED", "OCR çalıştırılamadı (dil paketi eksik olabilir)") from exc


def extract_pdf(data: bytes, ocr: bool = False, ocr_lang: str = "eng") -> Extraction:
    pymupdf = _pymupdf()
    try:
        doc = pymupdf.open(stream=data, filetype="pdf")
    except Exception as exc:  # noqa: BLE001
        raise EngineError("E_PDF_OPEN", "PDF açılamadı (bozuk veya parola korumalı)") from exc

    if doc.needs_pass:
        raise EngineError("E_PDF_PASSWORD", "belge parola korumalı; parola desteği henüz yok")

    pages: list[Page] = []
    warnings: list[str] = []
    try:
        count = doc.page_count
        if count > MAX_PAGES:
            raise EngineError("E_PDF_TOO_MANY_PAGES", f"belge {MAX_PAGES} sayfa sınırını aşıyor")
        for index in range(count):
            page = doc.load_page(index)
            text = page.get_text("text") or ""
            page_warnings: list[str] = []
            if not text.strip():
                if ocr:
                    text = _ocr_page(page, ocr_lang)
                    if text.strip():
                        page_warnings.append(f"sayfa {index + 1}: OCR ile okundu ({ocr_lang})")
                    else:
                        page_warnings.append(f"sayfa {index + 1}: OCR sonuç vermedi")
                else:
                    page_warnings.append(f"sayfa {index + 1}: metin katmanı yok (OCR gerekli)")
                warnings.extend(page_warnings)
            rect = page.rect
            pages.append(
                Page(index=index, text=text, width=float(rect.width), height=float(rect.height), warnings=page_warnings)
            )
    finally:
        doc.close()

    version = str(getattr(pymupdf, "__version__", "unknown"))
    return Extraction(engine="pymupdf", engine_version=version, pages=pages, warnings=warnings)


def extract_docx(data: bytes) -> Extraction:
    try:
        import docx  # type: ignore
    except ImportError as exc:
        raise EngineError("E_DOCX_MISSING", "python-docx bu worker paketinde yok") from exc

    try:
        document = docx.Document(io.BytesIO(data))
    except Exception as exc:  # noqa: BLE001
        raise EngineError("E_DOCX_OPEN", "DOCX açılamadı (bozuk veya desteklenmeyen biçim)") from exc

    blocks: list[str] = [p.text for p in document.paragraphs if p.text.strip()]
    for table in document.tables:
        for row in table.rows:
            cells = [c.text.strip() for c in row.cells if c.text.strip()]
            if cells:
                blocks.append(" | ".join(cells))
    for section in document.sections:
        for header in (section.header, section.footer):
            for p in header.paragraphs:
                if p.text.strip():
                    blocks.append(p.text)

    text = "\n\n".join(blocks)
    warnings = ["DOCX sayfa düzeni doğrulanmadı; kaynak paragraf bazlıdır"]
    if not blocks:
        warnings.append("belgede metin bulunamadı")
    version = str(getattr(docx, "__version__", "unknown"))
    page = Page(index=0, text=text, warnings=warnings)
    return Extraction(engine="python-docx", engine_version=version, pages=[page], warnings=warnings)


def extract_plain(data: bytes, encoding: str = "utf-8") -> Extraction:
    try:
        text = data.decode(encoding)
    except (UnicodeDecodeError, LookupError) as exc:
        raise EngineError("E_TEXT_ENCODING", f"metin {encoding} olarak çözülemedi") from exc
    warnings: list[str] = []
    if not text.strip():
        warnings.append("belgede metin yok")
    page = Page(index=0, text=text, warnings=warnings)
    return Extraction(engine="plain", engine_version="1", pages=[page], warnings=warnings)


def extract(data: bytes, mime: str, ocr: bool = False, ocr_lang: str = "eng", encoding: str = "utf-8") -> Extraction:
    """MIME ve içerik imzasına göre doğru motoru seçer."""
    if mime == "application/pdf" or data.startswith(b"%PDF-"):
        return extract_pdf(data, ocr=ocr, ocr_lang=ocr_lang)
    if "wordprocessingml.document" in mime or (data.startswith(b"PK\x03\x04") and mime.endswith("document")):
        return extract_docx(data)
    if mime.startswith("text/") or mime in {"application/json", "text/csv"}:
        return extract_plain(data, encoding=encoding)
    raise EngineError("E_MIME_UNSUPPORTED", f"desteklenmeyen MIME: {mime}")


def capabilities() -> dict[str, Any]:
    pdf = False
    pdf_version = None
    try:
        import pymupdf  # type: ignore

        pdf = True
        pdf_version = str(pymupdf.__version__)
    except ImportError:
        pass

    docx_ok = False
    try:
        import docx  # type: ignore

        docx_ok = True
        docx_version = str(docx.__version__)
    except ImportError:
        docx_version = None

    ocr_langs: list[str] = []
    if _tesseract_available():
        try:
            import pytesseract  # type: ignore

            ocr_langs = list(pytesseract.get_languages(config=""))
        except Exception:  # noqa: BLE001
            ocr_langs = []

    return {
        "protocolVersion": 1,
        "engines": {
            "pdf": {"available": pdf, "version": pdf_version},
            "docx": {"available": docx_ok, "version": docx_version},
            "text": {"available": True},
            "ocr": {"available": bool(ocr_langs), "languages": ocr_langs, "dpi": OCR_DPI},
        },
        "limits": {"maxPages": MAX_PAGES, "maxInputBytes": MAX_INPUT_BYTES},
    }


@dataclass
class RedactionHit:
    page: int
    term: str
    rect: list[float]


@dataclass
class RedactionResult:
    pdf: bytes
    hits: list[RedactionHit]
    remaining: list[str]
    engine: str
    engine_version: str


def redact_pdf(data: bytes, terms: list[str], fill: tuple[float, float, float] = (0.0, 0.0, 0.0)) -> RedactionResult:
    """Terimleri PDF'in **metin katmanından ve görüntüsünden** gerçekten çıkarır.

    Yalnız siyah dikdörtgen çizmek maskeleme sayılmaz: `apply_redactions`
    içeriği kaldırır, ardından çıktı yeniden açılıp terimlerin kalmadığı
    bağımsız olarak doğrulanır. Doğrulama başarısızsa sonuç kullanılmaz.
    """
    pymupdf = _pymupdf()
    cleaned = [t for t in (terms or []) if isinstance(t, str) and t.strip()]
    if not cleaned:
        raise EngineError("E_REDACT_TERMS", "maskelenecek terim listesi boş")

    try:
        doc = pymupdf.open(stream=data, filetype="pdf")
    except Exception as exc:  # noqa: BLE001
        raise EngineError("E_PDF_OPEN", "PDF açılamadı (bozuk veya parola korumalı)") from exc
    if doc.needs_pass:
        raise EngineError("E_PDF_PASSWORD", "belge parola korumalı; parola desteği henüz yok")

    hits: list[RedactionHit] = []
    try:
        for page in doc:
            for term in cleaned:
                for rect in page.search_for(term):
                    page.add_redact_annot(rect, fill=fill)
                    hits.append(
                        RedactionHit(
                            page=page.number,
                            term=term,
                            rect=[float(rect.x0), float(rect.y0), float(rect.x1), float(rect.y1)],
                        )
                    )
            # Görüntü pikselleri de temizlenir; yalnız vektör kutusu bırakılmaz.
            page.apply_redactions(images=pymupdf.PDF_REDACT_IMAGE_PIXELS)
        doc.scrub(metadata=True, xml_metadata=True, attached_files=True, hidden_text=True)
        out = doc.tobytes(garbage=4, deflate=True)
    finally:
        doc.close()

    check = pymupdf.open(stream=out, filetype="pdf")
    remaining: list[str] = []
    try:
        for page in check:
            text = page.get_text("text") or ""
            for term in cleaned:
                if term in text or page.search_for(term):
                    remaining.append(term)
                    break
    finally:
        check.close()

    version = str(getattr(pymupdf, "__version__", "unknown"))
    return RedactionResult(
        pdf=out,
        hits=hits,
        remaining=sorted(set(remaining)),
        engine="pymupdf",
        engine_version=version,
    )


def render_report(payload: dict[str, Any]) -> bytes:
    """Duruşma/devir raporunu DOCX olarak üretir. Girdi çağıran çekirdekten gelir;
    worker veritabanına erişmez. İçerik düz metindir, HTML çalıştırılmaz."""
    try:
        import docx  # type: ignore
        from docx.shared import Pt  # type: ignore
    except ImportError as exc:
        raise EngineError("E_DOCX_MISSING", "python-docx bu worker paketinde yok") from exc

    document = docx.Document()
    document.add_heading(str(payload.get("title") or "Dosya raporu"), 0)
    meta = payload.get("meta") or {}
    if isinstance(meta, dict):
        for key, value in meta.items():
            if value not in (None, ""):
                p = document.add_paragraph()
                p.add_run(f"{key}: ").bold = True
                p.add_run(str(value))

    for section in payload.get("sections") or []:
        if not isinstance(section, dict):
            continue
        document.add_heading(str(section.get("heading") or ""), level=1)
        rows = section.get("rows") or []
        if rows and isinstance(rows[0], list):
            table = document.add_table(rows=1, cols=len(rows[0]))
            table.style = "Light Grid Accent 1"
            header = table.rows[0].cells
            for index, value in enumerate(rows[0]):
                header[index].text = str(value)
            for row in rows[1:]:
                cells = table.add_row().cells
                for index, value in enumerate(row[: len(cells)]):
                    cells[index].text = str(value)
        for line in section.get("lines") or []:
            document.add_paragraph(str(line))

    footer = document.add_paragraph()
    run = footer.add_run("Bu rapor Dosya İnceleme Masası tarafından üretildi; hukuki sonuç değildir.")
    run.font.size = Pt(8)
    buffer = io.BytesIO()
    document.save(buffer)
    return buffer.getvalue()


def _report_html(payload: dict[str, Any]) -> str:
    import html as _html

    esc = _html.escape
    parts = [f"<h1>{esc(str(payload.get('title') or 'Dosya raporu'))}</h1>"]
    meta = payload.get("meta") or {}
    if isinstance(meta, dict):
        parts.append("<p>")
        for key, value in meta.items():
            if value not in (None, ""):
                parts.append(f"<b>{esc(str(key))}:</b> {esc(str(value))}<br/>")
        parts.append("</p>")

    for section in payload.get("sections") or []:
        if not isinstance(section, dict):
            continue
        parts.append(f"<h2>{esc(str(section.get('heading') or ''))}</h2>")
        rows = section.get("rows") or []
        if rows and isinstance(rows[0], list):
            parts.append('<table border="1" cellspacing="0" cellpadding="4">')
            header = rows[0]
            parts.append("<tr>" + "".join(f"<th>{esc(str(c))}</th>" for c in header) + "</tr>")
            for row in rows[1:]:
                parts.append("<tr>" + "".join(f"<td>{esc(str(c))}</td>" for c in row) + "</tr>")
            parts.append("</table>")
        for line in section.get("lines") or []:
            parts.append(f"<p>{esc(str(line))}</p>")
    parts.append(
        "<p><i>Bu rapor Dosya İnceleme Masası tarafından üretildi; hukuki sonuç değildir.</i></p>"
    )
    return "".join(parts)


def render_report_pdf(payload: dict[str, Any]) -> bytes:
    """Raporu PDF olarak üretir. HTML kaçışlanır; kullanıcı verisi HTML olarak
    çalıştırılmaz. Uzun içerik `Story` ile sayfalara akıtılır."""
    pymupdf = _pymupdf()
    html = _report_html(payload)
    story = pymupdf.Story(html=html)
    buffer = io.BytesIO()
    writer = pymupdf.DocumentWriter(buffer)
    rect = pymupdf.paper_rect("a4")
    where = rect + (36, 36, -36, -36)
    more = True
    pages = 0
    while more:
        device = writer.begin_page(rect)
        more, _ = story.place(where)
        story.draw(device)
        writer.end_page()
        pages += 1
        if pages > 200:
            break
    writer.close()
    return buffer.getvalue()
