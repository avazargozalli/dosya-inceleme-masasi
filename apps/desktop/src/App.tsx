import { useCallback, useEffect, useState } from "react";
import { api, toApiError, type ApiError, type VaultStatus } from "./lib/api";
import { Cases } from "./screens/Cases";
import { Precedents } from "./screens/Precedents";
import { Redaction } from "./screens/Redaction";
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
  | "review"
  | "precedents"
  | "redaction"
  | "ai"
  | "settings";

const NAV: Array<{ id: Screen; label: string; phase?: string }> = [
  { id: "vault", label: "Kasa" },
  { id: "files", label: "Dosyalar", phase: "F04" },
  { id: "search", label: "Arama", phase: "F07" },
  { id: "timeline", label: "Kronoloji", phase: "F08" },
  { id: "tasks", label: "Görevler", phase: "F08" },
  { id: "review", label: "İnceleme", phase: "F08" },
  { id: "precedents", label: "Emsal" },
  { id: "redaction", label: "Maskeleme" },
  { id: "ai", label: "AI bağlantıları" },
  { id: "settings", label: "Ayarlar" },
];

export function App() {
  const [screen, setScreen] = useState<Screen>("vault");
  const [status, setStatus] = useState<VaultStatus>({
    unlocked: false,
    vaultPath: null,
    vaultId: null,
    schemaVersion: null,
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

  const locked = !status.unlocked;

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark" aria-hidden="true">Dİ</div>
          <div>
            <div className="brand-name">Dosya İnceleme</div>
            <div className="brand-sub">yerel · şifreli</div>
          </div>
        </div>

        <nav className="nav" aria-label="Ana gezinme">
          <div className="nav-label">Çalışma</div>
          {NAV.map((item) => (
            <button
              key={item.id}
              className="nav-item"
              aria-current={screen === item.id ? "page" : undefined}
              onClick={() => setScreen(item.id)}
            >
              <span>{item.label}</span>
              {item.phase ? <span className="nav-tag">{item.phase}</span> : null}
            </button>
          ))}
        </nav>

        <div className="sidebar-foot">
          <div className="lock-state">
            <span className={status.unlocked ? "dot ok" : "dot locked"} aria-hidden="true" />
            {status.unlocked ? "Kasa açık" : "Kasa kilitli"}
          </div>
          <div className="muted small">v0.1.0 · F02</div>
        </div>
      </aside>

      <main className="main">
        <header className="topbar">
          <div className="crumb">
            Kasa <span aria-hidden="true">/</span> <strong>{NAV.find((n) => n.id === screen)?.label}</strong>
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
              Kasayı kilitle
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
          ) : screen === "precedents" ? (
            <Precedents />
          ) : screen === "redaction" ? (
            <Redaction />
          ) : screen === "timeline" ? (
            <Timeline />
          ) : screen === "tasks" ? (
            <Tasks />
          ) : screen === "settings" ? (
            <Settings />
          ) : (
            <ComingPhase label={NAV.find((n) => n.id === screen)?.label ?? ""} phase={NAV.find((n) => n.id === screen)?.phase ?? ""} />
          )}
        </div>
      </main>
    </div>
  );
}

function ComingPhase({ label, phase }: { label: string; phase: string }) {
  return (
    <div className="page">
      <div className="page-head">
        <h1>{label}</h1>
        <p className="lead">Bu ekran {phase} fazında açılır. Şu an çalışıyormuş gibi gösterilmiyor.</p>
      </div>
      <div className="empty">
        <p style={{ margin: 0 }}>Hazır değil — sahte veri veya pasif buton üretilmez (INV-09).</p>
      </div>
    </div>
  );
}
