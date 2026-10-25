"""Belge envanteri: yapı, ekler ve değişiklik izi (DOCX + UDF).

Envanter **paket düzeyinde** çıkarılır (ZIP üyeleri + XML parçaları); metin
çıkarımından bağımsızdır. Böylece python-docx'in gövdeye katmadığı içerik
(metin kutusu, altChunk, gömülü nesne) "yok" sayılmaz, **sayılır** ve
çıkarılamadığı açıkça yazılır.

Dışarı verilen bilgi en azdır:

- ekler: paket içi ad + açılmış boyut + tür (içerik okunmaz),
- değişiklik izi: sayı, yazar adı, tarih aralığı, sürüm/ son kaydeden,
- dış bağlantı: yalnız **sayı** (hedef adres listelenmez).

Güvenlik sınırları `udf`/`engines` ile aynı kuralları yeniden kullanır: yol
kaçışı, şifreli giriş, giriş sayısı/boyut toplamı, parça boyutu ve DTD/ENTITY
reddi. Yeni bir kural yazılmaz.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from . import engines, udf

#: Paket giriş sınırları (UDF ile aynı değerler; tek yerden).
MAX_ENTRIES = udf.MAX_ENTRIES
MAX_ENTRY_BYTES = udf.MAX_ENTRY_BYTES
MAX_TOTAL_BYTES = udf.MAX_TOTAL_BYTES

#: Listede gösterilecek en çok ek sayısı. Sayım her zaman tam sayıdır;
#: liste kesilirse `truncated=true` ve not yazılır (sessiz kesme yok).
MAX_LISTED_ATTACHMENTS = 100
MAX_AUTHORS = 20


class InventoryError(Exception):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code
        self.message = message


@dataclass
class Attachment:
    name: str
    kind: str
    size: int
    detail: str | None = None

    def to_dict(self) -> dict[str, Any]:
        payload: dict[str, Any] = {"name": self.name, "kind": self.kind, "size": self.size}
        if self.detail:
            payload["detail"] = self.detail
        return payload


@dataclass
class Inventory:
    kind: str
    structure: dict[str, int] = field(default_factory=dict)
    attachments: list[Attachment] = field(default_factory=list)
    attachment_count: int = 0
    attachment_bytes: int = 0
    changes: dict[str, Any] = field(default_factory=dict)
    external_links: int = 0
    notes: list[str] = field(default_factory=list)
    truncated: bool = False
    limits: dict[str, int] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return {
            "kind": self.kind,
            "structure": self.structure,
            "attachments": [item.to_dict() for item in self.attachments],
            "attachmentCount": self.attachment_count,
            "attachmentBytes": self.attachment_bytes,
            "changes": self.changes,
            "externalLinks": self.external_links,
            "notes": self.notes,
            "truncated": self.truncated,
            "limits": self.limits,
        }


def _checked_archive(data: bytes, code: str) -> Any:
    """DOCX ZIP'ini giriş güvenlik denetimleriyle açar (UDF kuralları)."""
    import io
    import re
    import zipfile

    try:
        archive = zipfile.ZipFile(io.BytesIO(data))
    except zipfile.BadZipFile as exc:
        raise InventoryError(code, "belge arşivi açılamadı (bozuk ZIP)") from exc

    infos = archive.infolist()
    if len(infos) > MAX_ENTRIES:
        archive.close()
        raise InventoryError("E_INVENTORY_ZIP_LIMIT", f"arşiv {MAX_ENTRIES} giriş sınırını aşıyor")
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
            raise InventoryError("E_INVENTORY_ZIP_UNSAFE", "arşiv girişi arşiv dışına çıkıyor")
        if info.flag_bits & 0x1:
            archive.close()
            raise InventoryError("E_INVENTORY_ZIP_UNSAFE", "şifreli arşiv girişi desteklenmiyor")
        if info.file_size > MAX_ENTRY_BYTES:
            archive.close()
            raise InventoryError("E_INVENTORY_ZIP_LIMIT", "arşiv girişi boyut sınırını aşıyor")
        total += info.file_size
        if total > MAX_TOTAL_BYTES:
            archive.close()
            raise InventoryError("E_INVENTORY_ZIP_LIMIT", "arşiv açılmış boyutu sınırı aşıyor")
    return archive


