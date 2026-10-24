import { useEffect, useState, type CSSProperties } from "react";
import { api, toApiError, type AdapterView, type ApiError, type CliProbeView } from "../lib/api";
import {
  probeArgv,
  probeCanRun,
  probeOutputs,
  probeStatusLabel,
  probeSummary,
  probeWarnings,
} from "../lib/providers";

const AUTH_LABEL: Record<string, string> = {
  subscription: "Abonelik (CLI kendi oturumunu yönetir)",
  api_key: "API anahtarı (yalnız OS secret ref)",
  interactive_only: "Etkileşimli kullanım",
};

const MONO: CSSProperties = { fontFamily: "var(--mono)", fontSize: 11, wordBreak: "break-all" };

const PRE: CSSProperties = {
  margin: "4px 0 0",
  padding: "var(--space-3)",
  background: "var(--surface)",
  border: "1px solid var(--line)",
  borderRadius: "var(--radius)",
  fontFamily: "var(--mono)",
  fontSize: 11,
  whiteSpace: "pre-wrap",
  wordBreak: "break-word",
  maxHeight: 200,
  overflow: "auto",
};

export function Providers() {
  const [rows, setRows] = useState<AdapterView[]>([]);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  const [running, setRunning] = useState<string | null>(null);
  const [results, setResults] = useState<Record<string, CliProbeView>>({});
  const [runErrors, setRunErrors] = useState<Record<string, ApiError>>({});
  const [cancelHint, setCancelHint] = useState<string | null>(null);

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

  async function runProbe(adapterId: string) {
    setRunning(adapterId);
    setCancelHint(null);
    setRunErrors((prev) => {
      const next = { ...prev };
      delete next[adapterId];
      return next;
    });
    try {
      const result = await api.providerProbeRun(adapterId);
      setResults((prev) => ({ ...prev, [adapterId]: result }));
    } catch (e) {
      const failure = toApiError(e);
      setRunErrors((prev) => ({ ...prev, [adapterId]: failure }));
    } finally {
      setRunning(null);
    }
  }

  async function cancelProbe() {
    try {
      const signalled = await api.providerProbeCancel();
      setCancelHint(
        signalled
          ? "İptal istendi; alt süreç öldürülüyor ve kısmi çıktı raporlanacak."
          : "İptal edilecek bir koşu yok.",
      );
    } catch (e) {
      setError(toApiError(e));
    }
  }

  return (
    <div className="page" style={{ maxWidth: 920 }}>
      <div className="page-head">
        <h1>AI bağlantıları</h1>
        <p className="lead">
          Uygulama, sağlayıcı kimlik dosyalarını okumaz ve token kopyalamaz. Bu ekran araçların
          kurulu olup olmadığını yoklar; <strong>“Bağlantıyı sına” düğmesi aracı gerçekten
          çalıştırır</strong> (izinli argv, allowlist ortam, boş geçici iş dizini, 15 sn sınır,
          iptal). İstem gönderilmez, model çağrılmaz; AI bu sürümde kapalıdır.
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
          {rows.map((row) => {
            const result = results[row.id];
            const runError = runErrors[row.id];
            const status = result ? probeStatusLabel(result) : null;
            const warnings = result ? probeWarnings(result) : [];
            const outputs = result ? probeOutputs(result) : [];
            return (
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
                    Sınama düğmesi bu yüzden kapalı.
                  </div>
                ) : null}
                {row.state !== "installed" && !row.batchForbidden ? (
                  <div className="muted small" style={{ marginTop: "var(--space-2)" }}>
                    Araç PATH üzerinde bulunamadı ya da sürümü okunamadı; sınama çalıştırılamaz.
                  </div>
                ) : null}

                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: "var(--space-2)",
                    flexWrap: "wrap",
                    marginTop: "var(--space-3)",
                  }}
                >
                  <button
                    className="btn"
                    disabled={!probeCanRun(row, running)}
                    onClick={() => void runProbe(row.id)}
                  >
                    {running === row.id ? "Çalışıyor…" : "Bağlantıyı sına"}
                  </button>
                  {running === row.id ? (
                    <button className="btn ghost" onClick={() => void cancelProbe()}>
                      İptal
                    </button>
                  ) : null}
                  <span className="muted small">gerçek süreç · boş geçici dizin · 15 sn</span>
                </div>

                {runError ? (
                  <div className="notice danger" role="alert" style={{ marginTop: "var(--space-3)", fontSize: 12 }}>
                    <strong>{runError.code}</strong> — {runError.message}
                  </div>
                ) : null}

                {result && status ? (
                  <div
                    style={{
                      marginTop: "var(--space-3)",
                      borderTop: "1px solid var(--line)",
                      paddingTop: "var(--space-3)",
                    }}
                  >
                    <div style={{ display: "flex", gap: "var(--space-2)", alignItems: "center", flexWrap: "wrap" }}>
                      <span className={`badge ${status.tone}`}>{status.label}</span>
                      <span className="muted small">{probeSummary(result)}</span>
                    </div>
                    <dl className="kv" style={{ gridTemplateColumns: "88px 1fr", marginTop: "var(--space-2)" }}>
                      <dt>Komut</dt><dd style={MONO}>{probeArgv(result)}</dd>
                      <dt>Ortam</dt><dd style={MONO}>{result.envKeys.join(", ") || "—"}</dd>
                      <dt>İş dizini</dt>
                      <dd style={MONO}>
                        {result.workdir} <span className="muted">(boş; çalıştırmadan sonra silindi)</span>
                      </dd>
                    </dl>
                    {warnings.map((warning) => (
                      <div key={warning} className="notice" style={{ fontSize: 12, marginTop: "var(--space-2)" }}>
                        {warning}
                      </div>
                    ))}
                    {outputs.length === 0 ? (
                      <p className="muted small" style={{ margin: "var(--space-2) 0 0" }}>
                        Çıktı yok.
                      </p>
                    ) : (
                      outputs.map((output) => (
                        <div key={output.stream} style={{ marginTop: "var(--space-2)" }}>
                          <span className="muted small">{output.stream}</span>
                          <pre style={PRE}>{output.text}</pre>
                        </div>
                      ))
                    )}
                  </div>
                ) : null}
              </div>
            );
          })}
          {rows.length === 0 && !busy ? (
            <div className="empty"><p style={{ margin: 0 }}>Keşif sonucu yok.</p></div>
          ) : null}
        </div>
        {cancelHint ? (
          <div className="muted small" style={{ marginTop: "var(--space-3)" }}>
            {cancelHint}
          </div>
        ) : null}
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
