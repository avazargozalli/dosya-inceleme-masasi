import { Fragment, useEffect, useRef, useState, type CSSProperties } from "react";
import {
  api,
  toApiError,
  type AdapterView,
  type ApiError,
  type ApiKeyStatus,
  type CliArchReport,
  type CliAttackReport,
  type CliCapabilityMatrix,
  type CliHardeningProbe,
  type CliProbeView,
  type CliResidueCleanup,
  type CliResidueMeasure,
  type CliResidueScan,
  type CliSandboxSweep,
  type WorkerPackageView,
} from "../lib/api";
import {
  adapterArchBadge,
  archBadgeText,
  archFormatText,
  archInterpreterText,
  archList,
  archPathText,
  archProbedAt,
  archReasonText,
  archSummaryText,
  archTone,
  archWarnings,
  archWorst,
} from "../lib/arch";
import {
  attackAdapterWarnings,
  attackCommand,
  attackFixtureSummary,
  attackFixtureWarnings,
  attackGuardText,
  attackProductText,
  attackSkipText,
  attackTotalHits,
  attackVerdictText,
  attackVerdictTone,
  attackWorstVerdict,
} from "../lib/attack";
import { formatUnixSeconds } from "../lib/format";
import { t } from "../lib/i18n";
import {
  packageLimits,
  packageStatusText,
  packageSummary,
  packageTone,
  packageWarnings,
  problemText,
  roleRows,
  verificationCounts,
} from "../lib/package";
import {
  formatBytes,
  persistenceText,
  probeArgv,
  probeCanRun,
  probeOutputs,
  probeScopeLines,
  probeScopeWarnings,
  probeStatusLabel,
  probeSummary,
  probeWarnings,
  capabilityDetailText,
  capabilityGateText,
  capabilityHelpCommand,
  capabilityReasonLines,
  capabilitySummary,
  capabilityTone,
  capabilityWarnings,
  hardeningProbeText,
  linkTone,
  matrixWorstLink,
  promptShapeText,
  residueAge,
  residueCleanupSummary,
  residueCleanupTargets,
  residueMeasureSummary,
  residueMeasureWarnings,
  residuePathLine,
  residueScanWarnings,
  sandboxSweepSummary,
} from "../lib/providers";

/** Kimlik kipi → etiket. Sözlük çağrı anında okunur (modül sabiti metin taşımaz). */
function authLabel(mode: string): string {
  switch (mode) {
    case "subscription":
      return t("providers.authSubscription");
    case "api_key":
      return t("providers.authApiKey");
    case "interactive_only":
      return t("providers.authInteractiveOnly");
    default:
      return mode;
  }
}

const MONO: CSSProperties = { fontFamily: "var(--mono)", fontSize: 11, wordBreak: "break-all" };

const PRE: CSSProperties = {
  margin: "4px 0 0",
  padding: "var(--space-3)",
  background: "var(--surface)",
  border: "1px solid var(--line)",
  borderRadius: "var(--radius)",
  fontFamily: "var(--mono)",
  fontSize: 11,
  whiteSpace: "pre-wrap",
  wordBreak: "break-word",
  maxHeight: 200,
  overflow: "auto",
};

/**
 * Profil kaydının hata iletisi. Çekirdek reddi `{code, message}` nesnesidir
 * (Tauri `invoke`), bu yüzden `String(error)` ekrana "[object Object]" yazardı.
 * `Error` atan yollar iletisini korur; tanınmayan değer `toApiError`nun
 * "beklenmeyen hata" karşılığına düşer.
 */
function profileFailureText(error: unknown): string {
  const failure = toApiError(error);
  if (failure.code === "E_UI_UNKNOWN" && error instanceof Error && error.message.trim() !== "") {
    return error.message;
  }
  return `${failure.code} — ${failure.message}`;
}

// Anahtarın **gerçekten** okunduğu yeri yazar; "nereden okunur" tahmin
// edilmez. Kaynak okunamadıysa bunu da açıkça söyler.
function keySourceText(status: ApiKeyStatus | null): string {
  if (status === null) return t("providers.keySourceUnknown");
  if (status.target === "os_keychain") return t("providers.keySourceKeychain");
  if (status.target === "env_var") return t("providers.keySourceEnv");
  return t("providers.keySourceMissing");
}

