import { useEffect, useState } from "react";
import { api, toApiError, type ApiError } from "../lib/api";
import { t } from "../lib/i18n";
import { checkPassword, checkPasswordConfirmation } from "../lib/vault-input";

type Mode = "open" | "create" | "recovery";

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
  onKeyDismissed: () => void;
};

export function Unlock({
  onUnlocked,
  pendingKey = null,
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

  const check = mode === "create" ? checkPasswordConfirmation(password, confirm) : checkPassword(password);
  const canSubmit =
    !busy && (mode === "recovery" ? recovery.trim().length > 0 : check.ok && path.trim().length > 0);

  function changeMode(next: Mode) {
    setMode(next);
    // Kip değişince önceki kipin hatası ekranda kalmaz (çelişkili ekran yok).
    setError(null);
  }

  async function submit() {
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
                <button className="btn ghost" onClick={() => setConfirmLeave(false)}>
                  {t("unlock.leaveCancel")}
                </button>
                <button
                  className="btn"
                  onClick={() => {
                    setConfirmLeave(false);
                    setMode("open");
                    onKeyDismissed();
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
            <button className="btn primary" disabled={!keySaved || busy} onClick={() => void openCreated()}>
              {busy ? t("common.busy") : t("unlock.openVault")}
            </button>
          </div>
        </div>
        <div className="notice">{t("unlock.recoveryNotice")}</div>
      </div>
    );
  }

  return (
    <div className="page" style={{ maxWidth: 620 }}>
      <div className="page-head">
        <h1>{t("unlock.title")}</h1>
        <p className="lead">{t("unlock.lead")}</p>
      </div>

      <div className="card">
        <div className="steps" aria-label={t("unlock.modesAria")}>
          <button
            className={mode === "create" ? "btn primary" : "btn"}
            aria-pressed={mode === "create"}
            disabled={busy}
            onClick={() => changeMode("create")}
          >
            {t("unlock.modeCreate")}
          </button>
          <button
            className={mode === "open" ? "btn primary" : "btn"}
            aria-pressed={mode === "open"}
            disabled={busy}
            onClick={() => changeMode("open")}
          >
            {t("unlock.modeOpen")}
          </button>
          <button
            className={mode === "recovery" ? "btn primary" : "btn"}
            aria-pressed={mode === "recovery"}
            disabled={busy}
            onClick={() => changeMode("recovery")}
          >
            {t("unlock.modeRecovery")}
          </button>
        </div>

        <div className="field">
          <label htmlFor="vault-path">{t("unlock.pathLabel")}</label>
          <input
            id="vault-path"
            className="input"
            value={path}
            onChange={(e) => setPath(e.target.value)}
            placeholder={DEFAULT_DIR}
            autoComplete="off"
            spellCheck={false}
            disabled={busy}
          />
          <span className="hint">{t("unlock.pathHint")}</span>
        </div>

        {mode === "recovery" ? (
          <div className="field">
            <label htmlFor="recovery-key">{t("unlock.recoveryKeyLabel")}</label>
            <input
              id="recovery-key"
              className="input"
              value={recovery}
              onChange={(e) => setRecovery(e.target.value)}
              placeholder="XXXX-XXXX-XXXX-..."
              autoComplete="off"
              spellCheck={false}
              style={{ fontFamily: "var(--mono)" }}
              disabled={busy}
            />
            <span className="hint">{t("unlock.recoveryKeyHint")}</span>
          </div>
        ) : (
          <>
            <div className="field">
              <label htmlFor="password">
                {mode === "create" ? t("unlock.passwordCreate") : t("unlock.password")}
              </label>
              <input
                id="password"
                className="input"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                autoComplete={mode === "create" ? "new-password" : "current-password"}
                disabled={busy}
              />
              {mode === "create" && password.length > 0 && !check.ok ? (
                <span className="hint error">{check.reason}</span>
              ) : (
                <span className="hint">{t("unlock.passwordHint")}</span>
              )}
            </div>
            {mode === "create" ? (
              <div className="field">
                <label htmlFor="confirm">{t("unlock.passwordConfirm")}</label>
                <input
                  id="confirm"
                  className="input"
                  type="password"
                  value={confirm}
                  onChange={(e) => setConfirm(e.target.value)}
                  autoComplete="new-password"
                  disabled={busy}
                />
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

        <div className="row" style={{ justifyContent: "flex-end" }}>
          <button className="btn primary" disabled={!canSubmit} onClick={() => void submit()}>
            {busy
              ? t("common.busy")
              : mode === "create"
                ? t("unlock.submitCreate")
                : mode === "open"
                  ? t("unlock.submitOpen")
                  : t("unlock.submitRecovery")}
          </button>
        </div>
      </div>

      <div className="card tight">
        <h2>{t("unlock.phaseTitle")}</h2>
        <dl className="kv">
          <dt>{t("unlock.phaseVault")}</dt>
          <dd><span className="badge ok">{t("unlock.badgeReal")}</span> {t("unlock.phaseVaultValue")}</dd>
          <dt>{t("unlock.phaseUnlock")}</dt>
          <dd><span className="badge ok">{t("unlock.badgeReal")}</span> {t("unlock.phaseUnlockValue")}</dd>
          <dt>{t("unlock.phaseRecovery")}</dt>
          <dd><span className="badge ok">{t("unlock.badgeReal")}</span> {t("unlock.phaseRecoveryValue")}</dd>
          <dt>{t("unlock.phaseImport")}</dt>
          <dd><span className="badge ok">{t("unlock.badgeReal")}</span> {t("unlock.phaseImportValue")}</dd>
        </dl>
      </div>
    </div>
  );
}
