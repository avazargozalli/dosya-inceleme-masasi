"""Belge motorları: PDF, DOCX, düz metin ve isteğe bağlı OCR.

Worker'a dosya yolu değil **bayt** verilir (ADR-03). Ağır bağımlılıklar
isteğe bağlıdır; yoksa açık hata kodu döner, sessizce boş metin üretilmez.
"""

from __future__ import annotations

import base64
import binascii
import codecs
import io
import os
import re
import shutil
import subprocess
import unicodedata
import zlib
from collections import Counter
from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Any

from .udf import UdfError, extract_udf_document

MAX_PAGES = 2000
MAX_INPUT_BYTES = 250 * 1024 * 1024
MAX_IMAGE_FRAMES = 100
MAX_IMAGE_PIXELS = 50_000_000
OCR_DPI = 300
OCR_MIN_CONFIDENCE = 60.0

# Ortak koordinat uzayları. Okuyucu, kırmızı çizim ve redaksiyon aynı uzayı
# kullanır; uzay adı kayıtla birlikte taşınır (F06-T08).
COORDINATE_SPACE_PDF = "unrotated_cropbox_points"
COORDINATE_SPACE_IMAGE = "image_pixels"
COORDINATE_SPACE_STRUCTURE = "structure"

# Sayfa başına blok ve blok başına satır kutusu sınırı: çerçeve 1 MiB'tir,
# sınırsız kutu yanıtı büyütüp ayrıştırmayı düşürür.
MAX_BLOCKS_PER_PAGE = 300
MAX_LINES_PER_BLOCK = 60
# Basılı etiket kısa olur (iv, A-12, Ek-3); aşırı uzun değer kırpılır.
MAX_PAGE_LABEL_CHARS = 60


class EngineError(Exception):
    def __init__(self, code: str, message: str, retryable: bool = False) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.retryable = retryable


@dataclass
class TextBlock:
    """Sayfa metninin bir bloğu: karakter aralığı + koordinatlar.

    `start`/`end` **Unicode scalar** (Python kod noktası) offsetleridir ve
    `Page.text` içine göredir. `bbox`/`lines` koordinatları sayfanın
    `coordinate_space` uzayındadır; yapısal motorlarda (DOCX/UDF/plain) kutu
    yoktur, yalnız offset aralığı taşınır.
    """

    start: int
    end: int
    bbox: list[float] | None = None
    lines: list[list[float]] = field(default_factory=list)
    # Yapısal motorlarda (DOCX/UDF) paragraf/bölüm bağlantısı. PDF/görüntüde
    # yoktur: uydurulmaz. `path` başlık yığını + paragraf konumudur.
    path: str | None = None
    # "heading" | "paragraph" | "table" | "header" | "footer" | "annotation";
    # yalnız yapısal motorlarda dolaşır.
    kind: str | None = None


def _rounded_box(values: Any) -> list[float]:
    """Kutuyu 0.01 birim duyarlığa yuvarlar; JSON boyutu küçülür, konum korunur."""
    return [round(float(value), 2) for value in values]


@dataclass
class Page:
    index: int
    text: str
    width: float = 0.0
    height: float = 0.0
    warnings: list[str] = field(default_factory=list)
    # Döndürme yalnız gösterimi etkiler; koordinatlar döndürme öncesi uzaydadır.
    rotation: int = 0
    coordinate_space: str = COORDINATE_SPACE_STRUCTURE
    blocks: list[TextBlock] = field(default_factory=list)
    # PDF `/PageLabels` ağacından gelen basılı sayfa etiketi. Ağaç tanımlı
    # değilse `None`: fiziksel numaradan etiket **türetilmez** (uydurulmaz).
    label: str | None = None


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


# Önizleme çizimi: düşük DPI'da PNG. Sınırlar dışındaki istek kırpılmaz,
# reddedilir; var olmayan sayfa için boş görüntü üretilmez.
PREVIEW_MIN_DPI = 48
PREVIEW_MAX_DPI = 200
MAX_PREVIEW_BYTES = 8 * 1024 * 1024


def render_page_png(data: bytes, page_index: int, dpi: int = 96) -> tuple[bytes, int]:
    """PDF sayfasını PNG olarak çizer; `(png, page_count)` döner.

    Sayfa numarası **fiziksel sıradır** (0 tabanlı). Basılı `/PageLabels`
    etiketi kullanılmaz: önizleme fiziksel sayfayı gösterir.
    """
    pymupdf = _pymupdf()
    if isinstance(page_index, bool) or not isinstance(page_index, int):
        raise EngineError("E_PAGE_RANGE", "pageIndex tam sayı olmalı")
    if isinstance(dpi, bool) or not isinstance(dpi, int):
        raise EngineError("E_PAGE_RANGE", "dpi tam sayı olmalı")
    effective_dpi = max(PREVIEW_MIN_DPI, min(PREVIEW_MAX_DPI, dpi))
    try:
        doc = pymupdf.open(stream=data, filetype="pdf")
    except Exception as exc:  # noqa: BLE001 - bozuk PDF
        raise EngineError("E_PARSE", "PDF açılamadı") from exc
    try:
        if doc.needs_pass:
            raise EngineError("E_PDF_PASSWORD", "parola korumalı PDF: önizleme için parola gerekir")
        total = int(doc.page_count)
        if page_index < 0 or page_index >= total:
            raise EngineError("E_PAGE_RANGE", f"sayfa aralık dışında: {page_index} / {total}")
        page = doc.load_page(page_index)
        pix = page.get_pixmap(dpi=effective_dpi, alpha=False)
        png = pix.tobytes("png")
        if len(png) > MAX_PREVIEW_BYTES:
            raise EngineError("E_PREVIEW_TOO_LARGE", "önizleme 8 MiB sınırını aşıyor")
        return png, total
    finally:
        doc.close()


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


def _ocr_text_confidence_and_blocks(
    image, lang: str, scale: float = 1.0
) -> tuple[str, float | None, list[TextBlock]]:
    """Görüntüyü bir kez OCR eder; satır metnini, ortalama kelime güvenini
    (0–100) ve satır kutularını döndürür. Güven okunamazsa `None` döner,
    uydurma skor üretilmez.

    Kutu koordinatları `scale` ile çarpılır: PDF'te piksel → punto
    (72 / dpi), görüntüde 1.0 (piksel uzayı).
    """
    try:
        import pytesseract  # type: ignore
    except ImportError as exc:
        raise EngineError("E_OCR_MISSING", "OCR bağımlılıkları (pytesseract/Pillow) kurulu değil") from exc
    if shutil.which("tesseract") is None:
        raise EngineError("E_OCR_MISSING", "Tesseract yürütülebilir dosyası bulunamadı")

    try:
        data = pytesseract.image_to_data(image, lang=lang, output_type=pytesseract.Output.DICT)
    except Exception as exc:  # noqa: BLE001 - motor hatası normalize edilir
        raise EngineError("E_OCR_FAILED", "OCR çalıştırılamadı (dil paketi eksik olabilir)") from exc

    words = data.get("text") or []
    confidences = data.get("conf") or []
    lines: dict[tuple[int, int, int], list[str]] = {}
    boxes: dict[tuple[int, int, int], list[float]] = {}
    scores: list[float] = []
    for index, raw_word in enumerate(words):
        word = (raw_word or "").strip()
        if not word:
            continue
        key = (
            int(data.get("block_num", [0] * len(words))[index] or 0),
            int(data.get("par_num", [0] * len(words))[index] or 0),
            int(data.get("line_num", [0] * len(words))[index] or 0),
        )
        lines.setdefault(key, []).append(word)
        try:
            left = float(data["left"][index]) * scale
            top = float(data["top"][index]) * scale
            width = float(data["width"][index]) * scale
            height = float(data["height"][index]) * scale
            merged = boxes.get(key)
            if merged is None:
                boxes[key] = [left, top, left + width, top + height]
            else:
                merged[0] = min(merged[0], left)
                merged[1] = min(merged[1], top)
                merged[2] = max(merged[2], left + width)
                merged[3] = max(merged[3], top + height)
        except (KeyError, IndexError, TypeError, ValueError):
            pass
        try:
            score = float(confidences[index])
        except (IndexError, TypeError, ValueError):
            continue
        if score >= 0:
            scores.append(score)

    parts: list[str] = []
    blocks: list[TextBlock] = []
    cursor = 0
    for key in sorted(lines):
        line_text = " ".join(lines[key])
        parts.append(line_text)
        parts.append("\n")
        box = boxes.get(key)
        blocks.append(
            TextBlock(
                start=cursor,
                end=cursor + len(line_text) + 1,
                bbox=_rounded_box(box) if box else None,
                lines=[],
            )
        )
        cursor += len(line_text) + 1
    text = "".join(parts)
    mean = round(sum(scores) / len(scores), 1) if scores else None
    return text, mean, blocks