export function Providers() {
  const [rows, setRows] = useState<AdapterView[]>([]);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  const [running, setRunning] = useState<string | null>(null);
  const [results, setResults] = useState<Record<string, CliProbeView>>({});
  const [runErrors, setRunErrors] = useState<Record<string, ApiError>>({});
  const [cancelHint, setCancelHint] = useState<string | null>(null);
  const [cancelBusy, setCancelBusy] = useState(false);
  const [matrix, setMatrix] = useState<CliCapabilityMatrix | null>(null);
  const [matrixBusy, setMatrixBusy] = useState(false);
  const [matrixError, setMatrixError] = useState<ApiError | null>(null);
  /** F10-T08: adaptör başına **ölçülen** sertleştirme davranışı. */
  const [hardening, setHardening] = useState<Record<string, CliHardeningProbe>>({});
  const [hardeningBusy, setHardeningBusy] = useState<string | null>(null);
  const [hardeningErrors, setHardeningErrors] = useState<Record<string, ApiError>>({});
  const [attack, setAttack] = useState<CliAttackReport | null>(null);
  const [attackBusy, setAttackBusy] = useState(false);
  const [attackError, setAttackError] = useState<ApiError | null>(null);
  const [arch, setArch] = useState<CliArchReport | null>(null);
  const [archBusy, setArchBusy] = useState(false);
  const [archError, setArchError] = useState<ApiError | null>(null);
  const [pkg, setPkg] = useState<WorkerPackageView | null>(null);
  const [pkgBusy, setPkgBusy] = useState(false);
  const [pkgError, setPkgError] = useState<ApiError | null>(null);
  /**
   * Çekirdeğin **süreç-geneli** CLI kilidi (`cli_probe_lock`, `commands.rs`)
   * tek uçuştur: satır yoklaması, yetenek matrisi, saldırı sınaması ve kalıntı
   * ölçümü aynı slotu paylaşır; ikinci istek `E_CLI_BUSY` alır. Kartın kendi
   * `busy` değeri yalnız **etiket** içindir — kapı bu tek bayraktır. Bayrak
   * adaptör değişiminde bırakılmaz: çekirdekteki işlem hâlâ sürüyordur.
   */
  const [coreBusy, setCoreBusy] = useState(false);

  async function refresh() {
    setBusy(true);
    // Başarılı tazeleme eski kırmızı bildirimi bırakmaz: hata yalnız bu isteğin
    // sonucudur.
    setError(null);
    try {
      setRows(await api.providerProbe());
      // Liste yeniden okundu: önceki yoklamalar **önceki** satırların kanıtıdır.
      // Sürüm/yol değişmiş olabilir; eski komut ve çıktı yeni satırın altında
      // "güncel" gibi durmamalı (matris/sınama/mimari raporları da yeni koşuda
      // temizlenir).
      setResults({});
      setRunErrors({});
      // Sürüm/yol değişmiş olabilir: ölçülen sertleştirme davranışı da düşer.
      setHardening({});
      setHardeningErrors({});
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function runProbe(adapterId: string) {
    // Çekirdek kilidi bu istekle tutulur; yanıt dönene kadar diğer CLI
    // komutları kapalıdır (ikisi de aynı `cli_probe_lock` slotunu ister).
    setCoreBusy(true);
    setRunning(adapterId);
    setCancelHint(null);
    // Önceki koşunun sonucu ve hatası bu satıra ait değildir: yeni koşu
    // başlarken ikisi de silinir, böylece başarısız koşu eski yeşil sonucu
    // (veya eski hatayı) ekranda bırakmaz.
    setResults((prev) => {
      const next = { ...prev };
      delete next[adapterId];
      return next;
    });
    setRunErrors((prev) => {
      const next = { ...prev };
      delete next[adapterId];
      return next;
    });
    try {
      const result = await api.providerProbeRun(adapterId);
      setResults((prev) => ({ ...prev, [adapterId]: result }));
    } catch (e) {
      const failure = toApiError(e);
      setRunErrors((prev) => ({ ...prev, [adapterId]: failure }));
    } finally {
      setCoreBusy(false);
      setRunning(null);
    }
  }

  async function cancelProbe() {
    // İptal sinyali çekirdekte **tek** uçuştur: ikinci tık, o sırada başlatılan
    // yeni bir koşuyu iptal edebilecek ikinci bir istek göndermemeli.
    setCancelBusy(true);
    // İleti tek bir denemenin sonucudur; yeni deneme eskisini bırakmaz.
    setCancelHint(null);
    try {
      const signalled = await api.providerProbeCancel();
      setCancelHint(
        signalled ? t("providers.cancelSignalled") : t("providers.cancelNothing"),
      );
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setCancelBusy(false);
    }
  }

  async function runMatrix() {
    setCoreBusy(true);
    setMatrixBusy(true);
    setMatrixError(null);
    // Bayat rapor: yeni koşu düşerse önceki matris "güncel" gibi okunmamalı.
    setMatrix(null);
    try {
      setMatrix(await api.cliCapabilityMatrix());
    } catch (e) {
      setMatrixError(toApiError(e));
    } finally {
      setCoreBusy(false);
      setMatrixBusy(false);
    }
  }

  /**
   * Sertleştirme davranış sınaması: **gerçek istem** gönderir (kota harcar).
   * Sonuç adaptör kimliğine bağlanır; düşen istek eski ölçümü bırakmaz.
   */
  async function runHardening(adapterId: string) {
    setCoreBusy(true);
    setHardeningBusy(adapterId);
    setHardeningErrors((prev) => {
      const next = { ...prev };
      delete next[adapterId];
      return next;
    });
    setHardening((prev) => {
      const next = { ...prev };
      delete next[adapterId];
      return next;
    });
    try {
      const probe = await api.cliHardeningProbe(adapterId);
      setHardening((prev) => ({ ...prev, [adapterId]: probe }));
    } catch (e) {
      setHardeningErrors((prev) => ({ ...prev, [adapterId]: toApiError(e) }));
    } finally {
      setCoreBusy(false);
      setHardeningBusy(null);
    }
  }

  async function runAttack() {
    setCoreBusy(true);
    setAttackBusy(true);
    setAttackError(null);
    // Bayat sınama raporu yeni hatayla birlikte görünmez.
    setAttack(null);
    try {
      setAttack(await api.cliAttackProbe());
    } catch (e) {
      setAttackError(toApiError(e));
    } finally {
      setCoreBusy(false);
      setAttackBusy(false);
    }
  }

  async function runArch() {
    setArchBusy(true);
    setArchError(null);
    // Bayat mimari raporu yeni hatayla birlikte görünmez.
    setArch(null);
    try {
      setArch(await api.cliArchReport());
    } catch (e) {
      setArchError(toApiError(e));
    } finally {
      setArchBusy(false);
    }
  }

  async function runPackage() {
    setPkgBusy(true);
    setPkgError(null);
    // Bayat paket raporu yeni hatayla birlikte görünmez.
    setPkg(null);
    try {
      setPkg(await api.workerPackageReport());
    } catch (e) {
      setPkgError(toApiError(e));
    } finally {
      setPkgBusy(false);
    }
  }

  const [apiEndpoint, setApiEndpoint] = useState("");
  const [apiModel, setApiModel] = useState("");
  const [apiMessage, setApiMessage] = useState("");
  const [profileBusy, setProfileBusy] = useState(false);

  // API anahtarının **gerçekten** okunduğu yer. Anahtar değeri hiçbir zaman
  // durumda tutulmaz: yalnız yazılırken `keyDraft`ta durur, gönderilince
  // temizlenir; sunucudan geri okunmaz (INV-11).
  const [keyStatus, setKeyStatus] = useState<ApiKeyStatus | null>(null);
  const [keyDraft, setKeyDraft] = useState("");
  const [keyMessage, setKeyMessage] = useState("");
  const [keyError, setKeyError] = useState<ApiError | null>(null);
  const [keyBusy, setKeyBusy] = useState(false);
  // Okuma ve yazma aynı nesil sayacını paylaşır: geç gelen eski yanıt yeni
  // durumun üstüne yazmaz, kilidi de erken açmaz.
  const keySeq = useRef(0);

  useEffect(() => {
    const seq = ++keySeq.current;
    api
      .aiSecretStatus()
      .then((status) => {
        if (seq !== keySeq.current) return;
        setKeyStatus(status);
        setKeyError(null);
      })
      .catch((failure) => {
        if (seq !== keySeq.current) return;
        setKeyStatus(null);
        setKeyError(toApiError(failure));
      });
  }, []);

  async function saveKey() {
    const submitted = keyDraft;
    setKeyBusy(true);
    setKeyMessage("");
    setKeyError(null);
    const seq = ++keySeq.current;
    try {
      const status = await api.aiSecretStore(submitted.trim());
      if (seq !== keySeq.current) return;
      setKeyStatus(status);
      setKeyMessage(t("providers.keySaved"));
      // Taslak koruması: yalnız **gönderilen** değer hâlâ yazılıysa temizle;
      // uçuşta yazılan yeni anahtar yanıtla silinmesin.
      setKeyDraft((current) => (current === submitted ? "" : current));
    } catch (failure) {
      if (seq !== keySeq.current) return;
      setKeyError(toApiError(failure));
    } finally {
      if (seq === keySeq.current) setKeyBusy(false);
    }
  }

  async function clearKey() {
    setKeyBusy(true);
    setKeyMessage("");
    setKeyError(null);
    const seq = ++keySeq.current;
    try {
      const status = await api.aiSecretClear();
      if (seq !== keySeq.current) return;
      setKeyStatus(status);
      setKeyMessage(t("providers.keyCleared"));
    } catch (failure) {
      if (seq !== keySeq.current) return;
      setKeyError(toApiError(failure));
    } finally {
      if (seq === keySeq.current) setKeyBusy(false);
    }
  }

  // Yalnız boşluktan oluşan alan geçerli bir uç nokta/model değildir; çekirdeğe
  // kırpılmış değer gider (kopyala-yapıştır boşluğu yazımı bozmasın).
  const profileReady = apiEndpoint.trim() !== "" && apiModel.trim() !== "";

  async function saveProfile() {
    // Tek uçuş: ikinci tık ikinci bir yazma göndermez.
    setProfileBusy(true);
    try {
      await api.providerApiProfileSave(apiEndpoint.trim(), apiModel.trim());
      setApiMessage(t("providers.profileSaved"));
    } catch (error) {
      setApiMessage(profileFailureText(error));
    } finally {
      setProfileBusy(false);
    }
  }

  return (
    <div className="page" style={{ maxWidth: 920 }}>
      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("providers.apiProfileTitle")}</h2>
            <p className="card-sub">
              {t("providers.apiKeySubBefore")} <strong>{t("providers.apiKeySubStrong")}</strong>
              {t("providers.apiKeySubAfter")}
            </p>
          </div>
        </div>
        <div className="row" style={{ gap: 6, flexWrap: "wrap" }}>
          <input
            aria-label={t("providers.endpointAria")}
            placeholder={t("providers.endpointPlaceholder")}
            value={apiEndpoint}
            disabled={profileBusy}
            onChange={(event) => {
              setApiEndpoint(event.target.value);
              // Alan değişti: önceki kaydın yeşil satırı bu değeri anlatmaz.
              setApiMessage("");
            }}
            style={{ minWidth: 260 }}
          />
          <input
            aria-label={t("providers.modelAria")}
            placeholder={t("providers.modelPlaceholder")}
            value={apiModel}
            disabled={profileBusy}
            onChange={(event) => {
              setApiModel(event.target.value);
              setApiMessage("");
            }}
          />
          <button
            className="btn"
            disabled={profileBusy || !profileReady}
            onClick={() => void saveProfile()}
          >
            {t("common.save")}
          </button>
        </div>
        {apiMessage ? <p className="card-sub">{apiMessage}</p> : null}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("providers.keyTitle")}</h2>
            <p className="card-sub">{t("providers.keySub")}</p>
          </div>
        </div>
        <p className="card-sub" data-testid="api-key-source">
          {t("providers.keySourceLabel")}: <strong>{keySourceText(keyStatus)}</strong>
          {keyStatus ? (
            <>
              {" · "}
              {keyStatus.present ? t("providers.keyPresent") : t("providers.keyMissing")}
            </>
          ) : null}
        </p>
        {keyStatus && !keyStatus.osStoreAvailable ? (
          <div className="notice">{t("providers.keyStoreUnavailable")}</div>
        ) : null}
        {keyStatus?.note ? <p className="card-sub">{keyStatus.note}</p> : null}
        <div className="row" style={{ gap: 6, flexWrap: "wrap" }}>
          <input
            type="password"
            aria-label={t("providers.keyInputAria")}
            placeholder={t("providers.keyPlaceholder")}
            value={keyDraft}
            disabled={keyBusy}
            onChange={(event) => {
              setKeyDraft(event.target.value);
              setKeyMessage("");
            }}
            style={{ minWidth: 260 }}
          />
          <button
            className="btn"
            disabled={keyBusy || keyDraft.trim() === "" || keyStatus?.osStoreAvailable === false}
            onClick={() => void saveKey()}
          >
            {keyBusy ? t("providers.keySaving") : t("providers.keySave")}
          </button>
          <button
            className="btn"
            disabled={keyBusy || keyStatus?.osStoreAvailable === false}
            onClick={() => void clearKey()}
          >
            {t("providers.keyClear")}
          </button>
        </div>
        {keyMessage ? <p className="card-sub">{keyMessage}</p> : null}
        {keyError ? (
          <div className="notice danger">
            <strong>{keyError.code}</strong> — {keyError.message}
          </div>
        ) : null}
      </div>

      <div className="page-head">
        <h1>{t("nav.ai")}</h1>
        <p className="lead">
          {t("providers.leadBefore")} <strong>{t("providers.leadStrong")}</strong>{" "}
          {t("providers.leadAfter")}
        </p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="notice">{t("providers.aiOffNotice")}</div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("providers.matrixTitle")}</h2>
            <p className="card-sub">
              {t("providers.matrixSubBefore")} <code>--help</code>{" "}
              {t("providers.matrixSubAfter")}
            </p>
          </div>
          <button
            className="btn"
            disabled={coreBusy}
            onClick={() => void runMatrix()}
          >
            {matrixBusy ? t("providers.matrixBusy") : t("providers.matrixRun")}
          </button>
        </div>

        {matrixError ? (
          <div className="notice danger" role="alert">
            <strong>{matrixError.code}</strong> — {matrixError.message}
          </div>
        ) : null}

        {matrix ? (
          <>
            <div className="muted small" style={{ marginBottom: "var(--space-3)" }}>
              {t("providers.matrixProbed", {
                at: formatUnixSeconds(matrix.probedAtUnix),
                seconds: Math.round(matrix.helpTimeoutMs / 1000),
              })}{" "}
              <strong>{matrixWorstLink(matrix).label}</strong>
            </div>
            <div style={{ display: "grid", gap: "var(--space-3)" }}>
              {matrix.adapters.map((adapter) => {
                const warnings = capabilityWarnings(adapter);
                const prompt = promptShapeText(adapter);
                const measured = hardening[adapter.adapterId];
                const measuredText = measured ? hardeningProbeText(measured) : null;
                const probeError = hardeningErrors[adapter.adapterId];
                return (
                  <div
                    key={adapter.adapterId}
                    className="card tight"
                    style={{ background: "var(--surface)" }}
                  >
                    <div className="card-head">
                      <div>
                        <h3 style={{ margin: 0, fontSize: 14 }}>{adapter.displayName}</h3>
                        <p className="card-sub" style={MONO}>
                          {adapter.version ?? t("providers.versionUnknown")} ·{" "}
                          {capabilityHelpCommand(adapter)}
                        </p>
                      </div>
                      <div style={{ display: "flex", gap: "var(--space-2)", flexWrap: "wrap" }}>
                        <span className={`badge ${linkTone(adapter.link)}`}>{adapter.linkLabel}</span>
                        <span className={`badge ${adapter.gateOpen ? "warn" : "plain"}`}>
                          {adapter.gateOpen ? t("providers.gateOpen") : t("providers.gateClosed")}
                        </span>
                      </div>
                    </div>
                    <ul
                      className="small"
                      style={{ listStyle: "none", margin: 0, padding: 0, display: "grid", gap: 6 }}
                    >
                      {adapter.rows.map((row) => (
                        <li
                          key={row.capability}
                          style={{
                            display: "grid",
                            gridTemplateColumns: "1fr auto",
                            gap: "var(--space-2)",
                            alignItems: "start",
                          }}
                        >
                          <div>
                            <strong style={{ fontWeight: 600 }}>{row.label}</strong>
                            {row.required ? (
                              <span className="muted small">
                                {" "}
                                {t("providers.requiredSuffix")}
                              </span>
                            ) : null}
                            <div style={MONO}>{capabilityDetailText(row)}</div>
                            <div className="muted small">{row.note}</div>
                          </div>
                          <span className={`badge ${capabilityTone(row.state)}`}>{row.stateLabel}</span>
                        </li>
                      ))}
                    </ul>
                    <div className="muted small" style={{ marginTop: "var(--space-2)" }}>
                      {capabilitySummary(adapter)} · {capabilityGateText(adapter)}
                    </div>
                    <div
                      className={prompt.tone === "warn" ? "notice danger" : "notice"}
                      style={{ fontSize: 12, marginTop: "var(--space-2)" }}
                    >
                      {prompt.text}
                    </div>
                    {adapter.prompt.argv.length > 0 ? (
                      <div style={{ marginTop: "var(--space-2)", display: "grid", gap: 6 }}>
                        <div
                          style={{
                            display: "flex",
                            gap: "var(--space-2)",
                            alignItems: "center",
                            flexWrap: "wrap",
                          }}
                        >
                          <button
                            className="btn"
                            disabled={coreBusy}
                            onClick={() => void runHardening(adapter.adapterId)}
                          >
                            {hardeningBusy === adapter.adapterId
                              ? t("providers.hardeningBusy")
                              : t("providers.hardeningRun")}
                          </button>
                          <span className="muted small">{t("providers.hardeningHint")}</span>
                        </div>
                        {probeError ? (
                          <div className="notice danger" role="alert" style={{ fontSize: 12 }}>
                            <strong>{probeError.code}</strong> — {probeError.message}
                          </div>
                        ) : null}
                        {measured && measuredText ? (
                          <>
                            <div
                              className={measuredText.tone === "ok" ? "notice" : "notice danger"}
                              style={{ fontSize: 12 }}
                            >
                              {measuredText.text}
                            </div>
                            <div className="muted small" style={MONO}>
                              {t("providers.hardeningArgv", {
                                argv: measured.argvTemplate.join(" "),
                              })}
                            </div>
                            <details>
                              <summary className="muted small" style={{ cursor: "pointer" }}>
                                {t("providers.hardeningReply")}
                              </summary>
                              <pre
                                className="small"
                                style={{
                                  whiteSpace: "pre-wrap",
                                  margin: "var(--space-1) 0 0",
                                  maxHeight: 180,
                                  overflow: "auto",
                                }}
                              >
                                {measured.replyHead || t("providers.hardeningNoText")}
                              </pre>
                              {measured.stderrHead ? (
                                <>
                                  <div className="muted small" style={{ marginTop: 4 }}>
                                    {t("providers.hardeningStderr")}
                                  </div>
                                  <pre
                                    className="small"
                                    style={{
                                      whiteSpace: "pre-wrap",
                                      margin: "var(--space-1) 0 0",
                                      maxHeight: 140,
                                      overflow: "auto",
                                    }}
                                  >
                                    {measured.stderrHead}
                                  </pre>
                                </>
                              ) : null}
                              {measured.notes.map((note) => (
                                <div key={note} className="muted small">
                                  {note}
                                </div>
                              ))}
                            </details>
                          </>
                        ) : null}
                      </div>
                    ) : null}
                    {warnings.map((warning) => (
                      <div key={warning} className="notice" style={{ fontSize: 12, marginTop: "var(--space-2)" }}>
                        {warning}
                      </div>
                    ))}
                  </div>
                );
              })}
            </div>
            <details style={{ marginTop: "var(--space-3)" }}>
              <summary className="muted small" style={{ cursor: "pointer" }}>
                {t("providers.matrixLimits")}
              </summary>
              <ul className="small" style={{ margin: "var(--space-1) 0 0", paddingLeft: 18 }}>
                {matrix.notes.map((note) => (
                  <li key={note}>{note}</li>
                ))}
              </ul>
            </details>
          </>
        ) : (
          <p className="muted small">
            {t("providers.matrixIdleBefore")} <code>--help</code>{" "}
            {t("providers.matrixIdleAfter")}
          </p>
        )}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("providers.packageTitle")}</h2>
            <p className="card-sub">
              {t("providers.packageSubBefore")}{" "}
              <strong>{t("providers.packageSubStrong")}</strong>{" "}
              {t("providers.packageSubAfter")}
            </p>
          </div>
          <button className="btn" disabled={pkgBusy} onClick={() => void runPackage()}>
            {pkgBusy ? t("providers.packageBusy") : t("providers.packageRun")}
          </button>
        </div>

        {pkgError ? (
          <div className="notice danger" role="alert">
            <strong>{pkgError.code}</strong> — {pkgError.message}
          </div>
        ) : null}

        {pkg ? (
          <>
            <div
              className="muted small"
              style={{ display: "flex", gap: "var(--space-2)", flexWrap: "wrap", alignItems: "center" }}
            >
              <span className={`badge ${packageTone(pkg)}`}>{packageStatusText(pkg)}</span>
              <span>{packageSummary(pkg.manifest)}</span>
              {pkg.manifest ? (
                <span style={MONO}>
                  {pkg.manifest.buildPlatform} ·{" "}
                  {t("providers.manifestLine", {
                    python: pkg.manifest.pythonVersion,
                    pyinstaller: pkg.manifest.pyinstallerVersion,
                  })}
                </span>
              ) : null}
            </div>

            {pkg.verification ? (
              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "repeat(3, minmax(0, 1fr))",
                  gap: "var(--space-2)",
                  marginTop: "var(--space-3)",
                }}
              >
                {(() => {
                  const counts = verificationCounts(pkg.verification);
                  return (
                    <>
                      <div className="card tight">
                        <div className="muted small">{t("providers.countVerified")}</div>
                        <strong>{counts.verified}</strong>
                      </div>
                      <div className="card tight">
                        <div className="muted small">{t("providers.countProblems")}</div>
                        <strong style={{ color: counts.problems > 0 ? "var(--danger)" : undefined }}>
                          {counts.problems}
                        </strong>
                      </div>
                      <div className="card tight">
                        <div className="muted small">{t("providers.countExtra")}</div>
                        <strong style={{ color: counts.extra > 0 ? "var(--danger)" : undefined }}>
                          {counts.extra}
                        </strong>
                      </div>
                    </>
                  );
                })()}
              </div>
            ) : null}

            {pkg.packageDir ? (
              <div className="muted small" style={{ ...MONO, marginTop: "var(--space-2)" }}>
                {pkg.packageDir}
              </div>
            ) : null}

            {roleRows(pkg.manifest).length > 0 ? (
              <details style={{ marginTop: "var(--space-3)" }}>
                <summary className="muted small" style={{ cursor: "pointer" }}>
                  {t("providers.rolesSummary")}
                </summary>
                <ul className="small" style={{ margin: "var(--space-1) 0 0", paddingLeft: 18 }}>
                  {roleRows(pkg.manifest).map((row) => (
                    <li key={row.role}>
                      {t("providers.roleRow", { label: row.label, count: row.count })}
                    </li>
                  ))}
                </ul>
              </details>
            ) : null}

            {pkg.verification && pkg.verification.problems.length > 0 ? (
              <div className="notice danger" style={{ marginTop: "var(--space-2)" }}>
                <strong>{t("providers.problemsTitle")}</strong>
                <ul className="small" style={{ margin: "var(--space-1) 0 0", paddingLeft: 18 }}>
                  {pkg.verification.problems.map((problem) => (
                    <li key={`${problem.path}-${problem.problem}`}>
                      <code>{problem.path}</code> — {problemText(problem)}
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}

            {pkg.verification && pkg.verification.extra.length > 0 ? (
              <div className="notice" style={{ marginTop: "var(--space-2)" }}>
                <strong>{t("providers.extraTitle")}</strong>
                <ul className="small" style={{ margin: "var(--space-1) 0 0", paddingLeft: 18 }}>
                  {pkg.verification.extra.map((path) => (
                    <li key={path}>
                      <code>{path}</code>
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}

            <div className="muted small" style={{ marginTop: "var(--space-2)" }}>
              {t("providers.tesseractLabel")}{" "}
              <strong>
                {pkg.tesseractOnPath
                  ? t("providers.tesseractFound")
                  : t("providers.tesseractMissing")}
              </strong>{" "}
              · {t("providers.tesseractNoEmbed")}
              {pkg.manifest?.tesseract.note ? ` — ${pkg.manifest.tesseract.note}` : ""}
            </div>

            {packageWarnings(pkg).map((warning) => (
              <div
                key={warning}
                className="notice"
                style={{ fontSize: 12, marginTop: "var(--space-2)" }}
              >
                {warning}
              </div>
            ))}

            <details style={{ marginTop: "var(--space-3)" }}>
              <summary className="muted small" style={{ cursor: "pointer" }}>
                {t("providers.packageLimits")}
              </summary>
              <p className="muted small" style={{ margin: "var(--space-1) 0 0" }}>
                {packageLimits()}
              </p>
            </details>
          </>
        ) : (
          <p className="muted small">{t("providers.packageIdle")}</p>
        )}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("providers.attackTitle")}</h2>
            <p className="card-sub">
              {t("providers.attackSubBefore")} <code>--help</code>{" "}
              {t("providers.attackSubMid")} <code>HOME</code>{" "}
              {t("providers.attackSubAfter")}
            </p>
          </div>
          <button
            className="btn"
            disabled={coreBusy}
            onClick={() => void runAttack()}
          >
            {attackBusy ? t("providers.attackBusy") : t("providers.attackRun")}
          </button>
        </div>

        {attackError ? (
          <div className="notice danger" role="alert">
            <strong>{attackError.code}</strong> — {attackError.message}
          </div>
        ) : null}

        {attack ? (
          <>
            <div className="muted small" style={{ marginBottom: "var(--space-3)" }}>
              {t("providers.attackProbed", { at: formatUnixSeconds(attack.probedAtUnix) })}{" "}
              <strong>{attackWorstVerdict(attack.adapters).label}</strong> ·{" "}
              {t("providers.attackHitsLabel")} <strong>{attackTotalHits(attack)}</strong>
            </div>

            <div className="notice" style={{ fontSize: 12 }}>
              <strong>{t("providers.fixtureLabel")}</strong>{" "}
              {attackFixtureSummary(attack.fixture)}
              <ul style={{ margin: "var(--space-1) 0 0", paddingLeft: 18 }}>
                {attack.fixture.productConfig.map((item) => (
                  <li key={item.vector}>{attackProductText(item)}</li>
                ))}
              </ul>
              {attackFixtureWarnings(attack.fixture).map((warning) => (
                <div key={warning} style={{ marginTop: "var(--space-2)" }}>
                  {warning}
                </div>
              ))}
            </div>

            <div style={{ display: "grid", gap: "var(--space-3)", marginTop: "var(--space-3)" }}>
              {attack.adapters.map((adapter) => {
                const warnings = attackAdapterWarnings(adapter);
                return (
                  <div
                    key={adapter.adapterId}
                    className="card tight"
                    style={{ background: "var(--surface)" }}
                  >
                    <div className="card-head">
                      <div>
                        <h3 style={{ margin: 0, fontSize: 14 }}>{adapter.displayName}</h3>
                        <p className="card-sub" style={MONO}>
                          {attackCommand(adapter)}
                        </p>
                      </div>
                      <div style={{ display: "flex", gap: "var(--space-2)", flexWrap: "wrap" }}>
                        <span className={`badge ${linkTone(adapter.link)}`}>{adapter.linkLabel}</span>
                        <span className={`badge ${adapter.hits > 0 ? "danger" : "plain"}`}>
                          {adapter.hits > 0
                            ? t("providers.hitsBadge", { count: adapter.hits })
                            : t("providers.noHits")}
                        </span>
                        {adapter.skipped ? (
                          <span className="badge plain">{attackSkipText(adapter.skipped)}</span>
                        ) : null}
                      </div>
                    </div>
                    {adapter.vectors.length > 0 ? (
                      <ul
                        className="small"
                        style={{ listStyle: "none", margin: 0, padding: 0, display: "grid", gap: 6 }}
                      >
                        {adapter.vectors.map((vector) => (
                          <li
                            key={vector.vector}
                            style={{
                              display: "grid",
                              gridTemplateColumns: "1fr auto",
                              gap: "var(--space-2)",
                              alignItems: "start",
                            }}
                          >
                            <div>
                              <strong style={{ fontWeight: 600 }}>{vector.label}</strong>
                              <div style={MONO}>
                                {t("providers.guardPrefix")} {attackGuardText(vector)}
                              </div>
                              <div className="muted small">{vector.note}</div>
                            </div>
                            <span className={`badge ${attackVerdictTone(vector.verdict)}`}>
                              {attackVerdictText(vector)}
                            </span>
                          </li>
                        ))}
                      </ul>
                    ) : null}
                    {warnings.map((warning) => (
                      <div
                        key={warning}
                        className="notice"
                        style={{ fontSize: 12, marginTop: "var(--space-2)" }}
                      >
                        {warning}
                      </div>
                    ))}
                  </div>
                );
              })}
            </div>

            <details style={{ marginTop: "var(--space-3)" }}>
              <summary className="muted small" style={{ cursor: "pointer" }}>
                {t("providers.attackLimits")}
              </summary>
              <ul className="small" style={{ margin: "var(--space-1) 0 0", paddingLeft: 18 }}>
                {attack.notes.map((note) => (
                  <li key={note}>{note}</li>
                ))}
              </ul>
            </details>
          </>
        ) : (
          <p className="muted small">{t("providers.attackIdle")}</p>
        )}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("providers.archTitle")}</h2>
            <p className="card-sub">
              {t("providers.archSubBefore")} <code>#!</code>{" "}
              {t("providers.archSubAfter")}
            </p>
          </div>
          <button
            className="btn"
            disabled={archBusy || running !== null || matrixBusy || attackBusy}
            onClick={() => void runArch()}
          >
            {archBusy ? t("providers.archBusy") : t("providers.archRun")}
          </button>
        </div>

        {archError ? (
          <div className="notice danger" role="alert">
            <strong>{archError.code}</strong> — {archError.message}
          </div>
        ) : null}

        {arch ? (
          <>
            <div className="muted small" style={{ marginBottom: "var(--space-3)" }}>
              {t("providers.archProbed", {
                at: archProbedAt(arch.probedAtUnix),
                summary: archSummaryText(arch),
              })}{" "}
              <strong>{archWorst(arch).label}</strong>
            </div>
            <div style={{ display: "grid", gap: "var(--space-3)" }}>
              {arch.adapters.map((adapter) => {
                const reason = archReasonText(adapter.reason);
                const interpreter = archInterpreterText(adapter);
                return (
                  <div
                    key={adapter.adapterId}
                    className="card tight"
                    style={{ background: "var(--surface)" }}
                  >
                    <div className="card-head">
                      <div>
                        <h3 style={{ margin: 0, fontSize: 14 }}>{adapter.displayName}</h3>
                        <p className="card-sub" style={MONO}>
                          {adapter.command} · {archFormatText(adapter)}
                        </p>
                      </div>
                      <span className={`badge ${archTone(adapter.comparison)}`}>
                        {archBadgeText(adapter)}
                      </span>
                    </div>
                    <dl className="kv" style={{ gridTemplateColumns: "110px 1fr" }}>
                      <dt>{t("providers.pathLabel")}</dt>
                      <dd style={MONO}>{archPathText(adapter)}</dd>
                      <dt>{t("providers.archLabel")}</dt>
                      <dd style={MONO}>
                        {archList(adapter.architectures)} · {adapter.comparisonLabel}
                      </dd>
                      {interpreter ? (
                        <>
                          <dt>{t("providers.interpreterLabel")}</dt>
                          <dd style={MONO}>{interpreter}</dd>
                        </>
                      ) : null}
                      {reason ? (
                        <>
                          <dt>{t("providers.reasonLabel")}</dt>
                          <dd>{reason}</dd>
                        </>
                      ) : null}
                      <dt>{t("providers.headerLabel")}</dt>
                      <dd>
                        {adapter.headerBytes > 0
                          ? t("providers.headerBytes", { bytes: adapter.headerBytes })
                          : t("common.unknown")}
                      </dd>
                    </dl>
                  </div>
                );
              })}
            </div>
            {archWarnings(arch).map((warning) => (
              <div key={warning} className="notice" style={{ fontSize: 12, marginTop: "var(--space-2)" }}>
                {warning}
              </div>
            ))}
            <details style={{ marginTop: "var(--space-3)" }}>
              <summary className="muted small" style={{ cursor: "pointer" }}>
                {t("providers.archLimits")}
              </summary>
              <ul className="small" style={{ margin: "var(--space-1) 0 0", paddingLeft: 18 }}>
                {arch.notes.map((note) => (
                  <li key={note}>{note}</li>
                ))}
              </ul>
            </details>
          </>
        ) : (
          <p className="muted small">{t("providers.archIdle")}</p>
        )}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("providers.cliTitle")}</h2>
            <p className="card-sub">{t("providers.cliSub")}</p>
          </div>
          <button className="btn" disabled={busy} onClick={() => void refresh()}>
            {busy ? t("providers.refreshBusy") : t("providers.refresh")}
          </button>
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-3)" }}>
          {rows.map((row) => {
            const result = results[row.id];
            const runError = runErrors[row.id];
            const status = result ? probeStatusLabel(result) : null;
            const warnings = result
              ? [...probeWarnings(result), ...probeScopeWarnings(result)]
              : [];
            const outputs = result ? probeOutputs(result) : [];
            return (
              <div key={row.id} className="card tight" style={{ background: "var(--surface)" }}>
                <div className="card-head">
                  <div>
                    <h2>{row.displayName}</h2>
                    <p className="card-sub" style={{ fontFamily: "var(--mono)", fontSize: 11 }}>{row.command}</p>
                  </div>
                  <div style={{ display: "flex", gap: "var(--space-2)", flexWrap: "wrap" }}>
                    <span
                      className={`badge ${
                        row.state === "installed" ? "ok" : row.state === "unverified" ? "warn" : "plain"
                      }`}
                    >
                      {row.state === "installed"
                        ? t("providers.stateInstalled")
                        : row.state === "unverified"
                          ? t("providers.stateLimited")
                          : t("providers.stateMissing")}
                    </span>
                    {row.architecture ? (
                      <span className={`badge ${adapterArchBadge(row.architecture).tone}`}>
                        {adapterArchBadge(row.architecture).label}
                      </span>
                    ) : null}
                  </div>
                </div>
                <dl className="kv" style={{ gridTemplateColumns: "100px 1fr" }}>
                  <dt>{t("providers.versionLabel")}</dt><dd>{row.version ?? t("common.unknown")}</dd>
                  <dt>{t("providers.pathLabel")}</dt>
                  <dd style={MONO}>{row.path ?? t("providers.pathNotOnPath")}</dd>
                  <dt>{t("providers.archLabel")}</dt>
                  <dd style={MONO}>
                    {row.architecture
                      ? `${archList(row.architecture.architectures)} · ${archBadgeText(row.architecture)}`
                      : t("common.unknown")}
                  </dd>
                  <dt>{t("providers.identityLabel")}</dt><dd>{authLabel(row.authMode)}</dd>
                  <dt>{t("providers.allowedLabel")}</dt>
                  <dd style={{ fontFamily: "var(--mono)", fontSize: 11 }}>{row.allowedArgs.join(" ") || t("common.unknown")}</dd>
                </dl>
                {row.batchForbidden ? (
                  <div className="notice" style={{ fontSize: 12 }}>
                    {t("providers.batchForbidden")}
                  </div>
                ) : null}
                {row.state !== "installed" && !row.batchForbidden ? (
                  <div className="muted small" style={{ marginTop: "var(--space-2)" }}>
                    {t("providers.probeUnavailable")}
                  </div>
                ) : null}

                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: "var(--space-2)",
                    flexWrap: "wrap",
                    marginTop: "var(--space-3)",
                  }}
                >
                  <button
                    className="btn"
                    disabled={!probeCanRun(row, running) || coreBusy}
                    onClick={() => void runProbe(row.id)}
                  >
                    {running === row.id ? t("providers.probing") : t("providers.probeRun")}
                  </button>
                  {running === row.id ? (
                    <button className="btn ghost" disabled={cancelBusy} onClick={() => void cancelProbe()}>
                      {t("providers.probeCancel")}
                    </button>
                  ) : null}
                  <span className="muted small">{t("providers.probeHint")}</span>
                </div>

                {runError ? (
                  <div className="notice danger" role="alert" style={{ marginTop: "var(--space-3)", fontSize: 12 }}>
                    <strong>{runError.code}</strong> — {runError.message}
                  </div>
                ) : null}

                {result && status ? (
                  <div
                    style={{
                      marginTop: "var(--space-3)",
                      borderTop: "1px solid var(--line)",
                      paddingTop: "var(--space-3)",
                    }}
                  >
                    <div style={{ display: "flex", gap: "var(--space-2)", alignItems: "center", flexWrap: "wrap" }}>
                      <span className={`badge ${status.tone}`}>{status.label}</span>
                      <span className="muted small">{probeSummary(result)}</span>
                    </div>
                    <dl className="kv" style={{ gridTemplateColumns: "88px 1fr", marginTop: "var(--space-2)" }}>
                      <dt>{t("providers.commandLabel")}</dt><dd style={MONO}>{probeArgv(result)}</dd>
                      <dt>{t("providers.envLabel")}</dt><dd style={MONO}>{result.envKeys.join(", ") || t("common.unknown")}</dd>
                      {probeScopeLines(result).map((line) => (
                        <Fragment key={line.label}>
                          <dt>{line.label}</dt>
                          <dd>{line.value}</dd>
                        </Fragment>
                      ))}
                      <dt>{t("providers.workdirLabel")}</dt>
                      <dd style={MONO}>
                        {result.workdir} <span className="muted">{t("providers.workdirNote")}</span>
                      </dd>
                    </dl>
                    {result.sandbox.files.length > 0 ? (
                      <details style={{ marginTop: "var(--space-2)" }}>
                        <summary className="muted small" style={{ cursor: "pointer" }}>
                          {t("providers.sandboxFiles", { count: result.sandbox.files.length })}
                          {result.sandbox.filesTruncated ? t("providers.sandboxFilesCut") : ""}
                        </summary>
                        <ul className="small" style={{ margin: "var(--space-1) 0 0", paddingLeft: 18 }}>
                          {result.sandbox.files.map((file) => (
                            <li key={file.name} style={MONO}>
                              {file.name} · {formatBytes(file.bytes)}
                            </li>
                          ))}
                        </ul>
                      </details>
                    ) : null}
                    {warnings.map((warning) => (
                      <div key={warning} className="notice" style={{ fontSize: 12, marginTop: "var(--space-2)" }}>
                        {warning}
                      </div>
                    ))}
                    {outputs.length === 0 ? (
                      <p className="muted small" style={{ margin: "var(--space-2) 0 0" }}>
                        {t("providers.noOutput")}
                      </p>
                    ) : (
                      outputs.map((output) => (
                        <div key={output.stream} style={{ marginTop: "var(--space-2)" }}>
                          <span className="muted small">{output.stream}</span>
                          <pre style={PRE}>{output.text}</pre>
                        </div>
                      ))
                    )}
                  </div>
                ) : null}
              </div>
            );
          })}
          {rows.length === 0 && !busy ? (
            <div className="empty"><p style={{ margin: 0 }}>{t("providers.probeEmpty")}</p></div>
          ) : null}
        </div>
        {cancelHint ? (
          <div className="muted small" style={{ marginTop: "var(--space-3)" }}>
            {cancelHint}
          </div>
        ) : null}
      </div>

      <ResiduePanel rows={rows} coreBusy={coreBusy} onCoreBusy={setCoreBusy} />

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("providers.localModelTitle")}</h2>
            <p className="card-sub">{t("providers.localModelSub")}</p>
          </div>
          <span className="badge plain">{t("providers.closedBadge")}</span>
        </div>
        <p className="muted small" style={{ margin: 0 }}>
          {t("providers.localModelNote")}
        </p>
      </div>
    </div>
  );
}

