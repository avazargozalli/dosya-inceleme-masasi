import type { VaultStatus } from "../lib/api";

export function Workspace({ status }: { status: VaultStatus }) {
  return (
    <div className="page">
      <div className="page-head">
        <div className="steps" aria-label="Kurulum adımları">
          <span className="step done"><span className="step-dot">✓</span> Kasa</span>
          <span className="step done"><span className="step-dot">✓</span> Kurtarma</span>
          <span className="step active"><span className="step-dot">3</span> Kullanmaya başla</span>
        </div>
        <h1>Kasa açık</h1>
        <p className="lead">
          Şifreli kasa hazır. Bu fazda belge içe aktarma kapalı; F05 ile dosya ve sürüm yönetimi
          açılacak. Aşağıdaki bilgiler gerçek kasa başlığından okunur.
        </p>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Kasa bilgisi</h2>
            <p className="card-sub">Başlık sürümü ve konum; içerik değil.</p>
          </div>
          <span className="badge ok"><span className="dot ok" aria-hidden="true" /> açık</span>
        </div>
        <dl className="kv">
          <dt>Kasa kimliği</dt><dd>{status.vaultId ?? "—"}</dd>
          <dt>Konum</dt><dd style={{ fontFamily: "var(--mono)", fontSize: 12 }}>{status.vaultPath ?? "—"}</dd>
          <dt>Şifreleme</dt><dd>SQLCipher hedefi · blob AEAD etkin · başlık v1</dd>
          <dt>Kurtarma</dt><dd>parola yuvası + kurtarma yuvası ayrı sarılmış</dd>
        </dl>
      </div>

      <div className="card tight">
        <h2>Sıradaki</h2>
        <p className="muted small" style={{ margin: 0 }}>
          F03: SQLCipher şeması, migrasyonlar, kilit ve yedek. F04: dosya çalışma alanı.
        </p>
      </div>
    </div>
  );
}
