import { useCallback, useEffect, useState } from "react";

import { api, toApiError, type ApiError, type WorkerBudgetProfile } from "../lib/api";
import { mibText, profileLimits, secondsText } from "../lib/budget";

/**
 * §17.3 kaynak bütçesi kartı.
 *
 * Sınırlar **çekirdekten** gelir (`worker_budgets` → `crates/ingest/src/budget.rs`);
 * arayüz kendi sabitini yazmaz, hesaplamaz. Kart bir politika görünümüdür:
 * hangi worker'ın hangi sınırla koştuğunu kullanıcıya açıkça söyler.
 */
export function WorkerBudgetCard({ unlocked }: { unlocked: boolean }) {
  const [profiles, setProfiles] = useState<WorkerBudgetProfile[] | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    if (!unlocked) {
      setProfiles(null);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      setProfiles(await api.workerBudgets());
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }, [unlocked]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return (
    <div className="card">
      <div className="card-head">
        <div>
          <h2>Kaynak bütçeleri</h2>
          <p className="card-sub">
            Her worker türü kendi CPU, bellek, çıktı, açık dosya ve süre sınırıyla koşar;
            aşım ayrı hata koduyla (<code>E_WORKER_BUDGET</code>) raporlanır.
          </p>
        </div>
        <button className="btn" disabled={busy || !unlocked} onClick={() => void refresh()}>
          Yenile
        </button>
      </div>

      {!unlocked ? (
        <p className="muted small" style={{ margin: 0 }}>
          Bütçe profilleri kasa açılınca okunur.
        </p>
      ) : error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : profiles && profiles.length > 0 ? (
        <>
          <table className="table">
            <thead>
              <tr>
                <th>Profil</th>
                <th>CPU</th>
                <th>Bellek</th>
                <th>Çıktı</th>
                <th>Dosya</th>
                <th>Süre</th>
              </tr>
            </thead>
            <tbody>
              {profiles.map((profile) => (
                <tr key={profile.kind}>
                  <td>{profile.label}</td>
                  <td>{secondsText(profile.cpuSeconds)}</td>
                  <td>{mibText(profile.memoryBytes)}</td>
                  <td>{mibText(profile.outputBytes)}</td>
                  <td>{profile.openFiles}</td>
                  <td>{Math.round(profile.wallClockSeconds)} sn</td>
                </tr>
              ))}
            </tbody>
          </table>
          {profiles.map((profile) => (
            <p key={`line-${profile.kind}`} className="muted small" style={{ margin: "4px 0 0" }}>
              <strong>{profile.label}:</strong> {profileLimits(profile)}
            </p>
          ))}
        </>
      ) : (
        <p className="muted small" style={{ margin: 0 }}>
          {busy ? "Profiller okunuyor…" : "Profil bulunamadı."}
        </p>
      )}

      <p className="muted small" style={{ marginBottom: 0 }}>
        Sınır: CPU süresi ve açık dosya sayısı POSIX kabuğunda <code>ulimit</code> ile
        uygulanır — Windows'ta bu ikisi uygulanmaz (süre ve çıktı bütçesi uygulanır).
        Bellek gözcüsü 500 ms'de bir örnek alır; sınır örnekler arasında aşılabilir,
        aşımda süreç durdurulur ve iş <code>E_WORKER_BUDGET</code> ile kapanır. AI
        profilinde CPU sınırı yoktur: çağrı yerel süreç değildir, duvar saati ve yanıt
        boyutu ölçülür. Bu değerler başlangıç ayarlarıdır; F15 ölçümüyle güncellenir.
      </p>
    </div>
  );
}
