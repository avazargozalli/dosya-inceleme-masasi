"""UYAP UDF (ZIP + content.xml) güvenli ayrıştırma.

UDF bir ZIP arşividir ve belge metni `content.xml` içindeki tek bir CDATA
havuzunda durur; yapısal öğeler bu havuzu `startOffset`/`length` (karakter,
bayt değil) ile referans alır.

Güvenlik sınırları:
- DTD/ENTITY reddedilir; dış varlık hiçbir koşulda çözülmez.
- ZIP girişlerinde yol kaçışı, şifreleme ve boyut bombası reddedilir.
- Bilinmeyen öğeler sessizce atlanmaz; adetli envanter uyarısı üretilir.
- İmza **doğrulanmaz**; yalnız "tespit edildi — denetlenmedi" durumu bildirilir.
"""

from __future__ import annotations

import io
import re
import zipfile
from collections import Counter
from dataclasses import dataclass, field

MAX_ENTRIES = 512
MAX_ENTRY_BYTES = 32 * 1024 * 1024
MAX_TOTAL_BYTES = 64 * 1024 * 1024
MAX_TEXT_CHARS = 5_000_000

#: Bilinen UDF öğeleri (yerel ad, küçük harf). Burada olmayan her öğe raporlanır.
KNOWN_ELEMENTS = {
    "template",
    "content",
    "properties",
    "page",
    "background",
    "elements",
    "header",
    "footer",
    "body",
    "paragraph",
    "image",
    "table",
    "row",
    "cell",
    "space",
    "tab",
    "page-break",
    "page_break",
    "field",
    "list",
    "list-item",
    "section",
    "styles",
    "style",
    "data",
    "watermark",
    "text",
}

SIGNATURE_PATTERN = re.compile(r"(signature|\.p7s$|\.sig$|^meta-inf/)", re.IGNORECASE)


class UdfError(Exception):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code
        self.message = message


@dataclass
class UdfDocument:
    text: str
    warnings: list[str] = field(default_factory=list)
    paragraphs: int = 0
    images: int = 0
    signature_status: str | None = None
    embedded: list[str] = field(default_factory=list)
    # Metnin üretildiği paragraf blokları; `text` bunların `_join_blocks` ile
    # birleşimidir. Paragraf yolu (p/N) bu sıradan gelir, uydurulmaz.
    blocks: list[str] = field(default_factory=list)


def _local(tag: str) -> str:
    """`{ns}etiket` biçimindeki adı yerel ada indirger."""
    if "}" in tag:
        tag = tag.rsplit("}", 1)[1]
    return tag.lower()


def _attr(element, name: str) -> str | None:
    """Özniteliği büyük/küçük harf duyarsız okur (UDF sürümleri değişebiliyor)."""
    wanted = name.lower()
    for key, value in element.attrib.items():
        if _local(key) == wanted:
            return value
    return None


def _int_attr(element, name: str) -> int | None:
    raw = _attr(element, name)
    if raw is None:
        return None
    try:
        value = int(str(raw).strip())
    except ValueError:
        return None
    return value if value >= 0 else None


def _open_zip(data: bytes) -> zipfile.ZipFile:
    try:
        archive = zipfile.ZipFile(io.BytesIO(data))
    except zipfile.BadZipFile as exc:
        raise UdfError("E_UDF_ZIP", "UDF arşivi açılamadı (bozuk ZIP)") from exc

    infos = archive.infolist()
    if len(infos) > MAX_ENTRIES:
        archive.close()
        raise UdfError("E_UDF_ZIP_LIMIT", f"UDF {MAX_ENTRIES} giriş sınırını aşıyor")
    total = 0
    for info in infos:
        name = info.filename
        if (
            name.startswith("/")
            or "\\" in name
            or re.match(r"^[A-Za-z]:", name)
            or ".." in name.split("/")
        ):
            archive.close()
            raise UdfError("E_UDF_ZIP_UNSAFE", "UDF giriş yolu arşiv dışına çıkıyor")
        if info.flag_bits & 0x1:
            archive.close()
            raise UdfError("E_UDF_ZIP_UNSAFE", "şifreli UDF girişi desteklenmiyor")
        if info.file_size > MAX_ENTRY_BYTES:
            archive.close()
            raise UdfError("E_UDF_ZIP_LIMIT", "UDF girişi 32 MiB sınırını aşıyor")
        total += info.file_size
        if total > MAX_TOTAL_BYTES:
            archive.close()
            raise UdfError("E_UDF_ZIP_LIMIT", "UDF açılmış boyutu 64 MiB sınırını aşıyor")
    return archive


def _read_content(archive: zipfile.ZipFile) -> bytes:
    for info in archive.infolist():
        if info.filename.lower() == "content.xml":
            with archive.open(info) as handle:
                return handle.read(MAX_ENTRY_BYTES + 1)
    raise UdfError("E_UDF_STRUCTURE", "UDF içinde content.xml bulunamadı")