def _loc(tag: Any) -> str:
    """`{ns}etiket` → `etiket` (küçük harf)."""
    text = str(tag)
    if "}" in text:
        text = text.rsplit("}", 1)[1]
    return text.lower()


def _attr_loc(element: Any, name: str) -> str | None:
    wanted = name.lower()
    for key, value in element.attrib.items():  # type: ignore[union-attr]
        if _loc(key) == wanted:
            return value
    return None


def _counts(root: Any) -> dict[str, int]:
    out: dict[str, int] = {}
    for element in root.iter():
        name = _loc(element.tag)
        out[name] = out.get(name, 0) + 1
    return out


def _w(name: str) -> str:
    return f"{engines._W_NS}{name}"


def _docx_attachment_kind(path: str) -> str:
    lowered = path.lower()
    if lowered.startswith("word/embeddings/"):
        if lowered.endswith((".docx", ".docm", ".xlsx", ".xlsm", ".pptx", ".pptm", ".pdf")):
            return "embedded_package"
        return "ole_object"
    if lowered.startswith("word/media/"):
        return "image"
    if lowered.startswith("word/afchunk") or lowered.endswith((".mht", ".mhtml")):
        return "alt_chunk"
    return "other"


def _relationship_targets(zf: Any) -> tuple[list[tuple[str, str, str]], list[str]]:
    """`(Id, Type, Target, TargetMode)` üçlüleri + okunamayan parça notları."""
    names = [
        "word/_rels/document.xml.rels",
        "word/_rels/header1.xml.rels",
        "word/_rels/footer1.xml.rels",
    ]
    rows: list[tuple[str, str, str, str]] = []
    for name in names:
        blob = engines._docx_part(zf, name)
        if blob is None:
            continue
        root = engines._xml_root(blob, name)
        for rel in root.iter():
            if _loc(rel.tag) != "relationship":
                continue
            rows.append(
                (
                    str(_attr_loc(rel, "Id") or ""),
                    str(_attr_loc(rel, "Type") or ""),
                    str(_attr_loc(rel, "Target") or ""),
                    str(_attr_loc(rel, "TargetMode") or ""),
                )
            )
    return [(rid, kind, target) for rid, kind, target, mode in rows if mode.lower() != "external"], [
        target for _rid, _kind, target, mode in rows if mode.lower() == "external"
    ]


def _core_properties(zf: Any) -> dict[str, str]:
    blob = engines._docx_part(zf, "docProps/core.xml")
    if blob is None:
        return {}
    root = engines._xml_root(blob, "docProps/core.xml")
    out: dict[str, str] = {}
    for child in root:
        value = (child.text or "").strip()
        if value:
            out[_loc(child.tag)] = value
    return out


