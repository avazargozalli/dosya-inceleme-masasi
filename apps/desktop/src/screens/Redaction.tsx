import { useEffect, useRef, useState } from "react";
import { api, toApiError, type ApiError, type CaseRow, type DocumentRow, type RedactionResult } from "../lib/api";

export function Redaction() {
  const [cases, setCases] = useState<CaseRow[]>([]);
  const [caseId, setCaseId] = useState("");
  const [docs, setDocs] = useState<DocumentRow[]>([]);
  const [versionId, setVersionId] = useState("");
  const [terms, setTerms] = useState("");
  const [result, setResult] = useState<RedactionResult | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  const [mode, setMode] = useState<"terms" | "regions">("terms");
  const [page, setPage] = useState(0);
  const [preview, setPreview] = useState<{ src: string; dpi: number } | null>(null);
  const [regions, setRegions] = useState<Array<{ pageIndex: number; rect: [number, number, number, number] }>>([]);
  const [drag, setDrag] = useState<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  const imageRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const rows = await api.caseList(false);
        setCases(rows);
        if (rows.length > 0) setCaseId((prev) => prev || rows[0]!.id);
      } catch (e) {
        setError(toApiError(e));
      }
    })();
  }, []);

  useEffect(() => {
    if (!caseId) return;
    void api
      .documentList(caseId)
      .then((list) => {
        setDocs(list);
        setVersionId(list.find((d) => d.currentVersionId)?.currentVersionId ?? "");
      })
      .catch((e) => setError(toApiError(e)));
  }, [caseId]);

  async function loadPage() {
    if (!versionId) return;
    setBusy(true);
    setError(null);
    try {
      const result = await api.documentPagePreview(versionId, page);
      setPreview({ src: `data:image/png;base64,${result.pngBase64}`, dpi: result.dpi });
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function applyRegions() {
    if (regions.length === 0) {
      setError({ code: "E_REDACT_REGIONS", message: "En az bir bölge seçin.", retryable: false });
      return;
    }
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      setResult(await api.documentRedactRegions(versionId, regions));
      setRegions([]);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  function point(event: React.PointerEvent<HTMLDivElement>) {
    const box = imageRef.current?.getBoundingClientRect();
    if (!box) return { x: 0, y: 0 };
    return { x: event.clientX - box.left, y: event.clientY - box.top };
  }

  function startDrag(event: React.PointerEvent<HTMLDivElement>) {
    const { x, y } = point(event);
    setDrag({ x0: x, y0: y, x1: x, y1: y });
  }

  function moveDrag(event: React.PointerEvent<HTMLDivElement>) {
    if (!drag) return;
    const { x, y } = point(event);
    setDrag({ ...drag, x1: x, y1: y });
  }

  function endDrag() {
    if (!drag || !preview) return;
    const scale = 72 / preview.dpi;
    const rect: [number, number, number, number] = [
      Math.min(drag.x0, drag.x1) * scale,
      Math.min(drag.y0, drag.y1) * scale,
      Math.max(drag.x0, drag.x1) * scale,
      Math.max(drag.y0, drag.y1) * scale,
    ];
    if (rect[2] - rect[0] >= 2 && rect[3] - rect[1] >= 2) {
      setRegions((prev) => [...prev, { pageIndex: page, rect }]);
    }
    setDrag(null);
  }

  async function run() {
    const list = terms
      .split(/[\n,]/)
      .map((t) => t.trim())
      .filter(Boolean);
    if (!versionId || list.length === 0) {
      setError({ code: "E_TERMS", message: "Belge ve en az bir terim gerekli.", retryable: false });
      return;
    }
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      setResult(await api.documentRedact(versionId, list));
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page" style={{ maxWidth: 860 }}>
      <div className="page-head">
        <h1>Maskeleme</h1>
        <p className="lead">
          Terimler belgenin metin katmanından ve görüntüsünden gerçekten çıkarılır; siyah kutu
          çizmek yeterli sayılmaz. Çıktı yeniden açılıp doğrulanmadan kaydedilmez.
        </p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="card">
        <div className="row" style={{ gap: 6, marginBottom: 8 }}>
          <button
            className={`btn ${mode === "terms" ? "primary" : "ghost"}`}
            onClick={() => setMode("terms")}
          >
            Terimle maskele
          </button>
          <button
            className={`btn ${mode === "regions" ? "primary" : "ghost"}`}
            onClick={() => setMode("regions")}
          >
            Bölge seçerek maskele
          </button>
        </div>
        <div className="row">
          <div className="field">
            <label htmlFor="rd-case">Dosya</label>
            <select id="rd-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
              {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
            </select>
          </div>
          <div className="field">
            <label htmlFor="rd-doc">Belge</label>
            <select id="rd-doc" className="input" value={versionId} onChange={(e) => setVersionId(e.target.value)}>
              <option value="">Seçin…</option>
              {docs.filter((d) => d.currentVersionId).map((d) => (
                <option key={d.id} value={d.currentVersionId!}>{d.displayName}</option>
              ))}
            </select>
          </div>
        </div>
        {mode === "terms" ? (
        <div className="field">
          <label htmlFor="rd-terms">Maskelenecek terimler (her satır veya virgül ayrı)</label>
          <textarea
            id="rd-terms"
            className="input"
            style={{ height: 90, padding: 10, fontFamily: "var(--mono)", fontSize: 12 }}
            value={terms}
            onChange={(e) => setTerms(e.target.value)}
            placeholder={"Ahmet Yilmaz\n00000000000"}
          />
          <span className="hint">Terimler birebir aranır; eşleşme yoksa o terim için bölge oluşmaz.</span>
        </div>
        ) : (
          <div>
            <div className="row" style={{ alignItems: "flex-end", gap: 8 }}>
              <div className="field" style={{ maxWidth: 120 }}>
                <label htmlFor="rd-page">Sayfa (0'dan)</label>
                <input
                  id="rd-page"
                  className="input"
                  type="number"
                  min={0}
                  value={page}
                  onChange={(e) => setPage(Math.max(0, Number(e.target.value)))}
                />
              </div>
              <button className="btn" disabled={busy || !versionId} onClick={() => void loadPage()}>
                Sayfayı getir
              </button>
              <button
                className="btn ghost"
                disabled={regions.length === 0}
                onClick={() => setRegions([])}
              >
                Seçimleri temizle ({regions.length})
              </button>
            </div>
            {preview ? (
              <div
                ref={imageRef}
                style={{ position: "relative", display: "inline-block", marginTop: 8, touchAction: "none", cursor: "crosshair" }}
                onPointerDown={startDrag}
                onPointerMove={moveDrag}
                onPointerUp={endDrag}
              >
                <img src={preview.src} alt={`Sayfa ${page + 1} önizleme`} style={{ display: "block", maxWidth: "100%" }} />
                {regions
                  .filter((region) => region.pageIndex === page)
                  .map((region, index) => {
                    const scale = preview.dpi / 72;
                    const left = region.rect[0] * scale;
                    const top = region.rect[1] * scale;
                    return (
                      <span
                        key={`${region.pageIndex}-${index}`}
                        style={{
                          position: "absolute",
                          left,
                          top,
                          width: (region.rect[2] - region.rect[0]) * scale,
                          height: (region.rect[3] - region.rect[1]) * scale,
                          border: "1px solid var(--accent)",
                          background: "rgba(0,0,0,0.35)",
                        }}
                      />
                    );
                  })}
                {drag ? (
                  <span
                    style={{
                      position: "absolute",
                      left: Math.min(drag.x0, drag.x1),
                      top: Math.min(drag.y0, drag.y1),
                      width: Math.abs(drag.x1 - drag.x0),
                      height: Math.abs(drag.y1 - drag.y0),
                      border: "1px dashed var(--foreground)",
                    }}
                  />
                ) : null}
              </div>
            ) : (
              <p className="hint" style={{ marginTop: 8 }}>
                Sayfayı getirin, fareyle dikdörtgen çizin. Seçimler punto koordinatına çevrilir.
              </p>
            )}
          </div>
        )}
        <div className="row" style={{ justifyContent: "flex-end" }}>
          {mode === "terms" ? (
            <button className="btn primary" disabled={busy || !versionId} onClick={() => void run()}>
              {busy ? "Maskeleniyor…" : "Maskeli kopya üret"}
            </button>
          ) : (
            <button className="btn primary" disabled={busy || !versionId || regions.length === 0} onClick={() => void applyRegions()}>
              {busy ? "Maskeleniyor…" : `Seçili ${regions.length} bölgeyi maskele`}
            </button>
          )}
        </div>
      </div>

      {result ? (
        <div className="card">
          <div className="card-head">
            <div>
              <h2>Maskeli kopya hazır</h2>
              <p className="card-sub">Yeni sürüm olarak kaydedildi; özgün sürüm değişmedi.</p>
            </div>
            <span className="badge ok">doğrulandı</span>
          </div>
          <dl className="kv">
            <dt>Yeni belge</dt><dd>{result.document.displayName}</dd>
            <dt>Sürüm</dt><dd>{result.version.versionNo}</dd>
            <dt>Maskelenen bölge</dt><dd>{result.hits}</dd>
            <dt>Kalan terim</dt><dd>{result.remaining.length === 0 ? "yok" : result.remaining.join(", ")}</dd>
            <dt>Doğrulayan motorlar</dt><dd>{result.engines.join(", ")}</dd>
            <dt>Bağımsız tarama</dt>
            <dd>
              {result.streamsScanned} akış açıldı · {result.streamsBinary} ikili akış
              {result.streamsUnreadable > 0 ? (
                <>
                  {" · "}
                  <span style={{ color: "var(--danger)" }}>
                    {result.streamsUnreadable} akış açılamadı
                  </span>
                </>
              ) : null}
            </dd>
            {result.annotationsRemoved +
              result.offpageSpansRemoved +
              result.scriptsRemoved >
            0 ? (
              <>
                <dt>Gizli nesne temizliği</dt>
                <dd>
                  {result.annotationsRemoved} açıklama · {result.offpageSpansRemoved} sayfa dışı
                  metin · {result.scriptsRemoved} JavaScript
                </dd>
              </>
            ) : null}
            <dt>Görüntü (OCR) doğrulaması</dt>
            <dd>{result.ocrVerified ? "evet" : "hayır — tesseract yok, yalnız metin katmanı doğrulandı"}</dd>
            {result.warnings.length > 0 ? (
              <>
                <dt>Doğrulama uyarıları</dt>
                <dd>
                  <ul style={{ margin: 0, paddingLeft: 16 }}>
                    {result.warnings.map((warning) => (
                      <li key={warning}>{warning}</li>
                    ))}
                  </ul>
                </dd>
              </>
            ) : null}
            <dt>Plan</dt><dd style={{ fontFamily: "var(--mono)", fontSize: 11 }}>{result.plan.id}</dd>
          </dl>
          <p className="hint" style={{ margin: 0 }}>
            Bu kopya paylaşım içindir. Özgün belge ve eski sürümler kasadan silinmedi.
          </p>
        </div>
      ) : null}
    </div>
  );
}
