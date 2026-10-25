/**
 * Belge envanteri yardımcıları (F01-T06).
 *
 * Saf fonksiyonlar: çekirdekten gelen envanteri ekran satırlarına çevirir.
 * Hiçbiri sayı uydurmaz; bilinmeyen anahtar kendi adıyla görünür, sıfır
 * değerler listelenmez. `notes` çekirdekten geldiği gibi taşınır.
 */

import type { DocumentChanges, DocumentInventory, InventoryAttachment } from "./api";
import { formatBytes } from "./providers";

export const STRUCTURE_LABELS: Record<string, string> = {
  paragraphs: "Paragraf",
  tables: "Tablo",
  rows: "Satır",
  cells: "Hücre",
  sections: "Bölüm (sectPr)",
  styles: "Stil",
  hyperlinks: "Bağlantı",
  bookmarks: "Yer imi",
  drawings: "Çizim (w:drawing)",
  pictures: "Resim/nesne (w:pict)",
  mediaParts: "Medya parçası",
  textBoxes: "Metin kutusu",
  contentControls: "İçerik denetimi",
  headers: "Üstbilgi",
  footers: "Altbilgi",
  footnotes: "Dipnot",
  endnotes: "Sonnot",
  comments: "Yorum",
  pages: "Sayfa (bildirilen)",
  images: "Görsel (öğe)",
  lists: "Liste",
  fields: "Alan",
  watermarks: "Filigran",
  unknown: "Bilinmeyen öğe",
};

export const ATTACHMENT_KIND_LABELS: Record<string, string> = {
  ole_object: "Gömülü nesne (OLE)",
  embedded_package: "Gömülü paket",
  alt_chunk: "altChunk",
  image: "Görsel",
  signature: "İmza dosyası",
  other: "Diğer",
};

export type StructureRow = { key: string; label: string; count: number };

/** Sıfırdan büyük yapı sayıları; çoktan aza, eşitse anahtar adına göre. */
export function structureRows(inventory: DocumentInventory | null): StructureRow[] {
  if (!inventory) return [];
  return Object.entries(inventory.structure)
    .filter(([, count]) => count > 0)
    .map(([key, count]) => ({
      key,
      label: STRUCTURE_LABELS[key] ?? key,
      count,
    }))
    .sort((left, right) => right.count - left.count || left.key.localeCompare(right.key));
}

export function structureTotal(inventory: DocumentInventory | null): number {
  return structureRows(inventory).reduce((sum, row) => sum + row.count, 0);
}

export function attachmentKindLabel(kind: string): string {
  return ATTACHMENT_KIND_LABELS[kind] ?? kind;
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
  if (inventory.attachmentCount === 0) return "Ek yok";
  return `${inventory.attachmentCount} ek · ${formatBytes(inventory.attachmentBytes)}`;
}

export function changeSummary(changes: DocumentChanges | undefined): string {
  if (!changes) return "Değişiklik izi yok";
  const parts: string[] = [];
  if (changes.insertions > 0) parts.push(`${changes.insertions} ekleme`);
  if (changes.deletions > 0) parts.push(`${changes.deletions} silme`);
  if (changes.comments > 0) parts.push(`${changes.comments} yorum`);
  return parts.length > 0 ? parts.join(" · ") : "İzlenen değişiklik yok";
}

/** Değişiklik izinin künye satırları: yalnız gerçekten dolu alanlar. */
export function changeRows(
  inventory: DocumentInventory | null,
): { label: string; value: string }[] {
  const changes = inventory?.changes;
  if (!changes) return [];
  const rows: { label: string; value: string }[] = [];
  if (changes.authors.length > 0) rows.push({ label: "Yazarlar", value: changes.authors.join(", ") });
  if (changes.firstDate) rows.push({ label: "İlk değişiklik", value: changes.firstDate });
  if (changes.lastDate) rows.push({ label: "Son değişiklik", value: changes.lastDate });
  if (changes.revision) rows.push({ label: "Revizyon", value: changes.revision });
  if (changes.lastModifiedBy) rows.push({ label: "Son kaydeden", value: changes.lastModifiedBy });
  if (changes.creator) rows.push({ label: "Oluşturan", value: changes.creator });
  if (changes.created) rows.push({ label: "Oluşturulma", value: changes.created });
  if (changes.modified) rows.push({ label: "Değiştirilme", value: changes.modified });
  if (changes.protection) rows.push({ label: "Koruma", value: `edit=${changes.protection}` });
  if (changes.trackRevisions === true) rows.push({ label: "İzleme", value: "açık" });
  if (changes.signatures > 0)
    rows.push({ label: "İmza parçası", value: `${changes.signatures} — doğrulanmadı` });
  return rows;
}

/**
 * Uyarılar: envanterin **sınırlarını** görünür kılar. Sıra sabittir, böylece
 * test ve arayüz aynı listeyi görür.
 */
export function inventoryNotices(inventory: DocumentInventory | null): string[] {
  if (!inventory) return [];
  const notices: string[] = [];
  if (inventory.truncated) notices.push("Ek listesi kesildi: sayı tam, liste sınırlı.");
  if (inventory.externalLinks > 0)
    notices.push(
      `${inventory.externalLinks} dış bağlantı: hedef adresler gösterilmez, ürün ağ isteği göndermez.`,
    );
  if (inventory.changes.signatures > 0)
    notices.push("İmza parçası tespit edildi — kriptografik doğrulama yapılmadı.");
  if (inventory.kind === "udf")
    notices.push("UDF'te izlenen değişiklik (revizyon) kavramı yok; o alanlar boş.");
  if (inventory.notes.some((note) => note.includes("metin kutusu")))
    notices.push("Metin kutusu içeriği gövde metnine katılmıyor (sayıldı, çıkarılmadı).");
  if (inventory.notes.some((note) => note.includes("altChunk")))
    notices.push("altChunk içeriği ayrıştırılmıyor; yalnız envanterde görünür.");
  return notices;
}

/** Düğme kapısı: yalnız DOCX/UDF paketlerinde envanter var. */
export function inventorySupported(documentType: string | null | undefined): boolean {
  return documentType === "docx" || documentType === "udf";
}

export function inventoryGateReason(documentType: string | null | undefined): string | null {
  if (inventorySupported(documentType)) return null;
  if (!documentType) return "Belge türü bilinmiyor: envanter çıkarılamaz.";
  return `Envanter yalnız DOCX/UDF için çıkarılır (bu belge: ${documentType}).`;
}
