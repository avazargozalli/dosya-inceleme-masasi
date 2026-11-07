import { useCallback, useEffect, useRef, useState } from "react";
import { api, toApiError, type ApiError, type VaultStatus } from "./lib/api";
import { t, type MessageKey } from "./lib/i18n";
import { version as appVersion } from "../package.json";
import { Cases } from "./screens/Cases";
import { Checklists } from "./screens/Checklists";
import { Exports } from "./screens/Exports";
import { Precedents } from "./screens/Precedents";
import { Redaction } from "./screens/Redaction";
import { Ai } from "./screens/Ai";
import { AUTO_LOCK_CHECK_MS, shouldAutoLock } from "./lib/autolock";
import { canShowScreen } from "./lib/lock";
import {
  BACKUP_CHECK_MS,
  BACKUP_LAST_KEY,
  isBackupDue,
  readBackupLast,
  writeBackupAttempt,
} from "./lib/backup-schedule";
import { loadBackupSchedule } from "./lib/backup-config";
import { Providers } from "./screens/Providers";
import { Review } from "./screens/Review";
import { Search } from "./screens/Search";
import { Settings } from "./screens/Settings";
import { Tasks } from "./screens/Tasks";
import { Timeline } from "./screens/Timeline";
import { Unlock } from "./screens/Unlock";
import { Workspace } from "./screens/Workspace";

type Screen =
  | "vault"
  | "files"
  | "search"
  | "timeline"
  | "tasks"
  | "checklists"
  | "review"
  | "precedents"
  | "redaction"
  | "exports"
  | "ai"
  | "ai-review"
  | "settings";

// Sürüm etiketi tek kaynaktan gelir: paket sürümü. Eskiden burada gömülü
// eski sürüm/faz etiketi yazıyordu; uygulama F04'teyken etiket bayat kalmıştı.
const APP_VERSION: string = appVersion;

const NAV: Array<{ id: Screen; labelKey: MessageKey; phase?: string }> = [
  { id: "vault", labelKey: "nav.vault" },
  { id: "files", labelKey: "nav.files", phase: "F04" },
  { id: "search", labelKey: "nav.search", phase: "F07" },
  { id: "timeline", labelKey: "nav.timeline", phase: "F08" },
  { id: "tasks", labelKey: "nav.tasks", phase: "F08" },
  { id: "checklists", labelKey: "nav.checklists", phase: "F08" },
  { id: "review", labelKey: "nav.review", phase: "F08" },
  { id: "precedents", labelKey: "nav.precedents" },
  { id: "redaction", labelKey: "nav.redaction" },
  { id: "exports", labelKey: "nav.exports" },
  { id: "ai", labelKey: "nav.ai" },
  { id: "ai-review", labelKey: "nav.aiReview" },
  { id: "settings", labelKey: "nav.settings" },
];

