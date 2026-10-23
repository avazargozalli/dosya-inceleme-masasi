import type { ChecklistItemState, ChecklistTemplateRow } from "./api";

/**
 * Durum etiketi. `missing` bilinçli olarak "incelenen kapsamda bulunamadı"
 * der: liste, dosyanın tamamında belge olmadığı iddiasını üretmez (AI-07).
 */
export const CHECKLIST_STATE_LABEL: Record<ChecklistItemState, string> = {
  open: "Açık",
  present: "Bulundu",
  missing: "İncelenen kapsamda bulunamadı",
  not_applicable: "Uygulanamaz",
};

export const CHECKLIST_STATE_HELP: Record<ChecklistItemState, string> = {
  open: "Henüz değerlendirilmedi.",
  present: "En az bir belge bağı gerekir.",
  missing: "İncelenen kapsam yazılmalı; “dosyada yok” iddiası üretilmez.",
  not_applicable: "Gerekçe yazılmalı.",
};

export const MISSING_SCOPE_NOTE =
  "“İncelenen kapsamda bulunamadı” yalnız bakılan aralığı anlatır; dosyanın tamamında belge yok anlamına gelmez.";

export function isChecklistState(value: string): value is ChecklistItemState {
  return value === "open" || value === "present" || value === "missing" || value === "not_applicable";
}

export type ChecklistEntryInput = {
  state: ChecklistItemState;
  linkedDocumentIds: string[];
  note: string;
};

/**
 * Arayüzdeki kapı, çekirdekteki kuralla aynı: buton ancak bu kontrol `null`
 * döndüğünde açılır. Böylece "tıklayınca hata" yerine kural önceden görünür.
 */
export function validateChecklistEntry(input: ChecklistEntryInput): string | null {
  const links = input.linkedDocumentIds.filter((id) => id.trim().length > 0);
  const note = input.note.trim();
  if (input.state === "present") {
    return links.length === 0 ? "“Bulundu” için en az bir belge seçin." : null;
  }
  if (input.state === "missing") {
    if (links.length > 0) return "Belge bağı yalnız “Bulundu” durumunda kullanılır.";
    return note.length === 0 ? "İncelenen kapsamı yazın (örn. 2020-2024 arası sözleşmeler)." : null;
  }
  if (input.state === "not_applicable") {
    if (links.length > 0) return "Belge bağı yalnız “Bulundu” durumunda kullanılır.";
    return note.length === 0 ? "“Uygulanamaz” için gerekçe yazın." : null;
  }
  return links.length > 0 ? "Açık duruma dönerken belge bağlarını kaldırın." : null;
}

export type ChecklistSummary = {
  total: number;
  open: number;
  present: number;
  missing: number;
  notApplicable: number;
};

export function summarizeChecklist(items: Array<{ state: string }>): ChecklistSummary {
  const summary: ChecklistSummary = {
    total: items.length,
    open: 0,
    present: 0,
    missing: 0,
    notApplicable: 0,
  };
  for (const item of items) {
    if (item.state === "present") summary.present += 1;
    else if (item.state === "missing") summary.missing += 1;
    else if (item.state === "not_applicable") summary.notApplicable += 1;
    else summary.open += 1;
  }
  return summary;
}

/** Katalog kartındaki lisans/sürüm künyesi; kopyalanan kayıt da aynı bilgiyi taşır. */
export function checklistLicenseLabel(template: {
  licenseId: string | null;
  version: number;
  author: string;
  reviewDate: string | null;
}): string {
  const parts = [`sürüm ${template.version}`, template.licenseId ?? "lisans belirtilmemiş"];
  if (template.reviewDate) parts.push(`gözden geçirme ${template.reviewDate}`);
  parts.push(template.author);
  return parts.join(" · ");
}

/** Kısa hash: kullanıcı iki şablonun aynı içerik olup olmadığını karşılaştırabilsin. */
export function shortContentHash(hash: string): string {
  return hash.length > 12 ? `${hash.slice(0, 12)}…` : hash;
}

/** Katalogda "bu dosyaya kopyalandı" bilgisini tek yerde tutar. */
export function copiedLabel(template: ChecklistTemplateRow): string | null {
  if (template.copiedItemCount === 0) return null;
  return `${template.copiedItemCount} öğe dosyada`;
}
