import { useCallback, useEffect, useRef, useState } from "react";
import { open as openDirectory, save as saveFile } from "@tauri-apps/plugin-dialog";
import {
  api,
  toApiError,
  type ApiError,
  type CaseRow,
  type ExportCoverage,
  type ExportRecordRow,
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
import { formatDateTime } from "../lib/format";
import { t } from "../lib/i18n";

/**
 * Kapsam şeridinde gösterilen sayıların **kaynağı**. Aynı panel iki farklı şeyi
 * gösterebiliyor: kasadan şimdi okunan değer ya da bir çıktıya **yazılan** blok.
 * Etiket olmadan avukat "şu anki kapsam" sanır; ikisi ayrı zamanlardan gelir.
 */
type CoverageSource = "vault" | "export" | "package";

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

/** Denetim defteri tür etiketi: çekirdeğin yazdığı `type` değerinden türetilir. */
function recordTypeLabel(type: string): string {
  return type === "package" ? t("exports.ledgerTypePackage") : type.toUpperCase();
}

/**
 * Kayıt durumu. `ready` **yazıldı**, `failed` **başarısız**; tanınmayan bir
 * durum gizlenmez — ham değeriyle yazılır (uydurulmuş Türkçe etiket yok).
 */
function recordStatus(status: string): { label: string; tone: "ok" | "danger" | "warn" | "plain" } {
  switch (status) {
    case "ready":
      return { label: t("exports.ledgerStatusReady"), tone: "ok" };
    case "failed":
      return { label: t("exports.ledgerStatusFailed"), tone: "danger" };
    case "draft":
      return { label: t("exports.ledgerStatusDraft"), tone: "plain" };
    case "building":
      return { label: t("exports.ledgerStatusBuilding"), tone: "plain" };
    case "verifying":
      return { label: t("exports.ledgerStatusVerifying"), tone: "plain" };
    case "blocked":
      return { label: t("exports.ledgerStatusBlocked"), tone: "warn" };
    default:
      return { label: t("exports.ledgerStatusUnknown", { status }), tone: "warn" };
  }
}

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
  // Panelde gösterilen kapsamın kaynağı: kasadan mı okundu, bir çıktıya mı
  // yazıldı? `generatedAt` ile birlikte yazılır; aksi hâlde yazma anındaki blok
  // “şu anki kapsam” sanılır.
  const [coverageSource, setCoverageSource] = useState<CoverageSource>("vault");
  // Dışa aktarma kayıtları (denetim defteri): her yazma zaten kasaya kaydediliyor
  // ama **okuma yolu yoktu** — kayıt yalnız SQLite dosyasındaydı. Liste dosyaya
  // kapsanır; okuma hatası kendi satırında görünür (sessiz yutulmaz).
  const [records, setRecords] = useState<ExportRecordRow[]>([]);
  const [recordsError, setRecordsError] = useState<ApiError | null>(null);
  const [recordsLoading, setRecordsLoading] = useState(false);
  // İki ayrı kuşak: kapsam **okumaları** (dosya değişimi + tazeleme) ile **yazmalar**
  // (dışa aktarma + paket). Tek sayaç olsaydı ilgisiz bir yazma, kullanıcının
  // istediği tazelemenin yanıtını düşürür; düşen okumanın `finally`si kapıdan
  // geçmediği için `coverageLoading` kilidi asılı kalırdı.
  const writeSeq = useRef(0);
  const coverSeq = useRef(0);
  /** Defter okumalarının kuşağı: geç yanıt yeni dosyanın listesine yazmaz. */
  const recordsSeq = useRef(0);
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

  const loadRecords = useCallback(async (id: string) => {
    if (!id) {
      // Dosya seçimi kalktıysa askıdaki okuma da geçersizdir.
      recordsSeq.current += 1;
      return;
    }
    const seq = ++recordsSeq.current;
    setRecordsLoading(true);
    try {
      const rows = await api.exportList(id);
      if (seq !== recordsSeq.current) return;
      setRecords(rows);
      setRecordsError(null);
    } catch (e) {
      if (seq !== recordsSeq.current) return;
      // Sessiz yutma yok: defter okunamadıysa yazılır; boş liste gösterilmez.
      setRecordsError(toApiError(e));
    } finally {
      if (seq === recordsSeq.current) setRecordsLoading(false);
    }
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
    // Denetim defteri de dosyaya kapsanır: önceki dosyanın kayıtları **istekten
    // önce** temizlenir, yeni dosyanın kayıtları hemen istenir.
    setRecords([]);
    setRecordsError(null);
    void loadRecords(caseId);
    if (!caseId) {
      setCoverageLoading(false);
      setRecordsLoading(false);
      return;
    }
    let alive = true;
    setCoverageLoading(true);
    (async () => {
      try {
        const next = await api.caseCoverage(caseId);
        if (alive && seq === coverSeq.current) {
          setCoverage(next);
          setCoverageSource("vault");
        }
      } catch (e) {
        if (alive && seq === coverSeq.current) setError(toApiError(e));
      } finally {
        if (alive && seq === coverSeq.current) setCoverageLoading(false);
      }
    })();
    return () => {
      alive = false;
    };
  }, [caseId, loadRecords]);

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
        setCoverageSource("export");
        // Yazma kapsam şeridini devralır: uçuştaki okuma eski değeri ezemez,
        // okumanın kilidi de (kapıdan geçemeyeceği için) burada kapanır.
        coverSeq.current += 1;
        setCoverageLoading(false);
        // Yeni kayıt defterde görünmeli: "yazıldı" satırı kalıcı kanıt değildir,
        // ekran kapanınca kaybolur.
        void loadRecords(caseId);
      }
    } catch (e) {
      if (seq === writeSeq.current) {
        setError(toApiError(e));
        // Düşen yazma da defteri tazeler: paket kaydı **önce** yazılıp
        // `failed` işaretlendiği için başarısız deneme yalnız burada görünür.
        void loadRecords(caseId);
      }
    } finally {
      // Yalnız **kendi** isteğinde kilidi bırakır: dosya değiştiyse kilit zaten
      // devredilmiştir; yeni dosyada başlatılan yazmanın kilidi erken açılmaz.
      if (seq === writeSeq.current) setBusy(null);
    }
  }

  async function runPackage() {
    if (!caseId) return;
    // Parola ölçüsü **çekirdekle aynı** olmalı: Rust `chars().count()` sayar,
    // JS `.length` UTF-16 birimi sayar (5 emoji = 10 birim ama 5 karakter) ve
    // çekirdek ham dizeyi kullanır (kırpma yok). Eski kapı `trim().length`
    // kullanıyordu: `" 12345678 "` 10 birim görünüp 8 sayılıyor, parola
    // sessizce düşüyor ve **açık metin** klasör yazılıyordu; 5 emoji ise
    // gönderilip çekirdekte sessizce düşüyordu.
    const typed = packagePassword;
    const secretLength = [...typed].length;
    if (secretLength > 0 && secretLength < 10) {
      // Sessiz düşüş yok: parola yazıldıysa paket **hiç** yazılmaz. Açık metin
      // klasör üretmek “şifreli” sanılan bir çıktı bırakırdı.
      setError({
        code: "E_PACKAGE_PASSWORD",
        message: t("exports.packagePasswordTooShort", { count: secretLength }),
        retryable: true,
      });
      return;
    }
    // Gönderilen parola donar: yanıt geldiğinde alanda **başka** bir parola
    // yazılıysa silinmez (uçuşta yazılan kaybolmaz).
    const submitted = secretLength >= 10 ? typed : undefined;
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
      const created = await api.exportPackage(caseId, dir, submitted);
      if (seq === writeSeq.current) {
        setPkg(created);
        setCoverage(created.coverage);
        setCoverageSource("package");
        coverSeq.current += 1;
        setCoverageLoading(false);
        // Parola işi bitti: düz metin bellekte/DOM'da kalmaz — ama uçuşta
        // yazılan yeni parola bu temizliğin kurbanı olmaz.
        setPackagePassword((prev) => (prev === submitted ? "" : prev));
        // Çekirdek parolayı aldı ama şifrelemediyse bu **sessiz** kalmaz:
        // yeşil “paket hazır” satırı tek başına şifreli sanılırdı.
        if (submitted !== undefined && !created.sealedPath) {
          setError({
            code: "E_PACKAGE_NOT_SEALED",
            message: t("exports.packageNotSealed"),
            retryable: true,
          });
        }
        void loadRecords(caseId);
      }
    } catch (e) {
      if (seq === writeSeq.current) {
        setError(toApiError(e));
        // Düşen pakette kayıt `failed` olarak kasada durur; tazeleme olmadan
        // avukat bunu hiçbir yerde göremezdi.
        void loadRecords(caseId);
      }
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
      if (seq === coverSeq.current) {
        setCoverage(next);
        setCoverageSource("vault");
      }
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
            <p className="hint" style={{ margin: "0 0 var(--space-2)" }}>
              {t(
                coverageSource === "vault"
                  ? "exports.coverageFromVault"
                  : coverageSource === "export"
                    ? "exports.coverageFromExport"
                    : "exports.coverageFromPackage",
                { at: formatDateTime(coverage.generatedAt) },
              )}
            </p>
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
            {t("exports.verifiedReadback")}
            {result.replacedExisting ? (
              <>
                <br />
                {t("exports.replacedExisting")}
              </>
            ) : null}
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
            {/* Şifreliyse birincil konum `.dimpak` dosyasıdır: çekirdek geçici
                klasörü paketledikten sonra siler, `pkg.path` **null** döner.
                Klasör silinemezse çekirdek yolu yine bildirir — o zaman açık
                metin diskte kaldığı için ayrı uyarı yazılır. */}
            {pkg.sealedPath ? (
              <>
                {t("exports.sealed")} <code>{pkg.sealedPath}</code>
              </>
            ) : pkg.path ? (
              t("exports.packageReady", { path: pkg.path })
            ) : (
              t("exports.packagePathMissing")
            )}
            <br />
            {t("exports.packageFiles", { files: pkg.files, hash: pkg.sha256.slice(0, 16) })}
            <br />
            {coverageEmbedNote("package", pkg.coverage)}
            <br />
            {pkg.sealedPath
              ? pkg.path
                ? t("exports.sealedLeftover", { path: pkg.path })
                : t("exports.sealedConverted")
              : pkg.path
                ? t("exports.packagePlaintext")
                : null}
          </div>
        ) : null}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("exports.ledgerTitle")}</h2>
            <p className="card-sub">{t("exports.ledgerSub")}</p>
          </div>
          <button
            className="btn ghost"
            disabled={!caseId || recordsLoading}
            onClick={() => {
              if (caseId) void loadRecords(caseId);
            }}
          >
            {t("exports.ledgerRefresh")}
          </button>
        </div>
        {recordsError ? (
          // Okunamayan defter "kayıt yok" diye gösterilmez: avukat çıktının
          // izlenmediğini sanardı. Hata kendi satırında yazılır.
          <div className="notice danger" role="alert">
            {t("exports.ledgerUnreadable", {
              code: recordsError.code,
              message: recordsError.message,
            })}
          </div>
        ) : records.length === 0 ? (
          <p className="hint" style={{ margin: 0 }}>
            {recordsLoading ? t("exports.ledgerLoading") : t("exports.ledgerEmpty")}
          </p>
        ) : (
          <>
            <p className="hint" style={{ margin: "0 0 6px" }}>
              {t("exports.ledgerCount", { count: records.length })}
            </p>
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              {records.map((record) => {
                const status = recordStatus(record.status);
                return (
                  <div
                    key={record.id}
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
                          {recordTypeLabel(record.exportType)} · {formatDateTime(record.createdAt)}
                          {record.outputHash ? (
                            <>
                              {" · "}
                              <span style={{ fontFamily: "var(--mono)" }}>
                                {record.outputHash.slice(0, 16)}…
                              </span>
                            </>
                          ) : null}
                        </div>
                        <div
                          style={{
                            fontFamily: "var(--mono)",
                            fontSize: 12,
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                          }}
                          title={record.destinationPath ?? undefined}
                        >
                          {record.destinationPath ?? t("exports.ledgerNoPath")}
                        </div>
                      </div>
                      <span className={`badge ${status.tone}`}>{status.label}</span>
                    </div>
                  </div>
                );
              })}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
