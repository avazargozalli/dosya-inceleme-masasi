import type { SearchFilters, SearchHit, SearchMode, SearchPage } from "./api";
import { t } from "./i18n";

/**
 * Arama filtresi yardımcıları (F07-T03/T05).
 *
 * Buradaki kararlar çekirdekle aynı olmalı: etiketler küçük harfe iner,
 * `ocrStatus: "any"` filtre sayılmaz, "daha fazla" yalnız sunucunun verdiği
 * imleçle çağrılır. Arayüz "kaç sonuç var" iddiasını yalnız sunucudan gelen
 * sayıyla kurar; tahmin üretmez.
 *
 * Metinler F04-T08 gereği `lib/i18n.ts`ten gelir (modül düzeyi sabitler import
 * anında çözülür — tek dilli ürün kararı).
 */

export const EMPTY_SEARCH_FILTERS: SearchFilters = {
  documentTypes: [],
  dateRole: null,
  tags: [],
  ocrStatus: "any",
};

export const DATE_ROLE_LABELS: Record<string, string> = {
  event: t("searchFilters.dateRole.event"),
  document: t("searchFilters.dateRole.document"),
  notification: t("searchFilters.dateRole.notification"),
  contract: t("searchFilters.dateRole.contract"),
  hearing: t("searchFilters.dateRole.hearing"),
  deadline_candidate: t("searchFilters.dateRole.deadline_candidate"),
  unknown: t("searchFilters.dateRole.unknown"),
};

export const OCR_STATUS_LABELS: Record<string, string> = {
  any: t("searchFilters.ocrStatus.any"),
  ocr: t("searchFilters.ocrStatus.ocr"),
  native: t("searchFilters.ocrStatus.native"),
};

/** `pages.text_origin` → okunur etiket. Bilinmeyen değer olduğu gibi gösterilir. */
export function textOriginLabel(origin: string | null): string {
  switch (origin) {
    case "native":
      return t("searchFilters.textOrigin.native");
    case "ocr":
      return t("searchFilters.textOrigin.ocr");
    case "mixed":
      return t("searchFilters.textOrigin.mixed");
    case "none":
      return t("searchFilters.textOrigin.none");
    default:
      return t("searchFilters.textOrigin.unknown");
  }
}

function normalizeTag(tag: string): string {
  // Çekirdekteki `normalize_tag` ile aynı: boşluk sadeleşir, Unicode küçük
  // harfe iner, `İ` küçültmesinin bıraktığı birleşik nokta atılır.
  return tag
    .split(/\s+/)
    .filter(Boolean)
    .join(" ")
    .toLowerCase()
    .replace(/\u0307/g, "");
}

/**
 * Filtreyi karşılaştırılabilir kanonik biçime çevirir: boşluklar sadeleşir,
 * etiketler küçülür ve sıralanır, `any`/boş OCR değeri `null` olur.
 */
export function canonicalFilters(filters: SearchFilters): SearchFilters {
  const documentTypes = Array.from(
    new Set(filters.documentTypes.map((value) => value.trim()).filter((value) => value.length > 0)),
  ).sort();
  const tags = Array.from(
    new Set(filters.tags.map(normalizeTag).filter((value) => value.length > 0)),
  ).sort();
  const ocrStatus =
    filters.ocrStatus && filters.ocrStatus !== "any" ? filters.ocrStatus : null;
  return {
    documentTypes,
    tags,
    ocrStatus,
    dateRole: filters.dateRole?.trim() ? filters.dateRole.trim() : null,
  };
}

export function filtersEqual(left: SearchFilters, right: SearchFilters): boolean {
  return JSON.stringify(canonicalFilters(left)) === JSON.stringify(canonicalFilters(right));
}

export function isFiltered(filters: SearchFilters): boolean {
  const canonical = canonicalFilters(filters);
  return (
    canonical.documentTypes.length > 0 ||
    canonical.tags.length > 0 ||
    canonical.dateRole !== null ||
    canonical.ocrStatus !== null
  );
}

/** Etkin filtre sayısı; rozet metni için. */
export function activeFilterCount(filters: SearchFilters): number {
  const canonical = canonicalFilters(filters);
  return (
    canonical.documentTypes.length +
    canonical.tags.length +
    (canonical.dateRole ? 1 : 0) +
    (canonical.ocrStatus ? 1 : 0)
  );
}

