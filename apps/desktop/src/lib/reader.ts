import type { AnchorLocationRow, DocumentRow, PageBlock, PageRow, SearchHit } from "./api";
import { formatSize } from "./format";
import { t } from "./i18n";

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
  // Metin **sözlükten** gelir: aynı etiket DualReader'da `readerUi.physicalPage`
  // ile de yazılıyordu (iki kaynak). Gömülü hâli harf kapısından kaçar çünkü
  // "Fiziksel sayfa" Türkçeye özgü harf (ığüşöç) taşımaz.
  const physical = t("readerUi.physicalPage", { page: page.pageIndex + 1 });
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

/** Önizleme görüntüsü için data URL: baytlar zaten çekirdekten base64 gelir. */
export function previewDataUrl(pngBase64: string): string {
  return `data:image/png;base64,${pngBase64}`;
}

export type PreviewMeta = { cached: boolean; byteSize: number; dpi: number };

/** Önizleme alt yazısı: kaynağı (şifreli önbellek/yeni) ve boyutunu dürüstçe yazar. */
export function previewCaption(meta: PreviewMeta): string {
  const source = meta.cached ? t("readerLib.cachedSource") : t("readerLib.freshSource");
  return t("readerLib.caption", { source, size: formatSize(meta.byteSize), dpi: meta.dpi });
}

/** Paneldeki metin seçimi: sayfa metnine göre Unicode scalar aralığı. */
export type TextSelection = { quote: string; start: number; end: number };

/** Bir okuyucu panelinin görünüm durumu. Yalnız gösterimi etkiler; kayıtlı
 *  sayfa verisi, `pages.rotation` ve alıntı offsetleri değişmez. */
export type ReaderView = {
  page: number;
  zoom: number;
  rotation: number;
  selection: TextSelection | null;
};

export const EMPTY_VIEW: ReaderView = { page: 0, zoom: 1, rotation: 0, selection: null };

/** Panel başına hatırlanan belge sürümü sayısı. Sınır, uzun incelemede
 *  belleğin sınırsız büyümesini engeller. */
export const VIEW_MEMORY_LIMIT = 8;

/** Panelin sürüm başına görünüm belleği; sıra eskiden yeniye doğrudur. */
export type ViewMemory = ReadonlyArray<{ versionId: string; view: ReaderView }>;

function cloneView(view: ReaderView): ReaderView {
  return {
    page: view.page,
    zoom: view.zoom,
    rotation: view.rotation,
    selection: view.selection ? { ...view.selection } : null,
  };
}

/** Sürümün görünümünü hatırlar. Aynı sürüm yeniden kaydedilirse kayıt
 *  güncellenir ve sona taşınır; sınır aşılırsa en eski kayıt düşer. */
export function rememberView(
  memory: ViewMemory,
  versionId: string,
  view: ReaderView,
  limit: number = VIEW_MEMORY_LIMIT,
): ViewMemory {
  if (!versionId) return memory;
  const bounded = Math.max(1, Math.floor(limit));
  const rest = memory.filter((entry) => entry.versionId !== versionId);
  const next = [...rest, { versionId, view: cloneView(view) }];
  return next.length > bounded ? next.slice(next.length - bounded) : next;
}

/** Sürümün hatırlanan görünümü; yoksa `null` (uydurma konum üretilmez). */
export function recallView(memory: ViewMemory, versionId: string | null): ReaderView | null {
  if (!versionId) return null;
  const found = memory.find((entry) => entry.versionId === versionId);
  return found ? cloneView(found.view) : null;
}

/** Kayıtlı seçimi sayfa metnine karşı doğrular. Alıntı, kaydedildiği
 *  offset'te birebir durmuyorsa seçim geri yüklenmez: yeniden ayrıştırma
 *  metni kaydırmışsa yanlış alıntı işaretlenmez. */
export function validSelection(text: string, selection: TextSelection | null): TextSelection | null {
  if (!selection) return null;
  const { quote, start, end } = selection;
  if (!Number.isInteger(start) || !Number.isInteger(end)) return null;
  if (start < 0 || end <= start) return null;
  const chars = Array.from(text);
  if (end > chars.length) return null;
  const actual = chars.slice(start, end).join("");
  return actual === quote ? { quote, start, end } : null;
}