const RESIDUE_TH: CSSProperties = {
  textAlign: "left",
  fontWeight: 500,
  padding: "2px 8px 2px 0",
  color: "var(--muted)",
};

const RESIDUE_TD: CSSProperties = { padding: "2px 8px 2px 0", verticalAlign: "top" };

/**
 * F10-T13 — kalıntı envanteri, ölçümü ve **yalnız uygulamaya ait** dosyaların
 * temizliği. Tarama yalnız bildirilen yolları üst veriyle okur; dosya içeriği
 * açılmaz. Ölçüm gerçek ev dizininde çalışır ve açık onay ister.
 */
function ResiduePanel({
  rows,
  coreBusy,
  onCoreBusy,
}: {
  rows: AdapterView[];
  /** Çekirdeğin süreç-geneli CLI kilidi başka bir kartta tutuluyor mu. */
  coreBusy: boolean;
  /** Ölçüm çekirdek kilidini tutarken üst panele bildirilir. */
  onCoreBusy: (busy: boolean) => void;
}) {
  const [adapterId, setAdapterId] = useState("claude-code");
  const [scan, setScan] = useState<CliResidueScan | null>(null);
  const [measure, setMeasure] = useState<CliResidueMeasure | null>(null);
  const [cleanup, setCleanup] = useState<CliResidueCleanup | null>(null);
  const [sweep, setSweep] = useState<CliSandboxSweep | null>(null);
  const [busy, setBusy] = useState<"scan" | "measure" | "cleanup" | "sweep" | null>(null);
  const [failure, setFailure] = useState<ApiError | null>(null);
  const [confirmHome, setConfirmHome] = useState(false);
  // Uçuştaki yanıt **istek anındaki adaptöre** aittir. Adaptör değişince sayaç
  // artar: geç gelen tarama/ölçüm/temizlik yanıtı yeni adaptörün paneli gibi
  // yazılmaz ve kilidi yeni adaptörün isteğinden çalmaz.
  const contextSeq = useRef(0);

  const selected = rows.find((row) => row.id === adapterId) ?? null;
  const runnable = selected !== null && !selected.batchForbidden;

  async function runScan() {
    const seq = ++contextSeq.current;
    setBusy("scan");
    setFailure(null);
    try {
      const result = await api.cliResidueScan(adapterId);
      if (seq === contextSeq.current) setScan(result);
    } catch (e) {
      if (seq === contextSeq.current) setFailure(toApiError(e));
    } finally {
      if (seq === contextSeq.current) setBusy(null);
    }
  }

  async function runMeasure() {
    // Ölçüm çekirdekteki `cli_probe_lock` slotunu tutar: yanıt dönene kadar
    // yoklama/matris/sınama kapalı, kendi adaptörü değişse bile yeni ölçüm
    // kapalıdır (bitmemiş işlem ikinci isteğe `E_CLI_BUSY` verirdi).
    onCoreBusy(true);
    const seq = ++contextSeq.current;
    setBusy("measure");
    setFailure(null);
    setCleanup(null);
    // Yeni ölçüm düşerse önceki ölçüm "güncel" gibi okunmamalı: temizlik
    // düğmesi bayat hedef listesiyle açık kalırdı.
    setMeasure(null);
    try {
      const result = await api.cliResidueMeasure(adapterId, confirmHome);
      if (seq === contextSeq.current) {
        setMeasure(result);
        setScan(result.after);
      }
    } catch (e) {
      if (seq === contextSeq.current) setFailure(toApiError(e));
    } finally {
      if (seq === contextSeq.current) setBusy(null);
      // Çekirdek işlemi bitti: adaptör değişmiş olsa da kilit **burada** açılır
      // (bayrak adaptör değişiminde bırakılmaz, yoksa ikinci istek `E_CLI_BUSY`).
      onCoreBusy(false);
    }
  }

  async function runCleanup() {
    if (measure === null) return;
    const seq = ++contextSeq.current;
    // Hedefler istek anında donar: yanıt beklerken adaptör değişse bile
    // temizlik başka adaptörün ölçümüyle eşleşmez.
    const target = adapterId;
    const measuredAt = measure.measuredAtUnix;
    const targets = residueCleanupTargets(measure);
    setBusy("cleanup");
    setFailure(null);
    try {
      const result = await api.cliResidueCleanup(target, measuredAt, targets);
      if (seq !== contextSeq.current) return;
      setCleanup(result);
      setMeasure(null);
      // Tazeleme de istek anındaki adaptör için koşar.
      const refreshed = await api.cliResidueScan(target);
      if (seq === contextSeq.current) setScan(refreshed);
    } catch (e) {
      if (seq === contextSeq.current) setFailure(toApiError(e));
    } finally {
      if (seq === contextSeq.current) setBusy(null);
    }
  }

  async function runSweep() {
    // Süpürme adaptörden bağımsızdır ama tek `busy` bayrağını paylaşır:
    // adaptör değişiminden sonra dönen eski istek yeni isteğin kilidini açmamalı.
    const seq = ++contextSeq.current;
    setBusy("sweep");
    setFailure(null);
    // Bayat süpürme özeti yeni hatayla birlikte görünmez.
    setSweep(null);
    try {
      const result = await api.cliStaleSandboxSweep();
      if (seq === contextSeq.current) setSweep(result);
    } catch (e) {
      if (seq === contextSeq.current) setFailure(toApiError(e));
    } finally {
      if (seq === contextSeq.current) setBusy(null);
    }
  }

  const scanWarnings = scan ? residueScanWarnings(scan) : [];
  const measureWarnings = measure ? residueMeasureWarnings(measure) : [];

  return (
    <div className="card">
      <div className="card-head">
        <div>
          <h2>{t("providers.residueTitle")}</h2>
          <p className="card-sub">
            {t("providers.residueSubBefore")} <strong>{t("providers.residueSubStrong")}</strong>{" "}
            {t("providers.residueSubAfter")}
          </p>
        </div>
        <span className="badge plain">{t("providers.residueBadge")}</span>
      </div>

      <div
        style={{
          display: "flex",
          gap: "var(--space-2)",
          alignItems: "center",
          flexWrap: "wrap",
          marginBottom: "var(--space-3)",
        }}
      >
        <select
          value={adapterId}
          onChange={(event) => {
            // Uçuştaki istek artık bu panelin bağlamına ait değil: sayacı artır
            // ve kilidi bırak (yeni adaptörün isteği eski yanıtı beklemez).
            contextSeq.current += 1;
            setBusy(null);
            setAdapterId(event.target.value);
            setScan(null);
            setMeasure(null);
            setCleanup(null);
            setConfirmHome(false);
            // Önceki adaptörün hatası yeni seçimin hatası gibi okunmamalı.
            setFailure(null);
          }}
          aria-label={t("providers.residueSelectAria")}
        >
          {rows.map((row) => (
            <option key={row.id} value={row.id}>
              {row.displayName}
            </option>
          ))}
        </select>
        <button className="btn" disabled={busy !== null} onClick={() => void runScan()}>
          {busy === "scan" ? t("providers.residueScanBusy") : t("providers.residueScan")}
        </button>
        <button
          className="btn ghost"
          disabled={busy !== null}
          onClick={() => void runSweep()}
          title={t("providers.residueSweepTitle")}
        >
          {busy === "sweep" ? t("providers.residueSweepBusy") : t("providers.residueSweep")}
        </button>
      </div>

      {failure ? (
        <div className="notice danger" role="alert" style={{ fontSize: 12 }}>
          <strong>{failure.code}</strong> — {failure.message}
        </div>
      ) : null}

      {scan ? (
        <>
          <dl className="kv" style={{ gridTemplateColumns: "110px 1fr" }}>
            <dt>{t("providers.rootLabel")}</dt>
            <dd style={MONO}>{scan.root}</dd>
            <dt>{t("providers.totalLabel")}</dt>
            <dd>
              {t("probe.filesWithSize", {
                count: scan.totalFiles,
                size: formatBytes(scan.totalBytes),
              })}
            </dd>
            <dt>{t("providers.persistenceLabel")}</dt>
            <dd>{persistenceText(scan)}</dd>
          </dl>
          <table className="small" style={{ width: "100%", borderCollapse: "collapse", marginTop: "var(--space-2)" }}>
            <thead>
              <tr>
                <th style={RESIDUE_TH}>{t("providers.thPath")}</th>
                <th style={RESIDUE_TH}>{t("providers.thKind")}</th>
                <th style={RESIDUE_TH}>{t("providers.thState")}</th>
                <th style={RESIDUE_TH}>{t("providers.thAge")}</th>
                <th style={RESIDUE_TH}>{t("providers.thNote")}</th>
              </tr>
            </thead>
            <tbody>
              {scan.paths.map((path) => (
                <tr key={path.relative}>
                  <td style={{ ...RESIDUE_TD, ...MONO }}>{path.relative}</td>
                  <td style={RESIDUE_TD}>{path.kind}</td>
                  <td style={RESIDUE_TD}>{residuePathLine(path)}</td>
                  <td style={RESIDUE_TD}>{residueAge(path.newestAgeSecs)}</td>
                  <td style={{ ...RESIDUE_TD, color: "var(--muted)" }}>{path.note}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {scanWarnings.map((warning) => (
            <div key={warning} className="notice" style={{ fontSize: 12, marginTop: "var(--space-2)" }}>
              {warning}
            </div>
          ))}
          <div
            style={{
              marginTop: "var(--space-3)",
              borderTop: "1px solid var(--line)",
              paddingTop: "var(--space-3)",
            }}
          >
            <label className="small" style={{ display: "flex", gap: "var(--space-2)", alignItems: "center" }}>
              <input
                type="checkbox"
                checked={confirmHome}
                onChange={(event) => setConfirmHome(event.target.checked)}
              />
              {t("providers.confirmHome")}
            </label>
            <div style={{ display: "flex", gap: "var(--space-2)", alignItems: "center", marginTop: "var(--space-2)", flexWrap: "wrap" }}>
              <button
                className="btn"
                disabled={!confirmHome || !runnable || busy !== null || coreBusy}
                onClick={() => void runMeasure()}
              >
                {busy === "measure" ? t("providers.measureBusy") : t("providers.measureRun")}
              </button>
              <span className="muted small">{t("providers.measureHint")}</span>
            </div>
            {!runnable && selected ? (
              <p className="muted small" style={{ margin: "var(--space-2) 0 0" }}>
                {selected.batchForbidden
                  ? t("providers.measureForbidden")
                  : t("providers.measureNotInstalled")}
              </p>
            ) : null}
          </div>
        </>
      ) : (
        <p className="muted small" style={{ margin: 0 }}>
          {t("providers.residueIdle")}
        </p>
      )}

      {measure ? (
        <div
          style={{
            marginTop: "var(--space-3)",
            borderTop: "1px solid var(--line)",
            paddingTop: "var(--space-3)",
          }}
        >
          <div style={{ display: "flex", gap: "var(--space-2)", alignItems: "center", flexWrap: "wrap" }}>
            <span className="badge ok">{t("providers.measureDone")}</span>
            <span className="muted small">{residueMeasureSummary(measure)}</span>
          </div>
          <dl className="kv" style={{ gridTemplateColumns: "110px 1fr", marginTop: "var(--space-2)" }}>
            <dt>{t("providers.commandLabel")}</dt>
            <dd style={MONO}>{[measure.root, ...measure.args].join(" ")}</dd>
            <dt>{t("providers.beforeLabel")}</dt>
            <dd>
              {t("probe.filesWithSize", {
                count: measure.before.totalFiles,
                size: formatBytes(measure.before.totalBytes),
              })}
            </dd>
            <dt>{t("providers.afterLabel")}</dt>
            <dd>
              {t("probe.filesWithSize", {
                count: measure.after.totalFiles,
                size: formatBytes(measure.after.totalBytes),
              })}
            </dd>
          </dl>
          {measure.created.length > 0 ? (
            <details open style={{ marginTop: "var(--space-2)" }}>
              <summary className="muted small" style={{ cursor: "pointer" }}>
                {t("providers.createdFiles", { count: measure.created.length })}
              </summary>
              <ul className="small" style={{ margin: "var(--space-1) 0 0", paddingLeft: 18 }}>
                {measure.created.map((file) => (
                  <li key={file.relative} style={MONO}>
                    {file.relative} · {formatBytes(file.bytes)}
                  </li>
                ))}
              </ul>
            </details>
          ) : null}
          {measure.modified.length > 0 ? (
            <details style={{ marginTop: "var(--space-2)" }}>
              <summary className="muted small" style={{ cursor: "pointer" }}>
                {t("providers.modifiedFiles", { count: measure.modified.length })}
              </summary>
              <ul className="small" style={{ margin: "var(--space-1) 0 0", paddingLeft: 18 }}>
                {measure.modified.map((file) => (
                  <li key={file.relative} style={MONO}>
                    {file.relative} · {formatBytes(file.bytes)}
                  </li>
                ))}
              </ul>
            </details>
          ) : null}
          {measureWarnings.map((warning) => (
            <div key={warning} className="notice" style={{ fontSize: 12, marginTop: "var(--space-2)" }}>
              {warning}
            </div>
          ))}
          <div style={{ display: "flex", gap: "var(--space-2)", alignItems: "center", marginTop: "var(--space-2)", flexWrap: "wrap" }}>
            <button
              className="btn ghost"
              disabled={measure.created.length === 0 || busy !== null}
              onClick={() => void runCleanup()}
            >
              {busy === "cleanup"
                ? t("providers.cleanupBusy")
                : t("providers.cleanupRun", { count: measure.created.length })}
            </button>
            <span className="muted small">{measure.cleanupHint}</span>
          </div>
        </div>
      ) : null}

      {cleanup ? (
        <div className="notice" style={{ fontSize: 12, marginTop: "var(--space-2)" }}>
          {residueCleanupSummary(cleanup)}
          {cleanup.skipped.length > 0 ? (
            <ul className="small" style={{ margin: "var(--space-1) 0 0", paddingLeft: 18 }}>
              {cleanup.skipped.map((skip) => (
                <li key={`${skip.path}-${skip.reason}`} style={MONO}>
                  {skip.path} — {skip.reason}
                </li>
              ))}
            </ul>
          ) : null}
        </div>
      ) : null}

      {sweep ? (
        <div className="notice" style={{ fontSize: 12, marginTop: "var(--space-2)" }}>
          {sandboxSweepSummary(sweep)}
          {sweep.skipped.length > 0 ? (
            <ul className="small" style={{ margin: "var(--space-1) 0 0", paddingLeft: 18 }}>
              {sweep.skipped.map((skip) => (
                <li key={`${skip.path}-${skip.reason}`} style={MONO}>
                  {skip.path} — {skip.reason}
                </li>
              ))}
            </ul>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
