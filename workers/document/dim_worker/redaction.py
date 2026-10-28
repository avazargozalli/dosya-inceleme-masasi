"""PDF redaksiyonu: terim ve bölge ile gerçek içerik kaldırma + bağımsız doğrulama.

`engines` modülünden ayrıldı. Ağır bağımlılıklar tembel yüklenir; testlerin
`engines.redaction` üzerinden patch'lemesi gerekir.
"""

from __future__ import annotations

import binascii
import codecs
import io
import re
import shutil
import unicodedata
import zlib
from collections import Counter
from dataclasses import dataclass, field
from typing import Any

# `engines` sonunda bu modülü import eder; `EngineError` o an tanımlıdır.
from .engines import EngineError


def _pymupdf():
    from .engines import _pymupdf as loader

    return loader()


def _tesseract_available() -> bool:
    from .engines import _tesseract_available as probe

    return probe()


def _ocr_text_confidence_and_blocks(image, lang, scale):
    from .engines import _ocr_text_confidence_and_blocks as run

    return run(image, lang, scale)


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


def _verify_with_pypdf(
    data: bytes, terms: list[str]
) -> tuple[list[str], bool, str | None]:
    """Bağımsız ikinci okuyucu.

    Dönen üçlü: `(bulunan, çalıştı_mı, hata)`. Kurulu değilse `ran=False`,
    `hata=None` (motorda "pypdf kurulu değil" uyarısı yazılır). **Çökme sızıntı
    değildir**: eskiden `list(terms)` dönüyordu, yani doğrulayıcı hatası
    "terim hâlâ var" diye raporlanıp maskeli dosya kaydettirilmiyordu
    (ölçüldü: TMPDIR'siz ortamda tesseract hatası aynı sınıftı). Artık çökme
    "doğrulanamadı" olarak ayrılır ve uyarı olarak yazılır."""
    pypdf = _pypdf_module()
    if pypdf is None:
        return [], False, None
    try:
        reader = pypdf.PdfReader(io.BytesIO(data))
        found: list[str] = []
        for page in reader.pages:
            text = page.extract_text() or ""
            for term in terms:
                if term in text:
                    found.append(term)
        return sorted(set(found)), True, None
    except Exception:  # noqa: BLE001 - doğrulayıcı çöktü: "temiz" de değil, "sızıntı" da değil
        return [], False, "pypdf doğrulaması yapılamadı (okuyucu hatası)"


def _verify_with_ocr(
    data: bytes, terms: list[str]
) -> tuple[list[str], bool, str | None]:
    """Sayfa görüntüsünü OCR ile okur.

    `(bulunan, çalıştı_mı, hata)`. Tesseract/pytesseract yoksa `ran=False` ve
    `hata=None` (görüntü doğrulandı diye sunulmaz). **OCR çökmesi sızıntı
    değildir**: eskiden `list(terms)` dönüyordu ve tek bir ortam eksiği
    (TMPDIR) yüzünden maskeli dosya hiç kaydedilmiyordu — ölçüldü."""
    try:
        import pytesseract  # type: ignore
        from PIL import Image  # type: ignore
    except ImportError:
        return [], False, None
    import shutil

    if shutil.which("tesseract") is None:
        return [], False, None
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
    except Exception:  # noqa: BLE001 - çökme: doğrulanamadı (sızıntı değil)
        return [], False, "OCR doğrulaması yapılamadı (tesseract hatası)"
    return found, True, None


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
) -> tuple[list[str], bool, str | None]:
    """Karartılan bölgelerin **görüntüsünü** OCR ile okur.

    Bölge karartması pikselleri siler; metin katmanı temiz görünse bile
    görüntüde kalan yazı sızıntıdır. Tesseract yoksa `ran=False`."""
    if not probes:
        return [], False, None
    try:
        import pytesseract  # type: ignore
        from PIL import Image  # type: ignore
    except ImportError:
        return [], False, None
    if shutil.which("tesseract") is None:
        return [], False, None
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
    except Exception:  # noqa: BLE001 - çökme: doğrulanamadı (sızıntı değil)
        return [], False, "bölge OCR doğrulaması yapılamadı (tesseract hatası)"
    return sorted(found), True, None


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
    progress: Callable[[str, int, int | None], None] | None = None,
) -> RedactionResult:
    """Kullanıcının seçtiği dikdörtgenleri gerçekten kaldırır.

    `regions`: `{"page": int, "rect": [x0,y0,x1,y1]}` — punto, **döndürme öncesi
    cropbox** uzayı. Metin katmanı ve görüntü pikselleri birlikte silinir; ardından
    her bölge yeniden okunur ve içinde metin kaldıysa sonuç reddedilir.
    """
    pymupdf = _pymupdf()
    if not regions:
        raise EngineError("E_REDACT_REGIONS", "maskelenecek bölge yok")
    notify = progress or (lambda stage, done, total: None)

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
    total_pages = len({item[0] for item in parsed})
    notify("redact", 0, total_pages)
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
        for done, page_index in enumerate(sorted({item[0] for item in parsed}), start=1):
            doc.load_page(page_index).apply_redactions(images=pymupdf.PDF_REDACT_IMAGE_PIXELS)
            notify("redact", done, total_pages)
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
        notify("pack", 0, 1)
        out = doc.tobytes(garbage=4, deflate=True)
        notify("pack", 1, 1)
    finally:
        doc.close()

    check = pymupdf.open(stream=out, filetype="pdf")
    remaining: list[str] = []
    notify("verify", 0, total_pages)
    try:
        for done, (page_index, _rect, cleaned) in enumerate(parsed, start=1):
            page = check.load_page(page_index)
            leftover = (page.get_text("text", clip=pymupdf.Rect(*cleaned)) or "").strip()
            if leftover:
                remaining.append(f"sayfa {page_index + 1}: bölgede metin kaldı")
            notify("verify", done, total_pages)
    finally:
        check.close()

    # Bölgede ne olduğu bilinmediği için doğrulama bölgedeki kelimelerle yapılır.
    notify("scan", 0, 1)
    leaked, streams_scanned, streams_binary, streams_unreadable = _scan_terms(out, probes)
    remaining.extend(leaked)
    pypdf_found, pypdf_ran, pypdf_error = _verify_with_pypdf(out, probes)
    remaining.extend(pypdf_found)
    verify_words = sorted(set(probes) | set(ocr_probes))
    region_ocr_found, region_ocr_ran, region_ocr_error = _region_ocr_leaks(
        out, parsed, verify_words
    )
    remaining.extend(region_ocr_found)
    notify("scan", 1, 1)

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
    if pypdf_error:
        warnings.append(pypdf_error)
    elif not pypdf_ran:
        warnings.append("pypdf kurulu değil: ikinci okuyucu doğrulaması yapılmadı")
    if region_ocr_error:
        warnings.append(region_ocr_error)
    if ocr_probes and not probes and region_ocr_ran:
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