/** Panelde sayfa gezinme tuşları. Yalnız **Alt** ile çalışır: düz PageUp/
 *  PageDown ve ok tuşları metin alanının kendi kaydırmasını bozmaz. Değişiklik
 *  yoksa `null` döner ve tuş yutulmaz. */
export function pageForKey(
  key: string,
  modifiers: { alt?: boolean; ctrl?: boolean; meta?: boolean; shift?: boolean },
  page: number,
  pageCount: number,
): number | null {
  if (pageCount <= 0) return null;
  if (!modifiers.alt || modifiers.ctrl || modifiers.meta || modifiers.shift) return null;
  const current = Math.min(Math.max(page, 0), pageCount - 1);
  const last = pageCount - 1;
  switch (key) {
    case "PageDown":
    case "ArrowRight":
    case "ArrowDown":
      return current >= last ? null : current + 1;
    case "PageUp":
    case "ArrowLeft":
    case "ArrowUp":
      return current <= 0 ? null : current - 1;
    case "Home":
      return current === 0 ? null : 0;
    case "End":
      return current === last ? null : last;
    default:
      return null;
  }
}

/** Sayfa metnini işaret ve hedef aralıklarına göre parçalar. Dönen parçalar
 *  birleştirildiğinde metnin **tamamı** elde edilir (seçim offsetleri kaymaz).
 *  Kesişen aralıklar birleşir; hedef aralık işaretten önceliklidir. */
export type TextSegment = { text: string; marked: boolean; targeted: boolean };

export function textSegments(
  text: string,
  marks: OffsetRange[],
  target: OffsetRange | null,
): TextSegment[] {
  const chars = Array.from(text);
  const length = chars.length;
  const bounds = new Set<number>([0, length]);
  const clamp = (range: OffsetRange): OffsetRange | null => {
    const start = Math.max(0, Math.min(range.start, length));
    const end = Math.max(0, Math.min(range.end, length));
    return end > start ? { start, end } : null;
  };
  const cleanMarks = marks.map(clamp).filter((range): range is OffsetRange => range !== null);
  const cleanTarget = target ? clamp(target) : null;
  for (const range of cleanMarks) {
    bounds.add(range.start);
    bounds.add(range.end);
  }
  if (cleanTarget) {
    bounds.add(cleanTarget.start);
    bounds.add(cleanTarget.end);
  }
  const points = [...bounds].sort((a, b) => a - b);
  const out: TextSegment[] = [];
  for (let i = 0; i < points.length - 1; i += 1) {
    const start = points[i]!;
    const end = points[i + 1]!;
    if (end === start) continue;
    const inTarget = cleanTarget ? start >= cleanTarget.start && end <= cleanTarget.end : false;
    const inMark = cleanMarks.some((range) => start >= range.start && end <= range.end);
    out.push({
      text: chars.slice(start, end).join(""),
      marked: inMark,
      targeted: inTarget,
    });
  }
  if (out.length === 0) return [{ text, marked: false, targeted: false }];
  return out;
}

/** Bu sayfaya ait işaretlerin metin aralıkları. Bölge işaretleri metne
 *  uygulanmaz; sayfaya ait olmayan metin işareti sessizce taşınmaz. */
export function markRangesForPage(
  marks: { markKind: string; pageId: string | null; startOffset: number | null; endOffset: number | null }[],
  page: { id: string },
): OffsetRange[] {
  const out: OffsetRange[] = [];
  for (const mark of marks) {
    if (mark.markKind !== "text") continue;
    if (mark.pageId !== null && mark.pageId !== page.id) continue;
    if (mark.startOffset === null || mark.endOffset === null) continue;
    if (mark.endOffset <= mark.startOffset) continue;
    out.push({ start: mark.startOffset, end: mark.endOffset });
  }
  return out;
}

