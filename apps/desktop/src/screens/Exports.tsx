import { useEffect, useRef, useState } from "react";
import { open as openDirectory, save as saveFile } from "@tauri-apps/plugin-dialog";
import {
  api,
  toApiError,
  type ApiError,
  type CaseRow,
  type ExportCoverage,
  type ExportResult,
  type PackageResult,
} from "../lib/api";
import {
  coverageEmbedNote,
  coverageHeadline,
  coverageRows,
  coverageState,
  emptyCoverageWarning,
} from "../lib/export-coverage";
import { fileStem } from "../lib/filename";
import { t } from "../lib/i18n";

const FORMATS: Array<{
  id: "json" | "csv" | "docx" | "pdf";
  title: string;
  description: string;
  ext: string;
}> = [
  { id: "json", title: "JSON", description: t("exports.formatJson"), ext: "json" },
  { id: "csv", title: "CSV", description: t("exports.formatCsv"), ext: "csv" },
  { id: "docx", title: "DOCX", description: t("exports.formatDocx"), ext: "docx" },
  { id: "pdf", title: "PDF", description: t("exports.formatPdf"), ext: "pdf" },
];

export function Exports() {
  const [cases, setCases] = useState<CaseRow[]>([]);
  const [caseId, setCaseId] = useState("");
  const [result, setResult] = useState<ExportResult | null>(null);
  const [pkg, setPkg] = useState<PackageResult | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  // Seçici diyaloğu açıkken ikinci tıklama ikinci pencere açmasın: kilit yazma
  // başlamadan devreye girer, düğme metni değişmez (henüz yazılmıyor).
  const [pickBusy, setPickBusy] = useState(false);
  const [packagePassword, setPackagePassword] = useState("");
  const [coverage, setCoverage] = useState<ExportCoverage | null>(null);
  const [coverageLoading, setCoverageLoading] = useState(false);
  // İki ayrı kuşak: kapsam **okumaları** (dosya değişimi + tazeleme) ile **yazmalar**
  // (dışa aktarma + paket). Tek sayaç olsaydı ilgisiz bir yazma, kullanıcının
  // istediği tazelemenin yanıtını düşürür; düşen okumanın `finally`si kapıdan
  // geçmediği için `coverageLoading` kilidi asılı kalırdı.
  const writeSeq = useRef(0);
  const coverSeq = useRef(0);
  // Diyalog uçuşunda dosya kimliği: seçim yanıtı geldiğinde başka dosyaya
  // geçilmişse yazma **başlatılmaz** — yoksa eski dosyanın çıktısı yeni dosyanın
  // altına “yazıldı” diye düşerdi (INV-05).
  const caseRef = useRef("");

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

  // Kapsam kasadan okunur; arayüz kendi sayısını üretmez. Dosya değişince
  // önceki dosyanın kapsamı **ve** çıktı/paket satırları ekranda kalmaz:
  // yoksa yeni dosyanın isteği askıdayken başka dosyanın çıktısı güncel
  // sanılır (INV-05).
  useEffect(() => {
    caseRef.current = caseId;
    const seq = ++coverSeq.current;
    // Kilit devri: yeni dosyanın altı düğmesi eski dosyanın isteğini beklemez
    // (yanıt hiç gelmezse süresiz kilitli kalırdı). Devredilen yazmanın
    // kapanışı artık `seq` kapısından geçmediği için kilidi geri açamaz.
    writeSeq.current += 1;
    setResult(null);
    setPkg(null);
    setCoverage(null);
    setError(null);
    setBusy(null);
    if (!caseId) {
      setCoverageLoading(false);
      return;
    }
    let alive = true;
    setCoverageLoading(true);
    (async () => {
      try {
        const next = await api.caseCoverage(caseId);
        if (alive && seq === coverSeq.current) setCoverage(next);
      } catch (e) {
        if (alive && seq === coverSeq.current) setError(toApiError(e));
      } finally {
        if (alive && seq === coverSeq.current) setCoverageLoading(false);
      }
    })();
    return () => {
      alive = false;
    };
  }, [caseId]);

  async function run(format: "json" | "csv" | "docx" | "pdf") {
    const selected = cases.find((c) => c.id === caseId);
    if (!selected) return;
    // Diyalog uçuşu: ikinci tıklama ikinci pencere açmaz (hedefi ikincisi ezerdi).
    const asked = caseId;
    setPickBusy(true);
    let target: string | null = null;
    try {
      target = await saveFile({
        title: t("exports.saveDialog"),
        defaultPath: `${fileStem(selected.title)}.${format}`,
        filters: [{ name: format.toUpperCase(), extensions: [format] }],
      });
    } catch {
      // Seçici reddi sessiz kalmaz: düğmeye basıp hiçbir şey olmaması yalan olurdu.
      setError({ code: "E_DIALOG", message: t("exports.savePickFailed"), retryable: true });
    } finally {
      setPickBusy(false);
    }
    if (typeof target !== "string") return;
    // Dosya, diyalog açıkken değiştiyse yazma **başlatılmaz**: hedef artık
    // ekranda değil ve yanıt yeni dosyanın “yazıldı” satırı olarak görünürdü.
    if (caseRef.current !== asked) return;
    const seq = ++writeSeq.current;
    setBusy(format);
    setError(null);
    setResult(null);
    try {
      const fn =
        format === "json" ? api.exportJson : format === "csv" ? api.exportCsv : format === "docx" ? api.exportDocx : api.exportPdf;
      const exported = await fn(caseId, target);
      if (seq === writeSeq.current) {
        setResult(exported);
        // Gösterilen kapsam, dosyaya gerçekten yazılan bloktur (yanıttan gelir).
        setCoverage(exported.coverage);
        // Yazma kapsam şeridini devralır: uçuştaki okuma eski değeri ezemez,
        // okumanın kilidi de (kapıdan geçemeyeceği için) burada kapanır.
        coverSeq.current += 1;
        setCoverageLoading(false);
      }
    } catch (e) {
      if (seq === writeSeq.current) setError(toApiError(e));
    } finally {
      // Yalnız **kendi** isteğinde kilidi bırakır: dosya değiştiyse kilit zaten
      // devredilmiştir; yeni dosyada başlatılan yazmanın kilidi erken açılmaz.
      if (seq === writeSeq.current) setBusy(null);
    }
  }

  async function runPackage() {
    if (!caseId) return;
    // Gönderilen parola donar: yanıt geldiğinde alanda **başka** bir parola
    // yazılıysa silinmez (uçuşta yazılan kaybolmaz).
    const submitted = packagePassword;
    const asked = caseId;
    setPickBusy(true);
    let dir: string | null = null;
    try {
      dir = await openDirectory({ directory: true, multiple: false, title: t("exports.dirDialog") });
    } catch {
      setError({ code: "E_DIALOG", message: t("exports.dirPickFailed"), retryable: true });
    } finally {
      setPickBusy(false);
    }
    if (typeof dir !== "string") return;
    if (caseRef.current !== asked) return;
    const seq = ++writeSeq.current;
    setBusy("package");
    setError(null);
    setPkg(null);
    try {
      const created = await api.exportPackage(
        caseId,
        dir,
        submitted.trim().length >= 10 ? submitted : undefined,
      );
      if (seq === writeSeq.current) {
        setPkg(created);
        setCoverage(created.coverage);
        coverSeq.current += 1;
        setCoverageLoading(false);
        // Parola işi bitti: düz metin bellekte/DOM'da kalmaz — ama uçuşta
        // yazılan yeni parola bu temizliğin kurbanı olmaz.
        setPackagePassword((prev) => (prev === submitted ? "" : prev));
      }
    } catch (e) {
      if (seq === writeSeq.current) setError(toApiError(e));
    } finally {
      if (seq === writeSeq.current) setBusy(null);
    }
  }

  async function refreshCoverage() {
    // Okuma tek uçuşu düğmenin `disabled` kapısı tutar (React kapalı düğmede
    // `onClick` üretmez); burada ikinci bir bayrak tutmak yedekli olurdu.
    if (!caseId) return;
    const seq = ++coverSeq.current;
    setError(null);
    setCoverageLoading(true);
    try {
      const next = await api.caseCoverage(caseId);
      if (seq === coverSeq.current) setCoverage(next);
    } catch (e) {
      if (seq === coverSeq.current) setError(toApiError(e));
    } finally {
      if (seq === coverSeq.current) setCoverageLoading(false);
    }
  }


  const coverageInfo = coverage ? coverageState(coverage) : null;
  const emptyWarning = coverage ? emptyCoverageWarning(coverage) : null;

  return (
    <div className="page" style={{ maxWidth: 900 }}>
      <div className="page-head">
        <h1>{t("nav.exports")}</h1>
        <p className="lead">{t("exports.lead")}</p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="card">
        <div className="field" style={{ maxWidth: 420 }}>
          <label htmlFor="ex-case">{t("common.case")}</label>
          <select id="ex-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
            {cases.length === 0 ? <option value="">{t("common.noCase")}</option> : null}
            {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
          </select>
        </div>

        <div className="card-head" style={{ marginTop: "var(--space-4)" }}>
          <div>
            <h2>{t("exports.coverageTitle")}</h2>
            <p className="card-sub">{t("exports.coverageSub")}</p>
          </div>
          <button
            className="btn ghost"
            disabled={!caseId || busy !== null || pickBusy || coverageLoading}
            onClick={() => void refreshCoverage()}
          >
            {t("exports.refresh")}
          </button>
        </div>
        {coverage && coverageInfo ? (
          <div>
            <div className="row" style={{ alignItems: "center", gap: "var(--space-2)" }}>
              <span className={`badge ${coverageInfo.tone === "ok" ? "ok" : "warn"}`}>{coverageInfo.label}</span>
              <span className="muted">{coverageHeadline(coverage)}</span>
            </div>
            <p className="hint" style={{ margin: "6px 0 var(--space-3)" }}>{coverageInfo.detail}</p>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(5, minmax(0, 1fr))", gap: "var(--space-2)" }}>
              {coverageRows(coverage).map((row) => (
                <div key={row.label} className="card tight" style={{ background: "var(--surface)" }}>
                  <div className="muted" style={{ fontSize: 11 }}>{row.label}</div>
                  <div
                    style={{
                      fontSize: 18,
                      fontWeight: 600,
                      color: row.tone === "warn" ? "var(--warn)" : undefined,
                    }}
                  >
                    {row.value}
                  </div>
                </div>
              ))}
            </div>
            {coverage.warnings.length > 0 ? (
              <div className="notice" style={{ marginTop: "var(--space-3)" }}>
                <strong>{t("exports.coverageWarnings")}</strong>
                <ul style={{ margin: "6px 0 0", paddingLeft: 18 }}>
                  {coverage.warnings.map((warning) => (
                    <li key={warning}>{warning}</li>
                  ))}
                </ul>
              </div>
            ) : null}
            {emptyWarning ? <p className="hint">{emptyWarning}</p> : null}
            <p className="hint" style={{ margin: "8px 0 0" }}>{coverage.statement}</p>
          </div>
        ) : (
          <p className="hint" style={{ margin: 0 }}>
            {coverageLoading ? t("exports.coverageLoading") : t("exports.coverageUnreadable")}
          </p>
        )}


        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-3)" }}>
          {FORMATS.map((format) => (
            <div key={format.id} className="card tight" style={{ background: "var(--surface)" }}>
              <div className="card-head">
                <div>
                  <h2>{format.title}</h2>
                  <p className="card-sub">{format.description}</p>
                </div>
                <button className="btn" disabled={!caseId || busy !== null || pickBusy} onClick={() => void run(format.id)}>
                  {busy === format.id ? t("exports.writing") : t("common.save")}
                </button>
              </div>
            </div>
          ))}
        </div>

        {result ? (
          <div className="notice ok">
            <strong>{result.record.exportType.toUpperCase()}</strong> {t("exports.written")} {result.path}
            <br />
            {result.bytes} {t("units.byte")} · sha256 {result.sha256.slice(0, 16)}…
            <br />
            {coverageEmbedNote(result.record.exportType, result.coverage)}
          </div>
        ) : null}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("exports.packageTitle")}</h2>
            <p className="card-sub">{t("exports.packageSub")}</p>
          </div>
          <button className="btn" disabled={!caseId || busy !== null || pickBusy} onClick={() => void runPackage()}>
            {busy === "package" ? t("exports.packageBusy") : t("exports.packageCreate")}
          </button>
        </div>
        <div className="field" style={{ maxWidth: 360 }}>
          <label htmlFor="pkg-password">{t("exports.packagePassword")}</label>
          <input
            id="pkg-password"
            className="input"
            type="password"
            autoComplete="new-password"
            value={packagePassword}
            onChange={(e) => setPackagePassword(e.target.value)}
            placeholder={t("exports.packagePasswordPlaceholder")}
          />
        </div>
        <p className="hint" style={{ margin: 0 }}>
          {t("exports.packageHint")}
        </p>
        {pkg ? (
          <div className="notice ok">
            {t("exports.packageReady", { path: pkg.path })}
            <br />
            {t("exports.packageFiles", { files: pkg.files, hash: pkg.sha256.slice(0, 16) })}
            <br />
            {coverageEmbedNote("package", pkg.coverage)}
            {pkg.sealedPath ? (
              <>
                <br />
                {t("exports.sealed")} <code>{pkg.sealedPath}</code>
              </>
            ) : null}
          </div>
        ) : null}
      </div>
    </div>
  );
}
