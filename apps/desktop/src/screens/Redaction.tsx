import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  toApiError,
  type ApiError,
  type CaseRow,
  type DocumentRow,
  type ParseProgressEvent,
  type RedactionPlanRow,
  type RedactionRegionRow,
  type RedactionResult,
} from "../lib/api";
import { formatDateTime } from "../lib/format";
import { t } from "../lib/i18n";
import { parseProgressLine, parseProgressWidth } from "../lib/parseProgress";

/**
 * Plan durumu. Tanınmayan bir durum **gizlenmez** — ham değeriyle yazılır
 * (uydurulmuş Türkçe etiket yok).
 */
function planState(status: string): { label: string; tone: "ok" | "danger" | "warn" | "plain" } {
  switch (status) {
    case "draft":
      return { label: t("redaction.planStateDraft"), tone: "plain" };
    case "reviewing":
      return { label: t("redaction.planStateReviewing"), tone: "warn" };
    case "approved":
      return { label: t("redaction.planStateApproved"), tone: "ok" };
    case "applied":
      return { label: t("redaction.planStateApplied"), tone: "ok" };
    case "failed":
      return { label: t("redaction.planStateFailed"), tone: "danger" };
    default:
      return { label: t("redaction.planStateUnknown", { state: status }), tone: "warn" };
  }
}

/**
 * Bölgeleri **sayfaya göre** sayar. Sıra sayfa numarasıdır — kayıt sırası
 * değil: avukat "hangi sayfada kaç bölge" sorusunu okur. Sayı ekranda
 * hesaplanır, çekirdekten gelen satırlardan türetilir (uydurma yok).
 */
