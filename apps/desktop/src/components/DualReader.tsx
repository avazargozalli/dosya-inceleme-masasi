import { Fragment, useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import {
  api,
  toApiError,
  type AnchorRow,
  type ApiError,
  type DocumentPages,
  type DocumentRow,
  type MarkRow,
  type NoteRow,
  type PageRow,
} from "../lib/api";
import {
  EMPTY_VIEW,
  blocksForRange,
  clampZoom,
  displayPageSize,
  entrySlice,
  mapRectForDisplay,
  markRangesForPage,
  nextZoom,
  pageForKey,
  pageLabelInfo,
  parseWarnings,
  pickInitialVersion,
  previewCaption,
  previewDataUrl,
  rangeForPage,
  recallView,
  rectFromDisplay,
  rememberView,
  resolvePageIndex,
  rotateView,
  sliceForPage,
  structureEntries,
  textSegments,
  validSelection,
  type ReaderTarget,
  type ReaderView,
  type ViewMemory,
} from "../lib/reader";

/** Sayfa haritası: ayrıştırıcının **gerçekten ürettiği** blok kutuları.
 *  Sayfanın görüntüsü değildir; kutu yoksa hiç çizilmez (konum uydurulmaz).
 *  `rotation` belgenin kendi döndürmesi ile görünüm döndürmesinin toplamıdır. */
function PageMap({
  page,
  highlight,
  regions,
  rotation,
}: {
  page: PageRow;
  highlight: [number, number, number, number][];
  /** Kalıcı bölge işaretleri (sayfa koordinat uzayında). */
  regions: [number, number, number, number][];
  rotation: number;
}) {
  const width = page.width ?? 0;
  const height = page.height ?? 0;
  if (width <= 0 || height <= 0) return null;
  const display = displayPageSize(width, height, rotation);
  const unit = page.coordinateSpace === "image_pixels" ? "piksel" : "punto";
  const label = pageLabelInfo(page);
  return (
    <figure style={{ margin: 0 }}>
      <svg
        viewBox={`0 0 ${display.width} ${display.height}`}
        role="img"
        aria-label={`${label.physical} konum haritası: ${page.blocks.length} metin bloğu`}
        style={{
          display: "block",
          width: "100%",
          maxWidth: 300,
          maxHeight: 150,
          background: "var(--surface)",
          border: "1px solid var(--line)",
          borderRadius: "var(--radius)",
        }}
      >
        {page.blocks.map((block, index) => {
          if (!block.bbox) return null;
          const [x0, y0, x1, y1] = mapRectForDisplay(block.bbox, width, height, rotation);
          return (
            <rect
              key={`block-${index}`}
              x={x0}
              y={y0}
              width={Math.max(x1 - x0, 0.5)}
              height={Math.max(y1 - y0, 0.5)}
              fill="none"
              stroke="var(--line-strong)"
              strokeWidth={0.6}
              vectorEffect="non-scaling-stroke"
            />
          );
        })}
        {regions.map((rect, index) => {
          const [x0, y0, x1, y1] = mapRectForDisplay(rect, width, height, rotation);
          return (
            <rect
              key={`region-${index}`}
              x={x0}
              y={y0}
              width={Math.max(x1 - x0, 0.5)}
              height={Math.max(y1 - y0, 0.5)}
              fill="none"
              stroke="var(--warn)"
              strokeDasharray="3 2"
              strokeWidth={1}
              vectorEffect="non-scaling-stroke"
            />
          );
        })}
        {highlight.map((rect, index) => {
          const [x0, y0, x1, y1] = mapRectForDisplay(rect, width, height, rotation);
          return (
            <rect
              key={`hit-${index}`}
              x={x0}
              y={y0}
              width={Math.max(x1 - x0, 0.5)}
              height={Math.max(y1 - y0, 0.5)}
              fill="var(--accent-soft)"
              stroke="var(--accent)"
              strokeWidth={0.8}
              vectorEffect="non-scaling-stroke"
            />
          );
        })}
      </svg>
      <figcaption className="hint" style={{ marginTop: 4 }}>
        konum haritası (sayfa görüntüsü değil) · {Math.round(width)}×{Math.round(height)} {unit}
        {rotation ? ` · ${rotation}° döndürülmüş` : ""} · {page.blocks.length} blok
      </figcaption>
    </figure>
  );
}

/** Tek okuyucu paneli: kendi belge sürümünü, sayfasını ve seçimini tutar.
 *  Bir panelde belge değiştirmek diğerini etkilemez (F07-T08). */
function ReaderPane({
  caseId,
  label,
  documents,
  versionId,
  target,
  otherVersionId,
  onPickVersion,
  onAnchor,
  onNote,
}: {
  caseId: string;
  label: string;
  documents: DocumentRow[];
  versionId: string | null;
  target: ReaderTarget | null;
  /** Diğer panelde açık sürüm: boş panel aynı belgeyi ikinci kez açmaz. */
  otherVersionId: string | null;
  onPickVersion: (versionId: string) => void;
  onAnchor?: ((anchor: AnchorRow) => void) | undefined;
  onNote?: ((note: NoteRow) => void) | undefined;
}) {
  const [data, setData] = useState<DocumentPages | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [loading, setLoading] = useState(false);
  // F07-T08: sayfa, yakınlaştırma, döndürme ve metin seçimi tek bir panel
  // görünümüdür. Sürüm başına hatırlanır: belge değiştirip geri dönüldüğünde
  // eski konum ve seçim korunur. Bellek yalnız bu panelin oturumundadır;
  // diske/localStorage'a yazılmaz (seçim metni kasa dışına çıkmaz).
  const [view, setView] = useState<ReaderView>(EMPTY_VIEW);
  const [savedQuote, setSavedQuote] = useState<string | null>(null);
  const [focused, setFocused] = useState(false);
  // F07-T09: bu belgedeki kalıcı işaretler sunucudan okunur (panel kopyası
  // türetilmez); işaret/not oluşturma çekirdek komutlarıyla yapılır.
  const [marks, setMarks] = useState<MarkRow[]>([]);
  const [markBusy, setMarkBusy] = useState(false);
  const [markInfo, setMarkInfo] = useState<string | null>(null);
  const [noteDraft, setNoteDraft] = useState<{ title: string; body: string } | null>(null);
  const [noteBusy, setNoteBusy] = useState(false);
  const [noteInfo, setNoteInfo] = useState<string | null>(null);
  // F07-T09 görsel seçim: sayfa görüntüsü üzerinde sürüklenen bölge,
  // görünen uzayda 0–1 oranıyla tutulur; kayda geçerken sayfa uzayına çevrilir.
  const [dragRegion, setDragRegion] = useState<{ x0: number; y0: number; x1: number; y1: number } | null>(
    null,
  );
  const dragStart = useRef<{ x: number; y: number } | null>(null);
  const viewMemory = useRef<ViewMemory>([]);
  const previousVersion = useRef<string | null>(null);
  const restoredFromMemory = useRef(false);
  const viewRef = useRef(view);
  viewRef.current = view;
  const page = view.page;
  const zoom = view.zoom;
  const viewRotation = view.rotation;
  // F07-T11: sayfa görüntüsü kasadaki şifreli önbellekten gelir; kasa
  // kilitlenince çekirdek belleği temizler ve bu panel görüntüyü bırakır.
  const [previewOn, setPreviewOn] = useState(false);
  const [preview, setPreview] = useState<{
    pageIndex: number;
    dataUrl: string;
    meta: { cached: boolean; byteSize: number; dpi: number };
  } | null>(null);
  const [previewBusy, setPreviewBusy] = useState(false);
  const [previewError, setPreviewError] = useState<ApiError | null>(null);
  const textRef = useRef<HTMLDivElement>(null);
  const markRef = useRef<HTMLElement>(null);
  const entryRefs = useRef<Record<number, HTMLElement | null>>({});

  const paneTarget = target && target.versionId === versionId ? target : null;
  // İki panel aynı anda açıkken form alanı kimlikleri çakışmasın.
  const paneKey = label === "Sol belge" ? "sol" : "sag";

  // Boş panel kendiliğinden bir belge açar; diğer panelde açık sürümü seçmez,
  // böylece yan yana iki farklı belge gelir (tek belge varsa ona döner).
  useEffect(() => {
    if (versionId) return;
    const initial = pickInitialVersion(documents, otherVersionId);
    if (initial) onPickVersion(initial);
  }, [versionId, documents, otherVersionId, onPickVersion]);

  // Sürüm değişiminde bu panelin önceki görünümü hatırlanır, yeni sürümün
  // hatırlanan görünümü geri yüklenir. Hatıra yoksa temiz görünüm başlar;
  // başka panelin görünümü buraya taşınmaz.
  useEffect(() => {
    const previous = previousVersion.current;
    if (previous && previous !== versionId) {
      viewMemory.current = rememberView(viewMemory.current, previous, viewRef.current);
    }
    previousVersion.current = versionId ?? null;
    const remembered = recallView(viewMemory.current, versionId);
    // Hatırlanan konum varsa hedef (arama/bağ) sayfasına geri zıplanmaz:
    // kullanıcının bıraktığı yer korunur. Yeni bir hedef tıklaması yine açar.
    restoredFromMemory.current = remembered !== null;
    setView(remembered ?? EMPTY_VIEW);
    setSavedQuote(null);
  }, [versionId]);

  /** Sayfa değişimi seçimi bırakır: seçim offsetleri sayfaya göredir, başka
   *  sayfaya taşınmaz. Aynı sayfaya dönüşte seçim korunur. */
  function goToPage(index: number) {
    setSavedQuote(null);
    setView((state) =>
      state.page === index ? state : { ...state, page: index, selection: null },
    );
  }

  /** Panelin seçim kaydını bırakır (DOM seçimine dokunmaz). */
  function dropSelection() {
    setView((state) => (state.selection ? { ...state, selection: null } : state));
  }

  // İşaretler sürümden okunur; panelde türetilmiş kopya tutulmaz.
  useEffect(() => {
    setMarks([]);
    setMarkInfo(null);
    setNoteInfo(null);
    setNoteDraft(null);
    if (!versionId) return;
    let cancelled = false;
    void (async () => {
      try {
        const rows = await api.marksForVersion(versionId);
        if (!cancelled) setMarks(rows);
      } catch (e) {
        if (!cancelled) setError(toApiError(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [versionId]);

  useEffect(() => {
    if (!versionId) {
      setData(null);
      return;
    }
    let cancelled = false;
    // Sürüm değişince önceki belgenin sayfaları gösterilmez; hedef yeni
    // belgenin sayfaları yüklenince çözülür.
    setData(null);
    setLoading(true);
    setError(null);
    (async () => {
      try {
        const result = await api.documentPages(versionId);
        if (!cancelled) setData(result);
      } catch (e) {
        if (!cancelled) setError(toApiError(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [versionId]);

  // Hedef (arama sonucu veya kaynak bağ) geldiğinde doğru sayfayı aç. Sürüm
  // hatırlanan görünümle geri yüklendiyse hedef zıplaması yapılmaz; hedef yoksa
  // hatırlanan sayfa korunur, sıfıra zorlanmaz.
  useEffect(() => {
    if (!data) return;
    const skip = restoredFromMemory.current;
    restoredFromMemory.current = false;
    if (skip || !paneTarget) return;
    const index = resolvePageIndex(data.pages, paneTarget);
    if (index >= 0) goToPage(index);
  }, [data, paneTarget]);

  // Hatırlanan sayfa yeni sürümde yoksa (daha az sayfalı) son sayfaya çekilir.
  useEffect(() => {
    if (!data) return;
    if (data.pages.length > 0 && view.page >= data.pages.length) {
      goToPage(data.pages.length - 1);
    }
  }, [data, view.page]);

  // Görüntü isteği yalnız kullanıcı açtığında ve gerçek sayfa için yapılır.
  useEffect(() => {
    setPreview(null);
    setPreviewError(null);
    if (!previewOn || !versionId) {
      setPreviewBusy(false);
      return;
    }
    let cancelled = false;
    setPreviewBusy(true);
    void (async () => {
      try {
        const result = await api.documentPagePreview(versionId, page, 96);
        if (cancelled) return;
        setPreview({
          pageIndex: page,
          dataUrl: previewDataUrl(result.pngBase64),
          meta: { cached: result.cached, byteSize: result.byteSize, dpi: result.dpi },
        });
      } catch (e) {
        if (!cancelled) setPreviewError(toApiError(e));
      } finally {
        if (!cancelled) setPreviewBusy(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [previewOn, versionId, page]);

  // Seçim sayfa metnine karşı doğrulanır: yeniden ayrıştırma metni kaydırdıysa
  // eski offset'teki alıntı işaretlenmez ve kullanıcıya sebep yazılır.
  const storedSelection = view.selection;
  const current: PageRow | undefined = data?.pages[page];
  const selection = current ? validSelection(current.text, storedSelection) : null;
  const selectionDropped = Boolean(storedSelection) && selection === null;
  // Toplam döndürme: belgenin kendi değeri + kullanıcının görünüm ayarı.
  const rotation = rotateView(current?.rotation ?? 0, viewRotation);
  const zoomPercent = Math.round(clampZoom(zoom) * 100);
  const currentInfo = current ? pageLabelInfo(current) : null;
  // Yapısal belge (DOCX/UDF): paragraf/bölüm yolları türev sayfadan ayrı
  // gösterilir. Paragraflar metnin tamamını kaplamıyorsa tek parça gösterime
  // dönülür; yoksa seçim offsetleri metinle uyuşmaz.
  const entries = current ? structureEntries(current) : [];
  const structureMode = current ? current.coordinateSpace === "structure" : false;
  const entriesCoverText =
    current !== undefined &&
    entries.length > 0 &&
    entries[entries.length - 1]!.end >= Array.from(current.text).length;
  const structureText = structureMode && entriesCoverText;
  const labeledEntries = entries.filter((entry) => entry.path);
  const pageCaption = currentInfo
    ? structureMode
      ? `Türev sayfa ${current?.pageIndex !== undefined ? current.pageIndex + 1 : "?"}`
      : currentInfo.physical
    : "Fiziksel sayfa ?";
  const slice = current ? sliceForPage(current.text, paneTarget, current) : null;
  const highlightRange = current ? rangeForPage(current.text, paneTarget, current) : null;
  // F07-T09: bu sayfadaki metin işaretleri ve hedef aralık tek geçişte
  // parçalanır; parçalar birleşince sayfa metni birebir elde edilir.
  const pageMarkRanges = current ? markRangesForPage(marks, current) : [];
  const segments =
    current && (pageMarkRanges.length > 0 || highlightRange)
      ? textSegments(current.text, pageMarkRanges, highlightRange)
      : [];
  const pageRegionMarks = current
    ? marks.filter((mark) => mark.markKind === "region" && mark.pageId === current.id)
    : [];
  // Görüntü üzerindeki bölge işaretleri: sayfa uzayından görünen uzaya çevrilip
  // yüzdeye dönüştürülür (görüntü ölçeklense de konum kaymaz).
  const previewBox = current
    ? displayPageSize(current.width ?? 0, current.height ?? 0, rotation)
    : null;
  const previewRegions =
    current && previewBox && previewBox.width > 0 && previewBox.height > 0
      ? pageRegionMarks
          .flatMap((mark) => mark.rects)
          .map((rect) => {
            const box = mapRectForDisplay(
              rect,
              current.width ?? 0,
              current.height ?? 0,
              rotation,
            );
            return {
              left: (box[0] / previewBox.width) * 100,
              top: (box[1] / previewBox.height) * 100,
              width: ((box[2] - box[0]) / previewBox.width) * 100,
              height: ((box[3] - box[1]) / previewBox.height) * 100,
            };
          })
      : [];
  // Anchor kaydı kendi kutularını taşıyorsa onlar kullanılır; arama sonucunda
  // bloklardan hesaplanır. Kutu yoksa harita vurgusuz kalır.
  const highlightQuads =
    paneTarget?.quads.length ? paneTarget.quads : current ? blocksForRange(current.blocks, highlightRange) : [];
  // Kullanıcı hedef sayfadan başka sayfaya geçtiyse "bulunamadı" uyarısı
  // yanıltıcı olur; yalnız hedefin sayfasında gösterilir.
  const targetPage = data ? resolvePageIndex(data.pages, paneTarget) : -1;
  const parseWarningsList = data ? parseWarnings(data.parseRun.warnings) : [];

  useEffect(() => {
    if (slice) markRef.current?.scrollIntoView({ block: "center" });
  }, [slice?.hit, page, versionId]);

  /** DOM seçimini Unicode scalar aralığına çevirir; kaynak doğrulaması
   *  çekirdekte yapılır, burada yalnız konum hesaplanır. */
  function readSelection() {
    const node = textRef.current;
    const sel = window.getSelection();
    if (!node || !sel || sel.isCollapsed || sel.rangeCount === 0) {
      dropSelection();
      return;
    }
    const range = sel.getRangeAt(0);
    const quote = sel.toString();
    const full = node.textContent ?? "";
    if (!quote.trim() || !node.contains(range.startContainer)) {
      dropSelection();
      return;
    }
    let utf16Start: number;
    try {
      // Seçim, vurgulanmış <mark> içinde başlayabilir; kök metnin başından
      // seçim başlangıcına kadar olan aralık ölçülür (node offset'i değil).
      const pre = document.createRange();
      pre.selectNodeContents(node);
      pre.setEnd(range.startContainer, range.startOffset);
      utf16Start = pre.toString().length;
    } catch {
      dropSelection();
      return;
    }
    if (full.slice(utf16Start, utf16Start + quote.length) !== quote) {
      dropSelection();
      return;
    }
    const start = Array.from(full.slice(0, utf16Start)).length;
    const end = start + Array.from(quote).length;
    setView((state) => ({ ...state, selection: { quote, start, end } }));
  }

  /** Klavye gezinmesi yalnız bu panelde çalışır (odak buradaysa). Yalnız
   *  Alt ile: düz PageUp/PageDown metin alanının kaydırmasını bozmaz. */
  function onPaneKeyDown(event: ReactKeyboardEvent<HTMLElement>) {
    if (!data) return;
    const next = pageForKey(
      event.key,
      { alt: event.altKey, ctrl: event.ctrlKey, meta: event.metaKey, shift: event.shiftKey },
      view.page,
      data.pages.length,
    );
    if (next === null) return;
    event.preventDefault();
    goToPage(next);
  }

  async function saveAnchor() {
    if (!selection || !current?.segmentId || !data) return;
    try {
      const anchor = await api.anchorCreate({
        caseId,
        documentVersionId: data.parseRun.documentVersionId,
        parseRunId: data.parseRun.id,
        segmentId: current.segmentId,
        startOffset: selection.start,
        endOffset: selection.end,
        exactQuote: selection.quote,
      });
      setSavedQuote(anchor.exactQuote);
      onAnchor?.(anchor);
      window.getSelection()?.removeAllRanges();
      dropSelection();
    } catch (e) {
      setError(toApiError(e));
    }
  }

  /** Seçimden çekirdeğe gidecek anchor girdisi; kaynak doğrulaması çekirdekte. */
  function selectionAnchor() {
    if (!selection || !current?.segmentId || !data) return null;
    return {
      caseId,
      documentVersionId: data.parseRun.documentVersionId,
      parseRunId: data.parseRun.id,
      segmentId: current.segmentId,
      startOffset: selection.start,
      endOffset: selection.end,
      exactQuote: selection.quote,
    };
  }

  /** Seçimi kalıcı işarete çevirir: çekirdek anchor'ı doğrular, işaret aynı
   *  işlemde yazılır. Doğrulanamayan alıntı için işaret oluşmaz. */
  async function createMark() {
    const input = selectionAnchor();
    if (!input) return;
    setMarkBusy(true);
    setMarkInfo(null);
    try {
      const created = await api.markCreateFromSelection(input, "");
      setMarks((prev) => (prev.some((mark) => mark.id === created.mark.id) ? prev : [...prev, created.mark]));
      setMarkInfo("İşaret kaydedildi.");
      window.getSelection()?.removeAllRanges();
      dropSelection();
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setMarkBusy(false);
    }
  }

  /** Seçimden not taslağı açar; not yalnız “Notu kaydet” ile yazılır. */
  function startNoteFromSelection() {
    if (!selection) return;
    setNoteDraft({
      title: `${currentInfo?.physical ?? "Sayfa"} notu`,
      body: selection.quote,
    });
    setNoteInfo(null);
  }

  async function saveNoteFromSelection() {
    const input = selectionAnchor();
    if (!input || !noteDraft) return;
    setNoteBusy(true);
    try {
      const saved = await api.noteCreateFromSelection(input, {
        title: noteDraft.title,
        body: noteDraft.body,
      });
      setNoteDraft(null);
      setNoteInfo(`Not oluşturuldu: “${saved.note.title || saved.note.body.slice(0, 40)}”`);
      window.getSelection()?.removeAllRanges();
      dropSelection();
      onNote?.(saved.note);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setNoteBusy(false);
    }
  }

  async function removeMark(markId: string) {
    try {
      await api.markDelete(markId);
      setMarks((prev) => prev.filter((mark) => mark.id !== markId));
      setMarkInfo("İşaret silindi.");
    } catch (e) {
      setError(toApiError(e));
    }
  }

  /** İşaretin sayfasını açar; sayfa yoksa hiçbir şey yapmaz (uydurma yok). */
  function jumpToMark(mark: MarkRow) {
    if (!data || !mark.pageId) return;
    const index = data.pages.findIndex((p) => p.id === mark.pageId);
    if (index >= 0) goToPage(index);
  }

  /** Görüntü üzerindeki imleç konumunu 0–1 oranına çevirir. */
  function regionPoint(event: React.PointerEvent<HTMLDivElement>) {
    const box = event.currentTarget.getBoundingClientRect();
    if (box.width <= 0 || box.height <= 0) return null;
    return {
      x: Math.min(Math.max((event.clientX - box.left) / box.width, 0), 1),
      y: Math.min(Math.max((event.clientY - box.top) / box.height, 0), 1),
    };
  }

  function startRegion(event: React.PointerEvent<HTMLDivElement>) {
    const point = regionPoint(event);
    if (!point) return;
    dragStart.current = point;
    setDragRegion({ x0: point.x, y0: point.y, x1: point.x, y1: point.y });
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function moveRegion(event: React.PointerEvent<HTMLDivElement>) {
    const start = dragStart.current;
    if (!start) return;
    const point = regionPoint(event);
    if (!point) return;
    setDragRegion({
      x0: Math.min(start.x, point.x),
      y0: Math.min(start.y, point.y),
      x1: Math.max(start.x, point.x),
      y1: Math.max(start.y, point.y),
    });
  }

  function endRegion(event: React.PointerEvent<HTMLDivElement>) {
    if (!dragStart.current) return;
    dragStart.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  /** Sürüklenen bölgeyi sayfa uzayına çevirip kalıcı işaret yazar. */
  async function createRegionMark() {
    if (!dragRegion || !current || !versionId) return;
    const width = current.width ?? 0;
    const height = current.height ?? 0;
    if (width <= 0 || height <= 0) return;
    if (dragRegion.x1 - dragRegion.x0 < 0.01 || dragRegion.y1 - dragRegion.y0 < 0.01) return;
    const display = displayPageSize(width, height, rotation);
    const displayRect: [number, number, number, number] = [
      dragRegion.x0 * display.width,
      dragRegion.y0 * display.height,
      dragRegion.x1 * display.width,
      dragRegion.y1 * display.height,
    ];
    const pageRect = rectFromDisplay(displayRect, width, height, rotation);
    setMarkBusy(true);
    setMarkInfo(null);
    try {
      const mark = await api.markCreateRegion({
        caseId,
        documentVersionId: versionId,
        pageId: current.id,
        rects: [pageRect],
        label: "",
      });
      setMarks((prev) => [...prev, mark]);
      setDragRegion(null);
      setMarkInfo("Bölge işareti kaydedildi.");
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setMarkBusy(false);
    }
  }

  const parsed = documents
    .map((doc) => ({ doc, versionId: doc.currentVersionId }))
    .filter((entry): entry is { doc: DocumentRow; versionId: string } => Boolean(entry.versionId));
  // Arama sonucu belgenin güncel olmayan bir sürümünü açabilir; seçicide
  // o sürüm yoksa "kayıt sürümü" olarak eklenir, böylece kaynak görünür kalır.
  const options = parsed.map(({ doc, versionId: vid }) => ({ versionId: vid, label: doc.displayName }));
  if (versionId && !options.some((option) => option.versionId === versionId)) {
    options.unshift({
      versionId,
      label: `${target?.documentName ?? "Seçili sürüm"} (kayıt sürümü)`,
    });
  }

  return (
    <section
      aria-label={label}
      onKeyDown={onPaneKeyDown}
      onFocus={() => setFocused(true)}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setFocused(false);
      }}
      style={{
        minWidth: 0,
        display: "flex",
        flexDirection: "column",
        gap: "var(--space-2)",
        border: `1px solid ${focused ? "var(--accent)" : "var(--line)"}`,
        borderRadius: "var(--radius)",
        padding: "var(--space-3)",
        background: "var(--bg)",
      }}
    >
      <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
        <span className="badge plain">{label}</span>
        {focused ? (
          <span
            className="badge ok"
            title="Klavye gezinmesi bu panelde; diğer panelin sayfası ve seçimi değişmez"
          >
            etkin panel
          </span>
        ) : null}
        <select
          className="input"
          aria-label={`${label} belgesi`}
          style={{ flex: "1 1 160px", height: 30, maxWidth: 320 }}
          value={versionId ?? ""}
          onChange={(e) => onPickVersion(e.target.value)}
        >
          {options.length === 0 ? <option value="">Belge yok</option> : null}
          {options.map((option) => (
            <option key={option.versionId} value={option.versionId}>
              {option.label}
            </option>
          ))}
        </select>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      {paneTarget?.anchorId && paneTarget.quoteIntact === false ? (
        <div className="notice warn" role="status">
          Kaynak kopmuş: alıntı segmentte birebir bulunamadı. Anchor silinmedi; sayfa yine açıldı.
        </div>
      ) : null}

      {!versionId ? (
        <div className="empty" style={{ padding: "var(--space-4)" }}>
          <p style={{ margin: 0 }}>Bu panelde açılacak belge seçin.</p>
        </div>
      ) : loading && !data ? (
        <p className="hint" style={{ margin: 0 }}>Yükleniyor…</p>
      ) : !data ? (
        <div className="empty" style={{ padding: "var(--space-4)" }}>
          <p style={{ margin: 0 }}>Ayrıştırılmış metin yok. Önce “Ayrıştır” çalıştırın.</p>
        </div>
      ) : (
        <>
          <div className="row" style={{ gap: 6, flexWrap: "wrap" }}>
            {data.pages.map((p, index) => {
              const info = pageLabelInfo(p);
              return (
                <button
                  key={p.id}
                  className={index === page ? "btn primary" : "btn"}
                  style={{ height: 26, minWidth: 34, padding: "0 7px" }}
                  onClick={() => goToPage(index)}
                  aria-label={info.showsLabel ? `${info.physical} (${info.label})` : info.physical}
                  title={
                    info.showsLabel
                      ? `${info.label} · ${p.textOrigin === "none" ? "metin katmanı yok" : "metin var"}`
                      : p.textOrigin === "none"
                        ? "metin katmanı yok"
                        : "metin var"
                  }
                >
                  {p.pageIndex + 1}
                </button>
              );
            })}
          </div>

          {previewOn ? (
            <figure style={{ margin: 0 }}>
              {previewError ? (
                <div className="notice danger" role="alert">
                  <strong>{previewError.code}</strong> — {previewError.message}
                </div>
              ) : null}
              {previewBusy && !preview ? (
                <p className="hint" style={{ margin: 0 }}>
                  Sayfa görüntüsü hazırlanıyor…
                </p>
              ) : null}
              {preview && preview.pageIndex === page ? (
                <>
                  <div
                    role="group"
                    aria-label={`${label}: sayfa görüntüsü üzerinde bölge seçimi`}
                    style={{
                      position: "relative",
                      display: "inline-block",
                      maxWidth: "100%",
                      touchAction: "none",
                      cursor: "crosshair",
                      userSelect: "none",
                    }}
                    onPointerDown={startRegion}
                    onPointerMove={moveRegion}
                    onPointerUp={endRegion}
                    onPointerCancel={endRegion}
                  >
                    <img
                      src={preview.dataUrl}
                      alt={`${label}: fiziksel sayfa ${page + 1} görüntüsü`}
                      draggable={false}
                      style={{
                        display: "block",
                        width: "100%",
                        maxWidth: 640,
                        border: "1px solid var(--line)",
                        borderRadius: "var(--radius)",
                        background: "var(--surface)",
                      }}
                    />
                    {previewRegions.map((box, index) => (
                      <div
                        key={`region-${index}`}
                        aria-hidden
                        style={{
                          position: "absolute",
                          left: `${box.left}%`,
                          top: `${box.top}%`,
                          width: `${box.width}%`,
                          height: `${box.height}%`,
                          border: "1.5px dashed var(--warn)",
                          background: "rgba(180, 130, 40, 0.12)",
                          pointerEvents: "none",
                        }}
                      />
                    ))}
                    {dragRegion ? (
                      <div
                        aria-hidden
                        style={{
                          position: "absolute",
                          left: `${dragRegion.x0 * 100}%`,
                          top: `${dragRegion.y0 * 100}%`,
                          width: `${(dragRegion.x1 - dragRegion.x0) * 100}%`,
                          height: `${(dragRegion.y1 - dragRegion.y0) * 100}%`,
                          border: "1.5px solid var(--accent)",
                          background: "var(--accent-soft)",
                          pointerEvents: "none",
                        }}
                      />
                    ) : null}
                  </div>
                  <div className="row" style={{ gap: 6, alignItems: "center", marginTop: 4 }}>
                    <button
                      className="btn"
                      style={{ height: 26 }}
                      disabled={
                        markBusy ||
                        !dragRegion ||
                        dragRegion.x1 - dragRegion.x0 < 0.01 ||
                        dragRegion.y1 - dragRegion.y0 < 0.01
                      }
                      onClick={() => void createRegionMark()}
                      title="Görüntü üzerinde sürüklediğiniz bölgeyi sayfa koordinatında işaret olarak sakla"
                    >
                      {markBusy ? "İşaretleniyor…" : "Seçili bölgeyi işaretle"}
                    </button>
                    <button
                      className="btn ghost"
                      style={{ height: 26 }}
                      disabled={!dragRegion}
                      onClick={() => setDragRegion(null)}
                    >
                      Bölge seçimini temizle
                    </button>
                    <span className="hint" style={{ margin: 0 }}>
                      İşaretler sayfa koordinatında saklanır; yakınlaştırma/döndürme konumu kaydırmaz.
                    </span>
                  </div>
                  <figcaption className="hint" style={{ marginTop: 4 }}>
                    {previewCaption(preview.meta)}
                  </figcaption>
                </>
              ) : null}
            </figure>
          ) : null}

          <div className="row" style={{ gap: 6, alignItems: "center", flexWrap: "wrap" }}>
            <span className="hint" style={{ margin: 0 }}>
              Görünüm:
            </span>
            <button
              className="btn"
              style={{ height: 26, minWidth: 26, padding: "0 6px" }}
              aria-label={`${label}: uzaklaştır`}
              title="Uzaklaştır"
              disabled={nextZoom(zoom, -1) === clampZoom(zoom)}
              onClick={() => setView((state) => ({ ...state, zoom: nextZoom(state.zoom, -1) }))}
            >
              −
            </button>
            <span
              className="badge plain"
              aria-live="polite"
              style={{ minWidth: 48, justifyContent: "center" }}
            >
              {zoomPercent}%
            </span>
            <button
              className="btn"
              style={{ height: 26, minWidth: 26, padding: "0 6px" }}
              aria-label={`${label}: yakınlaştır`}
              title="Yakınlaştır"
              disabled={nextZoom(zoom, 1) === clampZoom(zoom)}
              onClick={() => setView((state) => ({ ...state, zoom: nextZoom(state.zoom, 1) }))}
            >
              +
            </button>
            <button
              className="btn ghost"
              style={{ height: 26 }}
              disabled={zoom === 1}
              onClick={() => setView((state) => ({ ...state, zoom: 1 }))}
              title="Yakınlaştırmayı %100'e döndür"
            >
              %100
            </button>
            <span
              aria-hidden
              style={{ width: 1, height: 18, background: "var(--line)", margin: "0 2px" }}
            />
            <button
              className="btn"
              style={{ height: 26, minWidth: 26, padding: "0 6px" }}
              aria-label={`${label}: sola döndür`}
              title="Sola döndür (90°)"
              onClick={() => setView((state) => ({ ...state, rotation: rotateView(state.rotation, -90) }))}
            >
              ⟲
            </button>
            <button
              className="btn"
              style={{ height: 26, minWidth: 26, padding: "0 6px" }}
              aria-label={`${label}: sağa döndür`}
              title="Sağa döndür (90°)"
              onClick={() => setView((state) => ({ ...state, rotation: rotateView(state.rotation, 90) }))}
            >
              ⟳
            </button>
            <button
              className="btn ghost"
              style={{ height: 26 }}
              disabled={viewRotation === 0}
              onClick={() => setView((state) => ({ ...state, rotation: 0 }))}
              title="Görünüm döndürmesini sıfırla (belgenin kendi döndürmesi kalır)"
            >
              Döndürmeyi sıfırla
            </button>
            <span
              aria-hidden
              style={{ width: 1, height: 18, background: "var(--line)", margin: "0 2px" }}
            />
            <button
              className={previewOn ? "btn primary" : "btn"}
              style={{ height: 26 }}
              aria-pressed={previewOn}
              title="Sayfanın gerçek görüntüsünü göster (yalnız PDF; önbellek şifreli tutulur)"
              onClick={() => setPreviewOn((value) => !value)}
            >
              {previewOn ? "Görüntü açık" : "Sayfa görüntüsü"}
            </button>
          </div>

          <div className="row" style={{ justifyContent: "space-between", alignItems: "center", gap: 6 }}>
            <span style={{ display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }}>
              <span className="badge plain">
                {pageCaption}/{data.pages.length}
              </span>
              {structureMode ? (
                <span
                  className="badge plain"
                  title="DOCX/UDF: sayfa ayrımı doğrulanmadı; kaynak paragraf bazlıdır"
                >
                  yapısal belge · paragraf bazlı
                </span>
              ) : null}
              {currentInfo?.showsLabel ? (
                <span
                  className="badge"
                  title="PDF /PageLabels ağacından gelen basılı etiket; fiziksel sıradan ayrıdır"
                >
                  basılı etiket “{currentInfo.label}”
                </span>
              ) : null}
              {data.parseRun.status === "succeeded" ? null : (
                <span className="badge warn">kısmi ayrıştırma · {data.parseRun.status}</span>
              )}
              {current && current.textOrigin !== "native" ? (
                <span className="badge warn">metin yok / OCR</span>
              ) : null}
              {current && rotation ? (
                <span
                  className="badge plain"
                  title={`belge ${current.rotation}° + görünüm ${viewRotation}°; koordinatlar döndürme öncesi uzaydadır`}
                >
                  görünüm {rotation}° döndürülmüş
                </span>
              ) : null}
            </span>
            <span style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
              <button
                className="btn ghost"
                style={{ height: 26 }}
                disabled={!selection || !current?.segmentId}
                onClick={() => void saveAnchor()}
                title={current?.segmentId ? "Seçili metni kaynak olarak kaydet" : "bu sayfada metin segmenti yok"}
              >
                Seçimi kaynağa bağla
              </button>
              <button
                className="btn"
                style={{ height: 26 }}
                disabled={!selection || !current?.segmentId || markBusy}
                onClick={() => void createMark()}
                title="Seçili metni bu belgede kalıcı işaret olarak sakla (alıntı çekirdekte doğrulanır)"
              >
                {markBusy ? "İşaretleniyor…" : "İşaretle"}
              </button>
              <button
                className="btn"
                style={{ height: 26 }}
                disabled={!selection || !current?.segmentId || noteBusy}
                onClick={startNoteFromSelection}
                title="Seçili metinden kaynaklı not oluştur"
              >
                Seçimden not
              </button>
            </span>
          </div>

          {noteDraft ? (
            <div className="card tight" style={{ background: "var(--surface)" }}>
              <div className="card-head">
                <div>
                  <h2 style={{ fontSize: 14 }}>Seçimden not</h2>
                  <p className="card-sub">
                    Kaynak: {currentInfo?.physical ?? "sayfa"} · alıntı {selection?.start}–{selection?.end}
                  </p>
                </div>
              </div>
              <div className="field">
                <label htmlFor={`note-title-${paneKey}`}>Başlık</label>
                <input
                  id={`note-title-${paneKey}`}
                  className="input"
                  value={noteDraft.title}
                  maxLength={200}
                  onChange={(e) => setNoteDraft((draft) => (draft ? { ...draft, title: e.target.value } : draft))}
                />
              </div>
              <div className="field">
                <label htmlFor={`note-body-${paneKey}`}>Not</label>
                <textarea
                  id={`note-body-${paneKey}`}
                  className="input"
                  rows={4}
                  value={noteDraft.body}
                  onChange={(e) => setNoteDraft((draft) => (draft ? { ...draft, body: e.target.value } : draft))}
                />
              </div>
              <div className="row" style={{ justifyContent: "flex-end" }}>
                <button className="btn ghost" onClick={() => setNoteDraft(null)}>
                  Vazgeç
                </button>
                <button
                  className="btn primary"
                  disabled={noteBusy || !noteDraft.body.trim()}
                  onClick={() => void saveNoteFromSelection()}
                >
                  {noteBusy ? "Kaydediliyor…" : "Notu kaydet"}
                </button>
              </div>
            </div>
          ) : null}

          {markInfo ? <div className="notice ok">{markInfo}</div> : null}
          {noteInfo ? <div className="notice ok">{noteInfo}</div> : null}

          <p className="hint" style={{ margin: 0 }}>
            Bu panelin görünümü: sayfa {view.page + 1}/{data.pages.length} · %{zoomPercent}
            {view.rotation ? ` · ${view.rotation}° döndürülmüş` : ""}. Belge değiştirip geri
            dönerseniz bu konum ve seçim geri yüklenir; diğer panelin görünümü değişmez. Klavye:
            <code>Alt</code>+<code>←</code>/<code>→</code> veya <code>Alt</code>+<code>PageUp</code>/
            <code>PageDown</code> sayfa değiştirir, <code>Alt</code>+<code>Home</code>/<code>End</code>{" "}
            ilk/son sayfaya gider.
          </p>

          {current && current.coordinateSpace && current.coordinateSpace !== "structure" && current.blocks.length > 0 ? (
            <PageMap
            page={current}
            highlight={highlightQuads}
            regions={pageRegionMarks.flatMap((mark) => mark.rects)}
            rotation={rotation}
          />
          ) : null}

          {structureMode && labeledEntries.length > 0 ? (
            <details style={{ fontSize: 12 }}>
              <summary style={{ cursor: "pointer", color: "var(--ink-3)" }}>
                Yapı — {labeledEntries.length} paragraf/bölüm (türev sayfadan ayrı)
              </summary>
              <ol style={{ margin: "6px 0 0", paddingLeft: 18, maxHeight: 160, overflow: "auto" }}>
                {labeledEntries.slice(0, 200).map((entry) => (
                  <li key={`structure-${entry.index}`}>
                    <button
                      className="btn ghost"
                      style={{
                        height: "auto",
                        padding: "1px 4px",
                        textAlign: "left",
                        whiteSpace: "normal",
                      }}
                      onClick={() => entryRefs.current[entry.index]?.scrollIntoView({ block: "center" })}
                      title={entry.path ?? undefined}
                    >
                      {entry.heading ? <strong>{entry.path}</strong> : entry.path}
                      {entry.text.trim() ? ` — ${entry.text.trim().slice(0, 60)}` : ""}
                    </button>
                  </li>
                ))}
              </ol>
              {labeledEntries.length > 200 ? (
                <p className="hint" style={{ margin: 0 }}>
                  +{labeledEntries.length - 200} kayıt daha var; metin içinde gezinin.
                </p>
              ) : null}
            </details>
          ) : null}

          <div
            ref={textRef}
            onMouseUp={readSelection}
            onKeyUp={readSelection}
            tabIndex={0}
            aria-label={currentInfo ? `${currentInfo.physical} metni` : "sayfa metni"}
            style={{
              background: "var(--surface)",
              border: "1px solid var(--line)",
              borderRadius: "var(--radius)",
              padding: "var(--space-3)",
              whiteSpace: "pre-wrap",
              fontFamily: "var(--mono)",
              fontSize: `${(12.5 * clampZoom(zoom)).toFixed(2)}px`,
              lineHeight: 1.7,
              height: 340,
              overflow: "auto",
            }}
          >
            {current ? (
              structureText ? (
                entries.map((entry, index) => {
                  const piece = entrySlice(entry, highlightRange);
                  return (
                    <Fragment key={`entry-${entry.index}`}>
                      {index > 0 ? "\n\n" : null}
                      <span
                        ref={(node) => {
                          entryRefs.current[entry.index] = node;
                        }}
                        title={entry.path ?? undefined}
                        style={entry.heading ? { fontWeight: 600 } : undefined}
                      >
                        {piece.hit ? (
                          <>
                            {piece.before}
                            <mark
                              ref={markRef}
                              style={{
                                background: "var(--accent-soft)",
                                color: "inherit",
                                padding: "0 2px",
                                borderRadius: 2,
                              }}
                            >
                              {piece.hit}
                            </mark>
                            {piece.after}
                          </>
                        ) : (
                          entry.text
                        )}
                      </span>
                    </Fragment>
                  );
                })
              ) : segments.length > 0 ? (
                segments.map((segment, index) =>
                  segment.targeted ? (
                    <mark
                      key={`seg-${index}`}
                      ref={markRef}
                      style={{ background: "var(--accent-soft)", color: "inherit", padding: "0 2px", borderRadius: 2 }}
                    >
                      {segment.text}
                    </mark>
                  ) : segment.marked ? (
                    <span
                      key={`seg-${index}`}
                      style={{ background: "var(--warn-soft)", borderRadius: 2 }}
                      title="kalıcı işaret"
                    >
                      {segment.text}
                    </span>
                  ) : (
                    <Fragment key={`seg-${index}`}>{segment.text}</Fragment>
                  ),
                )
              ) : (
                current.text.trim() || "(bu sayfada metin katmanı yok)"
              )
            ) : null}
          </div>

          {paneTarget && page === targetPage && !slice && (paneTarget.startOffset !== null || Boolean(paneTarget.query)) ? (
            <p className="hint" style={{ margin: 0 }}>
              İşaretlenecek alıntı bu sayfada bulunamadı; sayfa numarası yine de kaynaktan geldi.
            </p>
          ) : null}

          {parseWarningsList.length > 0 ? (
            <details style={{ fontSize: 12 }}>
              <summary style={{ cursor: "pointer", color: "var(--ink-3)" }}>
                {parseWarningsList.length} ayrıştırma uyarısı (imza, bilinmeyen öğe, sayfa…)
              </summary>
              <ul style={{ margin: "6px 0 0", paddingLeft: 18 }}>
                {parseWarningsList.slice(0, 20).map((warning, index) => (
                  <li key={`${index}-${warning.slice(0, 24)}`}>{warning}</li>
                ))}
              </ul>
              {parseWarningsList.length > 20 ? (
                <p className="hint" style={{ margin: 0 }}>
                  +{parseWarningsList.length - 20} uyarı daha var.
                </p>
              ) : null}
            </details>
          ) : null}

          {marks.length > 0 ? (
            <details style={{ fontSize: 12 }}>
              <summary style={{ cursor: "pointer", color: "var(--ink-3)" }}>
                {marks.length} kalıcı işaret (bu belgede)
              </summary>
              <ul style={{ margin: "6px 0 0", paddingLeft: 18, maxHeight: 160, overflow: "auto" }}>
                {marks.map((mark) => {
                  const markPage = data.pages.find((p) => p.id === mark.pageId);
                  const caption = markPage ? pageLabelInfo(markPage).physical : "sayfa bilinmiyor";
                  return (
                    <li key={mark.id} style={{ marginBottom: 4 }}>
                      <span className="muted small">
                        {caption} · {mark.markKind === "text" ? "metin" : "bölge"}
                      </span>{" "}
                      <button
                        className="btn ghost"
                        style={{ height: "auto", padding: "0 6px" }}
                        disabled={!markPage}
                        onClick={() => jumpToMark(mark)}
                        title={mark.markKind === "text" ? mark.quotedText : "bölge işaretini göster"}
                      >
                        {mark.markKind === "text"
                          ? `“${mark.quotedText.slice(0, 50)}${mark.quotedText.length > 50 ? "…" : ""}”`
                          : `bölge · ${mark.rects.length} kutu`}
                      </button>
                      <button
                        className="btn ghost"
                        style={{ height: 22, padding: "0 6px" }}
                        aria-label="işareti sil"
                        onClick={() => void removeMark(mark.id)}
                      >
                        Sil
                      </button>
                    </li>
                  );
                })}
              </ul>
            </details>
          ) : null}

          {selectionDropped ? (
            <p className="hint" style={{ margin: 0 }}>
              Önceki seçim bu sayfada doğrulanamadı (metin değişmiş olabilir); seçim geri
              yüklenmedi, yanlış alıntı işaretlenmez.
            </p>
          ) : null}
          {selection ? (
            <div className="notice">
              Seçim: “{selection.quote.length > 90 ? `${selection.quote.slice(0, 90)}…` : selection.quote}”
              {" · "}karakter {selection.start}–{selection.end}
            </div>
          ) : null}
          {savedQuote ? <div className="notice ok">Kaynak kaydedildi: “{savedQuote.slice(0, 80)}”</div> : null}
        </>
      )}
    </section>
  );
}

/** İki belgeyi yan yana açan okuyucu (REQ-009 / F07-T08). Her panelin
 *  kaynağı ve seçimi ayrı tutulur; arama sonucu veya anchor hedefi ilgili
 *  panelde doğru sürümü ve sayfayı açar. */
export function DualReader({
  caseId,
  documents,
  left,
  right,
  onClose,
  onAnchor,
  onNote,
}: {
  caseId: string;
  documents: DocumentRow[];
  left: ReaderTarget | null;
  right: ReaderTarget | null;
  onClose: () => void;
  onAnchor?: (anchor: AnchorRow) => void;
  onNote?: (note: NoteRow) => void;
}) {
  const [leftVersion, setLeftVersion] = useState<string | null>(left?.versionId ?? null);
  const [rightVersion, setRightVersion] = useState<string | null>(right?.versionId ?? null);

  useEffect(() => {
    if (left?.versionId) setLeftVersion(left.versionId);
  }, [left]);

  useEffect(() => {
    if (right?.versionId) setRightVersion(right.versionId);
  }, [right]);

  return (
    <div className="card" style={{ borderColor: "var(--line-strong)" }}>
      <div className="card-head">
        <div>
          <h2>Okuyucu — yan yana</h2>
          <p className="card-sub">
            Her panel kendi belgesini, sayfasını, yakınlaştırmasını ve metin seçimini korur.
            Bir panelde belge değiştirmek diğerini etkilemez; alıntı seçip kaynağa bağlayabilirsiniz.
          </p>
        </div>
        <button className="btn ghost" onClick={onClose}>Kapat</button>
      </div>

      <div className="reader-dual">
        <ReaderPane
          caseId={caseId}
          label="Sol belge"
          documents={documents}
          versionId={leftVersion}
          target={left}
          otherVersionId={rightVersion}
          onPickVersion={setLeftVersion}
          onAnchor={onAnchor}
          onNote={onNote}
        />
        <ReaderPane
          caseId={caseId}
          label="Sağ belge"
          documents={documents}
          versionId={rightVersion}
          target={right}
          otherVersionId={leftVersion}
          onPickVersion={setRightVersion}
          onAnchor={onAnchor}
          onNote={onNote}
        />
      </div>
      <p className="hint" style={{ margin: 0 }}>
        Sayfa düğmeleri <strong>fiziksel sırayı</strong> gösterir. PDF&apos;te{" "}
        <code>/PageLabels</code> tanımlıysa basılı etiket ayrıca yazılır; tanımlı değilse etiket
        uydurulmaz. DOCX/UDF belgede sayfa <strong>türevdir</strong>; paragraf/bölüm yolu ayrı
        gösterilir. Yakınlaştırma ve döndürme yalnız görünümü etkiler; kayıtlı sayfa verisi ve
        alıntı offsetleri değişmez. Alıntı segment metninde birebir bulunamazsa kaydedilmez.
      </p>
    </div>
  );
}