/** Etkin filtreleri okunur tek satırlık etiketlere çevirir. */
export function describeFilters(filters: SearchFilters): string[] {
  const canonical = canonicalFilters(filters);
  const out: string[] = [];
  if (canonical.documentTypes.length > 0) {
    out.push(
      t("searchFilters.describe.documentTypes", { types: canonical.documentTypes.join(", ") }),
    );
  }
  if (canonical.dateRole) {
    out.push(
      t("searchFilters.describe.dateRole", {
        role: DATE_ROLE_LABELS[canonical.dateRole] ?? canonical.dateRole,
      }),
    );
  }
  if (canonical.tags.length > 0) {
    out.push(t("searchFilters.describe.tags", { tags: canonical.tags.join(", ") }));
  }
  if (canonical.ocrStatus) {
    out.push(
      t("searchFilters.describe.ocrStatus", {
        label: OCR_STATUS_LABELS[canonical.ocrStatus] ?? canonical.ocrStatus,
      }),
    );
  }
  return out;
}

/** Listede değer varsa çıkarır, yoksa ekler (çip düğmeleri). */
export function toggleValue(values: string[], value: string): string[] {
  return values.includes(value) ? values.filter((item) => item !== value) : [...values, value];
}

/**
 * Sayfa özeti: toplam yalnız sunucudan geldiyse yazılır; gösterilen sayı her
 * zaman gerçek `returned` değeridir.
 */
export function resultSummary(page: SearchPage): string {
  const shown = t("searchFilters.result.shown", { returned: page.returned });
  if (page.total === null) {
    return page.hasMore ? t("searchFilters.result.shownMore", { shown }) : shown;
  }
  return t("searchFilters.result.withTotal", { total: page.total, shown });
}

export function canLoadMore(page: SearchPage | null): boolean {
  return page !== null && page.hasMore && page.nextCursor !== null;
}

// ---------------------------------------------------------------------------
// Ertelenmiş toplam sayım (F15)
// ---------------------------------------------------------------------------

/**
 * Arayüzde toplam sayının durumu. Sayım sayfayı bekletmediği için ayrı bir
 * durum makinesi gerekir: kullanıcı önce sonuçları görür, sayı sonra gelir.
 */
export type TotalProbe =
  | { status: "idle" }
  | { status: "counting" }
  | { status: "done"; total: number; tookMs: number; resolvedQuery: string; mode: string }
  | { status: "failed"; message: string };

export const TOTAL_PROBE_IDLE: TotalProbe = { status: "idle" };

/**
 * Toplam sayı satırı. Üç kural:
 *
 * 1. Sunucu toplamı sayfa yanıtında geldiyse (`page.total`) bu satır yazılmaz —
 *    iki kaynak aynı anda gösterilmez.
 * 2. Sayı **sayfayla aynı sorguya** ait değilse (kip/çözümlenmiş metin farklı)
 *    gösterilmez: bayat sayı yeni sonuçların toplamı sanılmasın.
 * 3. Sayı gelmediyse "hesaplanıyor" ya da gerçek hata metni yazılır; sayı
 *    uydurulmaz.
 */
export function totalSummary(page: SearchPage | null, probe: TotalProbe): string | null {
  if (!page) return null;
  if (page.total !== null) return null;
  if (probe.status === "idle") return null;
  if (probe.status === "counting") return t("searchFilters.total.counting");
  if (probe.status === "failed") {
    return t("searchFilters.total.failed", { message: probe.message });
  }
  if (probe.resolvedQuery !== page.resolvedQuery || probe.mode !== page.mode) {
    return t("searchFilters.total.staleQuery");
  }
  return t("searchFilters.total.done", {
    total: probe.total,
    ms: Math.round(probe.tookMs),
  });
}

/**
 * Aday penceresi özeti (F15). "Kaç aday doğrulandı" **gerçek ölçümdür**:
 * sayfa dolduğunda taramanın kesildiğini açıkça söyler, kesilmediyse adayların
 * tükendiğini. Ölçüm yoksa satır gösterilmez (sayı uydurulmaz).
 */
