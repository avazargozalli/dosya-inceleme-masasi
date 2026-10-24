import { useEffect, useState } from "react";
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

const FORMATS: Array<{
  id: "json" | "csv" | "docx" | "pdf";
  title: string;
  description: string;
  ext: string;
}> = [
  { id: "json", title: "JSON", description: "Tam metadata: dosya, belgeler, olaylar, iddialar, notlar, görevler, anchor'lar", ext: "json" },
  { id: "csv", title: "CSV", description: "Kronoloji ve görevler; tablo programlarında açılır", ext: "csv" },
  { id: "docx", title: "DOCX", description: "Duruşma/devir raporu; düzenlenebilir belge", ext: "docx" },
  { id: "pdf", title: "PDF", description: "Aynı raporun salt-okunur çıktısı", ext: "pdf" },
];

export function Exports() {
  const [cases, setCases] = useState<CaseRow[]>([]);
  const [caseId, setCaseId] = useState("");
  const [result, setResult] = useState<ExportResult | null>(null);
  const [pkg, setPkg] = useState<PackageResult | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [packagePassword, setPackagePassword] = useState("");
  const [coverage, setCoverage] = useState<ExportCoverage | null>(null);

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

  // Kapsam kasadan okunur; arayüz kendi sayısını üretmez.
  useEffect(() => {
    if (!caseId) {
      setCoverage(null);
      return;
    }
    let alive = true;
    (async () => {
      try {
        const next = await api.caseCoverage(caseId);
        if (alive) setCoverage(next);
      } catch (e) {
        if (alive) setError(toApiError(e));
      }
    })();
    return () => {
      alive = false;
    };
  }, [caseId]);

  async function run(format: "json" | "csv" | "docx" | "pdf") {
    const selected = cases.find((c) => c.id === caseId);
    if (!selected) return;
    const target = await saveFile({
      title: "Çıktıyı kaydet",
      defaultPath: `${selected.title.replace(/[^\w\- ığüşöçİĞÜŞÖÇ]+/g, "_")}.${format}`,
      filters: [{ name: format.toUpperCase(), extensions: [format] }],
    });
    if (typeof target !== "string") return;
    setBusy(format);
    setError(null);
    setResult(null);
    try {
      const fn =
        format === "json" ? api.exportJson : format === "csv" ? api.exportCsv : format === "docx" ? api.exportDocx : api.exportPdf;
      const exported = await fn(caseId, target);
      setResult(exported);
      // Gösterilen kapsam, dosyaya gerçekten yazılan bloktur (yanıttan gelir).
      setCoverage(exported.coverage);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(null);
    }
  }

  async function runPackage() {
    if (!caseId) return;
    const dir = await openDirectory({ directory: true, multiple: false, title: "Devir paketi klasörünü seç" });
    if (typeof dir !== "string") return;
    setBusy("package");
    setError(null);
    setPkg(null);
    try {
      const created = await api.exportPackage(
        caseId,
        dir,
        packagePassword.trim().length >= 10 ? packagePassword : undefined,
      );
      setPkg(created);
      setCoverage(created.coverage);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(null);
    }
  }

  async function refreshCoverage() {
    if (!caseId) return;
    try {
      setCoverage(await api.caseCoverage(caseId));
    } catch (e) {
      setError(toApiError(e));
    }
  }

  const coverageInfo = coverage ? coverageState(coverage) : null;
  const emptyWarning = coverage ? emptyCoverageWarning(coverage) : null;

  return (
    <div className="page" style={{ maxWidth: 900 }}>
      <div className="page-head">
        <h1>Çıktılar</h1>
        <p className="lead">
          Her dışa aktarma kasa içinde kayda alınır: tür, hedef ve çıktı hash'i. Belge baytları
          yalnız açıkça seçilen formatta ve yalnız kullanıcı seçtiği yola yazılır.
        </p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="card">
        <div className="field" style={{ maxWidth: 420 }}>
          <label htmlFor="ex-case">Dosya</label>
          <select id="ex-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
            {cases.length === 0 ? <option value="">Dosya yok</option> : null}
            {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
          </select>
        </div>

        <div className="card-head" style={{ marginTop: "var(--space-4)" }}>
          <div>
            <h2>İncelenen kapsam</h2>
            <p className="card-sub">
              Sayılar kasadan okunur ve her çıktının içine gömülür. Kapsam dışı belge/sayfa
              “temiz” sayılmaz.
            </p>
          </div>
          <button className="btn ghost" disabled={!caseId || busy !== null} onClick={() => void refreshCoverage()}>
            Kapsamı tazele
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
                <strong>Kapsam uyarıları</strong>
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
          <p className="hint" style={{ margin: 0 }}>Kapsam okunamadı; dışa aktarma yine kapsamı yazar.</p>
        )}

        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-3)" }}>
          {FORMATS.map((format) => (
            <div key={format.id} className="card tight" style={{ background: "var(--surface)" }}>
              <div className="card-head">
                <div>
                  <h2>{format.title}</h2>
                  <p className="card-sub">{format.description}</p>
                </div>
                <button className="btn" disabled={!caseId || busy !== null} onClick={() => void run(format.id)}>
                  {busy === format.id ? "Yazılıyor…" : "Kaydet"}
                </button>
              </div>
            </div>
          ))}
        </div>

        {result ? (
          <div className="notice ok">
            <strong>{result.record.exportType.toUpperCase()}</strong> yazıldı: {result.path}
            <br />
            {result.bytes} bayt · sha256 {result.sha256.slice(0, 16)}…
            <br />
            {coverageEmbedNote(result.record.exportType, result.coverage)}
          </div>
        ) : null}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Devir paketi</h2>
            <p className="card-sub">case.json + KAPSAM.md + manifest + README; belge baytları otomatik eklenmez</p>
          </div>
          <button className="btn" disabled={!caseId || busy !== null} onClick={() => void runPackage()}>
            {busy === "package" ? "Oluşturuluyor…" : "Paket oluştur"}
          </button>
        </div>
        <div className="field" style={{ maxWidth: 360 }}>
          <label htmlFor="pkg-password">Paket parolası (en az 10 karakter, isteğe bağlı)</label>
          <input
            id="pkg-password"
            className="input"
            type="password"
            autoComplete="new-password"
            value={packagePassword}
            onChange={(e) => setPackagePassword(e.target.value)}
            placeholder="Boş bırakılırsa klasör açık metin kalır"
          />
        </div>
        <p className="hint" style={{ margin: 0 }}>
          Parola girilirse klasör Argon2id + AES-256-GCM ile şifrelenip tek `.dimpak`
          dosyasına dönüşür. Paylaşılacak belgeler için önce Maskeleme ekranından maskeli
          kopya üretin.
        </p>
        {pkg ? (
          <div className="notice ok">
            Paket hazır: {pkg.path}
            <br />
            {pkg.files} dosya · manifest {pkg.sha256.slice(0, 16)}…
            <br />
            {coverageEmbedNote("package", pkg.coverage)}
            {pkg.sealedPath ? (
              <>
                <br />
                Şifreli paket: <code>{pkg.sealedPath}</code>
              </>
            ) : null}
          </div>
        ) : null}
      </div>
    </div>
  );
}
