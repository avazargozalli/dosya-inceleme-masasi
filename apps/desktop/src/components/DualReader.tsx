import { useEffect, useRef, useState } from "react";
import {
  api,
  toApiError,
  type AnchorRow,
  type ApiError,
  type DocumentPages,
  type DocumentRow,
  type PageRow,
} from "../lib/api";
import {
  blocksForRange,
  clampZoom,
  displayPageSize,
  mapRectForDisplay,
  nextZoom,
  pageLabelInfo,
  parseWarnings,
  rangeForPage,
  resolvePageIndex,
  rotateView,
  sliceForPage,
  type ReaderTarget,
} from "../lib/reader";

/** Sayfa haritası: ayrıştırıcının **gerçekten ürettiği** blok kutuları.
 *  Sayfanın görüntüsü değildir; kutu yoksa hiç çizilmez (konum uydurulmaz).
 *  `rotation` belgenin kendi döndürmesi ile görünüm döndürmesinin toplamıdır. */
function PageMap({
  page,
  highlight,
  rotation,
}: {
  page: PageRow;
  highlight: [number, number, number, number][];
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
  onPickVersion,
  onAnchor,
}: {
  caseId: string;
  label: string;
  documents: DocumentRow[];
  versionId: string | null;
  target: ReaderTarget | null;
  onPickVersion: (versionId: string) => void;
  onAnchor?: ((anchor: AnchorRow) => void) | undefined;
}) {
  const [data, setData] = useState<DocumentPages | null>(null);
  const [page, setPage] = useState(0);
  const [error, setError] = useState<ApiError | null>(null);
  const [loading, setLoading] = useState(false);
  const [selection, setSelection] = useState<{ quote: string; start: number; end: number } | null>(null);
  const [savedQuote, setSavedQuote] = useState<string | null>(null);
  // F07-T06: yakınlaştırma ve görünüm döndürmesi yalnız bu panelin
  // gösterimini etkiler; `pages.rotation` ve metin offsetleri değişmez.
  const [zoom, setZoom] = useState(1);
  const [viewRotation, setViewRotation] = useState(0);
  const textRef = useRef<HTMLDivElement>(null);
  const markRef = useRef<HTMLElement>(null);

  const paneTarget = target && target.versionId === versionId ? target : null;

  useEffect(() => {
    if (versionId) return;
    const first = documents.find((doc) => doc.currentVersionId);
    if (first?.currentVersionId) onPickVersion(first.currentVersionId);
  }, [versionId, documents, onPickVersion]);

  useEffect(() => {
    if (!versionId) {
      setData(null);
      return;
    }
    let cancelled = false;
    // Sürüm değişince önceki belgenin sayfaları gösterilmez; hedef yeni
    // belgenin sayfaları yüklenince çözülür.
    setData(null);
    setPage(0);
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

  // Hedef (arama sonucu veya kaynak bağ) geldiğinde doğru sayfayı aç.
  useEffect(() => {
    if (!data) return;
    const index = resolvePageIndex(data.pages, paneTarget);
    if (index >= 0) setPage(index);
  }, [data, paneTarget]);

  useEffect(() => {
    setSelection(null);
    setSavedQuote(null);
  }, [page, versionId]);

  // Belge değişince yakınlaştırma/döndürme sıfırlanır: önceki belgenin
  // görünüm ayarı yeni belgeye sessizce taşınmaz.
  useEffect(() => {
    setZoom(1);
    setViewRotation(0);
  }, [versionId]);

  const current: PageRow | undefined = data?.pages[page];
  // Toplam döndürme: belgenin kendi değeri + kullanıcının görünüm ayarı.
  const rotation = rotateView(current?.rotation ?? 0, viewRotation);
  const zoomPercent = Math.round(clampZoom(zoom) * 100);
  const currentInfo = current ? pageLabelInfo(current) : null;
  const slice = current ? sliceForPage(current.text, paneTarget, current) : null;
  const highlightRange = current ? rangeForPage(current.text, paneTarget, current) : null;
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
      setSelection(null);
      return;
    }
    const range = sel.getRangeAt(0);
    const quote = sel.toString();
    const full = node.textContent ?? "";
    if (!quote.trim() || !node.contains(range.startContainer)) {
      setSelection(null);
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
      setSelection(null);
      return;
    }
    if (full.slice(utf16Start, utf16Start + quote.length) !== quote) {
      setSelection(null);
      return;
    }
    const start = Array.from(full.slice(0, utf16Start)).length;
    const end = start + Array.from(quote).length;
    setSelection({ quote, start, end });
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
      setSelection(null);
    } catch (e) {
      setError(toApiError(e));
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
      style={{
        minWidth: 0,
        display: "flex",
        flexDirection: "column",
        gap: "var(--space-2)",
        border: "1px solid var(--line)",
        borderRadius: "var(--radius)",
        padding: "var(--space-3)",
        background: "var(--bg)",
      }}
    >
      <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
        <span className="badge plain">{label}</span>
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
                  onClick={() => setPage(index)}
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
              onClick={() => setZoom(nextZoom(zoom, -1))}
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
              onClick={() => setZoom(nextZoom(zoom, 1))}
            >
              +
            </button>
            <button
              className="btn ghost"
              style={{ height: 26 }}
              disabled={zoom === 1}
              onClick={() => setZoom(1)}
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
              onClick={() => setViewRotation(rotateView(viewRotation, -90))}
            >
              ⟲
            </button>
            <button
              className="btn"
              style={{ height: 26, minWidth: 26, padding: "0 6px" }}
              aria-label={`${label}: sağa döndür`}
              title="Sağa döndür (90°)"
              onClick={() => setViewRotation(rotateView(viewRotation, 90))}
            >
              ⟳
            </button>
            <button
              className="btn ghost"
              style={{ height: 26 }}
              disabled={viewRotation === 0}
              onClick={() => setViewRotation(0)}
              title="Görünüm döndürmesini sıfırla (belgenin kendi döndürmesi kalır)"
            >
              Döndürmeyi sıfırla
            </button>
          </div>

          <div className="row" style={{ justifyContent: "space-between", alignItems: "center", gap: 6 }}>
            <span style={{ display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }}>
              <span className="badge plain">
                {currentInfo ? currentInfo.physical : "Fiziksel sayfa ?"}/{data.pages.length}
              </span>
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
            <button
              className="btn ghost"
              style={{ height: 26 }}
              disabled={!selection || !current?.segmentId}
              onClick={() => void saveAnchor()}
              title={current?.segmentId ? "Seçili metni kaynak olarak kaydet" : "bu sayfada metin segmenti yok"}
            >
              Seçimi kaynağa bağla
            </button>
          </div>

          {current && current.coordinateSpace && current.coordinateSpace !== "structure" && current.blocks.length > 0 ? (
            <PageMap page={current} highlight={highlightQuads} rotation={rotation} />
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
              slice ? (
                <>
                  {slice.before}
                  <mark
                    ref={markRef}
                    style={{ background: "var(--accent-soft)", color: "inherit", padding: "0 2px", borderRadius: 2 }}
                  >
                    {slice.hit}
                  </mark>
                  {slice.after}
                </>
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
}: {
  caseId: string;
  documents: DocumentRow[];
  left: ReaderTarget | null;
  right: ReaderTarget | null;
  onClose: () => void;
  onAnchor?: (anchor: AnchorRow) => void;
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
            Her panel kendi belgesini ve sayfasını korur. Alıntı seçip kaynağa bağlayabilirsiniz.
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
          onPickVersion={setLeftVersion}
          onAnchor={onAnchor}
        />
        <ReaderPane
          caseId={caseId}
          label="Sağ belge"
          documents={documents}
          versionId={rightVersion}
          target={right}
          onPickVersion={setRightVersion}
          onAnchor={onAnchor}
        />
      </div>
      <p className="hint" style={{ margin: 0 }}>
        Sayfa düğmeleri <strong>fiziksel sırayı</strong> gösterir. PDF&apos;te{" "}
        <code>/PageLabels</code> tanımlıysa basılı etiket ayrıca yazılır; tanımlı değilse etiket
        uydurulmaz. Yakınlaştırma ve döndürme yalnız görünümü etkiler; kayıtlı sayfa verisi ve
        alıntı offsetleri değişmez. Alıntı segment metninde birebir bulunamazsa kaydedilmez.
      </p>
    </div>
  );
}