function pageBreakdown(rows: RedactionRegionRow[]): Array<{ page: number; count: number }> {
  const counts = new Map<number, number>();
  for (const row of rows) counts.set(row.pageIndex, (counts.get(row.pageIndex) ?? 0) + 1);
  return [...counts.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([page, count]) => ({ page, count }));
}

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
  // Çalıştırma kimliği (F13 açık kayıt a): aynı sürümde art arda iki
  // maskelemede birincinin **gecikmiş** karesi ikincinin ilerleme satırına
  // yazılmaz. Kimliği arayüz üretir, olay her karede taşır.
  const activeRun = useRef<{ versionId: string; runId: string } | null>(null);
  const runSeq = useRef(0);
  const [mode, setMode] = useState<"terms" | "regions">("terms");
  const [page, setPage] = useState(0);
  // Önizleme **hangi sayfaya aitse** onunla taşınır: sayfa alanı değişince eski
  // görüntü yeni sayfa gibi etiketlenmez ve bölgeler görüntünün sayfasına yazılır.
  const [preview, setPreview] = useState<{ src: string; dpi: number; pageIndex: number } | null>(null);
  const [regions, setRegions] = useState<Array<{ pageIndex: number; rect: [number, number, number, number] }>>([]);
  const [drag, setDrag] = useState<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  const imageRef = useRef<HTMLDivElement | null>(null);
  // Sayfa getirme maskeleme değildir: ayrı durum, ayrı etiket.
  const [pageBusy, setPageBusy] = useState(false);
  // Dosya/sürüm değişimi uçuştaki yanıtları geçersiz kılar (INV-05).
  const genRef = useRef(0);
  const pageSeq = useRef(0);
  // Maskeleme sicili **çekirdekten** okunur: plan ve bölgeler her maskelemede
  // kasaya yazılıyordu ama okuma yolu yoktu — avukat "hangi belgede hangi
  // sayfalarda kaç bölge maskelendi" sorusunu yanıtlayamıyordu.
  const [plans, setPlans] = useState<RedactionPlanRow[]>([]);
  const [plansBusy, setPlansBusy] = useState(false);
  // Okuma hatası **kendi satırında** yazılır: düşen okumadan sonra "kayıt yok"
  // demek, olmayan bir olguyu bildirmek olurdu.
  const [plansError, setPlansError] = useState<ApiError | null>(null);
  const [openPlanId, setOpenPlanId] = useState<string | null>(null);
  const [planRegions, setPlanRegions] = useState<RedactionRegionRow[]>([]);
  const [planRegionsBusy, setPlanRegionsBusy] = useState(false);
  const [planRegionsError, setPlanRegionsError] = useState<ApiError | null>(null);
  // İki ayrı kuşak: plan listesi (dosya + tazeleme) ve bölge detayı (plan
  // seçimi). Tek sayaç olsaydı ilgisiz bir bölge okuması liste tazelemesinin
  // yanıtını düşürürdü.
  const plansSeq = useRef(0);
  const regionsSeq = useRef(0);

  // Hedef (dosya/belge) değişimi tek yerden geçer: uçuştaki yanıtlar
  // geçersizleşir, sayfa kilidi **devredilir** (yeni hedef kendi sayfasını
  // isteyebilir) ve yarım kalan sürükleme yeni önizlemeye taşınmaz.
  const resetTargetState = useCallback(() => {
    // Nesil artışı bütün uçuştaki yanıtları geçersiz kılar. `pageSeq` burada
    // artırılmaz: sayfa isteğinin sırası `loadPage` içinde kendi `++`'ıyla
    // ilerler, hedef değişiminde kapı zaten `gen`dir (ölçüldü: bu satır
    // tek başına hiçbir testi düşürmüyordu → yedekli kapı bırakılmadı).
    genRef.current += 1;
    setResult(null);
    setRegions([]);
    setPreview(null);
    setPage(0);
    setProgress(null);
    // Uçuştaki çalıştırmanın kimliği kapanır: eski hedefin gecikmiş karesi yeni
    // hedefin ilerleme satırına yazılmaz (INV-05).
    activeRun.current = null;
    setError(null);
    setPageBusy(false);
    setDrag(null);
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    // Tarayıcıda (Tauri yok) dinleyici kurulamaz; hata yutulur.
    void listen<ParseProgressEvent>("redact-progress", (event) => {
      if (disposed) return;
      // Yalnız **kendi** çalıştırmamızın kareleri: sürüm kimliği tek başına
      // yetmez, aynı sürümde art arda iki maskelemede eski çalıştırmanın
      // gecikmiş karesi yeni satıra yazılırdı.
      const active = activeRun.current;
      if (!active) return;
      if (event.payload.versionId !== active.versionId) return;
      if (event.payload.runId !== active.runId) return;
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

  const loadPlans = useCallback(async (id: string) => {
    if (!id) {
      // Dosya seçimi kalktıysa askıdaki okuma da geçersizdir.
      plansSeq.current += 1;
      return;
    }
    const seq = ++plansSeq.current;
    setPlansBusy(true);
    try {
      const rows = await api.redactionPlanList(id);
      if (seq !== plansSeq.current) return;
      setPlans(rows);
      setPlansError(null);
    } catch (e) {
      if (seq !== plansSeq.current) return;
      // Sessiz yutma yok: okunamayan sicil "kayıt yok" diye gösterilmez.
      setPlansError(toApiError(e));
    } finally {
      if (seq === plansSeq.current) setPlansBusy(false);
    }
  }, []);

  async function loadRegions(planId: string): Promise<void> {
    const seq = ++regionsSeq.current;
    setPlanRegionsBusy(true);
    setPlanRegionsError(null);
    try {
      const rows = await api.redactionPlanRegions(planId);
      if (seq !== regionsSeq.current) return;
      setPlanRegions(rows);
    } catch (e) {
      if (seq !== regionsSeq.current) return;
      // Düşen okumada eski planın bölgeleri yeni planınki gibi gösterilmez.
      setPlanRegions([]);
      setPlanRegionsError(toApiError(e));
    } finally {
      if (seq === regionsSeq.current) setPlanRegionsBusy(false);
    }
  }

  // Bölge detayı **aç/kapa**: aynı plana ikinci tıklama kapatır ve uçuştaki
  // okumayı düşürür; başka plana geçiş önceki satırları hemen temizler.
  function togglePlan(planId: string) {
    regionsSeq.current += 1;
    if (openPlanId === planId) {
      setOpenPlanId(null);
      setPlanRegions([]);
      setPlanRegionsError(null);
      setPlanRegionsBusy(false);
      return;
    }
    setOpenPlanId(planId);
    setPlanRegions([]);
    setPlanRegionsError(null);
    void loadRegions(planId);
  }

  useEffect(() => {
    if (!caseId) return;
    // INV-05: yeni dosyanın isteği askıdayken **önceki dosyanın** belgeleri,
    // sürümü, sonucu, önizlemesi ve bölgeleri ekranda kalmasın. Temizlik
    // istekten önce koşar; `gen` geç gelen yanıtı geçersiz kılar.
    resetTargetState();
    const gen = genRef.current;
    setDocs([]);
    setVersionId("");
    // Maskeleme sicili de dosyaya kapsanır: önceki dosyanın planları ve açık
    // bölge detayı **istekten önce** temizlenir (geç yanıt yeni dosyaya yazılmaz).
    setPlans([]);
    setPlansError(null);
    setOpenPlanId(null);
    setPlanRegions([]);
    setPlanRegionsError(null);
    setPlanRegionsBusy(false);
    regionsSeq.current += 1;
    void loadPlans(caseId);
    void api
      .documentList(caseId)
      .then((list) => {
        if (gen !== genRef.current) return;
        setDocs(list);
        setVersionId(list.find((d) => d.currentVersionId)?.currentVersionId ?? "");
      })
      .catch((e) => {
        if (gen !== genRef.current) return;
        setDocs([]);
        setVersionId("");
        setError(toApiError(e));
      });
  }, [caseId, loadPlans, resetTargetState]);

  async function loadPage() {
    if (!versionId) return;
    const gen = genRef.current;
    const requested = page;
    const seq = ++pageSeq.current;
    setPageBusy(true);
    setError(null);
    try {
      const result = await api.documentPagePreview(versionId, requested);
      if (gen !== genRef.current || seq !== pageSeq.current) return;
      setPreview({
        src: `data:image/png;base64,${result.pngBase64}`,
        dpi: result.dpi,
        pageIndex: requested,
      });
    } catch (e) {
      if (gen !== genRef.current || seq !== pageSeq.current) return;
      setError(toApiError(e));
    } finally {
      // Kilit yalnız **kendi** isteğine aittir: hedef değişip yeni bir sayfa
      // isteği başladıysa geç yanıt onun kilidini erken açmaz.
      if (seq === pageSeq.current) setPageBusy(false);
    }
  }

  async function applyRegions() {
    if (regions.length === 0) {
      setError({ code: "E_REDACT_REGIONS", message: t("redaction.regionsRequired"), retryable: false });
      return;
    }
    const gen = genRef.current;
    const version = versionId;
    const submitted = regions;
    const runId = `redact-${++runSeq.current}`;
    setBusy(true);
    setError(null);
    setResult(null);
    activeRun.current = { versionId: version, runId };
    setProgress(null);
    try {
      const result = await api.documentRedactRegions(version, regions, runId);
      if (gen !== genRef.current) return;
      setResult(result);
      // Sicil **gönderimden hemen sonra** tazelenir: yeni plan satırı ekranda
      // (satır `result.plan.id` ile işaretlenir); avukat sormak zorunda kalmaz.
      void loadPlans(caseId);
      // Yalnız **gönderilen** bölgeler düşer: maskeleme sürerken çizilen yeni
      // bölge sessizce silinmez (gönderim yükü tıklama anındaki listedir).
      setRegions((prev) => prev.filter((region) => !submitted.includes(region)));
    } catch (e) {
      if (gen !== genRef.current) return;
      setError(toApiError(e));
    } finally {
      // Kapanış yalnız **kendi** çalıştırmamızın kimliğinde temizler.
      if (activeRun.current?.runId === runId) {
        activeRun.current = null;
        setProgress(null);
      }
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
    // Bölge, **görüntünün ait olduğu sayfaya** yazılır; sayfa alanı sonradan
    // değişmiş olsa bile eski görüntüdeki seçim yeni sayfaya taşınmaz.
    const pageIndex = preview.pageIndex;
    const scale = 72 / preview.dpi;
    const rect: [number, number, number, number] = [
      Math.min(drag.x0, drag.x1) * scale,
      Math.min(drag.y0, drag.y1) * scale,
      Math.max(drag.x0, drag.x1) * scale,
      Math.max(drag.y0, drag.y1) * scale,
    ];
    if (rect[2] - rect[0] >= 2 && rect[3] - rect[1] >= 2) {
      setRegions((prev) => [...prev, { pageIndex, rect }]);
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
    const gen = genRef.current;
    const version = versionId;
    const runId = `redact-${++runSeq.current}`;
    setBusy(true);
    setError(null);
    setResult(null);
    activeRun.current = { versionId: version, runId };
    setProgress(null);
    try {
      const result = await api.documentRedact(version, list, runId);
      if (gen !== genRef.current) return;
      setResult(result);
      // Yeni plan satırı sicilde **hemen** görünür (gönderim sonrası tazeleme).
      void loadPlans(caseId);
    } catch (e) {
      if (gen !== genRef.current) return;
      setError(toApiError(e));
    } finally {
      if (activeRun.current?.runId === runId) {
        activeRun.current = null;
        setProgress(null);
      }
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
            <select id="rd-case" className="input" value={caseId} disabled={busy}
              onChange={(e) => {
                // Hedef hemen geçersizleşir: uçuştaki belge/sayfa/maskeleme
                // yanıtı yeni dosyanın altına yazılmaz (INV-05).
                genRef.current += 1;
                pageSeq.current += 1;
                setCaseId(e.target.value);
              }}>
              {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
            </select>
          </div>
          <div className="field">
            <label htmlFor="rd-doc">{t("redaction.docLabel")}</label>
            <select id="rd-doc" className="input" value={versionId} disabled={busy}
              onChange={(e) => {
                // §13.5: maskeleme planı özgün sürüme bağlıdır; farklı sürüme
                // kendiliğinden uygulanmaz. Seçim değişince eski bölgeler/
                // önizleme/sonuç taşınmaz; uçuştaki sayfa isteği devredilir.
                resetTargetState();
                setVersionId(e.target.value);
              }}>
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
            placeholder={t("redaction.termsPlaceholder")}
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
              <button className="btn" disabled={busy || pageBusy || !versionId} onClick={() => void loadPage()}>
                {pageBusy ? t("redaction.loadingPage") : t("redaction.loadPage")}
              </button>
              <button
                className="btn ghost"
                disabled={busy || regions.length === 0}
                onClick={() => setRegions([])}
              >
                {t("redaction.clearRegions", { count: regions.length })}
              </button>
            </div>
            {preview ? (
              <>
                <div
                  ref={imageRef}
                  style={{ position: "relative", display: "inline-block", marginTop: 8, touchAction: "none", cursor: "crosshair" }}
                  onPointerDown={startDrag}
                  onPointerMove={moveDrag}
                  onPointerUp={endDrag}
                >
                  <img src={preview.src} alt={t("redaction.previewAlt", { page: preview.pageIndex + 1 })} style={{ display: "block", maxWidth: "100%" }} />
                  {regions
                    .filter((region) => region.pageIndex === preview.pageIndex)
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
                <p className="hint" style={{ marginTop: 6, marginBottom: 0 }}>
                  <span>{t("redaction.previewCaption", { page: preview.pageIndex + 1 })}</span>
                  {preview.pageIndex !== page ? (
                    <span>{" "}{t("redaction.previewMismatch", { selected: page + 1 })}</span>
                  ) : null}
                </p>
              </>
            ) : (
              <p className="hint" style={{ marginTop: 8 }}>
                {t("redaction.previewHint")}
              </p>
            )}
          </div>
        )}
        <div className="row" style={{ justifyContent: "flex-end" }}>
          {mode === "terms" ? (
            <button className="btn primary" disabled={busy || pageBusy || !versionId} onClick={() => void run()}>
              {busy ? t("redaction.masking") : t("redaction.runTerms")}
            </button>
          ) : (
            <button className="btn primary" disabled={busy || pageBusy || !versionId || regions.length === 0} onClick={() => void applyRegions()}>
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

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("redaction.plansTitle")}</h2>
            <p className="card-sub">{t("redaction.plansSub")}</p>
          </div>
          <button
            className="btn ghost"
            disabled={!caseId || plansBusy}
            onClick={() => {
              if (caseId) void loadPlans(caseId);
            }}
          >
            {t("redaction.plansRefresh")}
          </button>
        </div>
        {plansError ? (
          // Okunamayan sicil "kayıt yok" diye gösterilmez: avukat maskelemenin
          // izlenmediğini sanardı. Hata kendi satırında yazılır.
          <div className="notice danger" role="alert">
            {t("redaction.plansUnreadable", {
              code: plansError.code,
              message: plansError.message,
            })}
          </div>
        ) : plans.length === 0 ? (
          <p className="hint" style={{ margin: 0 }}>
            {plansBusy ? t("redaction.plansLoading") : t("redaction.plansEmpty")}
          </p>
        ) : (
          <>
            <p className="hint" style={{ margin: "0 0 6px" }}>
              {t("redaction.plansCount", { count: plans.length })}
            </p>
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              {plans.map((plan) => {
                const state = planState(plan.state);
                const open = openPlanId === plan.id;
                const pages = open ? pageBreakdown(planRegions) : [];
                const unreadable = open
                  ? planRegions.filter((row) => row.rects === null).length
                  : 0;
                return (
                  <div
                    key={plan.id}
                    style={{
                      border: "1px solid var(--line)",
                      borderRadius: "var(--radius)",
                      padding: "8px var(--space-3)",
                    }}
                  >
                    <div
                      className="row"
                      style={{
                        justifyContent: "space-between",
                        alignItems: "center",
                        gap: "var(--space-2)",
                      }}
                    >
                      <div style={{ flex: "1 1 auto", minWidth: 0 }}>
                        <div className="small muted">
                          {plan.documentTitle ??
                            t("redaction.planMissingDocument", {
                              id: plan.documentVersionId,
                            })}
                          {" · "}
                          {plan.versionNo === null
                            ? t("redaction.planVersionUnknown")
                            : t("redaction.planVersionNo", { version: plan.versionNo })}
                          {" · "}
                          {formatDateTime(plan.createdAt)}
                        </div>
                        <div
                          style={{
                            fontFamily: "var(--mono)",
                            fontSize: 12,
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                          }}
                          title={plan.renderHash}
                        >
                          {t("redaction.planRenderHash", {
                            hash: plan.renderHash.slice(0, 16),
                          })}
                        </div>
                      </div>
                      <div className="row" style={{ gap: 6, alignItems: "center" }}>
                        {result && result.plan.id === plan.id ? (
                          <span className="badge ok">{t("redaction.planThisRun")}</span>
                        ) : null}
                        <span className={`badge ${state.tone}`}>{state.label}</span>
                      </div>
                    </div>
                    <div
                      className="row"
                      style={{
                        justifyContent: "space-between",
                        alignItems: "center",
                        marginTop: 6,
                      }}
                    >
                      <span className="small muted">
                        {t("redaction.planRegions", { count: plan.regionCount })}
                      </span>
                      <button
                        className="btn ghost"
                        aria-expanded={open}
                        onClick={() => togglePlan(plan.id)}
                      >
                        {open
                          ? t("redaction.planRegionsHide")
                          : t("redaction.planRegionsShow")}
                      </button>
                    </div>
                    {open ? (
                      planRegionsError ? (
                        <div className="notice danger" role="alert">
                          {t("redaction.planRegionsUnreadable", {
                            code: planRegionsError.code,
                            message: planRegionsError.message,
                          })}
                        </div>
                      ) : planRegions.length === 0 ? (
                        <p className="hint" style={{ margin: "6px 0 0" }}>
                          {planRegionsBusy
                            ? t("redaction.planRegionsLoading")
                            : t("redaction.planRegionsEmpty")}
                        </p>
                      ) : (
                        <>
                          <ul className="small" style={{ margin: "6px 0 0", paddingLeft: 16 }}>
                            {pages.map((entry) => (
                              <li key={entry.page}>
                                {t("redaction.planPageLine", {
                                  page: entry.page + 1,
                                  count: entry.count,
                                })}
                              </li>
                            ))}
                          </ul>
                          {unreadable > 0 ? (
                            <p
                              className="hint"
                              style={{ margin: "4px 0 0", color: "var(--danger)" }}
                            >
                              {t("redaction.planGeometryUnreadable", { count: unreadable })}
                            </p>
                          ) : null}
                        </>
                      )
                    ) : null}
                  </div>
                );
              })}
            </div>
          </>
        )}
        <p className="hint" style={{ margin: "8px 0 0" }}>
          {t("redaction.plansNote")}
        </p>
      </div>
    </div>
  );
}