def _ocr_confidence_note(confidence: float | None) -> str:
    """Uyarı metnine eklenen ölçülmüş güven notu. Skor yoksa hiçbir şey eklenmez."""
    if confidence is None:
        return ""
    if confidence < OCR_MIN_CONFIDENCE:
        return f"; güven skoru düşük (ortalama {confidence:.0f}/100)"
    return f"; güven skoru {confidence:.0f}/100"


def _transform_box(box: list[float], matrix: Any) -> list[float]:
    """Kutunun dört köşesini matrisle dönüştürüp eksen hizalı zarfını döndürür."""
    x0, y0, x1, y1 = box
    xs: list[float] = []
    ys: list[float] = []
    for x, y in ((x0, y0), (x1, y0), (x1, y1), (x0, y1)):
        xs.append(matrix.a * x + matrix.c * y + matrix.e)
        ys.append(matrix.b * x + matrix.d * y + matrix.f)
    return _rounded_box([min(xs), min(ys), max(xs), max(ys)])


def _ocr_page(page, lang: str) -> tuple[str, float | None, list[TextBlock]]:
    """Sayfayı 300 dpi'de OCR eder ve kutuları **ortak koordinat uzayına**
    (döndürme öncesi cropbox puntoları) çevirir. Görüntü, sayfanın
    döndürülmüş görünümüdür; bu yüzden `derotation_matrix` uygulanır.
    """
    try:
        from PIL import Image  # type: ignore
    except ImportError as exc:
        raise EngineError("E_OCR_MISSING", "OCR bağımlılıkları (pytesseract/Pillow) kurulu değil") from exc

    pix = page.get_pixmap(dpi=OCR_DPI, alpha=False)
    image = Image.open(io.BytesIO(pix.tobytes("png")))
    text, confidence, blocks = _ocr_text_confidence_and_blocks(image, lang, scale=72.0 / OCR_DPI)
    matrix = page.derotation_matrix
    for block in blocks:
        if block.bbox:
            block.bbox = _transform_box(block.bbox, matrix)
        block.lines = [_transform_box(line, matrix) for line in block.lines]
    return text, confidence, blocks


def _pdf_page_text_and_blocks(page) -> tuple[str, list[TextBlock]]:
    """Metni blok/satır kutularıyla **aynı** çıkarımdan üretir.

    `get_text("dict")` koordinatları döndürme uygulanmamış kırpma kutusu
    uzayındadır (origin cropbox sol-üstü, y aşağı, punto). Metin bu bloklardan
    kurulduğu için `start`/`end` offsetleri ile kutular aynı kaynağa dayanır;
    ayrı bir `get_text("text")` çağrısıyla kayma olmaz.
    """
    payload = page.get_text("dict")
    parts: list[str] = []
    blocks: list[TextBlock] = []
    cursor = 0
    for raw_block in payload.get("blocks", []):
        if raw_block.get("type") != 0:
            continue
        block_start = cursor
        line_boxes: list[list[float]] = []
        for line in raw_block.get("lines") or []:
            text = "".join(str(span.get("text") or "") for span in (line.get("spans") or []))
            if not text:
                continue
            box = line.get("bbox") or raw_block.get("bbox")
            if box and len(line_boxes) < MAX_LINES_PER_BLOCK:
                line_boxes.append(_rounded_box(box))
            parts.append(text)
            parts.append("\n")
            cursor += len(text) + 1
        if cursor == block_start:
            continue
        block_box = raw_block.get("bbox")
        blocks.append(
            TextBlock(
                start=block_start,
                end=cursor,
                bbox=_rounded_box(block_box) if block_box else None,
                lines=line_boxes,
            )
        )
    return "".join(parts), blocks


def _page_label(page: Any, *, defined: bool) -> str | None:
    """PDF `/PageLabels` ağacı tanımlıysa sayfanın basılı etiketini verir.

    Ağaç yoksa `None` döner: PyMuPDF `get_label()` bu durumda fiziksel sayfa
    numarasını verir; onu etiket diye göstermek uydurma olur. Okunamayan,
    boş veya metin olmayan değer de etiket sayılmaz.
    """
    if not defined:
        return None
    try:
        raw = page.get_label()
    except Exception:  # noqa: BLE001 - etiket okunamazsa etiket yok
        return None
    if not isinstance(raw, str):
        return None
    label = raw.strip()
    if not label:
        return None
    return label[:MAX_PAGE_LABEL_CHARS]


def extract_pdf(
    data: bytes,
    ocr: bool = False,
    ocr_lang: str = "eng",
    password: str | None = None,
    progress: Callable[[str, int, int | None], None] | None = None,
) -> Extraction:
    """PDF'i sayfa sayfa çıkarır. `progress(stage, done, total)` çağrısı her
    sayfa bittikçe yapılır (F04-T07 canlı sayaç); geri çağrı verilmezse hiçbir
    şey yapılmaz. İlerleme bilgilendiricidir: ayrıştırma sonucunu değiştirmez.
    """
    notify = progress or (lambda stage, done, total: None)
    pymupdf = _pymupdf()
    try:
        doc = pymupdf.open(stream=data, filetype="pdf")
    except Exception as exc:  # noqa: BLE001
        raise EngineError("E_PDF_OPEN", "PDF açılamadı (bozuk veya parola korumalı)") from exc

    if doc.needs_pass:
        if not password:
            raise EngineError("E_PDF_PASSWORD", "belge parola korumalı; parola gerekli")
        if not doc.authenticate(password):
            raise EngineError("E_PDF_PASSWORD", "parola yanlış; belge açılmadı")

    pages: list[Page] = []
    warnings: list[str] = []
    try:
        count = doc.page_count
        if count > MAX_PAGES:
            raise EngineError("E_PDF_TOO_MANY_PAGES", f"belge {MAX_PAGES} sayfa sınırını aşıyor")
        # Açılış bitti, toplam sayfa biliniyor: arayüz "0 / N" gösterebilir.
        notify("open", 0, count)
        # `/PageLabels` yalnız belgede gerçekten tanımlıysa okunur. PyMuPDF
        # `get_label()` ağaç yokken fiziksel numarayı döndürür; bu etiket gibi
        # gösterilirse uydurma olur. Bu yüzden önce ağacın varlığına bakılır.
        try:
            page_labels = list(doc.get_page_labels() or [])
        except Exception:  # noqa: BLE001 - etiket okunamazsa etiket yok sayılır
            page_labels = []
        for index in range(count):
            page = doc.load_page(index)
            text, blocks = _pdf_page_text_and_blocks(page)
            coordinate_space = COORDINATE_SPACE_PDF
            page_warnings: list[str] = []
            if not text.strip():
                if ocr:
                    text, confidence, blocks = _ocr_page(page, ocr_lang)
                    if text.strip():
                        page_warnings.append(
                            f"sayfa {index + 1}: OCR ile okundu ({ocr_lang}){_ocr_confidence_note(confidence)}"
                        )
                    else:
                        page_warnings.append(f"sayfa {index + 1}: OCR sonuç vermedi")
                else:
                    page_warnings.append(f"sayfa {index + 1}: metin katmanı yok (OCR gerekli)")
                warnings.extend(page_warnings)
            # Koordinat uzayı döndürme öncesidir; genişlik/yükseklik kırpma
            # kutusunun boyutudur. Döndürme ayrı alanda taşınır (90/270'te
            # görünen boyut bunun tersidir).
            crop = page.cropbox
            mupdf_rotation = int(getattr(page, "rotation", 0) or 0) % 360
            pages.append(
                Page(
                    index=index,
                    text=text,
                    width=float(crop.width),
                    height=float(crop.height),
                    warnings=page_warnings,
                    rotation=mupdf_rotation,
                    coordinate_space=coordinate_space,
                    blocks=blocks,
                    label=_page_label(page, defined=bool(page_labels)),
                )
            )
            # Sayfa kasaya yazılmadı; sayaç yalnız **okunan** sayfayı bildirir
            # (yazma aşaması ayrıdır, orada ilerleme olayı yok).
            notify("pages", index + 1, count)
    finally:
        doc.close()

    version = str(getattr(pymupdf, "__version__", "unknown"))
    return Extraction(engine="pymupdf", engine_version=version, pages=pages, warnings=warnings)