def redact_pdf(
    data: bytes,
    terms: list[str],
    fill: tuple[float, float, float] = (0.0, 0.0, 0.0),
    progress: Callable[[str, int, int | None], None] | None = None,
) -> RedactionResult:
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
    notify = progress or (lambda stage, done, total: None)

    try:
        doc = pymupdf.open(stream=data, filetype="pdf")
    except Exception as exc:  # noqa: BLE001
        raise EngineError("E_PDF_OPEN", "PDF açılamadı (bozuk veya parola korumalı)") from exc
    if doc.needs_pass:
        raise EngineError("E_PDF_PASSWORD", "belge parola korumalı; parola desteği henüz yok")

    hits: list[RedactionHit] = []
    total_pages = doc.page_count
    notify("redact", 0, total_pages)
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
            notify("redact", page.number + 1, total_pages)
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
        notify("pack", 0, 1)
        out = doc.tobytes(garbage=4, deflate=True)
        notify("pack", 1, 1)
    finally:
        doc.close()

    check = pymupdf.open(stream=out, filetype="pdf")
    remaining: list[str] = []
    notify("verify", 0, total_pages)
    try:
        for done, page in enumerate(check, start=1):
            text = page.get_text("text") or ""
            for term in cleaned:
                if term in text or page.search_for(term):
                    remaining.append(term)
                    break
            notify("verify", done, total_pages)
    finally:
        check.close()

    # Ham bayt taraması: 3 karakterden kısa terimler rastgele baytlarla
    # eşleşebileceği için yalnız metin katmanında doğrulanır (uyarıda yazılır).
    scan_terms = [t for t in cleaned if len(t) >= 4]
    notify("scan", 0, 1)
    leaked, streams_scanned, streams_binary, streams_unreadable = _scan_terms(out, scan_terms)
    remaining.extend(leaked)
    pypdf_found, pypdf_ran, pypdf_error = _verify_with_pypdf(out, cleaned)
    remaining.extend(pypdf_found)
    ocr_found, ocr_ran, ocr_error = _verify_with_ocr(out, cleaned)
    remaining.extend(ocr_found)
    notify("scan", 1, 1)

    engines = ["pymupdf", "raw-scan"]
    if pypdf_ran:
        engines.append("pypdf")
    if ocr_ran:
        engines.append("tesseract")

    warnings = _verification_warnings(
        scripts_removed=scripts_removed,
        annotations_removed=annotations_removed,
        offpage_spans_removed=offpage_spans_removed,
        streams_unreadable=streams_unreadable,
    )
    if pypdf_error:
        warnings.append(pypdf_error)
    elif not pypdf_ran:
        warnings.append("pypdf kurulu değil: ikinci okuyucu doğrulaması yapılmadı")
    if ocr_error:
        warnings.append(ocr_error)
    if len(scan_terms) < len(cleaned):
        warnings.append("3 karakterden kısa terimler yalnız metin katmanında doğrulandı")

    version = str(getattr(pymupdf, "__version__", "unknown"))
    return RedactionResult(
        pdf=out,
        hits=hits,
        remaining=sorted(set(remaining)),
        engine="pymupdf",
        engine_version=version,
        ocr_verified=ocr_ran,
        engines=engines,
        streams_scanned=streams_scanned,
        streams_binary=streams_binary,
        streams_unreadable=streams_unreadable,
        annotations_removed=annotations_removed,
        offpage_spans_removed=offpage_spans_removed,
        scripts_removed=scripts_removed,
        warnings=warnings,
    )


# Rapor üretimi ayrı modülde (F16): aynı yüzey korunur.
