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
import { parseWarnings, resolvePageIndex, sliceForPage, type ReaderTarget } from "../lib/reader";

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

  const current: PageRow | undefined = data?.pages[page];
  const slice = current ? sliceForPage(current.text, paneTarget, current) : null;
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
            {data.pages.map((p, index) => (
              <button
                key={p.id}
                className={index === page ? "btn primary" : "btn"}
                style={{ height: 26, minWidth: 34, padding: "0 7px" }}
                onClick={() => setPage(index)}
                title={p.textOrigin === "none" ? "metin katmanı yok" : "metin var"}
              >
                {p.pageIndex + 1}
              </button>
            ))}
          </div>

          <div className="row" style={{ justifyContent: "space-between", alignItems: "center", gap: 6 }}>
            <span style={{ display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }}>
              <span className="badge plain">
                sayfa {current ? current.pageIndex + 1 : "?"}/{data.pages.length}
              </span>
              {data.parseRun.status === "succeeded" ? null : (
                <span className="badge warn">kısmi ayrıştırma · {data.parseRun.status}</span>
              )}
              {current && current.textOrigin !== "native" ? (
                <span className="badge warn">metin yok / OCR</span>
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

          <div
            ref={textRef}
            onMouseUp={readSelection}
            onKeyUp={readSelection}
            tabIndex={0}
            style={{
              background: "var(--surface)",
              border: "1px solid var(--line)",
              borderRadius: "var(--radius)",
              padding: "var(--space-3)",
              whiteSpace: "pre-wrap",
              fontFamily: "var(--mono)",
              fontSize: 12.5,
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
        Sayfa numarası fiziksel sayfa sırasıdır; belgenin basılı numarası farklı olabilir.
        Alıntı segment metninde birebir bulunamazsa kaydedilmez.
      </p>
    </div>
  );
}
