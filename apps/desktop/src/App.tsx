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
import {
  BACKUP_CHECK_MS,
  BACKUP_LAST_KEY,
  isBackupDue,
  readBackupSettings,
} from "./lib/backup-schedule";
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

  const refresh = useCallback(async () => {
    try {
      setStatus(await api.status());
    } catch (e) {
      setError(toApiError(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Hareketsizlik sayacı: kasa açıkken süre dolunca kendiliğinden kilitlenir.
  const lastActivity = useRef(Date.now());
  const unlocked = useRef(false);
  useEffect(() => {
    unlocked.current = status.unlocked;
  }, [status.unlocked]);

  // Otomatik yedek: klasör seçilmişse ve süresi dolduysa kasa açıkken alınır.
  useEffect(() => {
    const run = async () => {
      if (!unlocked.current) return;
      const settings = readBackupSettings();
      if (!settings.dir) return;
      if (!isBackupDue(settings.last, Date.now(), settings.days)) return;
      try {
        await api.vaultBackup(settings.dir);
        localStorage.setItem(BACKUP_LAST_KEY, new Date().toISOString());
      } catch {
        // Sessiz: sonraki kontrolde yeniden denenir; kullanıcıya gürültü yapma.
      }
    };
    const timer = setInterval(() => void run(), BACKUP_CHECK_MS);
    void run();
    return () => clearInterval(timer);
  }, []);

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
      lastActivity.current = Date.now();
      void api
        .lockVault()
        .then(() => refresh())
        .catch(() => undefined);
    }, AUTO_LOCK_CHECK_MS);
    return () => {
      window.removeEventListener("pointerdown", bump);
      window.removeEventListener("keydown", bump);
      window.removeEventListener("wheel", bump);
      clearInterval(timer);
    };
  }, [refresh]);

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
            <button
              className="btn"
              onClick={async () => {
                try {
                  await api.lockVault();
                  await refresh();
                } catch (e) {
                  setError(toApiError(e));
                }
              }}
            >
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

          {screen === "vault" ? (
            locked ? (
              <Unlock
                onUnlocked={async () => {
                  setError(null);
                  await refresh();
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
