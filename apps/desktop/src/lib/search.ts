import type { SearchFilters, SearchHit, SearchMode, SearchPage } from "./api";

/**
 * Arama filtresi yardımcıları (F07-T03/T05).
 *
 * Buradaki kararlar çekirdekle aynı olmalı: etiketler küçük harfe iner,
 * `ocrStatus: "any"` filtre sayılmaz, "daha fazla" yalnız sunucunun verdiği
 * imleçle çağrılır. Arayüz "kaç sonuç var" iddiasını yalnız sunucudan gelen
 * sayıyla kurar; tahmin üretmez.
 */

export const EMPTY_SEARCH_FILTERS: SearchFilters = {
  documentTypes: [],
  dateRole: null,
  tags: [],
  ocrStatus: "any",
};

export const DATE_ROLE_LABELS: Record<string, string> = {
  event: "Olay",
  document: "Belge",
  notification: "Tebliğ",
  contract: "Sözleşme",
  hearing: "Duruşma",
  deadline_candidate: "Süre adayı",
  unknown: "Bilinmiyor",
};

export const OCR_STATUS_LABELS: Record<string, string> = {
  any: "Tümü",
  ocr: "OCR ile çıkarıldı",
  native: "Belgeden (yerel) metin",
};

/** `pages.text_origin` → okunur etiket. Bilinmeyen değer olduğu gibi gösterilir. */
export function textOriginLabel(origin: string | null): string {
  switch (origin) {
    case "native":
      return "Yerel metin";
    case "ocr":
      return "OCR";
    case "mixed":
      return "Karma (OCR + metin)";
    case "none":
      return "Metin yok";
    default:
      return "Kaynak bilinmiyor";
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
    out.push(`Belge türü: ${canonical.documentTypes.join(", ")}`);
  }
  if (canonical.dateRole) {
    out.push(`Tarih rolü: ${DATE_ROLE_LABELS[canonical.dateRole] ?? canonical.dateRole}`);
  }
  if (canonical.tags.length > 0) {
    out.push(`Etiket (hepsi): ${canonical.tags.join(", ")}`);
  }
  if (canonical.ocrStatus) {
    out.push(`Metin kaynağı: ${OCR_STATUS_LABELS[canonical.ocrStatus] ?? canonical.ocrStatus}`);
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
  const shown = `${page.returned} sonuç gösteriliyor`;
  if (page.total === null) {
    return page.hasMore ? `${shown} · devamı var` : shown;
  }
  return `${page.total} eşleşme · ${shown}`;
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
  if (probe.status === "counting") return "Eşleşme sayısı hesaplanıyor…";
  if (probe.status === "failed") return `Eşleşme sayısı alınamadı: ${probe.message}`;
  if (probe.resolvedQuery !== page.resolvedQuery || probe.mode !== page.mode) {
    return "Sayı önceki sorguya ait; yeni aramada tazelenir.";
  }
  return `${probe.total} eşleşme · sayım ${Math.round(probe.tookMs)} ms sürdü`;
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
  auto: "Otomatik",
  exact: "Tam",
  flexible: "Esnek",
  phrase: "İfade",
  docket: "E./K. numarası",
};

export const SEARCH_MODE_HINTS: Record<Exclude<SearchMode, "auto">, string> = {
  exact: "Türkçe karakterler korunur (ı ≠ i, ş ≠ s); kelimeler bitişik geçmeli, son kelime önek olabilir.",
  flexible: "Türkçe karakterler sadeleşir (ı→i, ş→s); noktalama önemsiz, son kelime önek olabilir.",
  phrase: "Kelimeler aynı sırada; araya noktalama girebilir, son kelime tam olmalı.",
  docket: "2024/123 E. biçimi; baştaki sıfırlar yok sayılır, E./K. rolü eşleşir.",
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
  const prefix = mode === "auto" ? `Otomatik: ${SEARCH_MODE_LABELS[resolved]} — ` : "";
  return `${prefix}${SEARCH_MODE_HINTS[resolved]}`;
}
