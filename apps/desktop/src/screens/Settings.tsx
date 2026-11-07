import { useEffect, useRef, useState } from "react";
import { open as openDirectory } from "@tauri-apps/plugin-dialog";
import { DiagnosticsCard } from "../components/DiagnosticsCard";
import { JobQueueCard } from "../components/JobQueueCard";
import { WorkerBudgetCard } from "../components/WorkerBudgetCard";
import {
  api,
  toApiError,
  type ApiError,
  type AuditChainStatus,
  type AuditEventRow,
  type BackupResult,
  type BackupRunRow,
  type PreUpdateBackup,
  type RollbackResult,
  type VaultStatus,
} from "../lib/api";
import type { VaultGcResult } from "../lib/api";
import { formatDateTime, formatSize } from "../lib/format";
import { t } from "../lib/i18n";
import { version as appVersion } from "../../package.json";
import {
  BACKUP_LAST_KEY,
  DEFAULT_BACKUP_DAYS,
  nextBackupAt,
  readBackupLast,
  readBackupStatus,
} from "../lib/backup-schedule";
import {
  clearBackupSchedule,
  loadBackupSchedule,
  saveBackupSchedule,
  type BackupScheduleLoad,
  type BackupScheduleSource,
} from "../lib/backup-config";
import {
  backupStatusText,
  backupVersionLabel,
  backupWarnings,
  formatBackupDate,
  rollbackConsequences,
  rollbackDisabledReason,
  rollbackSummary,
} from "../lib/rollback";

/** Denetim listesinde gösterilen kayıt sayısı; üst sınırı çekirdek de kırpar. */
const AUDIT_LIST_LIMIT = 50;

/** Yedek geçmişinde gösterilen kayıt sayısı; üst sınırı çekirdek de kırpar. */
const BACKUP_HISTORY_LIMIT = 20;

/**
 * Yedek durumu **kararlı** etiketlerden yazılır; tanınmayan durum gizlenmez,
 * ham değeriyle görünür (sessizce "tamamlandı" denmez).
 */
function backupRunStatusLabel(status: string): string {
  switch (status) {
    case "verified":
      return t("settings.backupHistoryStatusVerified");
    case "succeeded":
      return t("settings.backupHistoryStatusSucceeded");
    case "failed":
      return t("settings.backupHistoryStatusFailed");
    case "running":
      return t("settings.backupHistoryStatusRunning");
    default:
      return status;
  }
}

function backupRunStatusTone(status: string): string {
  if (status === "verified" || status === "succeeded") return "ok";
  if (status === "failed") return "danger";
  if (status === "running") return "warn";
  return "plain";
}

/** Uzun kimlik listede kısaltılır; tam değer `title`da durur (kaybolmaz). */
function shortId(value: string): string {
  return value.length > 12 ? `${value.slice(0, 8)}…` : value;
}

