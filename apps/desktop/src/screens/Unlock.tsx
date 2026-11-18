import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, toApiError, type ApiError, type VaultInfo } from "../lib/api";
import { t } from "../lib/i18n";
import { checkPassword, normalizeRecoveryInput } from "../lib/vault-input";

type Mode = "open" | "create" | "recovery";

const MODE_LABELS = {
  create: "unlock.modeCreate",
  open: "unlock.modeOpen",
  recovery: "unlock.modeRecovery",
} as const;

const DEFAULT_DIR = "Dosyalar/DosyaIncelemeMasasi";

export type UnlockProps = {
  onUnlocked: (confirmed?: VaultInfo) => Promise<void>;
  /**
   * Kurulum sırasında gezinip dönüldüğünde aynı anahtarı göstermek için App tutar.
   */
  pendingKey?: string | null;
  /** Zorunlu: anahtarın nereye yazılacağına çağıran karar verir (sessiz kayıp yok). */
  onKeyCreated: (recoveryKey: string) => void;
  onKeyDismissed: () => Promise<boolean>;
  locking?: boolean;
};

export function Unlock({
  onUnlocked,
  pendingKey = null,
  locking = false,
  onKeyCreated,
  onKeyDismissed,
}: UnlockProps) {
  const [mode, setMode] = useState<Mode>("create");
  const [path, setPath] = useState(DEFAULT_DIR);
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [recovery, setRecovery] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);
  const [savedKey, setSavedKey] = useState("");
  const [confirmLeave, setConfirmLeave] = useState(false);
  const keyMatches = !!pendingKey && normalizeRecoveryInput(savedKey) === normalizeRecoveryInput(pendingKey);

  const passwordCheck = checkPassword(password);
  const mismatch = mode === "create" && password !== confirm;
  const showMismatch = mismatch && confirm.length > 0;
  const validCredentials = mode === "create" ? passwordCheck.ok && !mismatch
    : mode === "open" ? password.length > 0 : recovery.trim().length > 0;
  const canSubmit = !busy && !locking && path.trim().length > 0 && validCredentials;

  function changeMode(next: Mode) {
    if (next === mode) return;
    setMode(next);
    setPassword("");
    setConfirm("");
    setRecovery("");
    // Kip değişince önceki kipin hatası ekranda kalmaz (çelişkili ekran yok).
    setError(null);
  }

  async function choosePath() {
    setBusy(true);
    setError(null);
    try {
      const selected = mode === "create"
        ? await open({ directory: true, multiple: false, title: t("unlock.chooseDirectory") })
        : await open({
            directory: false,
            multiple: false,
            title: t("unlock.chooseFile"),
            filters: [{ name: t("unlock.vaultFile"), extensions: ["dimvault"] }],
          });
      if (selected) setPath(selected);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function submit() {
    if (!canSubmit) return;
    setError(null);
    setBusy(true);
    try {
      if (mode === "create") {
        const key = await api.prepareVault(path.trim(), password);
        setPassword("");
        setConfirm("");
        setSavedKey("");
        setConfirmLeave(false);
        onKeyCreated(key);
      } else if (mode === "open") {
        await api.openVault(path.trim(), password);
        setPassword("");
        await onUnlocked();
      } else {
        await api.openWithRecovery(path.trim(), recovery.trim());
        setRecovery("");
        await onUnlocked();
      }
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function openCreated() {
    if (!keyMatches || busy || locking) return;
    setBusy(true);
    setError(null);
    try {
      const info = await api.confirmVault(savedKey);
      await onUnlocked(info);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  if (pendingKey) {
    return (
      <div className="page" style={{ maxWidth: 720 }}>
        <div className="page-head">
          <div className="steps" aria-label={t("steps.aria")}>
            <span className="step done"><span className="step-dot">✓</span> {t("steps.vault")}</span>
            <span className="step active"><span className="step-dot">2</span> {t("steps.recovery")}</span>
            <span className="step"><span className="step-dot">3</span> {t("steps.start")}</span>
          </div>
          <h1>{t("unlock.recoveryTitle")}</h1>
          <p className="lead">{t("unlock.recoveryLead")}</p>
        </div>

        <form className="card" onSubmit={(event) => {
          event.preventDefault();
          void openCreated();
        }}>
          <div className="recovery" aria-label={t("unlock.recoveryAria")}>{pendingKey}</div>
          <div className="hint">{t("unlock.recoveryHint")}</div>
          <div className="field">
            <label htmlFor="saved-recovery">{t("unlock.confirmRecovery")}</label>
            <input id="saved-recovery" className="input" type="password" autoComplete="off"
              spellCheck={false} value={savedKey} disabled={busy || locking}
              onChange={(event) => setSavedKey(event.target.value)} />
          </div>
          {error ? <div className="notice danger" role="alert">{error.message}</div> : null}
          {confirmLeave ? <div className="hint error">{t("unlock.leaveWarning")}</div> : null}
          <div className="row" style={{ justifyContent: "flex-end" }}>
            {confirmLeave ? (
              <>
                <button type="button" className="btn ghost" disabled={locking || busy} onClick={() => setConfirmLeave(false)}>
                  {t("unlock.leaveCancel")}
                </button>
                <button
                  type="button"
                  className="btn"
                  disabled={locking || busy}
                  onClick={async () => {
                    if (await onKeyDismissed()) {
                      setConfirmLeave(false);
                      setSavedKey("");
                      setPath(DEFAULT_DIR);
                      setMode("create");
                    }
                  }}
                >
                  {t("unlock.leaveConfirm")}
                </button>
              </>
            ) : (
              <button type="button" className="btn ghost" onClick={() => setConfirmLeave(true)}>
                {t("common.back")}
              </button>
            )}
            <button type="submit" className="btn primary" disabled={!keyMatches || busy || locking}>
              {busy ? t("common.busy") : t("unlock.openVault")}
            </button>
          </div>
        </form>
        <div className="notice">{t("unlock.recoveryNotice")}</div>
      </div>
    );
  }

  return (
    <div className="page vault-page">
      <div className="page-head">
        <h1>{t("unlock.title")}</h1>
        <p className="lead">{t("unlock.lead")}</p>
      </div>

      <div className="vault-layout">
        <form
          className="card vault-form"
          onSubmit={(event) => {
            event.preventDefault();
            void submit();
          }}
        >
          <div className="mode-switch" role="group" aria-label={t("unlock.modesAria")}>
            {(["create", "open", "recovery"] as const).map((item) => (
              <button
                type="button"
                key={item}
                aria-pressed={mode === item}
                disabled={busy}
                onClick={() => changeMode(item)}
              >
                {t(MODE_LABELS[item])}
              </button>
            ))}
          </div>

          <div className="field">
            <label htmlFor="vault-path">{t("unlock.pathLabel")}</label>
            <div className="path-control">
              <input
                id="vault-path"
                aria-describedby="path-hint"
                className="input"
                value={path}
                onChange={(e) => setPath(e.target.value)}
                placeholder={DEFAULT_DIR}
                autoComplete="off"
                spellCheck={false}
                disabled={busy}
              />
              <button type="button" className="btn" onClick={() => void choosePath()} disabled={busy}>
                {t(mode === "create" ? "unlock.chooseDirectory" : "unlock.chooseFile")}
              </button>
            </div>
            <span id="path-hint" className="hint">{t(mode === "create" ? "unlock.pathHint" : "unlock.existingPathHint")}</span>
          </div>

          {mode === "recovery" ? (
            <div className="field">
              <label htmlFor="recovery-key">{t("unlock.recoveryKeyLabel")}</label>
              <input
                id="recovery-key"
                type="password"
                aria-describedby="recovery-hint"
                className="input"
                value={recovery}
                onChange={(e) => setRecovery(e.target.value)}
                placeholder="XXXX-XXXX-XXXX-..."
                autoComplete="off"
                spellCheck={false}
                style={{ fontFamily: "var(--mono)" }}
                disabled={busy}
              />
              <span className="hint" id="recovery-hint">{t("unlock.recoveryKeyHint")}</span>
            </div>
          ) : (
            <>
              <div className="field">
                <label htmlFor="password">
                  {mode === "create" ? t("unlock.passwordCreate") : t("unlock.password")}
                </label>
                <input
                  id="password"
                  aria-describedby="password-hint"
                  className="input"
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  autoComplete={mode === "create" ? "new-password" : "current-password"}
                  disabled={busy}
                />
                {mode === "create" && password.length > 0 && !passwordCheck.ok ? (
                  <span id="password-hint" className="hint error">{passwordCheck.reason}</span>
                ) : (
                  <span id="password-hint" className="hint">{t(mode === "create" ? "unlock.passwordHint" : "unlock.existingPasswordHint")}</span>
                )}
              </div>
              {mode === "create" ? (
                <div className="field">
                  <label htmlFor="confirm">{t("unlock.passwordConfirm")}</label>
                  <input
                    id="confirm"
                    aria-invalid={showMismatch || undefined}
                    aria-describedby={showMismatch ? "confirm-hint" : undefined}
                    className="input"
                    type="password"
                    value={confirm}
                    onChange={(e) => setConfirm(e.target.value)}
                    autoComplete="new-password"
                    disabled={busy}
                  />
                  {showMismatch ? <span id="confirm-hint" className="hint error">{t("vaultInput.mismatch")}</span> : null}
                </div>
              ) : null}
            </>
          )}

          {error ? (
            <div className="notice danger" role="alert">
              <strong>{error.code}</strong> — {error.message}
              {error.retryable ? ` ${t("common.retryableSuffix")}` : ""}
            </div>
          ) : null}

          <div className="form-actions">
            <button type="submit" className="btn primary" disabled={!canSubmit}>
              {busy
                ? t("common.busy")
                : mode === "create"
                  ? t("unlock.submitCreate")
                  : mode === "open"
                    ? t("unlock.submitOpen")
                    : t("unlock.submitRecovery")}
            </button>
          </div>
        </form>
        <aside className="vault-guide" aria-labelledby="vault-guide-heading">
          <div className="guide-art" aria-hidden="true">
            <div className="archive-folder"><span /></div>
          </div>
          <div className="guide-body">
            <h2 id="vault-guide-heading">{t("unlock.phaseTitle")}</h2>
            <dl>
              <dt>{t("unlock.phaseVault")}</dt>
              <dd>{t("unlock.phaseVaultValue")}</dd>
              <dt>{t("unlock.phaseRecovery")}</dt>
              <dd>{t("unlock.phaseRecoveryValue")}</dd>
              <dt>{t("unlock.phaseImport")}</dt>
              <dd>{t("unlock.phaseImportValue")}</dd>
            </dl>
          </div>
        </aside>
      </div>
    </div>
  );
}
