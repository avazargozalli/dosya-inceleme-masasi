import { useEffect, useState } from "react";
import { open as openDirectory } from "@tauri-apps/plugin-dialog";
import { api, toApiError, type ApiError, type BackupResult, type VaultStatus } from "../lib/api";
import type { VaultGcResult } from "../lib/api";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function Settings() {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [backup, setBackup] = useState<BackupResult | null>(null);
  const [gc, setGc] = useState<VaultGcResult | null>(null);
  const [oldPassword, setOldPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [newPassword2, setNewPassword2] = useState("");
  const [passwordChanged, setPasswordChanged] = useState(false);
  const [restoreDir, setRestoreDir] = useState("");
  const [destDir, setDestDir] = useState("");
  const [restored, setRestored] = useState<string | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void api.status().then(setStatus).catch((e) => setError(toApiError(e)));
  }, []);

  async function runBackup() {
    const picked = await openDirectory({ directory: true, multiple: false, title: "Yedek klasörünü seç" });
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
      setError({ code: "E_PASSWORD_MISMATCH", message: "Yeni parolalar eşleşmiyor.", retryable: false });
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
    const picked = await openDirectory({ directory: true, multiple: false, title: "Yedek klasörünü seç" });
    if (typeof picked !== "string") {
      return;
    }
    setRestoreDir(picked);
    if (!destDir.trim()) {
      setError({ code: "E_PATH", message: "Geri yükleme için hedef konum girin.", retryable: false });
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const result = await api.vaultRestore(picked, destDir.trim());
      setRestored(result.vaultPath);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page" style={{ maxWidth: 820 }}>
      <div className="page-head">
        <h1>Ayarlar ve yedek</h1>
        <p className="lead">
          Yedek, kasa başlığını, SQLCipher veritabanını ve şifreli blob'ları tek klasöre kopyalar.
          Açık metin üretilmez; yedeği açmak için kasa parolası veya kurtarma anahtarı gerekir.
        </p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Kasa</h2>
            <p className="card-sub">{status?.unlocked ? "açık" : "kilitli"}</p>
          </div>
          <span className={`badge ${status?.unlocked ? "ok" : "warn"}`}>
            {status?.unlocked ? "kasa açık" : "kasa kilitli"}
          </span>
        </div>
        <dl className="kv">
          <dt>Kimlik</dt><dd>{status?.vaultId ?? "—"}</dd>
          <dt>Konum</dt>
          <dd style={{ fontFamily: "var(--mono)", fontSize: 12 }}>{status?.vaultPath ?? "—"}</dd>
          <dt>Şema sürümü</dt><dd>{status?.schemaVersion ?? "—"}</dd>
          <dt>Sürüm</dt><dd>0.2.0 (pre-alpha)</dd>
        </dl>
        {status?.preUpdateBackup ? (
          <div className="notice ok">
            <strong>Güncelleme öncesi yedek alındı.</strong> Bu kasa yeni şemaya
            geçirilmeden önce doğrulanmış bir kopya yazıldı:{" "}
            <span style={{ fontFamily: "var(--mono)", fontSize: 12 }}>{status.preUpdateBackup}</span>
            . Güncelleme yarıda kalırsa bu dosya geri konur; uygulama yedek
            alınamadıysa güncellemeyi başlatmaz.
          </div>
        ) : null}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Yedek al</h2>
            <p className="card-sub">Tutarlı snapshot; yedek açılmadan önce doğrulanır.</p>
          </div>
          <button className="btn primary" disabled={busy || !status?.unlocked} onClick={() => void runBackup()}>
            Yedek klasörü seç
          </button>
        </div>
        {backup ? (
          <div className="notice ok">
            Yedek hazır: <span style={{ fontFamily: "var(--mono)", fontSize: 12 }}>{backup.path}</span>
            <br />
            {backup.fileCount} dosya · {formatBytes(backup.totalBytes)} · manifest {backup.manifestHash.slice(0, 16)}…
            {backup.archivePath ? (
              <>
                <br />
                Tek dosya: <code>{backup.archivePath}</code>
              </>
            ) : null}
          </div>
        ) : (
          <p className="hint" style={{ margin: 0 }}>
            Kasa kilitliyken yedek alınamaz. Yedeği kasa dosyasından ayrı bir diskte saklayın.
          </p>
        )}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Kullanılmayan veriler</h2>
            <p className="card-sub">
              Hiçbir belge sürümüne veya türev sayfaya bağlı olmayan şifreli blob'lar.
            </p>
          </div>
          <div className="row" style={{ gap: 6 }}>
            <button className="btn" disabled={busy} onClick={() => void runGc(true)}>
              Tara
            </button>
            <button
              className="btn"
              disabled={busy || !gc || gc.candidates.length === 0}
              onClick={() => void runGc(false)}
            >
              Temizle
            </button>
          </div>
        </div>
        {gc ? (
          <div className={gc.deleted.length > 0 ? "notice ok" : "notice"}>
            {gc.dryRun
              ? `${gc.candidates.length} aday · ${formatBytes(gc.candidates.reduce((sum, b) => sum + b.encryptedSize, 0))}`
              : `${gc.deleted.length} kayıt silindi · ${formatBytes(gc.freedBytes)} boşaldı`}
            {gc.missingFiles.length > 0 ? (
              <>
                <br />
                Dosya uyarısı: {gc.missingFiles.join("; ")}
              </>
            ) : null}
          </div>
        ) : (
          <p className="hint" style={{ margin: 0 }}>
            Önce "Tara" ile adayları görün; silme yalnız referanssız kayıtlara dokunur.
          </p>
        )}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Yedekten geri yükle</h2>
            <p className="card-sub">Yeni bir kasaya kopyalar; mevcut kasanın üzerine yazmaz.</p>
          </div>
        </div>
        <div className="field">
          <label htmlFor="restore-dest">Yeni kasa konumu</label>
          <input
            id="restore-dest"
            className="input"
            value={destDir}
            onChange={(e) => setDestDir(e.target.value)}
            placeholder="~/Dosyalar/GeriYuklenenKasa"
          />
        </div>
        <div className="row" style={{ justifyContent: "flex-end" }}>
          <button className="btn" disabled={busy || !destDir.trim()} onClick={() => void runRestore()}>
            Yedek seç ve geri yükle
          </button>
        </div>
        {restoreDir ? <p className="hint" style={{ margin: 0 }}>Seçilen yedek: {restoreDir}</p> : null}
        {restored ? (
          <div className="notice ok">
            Geri yükleme tamamlandı: <span style={{ fontFamily: "var(--mono)", fontSize: 12 }}>{restored}</span>
            <br />
            Şimdi Kasa ekranından bu konumu ve parolanızı girerek açın.
          </div>
        ) : null}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Parola değiştir</h2>
            <p className="card-sub">
              Yalnız parola yuvası yeniden sarılır; kasa içeriği ve kurtarma anahtarı değişmez.
            </p>
          </div>
        </div>
        <div className="row">
          <div className="field" style={{ flex: "1 1 200px" }}>
            <label htmlFor="pw-old">Mevcut parola</label>
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
            <label htmlFor="pw-new">Yeni parola (en az 10 karakter)</label>
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
            <label htmlFor="pw-new2">Yeni parola (tekrar)</label>
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
            Parolayı değiştir
          </button>
        </div>
        {passwordChanged ? (
          <div className="notice ok">Parola değiştirildi. Kurtarma anahtarınız hâlâ geçerli.</div>
        ) : null}
      </div>

      <div className="card tight">
        <h2>Teşhis</h2>
        <p className="muted small" style={{ margin: 0 }}>
          Uygulama belge içeriğini günlüğe yazmaz; yalnız hata kodları ve sürüm bilgisi tutulur.
          Destek için hata kodu yeterlidir.
        </p>
      </div>
    </div>
  );
}
