import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, toApiError, type ApiError } from "../lib/api";
import { t } from "../lib/i18n";
import { checkPassword } from "../lib/vault-input";

type Mode = "open" | "create" | "recovery";

const MODE_LABELS = {
  create: "unlock.modeCreate",
  open: "unlock.modeOpen",
  recovery: "unlock.modeRecovery",
} as const;

const DEFAULT_DIR = "Dosyalar/DosyaIncelemeMasasi";

export type UnlockProps = {
  onUnlocked: () => Promise<void>;
  /**
   * Bir kez gösterilecek kurtarma anahtarı. Sahibi `App`: kasa oluşturulduktan
   * sonra başka ekrana gidip dönmek bileşeni yeniden bağlar; anahtar burada
   * tutulsaydı ilk gezinmede yok olurdu (tek kopya, bir daha gösterilmez).
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
  const [keySaved, setKeySaved] = useState(false);
  const [confirmLeave, setConfirmLeave] = useState(false);

  // Yeni bir anahtar ekranı eski onayı devralmaz: "kaydettim" işareti ve
  // "kaydetmeden devam" onayı anahtar ekranıyla birlikte sıfırlanır.
  useEffect(() => {
    if (!pendingKey) {
      setKeySaved(false);
      setConfirmLeave(false);
    }
  }, [pendingKey]);

  const passwordCheck = checkPassword(password);
  const mismatch = mode === "create" && password !== confirm;
  const showMismatch = mismatch && confirm.length > 0;
  const canSubmit =
    !busy && !locking && path.trim().length > 0 && (mode === "recovery" ? recovery.trim().length > 0 : passwordCheck.ok && !mismatch);

  function changeMode(next: Mode) {
    setMode(next);
    // Kip değişince önceki kipin hatası ekranda kalmaz (çelişkili ekran yok).
    setError(null);
  }

  async function chooseDirectory() {
    setBusy(true);
    setError(null);
    try {
      const directory = await open({ directory: true, multiple: false, title: t("unlock.chooseDirectory") });
      if (directory) setPath(directory);
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
        const result = await api.createVault(path.trim(), password);
        setPassword("");
        setConfirm("");
        onKeyCreated(result.recoveryKey);
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
    setBusy(true);
    try {
      await onUnlocked();
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

        <div className="card">
          <div className="recovery" aria-label={t("unlock.recoveryAria")}>{pendingKey}</div>
          <div className="hint">{t("unlock.recoveryHint")}</div>
          <label className="row" style={{ alignItems: "center", gap: 8 }}>
            <input
              type="checkbox"
              checked={keySaved}
              onChange={(e) => setKeySaved(e.target.checked)}
              style={{ width: 16, height: 16 }}
            />
            <span>{t("unlock.savedCheck")}</span>
          </label>
          {confirmLeave ? <div className="hint error">{t("unlock.leaveWarning")}</div> : null}
          <div className="row" style={{ justifyContent: "flex-end" }}>
            {confirmLeave ? (
              <>
                <button className="btn ghost" disabled={locking || busy} onClick={() => setConfirmLeave(false)}>
                  {t("unlock.leaveCancel")}
                </button>
                <button
                  className="btn"
                  disabled={locking || busy}
                  onClick={async () => {
                    if (await onKeyDismissed()) {
                      setConfirmLeave(false);
                      setMode("open");
                    }
                  }}
                >
                  {t("unlock.leaveConfirm")}
                </button>
              </>
            ) : (
              <button className="btn ghost" onClick={() => setConfirmLeave(true)}>
                {t("common.back")}
              </button>
            )}
            <button className="btn primary" disabled={!keySaved || busy || locking} onClick={() => void openCreated()}>
              {busy ? t("common.busy") : t("unlock.openVault")}
            </button>
          </div>
        </div>
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
              <button type="button" className="btn" onClick={() => void chooseDirectory()} disabled={busy}>
                {t("unlock.chooseDirectory")}
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
                  <span id="password-hint" className="hint">{t("unlock.passwordHint")}</span>
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
