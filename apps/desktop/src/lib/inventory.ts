/**
 * Belge envanteri yardımcıları (F01-T06).
 *
 * Saf fonksiyonlar: çekirdekten gelen envanteri ekran satırlarına çevirir.
 * Hiçbiri sayı uydurmaz; bilinmeyen anahtar kendi adıyla görünür, sıfır
 * değerler listelenmez. `notes` çekirdekten geldiği gibi taşınır.
 *
 * Metinler F04-T08 gereği `lib/i18n.ts`ten gelir. Etiket haritaları anahtar
 * haritasıdır (`Record<string, MessageKey>`): `t()` **çağrı anında** çözülür,
 * modül yüklenirken metin donmaz (tek dilli ürün kararı sürüyor).
 */

import type { DocumentChanges, DocumentInventory, InventoryAttachment } from "./api";
import { t, type MessageKey } from "./i18n";
import { formatBytes } from "./providers";

/** Yapı anahtarı → sözlük anahtarı. Bilinmeyen anahtar kendi adıyla görünür. */
const STRUCTURE_KEYS: Record<string, MessageKey> = {
  paragraphs: "inventory.structure.paragraphs",
  tables: "inventory.structure.tables",
  rows: "inventory.structure.rows",
  cells: "inventory.structure.cells",
  sections: "inventory.structure.sections",
  styles: "inventory.structure.styles",
  hyperlinks: "inventory.structure.hyperlinks",
  bookmarks: "inventory.structure.bookmarks",
  drawings: "inventory.structure.drawings",
  pictures: "inventory.structure.pictures",
  mediaParts: "inventory.structure.mediaParts",
  textBoxes: "inventory.structure.textBoxes",
  contentControls: "inventory.structure.contentControls",
  headers: "inventory.structure.headers",
  footers: "inventory.structure.footers",
  footnotes: "inventory.structure.footnotes",
  endnotes: "inventory.structure.endnotes",
  comments: "inventory.structure.comments",
  pages: "inventory.structure.pages",
  images: "inventory.structure.images",
  lists: "inventory.structure.lists",
  fields: "inventory.structure.fields",
  watermarks: "inventory.structure.watermarks",
  unknown: "inventory.structure.unknown",
};

const ATTACHMENT_KIND_KEYS: Record<string, MessageKey> = {
  ole_object: "inventory.attachmentKind.ole_object",
  embedded_package: "inventory.attachmentKind.embedded_package",
  alt_chunk: "inventory.attachmentKind.alt_chunk",
  image: "inventory.attachmentKind.image",
  signature: "inventory.attachmentKind.signature",
  other: "inventory.attachmentKind.other",
};

/** Yapı anahtarı etiketi; sözlükte yoksa anahtarın kendisi yazılır. */
export function structureLabel(key: string): string {
  const messageKey = STRUCTURE_KEYS[key];
  return messageKey ? t(messageKey) : key;
}

export type StructureRow = { key: string; label: string; count: number };

/** Sıfırdan büyük yapı sayıları; çoktan aza, eşitse anahtar adına göre. */
export function structureRows(inventory: DocumentInventory | null): StructureRow[] {
  if (!inventory) return [];
  return Object.entries(inventory.structure)
    .filter(([, count]) => count > 0)
    .map(([key, count]) => ({
      key,
      label: structureLabel(key),
      count,
    }))
    .sort((left, right) => right.count - left.count || left.key.localeCompare(right.key));
}

export function structureTotal(inventory: DocumentInventory | null): number {
  return structureRows(inventory).reduce((sum, row) => sum + row.count, 0);
}

export function attachmentKindLabel(kind: string): string {
  const messageKey = ATTACHMENT_KIND_KEYS[kind];
  return messageKey ? t(messageKey) : kind;
}

/** Ek satırı: paket içi ad + tür + boyut. Ad çekirdekten geldiği gibi yazılır. */
export function attachmentRows(
  inventory: DocumentInventory | null,
): { name: string; kindLabel: string; sizeLabel: string; detail: string | null }[] {
  if (!inventory) return [];
  return inventory.attachments.map((item: InventoryAttachment) => ({
    name: item.name,
    kindLabel: attachmentKindLabel(item.kind),
    sizeLabel: formatBytes(item.size),
    detail: item.detail ?? null,
  }));
}