export function App() {
  const [screen, setScreen] = useState<Screen>("vault");
  const [status, setStatus] = useState<VaultStatus>({
    unlocked: false,
    vaultPath: null,
    vaultId: null,
    schemaVersion: null,
    preUpdateBackup: null,
  });
  const [error, setError] = useState<ApiError | null>(null);
  // Kasa oluşturulduktan sonra bir kez gösterilen kurtarma anahtarı burada
  // tutulur: `Unlock` ekranı gezinmede yeniden bağlandığı için anahtar orada
  // tutulsaydı ilk ekran değişiminde yok olurdu.
  const [pendingKey, setPendingKey] = useState<string | null>(null);

  // Hareketsizlik sayacı ve uçuştaki kilit isteği ref'te tutulur: zamanlayıcı
  // geri çağrısı render anındaki değerleri değil **güncel** olanları görmeli.
  const lastActivity = useRef(Date.now());
  const unlocked = useRef(false);
  const lockingRef = useRef(false);
  const [locking, setLocking] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setStatus(await api.status());
      // Başarılı okuma önceki durum/kilit hatasını siler: kırmızı bant bayat
      // kalmaz (ör. bir kez düşen kilit denemesinden sonra başarılı kilit).
      setError(null);
      return true;
    } catch (e) {
      setError(toApiError(e));
      return false;
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Kasa açıldığında hareketsizlik sayacı sıfırlanır: kilitliyken geçen süre
  // "kasa açıkken hareketsizlik" sayılmaz. Aksi hâlde kasa açıldıktan hemen
  // sonra yeniden kilitlenebilir.
  useEffect(() => {
    unlocked.current = status.unlocked;
    if (status.unlocked) lastActivity.current = Date.now();
  }, [status.unlocked]);

  // Otomatik yedek: plan **kasada** (`settings`, kapsam `vault`) durur; kasa
  // açıkken süresi dolunca yedek alınır. Cihaz kaydı yalnız göç kaynağı ve
  // önbellektir. Kasa ayarı bozuksa yedek **alınmaz** (Ayarlar nedeni yazar).
  useEffect(() => {
    if (!status.unlocked) return;
    const run = async () => {
      // Okuma yolu kendi hatasını taşır; burada beklenmedik hata olursa yedek
      // alınmaz (sessiz yutma yok — Ayarlar nedeni yazar).
      const load = await loadBackupSchedule().catch(() => null);
      if (!load) return;
      if (load.source === "invalid" || !load.schedule) return;
      const schedule = load.schedule;
      if (!isBackupDue(readBackupLast(), Date.now(), schedule.days)) return;
      const at = new Date().toISOString();
      try {
        await api.vaultBackup(schedule.dir);
      } catch (e) {
        // Sessiz kal (modal/toast yok) ama deneme **kayda geçer**: Ayarlar'da
        // "son deneme" ve neden görünür; başarısız deneme tazeliği ilerletmez.
        writeBackupAttempt({ at, ok: false, code: toApiError(e).code });
        return;
      }
      try {
        localStorage.setItem(BACKUP_LAST_KEY, at);
      } catch {
        // kota/izin: tazelik yazılamadıysa sonraki kontrolde yeniden denenir
      }
      writeBackupAttempt({ at, ok: true, code: null });
    };
    const timer = setInterval(() => void run(), BACKUP_CHECK_MS);
    void run();
    return () => clearInterval(timer);
  }, [status.unlocked]);

  // Kilit tek kapıdan geçer (elle + otomatik): çift istek yok, başarıda durum
  // **yerel olarak** kilitliye çekilir. Durum okuması düşse bile kasa kapalıyken
  // açık ekran/içerik görünür kalmaz (fail-closed).
  const lockNow = useCallback(async (): Promise<boolean> => {
    if (lockingRef.current) return false;
    lockingRef.current = true;
    setLocking(true);
    try {
      await api.lockVault();
      setStatus((s) => ({ ...s, unlocked: false }));
      lastActivity.current = Date.now();
      await refresh();
      return true;
    } catch (e) {
      // Sayaç sıfırlanmaz: sonraki kontrol (30 sn) yeniden dener.
      setError(toApiError(e));
      return false;
    } finally {
      lockingRef.current = false;
      setLocking(false);
    }
  }, [refresh]);

  // Hareketsizlik sayacı: kasa açıkken süre dolunca kendiliğinden kilitlenir.
  useEffect(() => {
    const bump = () => {
      lastActivity.current = Date.now();
    };
    window.addEventListener("pointerdown", bump);
    window.addEventListener("keydown", bump);
    window.addEventListener("wheel", bump, { passive: true });
    const timer = setInterval(() => {
      if (!unlocked.current) return;
      if (!shouldAutoLock(lastActivity.current, Date.now())) return;
      void lockNow();
    }, AUTO_LOCK_CHECK_MS);
    return () => {
      window.removeEventListener("pointerdown", bump);
      window.removeEventListener("keydown", bump);
      window.removeEventListener("wheel", bump);
      clearInterval(timer);
    };
  }, [lockNow]);

  const locked = !status.unlocked;

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark" aria-hidden="true">{t("shell.mark")}</div>
          <div>
            <div className="brand-name">{t("shell.appName")}</div>
            <div className="brand-sub">{t("shell.tagline")}</div>
          </div>
        </div>

        <nav className="nav" aria-label={t("nav.aria")}>
          <div className="nav-label">{t("nav.group")}</div>
          {NAV.map((item) => (
            <button
              key={item.id}
              className="nav-item"
              aria-current={screen === item.id ? "page" : undefined}
              onClick={() => setScreen(item.id)}
            >
              <span>{t(item.labelKey)}</span>
              {item.phase ? <span className="nav-tag">{item.phase}</span> : null}
            </button>
          ))}
        </nav>

        <div className="sidebar-foot">
          <div className="lock-state">
            <span className={status.unlocked ? "dot ok" : "dot locked"} aria-hidden="true" />
            {status.unlocked ? t("shell.unlocked") : t("shell.locked")}
          </div>
          <div className="muted small">{t("shell.version", { version: APP_VERSION })}</div>
        </div>
      </aside>

      <main className="main">
        <header className="topbar">
          <div className="crumb">
            {t("shell.crumbRoot")} <span aria-hidden="true">/</span>{" "}
            <strong>{t(NAV.find((n) => n.id === screen)?.labelKey ?? "shell.crumbRoot")}</strong>
          </div>
          {status.unlocked ? (
            <button className="btn" disabled={locking} onClick={() => void lockNow()}>
              {t("shell.lock")}
            </button>
          ) : null}
        </header>

        <div className="content">
          {error ? (
            <div className="notice danger" role="alert">
              <strong>{error.code}</strong> — {error.message}
            </div>
          ) : null}

          {!canShowScreen(screen, locked) ? (
            <LockedNotice onUnlock={() => setScreen("vault")} />
          ) : screen === "vault" ? (
            pendingKey || locked ? (
              <Unlock
                pendingKey={pendingKey}
                onKeyCreated={setPendingKey}
                onKeyDismissed={() => setPendingKey(null)}
                onUnlocked={async () => {
                  setError(null);
                  // Durum okunamazsa anahtar ekranı açık kalır: tek kopyadır,
                  // başarısız bir tazeleme yüzünden düşürülmez.
                  if (await refresh()) setPendingKey(null);
                }}
              />
            ) : (
              <Workspace status={status} />
            )
          ) : screen === "files" ? (
            <Cases />
          ) : screen === "search" ? (
            <Search />
          ) : screen === "review" ? (
            <Review />
          ) : screen === "ai" ? (
            <Providers />
          ) : screen === "ai-review" ? (
            <Ai />
          ) : screen === "precedents" ? (
            <Precedents />
          ) : screen === "redaction" ? (
            <Redaction />
          ) : screen === "exports" ? (
            <Exports />
          ) : screen === "timeline" ? (
            <Timeline />
          ) : screen === "tasks" ? (
            <Tasks />
          ) : screen === "checklists" ? (
            <Checklists />
          ) : screen === "settings" ? (
            <Settings />
          ) : (
            <ComingPhase
              labelKey={NAV.find((n) => n.id === screen)?.labelKey ?? "shell.crumbRoot"}
              phase={NAV.find((n) => n.id === screen)?.phase ?? ""}
            />
          )}
        </div>
      </main>
    </div>
  );
}

/// Kilitliyken `vault` dışındaki ekranlar **render edilmez**: bileşen
/// ağacı sökülür, yüklenmiş belge/segment state'i bellekte kalmaz. Yalnız bu
/// ekran gösterilir; "Kasa"ya dönüp parola girilir.
function LockedNotice({ onUnlock }: { onUnlock: () => void }) {
  return (
    <div className="card" role="status" aria-live="polite">
      <div className="card-head">
        <div>
          <h2>{t("shell.locked")}</h2>
          <p className="card-sub">{t("shell.lockedHint")}</p>
        </div>
        <button className="btn primary" onClick={onUnlock}>
          {t("shell.goVault")}
        </button>
      </div>
    </div>
  );
}

function ComingPhase({ labelKey, phase }: { labelKey: MessageKey; phase: string }) {
  return (
    <div className="page">
      <div className="page-head">
        <h1>{t(labelKey)}</h1>
        <p className="lead">{t("shell.comingLead", { phase })}</p>
      </div>
      <div className="empty">
        <p style={{ margin: 0 }}>{t("shell.comingEmpty")}</p>
      </div>
    </div>
  );
}