/** Panel boşken açılacak belge: diğer panelde açık sürüm **seçilmez**, böylece
 *  yan yana iki farklı belge açılır. Başka belge yoksa tek belgeye döner. */
export function pickInitialVersion(
  documents: DocumentRow[],
  excludeVersionId: string | null,
): string | null {
  const withVersion = documents.filter(
    (doc): doc is DocumentRow & { currentVersionId: string } => Boolean(doc.currentVersionId),
  );
  if (withVersion.length === 0) return null;
  const other = withVersion.find((doc) => doc.currentVersionId !== excludeVersionId);
  return (other ?? withVersion[0]!).currentVersionId;
}

/** Görünen (döndürülmüş) uzaydaki noktayı döndürme öncesi sayfa uzayına
 *  çevirir: `mapRectForDisplay`in tersi. */
export function unmapPointFromDisplay(
  x: number,
  y: number,
  pageWidth: number,
  pageHeight: number,
  rotation: number,
): { x: number; y: number } {
  switch (normalizeRotation(rotation)) {
    case 90:
      return { x: y, y: pageHeight - x };
    case 180:
      return { x: pageWidth - x, y: pageHeight - y };
    case 270:
      return { x: pageWidth - y, y: x };
    default:
      return { x, y };
  }
}

/** Görünen uzaydaki dikdörtgeni sayfa uzayına çevirir (bölge işareti için).
 *  Köşeler tek tek çevrilip min/max alınır; sıra döndürmede bozulmaz. */
export function rectFromDisplay(
  rect: [number, number, number, number],
  pageWidth: number,
  pageHeight: number,
  rotation: number,
): [number, number, number, number] {
  const [x0, y0, x1, y1] = rect;
  const a = unmapPointFromDisplay(x0, y0, pageWidth, pageHeight, rotation);
  const b = unmapPointFromDisplay(x1, y1, pageWidth, pageHeight, rotation);
  return [
    Math.min(a.x, b.x),
    Math.min(a.y, b.y),
    Math.max(a.x, b.x),
    Math.max(a.y, b.y),
  ];
}

/** Eski sürüme bağlı kaynağın rozet metni. Anchor güncel sürümdeyse `null`
 *  döner: güncel bağ için "eski sürüm" etiketi gösterilmez. */
export function versionBadge(location: {
  isCurrentVersion: boolean;
  versionNo: number | null;
  currentVersionNo: number | null;
}): string | null {
  if (location.isCurrentVersion) return null;
  const from =
    location.versionNo !== null ? `v${location.versionNo}` : t("readerLib.unknownVersion");
  const to =
    location.currentVersionNo !== null
      ? `v${location.currentVersionNo}`
      : t("readerLib.unknownVersion");
  return t("readerLib.versionBadge", { from, to });
}

/** Yeniden bağlama durum metni. Sayı uydurulmaz; eşleşme sayısı
 *  gerçek aday listesinden gelir. Aynı sürüm yeniden ayrıştırıldıysa
 *  (`isCurrentRun === false`) metin "sürüm" değil "ayrıştırma" der. */
export function rebindStatusText(suggestion: {
  status: string;
  candidates: unknown[];
  isCurrentRun: boolean;
}): string {
  const scope = suggestion.isCurrentRun
    ? t("readerLib.scopeVersion")
    : t("readerLib.scopeRun");
  switch (suggestion.status) {
    case "current":
      return t("readerLib.alreadyCurrent");
    case "exact":
      return t("readerLib.exact", { scope });
    case "ambiguous":
      return t("readerLib.ambiguous", { scope, count: suggestion.candidates.length });
    default:
      return t("readerLib.unresolved", { scope });
  }
}

/** Sürüm güncel ama ayrıştırma eskiyse rozet metni; sürüm eskiyse `null`
 *  döner (o durumu `versionBadge` anlatır). */
export function runBadge(location: {
  isCurrentVersion: boolean;
  isCurrentRun: boolean;
}): string | null {
  if (!location.isCurrentVersion || location.isCurrentRun) return null;
  return t("readerLib.oldRunBadge");
}