export function verificationSummary(page: SearchPage | null): string | null {
  if (!page) return null;
  const verification = page.verification;
  if (!verification || verification.candidatesExamined <= 0) return null;
  const base = t("searchFilters.verification.base", {
    candidates: verification.candidatesExamined,
    ms: Math.round(verification.scanMs),
  });
  return verification.stoppedEarly
    ? t("searchFilters.verification.stoppedEarly", { base })
    : t("searchFilters.verification.exhausted", { base });
}

/**
 * "Daha fazla" sonuçlarını öncekiyle birleştirir. Aynı segment iki kez
 * görünmez; sunucu imleci doğru olsa bile arayüz kendi tarafında tekilleştirir.
 */
export function mergeHits(previous: SearchHit[], next: SearchHit[]): SearchHit[] {
  const seen = new Set(previous.map((hit) => hit.segmentId));
  const merged = [...previous];
  for (const hit of next) {
    if (!seen.has(hit.segmentId)) {
      merged.push(hit);
      seen.add(hit.segmentId);
    }
  }
  return merged;
}

/**
 * Kullanıcıya gösterilecek "sonuçlar eski filtreye ait" uyarısı: yeni arama
 * çalıştırılmadan önce filtre değiştiyse true. Sessizce eski sonucu yeni
 * filtreyle göstermek yanlış eşleşme izlenimi verir.
 */
export function filtersStale(page: SearchPage | null, draft: SearchFilters): boolean {
  if (!page) return false;
  return !filtersEqual(page.filters, draft);
}

// ---------------------------------------------------------------------------
// Arama kipi (F07-T02)
// ---------------------------------------------------------------------------

export const SEARCH_MODE_LABELS: Record<SearchMode, string> = {
  auto: t("searchFilters.mode.auto"),
  exact: t("searchFilters.mode.exact"),
  flexible: t("searchFilters.mode.flexible"),
  phrase: t("searchFilters.mode.phrase"),
  docket: t("searchFilters.mode.docket"),
};

export const SEARCH_MODE_HINTS: Record<Exclude<SearchMode, "auto">, string> = {
  exact: t("searchFilters.modeHint.exact"),
  flexible: t("searchFilters.modeHint.flexible"),
  phrase: t("searchFilters.modeHint.phrase"),
  docket: t("searchFilters.modeHint.docket"),
};

/** Yıl/numara: çekirdeğin `parse_docket` kuralıyla aynı (4 hane yıl, 1–7 hane no). */
const DOCKET_PATTERN = /(?:^|\D)(\d{4})\s*\/\s*(\d{1,7})(?:\s*([EK])\.?)?/i;
const QUOTED_PATTERN = /^["“](.+)["”]$/s;

export function isDocketQuery(text: string): boolean {
  return DOCKET_PATTERN.test(text.trim());
}

/** Tırnakla sarılmış metni içeriğe çevirir; sarılmamışsa null. */
export function stripWrappingQuotes(text: string): string | null {
  const match = QUOTED_PATTERN.exec(text.trim());
  const inner = match?.[1]?.trim();
  return inner && inner.length > 0 ? inner : null;
}

/**
 * Çekirdeğin `auto` çözümüyle aynı kural: tırnaklı metin → ifade,
 * E./K. numarası → docket, diğerleri → esnek. Sunucu da aynı sırayı uygular.
 */
export function resolveSearchMode(
  text: string,
  mode: SearchMode,
): Exclude<SearchMode, "auto"> {
  if (mode !== "auto") return mode;
  if (stripWrappingQuotes(text) !== null) return "phrase";
  if (isDocketQuery(text)) return "docket";
  return "flexible";
}

/** Ne aranacağını önizler; sunucu kararıyla aynı dili kullanır. */
export function searchModeHint(text: string, mode: SearchMode): string {
  const resolved = resolveSearchMode(text, mode);
  const prefix =
    mode === "auto"
      ? `${t("searchFilters.modeHint.autoPrefix", { label: SEARCH_MODE_LABELS[resolved] })} `
      : "";
  return `${prefix}${SEARCH_MODE_HINTS[resolved]}`;
}