export function attachmentSummary(inventory: DocumentInventory | null): string {
  if (!inventory) return "—";
  if (inventory.attachmentCount === 0) return t("inventory.noAttachments");
  return t("inventory.attachmentSummary", {
    count: inventory.attachmentCount,
    size: formatBytes(inventory.attachmentBytes),
  });
}

export function changeSummary(changes: DocumentChanges | undefined): string {
  if (!changes) return t("inventory.noChanges");
  const parts: string[] = [];
  if (changes.insertions > 0) parts.push(t("inventory.insertions", { count: changes.insertions }));
  if (changes.deletions > 0) parts.push(t("inventory.deletions", { count: changes.deletions }));
  if (changes.comments > 0) parts.push(t("inventory.comments", { count: changes.comments }));
  return parts.length > 0 ? parts.join(" · ") : t("inventory.noTrackedChanges");
}

/** Değişiklik izinin künye satırları: yalnız gerçekten dolu alanlar. */
export function changeRows(
  inventory: DocumentInventory | null,
): { label: string; value: string }[] {
  const changes = inventory?.changes;
  if (!changes) return [];
  const rows: { label: string; value: string }[] = [];
  if (changes.authors.length > 0)
    rows.push({ label: t("inventory.change.authors"), value: changes.authors.join(", ") });
  if (changes.firstDate)
    rows.push({ label: t("inventory.change.firstDate"), value: changes.firstDate });
  if (changes.lastDate)
    rows.push({ label: t("inventory.change.lastDate"), value: changes.lastDate });
  if (changes.revision) rows.push({ label: t("inventory.change.revision"), value: changes.revision });
  if (changes.lastModifiedBy)
    rows.push({ label: t("inventory.change.lastModifiedBy"), value: changes.lastModifiedBy });
  if (changes.creator) rows.push({ label: t("inventory.change.creator"), value: changes.creator });
  if (changes.created) rows.push({ label: t("inventory.change.created"), value: changes.created });
  if (changes.modified) rows.push({ label: t("inventory.change.modified"), value: changes.modified });
  if (changes.protection)
    rows.push({ label: t("inventory.change.protection"), value: `edit=${changes.protection}` });
  if (changes.trackRevisions === true)
    rows.push({ label: t("inventory.change.tracking"), value: t("inventory.change.on") });
  if (changes.signatures > 0)
    rows.push({
      label: t("inventory.change.signaturePart"),
      value: t("inventory.change.signatureUnverified", { count: changes.signatures }),
    });
  return rows;
}

/**
 * Uyarılar: envanterin **sınırlarını** görünür kılar. Sıra sabittir, böylece
 * test ve arayüz aynı listeyi görür.
 */
export function inventoryNotices(inventory: DocumentInventory | null): string[] {
  if (!inventory) return [];
  const notices: string[] = [];
  if (inventory.truncated) notices.push(t("inventory.notice.truncated"));
  if (inventory.externalLinks > 0)
    notices.push(t("inventory.notice.externalLinks", { count: inventory.externalLinks }));
  if (inventory.changes.signatures > 0) notices.push(t("inventory.notice.signature"));
  if (inventory.kind === "udf") notices.push(t("inventory.notice.udf"));
  if (inventory.notes.some((note) => note.includes("metin kutusu")))
    notices.push(t("inventory.notice.textBox"));
  if (inventory.notes.some((note) => note.includes("altChunk")))
    notices.push(t("inventory.notice.altChunk"));
  return notices;
}

/** Düğme kapısı: yalnız DOCX/UDF paketlerinde envanter var. */
export function inventorySupported(documentType: string | null | undefined): boolean {
  return documentType === "docx" || documentType === "udf";
}

export function inventoryGateReason(documentType: string | null | undefined): string | null {
  if (inventorySupported(documentType)) return null;
  if (!documentType) return t("inventory.gateUnknown");
  return t("inventory.gateUnsupported", { type: documentType });
}
