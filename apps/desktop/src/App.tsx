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
  clearBackupForce,
  isBackupDue,
  readBackupForce,
  writeBackupAttempt,
} from "./lib/backup-schedule";
import { loadBackupFreshness, loadBackupSchedule } from "./lib/backup-config";
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

type NavItem = { id: Screen; labelKey: MessageKey; icon: string };

const NAV_GROUPS: Array<{ label: MessageKey; items: NavItem[] }> = [
  {
    label: "nav.group",
    items: [
      { id: "vault", labelKey: "nav.vault", icon: "M4 7h16v14H4z M3 3h18v4H3z M9 11h6" },
      { id: "files", labelKey: "nav.files", icon: "M3 6h7l2 3h9v11H3z" },
      { id: "search", labelKey: "nav.search", icon: "M17 10a7 7 0 1 1-14 0 7 7 0 0 1 14 0z M15 15l6 6" },
      { id: "timeline", labelKey: "nav.timeline", icon: "M6 3v18 M6 6h12 M6 12h8 M6 18h14" },
      { id: "tasks", labelKey: "nav.tasks", icon: "M3 12l6 6L21 5" },
      { id: "checklists", labelKey: "nav.checklists", icon: "M3 6l2 2 3-4 M11 6h10 M3 15l2 2 3-4 M11 15h10" },
    ],
  },
  {
    label: "nav.analysisGroup",
    items: [
      { id: "review", labelKey: "nav.review", icon: "M5 3h14v18H5z M8 8h8 M8 12h8 M8 16h4" },
      { id: "precedents", labelKey: "nav.precedents", icon: "M3 4h4v16H3z M10 4h4v16h-4z M16 6l3-1 3 14-3 1z" },
      { id: "redaction", labelKey: "nav.redaction", icon: "M4 5h16 M4 9h16v5H4z M4 18h10" },
      { id: "exports", labelKey: "nav.exports", icon: "M12 15V3 M7 8l5-5 5 5 M4 14v6h16v-6" },
    ],
  },
  {
    label: "nav.toolsGroup",
    items: [
      { id: "ai", labelKey: "nav.ai", icon: "M8 3v5 M16 3v5 M6 8h12v3a6 6 0 0 1-12 0z M12 17v4" },
      { id: "ai-review", labelKey: "nav.aiReview", icon: "M3 4h18v12H9l-6 5z M7 8h10 M7 12h6" },
      { id: "settings", labelKey: "nav.settings", icon: "M3 7h8m4 0h6 M3 17h3m4 0h11 M11 4h4v6h-4z M6 14h4v6H6z" },
    ],
  },
];
const NAV = NAV_GROUPS.flatMap((group) => group.items);

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
  // Kasa kurulumu sırasında bir kez gösterilen kurtarma anahtarı burada
  // tutulur: `Unlock` ekranı gezinmede yeniden bağlandığı için anahtar orada
  // tutulsaydı ilk ekran değişiminde yok olurdu.
  const [lockGeneration, setLockGeneration] = useState(0);
  const lockEpoch = useRef(0);
  const statusRead = useRef(0);
  const [pendingKey, setPendingKey] = useState<string | null>(null);

  // Hareketsizlik sayacı ve uçuştaki kilit isteği ref'te tutulur: zamanlayıcı
  // geri çağrısı render anındaki değerleri değil **güncel** olanları görmeli.
  const lastActivity = useRef(Date.now());
  const unlocked = useRef(false);
  const lockingRef = useRef(false);
  const [locking, setLocking] = useState(false);

  const refresh = useCallback(async () => {
    const read = ++statusRead.current;
    try {
      const next = await api.status();
      if (read !== statusRead.current) return false;
      setStatus(next);
      // Başarılı okuma önceki durum/kilit hatasını siler: kırmızı bant bayat
      // kalmaz (ör. bir kez düşen kilit denemesinden sonra başarılı kilit).
      setError(null);
      return true;
    } catch (e) {
      if (read !== statusRead.current) return false;
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
  // Kurulum onayı beklerken de hareketsizlik sayacı işler.
  const vaultOpen = status.unlocked || pendingKey !== null;
  useEffect(() => {
    unlocked.current = vaultOpen;
    if (vaultOpen) lastActivity.current = Date.now();
  }, [vaultOpen]);

  // Otomatik yedek: plan **kasada** (`settings`, kapsam `vault`) durur; kasa
  // açıkken süresi dolunca yedek alınır. **Tazelik kanıtı da kasadır**
  // (`backup_runs`): cihaz kaydı yalnız yedektir ve kasa okunamazsa kullanılır.
  // Kasa ayarı bozuksa yedek **alınmaz** (Ayarlar nedeni yazar).
  const lastSuccessRef = useRef<string | null>(null);
  useEffect(() => {
    if (!status.unlocked) {
      // Kasa kapandı: başka kasanın tazeliğiyle devam edilmez.
      lastSuccessRef.current = null;
      return;
    }
    const run = async () => {
      // Okuma yolu kendi hatasını taşır; burada beklenmedik hata olursa yedek
      // alınmaz (sessiz yutma yok — Ayarlar nedeni yazar).
      const load = await loadBackupSchedule().catch(() => null);
      if (!load) return;
      if (load.source === "invalid" || !load.schedule) return;
      const schedule = load.schedule;
      const freshness = await loadBackupFreshness().catch(() => null);
      const lastIso = freshness?.iso ?? lastSuccessRef.current;
      const forced = readBackupForce();
      if (!forced && !isBackupDue(lastIso, Date.now(), schedule.days)) return;
      const at = new Date().toISOString();
      try {
        await api.vaultBackup(schedule.dir);
      } catch (e) {
        // Sessiz kal (modal/toast yok) ama deneme **kayda geçer**: Ayarlar'da
        // "son deneme" ve neden görünür; başarısız deneme tazeliği ilerletmez.
        writeBackupAttempt({ at, ok: false, code: toApiError(e).code });
        return;
      }
      // Yedek geçmişi (`backup_runs`) çekirdekte yazıldı; bu oturumda tazelik
      // ayrıca tutulur ki saatlik kontrol aynı yedeği yinelemesin.
      lastSuccessRef.current = at;
      clearBackupForce();
      try {
        localStorage.setItem(BACKUP_LAST_KEY, at);
      } catch {
        // kota/izin: önbellek yazılamadıysa kasa kaydı geçerli
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
      setLockGeneration(++lockEpoch.current);
      setStatus((s) => ({ ...s, unlocked: false }));
      setPendingKey(null);
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

  const contentRef = useRef<HTMLDivElement>(null);
  const focusContentAfterNavigation = useRef(false);
  useEffect(() => {
    if (focusContentAfterNavigation.current) {
      focusContentAfterNavigation.current = false;
      contentRef.current?.focus();
    }
  }, [screen]);

  function navigate(next: Screen, focusTarget: "navigation" | "content" = "navigation") {
    focusContentAfterNavigation.current = focusTarget === "content";
    setScreen(next);
    if (contentRef.current) contentRef.current.scrollTop = 0;
  }

  const locked = !status.unlocked || pendingKey !== null;

  return (
    <div className="app">
      <a className="skip-link" href="#workspace-content">{t("shell.skip")}</a>
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark" aria-hidden="true">{t("shell.mark")}</div>
          <div>
            <div className="brand-name">{t("shell.appName")}</div>
            <div className="brand-sub">{t("shell.tagline")}</div>
          </div>
        </div>

        <nav className="nav" aria-label={t("nav.aria")}>
          <select
            className="nav-select input"
            aria-label={t("nav.aria")}
            value={screen}
            onChange={(event) => {
              const item = NAV.find(({ id }) => id === event.target.value);
              if (item) navigate(item.id);
            }}
          >
            {NAV_GROUPS.map((group) => (
              <optgroup key={group.label} label={t(group.label)}>
                {group.items.map((item) => <option key={item.id} value={item.id}>{t(item.labelKey)}</option>)}
              </optgroup>
            ))}
          </select>
          {NAV_GROUPS.map((group) => (
            <div className="nav-group" key={group.label}>
              <div className="nav-label">{t(group.label)}</div>
              {group.items.map((item) => (
                <button
                  key={item.id}
                  className="nav-item"
                  aria-current={screen === item.id ? "page" : undefined}
                  onClick={() => navigate(item.id)}
                >
                  <svg viewBox="0 0 24 24" aria-hidden="true"><path d={item.icon} /></svg>
                  {t(item.labelKey)}
                </button>
              ))}
            </div>
          ))}
        </nav>

        <div className="sidebar-foot">
          <div className="muted small">{t("shell.version", { version: APP_VERSION })}</div>
        </div>
      </aside>

      <main className="main">
        <header className="topbar">
          <div className="crumb">
            {t("shell.crumbRoot")} <span aria-hidden="true">/</span>{" "}
            <strong>{t(NAV.find((n) => n.id === screen)?.labelKey ?? "shell.crumbRoot")}</strong>
          </div>
          <div className="topbar-actions">
            <div className="lock-state">
              <span className={vaultOpen ? "dot ok" : "dot locked"} aria-hidden="true" />
              {pendingKey ? t("shell.setupPending") : vaultOpen ? t("shell.unlocked") : t("shell.locked")}
            </div>
            {!locked ? (
              <button className="btn" disabled={locking} onClick={() => void lockNow()}>
                {t("shell.lock")}
              </button>
            ) : null}
          </div>
        </header>

        <div id="workspace-content" className="content" ref={contentRef} tabIndex={-1}>
          {error ? (
            <div className="notice danger" role="alert">
              <strong>{error.code}</strong> — {error.message}
            </div>
          ) : null}

          {!canShowScreen(screen, locked) ? (
            <LockedNotice pendingRecovery={pendingKey !== null} onUnlock={() => navigate("vault", "content")} />
          ) : screen === "vault" ? (
            pendingKey || locked ? (
              <Unlock
                key={lockGeneration}
                pendingKey={pendingKey}
                onKeyCreated={(key) => {
                  if (lockGeneration === lockEpoch.current) setPendingKey(key);
                }}
                onKeyDismissed={lockNow}
                locking={locking}
                onUnlocked={async (confirmed) => {
                  if (lockingRef.current || lockGeneration !== lockEpoch.current) return;
                  if (confirmed) {
                    setPendingKey(null);
                    setStatus({
                      unlocked: !confirmed.locked, vaultId: confirmed.vaultId, vaultPath: confirmed.vaultPath,
                      schemaVersion: confirmed.schemaVersion, preUpdateBackup: null,
                    });
                  }
                  await refresh();
                }}
              />
            ) : (
              <Workspace status={status} onOpenFiles={() => navigate("files", "content")} />
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
          ) : (
            <Settings />
          )}
        </div>
      </main>
    </div>
  );
}

/// Kilitliyken `vault` dışındaki ekranlar **render edilmez**: bileşen
/// ağacı sökülür, yüklenmiş belge/segment state'i bellekte kalmaz. Yalnız bu
/// ekran gösterilir; "Kasa"ya dönüp parola girilir.
function LockedNotice({ pendingRecovery, onUnlock }: { pendingRecovery: boolean; onUnlock: () => void }) {
  return (
    <div className="card" role="status" aria-live="polite">
      <div className="card-head">
        <div>
          <h2>{t(pendingRecovery ? "shell.setupPending" : "shell.locked")}</h2>
          <p className="card-sub">{t(pendingRecovery ? "unlock.recoveryNotice" : "shell.lockedHint")}</p>
        </div>
        <button className="btn primary" onClick={onUnlock}>
          {t("shell.goVault")}
        </button>
      </div>
    </div>
  );
}
