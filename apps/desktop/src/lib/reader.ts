import type { AnchorLocationRow, PageBlock, PageRow, SearchHit } from "./api";

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
  /** Anchor kaydından gelen blok kutuları; arama sonucunda boştur. */
  quads: [number, number, number, number][];
};

export type ReaderSlice = { before: string; hit: string; after: string };

/** Unicode scalar aralığı. Backend offsetleri de scalardır; UTF-16 dilimlemesi
 *  emoji/aksanda kaymaz. */
export type OffsetRange = { start: number; end: number };

/** `parse_runs.warnings` JSON metnini güvenli çözer; bozuk kayıt boş liste olur
 *  ve arayüz uydurma uyarı göstermez. */
export function parseWarnings(raw: string | null | undefined): string[] {
  if (!raw) return [];
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((item): item is string => typeof item === "string");
  } catch {
    return [];
  }
}

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
    quads: [],
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
    quads: location.quads ?? [],
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
export function queryRange(text: string, query: string | null): OffsetRange | null {
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
  return { start, end: start + Array.from(needle).length };
}

export function querySlice(text: string, query: string | null): ReaderSlice | null {
  const range = queryRange(text, query);
  return range ? offsetSlice(text, range.start, range.end) : null;
}

/** Hedefin bu sayfadaki işaretlenecek aralığı (Unicode scalar). Anchor
 *  offset'i sayfa ile eşleşmiyorsa uygulanmaz; böylece yanlış sayfada yanlış
 *  alıntı boyanmaz. */
export function rangeForPage(
  text: string,
  target: ReaderTarget | null,
  page: PageRow,
): OffsetRange | null {
  if (!target) return null;
  if (target.startOffset !== null && target.endOffset !== null) {
    const samePage =
      (target.pageId !== null && target.pageId === page.id) ||
      (target.segmentId !== null && target.segmentId === page.segmentId);
    if (!samePage) return null;
    if (!Number.isInteger(target.startOffset) || !Number.isInteger(target.endOffset)) return null;
    if (target.startOffset < 0 || target.endOffset < target.startOffset) return null;
    if (target.endOffset > Array.from(text).length) return null;
    return { start: target.startOffset, end: target.endOffset };
  }
  return queryRange(text, target.query);
}

/** Hedefin bu sayfada işaretlenecek parçası. */
export function sliceForPage(
  text: string,
  target: ReaderTarget | null,
  page: PageRow,
): ReaderSlice | null {
  const range = rangeForPage(text, target, page);
  return range ? offsetSlice(text, range.start, range.end) : null;
}

/** Döndürme uygulandığında sayfanın görünen boyutu (90/270'te eksenler yer
 *  değiştirir). Koordinatlar döndürme öncesi uzaydadır. */
export function displayPageSize(
  width: number,
  height: number,
  rotation: number,
): { width: number; height: number } {
  const normalized = normalizeRotation(rotation);
  return normalized === 90 || normalized === 270
    ? { width: height, height: width }
    : { width, height };
}

export function normalizeRotation(rotation: number): number {
  if (!Number.isFinite(rotation)) return 0;
  const rounded = Math.round(rotation / 90) * 90;
  return ((rounded % 360) + 360) % 360;
}

/** Okuyucu yakınlaştırma kademeleri. Serbest oran yok: iki uç arasında
 *  adımlarla ilerlenir, metin okunabilir kalır. */
export const ZOOM_STEPS: readonly number[] = [0.75, 1, 1.25, 1.5, 2];

/** Yakınlaştırmayı geçerli aralığa kırpar; sayı değilse %100. */
export function clampZoom(value: number): number {
  if (!Number.isFinite(value)) return 1;
  const first = ZOOM_STEPS[0]!;
  const last = ZOOM_STEPS[ZOOM_STEPS.length - 1]!;
  return Math.min(Math.max(value, first), last);
}

/** Bir üst/alt kademeye geçer. Uçtaysa aynı değeri döndürür; arayüz düğmeyi
 *  pasif gösterir, sessizce taşmaz. */
export function nextZoom(zoom: number, direction: 1 | -1): number {
  const current = clampZoom(zoom);
  let index = 0;
  for (let i = 0; i < ZOOM_STEPS.length; i += 1) {
    if (Math.abs(ZOOM_STEPS[i]! - current) < Math.abs(ZOOM_STEPS[index]! - current)) index = i;
  }
  const target = index + direction;
  if (target < 0 || target >= ZOOM_STEPS.length) return ZOOM_STEPS[index]!;
  return ZOOM_STEPS[target]!;
}

