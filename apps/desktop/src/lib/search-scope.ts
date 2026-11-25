import type { PrecedentSearchHit, PrecedentSearchPage } from "./api";
import { t } from "./i18n";

/**
 * Arama kapsamı (F12-T10). İki kapsam **birbirine karışmaz**: dosya belgeleri
 * aktif dosyanın segmentlerini, emsal kütüphanesi yalnız emsal kayıtlarının
 * tam metinlerini tarar.
 *
 * Metinler F04-T08 gereği `lib/i18n.ts`ten gelir (modül düzeyi sabitler import
 * anında çözülür — tek dilli ürün kararı).
 */
export type SearchScope = "case_documents" | "precedent_library";

export const SEARCH_SCOPE_LABELS: Record<SearchScope, string> = {
  case_documents: t("searchScope.label.case_documents"),
  precedent_library: t("searchScope.label.precedent_library"),
};

/** Kapsamın ne taradığını söyleyen kısa uyarı; arayüzde kalıcı gösterilir. */
export function scopeNotice(scope: SearchScope): string {
  if (scope === "precedent_library") {
    return t("searchScope.notice.precedent_library");
  }
  return t("searchScope.notice.case_documents");
}

/** Emsal sonucunun künye satırı; yalnız var olan alanlar yazılır. */
export function precedentCitation(hit: PrecedentSearchHit): string {
  const parts: string[] = [];
  const court = hit.court.trim();
  if (court) {
    const chamber = hit.chamber?.trim();
    parts.push(chamber ? `${court} ${chamber}` : court);
  }
  if (hit.docketNo?.trim()) {
    parts.push(t("searchScope.citationDocket", { no: hit.docketNo.trim() }));
  }
  if (hit.decisionNo?.trim()) {
    parts.push(t("searchScope.citationDecision", { no: hit.decisionNo.trim() }));
  }
  if (hit.decisionDate?.trim()) parts.push(hit.decisionDate.trim());
  return parts.length > 0 ? parts.join(", ") : t("searchScope.citationMissing");
}

export function precedentMatchLabel(hit: PrecedentSearchHit): string {
  return hit.matchSource === "metadata"
    ? t("searchScope.matchSource.metadata")
    : t("searchScope.matchSource.text");
}

/** Sonuç özeti: kaç eşleşme, kaç tam metin tarandı, kaç kayıt metinsiz. */
export function precedentSearchSummary(page: PrecedentSearchPage): string {
  const base = t("searchScope.summary.base", {
    matched: page.matched,
    scanned: page.scanned,
  });
  if (page.withoutText > 0) {
    return t("searchScope.summary.withoutText", { base, count: page.withoutText });
  }
  return base;
}

/** Kopyalanacak alıntı metni: künye + pencere; tam metin olduğu iddia edilmez. */
export function precedentSnippetForCopy(hit: PrecedentSearchHit): string {
  return t("searchScope.snippetForCopy", {
    citation: precedentCitation(hit),
    snippet: hit.snippet,
    version: hit.versionId.slice(0, 8),
  });
}

/** Snippet içinde işaretlenecek aralık; snippet'in **kendi** kod noktası indeksi. */
export type SnippetSpan = { start: number; length: number };

/**
 * F12-T14v: kütüphane vurgusu artık birebir `indexOf` değil, eşleştiricinin
 * **ölçtüğü kaynak aralığıdır** (`matchStart`/`matchLength`/`snippetStart`).
 * Böylece `İfade`/`Esnek` köprüsüyle (noktalama: `kredi-kartı` ↔
 * `kredi kartı`) ya da satır sonu tiresi birleştirmesiyle (`bedel-\nlini` ↔
 * `bedellini`) bulunan eşleşme de görünür işaretlenir; birebir arama bu
 * sonuçlarda sessizce ıskalıyordu.
 *
 * Konumlar **kaynak metnin** kod noktası indeksidir; snippet `snippetStart`ten
 * başlar ve metnin ortasından alındıysa başına `…` eklenir (çekirdek kuralı:
 * `snippet_window`). Eşleşme pencereyle kesişmiyorsa `null` döner — uydurma
 * vurgu yok; pencere sonunu taşan aralık **görünen kısmıyla** işaretlenir.
 */
export function snippetHighlightSpan(hit: PrecedentSearchHit): SnippetSpan | null {
  const { matchStart, matchLength, snippetStart } = hit;
  if (matchStart === null || matchLength === null || matchLength <= 0) return null;
  const chars = Array.from(hit.snippet);
  // `…` yalnız pencere metnin ortasından başladığında eklenir; kaynak metnin
  // kendi `…`si ile karışmasın diye iki koşul birlikte aranır.
  const lead = snippetStart > 0 && chars[0] === "…" ? 1 : 0;
  const start = matchStart - snippetStart + lead;
  if (start < 0 || start >= chars.length) return null;
  return { start, length: Math.min(matchLength, chars.length - start) };
}
