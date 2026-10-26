import { useCallback, useEffect, useState } from "react";

import { api, toApiError, type ApiError, type JobQueueView } from "../lib/api";
import {
  jobMetaLine,
  jobStateBadgeClass,
  jobStateLabel,
  jobTypeLabel,
  queueAttention,
  queueSummaryLine,
  recoveryLine,
} from "../lib/jobs";

/**
 * İş kuyruğu kartı: yalnız sayı ve durum gösterir; dosya adı, belge metni ve
 * sağlayıcı bilgisi taşımaz. "Bekleyen işleri kurtar" gerçek çekirdek
 * komutunu (`job_queue_recover`) çağırır — sadece görünen bir düğme değildir.
 */
export function JobQueueCard({ unlocked }: { unlocked: boolean }) {
  const [view, setView] = useState<JobQueueView | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!unlocked) {
      setView(null);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      setView(await api.jobQueueStatus());
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }, [unlocked]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const recover = useCallback(async () => {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const report = await api.jobQueueRecover();
      setMessage(recoveryLine(report.interrupted, report.cancelled));
      setView(await api.jobQueueStatus());
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }, []);

  const summary = view?.summary ?? null;
  const attention = summary ? queueAttention(summary) : null;

  return (
    <div className="card">
      <div className="card-head">
        <div>
          <h2>İş kuyruğu</h2>
          <p className="card-sub">
            Ayrıştırma ve uzun işler kalıcı kayıtta tutulur; uygulama kapanırsa yarıda
            kalan iş açılışta yeniden sıraya alınır.
          </p>
        </div>
        <button className="btn" disabled={busy || !unlocked} onClick={() => void refresh()}>
          Yenile
        </button>
      </div>

      {!unlocked ? (
        <p className="muted small" style={{ margin: 0 }}>
          Kuyruk kasa açılınca görünür.
        </p>
      ) : error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : summary ? (
        <>
          <p style={{ marginTop: 0 }}>{queueSummaryLine(summary)}</p>
          {attention ? <div className="notice">{attention}</div> : null}
          {view && view.jobs.length > 0 ? (
            <table className="table">
              <thead>
                <tr>
                  <th>Tür</th>
                  <th>Durum</th>
                  <th>Deneme</th>
                </tr>
              </thead>
              <tbody>
                {view.jobs.map((job) => (
                  <tr key={job.id}>
                    <td>{jobTypeLabel(job.jobType)}</td>
                    <td>
                      <span className={`badge ${jobStateBadgeClass(job.state)}`}>
                        {jobStateLabel(job.state)}
                      </span>
                    </td>
                    <td style={{ fontFamily: "var(--mono)", fontSize: 12 }}>
                      {jobMetaLine(job)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p className="muted small" style={{ margin: 0 }}>
              Kayıtlı iş yok.
            </p>
          )}
          <div className="row" style={{ justifyContent: "space-between", marginTop: 8 }}>
            <button className="btn" disabled={busy} onClick={() => void recover()}>
              Bekleyen işleri kurtar
            </button>
            <span className="muted small">
              Kurtarma, kirası dolmuş işleri yeniden sıraya alır; iptal istenmiş işleri kapatır.
            </span>
          </div>
          {message ? <div className="notice ok">{message}</div> : null}
          <p className="muted small" style={{ marginBottom: 0 }}>
            Sınır: kuyruk yürütücüsü tek süreçlidir (kasa başına tek yazıcı). Kuyruk
            gecikmesi ölçümü release derlemesinde yapılır; bu kart yalnız durumu gösterir.
          </p>
        </>
      ) : (
        <p className="muted small" style={{ margin: 0 }}>
          {busy ? "Kuyruk okunuyor…" : "Kuyruk okunamadı."}
        </p>
      )}
    </div>
  );
}
