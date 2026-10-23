import type { AnchorLocationRow, PageRow, SearchHit } from "./api";

/** Okuyucunun açılacağı yer. `versionId` zorunludur: sonuç/bağ, doğru
 *  sürümü açar; "en son sürüm" tahmini yapılmaz (F07 kabul testi). */
export type ReaderTarget = {
  versionId: string;
  documentName: string | null;
  pageId: string | null;
  segmentId: string | null;
  pageIndex: number | null;
  startOffset: number | null;
  endOffset: number | null;
  quote: string | null;
  /** Arama sonucunda offset yoktur; sayfadaki ilk sorgu geçişi işaretlenir. */
  query: string | null;
  anchorId: string | null;
  quoteIntact: boolean | null;
};

export type ReaderSlice = { before: string; hit: string; after: string };

/** Arama sonucundan hedef üretir. Sürüm kimliği taşınmayan eski sonuç
 *  okuyucuda açılamaz; `null` döner ve arayüz nedenini yazar. */
export function targetFromHit(hit: SearchHit, query: string): ReaderTarget | null {
  if (!hit.documentVersionId) return null;
  return {
    versionId: hit.documentVersionId,
    documentName: hit.documentName,
    pageId: hit.pageId,
    segmentId: hit.segmentId,
    pageIndex: hit.pageIndex,
    startOffset: null,
    endOffset: null,
    quote: null,
    query: query.trim() || null,
    anchorId: null,
    quoteIntact: null,
  };
}

/** Kaynak bağdan (anchor) hedef üretir; alıntı koptuysa bunu taşır. */
export function targetFromAnchor(location: AnchorLocationRow): ReaderTarget {
  return {
    versionId: location.documentVersionId,
    documentName: location.documentName,
    pageId: location.pageId,
    segmentId: location.segmentId,
    pageIndex: location.pageIndex,
    startOffset: location.startOffset,
    endOffset: location.endOffset,
    quote: location.exactQuote,
    query: null,
    anchorId: location.anchorId,
    quoteIntact: location.quoteIntact,
  };
}

/** Sayfa indeksini çözer: önce sayfa kimliği, sonra segment, en son fiziksel
 *  indeks. Aralık dışı indeks kırpılır; boş belgede -1 döner. */
export function resolvePageIndex(pages: PageRow[], target: ReaderTarget | null): number {
  if (pages.length === 0) return -1;
  if (target) {
    if (target.pageId) {
      const byId = pages.findIndex((p) => p.id === target.pageId);
      if (byId >= 0) return byId;
    }
    if (target.segmentId) {
      const bySegment = pages.findIndex((p) => p.segmentId === target.segmentId);
      if (bySegment >= 0) return bySegment;
    }
    if (target.pageIndex !== null) {
      return Math.min(Math.max(target.pageIndex, 0), pages.length - 1);
    }
  }
  return 0;
}

/** Unicode scalar aralığını metinden keser. Backend offset'leri scalardır;
 *  UTF-16 dilimlemesi emoji/aksanda kaymaz. Aralık geçersizse `null`. */
export function offsetSlice(text: string, start: number | null, end: number | null): ReaderSlice | null {
  if (start === null || end === null) return null;
  if (!Number.isInteger(start) || !Number.isInteger(end)) return null;
  const chars = Array.from(text);
  if (start < 0 || end < start || end > chars.length) return null;
  return {
    before: chars.slice(0, start).join(""),
    hit: chars.slice(start, end).join(""),
    after: chars.slice(end).join(""),
  };
}

/** Sayfadaki sorgu geçişini işaretler. Önce birebir, bulunamazsa Türkçe
 *  küçük harfe çevrilmiş metinde arar; uzunluk değişirse işaretleme yapmaz. */
export function querySlice(text: string, query: string | null): ReaderSlice | null {
  const needle = (query ?? "").trim();
  if (!needle) return null;
  let index = text.indexOf(needle);
  if (index < 0) {
    const lowered = text.toLocaleLowerCase("tr");
    const loweredNeedle = needle.toLocaleLowerCase("tr");
    if (lowered.length === text.length && loweredNeedle.length === needle.length) {
      index = lowered.indexOf(loweredNeedle);
    }
  }
  if (index < 0) return null;
  const start = Array.from(text.slice(0, index)).length;
  return offsetSlice(text, start, start + Array.from(needle).length);
}

/** Hedefin bu sayfada işaretlenecek parçası. Anchor offset'i sayfa ile
 *  eşleşmiyorsa uygulanmaz; böylece yanlış sayfada yanlış alıntı boyanmaz. */
export function sliceForPage(text: string, target: ReaderTarget | null, page: PageRow): ReaderSlice | null {
  if (!target) return null;
  if (target.startOffset !== null && target.endOffset !== null) {
    const samePage =
      (target.pageId !== null && target.pageId === page.id) ||
      (target.segmentId !== null && target.segmentId === page.segmentId);
    if (!samePage) return null;
    return offsetSlice(text, target.startOffset, target.endOffset);
  }
  return querySlice(text, target.query);
}
