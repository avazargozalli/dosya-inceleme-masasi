import type { PrecedentSearchHit, PrecedentSearchPage } from "./api";

/**
 * Arama kapsamı (F12-T10). İki kapsam **birbirine karışmaz**: dosya belgeleri
 * aktif dosyanın segmentlerini, emsal kütüphanesi yalnız emsal kayıtlarının
 * tam metinlerini tarar.
 */
export type SearchScope = "case_documents" | "precedent_library";

export const SEARCH_SCOPE_LABELS: Record<SearchScope, string> = {
  case_documents: "Dosya belgeleri",
  precedent_library: "Emsal kütüphanesi",
};

/** Kapsamın ne taradığını söyleyen kısa uyarı; arayüzde kalıcı gösterilir. */
export function scopeNotice(scope: SearchScope): string {
  if (scope === "precedent_library") {
    return "Kapsam: emsal kütüphanesi. Sonuçlar aktif dosyanın belgelerinden gelmez; dosya aramasına karışmaz ve delil sayılmaz.";
  }
  return "Kapsam: yalnız aktif dosyanın belgeleri. Emsal kütüphanesi bu aramaya dahil edilmez.";
}

/** Emsal sonucunun künye satırı; yalnız var olan alanlar yazılır. */
export function precedentCitation(hit: PrecedentSearchHit): string {
  const parts: string[] = [];
  const court = hit.court.trim();
  if (court) {
    const chamber = hit.chamber?.trim();
    parts.push(chamber ? `${court} ${chamber}` : court);
  }
  if (hit.docketNo?.trim()) parts.push(`E. ${hit.docketNo.trim()}`);
  if (hit.decisionNo?.trim()) parts.push(`K. ${hit.decisionNo.trim()}`);
  if (hit.decisionDate?.trim()) parts.push(hit.decisionDate.trim());
  return parts.length > 0 ? parts.join(", ") : "Künye eksik";
}

export function precedentMatchLabel(hit: PrecedentSearchHit): string {
  return hit.matchSource === "metadata" ? "künye numarası" : "karar metni";
}

/** Sonuç özeti: kaç eşleşme, kaç tam metin tarandı, kaç kayıt metinsiz. */
export function precedentSearchSummary(page: PrecedentSearchPage): string {
  const base = `${page.matched} eşleşme · ${page.scanned} tam metin tarandı`;
  if (page.withoutText > 0) {
    return `${base} · ${page.withoutText} kayıt tam metinsiz (aranmadı)`;
  }
  return base;
}

/** Kopyalanacak alıntı metni: künye + pencere; tam metin olduğu iddia edilmez. */
export function precedentSnippetForCopy(hit: PrecedentSearchHit): string {
  return `${precedentCitation(hit)}\n…${hit.snippet}\n(kaynak: emsal kütüphanesi, sürüm ${hit.versionId.slice(0, 8)})`;
}
