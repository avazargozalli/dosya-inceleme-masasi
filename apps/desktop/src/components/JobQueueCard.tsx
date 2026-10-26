import { useCallback, useEffect, useState } from "react";

import { api, toApiError, type ApiError, type JobQueueView, type JobRow } from "../lib/api";
import {
  cancelOutcomeLine,
  jobActionLabel,
  jobActions,
  jobMetaLine,
  jobProgressLine,
  jobProgressPercent,
  jobProgressWidth,
  jobStateBadgeClass,
  jobStateLabel,
  jobTypeLabel,
  queueAttention,
  queueHasLiveJobs,
  queueSummaryLine,
  recoveryLine,
  type JobAction,
} from "../lib/jobs";

/** Canlı sayaç için kartın kendini tazeleme aralığı (iş yürürken). */
const LIVE_REFRESH_MS = 1200;

/**
 * İş kuyruğu kartı: yalnız sayı ve durum gösterir; dosya adı, belge metni ve
 * sağlayıcı bilgisi taşımaz. Bütün düğmeler gerçek çekirdek komutunu çağırır
 * (`job_queue_recover`, `job_cancel`, `job_retry`, `job_unblock`) — sadece
 * görünen bir düğme yoktur.
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

  /**
   * Sessiz tazeleme: canlı sayaç için kullanılır. Düğmeleri kilitlemez
   * (`busy` false kalır), böylece 1,2 sn'de bir tazeleme kullanıcıyı engellemez.
   */
  const loadQuiet = useCallback(async () => {
    if (!unlocked) return;
    try {
      setView(await api.jobQueueStatus());
      setError(null);
    } catch (e) {
      setError(toApiError(e));
    }
  }, [unlocked]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // İş yürürken kart kendini tazeler: sayaç `jobs.progress_*` sütunlarından
  // gelir (0022) ve ayrıştırma bitince kendiliğinden durur.
  const live = view ? queueHasLiveJobs(view.jobs) : false;
  useEffect(() => {
    if (!unlocked || !live) return;
    const timer = window.setInterval(() => {
      void loadQuiet();
    }, LIVE_REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [live, loadQuiet, unlocked]);

  const recover = useCallback(async () => {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const report = await api.jobQueueRecover();
      setMessage(recoveryLine(report));
      setView(await api.jobQueueStatus());
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }, []);

  const act = useCallback(
    async (job: JobRow, action: JobAction) => {
      setBusy(true);
      setError(null);
      setMessage(null);
      try {
        if (action === "cancel") {
          const result = await api.jobCancel(job.id);
          setMessage(cancelOutcomeLine(result));
        } else if (action === "retry") {
          await api.jobRetry(job.id);
          setMessage("İş yeniden sıraya alındı; yeni deneme numarası açılacak.");
        } else {
          await api.jobUnblock(job.id);
          setMessage("Engel kaldırıldı; iş yeniden sıraya alındı.");
        }
        setView(await api.jobQueueStatus());
      } catch (e) {
        setError(toApiError(e));
      } finally {
        setBusy(false);
      }
    },
    [],
  );

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
                  <th>İlerleme</th>
                  <th>Deneme</th>
                  <th>Eylem</th>
                </tr>
              </thead>
              <tbody>
                {view.jobs.map((job) => {
                  const actions = jobActions(job);
                  const progressLine = jobProgressLine(job);
                  const progressWidth = jobProgressWidth(job);
                  return (
                    <tr key={job.id}>
                      <td>{jobTypeLabel(job.jobType)}</td>
                      <td>
                        <span className={`badge ${jobStateBadgeClass(job.state)}`}>
                          {jobStateLabel(job.state)}
                        </span>
                      </td>
                      <td>
                        {progressLine === null ? (
                          <span className="muted small">—</span>
                        ) : (
                          <div style={{ minWidth: 170 }} aria-live="polite">
                            <span className="muted small">{progressLine}</span>
                            {progressWidth !== null ? (
                              <div
                                role="progressbar"
                                aria-valuemin={0}
                                aria-valuemax={100}
                                aria-valuenow={progressWidth}
                                aria-label={`İş ilerlemesi: ${progressLine}`}
                                style={{
                                  height: 4,
                                  borderRadius: 2,
                                  background: "var(--line)",
                                  overflow: "hidden",
                                  marginTop: 4,
                                }}
                              >
                                <div
                                  style={{
                                    width: `${progressWidth}%`,
                                    height: "100%",
                                    background: "var(--accent)",
                                  }}
                                />
                              </div>
                            ) : null}
                          </div>
                        )}
                      </td>
                      <td style={{ fontFamily: "var(--mono)", fontSize: 12 }}>
                        {jobMetaLine(job)}
                      </td>
                      <td>
                        {actions.length === 0 ? (
                          <span className="muted small">—</span>
                        ) : (
                          <div className="row" style={{ gap: 4 }}>
                            {actions.map((action) => (
                              <button
                                key={action}
                                className="btn"
                                disabled={busy}
                                onClick={() => void act(job, action)}
                              >
                                {jobActionLabel(action)}
                              </button>
                            ))}
                          </div>
                        )}
                      </td>
                    </tr>
                  );
                })}
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
              Kurtarma, kesintiye uğramış işleri yeniden sıraya alır; iptal istenmiş
              işleri kapatır.
            </span>
          </div>
          {message ? <div className="notice ok">{message}</div> : null}
          <p className="muted small" style={{ marginBottom: 0 }}>
            Sınır: kuyruk yürütücüsü tek süreçlidir (kasa başına tek yazıcı). Parola
            bekleyen belge işi "Engelli" kalır; parolayla yeniden ayrıştırınca sıraya
            döner. İlerleme sütunu ayrıştırma sırasında yazılır (en çok 0,3 sn'de bir
            güncellenir, kart 1,2 sn'de bir tazelenir) — çok kısa belgede sayaç
            0'dan %100'e atlayabilir; bitmiş işte sayaç gösterilmez. Kuyruk gecikmesi
            ölçümü release derlemesinde yapılır.
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