/** Belgenin kendi döndürmesi ile kullanıcının görünüm döndürmesini toplar.
 *  Kayıtlı veri değişmez; toplam yalnız gösterimi etkiler. */
export function rotateView(pageRotation: number, viewRotation: number): number {
  return normalizeRotation(normalizeRotation(pageRotation) + normalizeRotation(viewRotation));
}

/** Sayfa etiketini fiziksel numaradan ayırır. Etiket yalnız PDF'te gerçekten
 *  tanımlıysa gösterilir; fiziksel numarayla aynıysa yinelenmez. */
export function pageLabelInfo(page: {
  pageIndex: number;
  pageLabel?: string | null;
}): { label: string | null; physical: string; showsLabel: boolean } {
  const raw = (page.pageLabel ?? "").trim();
  const physical = `Fiziksel sayfa ${page.pageIndex + 1}`;
  const label = raw.length > 0 ? raw : null;
  return {
    label,
    physical,
    showsLabel: label !== null && label !== String(page.pageIndex + 1),
  };
}

/** Yapısal belgede (DOCX/UDF) bir paragraf/bölüm girdisi. Aralıklar worker'ın
 *  gerçek bloklarından gelir; başlık bilgisi `kind` alanındandır, tahmin
 *  edilmez. `path` yoksa etiket uydurulmaz. */
export type StructureEntry = {
  index: number;
  start: number;
  end: number;
  text: string;
  path: string | null;
  kind: string | null;
  heading: boolean;
};

/** Yapısal sayfanın paragraf listesi. PDF/görüntü sayfasında boş döner:
 *  türev sayfa ile paragraf yapısı karıştırılmaz. */
export function structureEntries(page: PageRow): StructureEntry[] {
  if (page.coordinateSpace !== "structure") return [];
  const chars = Array.from(page.text);
  return page.blocks.map((block, index) => {
    const start = Math.max(0, Math.min(block.start, chars.length));
    const end = Math.max(start, Math.min(block.end, chars.length));
    return {
      index,
      start,
      end,
      text: chars.slice(start, end).join(""),
      path: block.path ?? null,
      kind: block.kind ?? null,
      heading: block.kind === "heading",
    };
  });
}

/** Yapısal girdi içinde vurgulanacak aralığı (girdiye göre) keser. Aralık
 *  girdiyle kesişmiyorsa tüm metin `before` olarak döner. */
export function entrySlice(
  entry: StructureEntry,
  range: OffsetRange | null,
): { before: string; hit: string; after: string } {
  const chars = Array.from(entry.text);
  if (!range) return { before: entry.text, hit: "", after: "" };
  const start = Math.max(range.start, entry.start);
  const end = Math.min(range.end, entry.end);
  if (end <= start) return { before: entry.text, hit: "", after: "" };
  return {
    before: chars.slice(0, start - entry.start).join(""),
    hit: chars.slice(start - entry.start, end - entry.start).join(""),
    after: chars.slice(end - entry.start).join(""),
  };
}

/** Döndürme öncesi uzaydaki kutuyu görünen (döndürülmüş) uzaya çevirir.
 *  PyMuPDF `rotation_matrix` ile aynı yön: saat yönünde. */
export function mapRectForDisplay(
  rect: [number, number, number, number],
  pageWidth: number,
  pageHeight: number,
  rotation: number,
): [number, number, number, number] {
  const [x0, y0, x1, y1] = rect;
  switch (normalizeRotation(rotation)) {
    case 90:
      return [pageHeight - y1, x0, pageHeight - y0, x1];
    case 180:
      return [pageWidth - x1, pageHeight - y1, pageWidth - x0, pageHeight - y0];
    case 270:
      return [y0, pageWidth - x1, y1, pageWidth - x0];
    default:
      return [x0, y0, x1, y1];
  }
}

/** Aralıkla kesişen blokların kutuları. Blok sınırları karakter aralığı
 *  taşır; satır düzeyinde bölme yapılmaz (satır offsetleri kayıtlı değil). */
export function blocksForRange(
  blocks: PageBlock[],
  range: OffsetRange | null,
): [number, number, number, number][] {
  const quads: [number, number, number, number][] = [];
  for (const block of blocks) {
    if (!block.bbox) continue;
    if (range && !(block.end > range.start && block.start < range.end)) continue;
    quads.push(block.bbox);
  }
  return quads;
}
