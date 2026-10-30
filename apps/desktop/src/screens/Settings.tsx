import { useEffect, useState } from "react";
import { open as openDirectory } from "@tauri-apps/plugin-dialog";
import { JobQueueCard } from "../components/JobQueueCard";
import { WorkerBudgetCard } from "../components/WorkerBudgetCard";
import {
  api,
  toApiError,
  type ApiError,
  type BackupResult,
  type PreUpdateBackup,
  type RollbackResult,
  type VaultStatus,
} from "../lib/api";
import type { VaultGcResult } from "../lib/api";
import { formatDateTime, formatSize } from "../lib/format";
import { t } from "../lib/i18n";
import { version as appVersion } from "../../package.json";
import {
  BACKUP_DAYS_KEY,
  BACKUP_DIR_KEY,
  BACKUP_LAST_KEY,
  readBackupSettings,
} from "../lib/backup-schedule";
import {
  backupStatusText,
  backupVersionLabel,
  backupWarnings,
  formatBackupDate,
  rollbackConsequences,
  rollbackDisabledReason,
  rollbackSummary,
} from "../lib/rollback";

export function Settings() {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [backup, setBackup] = useState<BackupResult | null>(null);
  const [gc, setGc] = useState<VaultGcResult | null>(null);
  const [autoBackup, setAutoBackup] = useState(() => readBackupSettings());
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

  async function refreshPreUpdate() {
    setBusy(true);
    setError(null);
    try {
      const list = await api.vaultPreUpdateBackups();
      setPreUpdate(list);
      setPreUpdateLoaded(true);
      setPreUpdateSelected(list.find((b) => b.usable)?.fileName ?? "");
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
      setStatus(await api.status());
      const list = await api.vaultPreUpdateBackups();
      setPreUpdate(list);
      setPreUpdateSelected(list.find((b) => b.usable)?.fileName ?? "");
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
    const picked = await openDirectory({ directory: true, multiple: false, title: t("settings.pickDirTitle") });
    if (typeof picked !== "string") return;
    setBusy(true);
    setError(null);
    try {
      setBackup(await api.vaultBackup(picked));
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function changePassword() {
    if (newPassword !== newPassword2) {
      setError({ code: "E_PASSWORD_MISMATCH", message: t("settings.passwordMismatch"), retryable: false });
      return;
    }
    setBusy(true);
    setError(null);
    setPasswordChanged(false);
    try {
      await api.vaultChangePassword(oldPassword, newPassword);
      setPasswordChanged(true);
      setOldPassword("");
      setNewPassword("");
      setNewPassword2("");
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function runGc(dryRun: boolean) {
    setBusy(true);
    setError(null);
    try {
      setGc(await api.vaultGc(dryRun));
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function runRestore() {
    const picked = await openDirectory({ directory: true, multiple: false, title: t("settings.pickDirTitle") });
    if (typeof picked !== "string") {
      return;
    }
    setRestoreDir(picked);
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
              onChange={() => {
                setPreUpdateSelected(item.fileName);
                setRollbackDone(null);
              }}
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
          </>
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
          </div>
        ) : (
          <p className="hint" style={{ margin: 0 }}>
            {t("settings.backupLocked")}
          </p>
        )}
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
              const picked = await openDirectory({ directory: true, multiple: false, title: t("settings.backupPick") });
              if (typeof picked !== "string") return;
              localStorage.setItem(BACKUP_DIR_KEY, picked);
              localStorage.setItem(BACKUP_DAYS_KEY, "7");
              setAutoBackup(readBackupSettings());
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
        </dl>
        <div className="row" style={{ gap: 6 }}>
          <button
            className="btn ghost"
            disabled={!autoBackup.dir}
            onClick={() => {
              localStorage.removeItem(BACKUP_DIR_KEY);
              setAutoBackup(readBackupSettings());
            }}
          >
            {t("settings.autoDisable")}
          </button>
          <button
            className="btn ghost"
            disabled={!autoBackup.last}
            onClick={() => {
              localStorage.removeItem(BACKUP_LAST_KEY);
              setAutoBackup(readBackupSettings());
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
              onChange={(e) => setOldPassword(e.target.value)}
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
              onChange={(e) => setNewPassword(e.target.value)}
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
              onChange={(e) => setNewPassword2(e.target.value)}
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

      <div className="card tight">
        <h2>{t("settings.diagnosticsTitle")}</h2>
        <p className="muted small" style={{ margin: 0 }}>
          {t("settings.diagnosticsBody")}
        </p>
      </div>
    </div>
  );
}