def _structure_text(
    parts: list[str],
    separator: str = "\n\n",
    labels: list[tuple[str | None, str | None]] | None = None,
) -> tuple[str, list[TextBlock]]:
    """Parça listesinden metin ve **kutusuz** blok aralıkları üretir.

    Yapısal motorlarda (DOCX/UDF/plain) sayfa koordinatı yoktur; yine de
    paragraf sınırları kaydedilir, böylece kaynak konumu blok düzeyinde
    gösterilebilir. `labels` verilirse her parçanın (yol, tür) bilgisi bloğa
    taşınır; verilmezse blok yolsuz kalır (uydurulmaz).
    """
    pieces: list[str] = []
    blocks: list[TextBlock] = []
    cursor = 0
    for index, text in enumerate(parts):
        if index:
            pieces.append(separator)
            cursor += len(separator)
        path, kind = labels[index] if labels and index < len(labels) else (None, None)
        blocks.append(TextBlock(start=cursor, end=cursor + len(text), path=path, kind=kind))
        pieces.append(text)
        cursor += len(text)
    return "".join(pieces), blocks


_W_NS = "{http://schemas.openxmlformats.org/wordprocessingml/2006/main}"
MAX_DOCX_PART_BYTES = 20 * 1024 * 1024
MAX_DOCX_ANNOTATIONS = 500


def _docx_part(zf: Any, name: str) -> bytes | None:
    try:
        info = zf.getinfo(name)
    except KeyError:
        return None
    if info.file_size > MAX_DOCX_PART_BYTES:
        raise EngineError("E_DOCX_PART_TOO_LARGE", f"{name} 20 MiB sınırını aşıyor")
    return zf.read(name)


def _xml_root(blob: bytes, name: str) -> Any:
    """DOCX XML parçasını ayrıştırır.

    DTD reddedilir: ElementTree varlık genişletmesini sınırlamaz ("billion
    laughs"), dış varlık yüklemez ama iç varlık patlaması belleği tüketir.
    Word bu parçalarda DTD yazmaz; DTD gören belge açık hatayla döner.
    """
    import xml.etree.ElementTree as ET

    if b"<!doctype" in blob[:4096].lower():
        raise EngineError("E_DOCX_XML", f"{name} DTD bildirimi içeriyor; reddedildi")
    try:
        return ET.fromstring(blob)
    except ET.ParseError as exc:
        raise EngineError("E_DOCX_XML", f"{name} geçerli XML değil") from exc


def _node_text(node: Any, tag: str = "t") -> str:
    return "".join(item.text or "" for item in node.iter(f"{_W_NS}{tag}")).strip()


def _docx_annotations(data: bytes) -> tuple[list[str], dict[str, int], list[str]]:
    """DOCX dipnot/sonnot/yorum ve izlenen değişiklik envanteri.

    python-docx bu içerikleri **gövde metnine katmaz**: izlenen ekleme/silme
    metni ve yorum/dipnot metni ayrı XML parçalarındadır. Burada güvenli biçimde
    (dış varlık yok, boyut sınırlı) okunur ve ayrı blok olarak metne eklenir;
    böylece arama ve alıntı bu içeriği görür, kaybolmaz.

    Dönen: (bloklar, sayımlar, uyarılar).
    """
    import zipfile

    try:
        zf = zipfile.ZipFile(io.BytesIO(data))
    except zipfile.BadZipFile as exc:
        raise EngineError("E_DOCX_OPEN", "DOCX arşivi açılamadı") from exc

    roots: dict[str, Any] = {}
    try:
        for name in (
            "word/document.xml",
            "word/footnotes.xml",
            "word/endnotes.xml",
            "word/comments.xml",
        ):
            blob = _docx_part(zf, name)
            if blob is not None:
                roots[name] = _xml_root(blob, name)
    finally:
        zf.close()

    if "word/document.xml" not in roots:
        raise EngineError("E_DOCX_XML", "DOCX gövdesi (document.xml) bulunamadı")

    sections: list[str] = []
    counts = {"footnotes": 0, "endnotes": 0, "comments": 0, "insertions": 0, "deletions": 0}
    authors: list[str] = []
    truncated = False

    def add(section: str) -> None:
        nonlocal truncated
        if len(sections) >= MAX_DOCX_ANNOTATIONS:
            truncated = True
            return
        sections.append(section)

    def note_author(node: Any) -> str:
        author = (node.get(f"{_W_NS}author") or "").strip()
        if author and author not in authors:
            authors.append(author)
        return author

    footnotes = roots.get("word/footnotes.xml")
    if footnotes is not None:
        for note in footnotes.iter(f"{_W_NS}footnote"):
            # -1/0 ayırıcı kayıtlardır; içerik taşımaz.
            if (note.get(f"{_W_NS}id") or "") in {"-1", "0"}:
                continue
            text = _node_text(note)
            counts["footnotes"] += 1
            if text:
                add(f"[dipnot {note.get(f'{_W_NS}id')}] {text}")

    endnotes = roots.get("word/endnotes.xml")
    if endnotes is not None:
        for note in endnotes.iter(f"{_W_NS}endnote"):
            if (note.get(f"{_W_NS}id") or "") in {"-1", "0"}:
                continue
            text = _node_text(note)
            counts["endnotes"] += 1
            if text:
                add(f"[sonnot {note.get(f'{_W_NS}id')}] {text}")

    comments = roots.get("word/comments.xml")
    if comments is not None:
        for comment in comments.iter(f"{_W_NS}comment"):
            text = _node_text(comment)
            counts["comments"] += 1
            if text:
                author = note_author(comment)
                add(f"[yorum {comment.get(f'{_W_NS}id')}{f' — {author}' if author else ''}] {text}")

    body = roots["word/document.xml"]
    for change in body.iter(f"{_W_NS}ins"):
        text = _node_text(change)
        counts["insertions"] += 1
        if text:
            author = note_author(change)
            date = (change.get(f"{_W_NS}date") or "").strip()
            label = ", ".join(part for part in (author, date) if part)
            add(f"[değişiklik ekleme{f' — {label}' if label else ''}] {text}")
    for change in body.iter(f"{_W_NS}del"):
        text = _node_text(change, "delText")
        counts["deletions"] += 1
        if text:
            author = note_author(change)
            date = (change.get(f"{_W_NS}date") or "").strip()
            label = ", ".join(part for part in (author, date) if part)
            add(f"[değişiklik silme{f' — {label}' if label else ''}] {text}")

    warnings: list[str] = []
    total = sum(counts.values())
    if total:
        summary = (
            f"DOCX envanteri: {counts['footnotes']} dipnot, {counts['endnotes']} sonnot, "
            f"{counts['comments']} yorum, {counts['insertions']} ekleme, {counts['deletions']} silme"
        )
        if authors:
            summary += " (yazarlar: " + ", ".join(authors[:5]) + ")"
        warnings.append(summary)
        warnings.append(
            "DOCX varsayılan metin: gövde paragrafları; izlenen ekleme/silme gövdeye uygulanmadı "
            "(Word görünümüyle birebir eşdeğer değil). Dipnot/sonnot/yorum ve değişiklik metni "
            "ayrı bloklardadır; silinen metin güncel belge metni sayılmaz"
        )
    if truncated:
        warnings.append(f"DOCX envanteri ilk {MAX_DOCX_ANNOTATIONS} öğe ile sınırlandı")

    return sections, counts, warnings


_DOCX_HEADING = re.compile(r"^(heading|başlık|baslik)\s*([1-9])$", re.IGNORECASE)


def _docx_heading(paragraph: Any) -> tuple[int, str] | None:
    """Paragraf başlık mı? Yerleşik stil adı/kimliği (`Heading 1`, `Başlık 2`,
    `Heading1`) tanınır. Başka stil adı başlık sayılmaz: bölüm yolu uydurulmaz.
    """
    text = (getattr(paragraph, "text", "") or "").strip()
    if not text:
        return None
    style = getattr(paragraph, "style", None)
    for attr in ("name", "style_id"):
        raw = getattr(style, attr, "") or ""
        match = _DOCX_HEADING.match(str(raw).strip())
        if match:
            return int(match.group(2)), text
    return None


