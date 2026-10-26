import { useEffect, useState } from "react";
import { open as openDirectory } from "@tauri-apps/plugin-dialog";
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

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

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
            <h2>Güncelleme öncesi yedeğe dön</h2>
            <p className="card-sub">
              Kasa dosyasının güncellemeden önceki doğrulanmış kopyası; veriyi geri alır.
            </p>
          </div>
          <button
            className="btn"
            disabled={busy || !unlocked}
            onClick={() => void refreshPreUpdate()}
          >
            Yedekleri tara
          </button>
        </div>
        {!unlocked ? (
          <p className="hint" style={{ margin: 0 }}>
            Kasa kilitliyken yedek listesi okunamaz.
          </p>
        ) : null}
        {unlocked && preUpdateLoaded && preUpdate.length === 0 ? (
          <p className="hint" style={{ margin: 0 }}>
            Kasa klasöründe güncelleme öncesi yedek yok. Bu yedek yalnız şema
            güncellemesi sırasında, doğrulanmış olarak yazılır.
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
                {item.usable ? "kullanılabilir" : "kullanılamaz"}
              </span>
              <br />
              <span style={{ fontFamily: "var(--mono)", fontSize: 12 }}>{item.fileName}</span>
              <br />
              <span className="hint">
                {formatBackupDate(item.modifiedAt)} · {formatBytes(item.sizeBytes)} ·{" "}
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
              <label htmlFor="rollback-confirm">Onay için “GERİ DÖN” yazın</label>
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
                Seçili yedeğe dön
              </button>
            </div>
            {rollbackDisabledReason(preUpdate, preUpdateSelected, rollbackConfirm) ? (
              <p className="hint" style={{ margin: 0 }}>
                {rollbackDisabledReason(preUpdate, preUpdateSelected, rollbackConfirm)}
              </p>
            ) : null}
            {rollbackDone ? (
              <div className="notice ok">
                <strong>Geri dönüş tamamlandı.</strong> {rollbackSummary(rollbackDone)}
                <br />
                Yedek:{" "}
                <span style={{ fontFamily: "var(--mono)", fontSize: 12 }}>
                  {rollbackDone.backupFile}
                </span>
                <br />
                Güvenlik kopyası:{" "}
                <span style={{ fontFamily: "var(--mono)", fontSize: 12 }}>
                  {rollbackDone.safetyCopy}
                </span>
                <br />
                Kasa açık; ekranlar tazelendiğinde geri dönen veri görünür.
              </div>
            ) : null}
          </>
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
            <h2>Otomatik yedek</h2>
            <p className="card-sub">
              Klasör seçilirse kasa açıkken süresi dolunca yedek alınır; sonuç kayda geçer.
            </p>
          </div>
          <button
            className="btn"
            onClick={async () => {
              const picked = await openDirectory({ directory: true, multiple: false, title: "Yedek klasörü seç" });
              if (typeof picked !== "string") return;
              localStorage.setItem(BACKUP_DIR_KEY, picked);
              localStorage.setItem(BACKUP_DAYS_KEY, "7");
              setAutoBackup(readBackupSettings());
            }}
          >
            Klasör seç
          </button>
        </div>
        <dl className="kv">
          <dt>Klasör</dt>
          <dd style={{ fontFamily: "var(--mono)", fontSize: 12 }}>{autoBackup.dir || "seçilmedi"}</dd>
          <dt>Aralık</dt>
          <dd>{autoBackup.days} gün</dd>
          <dt>Son yedek</dt>
          <dd>{autoBackup.last ? new Date(autoBackup.last).toLocaleString("tr-TR") : "hiç"}</dd>
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
            Otomatik yedeği kapat
          </button>
          <button
            className="btn ghost"
            disabled={!autoBackup.last}
            onClick={() => {
              localStorage.removeItem(BACKUP_LAST_KEY);
              setAutoBackup(readBackupSettings());
            }}
            title="Bir sonraki kontrolde yeniden yedek alınır"
          >
            Sıfırla
          </button>
        </div>
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
            {restoreInfo ? (
              <>
                <br />
                Kopyalamadan önce <strong>{restoreInfo.verifiedFiles}</strong> dosya ·{" "}
                {formatBytes(restoreInfo.verifiedBytes)} manifest sha256 ile doğrulandı ({restoreInfo.verifyMs} ms).
                {restoreInfo.unlistedBlobs.length > 0 ? (
                  <>
                    {" "}
                    Yedekte sayılmayan {restoreInfo.unlistedBlobs.length} dosya kasaya kopyalanmadı:{" "}
                    {restoreInfo.unlistedBlobs.slice(0, 5).join(", ")}
                    {restoreInfo.unlistedBlobs.length > 5 ? "…" : ""}
                  </>
                ) : null}
              </>
            ) : null}
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