export function Settings() {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [backup, setBackup] = useState<BackupResult | null>(null);
  const [gc, setGc] = useState<VaultGcResult | null>(null);
  const [autoBackup, setAutoBackup] = useState(() => readBackupStatus());
  // Planın kaynağı ve okuma hatası: "plan yok" ile "okunamadı" ayrı yazılır.
  const [scheduleSource, setScheduleSource] = useState<BackupScheduleSource | null>(null);
  const [scheduleError, setScheduleError] = useState<string | null>(null);
  const scheduleSeq = useRef(0);
  const [oldPassword, setOldPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [newPassword2, setNewPassword2] = useState("");
  const [passwordChanged, setPasswordChanged] = useState(false);
  const [restoreDir, setRestoreDir] = useState("");
  const [destDir, setDestDir] = useState("");
  const [restored, setRestored] = useState<string | null>(null);
  const [restoreInfo, setRestoreInfo] = useState<{
    verifiedFiles: number;
    verifiedBytes: number;
    verifyMs: number;
    unlistedBlobs: string[];
  } | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  const [preUpdate, setPreUpdate] = useState<PreUpdateBackup[]>([]);
  const [preUpdateLoaded, setPreUpdateLoaded] = useState(false);
  const [preUpdateSelected, setPreUpdateSelected] = useState("");
  const [rollbackConfirm, setRollbackConfirm] = useState("");
  const [rollbackDone, setRollbackDone] = useState<RollbackResult | null>(null);

  // Denetim defteri (kasa geneli). Okuma ve doğrulama **ayrı** uçuşlardır:
  // biri düşerken diğerinin sonucu yazılmaz. `auditRows === null` "bilinmiyor"
  // demektir; boş liste ancak başarılı okumadan sonra "kayıt yok" olur.
  const [auditRows, setAuditRows] = useState<AuditEventRow[] | null>(null);
  const [auditBusy, setAuditBusy] = useState(false);
  const [auditError, setAuditError] = useState<ApiError | null>(null);
  const [auditCheck, setAuditCheck] = useState<AuditChainStatus | null>(null);
  const [auditCheckBusy, setAuditCheckBusy] = useState(false);
  const [auditCheckError, setAuditCheckError] = useState<ApiError | null>(null);
  /** Uçuş nesilleri: geç yanıt yeni isteğin sonucunu ezmez. */
  const auditSeq = useRef(0);
  const auditCheckSeq = useRef(0);

  // Yedek geçmişi (kasa içi `backup_runs`). `historyRows === null` "bilinmiyor"
  // demektir; boş liste ancak **başarılı** okumadan sonra "kayıt yok" olur.
  const [historyRows, setHistoryRows] = useState<BackupRunRow[] | null>(null);
  const [historyBusy, setHistoryBusy] = useState(false);
  const [historyError, setHistoryError] = useState<ApiError | null>(null);
  const historySeq = useRef(0);

  useEffect(() => {
    void api.status().then(setStatus).catch((e) => setError(toApiError(e)));
  }, []);

  const unlocked = status?.unlocked ?? false;

  // Kasa açılınca güncelleme öncesi yedekler taranır. Liste çekirdekten gelir;
  // arayüz kendi başına "yedek var" varsaymaz.
  useEffect(() => {
    if (!unlocked) {
      setPreUpdate([]);
      setPreUpdateLoaded(false);
      setPreUpdateSelected("");
      setRollbackConfirm("");
      return;
    }
    let cancelled = false;
    void api
      .vaultPreUpdateBackups()
      .then((list) => {
        if (cancelled) return;
        setPreUpdate(list);
        setPreUpdateLoaded(true);
        setPreUpdateSelected(list.find((b) => b.usable)?.fileName ?? "");
      })
      .catch((e) => {
        if (!cancelled) setError(toApiError(e));
      });
    return () => {
      cancelled = true;
    };
  }, [unlocked]);

  // Denetim defteri kasa açılınca okunur; kilitlenince liste **ve** doğrulama
  // sonucu birlikte düşer (kilitli ekranda "zincir doğrulandı" yazmaz).
  useEffect(() => {
    if (!unlocked) {
      auditSeq.current += 1;
      auditCheckSeq.current += 1;
      setAuditRows(null);
      setAuditBusy(false);
      setAuditError(null);
      setAuditCheck(null);
      setAuditCheckBusy(false);
      setAuditCheckError(null);
      return;
    }
    void loadAudit();
  }, [unlocked]);

  // Yedek geçmişi kasa açılınca okunur; kilitlenince liste **ve** hata düşer
  // (kilitli ekranda son yedekler "güncel" gibi görünmez).
  useEffect(() => {
    if (!unlocked) {
      historySeq.current += 1;
      setHistoryRows(null);
      setHistoryBusy(false);
      setHistoryError(null);
      return;
    }
    void loadHistory();
  }, [unlocked]);

  /**
   * Otomatik yedek planı **kasadan** okunur (`settings`, kapsam `vault`).
   * Cihaz kaydındaki eski plan bir kez kasaya taşınır; taşınamazsa plan yine
   * kullanılır ama kaynak `legacy` ve hata kodu yazılır — "kasadan okundu"
   * denmez. Kilitliyken okuma yapılmaz.
   */
  useEffect(() => {
    if (!unlocked) {
      scheduleSeq.current += 1;
      setScheduleSource(null);
      setScheduleError(null);
      return;
    }
    const seq = ++scheduleSeq.current;
    void loadBackupSchedule()
      .then((load) => {
        if (seq !== scheduleSeq.current) return;
        applySchedule(load);
      })
      .catch((e) => {
        // Okuma yolu kendi hatalarını taşır; buraya düşen beklenmedik bir
        // hatadır ve "plan yok" diye yazılmaz.
        if (seq !== scheduleSeq.current) return;
        setScheduleError(toApiError(e).code);
        setScheduleSource("none");
      });
  }, [unlocked]);

  function applySchedule(load: BackupScheduleLoad) {
    setScheduleSource(load.source);
    setScheduleError(load.error);
    setAutoBackup((prev) => ({
      ...prev,
      dir: load.schedule?.dir ?? "",
      days: load.schedule?.days ?? DEFAULT_BACKUP_DAYS,
    }));
  }

  /**
   * Denetim defteri okuması. Liste **istekten önce** temizlenir: düşen bir
   * tazelemeden sonra eski satırlar "güncel" gibi kalmaz. Hata hâlinde liste
   * `null`a döner — ekran "kayıt yok" **demez**, "bilinmiyor" der.
   */
  async function loadAudit() {
    const seq = ++auditSeq.current;
    setAuditBusy(true);
    setAuditError(null);
    setAuditRows([]);
    try {
      const rows = await api.auditEventList(AUDIT_LIST_LIMIT);
      if (seq === auditSeq.current) setAuditRows(rows);
    } catch (e) {
      if (seq === auditSeq.current) {
        setAuditRows(null);
        setAuditError(toApiError(e));
      }
    } finally {
      if (seq === auditSeq.current) setAuditBusy(false);
    }
  }

  /**
   * Yedek geçmişi okuması. Liste **istekten önce** temizlenir: düşen bir
   * tazelemeden sonra eski satırlar "güncel" gibi kalmaz. Hata hâlinde liste
   * `null`a döner — ekran "kayıt yok" **demez**, okunamadığını yazar.
   */
  async function loadHistory() {
    const seq = ++historySeq.current;
    setHistoryBusy(true);
    setHistoryError(null);
    setHistoryRows([]);
    try {
      const rows = await api.backupRunList(BACKUP_HISTORY_LIMIT);
      if (seq === historySeq.current) setHistoryRows(rows);
    } catch (e) {
      if (seq === historySeq.current) {
        setHistoryRows(null);
        setHistoryError(toApiError(e));
      }
    } finally {
      if (seq === historySeq.current) setHistoryBusy(false);
    }
  }

  /**
   * Zincir doğrulaması. Sonuç yalnız **en son** isteğin neslinde yazılır;
   * düşen doğrulama önceki "tutarlı" satırını ekranda bırakmaz.
   */
  async function checkAuditChain() {
    const seq = ++auditCheckSeq.current;
    setAuditCheckBusy(true);
    setAuditCheckError(null);
    setAuditCheck(null);
    try {
      const result = await api.auditChainCheck();
      if (seq === auditCheckSeq.current) setAuditCheck(result);
    } catch (e) {
      if (seq === auditCheckSeq.current) setAuditCheckError(toApiError(e));
    } finally {
      if (seq === auditCheckSeq.current) setAuditCheckBusy(false);
    }
  }

  /**
   * Klasör seçici. Pencere açılamazsa sessiz kalmaz: seçim yapılmadığı
   * ekranda görünür (E_DIALOG) — düğmeye basıp hiçbir şey olmaması yalan olurdu.
   */
  async function pickDirectory(title: string): Promise<string | null> {
    try {
      const picked = await openDirectory({ directory: true, multiple: false, title });
      return typeof picked === "string" ? picked : null;
    } catch {
      setError({ code: "E_DIALOG", message: t("settings.pickDirFailed"), retryable: true });
      return null;
    }
  }

  /**
   * Yedek seçimi **onay metnine bağlıdır**: onay, görünen hedef için yazılmış
   * bir irade beyanıdır. Seçim değişince metin silinir — aksi hâlde bir yedek
   * için yazılan "GERİ DÖN" başka bir (belki de yanlış sürümlü) yedeği geri
   * koymaya yetki verirdi.
   */
  function selectBackup(fileName: string) {
    setPreUpdateSelected(fileName);
    setRollbackDone(null);
    setRollbackConfirm("");
  }

  async function refreshPreUpdate() {
    setBusy(true);
    setError(null);
    try {
      const list = await api.vaultPreUpdateBackups();
      setPreUpdate(list);
      setPreUpdateLoaded(true);
      selectBackup(list.find((b) => b.usable)?.fileName ?? "");
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function runRollback() {
    const blocked = rollbackDisabledReason(preUpdate, preUpdateSelected, rollbackConfirm);
    if (blocked) {
      setError({ code: "E_ROLLBACK_CONFIRM", message: blocked, retryable: false });
      return;
    }
    setBusy(true);
    setError(null);
    setRollbackDone(null);
    try {
      const result = await api.vaultRollbackPreUpdate(preUpdateSelected, rollbackConfirm);
      setRollbackDone(result);
      setRollbackConfirm("");
      // Tazeleme ayrı kapıda: geri dönüş **gerçekleşti**, panel korunur.
      try {
        setStatus(await api.status());
        const list = await api.vaultPreUpdateBackups();
        setPreUpdate(list);
        setPreUpdateSelected(list.find((b) => b.usable)?.fileName ?? "");
      } catch (e) {
        // Fail-closed: liste artık **bilinmiyor** (kullanılan yedek diskte
        // olmayabilir). Eski satırları seçilebilir bırakmak, ikinci bir geri
        // dönüşü var olmayan/yanlış bir ada göndermek olurdu.
        setPreUpdate([]);
        setPreUpdateSelected("");
        setRollbackConfirm("");
        setError(toApiError(e));
      }
    } catch (e) {
      // Hata mesajı çekirdekten gelir: geri dönüş başlamadıysa kasa açık kalır,
      // E_DB_ROLLBACK_REOPEN ise hangi sürümün geri konduğunu yazar.
      setError(toApiError(e));
      setStatus(await api.status().catch(() => null));
    } finally {
      setBusy(false);
    }
  }

  async function runBackup() {
    const picked = await pickDirectory(t("settings.pickDirTitle"));
    if (picked === null) return;
    // Yeni deneme başlarken önceki sonucun künyesi silinir: başarısız yedekten
    // sonra "Yedek hazır: <eski yol>" satırı kırmızı hatayla birlikte kalmamalı.
    setBackup(null);
    setBusy(true);
    setError(null);
    try {
      setBackup(await api.vaultBackup(picked));
      // Yedek kasaya yazıldıysa geçmiş listesi **hemen** tazelenir; kayıt
      // yazılamadıysa sonuç kartı bunu ayrıca uyarır.
      void loadHistory();
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function changePassword() {
    // Önceki başarı bildirimi yeni denemeden önce silinir: eşleşmeyen parola
    // erken dönüşle çıkıyor, yeşil "değiştirildi" satırı kırmızı hatayla
    // birlikte ekranda kalmamalı.
    setPasswordChanged(false);
    if (newPassword !== newPassword2) {
      setError({ code: "E_PASSWORD_MISMATCH", message: t("settings.passwordMismatch"), retryable: false });
      return;
    }
    const submitted = { old: oldPassword, next: newPassword, next2: newPassword2 };
    setBusy(true);
    setError(null);
    try {
      await api.vaultChangePassword(submitted.old, submitted.next);
      setPasswordChanged(true);
      // Alanlar yalnız **gönderilen değer** hâlâ oradaysa temizlenir: uçuş
      // sürerken yazılan yeni parola sessizce silinmemeli (kullanıcı yeni bir
      // deneme yazıyor olabilir).
      setOldPassword((value) => (value === submitted.old ? "" : value));
      setNewPassword((value) => (value === submitted.next ? "" : value));
      setNewPassword2((value) => (value === submitted.next2 ? "" : value));
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function runGc(dryRun: boolean) {
    setBusy(true);
    setError(null);
    // Düşen işlem önceki tarama/silme bildirimini bırakmamalı: "2 aday" satırı
    // kırmızı hatayla birlikte güncel sonuç gibi okunuyordu.
    setGc(null);
    try {
      setGc(await api.vaultGc(dryRun));
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  // Sıradaki plan: son **başarılı** yedekten hesaplanır (başarısız deneme
  // tazeliği ilerletmez). Klasör yoksa plan yok; süre dolduysa saatlik kontrol
  // denenecek demektir.
  function nextPlanText(): string {
    if (!autoBackup.dir) return t("settings.autoNextNone");
    const nextAt = nextBackupAt(autoBackup.last, autoBackup.days);
    if (nextAt === null || Date.now() >= nextAt) return t("settings.autoNextDue");
    return formatDateTime(new Date(nextAt).toISOString());
  }

  async function runRestore() {
    const picked = await pickDirectory(t("settings.pickDirTitle"));
    if (picked === null) {
      return;
    }
    setRestoreDir(picked);
    // Yeni hedef seçildi: önceki geri yüklemenin "tamamlandı" paneli artık bu
    // denemenin sonucu değil; düşen denemede ekranda kalmamalı.
    setRestored(null);
    setRestoreInfo(null);
    if (!destDir.trim()) {
      setError({ code: "E_PATH", message: t("settings.restoreDestRequired"), retryable: false });
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const result = await api.vaultRestore(picked, destDir.trim());
      setRestored(result.vaultPath);
      setRestoreInfo({
        verifiedFiles: result.verifiedFiles,
        verifiedBytes: result.verifiedBytes,
        verifyMs: result.verifyMs,
        unlistedBlobs: result.unlistedBlobs,
      });
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page" style={{ maxWidth: 820 }}>
      <div className="page-head">
        <h1>{t("settings.title")}</h1>
        <p className="lead">{t("settings.lead")}</p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("settings.vaultTitle")}</h2>
            <p className="card-sub">
              {status?.unlocked ? t("settings.vaultOpenShort") : t("settings.vaultLockedShort")}
            </p>
          </div>
          <span className={`badge ${status?.unlocked ? "ok" : "warn"}`}>
            {status?.unlocked ? t("settings.vaultOpenBadge") : t("settings.vaultLockedBadge")}
          </span>
        </div>
        <dl className="kv">
          <dt>{t("settings.vaultId")}</dt><dd>{status?.vaultId ?? "—"}</dd>
          <dt>{t("settings.vaultPath")}</dt>
          <dd style={{ fontFamily: "var(--mono)", fontSize: 12 }}>{status?.vaultPath ?? "—"}</dd>
          <dt>{t("settings.schemaVersion")}</dt><dd>{status?.schemaVersion ?? t("common.unknown")}</dd>
          <dt>{t("settings.appVersion")}</dt>
          <dd>{t("settings.versionValue", { version: appVersion })}</dd>
        </dl>
        {status?.preUpdateBackup ? (
          <div className="notice ok">
            <strong>{t("settings.preUpdateTitle")}</strong>{" "}
            {t("settings.preUpdateLead")}{" "}
            <span style={{ fontFamily: "var(--mono)", fontSize: 12 }}>{status.preUpdateBackup}</span>
            . {t("settings.preUpdateTail")}
          </div>
        ) : null}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("settings.auditTitle")}</h2>
            <p className="card-sub">{t("settings.auditSub")}</p>
          </div>
          <div className="row" style={{ gap: 8 }}>
            <button
              className="btn ghost"
              disabled={!unlocked || auditBusy}
              onClick={() => void loadAudit()}
            >
              {auditBusy ? t("settings.auditLoading") : t("settings.auditRefresh")}
            </button>
            <button
              className="btn"
              disabled={!unlocked || auditCheckBusy}
              onClick={() => void checkAuditChain()}
            >
              {auditCheckBusy ? t("settings.auditVerifying") : t("settings.auditVerify")}
            </button>
          </div>
        </div>
        {!unlocked ? (
          <p className="hint" style={{ margin: 0 }}>{t("settings.auditLocked")}</p>
        ) : null}
        {unlocked && auditCheckError ? (
          <div className="notice danger" role="alert">
            <strong>{t("settings.auditVerifyFailed")}</strong>{" "}
            <code>{auditCheckError.code}</code> — {auditCheckError.message}
          </div>
        ) : null}
        {unlocked && auditCheck ? (
          auditCheck.intact ? (
            <div className="notice ok" role="status">
              <strong>{t("settings.auditIntact")}</strong>{" "}
              {t("settings.auditIntactCount", { count: auditCheck.checked })}
            </div>
          ) : (
            <div className="notice danger" role="alert">
              <strong>{t("settings.auditBroken")}</strong>{" "}
              {t("settings.auditBrokenLead", { count: auditCheck.checked })}{" "}
              <code title={auditCheck.brokenEventId ?? undefined}>
                {auditCheck.brokenEventId ?? t("common.unknown")}
              </code>{" "}
              {auditCheck.brokenReason === "prior_hash"
                ? t("settings.auditBrokenPrior")
                : auditCheck.brokenReason === "event_hash"
                  ? t("settings.auditBrokenHash")
                  : t("settings.auditBrokenUnknown", {
                      reason: auditCheck.brokenReason ?? t("common.unknown"),
                    })}
            </div>
          )
        ) : null}
        {unlocked && auditError ? (
          <div className="notice danger" role="alert">
            <strong>{auditError.code}</strong> — {auditError.message}
          </div>
        ) : null}
        {unlocked && !auditError && auditRows === null ? (
          <p className="hint" style={{ margin: 0 }}>{t("settings.auditLoading")}</p>
        ) : null}
        {unlocked && !auditError && auditRows !== null && auditRows.length === 0 ? (
          <p className="hint" style={{ margin: 0 }}>
            {auditBusy ? t("settings.auditLoading") : t("settings.auditEmpty")}
          </p>
        ) : null}
        {unlocked && auditRows !== null && auditRows.length > 0 ? (
          <table className="table">
            <thead>
              <tr>
                <th>{t("settings.auditColTime")}</th>
                <th>{t("settings.auditColCategory")}</th>
                <th>{t("settings.auditColOperation")}</th>
                <th>{t("settings.auditColActor")}</th>
                <th>{t("settings.auditColEntity")}</th>
                <th>{t("settings.auditColHash")}</th>
              </tr>
            </thead>
            <tbody>
              {auditRows.map((row) => (
                <tr key={row.id}>
                  <td>{formatDateTime(row.timestamp)}</td>
                  <td>
                    <code>{row.category}</code>
                  </td>
                  <td>
                    <code>{row.operation}</code>
                  </td>
                  <td>{row.actorLabel}</td>
                  <td title={row.entityId ?? undefined}>
                    {row.entityId ? shortId(row.entityId) : "—"}
                  </td>
                  <td>
                    <code title={row.eventHash}>{`${row.eventHash.slice(0, 10)}…`}</code>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : null}
        {unlocked ? (
          <p className="hint" style={{ margin: 0 }}>
            {t("settings.auditLimitNote", { count: AUDIT_LIST_LIMIT })}
          </p>
        ) : null}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("settings.rollbackTitle")}</h2>
            <p className="card-sub">{t("settings.rollbackSub")}</p>
          </div>
          <button
            className="btn"
            disabled={busy || !unlocked}
            onClick={() => void refreshPreUpdate()}
          >
            {t("settings.scanBackups")}
          </button>
        </div>
        {!unlocked ? (
          <p className="hint" style={{ margin: 0 }}>
            {t("settings.rollbackLocked")}
          </p>
        ) : null}
        {unlocked && preUpdateLoaded && preUpdate.length === 0 ? (
          <p className="hint" style={{ margin: 0 }}>
            {t("settings.rollbackEmpty")}
          </p>
        ) : null}
        {preUpdate.map((item) => (
          <label
            key={item.fileName}
            className="row"
            style={{ alignItems: "flex-start", gap: 8, marginBottom: 6 }}
          >
            <input
              type="radio"
              name="pre-update-backup"
              checked={preUpdateSelected === item.fileName}
              disabled={!item.usable || busy}
              onChange={() => selectBackup(item.fileName)}
            />
            <span style={{ flex: 1 }}>
              <strong>{backupVersionLabel(item)}</strong>{" "}
              <span className={`badge ${item.usable ? "ok" : "danger"}`}>
                {item.usable ? t("rollback.usable") : t("rollback.unusable")}
              </span>
              <br />
              <span style={{ fontFamily: "var(--mono)", fontSize: 12 }}>{item.fileName}</span>
              <br />
              <span className="hint">
                {formatBackupDate(item.modifiedAt)} · {formatSize(item.sizeBytes)} ·{" "}
                {backupStatusText(item)}
              </span>
              {backupWarnings(item).map((warning) => (
                <span key={warning} className="hint error" style={{ display: "block" }}>
                  {warning}
                </span>
              ))}
            </span>
          </label>
        ))}
        {preUpdate.length > 0 ? (
          <>
            <ul className="hint" style={{ margin: "8px 0 0 18px", padding: 0 }}>
              {rollbackConsequences().map((line) => (
                <li key={line}>{line}</li>
              ))}
            </ul>
            <div className="field" style={{ marginTop: 10 }}>
              <label htmlFor="rollback-confirm">
                {t("settings.rollbackConfirmLabel", { token: t("rollback.confirmToken") })}
              </label>
              <input
                id="rollback-confirm"
                className="input"
                value={rollbackConfirm}
                autoComplete="off"
                disabled={busy}
                onChange={(e) => setRollbackConfirm(e.target.value)}
              />
            </div>
            <div className="row" style={{ justifyContent: "flex-end" }}>
              <button
                className="btn"
                disabled={
                  busy ||
                  rollbackDisabledReason(preUpdate, preUpdateSelected, rollbackConfirm) !== null
                }
                onClick={() => void runRollback()}
              >
                {t("settings.rollbackSubmit")}
              </button>
            </div>
            {rollbackDisabledReason(preUpdate, preUpdateSelected, rollbackConfirm) ? (
              <p className="hint" style={{ margin: 0 }}>
                {rollbackDisabledReason(preUpdate, preUpdateSelected, rollbackConfirm)}
              </p>
            ) : null}
          </>
        ) : null}
        {/*
          Sonuç paneli listenin dışında: geri dönüş sonrası tazeleme, kullanılan
          yedeği listeden düşürünce (liste boşalınca) işlemin **gerçekleştiği**
          bilgisi kaybolmamalı. Kullanıcı "tamamlandı" panelini görmeli.
        */}
        {rollbackDone ? (
          <div className="notice ok">
            <strong>{t("settings.rollbackDoneTitle")}</strong> {rollbackSummary(rollbackDone)}
            <br />
            {t("settings.rollbackDoneBackup")}{" "}
            <span style={{ fontFamily: "var(--mono)", fontSize: 12 }}>
              {rollbackDone.backupFile}
            </span>
            <br />
            {t("settings.rollbackDoneSafety")}{" "}
            <span style={{ fontFamily: "var(--mono)", fontSize: 12 }}>
              {rollbackDone.safetyCopy}
            </span>
            <br />
            {t("settings.rollbackDoneHint")}
          </div>
        ) : null}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("settings.backupTitle")}</h2>
            <p className="card-sub">{t("settings.backupSub")}</p>
          </div>
          <button className="btn primary" disabled={busy || !status?.unlocked} onClick={() => void runBackup()}>
            {t("settings.backupPick")}
          </button>
        </div>
        {backup ? (
          <div className="notice ok">
            {t("settings.backupReady")}{" "}
            <span style={{ fontFamily: "var(--mono)", fontSize: 12 }}>{backup.path}</span>
            <br />
            {t("settings.backupSummary", {
              count: backup.fileCount,
              size: formatSize(backup.totalBytes),
              hash: backup.manifestHash.slice(0, 16),
            })}
            {backup.archivePath ? (
              <>
                <br />
                {t("settings.backupArchive")} <code>{backup.archivePath}</code>
              </>
            ) : null}
            {backup.historyRecorded ? null : (
              <p className="notice danger" role="alert" style={{ marginTop: 8 }}>
                {t("settings.backupHistoryNotRecorded", {
                  code: backup.historyError ?? t("common.unknown"),
                })}
              </p>
            )}
          </div>
        ) : (
          <p className="hint" style={{ margin: 0 }}>
            {status?.unlocked ? t("settings.backupNone") : t("settings.backupLocked")}
          </p>
        )}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("settings.backupHistoryTitle")}</h2>
            <p className="card-sub">{t("settings.backupHistorySub")}</p>
          </div>
          <button
            className="btn ghost"
            disabled={!unlocked || historyBusy}
            onClick={() => void loadHistory()}
          >
            {historyBusy ? t("settings.backupHistoryLoading") : t("settings.backupHistoryRefresh")}
          </button>
        </div>
        {!unlocked ? (
          <p className="hint" style={{ margin: 0 }}>{t("settings.backupHistoryLocked")}</p>
        ) : null}
        {unlocked && historyError ? (
          <div className="notice danger" role="alert">
            <strong>{historyError.code}</strong> — {historyError.message}
          </div>
        ) : null}
        {unlocked && !historyError && historyRows === null ? (
          <p className="hint" style={{ margin: 0 }}>{t("settings.backupHistoryLoading")}</p>
        ) : null}
        {unlocked && historyRows !== null && historyRows.length === 0 ? (
          <p className="hint" style={{ margin: 0 }}>
            {historyBusy ? t("settings.backupHistoryLoading") : t("settings.backupHistoryEmpty")}
          </p>
        ) : null}
        {unlocked && historyRows !== null && historyRows.length > 0 ? (
          <table className="table">
            <thead>
              <tr>
                <th>{t("settings.backupHistoryColTime")}</th>
                <th>{t("settings.backupHistoryColTarget")}</th>
                <th>{t("settings.backupHistoryColManifest")}</th>
                <th>{t("settings.backupHistoryColStatus")}</th>
              </tr>
            </thead>
            <tbody>
              {historyRows.map((row) => (
                <tr key={row.id}>
                  <td>{formatDateTime(row.createdAt)}</td>
                  <td>
                    <code style={{ fontFamily: "var(--mono)", fontSize: 12 }}>
                      {row.destinationLabel}
                    </code>
                    <div className="hint" style={{ margin: 0 }}>{row.snapshotId}</div>
                  </td>
                  <td>
                    <code title={row.manifestHash}>{`${row.manifestHash.slice(0, 10)}…`}</code>
                  </td>
                  <td>
                    <span className={`badge ${backupRunStatusTone(row.status)}`}>
                      {backupRunStatusLabel(row.status)}
                    </span>
                    {row.verificationResult ? (
                      <div className="hint" style={{ margin: 0 }}>
                        <code>{row.verificationResult}</code>
                      </div>
                    ) : null}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : null}
        {unlocked ? (
          <p className="hint" style={{ margin: 0 }}>
            {t("settings.backupHistoryLimitNote", { count: BACKUP_HISTORY_LIMIT })}
          </p>
        ) : null}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("settings.autoTitle")}</h2>
            <p className="card-sub">{t("settings.autoSub")}</p>
          </div>
          <button
            className="btn"
            onClick={async () => {
              const picked = await pickDirectory(t("settings.backupPick"));
              if (picked === null) return;
              setScheduleError(null);
              try {
                // Plan **kasaya** yazılır ve geri okunarak doğrulanır; cihaz
                // kaydı yalnız doğrulanmış yazımdan sonra tazelenir.
                const saved = await saveBackupSchedule({
                  dir: picked,
                  days: DEFAULT_BACKUP_DAYS,
                });
                setScheduleSource("vault");
                setAutoBackup((prev) => ({ ...prev, dir: saved.dir, days: saved.days }));
              } catch (e) {
                const err = toApiError(e);
                setScheduleError(err.code);
                setError(err);
              }
            }}
          >
            {t("settings.autoPick")}
          </button>
        </div>
        <dl className="kv">
          <dt>{t("settings.autoDir")}</dt>
          <dd style={{ fontFamily: "var(--mono)", fontSize: 12 }}>{autoBackup.dir || t("settings.autoDirNone")}</dd>
          <dt>{t("settings.autoDays")}</dt>
          <dd>{t("settings.autoDaysValue", { days: autoBackup.days })}</dd>
          <dt>{t("settings.autoLast")}</dt>
          <dd>{autoBackup.last ? formatDateTime(autoBackup.last) : t("settings.autoLastNone")}</dd>
          <dt>{t("settings.autoAttempt")}</dt>
          <dd>
            {autoBackup.attempt
              ? `${formatDateTime(autoBackup.attempt.at)} · ${
                  autoBackup.attempt.ok
                    ? t("settings.autoAttemptOk")
                    : t("settings.autoAttemptFailed")
                }`
              : t("settings.autoAttemptNone")}
          </dd>
          <dt>{t("settings.autoNext")}</dt>
          <dd>{nextPlanText()}</dd>
        </dl>
        {/* Plan kaynağı/hatası **gizlenmez**: "seçilmedi" ile "okunamadı" ayrı. */}
        {scheduleError ? (
          <p className="notice danger" role="alert">
            {scheduleSource === "invalid"
              ? t("settings.autoSettingInvalid", { code: scheduleError })
              : scheduleSource === "legacy"
                ? t("settings.autoSettingLegacy", { code: scheduleError })
                : t("settings.autoSettingReadFailed", { code: scheduleError })}
          </p>
        ) : null}
        {scheduleSource === "migrated" ? (
          <p className="hint" style={{ margin: 0 }}>{t("settings.autoSettingMigrated")}</p>
        ) : null}
        {autoBackup.attempt && !autoBackup.attempt.ok ? (
          <p className="notice danger" role="alert">
            {t("settings.autoAttemptReason", {
              code: autoBackup.attempt.code ?? t("settings.autoAttemptUnknown"),
            })}
          </p>
        ) : null}
        <div className="row" style={{ gap: 6 }}>
          <button
            className="btn ghost"
            disabled={!autoBackup.dir}
            onClick={async () => {
              setScheduleError(null);
              try {
                // Plan kasadan silinir; cihaz önbelleği de temizlenir. Başarısız
                // silmede ekran "kapandı" demez.
                await clearBackupSchedule();
                setScheduleSource("none");
                setAutoBackup((prev) => ({ ...prev, dir: "", days: DEFAULT_BACKUP_DAYS }));
              } catch (e) {
                const err = toApiError(e);
                setScheduleError(err.code);
                setError(err);
              }
            }}
          >
            {t("settings.autoDisable")}
          </button>
          <button
            className="btn ghost"
            disabled={!autoBackup.last}
            onClick={() => {
              localStorage.removeItem(BACKUP_LAST_KEY);
              setAutoBackup((prev) => ({ ...prev, last: readBackupLast() }));
            }}
            title={t("settings.autoResetTitle")}
          >
            {t("settings.autoReset")}
          </button>
        </div>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("settings.gcTitle")}</h2>
            <p className="card-sub">{t("settings.gcSub")}</p>
          </div>
          <div className="row" style={{ gap: 6 }}>
            <button className="btn" disabled={busy} onClick={() => void runGc(true)}>
              {t("settings.gcScan")}
            </button>
            <button
              className="btn"
              disabled={busy || !gc || gc.candidates.length === 0}
              onClick={() => void runGc(false)}
            >
              {t("settings.gcClean")}
            </button>
          </div>
        </div>
        {gc ? (
          <div className={gc.deleted.length > 0 ? "notice ok" : "notice"}>
            {gc.dryRun
              ? t("settings.gcCandidates", {
                  count: gc.candidates.length,
                  size: formatSize(gc.candidates.reduce((sum, b) => sum + b.encryptedSize, 0)),
                })
              : t("settings.gcDeleted", {
                  count: gc.deleted.length,
                  size: formatSize(gc.freedBytes),
                })}
            {gc.missingFiles.length > 0 ? (
              <>
                <br />
                {t("settings.gcMissing", { files: gc.missingFiles.join("; ") })}
              </>
            ) : null}
          </div>
        ) : (
          <p className="hint" style={{ margin: 0 }}>
            {t("settings.gcHint")}
          </p>
        )}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("settings.restoreTitle")}</h2>
            <p className="card-sub">{t("settings.restoreSub")}</p>
          </div>
        </div>
        <div className="field">
          <label htmlFor="restore-dest">{t("settings.restoreDest")}</label>
          <input
            id="restore-dest"
            className="input"
            value={destDir}
            onChange={(e) => setDestDir(e.target.value)}
            placeholder={t("settings.restoreDestPlaceholder")}
          />
        </div>
        <div className="row" style={{ justifyContent: "flex-end" }}>
          <button className="btn" disabled={busy || !destDir.trim()} onClick={() => void runRestore()}>
            {t("settings.restorePick")}
          </button>
        </div>
        {restoreDir ? (
          <p className="hint" style={{ margin: 0 }}>
            {t("settings.restoreSelected", { dir: restoreDir })}
          </p>
        ) : null}
        {restored ? (
          <div className="notice ok">
            {t("settings.restoreDone")}{" "}
            <span style={{ fontFamily: "var(--mono)", fontSize: 12 }}>{restored}</span>
            {restoreInfo ? (
              <>
                <br />
                {t("settings.restoreVerifiedLead")} <strong>{restoreInfo.verifiedFiles}</strong>{" "}
                {t("settings.restoreVerifiedTail", {
                  size: formatSize(restoreInfo.verifiedBytes),
                  ms: restoreInfo.verifyMs,
                })}
                {restoreInfo.unlistedBlobs.length > 0 ? (
                  <>
                    {" "}
                    {t("settings.restoreUnlisted", { count: restoreInfo.unlistedBlobs.length })}{" "}
                    {restoreInfo.unlistedBlobs.slice(0, 5).join(", ")}
                    {restoreInfo.unlistedBlobs.length > 5 ? "…" : ""}
                  </>
                ) : null}
              </>
            ) : null}
            <br />
            {t("settings.restoreOpenHint")}
          </div>
        ) : null}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("settings.passwordTitle")}</h2>
            <p className="card-sub">{t("settings.passwordSub")}</p>
          </div>
        </div>
        <div className="row">
          <div className="field" style={{ flex: "1 1 200px" }}>
            <label htmlFor="pw-old">{t("settings.passwordOld")}</label>
            <input
              id="pw-old"
              className="input"
              type="password"
              autoComplete="current-password"
              value={oldPassword}
              onChange={(e) => {
                setOldPassword(e.target.value);
                // Yazılan yeni parola, önceki değişimin "değiştirildi" satırını
                // güncel sonuçmuş gibi bırakmaz.
                setPasswordChanged(false);
              }}
            />
          </div>
          <div className="field" style={{ flex: "1 1 200px" }}>
            <label htmlFor="pw-new">{t("settings.passwordNew")}</label>
            <input
              id="pw-new"
              className="input"
              type="password"
              autoComplete="new-password"
              value={newPassword}
              onChange={(e) => {
                setNewPassword(e.target.value);
                setPasswordChanged(false);
              }}
            />
          </div>
          <div className="field" style={{ flex: "1 1 200px" }}>
            <label htmlFor="pw-new2">{t("settings.passwordNew2")}</label>
            <input
              id="pw-new2"
              className="input"
              type="password"
              autoComplete="new-password"
              value={newPassword2}
              onChange={(e) => {
                setNewPassword2(e.target.value);
                setPasswordChanged(false);
              }}
            />
          </div>
        </div>
        <div className="row" style={{ justifyContent: "flex-end" }}>
          <button
            className="btn"
            disabled={busy || !oldPassword || newPassword.length < 10}
            onClick={() => void changePassword()}
          >
            {t("settings.passwordSubmit")}
          </button>
        </div>
        {passwordChanged ? (
          <div className="notice ok">{t("settings.passwordDone")}</div>
        ) : null}
      </div>

      <JobQueueCard unlocked={unlocked} />

      <WorkerBudgetCard unlocked={unlocked} />

      <DiagnosticsCard uiVersion={appVersion} />
    </div>
  );
}
