import type { SearchFilters, SearchHit, SearchPage } from "./api";

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
