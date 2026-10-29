import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  toApiError,
  type ApiError,
  type CaseRow,
  type DocumentRow,
  type ParseProgressEvent,
  type RedactionResult,
} from "../lib/api";
import { t } from "../lib/i18n";
import { parseProgressLine, parseProgressWidth } from "../lib/parseProgress";

export function Redaction() {
  const [cases, setCases] = useState<CaseRow[]>([]);
  const [caseId, setCaseId] = useState("");
  const [docs, setDocs] = useState<DocumentRow[]>([]);
  const [versionId, setVersionId] = useState("");
  const [terms, setTerms] = useState("");
  const [result, setResult] = useState<RedactionResult | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  // F04-T07: maskeleme artık kuyruk işi olarak koşar; aşama/sayım
  // `redact-progress` olayıyla gelir. Sayaç uydurulmaz: olay yoksa satır yok.
  const [progress, setProgress] = useState<ParseProgressEvent | null>(null);
  const activeVersion = useRef<string | null>(null);
  const [mode, setMode] = useState<"terms" | "regions">("terms");
  const [page, setPage] = useState(0);
  const [preview, setPreview] = useState<{ src: string; dpi: number } | null>(null);
  const [regions, setRegions] = useState<Array<{ pageIndex: number; rect: [number, number, number, number] }>>([]);
  const [drag, setDrag] = useState<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  const imageRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    // Tarayıcıda (Tauri yok) dinleyici kurulamaz; hata yutulur.
    void listen<ParseProgressEvent>("redact-progress", (event) => {
      if (disposed) return;
      if (event.payload.versionId !== activeVersion.current) return;
      setProgress(event.payload);
    })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

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
      setError({ code: "E_REDACT_REGIONS", message: t("redaction.regionsRequired"), retryable: false });
      return;
    }
    setBusy(true);
    setError(null);
    setResult(null);
    activeVersion.current = versionId;
    setProgress(null);
    try {
      setResult(await api.documentRedactRegions(versionId, regions));
      setRegions([]);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      activeVersion.current = null;
      setProgress(null);
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
      setError({ code: "E_TERMS", message: t("redaction.termsRequired"), retryable: false });
      return;
    }
    setBusy(true);
    setError(null);
    setResult(null);
    activeVersion.current = versionId;
    setProgress(null);
    try {
      setResult(await api.documentRedact(versionId, list));
    } catch (e) {
      setError(toApiError(e));
    } finally {
      activeVersion.current = null;
      setProgress(null);
      setBusy(false);
    }
  }

  return (
    <div className="page" style={{ maxWidth: 860 }}>
      <div className="page-head">
        <h1>{t("nav.redaction")}</h1>
        <p className="lead">{t("redaction.lead")}</p>
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
            {t("redaction.modeTerms")}
          </button>
          <button
            className={`btn ${mode === "regions" ? "primary" : "ghost"}`}
            onClick={() => setMode("regions")}
          >
            {t("redaction.modeRegions")}
          </button>
        </div>
        <div className="row">
          <div className="field">
            <label htmlFor="rd-case">{t("common.case")}</label>
            <select id="rd-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
              {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
            </select>
          </div>
          <div className="field">
            <label htmlFor="rd-doc">{t("redaction.docLabel")}</label>
            <select id="rd-doc" className="input" value={versionId} onChange={(e) => setVersionId(e.target.value)}>
              <option value="">{t("common.select")}</option>
              {docs.filter((d) => d.currentVersionId).map((d) => (
                <option key={d.id} value={d.currentVersionId!}>{d.displayName}</option>
              ))}
            </select>
          </div>
        </div>
        {mode === "terms" ? (
        <div className="field">
          <label htmlFor="rd-terms">{t("redaction.termsLabel")}</label>
          <textarea
            id="rd-terms"
            className="input"
            style={{ height: 90, padding: 10, fontFamily: "var(--mono)", fontSize: 12 }}
            value={terms}
            onChange={(e) => setTerms(e.target.value)}
            placeholder={"Ahmet Yilmaz\n00000000000"}
          />
          <span className="hint">{t("redaction.termsHint")}</span>
        </div>
        ) : (
          <div>
            <div className="row" style={{ alignItems: "flex-end", gap: 8 }}>
              <div className="field" style={{ maxWidth: 120 }}>
                <label htmlFor="rd-page">{t("redaction.pageLabel")}</label>
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
                {t("redaction.loadPage")}
              </button>
              <button
                className="btn ghost"
                disabled={regions.length === 0}
                onClick={() => setRegions([])}
              >
                {t("redaction.clearRegions", { count: regions.length })}
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
                <img src={preview.src} alt={t("redaction.previewAlt", { page: page + 1 })} style={{ display: "block", maxWidth: "100%" }} />
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
                {t("redaction.previewHint")}
              </p>
            )}
          </div>
        )}
        <div className="row" style={{ justifyContent: "flex-end" }}>
          {mode === "terms" ? (
            <button className="btn primary" disabled={busy || !versionId} onClick={() => void run()}>
              {busy ? t("redaction.masking") : t("redaction.runTerms")}
            </button>
          ) : (
            <button className="btn primary" disabled={busy || !versionId || regions.length === 0} onClick={() => void applyRegions()}>
              {busy ? t("redaction.masking") : t("redaction.runRegions", { count: regions.length })}
            </button>
          )}
        </div>
        {busy ? (
          <div style={{ marginTop: 10 }}>
            <div className="hint" role="status" style={{ marginBottom: 4 }}>
              {parseProgressLine(progress) ?? t("redaction.preparing")}
            </div>
            {parseProgressWidth(progress) === null ? null : (
              <div
                role="progressbar"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={parseProgressWidth(progress) ?? undefined}
                style={{ height: 6, background: "var(--border)", borderRadius: 3, overflow: "hidden" }}
              >
                <div
                  style={{
                    width: `${parseProgressWidth(progress) ?? 0}%`,
                    height: "100%",
                    background: "var(--accent)",
                    transition: "width 120ms linear",
                  }}
                />
              </div>
            )}
            <p className="hint" style={{ marginTop: 4, marginBottom: 0 }}>
              {t("redaction.busyHint")}
            </p>
          </div>
        ) : null}
      </div>

      {result ? (
        <div className="card">
          <div className="card-head">
            <div>
              <h2>{t("redaction.resultTitle")}</h2>
              <p className="card-sub">{t("redaction.resultSub")}</p>
            </div>
            <span className="badge ok">{t("redaction.verified")}</span>
          </div>
          <dl className="kv">
            <dt>{t("redaction.newDocument")}</dt><dd>{result.document.displayName}</dd>
            <dt>{t("redaction.version")}</dt><dd>{result.version.versionNo}</dd>
            <dt>{t("redaction.maskedRegions")}</dt><dd>{result.hits}</dd>
            <dt>{t("redaction.remaining")}</dt><dd>{result.remaining.length === 0 ? t("common.none") : result.remaining.join(", ")}</dd>
            <dt>{t("redaction.engines")}</dt><dd>{result.engines.join(", ")}</dd>
            <dt>{t("redaction.streams")}</dt>
            <dd>
              {t("redaction.streamsLine", { scanned: result.streamsScanned, binary: result.streamsBinary })}
              {result.streamsUnreadable > 0 ? (
                <>
                  {" · "}
                  <span style={{ color: "var(--danger)" }}>
                    {t("redaction.unreadableStreams", { count: result.streamsUnreadable })}
                  </span>
                </>
              ) : null}
            </dd>
            {result.annotationsRemoved +
              result.offpageSpansRemoved +
              result.scriptsRemoved >
            0 ? (
              <>
                <dt>{t("redaction.hiddenObjects")}</dt>
                <dd>
                  {t("redaction.hiddenObjectsLine", {
                    annotations: result.annotationsRemoved,
                    offpage: result.offpageSpansRemoved,
                    scripts: result.scriptsRemoved,
                  })}
                </dd>
              </>
            ) : null}
            <dt>{t("redaction.ocrTitle")}</dt>
            <dd>{result.ocrVerified ? t("redaction.ocrYes") : t("redaction.ocrNo")}</dd>
            {result.warnings.length > 0 ? (
              <>
                <dt>{t("redaction.warnings")}</dt>
                <dd>
                  <ul style={{ margin: 0, paddingLeft: 16 }}>
                    {result.warnings.map((warning) => (
                      <li key={warning}>{warning}</li>
                    ))}
                  </ul>
                </dd>
              </>
            ) : null}
            <dt>{t("redaction.plan")}</dt><dd style={{ fontFamily: "var(--mono)", fontSize: 11 }}>{result.plan.id}</dd>
          </dl>
          <p className="hint" style={{ margin: 0 }}>
            {t("redaction.shareHint")}
          </p>
        </div>
      ) : null}
    </div>
  );
}