def docx_inventory(data: bytes) -> Inventory:
    """DOCX paketi: yapı sayıları, ekler, değişiklik izi."""
    zf = _checked_archive(data, "E_DOCX_OPEN")
    try:
        body_blob = engines._docx_part(zf, "word/document.xml")
        if body_blob is None:
            raise InventoryError("E_DOCX_XML", "DOCX gövdesi (document.xml) bulunamadı")
        body = engines._xml_root(body_blob, "word/document.xml")
        settings_blob = engines._docx_part(zf, "word/settings.xml")
        settings = (
            engines._xml_root(settings_blob, "word/settings.xml") if settings_blob is not None else None
        )
        styles_blob = engines._docx_part(zf, "word/styles.xml")
        styles = engines._xml_root(styles_blob, "word/styles.xml") if styles_blob is not None else None
        notes_blobs = {
            name: engines._docx_part(zf, name)
            for name in ("word/footnotes.xml", "word/endnotes.xml", "word/comments.xml")
        }
        infos = zf.infolist()
        sizes = {info.filename: info.file_size for info in infos}
        internal_targets, external_targets = _relationship_targets(zf)
        core = _core_properties(zf)
    finally:
        zf.close()

    body_counts = _counts(body)
    structure: dict[str, int] = {
        "paragraphs": body_counts.get("p", 0),
        "tables": body_counts.get("tbl", 0),
        "rows": body_counts.get("tr", 0),
        "cells": body_counts.get("tc", 0),
        "sections": body_counts.get("sectpr", 0),
        "styles": len(list(styles.iter(_w("style")))) if styles is not None else 0,
        "hyperlinks": body_counts.get("hyperlink", 0),
        "bookmarks": body_counts.get("bookmarkstart", 0),
        # `w:drawing` ve `w:pict` ayrı sayılır: `w:pict` metin kutusu ya da
        # OLE sarmalayıcısı da olabilir; "resim" demek yanıltıcı olurdu.
        "drawings": body_counts.get("drawing", 0),
        "pictures": body_counts.get("pict", 0),
        "mediaParts": sum(1 for name in sizes if name.startswith("word/media/")),
        "textBoxes": body_counts.get("txbxcontent", 0),
        "contentControls": body_counts.get("sdt", 0),
        "headers": sum(1 for name in sizes if name.startswith("word/header") and name.endswith(".xml")),
        "footers": sum(1 for name in sizes if name.startswith("word/footer") and name.endswith(".xml")),
    }

    annotation_counts: dict[str, int] = {}
    for part, key in (
        ("word/footnotes.xml", "footnotes"),
        ("word/endnotes.xml", "endnotes"),
        ("word/comments.xml", "comments"),
    ):
        blob = notes_blobs.get(part)
        if blob is None:
            structure[key] = 0
            annotation_counts[key] = 0
            continue
        root = engines._xml_root(blob, part)
        tag = {"footnotes": "footnote", "endnotes": "endnote", "comments": "comment"}[key]
        total = 0
        for node in root.iter(_w(tag)):
            # -1/0 ayırıcı kayıtlar içerik taşımaz (engines ile aynı kural).
            if key != "comments" and (node.get(_w("id")) or "") in {"-1", "0"}:
                continue
            total += 1
        structure[key] = total
        annotation_counts[key] = total

    # --- ekler: gömülü nesneler, altChunk, medya -------------------------
    prog_ids = sorted(
        {
            value
            for element in body.iter()
            for key, value in element.attrib.items()
            if _loc(key) == "progid" and value
        }
    )
    prog_detail = ", ".join(prog_ids[:5]) if prog_ids else None

    attachments: list[Attachment] = []
    attachment_bytes = 0
    raw_attachments: list[Attachment] = []
    for info in infos:
        name = info.filename
        if not (
            name.startswith("word/embeddings/")
            or name.startswith("word/media/")
            or name.startswith("word/afchunk")
        ):
            continue
        kind = _docx_attachment_kind(name)
        detail = prog_detail if kind in {"ole_object", "embedded_package"} else None
        raw_attachments.append(Attachment(name=name, kind=kind, size=info.file_size, detail=detail))
        attachment_bytes += info.file_size
    # AltChunk hedefleri ilişkilerden gelir; paket içi hedef varsa listelenir.
    listed_names = {item.name for item in raw_attachments}
    for _rid, rel_kind, target in internal_targets:
        if "afchunk" not in rel_kind.lower():
            continue
        resolved = target.lstrip("/")
        if not resolved.startswith("word/"):
            resolved = f"word/{resolved}"
        if resolved in listed_names:
            continue
        raw_attachments.append(
            Attachment(name=resolved, kind="alt_chunk", size=sizes.get(resolved, 0))
        )
        listed_names.add(resolved)

    if len(raw_attachments) > MAX_LISTED_ATTACHMENTS:
        attachments = raw_attachments[:MAX_LISTED_ATTACHMENTS]
    else:
        attachments = raw_attachments

    # --- değişiklik izi --------------------------------------------------
    authors: list[str] = []
    dates: list[str] = []

    def note_author(element: Any) -> None:
        author = (element.get(_w("author")) or "").strip()
        if author and author not in authors and len(authors) < MAX_AUTHORS:
            authors.append(author)
        date = (element.get(_w("date")) or "").strip()
        if date:
            dates.append(date)

    insertions = 0
    for element in body.iter(_w("ins")):
        note_author(element)
        insertions += 1
    deletions = 0
    for element in body.iter(_w("del")):
        note_author(element)
        deletions += 1
    comments = 0
    comments_blob = notes_blobs.get("word/comments.xml")
    if comments_blob is not None:
        comments_root = engines._xml_root(comments_blob, "word/comments.xml")
        for element in comments_root.iter(_w("comment")):
            note_author(element)
            comments += 1

    track_revisions = False
    protection: str | None = None
    if settings is not None:
        track_revisions = any(True for _ in settings.iter(_w("trackRevisions")))
        for element in settings.iter(_w("documentProtection")):
            protection = (_attr_loc(element, "edit") or "on").strip() or "on"

    signatures = sum(
        1
        for name in sizes
        if name.lower().startswith("_xmlsignatures/") and name.lower().endswith(".xml")
    )

    changes: dict[str, Any] = {
        "insertions": insertions,
        "deletions": deletions,
        "comments": comments,
        "authors": authors,
        "firstDate": min(dates) if dates else None,
        "lastDate": max(dates) if dates else None,
        "revision": core.get("revision"),
        "lastModifiedBy": core.get("lastmodifiedby"),
        "creator": core.get("creator"),
        "created": core.get("created"),
        "modified": core.get("modified"),
        "trackRevisions": track_revisions,
        "protection": protection,
        "signatures": signatures,
    }

    notes: list[str] = []
    if structure["textBoxes"]:
        notes.append(
            f"{structure['textBoxes']} metin kutusu: metin kutusu içeriği gövde metnine "
            "katılmıyor (python-docx); yalnız sayıldı"
        )
    alt_chunks = sum(1 for item in raw_attachments if item.kind == "alt_chunk")
    if alt_chunks:
        notes.append(f"{alt_chunks} altChunk: içeriği ayrıştırılmıyor, yalnız envanterde")
    embedded = sum(1 for item in raw_attachments if item.kind in {"ole_object", "embedded_package"})
    if embedded:
        notes.append(
            f"{embedded} gömülü nesne: içeriği çıkarılmıyor (yeniden kullanılmıyor), "
            "yalnız ad/boyut/tür listelenir"
        )
    if signatures:
        notes.append(
            f"{signatures} XML imza parçası: tespit edildi — kriptografik doğrulama yapılmadı"
        )
    if protection:
        notes.append(f"belge koruması açık (edit={protection}); düzenleme kısıtlı olabilir")
    if track_revisions:
        notes.append("izlenen değişiklikler ayarı açık (trackRevisions)")
    if external_targets:
        notes.append(
            f"{len(external_targets)} dış bağlantı: hedef adresler listelenmez, ağ isteği gönderilmez"
        )
    if len(raw_attachments) > len(attachments):
        notes.append(
            f"ek listesi ilk {MAX_LISTED_ATTACHMENTS} öğe ile sınırlandı; "
            f"toplam {len(raw_attachments)}"
        )
    notes.append(
        "değişiklik sayıları XML işaretlerine dayanır (w:ins/w:del); Word'ün "
        "değişiklik panelindeki gruplamayla birebir aynı olmayabilir"
    )
    notes.append(
        "yapı sayıları XML öğe sayısıdır: tablo hücresi ve metin kutusu "
        "paragrafları da sayılır; w:pict metin kutusu/OLE sarmalayıcısı olabilir"
    )

    return Inventory(
        kind="docx",
        structure=structure,
        attachments=attachments,
        attachment_count=len(raw_attachments),
        attachment_bytes=attachment_bytes,
        changes=changes,
        external_links=len(external_targets),
        notes=notes,
        truncated=len(raw_attachments) > len(attachments),
        limits={
            "entries": MAX_ENTRIES,
            "entryBytes": MAX_ENTRY_BYTES,
            "totalBytes": MAX_TOTAL_BYTES,
            "listedAttachments": MAX_LISTED_ATTACHMENTS,
        },
    )


