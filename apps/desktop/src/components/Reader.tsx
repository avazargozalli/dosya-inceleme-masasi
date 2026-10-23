import { useEffect, useRef, useState } from "react";
import { api, toApiError, type ApiError, type AnchorRow, type DocumentPages } from "../lib/api";

/** Belge okuyucu: sayfa metnini kaynak sırasıyla gösterir. Görüntü render'ı
 *  F07 devamında; bu sürüm metin katmanını gösterir ve uyarıları açık yazar. */
export function Reader({
  caseId,
  versionId,
  title,
  onClose,
  onAnchor,
}: {
  caseId: string;
  versionId: string;
  title: string;
  onClose: () => void;
  onAnchor?: (anchor: AnchorRow) => void;
}) {
  const [data, setData] = useState<DocumentPages | null>(null);
  const [page, setPage] = useState(0);
  const [error, setError] = useState<ApiError | null>(null);
  const [selection, setSelection] = useState<{ quote: string; start: number; end: number } | null>(null);
  const [savedQuote, setSavedQuote] = useState<string | null>(null);
  const textRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    (async () => {
      try {
        const result = await api.documentPages(versionId);
        setData(result);
      } catch (e) {
        setError(toApiError(e));
      }
    })();
  }, [versionId]);

  const current = data?.pages[page];

  /** Seçimi Unicode scalar aralığına çevirir; DOM offset'i UTF-16'dir. */
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
    let utf16Start = range.startContainer === node ? 0 : range.startOffset;
    if (full.slice(utf16Start, utf16Start + quote.length) !== quote) {
      const idx = full.indexOf(quote);
      if (idx < 0) {
        setSelection(null);
        return;
      }
      utf16Start = idx;
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
        documentVersionId: versionId,
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

  return (
    <div className="card" style={{ borderColor: "var(--line-strong)" }}>
      <div className="card-head">
        <div>
          <h2>{title}</h2>
          <p className="card-sub">
            {data ? `${data.pages.length} sayfa · ${data.parseRun.engine} ${data.parseRun.engineVersion}` : "yükleniyor…"}
          </p>
        </div>
        <button className="btn ghost" onClick={onClose}>Kapat</button>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      {!data ? (
        <div className="empty"><p style={{ margin: 0 }}>Ayrıştırılmış metin yok. Önce “Ayrıştır” çalıştırın.</p></div>
      ) : (
        <>
          <div className="row" style={{ gap: 6, flexWrap: "wrap" }}>
            {data.pages.map((p, index) => (
              <button
                key={p.id}
                className={index === page ? "btn primary" : "btn"}
                style={{ height: 28, minWidth: 38, padding: "0 8px" }}
                onClick={() => setPage(index)}
                title={p.textOrigin === "none" ? "metin katmanı yok" : "metin var"}
              >
                {p.pageIndex + 1}
              </button>
            ))}
          </div>

          {current ? (
            <div>
              <div className="row" style={{ justifyContent: "space-between", marginBottom: 8 }}>
                <span style={{ display: "flex", gap: 6, alignItems: "center" }}>
                  <span className="badge plain">sayfa {current.pageIndex + 1}</span>
                  <span className={`badge ${current.textOrigin === "native" ? "ok" : "warn"}`}>
                    {current.textOrigin === "native" ? "metin katmanı" : "metin yok / OCR gerekli"}
                  </span>
                </span>
                <button
                  className="btn"
                  disabled={!selection || !current.segmentId}
                  onClick={() => void saveAnchor()}
                  title={current.segmentId ? "Seçili metni kaynak olarak kaydet" : "bu sayfada metin segmenti yok"}
                >
                  Seçimi kaynak olarak kaydet
                </button>
              </div>
              <div
                ref={textRef}
                onMouseUp={readSelection}
                onKeyUp={readSelection}
                style={{
                  background: "var(--surface)",
                  border: "1px solid var(--line)",
                  borderRadius: "var(--radius)",
                  padding: "var(--space-4)",
                  whiteSpace: "pre-wrap",
                  fontFamily: "var(--mono)",
                  fontSize: 12.5,
                  lineHeight: 1.7,
                  maxHeight: 420,
                  overflow: "auto",
                }}
              >
                {current.text.trim() || "(bu sayfada metin katmanı yok)"}
              </div>
            </div>
          ) : null}

          {selection ? (
            <div className="notice">
              Seçim: “{selection.quote.length > 120 ? `${selection.quote.slice(0, 120)}…` : selection.quote}”
              {" · "}karakter {selection.start}–{selection.end}
            </div>
          ) : null}
          {savedQuote ? <div className="notice ok">Kaynak kaydedildi: “{savedQuote.slice(0, 80)}”</div> : null}
          <p className="hint" style={{ margin: 0 }}>
            Sayfa numarası fiziksel sayfa sırasıdır; belgenin basılı numarası farklı olabilir.
            Alıntı, segment metninde birebir bulunamazsa kaydedilmez.
          </p>
        </>
      )}
    </div>
  );
}