def _parse_xml(raw: bytes):
    import xml.etree.ElementTree as ET

    if len(raw) > MAX_ENTRY_BYTES:
        raise UdfError("E_UDF_XML_LIMIT", "content.xml 32 MiB sınırını aşıyor")
    head = raw[:65536].upper()
    if b"<!DOCTYPE" in head or b"<!ENTITY" in head:
        # Dış varlık/entity genişletmesi kapalıdır; belge işlenmez.
        raise UdfError("E_UDF_XML_UNSAFE", "content.xml DTD/ENTITY bildirimi içeriyor")
    try:
        return ET.fromstring(raw)
    except ET.ParseError as exc:
        raise UdfError("E_UDF_XML", "content.xml geçerli XML değil") from exc


def _text_pool(root) -> str:
    for element in root.iter():
        if _local(element.tag) != "content":
            continue
        if _attr(element, "startOffset") is not None:
            continue
        if element.text:
            return element.text
    raise UdfError("E_UDF_CONTENT_MISSING", "UDF metin havuzu (content CDATA) bulunamadı")


def _refs(element) -> list[tuple[int, int, str]]:
    """Alt öğelerdeki offset/length referanslarını sıralı döndürür."""
    out: list[tuple[int, int, str]] = []
    for child in element.iter():
        if child is element:
            continue
        start = _int_attr(child, "startOffset")
        length = _int_attr(child, "length")
        if start is None or length is None:
            continue
        out.append((start, length, _local(child.tag)))
    out.sort(key=lambda item: (item[0], item[1]))
    return out


def _join_blocks(blocks: list[str]) -> str:
    text = ""
    for block in blocks:
        if not block:
            continue
        if text and not text.endswith("\n") and not block.startswith("\n"):
            text += "\n"
        text += block
    return text


def extract_udf_document(data: bytes) -> UdfDocument:
    archive = _open_zip(data)
    try:
        raw = _read_content(archive)
        entries = archive.infolist()
    finally:
        archive.close()

    root = _parse_xml(raw)
    pool = _text_pool(root)
    if len(pool) > MAX_TEXT_CHARS:
        raise UdfError("E_UDF_TEXT_LIMIT", "UDF metni 5 milyon karakter sınırını aşıyor")

    warnings: list[str] = []
    paragraphs = 0
    blocks: list[str] = []
    paragraph_elements = [el for el in root.iter() if _local(el.tag) == "paragraph"]
    for element in paragraph_elements:
        pieces = []
        for start, length, _kind in _refs(element):
            piece = pool[start : start + length]
            if len(piece) < length:
                warnings.append("UDF: offset metin havuzunu aşıyor; eksik parça atlandı")
            pieces.append(piece)
        block = "".join(pieces)
        if block.strip():
            blocks.append(block)
            paragraphs += 1

    if not blocks:
        all_refs = _refs(root)
        if all_refs:
            blocks.append("".join(pool[start : start + length] for start, length, _ in all_refs))
            warnings.append("UDF: paragraf yapısı bulunamadı; metin offset sırasıyla birleştirildi")
        elif pool.strip():
            blocks.append(pool.strip())
            warnings.append("UDF: yapısal öğe bulunamadı; metin havuzu bütün olarak alındı")

    text = _join_blocks(blocks)
    if not text.strip():
        warnings.append("UDF: belgede metin bulunamadı")

    counts = Counter(_local(el.tag) for el in root.iter())
    unknown = {name: count for name, count in counts.items() if name not in KNOWN_ELEMENTS}
    if unknown:
        listed = ", ".join(f"{name}×{count}" for name, count in sorted(unknown.items()))
        warnings.append(f"UDF: bilinmeyen öğe(lar) raporlandı: {listed}")

    image_refs = sum(1 for _, _, kind in _refs(root) if "image" in kind or kind == "resim")
    placeholder_count = text.count("\ufffc")
    images = max(image_refs, placeholder_count)

    embedded = [info.filename for info in entries if info.filename.lower() != "content.xml"]
    if embedded:
        preview = ", ".join(embedded[:10])
        suffix = "" if len(embedded) <= 10 else f" (+{len(embedded) - 10})"
        warnings.append(f"UDF: {len(embedded)} gömülü nesne: {preview}{suffix}")

    signature_entries = [name for name in (info.filename for info in entries) if SIGNATURE_PATTERN.search(name)]
    signature_status = None
    if signature_entries:
        signature_status = "tespit edildi — denetlenmedi"
        warnings.append(
            "UDF imza: tespit edildi — denetlenmedi ("
            + ", ".join(signature_entries[:5])
            + "); kriptografik doğrulama yapılmadı"
        )

    return UdfDocument(
        text=text,
        warnings=warnings,
        paragraphs=paragraphs,
        images=images,
        signature_status=signature_status,
        embedded=embedded,
        blocks=blocks,
    )