def extract_docx(data: bytes) -> Extraction:
    try:
        import docx  # type: ignore
    except ImportError as exc:
        raise EngineError("E_DOCX_MISSING", "python-docx bu worker paketinde yok") from exc

    try:
        document = docx.Document(io.BytesIO(data))
    except Exception as exc:  # noqa: BLE001
        raise EngineError("E_DOCX_OPEN", "DOCX açılamadı (bozuk veya desteklenmeyen biçim)") from exc

    # Her paragraf kendi yolu ve türüyle taşınır: başlık yığını bölüm
    # bağlantısını verir, `p/N` fiziksel paragraf sırasını (türev sayfadan
    # bağımsız) verir. Stil tanınmazsa başlık uydurulmaz, yol yalnız p/N olur.
    entries: list[tuple[str, str | None, str | None]] = []
    heading_stack: list[tuple[int, str]] = []
    paragraph_no = 0
    for paragraph in document.paragraphs:
        if not paragraph.text.strip():
            continue
        paragraph_no += 1
        heading = _docx_heading(paragraph)
        if heading:
            level, title = heading
            heading_stack = [(lvl, name) for lvl, name in heading_stack if lvl < level]
            heading_stack.append((level, title))
            trail = " › ".join(name for _, name in heading_stack)
            entries.append((paragraph.text, trail, "heading"))
            continue
        trail = " › ".join(name for _, name in heading_stack)
        path = f"{trail} · p/{paragraph_no}" if trail else f"p/{paragraph_no}"
        entries.append((paragraph.text, path, "paragraph"))

    for table_no, table in enumerate(document.tables, start=1):
        for row_no, row in enumerate(table.rows, start=1):
            cells = [c.text.strip() for c in row.cells if c.text.strip()]
            if cells:
                entries.append((" | ".join(cells), f"Tablo {table_no} · satır {row_no}", "table"))

    for section in document.sections:
        for area, area_kind in ((section.header, "header"), (section.footer, "footer")):
            for p in area.paragraphs:
                if p.text.strip():
                    label = "Üstbilgi" if area_kind == "header" else "Altbilgi"
                    entries.append((p.text, label, area_kind))

    # Gövde dışındaki içerik (dipnot/sonnot/yorum/izlenen değişiklik) ayrı
    # bloklar olarak eklenir; python-docx bunları gövde metnine katmaz.
    annotations, _counts, inventory_warnings = _docx_annotations(data)
    for index, annotation in enumerate(annotations, start=1):
        entries.append((annotation, f"Ek içerik · {index}", "annotation"))

    text, geometry = _structure_text(
        [entry[0] for entry in entries],
        labels=[(entry[1], entry[2]) for entry in entries],
    )
    warnings = ["DOCX sayfa düzeni doğrulanmadı; kaynak paragraf bazlıdır", *inventory_warnings]
    if not text.strip():
        warnings.append("belgede metin bulunamadı")
    version = str(getattr(docx, "__version__", "unknown"))
    page = Page(
        index=0,
        text=text,
        warnings=warnings,
        coordinate_space=COORDINATE_SPACE_STRUCTURE,
        blocks=geometry,
    )
    return Extraction(engine="python-docx", engine_version=version, pages=[page], warnings=warnings)


def extract_udf(data: bytes) -> Extraction:
    """UYAP UDF arşivini güvenli ayrıştırır. Sayfa ayrımı doğrulanmamıştır;
    kaynak paragraf bazlıdır ve bu durum uyarı olarak taşınır."""
    try:
        document = extract_udf_document(data)
    except UdfError as exc:
        raise EngineError(exc.code, exc.message) from exc

    warnings = list(document.warnings)
    warnings.append("UDF: sayfa ayrımı doğrulanmadı; kaynak paragraf bazlıdır")
    # UDF'te doğrulanmış başlık/bölüm kaynağı yok: yol yalnız paragraf sırasıdır
    # (p/N). Bölüm adı uydurulmaz. Blok sınırları `_join_blocks` kuralıyla
    # yeniden kurulur; metin birebir aynı kalır, offsetler kaymaz.
    pieces: list[str] = []
    geometry: list[TextBlock] = []
    cursor = 0
    for index, block in enumerate(document.blocks, start=1):
        if not block:
            continue
        if cursor and pieces and not pieces[-1].endswith("\n") and not block.startswith("\n"):
            pieces.append("\n")
            cursor += 1
        geometry.append(
            TextBlock(start=cursor, end=cursor + len(block), path=f"p/{index}", kind="paragraph")
        )
        pieces.append(block)
        cursor += len(block)
    text = "".join(pieces) if pieces else document.text
    if not geometry and text:
        geometry.append(TextBlock(start=0, end=len(text), path="p/1", kind="paragraph"))
    page = Page(
        index=0,
        text=text,
        warnings=warnings,
        coordinate_space=COORDINATE_SPACE_STRUCTURE,
        blocks=geometry,
    )
    return Extraction(engine="uyap-udf", engine_version="1", pages=[page], warnings=warnings)


def _looks_like_udf(data: bytes) -> bool:
    """ZIP merkezi dizininde `content.xml` adı geçiyor mu (ucuz ön kontrol)."""
    if not data.startswith(b"PK\x03\x04"):
        return False
    return b"content.xml" in data[-131072:].lower()


def _pillow():
    try:
        from PIL import Image  # type: ignore
    except ImportError as exc:  # pragma: no cover - paket yoksa
        raise EngineError("E_IMAGE_MISSING", "Pillow bu worker paketinde yok") from exc
    return Image


def _check_pixel_budget(width: int, height: int) -> None:
    if width <= 0 or height <= 0:
        raise EngineError("E_IMAGE_INVALID", "görüntü boyutu geçersiz")
    if width * height > MAX_IMAGE_PIXELS:
        raise EngineError(
            "E_IMAGE_TOO_LARGE",
            f"görüntü {MAX_IMAGE_PIXELS // 1_000_000} megapiksel sınırını aşıyor",
        )


def extract_image(
    data: bytes,
    ocr: bool = False,
    ocr_lang: str = "eng",
    progress: Callable[[str, int, int | None], None] | None = None,
) -> Extraction:
    """PNG/JPEG/TIFF görüntüsünü sayfalara ayırır. OCR kapalıysa metin
    uydurulmaz: sayfa "metin katmanı yok" uyarısıyla boş döner. Çok kareli
    TIFF'te her kare ayrı fiziksel sayfadır; piksel bütçesi kare başına uygulanır."""
    notify = progress or (lambda stage, done, total: None)
    Image = _pillow()
    try:
        image = Image.open(io.BytesIO(data))
    except Exception as exc:  # noqa: BLE001 - bozuk görüntü normalize edilir
        raise EngineError("E_IMAGE_OPEN", "görüntü açılamadı (bozuk veya desteklenmeyen biçim)") from exc

    frames = int(getattr(image, "n_frames", 1) or 1)
    if frames > MAX_IMAGE_FRAMES:
        raise EngineError("E_IMAGE_TOO_LARGE", f"görüntü {MAX_IMAGE_FRAMES} kare sınırını aşıyor")
    if ocr and not _tesseract_available():
        raise EngineError("E_OCR_MISSING", "Tesseract bulunamadı; OCR çalıştırılamadı")

    pages: list[Page] = []
    warnings: list[str] = []
    notify("open", 0, frames)
    try:
        for index in range(frames):
            image.seek(index)
            frame = image.convert("RGB")
            _check_pixel_budget(*frame.size)
            page_warnings: list[str] = []
            text = ""
            blocks: list[TextBlock] = []
            if ocr:
                text, confidence, blocks = _ocr_text_confidence_and_blocks(frame, ocr_lang, scale=1.0)
                if text.strip():
                    page_warnings.append(
                        f"sayfa {index + 1}: OCR ile okundu ({ocr_lang}){_ocr_confidence_note(confidence)}"
                    )
                else:
                    page_warnings.append(f"sayfa {index + 1}: OCR sonuç vermedi")
            else:
                page_warnings.append(f"sayfa {index + 1}: metin katmanı yok (OCR gerekli)")
            warnings.extend(page_warnings)
            pages.append(
                Page(
                    index=index,
                    text=text,
                    width=float(frame.size[0]),
                    height=float(frame.size[1]),
                    warnings=page_warnings,
                    coordinate_space=COORDINATE_SPACE_IMAGE,
                    blocks=blocks,
                )
            )
            notify("pages", index + 1, frames)
    finally:
        image.close()

    warnings.append("görüntü koordinatları piksel cinsindendir; basılı sayfa ölçüsü değildir")
    version = "1"
    if ocr:
        try:
            import pytesseract  # type: ignore

            version = str(pytesseract.get_tesseract_version())
        except Exception:  # noqa: BLE001
            version = "unknown"
    engine = "tesseract-image" if ocr else "image"
    return Extraction(engine=engine, engine_version=version, pages=pages, warnings=warnings)


def _looks_like_image(data: bytes) -> bool:
    return (
        data.startswith(b"\x89PNG\r\n\x1a\n")
        or data.startswith(b"\xff\xd8\xff")
        or data.startswith(b"II*\x00")
        or data.startswith(b"MM\x00*")
    )


#: Otomatik kodlama seçiminde denenen adaylar. Hepsi tek baytlıktır; seçim
#: "çözüldü mü" ile değil, çıktının bozulma puanıyla yapılır (tek baytlık her
#: kodlama her baytı çözer, bu yüzden sessiz mojibake riski gerçektir).
SINGLE_BYTE_CANDIDATES: tuple[tuple[str, str], ...] = (
    ("windows-1254", "Windows-1254 (Türkçe)"),
    ("iso-8859-9", "ISO-8859-9 (Latin-5)"),
    ("windows-1252", "Windows-1252"),
    ("iso-8859-1", "ISO-8859-1 (Latin-1)"),
)

