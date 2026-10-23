import { useState } from "react";
import { api, toApiError, type ApiError } from "../lib/api";
import { checkPassword, checkPasswordConfirmation } from "../lib/vault-input";

type Mode = "open" | "create" | "recovery";

const DEFAULT_DIR = "Dosyalar/DosyaIncelemeMasasi";

export function Unlock({ onUnlocked }: { onUnlocked: () => Promise<void> }) {
  const [mode, setMode] = useState<Mode>("create");
  const [path, setPath] = useState(DEFAULT_DIR);
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [recovery, setRecovery] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);
  const [createdKey, setCreatedKey] = useState<string | null>(null);
  const [keySaved, setKeySaved] = useState(false);

  const check = mode === "create" ? checkPasswordConfirmation(password, confirm) : checkPassword(password);
  const canSubmit =
    !busy && (mode === "recovery" ? recovery.trim().length > 0 : check.ok && path.trim().length > 0);

  async function submit() {
    setError(null);
    setBusy(true);
    try {
      if (mode === "create") {
        const result = await api.createVault(path.trim(), password);
        setCreatedKey(result.recoveryKey);
        setPassword("");
        setConfirm("");
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

  if (createdKey) {
    return (
      <div className="page" style={{ maxWidth: 720 }}>
        <div className="page-head">
          <div className="steps" aria-label="Kurulum adımları">
            <span className="step done"><span className="step-dot">✓</span> Kasa</span>
            <span className="step active"><span className="step-dot">2</span> Kurtarma</span>
            <span className="step"><span className="step-dot">3</span> Kullanmaya başla</span>
          </div>
          <h1>Kurtarma anahtarını şimdi kaydet</h1>
          <p className="lead">
            Bu anahtar yalnızca bir kez gösterilir. Parolanı unutursan kasayı bununla açarsın.
            Anahtarı kasa dosyasından ayrı bir yerde sakla; birlikte saklamak korumasını azaltır.
          </p>
        </div>

        <div className="card">
          <div className="recovery" aria-label="Kurtarma anahtarı">{createdKey}</div>
          <div className="hint">8 grup · 52 karakter · tireler yalnız okunabilirlik içindir.</div>
          <label className="row" style={{ alignItems: "center", gap: 8 }}>
            <input
              type="checkbox"
              checked={keySaved}
              onChange={(e) => setKeySaved(e.target.checked)}
              style={{ width: 16, height: 16 }}
            />
            <span>Anahtarı güvenli bir yere kaydettim.</span>
          </label>
          <div className="row" style={{ justifyContent: "flex-end" }}>
            <button
              className="btn ghost"
              onClick={() => {
                setCreatedKey(null);
                setKeySaved(false);
                setMode("open");
              }}
            >
              Geri
            </button>
            <button className="btn primary" disabled={!keySaved} onClick={() => void onUnlocked()}>
              Kasayı aç
            </button>
          </div>
        </div>
        <div className="notice">
          Anahtarı kaybetmediğinden emin olmadan bu ekranı kapatma. Uygulama anahtarı tekrar göstermez.
        </div>
      </div>
    );
  }

  return (
    <div className="page" style={{ maxWidth: 620 }}>
      <div className="page-head">
        <h1>Kasa</h1>
        <p className="lead">
          Belgeler cihazında şifreli bir kasada durur. Kasa kapalıyken içerik okunamaz; arama
          indeksi ve yedekler de aynı şifreleme kapsamındadır.
        </p>
      </div>

      <div className="card">
        <div className="steps" aria-label="Kasa işlemleri">
          <button className={mode === "create" ? "btn primary" : "btn"} onClick={() => setMode("create")}>Yeni kasa</button>
          <button className={mode === "open" ? "btn primary" : "btn"} onClick={() => setMode("open")}>Mevcut kasayı aç</button>
          <button className={mode === "recovery" ? "btn primary" : "btn"} onClick={() => setMode("recovery")}>Kurtarma anahtarı</button>
        </div>

        <div className="field">
          <label htmlFor="vault-path">Kasa konumu</label>
          <input
            id="vault-path"
            className="input"
            value={path}
            onChange={(e) => setPath(e.target.value)}
            placeholder={DEFAULT_DIR}
            autoComplete="off"
            spellCheck={false}
          />
          <span className="hint">
            Ev klasörüne göreli yol veya tam yol. Klasör yoksa oluşturulur; mevcut kasa dosyası
            üzerine yazılmaz.
          </span>
        </div>

        {mode === "recovery" ? (
          <div className="field">
            <label htmlFor="recovery-key">Kurtarma anahtarı</label>
            <input
              id="recovery-key"
              className="input"
              value={recovery}
              onChange={(e) => setRecovery(e.target.value)}
              placeholder="XXXX-XXXX-XXXX-..."
              autoComplete="off"
              spellCheck={false}
              style={{ fontFamily: "var(--mono)" }}
            />
            <span className="hint">Tireler ve boşluklar önemsiz; büyük/küçük harf fark etmez.</span>
          </div>
        ) : (
          <>
            <div className="field">
              <label htmlFor="password">{mode === "create" ? "Parola oluştur" : "Parola"}</label>
              <input
                id="password"
                className="input"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                autoComplete={mode === "create" ? "new-password" : "current-password"}
              />
              {mode === "create" && password.length > 0 && !check.ok ? (
                <span className="hint error">{check.reason}</span>
              ) : (
                <span className="hint">
                  En az 12 karakter. Parola cihazda kalır; hiçbir sunucuya gönderilmez.
                </span>
              )}
            </div>
            {mode === "create" ? (
              <div className="field">
                <label htmlFor="confirm">Parola (tekrar)</label>
                <input
                  id="confirm"
                  className="input"
                  type="password"
                  value={confirm}
                  onChange={(e) => setConfirm(e.target.value)}
                  autoComplete="new-password"
                />
              </div>
            ) : null}
          </>
        )}

        {error ? (
          <div className="notice danger" role="alert">
            <strong>{error.code}</strong> — {error.message}
            {error.retryable ? " (yeniden denenebilir)" : ""}
          </div>
        ) : null}

        <div className="row" style={{ justifyContent: "flex-end" }}>
          <button className="btn primary" disabled={!canSubmit} onClick={() => void submit()}>
            {busy ? "İşleniyor…" : mode === "create" ? "Kasayı oluştur" : mode === "open" ? "Kilidi aç" : "Kurtarma ile aç"}
          </button>
        </div>
      </div>

      <div className="card tight">
        <h2>Bu fazda ne çalışıyor?</h2>
        <dl className="kv">
          <dt>Kasa oluşturma</dt><dd><span className="badge ok">gerçek</span> Argon2id + AES-256-GCM</dd>
          <dt>Kilit açma</dt><dd><span className="badge ok">gerçek</span> dosyadan okuma, anahtar yalnız bellekte</dd>
          <dt>Kurtarma anahtarı</dt><dd><span className="badge ok">gerçek</span> 32 bayt, base32 gösterim</dd>
          <dt>Belge içe aktarma</dt><dd><span className="badge plain">F05</span> henüz yok</dd>
        </dl>
      </div>
    </div>
  );
}
