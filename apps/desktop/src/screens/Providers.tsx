import { useEffect, useState } from "react";
import { api, toApiError, type AdapterView, type ApiError } from "../lib/api";

const AUTH_LABEL: Record<string, string> = {
  subscription: "Abonelik (CLI kendi oturumunu yönetir)",
  api_key: "API anahtarı (yalnız OS secret ref)",
  interactive_only: "Etkileşimli kullanım",
};

export function Providers() {
  const [rows, setRows] = useState<AdapterView[]>([]);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

  async function refresh() {
    setBusy(true);
    try {
      setRows(await api.providerProbe());
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  return (
    <div className="page" style={{ maxWidth: 920 }}>
      <div className="page-head">
        <h1>AI bağlantıları</h1>
        <p className="lead">
          Uygulama, sağlayıcı kimlik dosyalarını okumaz ve token kopyalamaz. Bu ekran yalnız
          araçların kurulu olup olmadığını ve sürümünü yoklar. <strong>Hiçbir sağlayıcıya istek
          gönderilmez; AI bu sürümde kapalıdır.</strong>
        </p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="notice">
        AI bağlantısı F09–F11 işidir. Şu anda “AI kapalı” modu geçerlidir: belge, arama, kronoloji,
        iddia ve inceleme akışları tamamen AI olmadan çalışır.
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Resmî komut satırı araçları</h2>
            <p className="card-sub">Kurulu olmak güvenli kabul değildir; sürüm ve yetenek kapısı ayrıdır.</p>
          </div>
          <button className="btn" disabled={busy} onClick={() => void refresh()}>
            {busy ? "Yoklanıyor…" : "Yenile"}
          </button>
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-3)" }}>
          {rows.map((row) => (
            <div key={row.id} className="card tight" style={{ background: "var(--surface)" }}>
              <div className="card-head">
                <div>
                  <h2>{row.displayName}</h2>
                  <p className="card-sub" style={{ fontFamily: "var(--mono)", fontSize: 11 }}>{row.command}</p>
                </div>
                <span
                  className={`badge ${
                    row.state === "installed" ? "ok" : row.state === "unverified" ? "warn" : "plain"
                  }`}
                >
                  {row.state === "installed" ? "kurulu" : row.state === "unverified" ? "sınırlı" : "kurulu değil"}
                </span>
              </div>
              <dl className="kv" style={{ gridTemplateColumns: "100px 1fr" }}>
                <dt>Sürüm</dt><dd>{row.version ?? "—"}</dd>
                <dt>Kimlik</dt><dd>{AUTH_LABEL[row.authMode] ?? row.authMode}</dd>
                <dt>İzinli</dt>
                <dd style={{ fontFamily: "var(--mono)", fontSize: 11 }}>{row.allowedArgs.join(" ") || "—"}</dd>
              </dl>
              {row.batchForbidden ? (
                <div className="notice" style={{ fontSize: 12 }}>
                  Bu sağlayıcıda otomatik/toplu kullanım engellenir; yalnız etkileşimli kullanım.
                </div>
              ) : null}
            </div>
          ))}
          {rows.length === 0 && !busy ? (
            <div className="empty"><p style={{ margin: 0 }}>Keşif sonucu yok.</p></div>
          ) : null}
        </div>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Yerel model ve doğrudan API</h2>
            <p className="card-sub">Bu sürümde yapılandırılmadı</p>
          </div>
          <span className="badge plain">kapalı</span>
        </div>
        <p className="muted small" style={{ margin: 0 }}>
          Yerel model indirme/doğrulama ve API profili F09’un devamıdır. Yapılandırılmamış bir
          sağlayıcı “hazır” gibi gösterilmez; gönderim onayı olmadan hiçbir içerik dışarı çıkmaz.
        </p>
      </div>
    </div>
  );
}