#: Bozuk çözümün işaretleri: UTF-8'in Latin-1 gibi okunmasından kalan çiftler
#: ve C1 kontrol karakterleri. Türkçe metinde bunlar geçerli değildir.
_MOJIBAKE_LEADS = "ÃÂÄÅÐÞ"


def _mojibake_score(text: str) -> int:
    score = 0
    for char in text:
        if char in _MOJIBAKE_LEADS:
            score += 3
        elif unicodedata.category(char) == "Cc" and char not in "\t\n\r\f":
            score += 5
    return score


def _looks_binary(data: bytes) -> bool:
    """NUL veya yoğun kontrol baytı içeren içerik metin sayılmaz.

    UTF-16 BOM'u çağıran tarafta ayrıca ele alınır; buraya BOM'suz veri gelir.
    """
    sample = data[:8192]
    if not sample:
        return False
    if b"\x00" in sample:
        return True
    controls = sum(1 for byte in sample if byte < 0x09 or 0x0E <= byte < 0x20 or byte == 0x7F)
    return controls / len(sample) > 0.02


def decode_text(data: bytes, encoding: str = "auto") -> tuple[str, str, list[str]]:
    """Metni çözer; `(metin, kullanılan kodlama, uyarılar)` döndürür.

    `encoding="auto"` sırası: BOM (UTF-32/UTF-16/UTF-8) → katı UTF-8 → tek
    baytlık adaylar arasında en düşük bozulma puanı. Hangi kodlamanın
    kullanıldığı çağırana bildirilir; hiçbir varsayım sessiz değildir.
    """
    requested = (encoding or "auto").strip().lower()
    if requested in {"auto", "otomatik", "detect"}:
        return _decode_auto(data)

    try:
        decoded = data.decode(requested)
    except LookupError as exc:
        raise EngineError("E_TEXT_ENCODING", f"bilinmeyen kodlama: {encoding}") from exc
    except UnicodeDecodeError as exc:
        raise EngineError(
            "E_TEXT_ENCODING",
            f"metin {requested} olarak çözülemedi (bayt {exc.start}); kodlamayı elle seçin",
        ) from exc
    return decoded, requested, []


def _decode_auto(data: bytes) -> tuple[str, str, list[str]]:
    if data.startswith(codecs.BOM_UTF32_LE) or data.startswith(codecs.BOM_UTF32_BE):
        return data.decode("utf-32"), "utf-32", ["metin kodlaması BOM'dan belirlendi: utf-32"]
    if data.startswith(codecs.BOM_UTF8):
        return (
            data.decode("utf-8-sig"),
            "utf-8-sig",
            ["metin kodlaması BOM'dan belirlendi: utf-8"],
        )
    if data.startswith(codecs.BOM_UTF16_LE) or data.startswith(codecs.BOM_UTF16_BE):
        return data.decode("utf-16"), "utf-16", ["metin kodlaması BOM'dan belirlendi: utf-16"]

    if _looks_binary(data):
        raise EngineError(
            "E_TEXT_ENCODING",
            "içerik metin gibi görünmüyor (ikili veri); kodlama denetlenmedi",
        )

    try:
        return data.decode("utf-8"), "utf-8", []
    except UnicodeDecodeError:
        pass

    best: tuple[int, str, str, str] | None = None
    for codec_name, label in SINGLE_BYTE_CANDIDATES:
        try:
            candidate = data.decode(codec_name)
        except UnicodeDecodeError:
            continue
        score = _mojibake_score(candidate)
        if best is None or score < best[0]:
            best = (score, codec_name, label, candidate)

    if best is None:
        raise EngineError(
            "E_TEXT_ENCODING",
            "metin kodlaması belirlenemedi; kodlamayı elle seçin (denenenler: "
            + ", ".join([codec for codec, _ in SINGLE_BYTE_CANDIDATES])
            + ")",
        )

    score, codec_name, label, text = best
    warning = f"metin kodlaması otomatik belirlendi: {label}"
    if score > 0:
        warning += " (bozuk karakter olasılığı var; yanlışsa kodlamayı elle seçin)"
    return text, codec_name, [warning]


def extract_plain(data: bytes, encoding: str = "auto") -> Extraction:
    text, _used, warnings = decode_text(data, encoding)
    if not text.strip():
        warnings.append("belgede metin yok")
    geometry: list[TextBlock] = [TextBlock(start=0, end=len(text))] if text else []
    page = Page(
        index=0,
        text=text,
        warnings=warnings,
        coordinate_space=COORDINATE_SPACE_STRUCTURE,
        blocks=geometry,
    )
    result = Extraction(engine="plain", engine_version="2", pages=[page], warnings=warnings)
    # Sürüm 2: kodlama otomatik belirlenir (BOM → UTF-8 → Türkçe tek baytlık
    # adaylar). Hangi kodlamanın seçildiği `warnings` içinde açıkça yazılır.
    return result


def extract(
    data: bytes,
    mime: str,
    ocr: bool = False,
    ocr_lang: str = "eng",
    encoding: str = "auto",
    password: str | None = None,
    progress: Callable[[str, int, int | None], None] | None = None,
) -> Extraction:
    """MIME ve içerik imzasına göre doğru motoru seçer. Parola yalnız bu çağrı
    için kullanılır; hiçbir yere yazılmaz. `progress` sayfa bazlı ilerleme
    bildirimidir (F04-T07); çok sayfalı motorlarda doldurulur."""
    if mime == "application/pdf" or data.startswith(b"%PDF-"):
        return extract_pdf(
            data, ocr=ocr, ocr_lang=ocr_lang, password=password, progress=progress
        )
    if mime in {"image/png", "image/jpeg", "image/tiff", "image/tif"} or _looks_like_image(data):
        return extract_image(data, ocr=ocr, ocr_lang=ocr_lang, progress=progress)
    if mime in {"application/udf", "application/x-udf", "application/x-uyap-udf"}:
        return extract_udf(data)
    if mime in {"application/zip", "application/octet-stream"} and _looks_like_udf(data):
        return extract_udf(data)
    if "wordprocessingml.document" in mime or (data.startswith(b"PK\x03\x04") and mime.endswith("document")):
        return extract_docx(data)
    if mime.startswith("text/") or mime in {"application/json", "text/csv"}:
        return extract_plain(data, encoding=encoding)
    raise EngineError("E_MIME_UNSUPPORTED", f"desteklenmeyen MIME: {mime}")