def udf_inventory(data: bytes) -> Inventory:
    """UDF (UYAP) paketi: öğe sayıları, gömülü dosyalar, imza izi."""
    try:
        archive = udf._open_zip(data)
    except udf.UdfError as exc:
        raise InventoryError(exc.code, exc.message) from exc
    try:
        raw = udf._read_content(archive)
        infos = [(info.filename, info.file_size) for info in archive.infolist()]
    except udf.UdfError as exc:
        raise InventoryError(exc.code, exc.message) from exc
    finally:
        archive.close()

    try:
        root = udf._parse_xml(raw)
    except udf.UdfError as exc:
        raise InventoryError(exc.code, exc.message) from exc

    counts = _counts(root)
    structure = {
        "pages": counts.get("page", 0),
        "paragraphs": counts.get("paragraph", 0),
        "tables": counts.get("table", 0),
        "rows": counts.get("row", 0),
        "cells": counts.get("cell", 0),
        "images": counts.get("image", 0) + counts.get("resim", 0),
        "headers": counts.get("header", 0),
        "footers": counts.get("footer", 0),
        "lists": counts.get("list", 0),
        "fields": counts.get("field", 0),
        "watermarks": counts.get("watermark", 0),
        "unknown": sum(
            count for name, count in counts.items() if name not in udf.KNOWN_ELEMENTS
        ),
    }

    attachments: list[Attachment] = []
    attachment_bytes = 0
    signatures = 0
    for name, size in infos:
        if name.lower() == "content.xml":
            continue
        attachment_bytes += size
        if udf.SIGNATURE_PATTERN.search(name):
            kind = "signature"
            signatures += 1
        elif name.lower().endswith((".jpg", ".jpeg", ".png", ".gif", ".bmp", ".tif", ".tiff")):
            kind = "image"
        elif name.lower().endswith((".udf", ".pdf", ".docx", ".xlsx")):
            kind = "embedded_package"
        else:
            kind = "other"
        attachments.append(Attachment(name=name, kind=kind, size=size))

    truncated = len(attachments) > MAX_LISTED_ATTACHMENTS
    notes: list[str] = [
        "UDF'te izlenen değişiklik (revizyon) kavramı yok: bu alan UDF için boş",
        "UDF sayfa ayrımı doğrulanmadı; sayfa sayısı yalnız bildirilen öğe sayısıdır",
    ]
    if signatures:
        notes.append(
            f"{signatures} imza dosyası: tespit edildi — kriptografik doğrulama yapılmadı"
        )
    if structure["unknown"]:
        notes.append(f"{structure['unknown']} bilinmeyen öğe: tanınmayan yapı, sayıldı")
    if truncated:
        notes.append(
            f"ek listesi ilk {MAX_LISTED_ATTACHMENTS} öğe ile sınırlandı; toplam {len(attachments)}"
        )

    return Inventory(
        kind="udf",
        structure=structure,
        attachments=attachments[:MAX_LISTED_ATTACHMENTS],
        attachment_count=len(attachments),
        attachment_bytes=attachment_bytes,
        changes={
            "insertions": 0,
            "deletions": 0,
            "comments": 0,
            "authors": [],
            "firstDate": None,
            "lastDate": None,
            "revision": None,
            "lastModifiedBy": None,
            "creator": None,
            "created": None,
            "modified": None,
            "trackRevisions": None,
            "protection": None,
            "signatures": signatures,
        },
        external_links=0,
        notes=notes,
        truncated=truncated,
        limits={
            "entries": MAX_ENTRIES,
            "entryBytes": MAX_ENTRY_BYTES,
            "totalBytes": MAX_TOTAL_BYTES,
            "listedAttachments": MAX_LISTED_ATTACHMENTS,
        },
    )


DOCX_MIME = "application/vnd.openxmlformats-officedocument.wordprocessingml.document"


def inventory(data: bytes, mime: str) -> Inventory:
    """MIME'a göre envanter. Desteklenmeyen tür açık hatayla döner."""
    lowered = (mime or "").split(";")[0].strip().lower()
    if lowered == DOCX_MIME or lowered == "application/docx":
        return docx_inventory(data)
    if lowered in {"application/udf", "application/x-udf"}:
        return udf_inventory(data)
    raise InventoryError(
        "E_INVENTORY_UNSUPPORTED",
        f"envanter yalnız DOCX/UDF için çıkarılır ({lowered or 'bilinmeyen tür'})",
    )
