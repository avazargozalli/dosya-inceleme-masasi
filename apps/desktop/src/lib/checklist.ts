import type { ChecklistItemState, ChecklistTemplateRow } from "./api";
import { t } from "./i18n";

/**
 * Durum etiketi. `missing` bilinçli olarak "incelenen kapsamda bulunamadı"
 * der: liste, dosyanın tamamında belge olmadığı iddiasını üretmez (AI-07).
 *
 * Metinler `lib/i18n.ts` içinde (F04-T08): etiket ve yardım cümlesi tek yerde.
 */
export const CHECKLIST_STATE_LABEL: Record<ChecklistItemState, string> = {
  open: t("checklists.stateLabel.open"),
  present: t("checklists.stateLabel.present"),
  missing: t("checklists.stateLabel.missing"),
  not_applicable: t("checklists.stateLabel.not_applicable"),
};

export const CHECKLIST_STATE_HELP: Record<ChecklistItemState, string> = {
  open: t("checklists.stateHelp.open"),
  present: t("checklists.stateHelp.present"),
  missing: t("checklists.stateHelp.missing"),
  not_applicable: t("checklists.stateHelp.not_applicable"),
};

export const MISSING_SCOPE_NOTE = t("checklists.missingScopeNote");

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
    return links.length === 0 ? t("checklists.rule.presentNeedsDocument") : null;
  }
  if (input.state === "missing") {
    if (links.length > 0) return t("checklists.rule.linksOnlyPresent");
    return note.length === 0 ? t("checklists.rule.missingNeedsScope") : null;
  }
  if (input.state === "not_applicable") {
    if (links.length > 0) return t("checklists.rule.linksOnlyPresent");
    return note.length === 0 ? t("checklists.rule.notApplicableNeedsReason") : null;
  }
  return links.length > 0 ? t("checklists.rule.openClearsLinks") : null;
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
  const parts = [
    t("checklists.license.version", { version: template.version }),
    template.licenseId ?? t("checklists.licenseUnknown"),
  ];
  if (template.reviewDate) {
    parts.push(t("checklists.license.review", { date: template.reviewDate }));
  }
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
  return t("checklists.copiedCount", { count: template.copiedItemCount });
}