def _tessdata_dir() -> str | None:
    """Tesseract 5 `--print-tessdata-dir` verir; Tesseract 4 vermez. İkinci yol
    `--list-langs` başlığındaki dizindir. Bulunamazsa `None` döner."""
    try:
        out = subprocess.run(
            ["tesseract", "--print-tessdata-dir"],
            capture_output=True,
            text=True,
            timeout=5,
            check=False,
        )
        if out.returncode == 0 and (out.stdout or "").strip():
            return out.stdout.strip()
    except (OSError, subprocess.SubprocessError):
        pass
    try:
        listed = subprocess.run(
            ["tesseract", "--list-langs"],
            capture_output=True,
            text=True,
            timeout=5,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    import re

    match = re.search(r"in (.+?) \(\d+\)", listed.stderr or listed.stdout or "")
    if match:
        # Tesseract 4 dizini tırnak içinde yazar: in "/path/tessdata/" (2)
        return match.group(1).strip().strip('"')
    prefix = os.environ.get("TESSDATA_PREFIX")
    return prefix.strip() if prefix else None


def tessdata_fingerprint() -> str | None:
    """Kurulu `*.traineddata` dosyalarının birleşik sha256'sı. Dil paketi
    değişince ayrıştırma ayarı da değişmiş sayılır; uydurma sürüm yazılmaz."""
    directory = _tessdata_dir()
    if not directory:
        return None
    import glob
    import hashlib

    files = sorted(glob.glob(os.path.join(directory, "*.traineddata")))
    if not files:
        return None
    digest = hashlib.sha256()
    for path in files:
        digest.update(os.path.basename(path).encode("utf-8"))
        with open(path, "rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    return digest.hexdigest()


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

    image_ok = False
    image_version = None
    try:
        import PIL  # type: ignore

        image_ok = True
        image_version = str(PIL.__version__)
    except ImportError:
        pass

    return {
        "protocolVersion": 1,
        "ocr": {"langs": ocr_langs, "tessdataHash": tessdata_fingerprint()},
        "engines": {
            "pdf": {"available": pdf, "version": pdf_version},
            "docx": {"available": docx_ok, "version": docx_version},
            "udf": {"available": True, "version": "1"},
            "image": {
                "available": image_ok,
                "version": image_version,
                "maxPixels": MAX_IMAGE_PIXELS,
                "maxFrames": MAX_IMAGE_FRAMES,
            },
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
    ocr_verified: bool = False
    engines: list[str] = field(default_factory=list)
    # Bağımsız doğrulama sayıları (F01-T05). Arayüz bunları olduğu gibi gösterir;
    # "doğrulandı" iddiası bu sayılarla birlikte okunur.
    streams_scanned: int = 0
    streams_binary: int = 0
    streams_unreadable: int = 0
    annotations_removed: int = 0
    offpage_spans_removed: int = 0
    scripts_removed: int = 0
    warnings: list[str] = field(default_factory=list)


# --- Karartma doğrulaması ve gizli nesne temizliği (F01-T05) ---------------
#
# Kararı veren kontroller PyMuPDF'e dayanmaz: ham bayt taraması, Flate
# akışlarının kendi açıcımızla açılması ve pypdf okuması. PyMuPDF yalnız
# üretici olarak kendi çıktısını ikinci kez okur; tek başına kanıt sayılmaz.
# Bir doğrulayıcı çalışamazsa "temiz" denmez: ya `ran=False` döner (motor
# listesine girmez) ya da fail-closed olarak terimleri sızdırılmış sayar.

MAX_SCAN_STREAMS = 20000
_STREAM_START = re.compile(rb"(?<!end)stream\r\n|(?<!end)stream\n|(?<!end)stream\r")
_XREF_REF = re.compile(rb"(\d+)\s+\d+\s+R")
# Görüntü/font akışları Flate ile açılmaz; ham bayt taraması onları kapsar,
# pikselleri ayrıca OCR denetler. Bunları "okunamadı" saymak yanıltıcı olur.
_BINARY_STREAM_MARKERS = (
    b"/Image",
    b"/DCTDecode",
    b"/JPXDecode",
    b"/CCITTFaxDecode",
    b"/JBIG2Decode",
    b"/FontFile",
    b"/Subtype /Type1C",
)


def _term_variants(term: str) -> list[bytes]:
    """Terimin PDF içinde görünebileceği kodlamalar.

    PDF dize nesneleri üç biçimde yazılır ve **hepsi** aranmalıdır:
    düz kaçışlı dize (`\\(`, `\\`), UTF-16BE onaltılık (`<FEFF...>`) ve
    UTF-8 baytlarının onaltılığı (`<5a5a...>`). PyMuPDF metni onaltılık
    yazdığı için yalnız düz arama yapan tarayıcı sayfa metnini hiç görmez
    (F01-T05'te ölçüldü: tarayıcı "temiz" diyordu, metin dosyadaydı).
    """
    raw = term.encode("utf-8")
    escaped = raw.replace(b"\\", b"\\\\").replace(b"(", b"\\(").replace(b")", b"\\)")
    utf16 = binascii.hexlify(term.encode("utf-16-be"))
    utf8 = binascii.hexlify(raw)
    variants = [
        raw,
        escaped,
        b"<" + utf8 + b">",
        b"<" + utf8.upper() + b">",
        b"<FEFF" + utf16.upper() + b">",
        b"<feff" + utf16 + b">",
    ]
    if len(term) >= 6:
        # Sınırlayıcısız onaltılık: dize `<` `>` arasında satır sonuyla
        # bölünmüş olabilir. Kısa terimlerde rastgele eşleşme riski yüksek.
        variants.extend([utf8, utf8.upper()])
    unique: list[bytes] = []
    for item in variants:
        if item and item not in unique:
            unique.append(item)
    return unique


def _inflate_streams(data: bytes) -> tuple[list[bytes], int, int]:
    """`stream…endstream` gövdelerini Flate ile açar.

    Dönen: (açılan gövdeler, ikili akış sayısı, açılamayan akış sayısı).
    Hiçbir PDF kütüphanesi kullanılmaz; sıkıştırma katmanı doğrudan açılır ki
    "metin katmanında yok" ile "dosyada yok" ayrımı ölçülebilsin.

    Sıkıştırmasız (filtresiz) akışlar "açılamadı" sayılmaz: ham bayt taraması
    onları zaten kapsar. Görüntü/font akışları ikili sayılır; pikselleri OCR
    denetler, ham baytları da taranır.
    """
    opened: list[bytes] = []
    binary = 0
    unreadable = 0
    for match in _STREAM_START.finditer(data):
        start = match.end()
        end = data.find(b"endstream", start)
        if end < 0:
            continue
        body = data[start:end].strip(b"\r\n")
        if not body:
            continue
        try:
            opened.append(zlib.decompress(body))
            continue
        except zlib.error:
            pass
        try:
            opened.append(zlib.decompressobj().decompress(body))
            continue
        except zlib.error:
            pass
        header = data[max(0, match.start() - 400) : match.start()]
        if b"/Filter" not in header:
            continue  # sıkıştırmasız: ham bayt taraması kapsıyor
        if any(marker in header for marker in _BINARY_STREAM_MARKERS):
            binary += 1
        else:
            unreadable += 1
        if len(opened) + binary + unreadable >= MAX_SCAN_STREAMS:
            break
    return opened, binary, unreadable


def _scan_terms(data: bytes, terms: list[str]) -> tuple[list[str], int, int, int]:
    """Terimleri ham baytlarda ve açılmış akışlarda arar.

    Dönen: (bulunan terimler, açılan akış, ikili akış, açılamayan akış).
    """
    variants = {term: _term_variants(term) for term in terms}
    found: set[str] = set()
    for term, options in variants.items():
        if any(option in data for option in options):
            found.add(term)
    streams, binary, unreadable = _inflate_streams(data)
    for body in streams:
        for term, options in variants.items():
            if term in found:
                continue
            if any(option in body for option in options):
                found.add(term)
    return sorted(found), len(streams), binary, unreadable


def _pypdf_module() -> Any:
    try:
        import pypdf  # type: ignore

        return pypdf
    except ImportError:
        return None


def _verify_with_pypdf(data: bytes, terms: list[str]) -> tuple[list[str], bool]:
    """Bağımsız ikinci okuyucu. Kurulu değilse `ran=False` döner ve motor
    listesine **girmez**; "pypdf doğruladı" yalanı yazılmaz."""
    pypdf = _pypdf_module()
    if pypdf is None:
        return [], False
    try:
        reader = pypdf.PdfReader(io.BytesIO(data))
        found: list[str] = []
        for page in reader.pages:
            text = page.extract_text() or ""
            for term in terms:
                if term in text:
                    found.append(term)
        return sorted(set(found)), True
    except Exception:  # noqa: BLE001 - doğrulayıcı çöktüyse sonuç "temiz" sayılmaz
        return list(terms), True


def _verify_with_ocr(data: bytes, terms: list[str]) -> tuple[list[str], bool]:
    """Sayfa görüntüsünü OCR ile okur. Tesseract yoksa `False` döner; bu durum
    'görüntü doğrulandı' diye sunulmaz."""
    try:
        import pytesseract  # type: ignore
        from PIL import Image  # type: ignore
    except ImportError:
        return [], False
    import shutil

    if shutil.which("tesseract") is None:
        return [], False
    pymupdf = _pymupdf()
    found: list[str] = []
    try:
        doc = pymupdf.open(stream=data, filetype="pdf")
        try:
            for page in doc:
                pix = page.get_pixmap(dpi=200)
                image = Image.open(io.BytesIO(pix.tobytes("png")))
                text = pytesseract.image_to_string(image, lang="tur+eng")
                for term in terms:
                    if term in text:
                        found.append(term)
        finally:
            doc.close()
    except Exception:  # noqa: BLE001
        return list(terms), True
    return found, True


def _annotation_blobs(doc: Any, xref: int) -> list[bytes]:
    """Annotation nesnesi ve ona bağlı nesnelerin **ham** baytları.

    Görünüm akışı (`/AP`) sıkıştırılmış olabilir; hem ham hem açılmış hâli
    taranır. Amaç metni bulmak, çözmek değil."""
    blobs: list[bytes] = []
    try:
        obj = doc.xref_object(xref, compressed=False)
    except Exception:  # noqa: BLE001 - bozuk xref taramayı durdurmaz
        return blobs
    encoded = obj.encode("utf-8", "surrogateescape")
    blobs.append(encoded)
    for match in _XREF_REF.finditer(encoded):
        target = int(match.group(1))
        if target == xref:
            continue
        try:
            stream = doc.xref_stream(target)
        except Exception:  # noqa: BLE001
            stream = None
        if stream:
            blobs.append(stream)
        try:
            blobs.append(doc.xref_object(target, compressed=False).encode("utf-8", "surrogateescape"))
        except Exception:  # noqa: BLE001
            continue
    return blobs


def _strip_leaking_annotations(doc: Any, terms: list[str]) -> int:
    """Metni karartılan terimi taşıyan açıklamaları siler.

    Annotation metni sayfa metnine girmez: `search_for` onu bulmaz, `scrub` da
    `/Contents` alanını temizlemez. Silinmezse dosya "temiz" görünürken
    yapışkan notta / yazar alanında terim kalır (F01-T05'te ölçüldü).
    Kısmi temizlik denenmez; terimi taşıyan açıklama bütünüyle kaldırılır.
    """
    removed = 0
    for page in doc:
        try:
            annots = list(page.annots() or [])
        except Exception:  # noqa: BLE001 - bozuk /Annots sayfayı atlatır
            continue
        for annot in annots:
            try:
                if annot.type[1] == "Redact":
                    continue
                joined = b"\n".join(_annotation_blobs(doc, annot.xref))
            except Exception:  # noqa: BLE001
                continue
            if not any(any(v in joined for v in _term_variants(t)) for t in terms):
                continue
            try:
                page.delete_annot(annot)
                removed += 1
            except Exception:  # noqa: BLE001
                continue
    return removed


def _strip_offpage_text(doc: Any, pymupdf: Any) -> int:
    """Sayfa kutusunun **tamamen** dışındaki metni kaldırır.

    Böyle metin görünmez ama metin olarak çıkarılır; dışa aktarılan kopyada
    kalırsa sızıntıdır. Sınır olarak `cropbox` kullanılır: `get_texttrace`
    kutuları döndürme öncesi uzaydadır (döndürülmüş sayfada `rect` yanıltır).
    Kutuyu kesen metne dokunulmaz — görünen içerik silinmez.
    """
    removed = 0
    for page in doc:
        try:
            spans = page.get_texttrace()
        except Exception:  # noqa: BLE001
            continue
        page_box = page.cropbox
        rects = []
        for span in spans:
            bbox = pymupdf.Rect(span.get("bbox") or (0, 0, 0, 0))
            if bbox.is_empty or bbox.is_infinite or bbox.intersects(page_box):
                continue
            rects.append(bbox)
        if not rects:
            continue
        for rect in rects:
            page.add_redact_annot(rect, fill=None)
        page.apply_redactions(images=pymupdf.PDF_REDACT_IMAGE_PIXELS)
        removed += len(rects)
    return removed


def _is_javascript_key(doc: Any, kind: str, value: str) -> bool:
    if kind == "xref":
        try:
            target = int(value.split()[0])
            return doc.xref_get_key(target, "S")[1] == "/JavaScript"
        except (ValueError, IndexError):
            return False
    return "/JavaScript" in value


def _strip_document_scripts(doc: Any) -> int:
    """Katalog/sayfa içine gömülü JavaScript eylemlerini kaldırır.

    `scrub(javascript=True)` yalnız **ayrı nesne** olan `/S /JavaScript`
    eylemini siler; `/OpenAction` içine gömülü sözlük gözden kaçar (ölçüldü).
    Yalnız JavaScript taşıyan anahtarlar kaldırılır; gezinme eylemi korunur.
    """
    removed = 0
    targets = [doc.pdf_catalog()] + [page.xref for page in doc]
    for xref in targets:
        for key in ("OpenAction", "AA"):
            try:
                kind, value = doc.xref_get_key(xref, key)
            except Exception:  # noqa: BLE001
                continue
            if kind == "null" or not _is_javascript_key(doc, kind, value):
                continue
            doc.xref_set_key(xref, key, "null")
            removed += 1
    try:
        kind, _value = doc.xref_get_key(doc.pdf_catalog(), "Names/JavaScript")
        if kind != "null":
            doc.xref_set_key(doc.pdf_catalog(), "Names/JavaScript", "null")
            removed += 1
    except Exception:  # noqa: BLE001
        pass
    return removed


def _ocr_region_words(pymupdf: Any, page: Any, rect: list[float]) -> list[str]:
    """Bölgenin görüntüsündeki kelimeler (metin katmanı yoksa)."""
    try:
        import pytesseract  # type: ignore
        from PIL import Image  # type: ignore
    except ImportError:
        return []
    if shutil.which("tesseract") is None:
        return []
    try:
        pix = page.get_pixmap(dpi=200, clip=pymupdf.Rect(*rect))
        image = Image.open(io.BytesIO(pix.tobytes("png")))
        text = pytesseract.image_to_string(image, lang="tur+eng")
    except Exception:  # noqa: BLE001
        return []
    return re.findall(r"[0-9A-Za-zÇĞİÖŞÜçğıöşü]{4,}", text)


def _region_probe_terms(
    doc: Any, regions: list[tuple[int, Any, list[float]]], pymupdf: Any
) -> tuple[list[str], list[str]]:
    """Bölge karartmasında doğrulanacak kelimeler: (metin katmanı, OCR).

    Bölgede aranacak "terim" yoktur; bölgedeki metnin kendisi kullanılır.
    Metin katmanı kelimelerinde yalnızca **sadece** karartılan bölgelerde
    geçenler seçilir (belgenin başka yerindeki aynı kelime yanlış sızıntı
    sayılmasın). Metin katmanı yoksa (taranmış sayfa) kelimeler OCR'dan gelir
    ve yalnız bölgenin görüntüsünde doğrulanır.
    """

    def words(text: str) -> list[str]:
        return re.findall(r"[0-9A-Za-zÇĞİÖŞÜçğıöşü]{4,}", text)

    whole: Counter[str] = Counter()
    for page in doc:
        whole.update(words(page.get_text("text") or ""))
    inside: Counter[str] = Counter()
    for page_index, rect, _cleaned in regions:
        inside.update(words(doc.load_page(page_index).get_text("text", clip=rect) or ""))
    text_probes = sorted({word for word, count in inside.items() if whole.get(word, 0) <= count})
    ocr_probes: list[str] = []
    if not text_probes:
        for page_index, _rect, cleaned in regions[:10]:
            page = doc.load_page(page_index)
            ocr_probes.extend(_ocr_region_words(pymupdf, page, cleaned))
        ocr_probes = sorted(set(ocr_probes))
    return text_probes[:20], ocr_probes[:20]


def _region_ocr_leaks(
    data: bytes, regions: list[tuple[int, Any, list[float]]], probes: list[str]
) -> tuple[list[str], bool]:
    """Karartılan bölgelerin **görüntüsünü** OCR ile okur.

    Bölge karartması pikselleri siler; metin katmanı temiz görünse bile
    görüntüde kalan yazı sızıntıdır. Tesseract yoksa `ran=False`."""
    if not probes:
        return [], False
    try:
        import pytesseract  # type: ignore
        from PIL import Image  # type: ignore
    except ImportError:
        return [], False
    if shutil.which("tesseract") is None:
        return [], False
    pymupdf = _pymupdf()
    found: set[str] = set()
    try:
        doc = pymupdf.open(stream=data, filetype="pdf")
        try:
            for page_index, _rect, cleaned in regions[:20]:
                page = doc.load_page(page_index)
                pix = page.get_pixmap(dpi=200, clip=pymupdf.Rect(*cleaned))
                image = Image.open(io.BytesIO(pix.tobytes("png")))
                text = pytesseract.image_to_string(image, lang="tur+eng")
                for probe in probes:
                    if probe in text:
                        found.add(probe)
        finally:
            doc.close()
    except Exception:  # noqa: BLE001 - OCR çöktüyse bölge "temiz" sayılmaz
        return list(probes), True
    return sorted(found), True


def _verification_warnings(
    *,
    scripts_removed: int,
    annotations_removed: int,
    offpage_spans_removed: int,
    streams_unreadable: int,
) -> list[str]:
    notes: list[str] = []
    if scripts_removed:
        notes.append(f"{scripts_removed} gömülü JavaScript eylemi kaldırıldı")
    if annotations_removed:
        notes.append(f"{annotations_removed} açıklama silindi: karartılan terimi taşıyordu")
    if offpage_spans_removed:
        notes.append(f"{offpage_spans_removed} sayfa dışı metin parçası kaldırıldı")
    if streams_unreadable:
        notes.append(f"{streams_unreadable} akış açılamadı; bu akışlarda bayt araması yapılmadı")
    return notes


def redact_pdf_regions(
    data: bytes,
    regions: list[dict[str, Any]],
    fill: tuple[float, float, float] = (0.0, 0.0, 0.0),
) -> RedactionResult:
    """Kullanıcının seçtiği dikdörtgenleri gerçekten kaldırır.

    `regions`: `{"page": int, "rect": [x0,y0,x1,y1]}` — punto, **döndürme öncesi
    cropbox** uzayı. Metin katmanı ve görüntü pikselleri birlikte silinir; ardından
    her bölge yeniden okunur ve içinde metin kaldıysa sonuç reddedilir.
    """
    pymupdf = _pymupdf()
    if not regions:
        raise EngineError("E_REDACT_REGIONS", "maskelenecek bölge yok")

    try:
        doc = pymupdf.open(stream=data, filetype="pdf")
    except Exception as exc:  # noqa: BLE001
        raise EngineError("E_PDF_OPEN", "PDF açılamadı (bozuk veya parola korumalı)") from exc
    if doc.needs_pass:
        raise EngineError("E_PDF_PASSWORD", "belge parola korumalı; önce parolayla açın")

    parsed: list[tuple[int, Any, list[float]]] = []
    for item in regions:
        try:
            page_index = int(item["page"])
            raw = [float(v) for v in item["rect"]]
        except (KeyError, TypeError, ValueError):
            raise EngineError("E_REDACT_REGIONS", "bölge biçimi geçersiz") from None
        if page_index < 0 or page_index >= doc.page_count:
            raise EngineError("E_REDACT_REGIONS", f"sayfa aralık dışı: {page_index}")
        if len(raw) != 4:
            raise EngineError("E_REDACT_REGIONS", "rect dört değer olmalı")
        cleaned = [min(raw[0], raw[2]), min(raw[1], raw[3]), max(raw[0], raw[2]), max(raw[1], raw[3])]
        if cleaned[2] - cleaned[0] < 1.0 or cleaned[3] - cleaned[1] < 1.0:
            raise EngineError("E_REDACT_REGIONS", "bölge çok küçük")
        parsed.append((page_index, pymupdf.Rect(*cleaned), cleaned))

    hits: list[RedactionHit] = []
    ocr_probes: list[str] = []
    try:
        probes, ocr_probes = _region_probe_terms(doc, parsed, pymupdf)
        for page_index, rect, cleaned in parsed:
            page = doc.load_page(page_index)
            page.add_redact_annot(rect, fill=fill)
            hits.append(
                RedactionHit(
                    page=page_index,
                    term="(seçili bölge)",
                    rect=cleaned,
                )
            )
        for page_index in sorted({item[0] for item in parsed}):
            doc.load_page(page_index).apply_redactions(images=pymupdf.PDF_REDACT_IMAGE_PIXELS)
        offpage_spans_removed = _strip_offpage_text(doc, pymupdf)
        scripts_removed = _strip_document_scripts(doc)
        # `reset_responses=False`: PyMuPDF 1.24.14'te `delete_responses`, yanıtı
        # olmayan her annotation'da `None.m_internal` ile çöküyor; yanıt
        # zincirleri zaten terim taramasıyla silinir.
        doc.scrub(
            metadata=True,
            xml_metadata=True,
            attached_files=True,
            hidden_text=True,
            reset_responses=False,
        )
        out = doc.tobytes(garbage=4, deflate=True)
    finally:
        doc.close()

    check = pymupdf.open(stream=out, filetype="pdf")
    remaining: list[str] = []
    try:
        for page_index, _rect, cleaned in parsed:
            page = check.load_page(page_index)
            leftover = (page.get_text("text", clip=pymupdf.Rect(*cleaned)) or "").strip()
            if leftover:
                remaining.append(f"sayfa {page_index + 1}: bölgede metin kaldı")
    finally:
        check.close()

    # Bölgede ne olduğu bilinmediği için doğrulama bölgedeki kelimelerle yapılır.
    leaked, streams_scanned, streams_binary, streams_unreadable = _scan_terms(out, probes)
    remaining.extend(leaked)
    pypdf_found, pypdf_ran = _verify_with_pypdf(out, probes)
    remaining.extend(pypdf_found)
    verify_words = sorted(set(probes) | set(ocr_probes))
    region_ocr_found, region_ocr_ran = _region_ocr_leaks(out, parsed, verify_words)
    remaining.extend(region_ocr_found)

    engines = ["pymupdf", "raw-scan"]
    if pypdf_ran:
        engines.append("pypdf")
    if region_ocr_ran:
        engines.append("tesseract")
    warnings = _verification_warnings(
        scripts_removed=scripts_removed,
        annotations_removed=0,
        offpage_spans_removed=offpage_spans_removed,
        streams_unreadable=streams_unreadable,
    )
    if not pypdf_ran:
        warnings.append("pypdf kurulu değil: ikinci okuyucu doğrulaması yapılmadı")
    if ocr_probes and not probes:
        warnings.append("bölgede metin katmanı yoktu; doğrulama bölge görüntüsünün OCR'ıyla yapıldı")
    if not verify_words:
        warnings.append("bölgede okunabilir metin yok: karşılaştırılacak kelime bulunamadı")

    version = str(getattr(pymupdf, "__version__", "unknown"))
    return RedactionResult(
        pdf=out,
        hits=hits,
        remaining=sorted(set(remaining)),
        engine="pymupdf",
        engine_version=version,
        ocr_verified=region_ocr_ran,
        engines=engines,
        streams_scanned=streams_scanned,
        streams_binary=streams_binary,
        streams_unreadable=streams_unreadable,
        annotations_removed=0,
        offpage_spans_removed=offpage_spans_removed,
        scripts_removed=scripts_removed,
        warnings=warnings,
    )


def redact_pdf(data: bytes, terms: list[str], fill: tuple[float, float, float] = (0.0, 0.0, 0.0)) -> RedactionResult:
    """Terimleri PDF'in **metin katmanından ve görüntüsünden** gerçekten çıkarır.

    Yalnız siyah dikdörtgen çizmek maskeleme sayılmaz: `apply_redactions`
    içeriği kaldırır, ardından çıktı yeniden açılıp terimlerin kalmadığı
    bağımsız olarak doğrulanır. Doğrulama başarısızsa sonuç kullanılmaz.

    Gizli nesneler de temizlenir: terimi taşıyan açıklamalar (yapışkan not,
    yazar alanı, görünüm akışı) silinir, sayfa dışı metin ve gömülü JavaScript
    kaldırılır. Bunlar metin katmanında görünmediği için `search_for` ile
    bulunamaz; temizlenmezlerse dosyada kalırlar (F01-T05'te ölçüldü).
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
        annotations_removed = _strip_leaking_annotations(doc, cleaned)
        scripts_removed = _strip_document_scripts(doc)
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
        offpage_spans_removed = _strip_offpage_text(doc, pymupdf)
        # `reset_responses=False`: PyMuPDF 1.24.14'te `delete_responses`, yanıtı
        # olmayan her annotation'da `None.m_internal` ile çöküyor (karartmayı
        # tamamen engelliyordu). Yanıt zincirleri yukarıda terim taramasıyla
        # silindi; kalan yanıtlar terimi taşımıyor.
        doc.scrub(
            metadata=True,
            xml_metadata=True,
            attached_files=True,
            hidden_text=True,
            reset_responses=False,
        )
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

    # Ham bayt taraması: 3 karakterden kısa terimler rastgele baytlarla
    # eşleşebileceği için yalnız metin katmanında doğrulanır (uyarıda yazılır).
    scan_terms = [t for t in cleaned if len(t) >= 4]
    leaked, streams_scanned, streams_binary, streams_unreadable = _scan_terms(out, scan_terms)
    remaining.extend(leaked)
    pypdf_found, pypdf_ran = _verify_with_pypdf(out, cleaned)
    remaining.extend(pypdf_found)
    ocr_found, ocr_verified = _verify_with_ocr(out, cleaned)
    remaining.extend(ocr_found)

    engines = ["pymupdf", "raw-scan"]
    if pypdf_ran:
        engines.append("pypdf")
    if ocr_verified:
        engines.append("tesseract")

    warnings = _verification_warnings(
        scripts_removed=scripts_removed,
        annotations_removed=annotations_removed,
        offpage_spans_removed=offpage_spans_removed,
        streams_unreadable=streams_unreadable,
    )
    if not pypdf_ran:
        warnings.append("pypdf kurulu değil: ikinci okuyucu doğrulaması yapılmadı")
    if len(scan_terms) < len(cleaned):
        warnings.append("3 karakterden kısa terimler yalnız metin katmanında doğrulandı")

    version = str(getattr(pymupdf, "__version__", "unknown"))
    return RedactionResult(
        pdf=out,
        hits=hits,
        remaining=sorted(set(remaining)),
        engine="pymupdf",
        engine_version=version,
        ocr_verified=ocr_verified,
        engines=engines,
        streams_scanned=streams_scanned,
        streams_binary=streams_binary,
        streams_unreadable=streams_unreadable,
        annotations_removed=annotations_removed,
        offpage_spans_removed=offpage_spans_removed,
        scripts_removed=scripts_removed,
        warnings=warnings,
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
