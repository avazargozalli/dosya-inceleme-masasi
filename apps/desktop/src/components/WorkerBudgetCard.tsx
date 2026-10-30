import { useCallback, useEffect, useState } from "react";

import { api, toApiError, type ApiError, type WorkerBudgetProfile } from "../lib/api";
import { mibText, profileLimits, secondsText } from "../lib/budget";
import { t } from "../lib/i18n";

/**
 * §17.3 kaynak bütçesi kartı.
 *
 * Sınırlar **çekirdekten** gelir (`worker_budgets` → `crates/ingest/src/budget.rs`);
 * arayüz kendi sabitini yazmaz, hesaplamaz. Kart bir politika görünümüdür:
 * hangi worker'ın hangi sınırla koştuğunu kullanıcıya açıkça söyler.
 *
 * Metinler F04-T08 gereği `lib/i18n.ts`ten gelir.
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
          <h2>{t("workerBudget.title")}</h2>
          <p className="card-sub">
            {t("workerBudget.subLead")}
            <code>E_WORKER_BUDGET</code>
            {t("workerBudget.subTail")}
          </p>
        </div>
        <button className="btn" disabled={busy || !unlocked} onClick={() => void refresh()}>
          {t("workerBudget.refresh")}
        </button>
      </div>

      {!unlocked ? (
        <p className="muted small" style={{ margin: 0 }}>
          {t("workerBudget.locked")}
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
                <th>{t("workerBudget.colProfile")}</th>
                <th>{t("workerBudget.colCpu")}</th>
                <th>{t("workerBudget.colMemory")}</th>
                <th>{t("workerBudget.colOutput")}</th>
                <th>{t("workerBudget.colFiles")}</th>
                <th>{t("workerBudget.colTime")}</th>
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
                  <td>{t("budget.seconds", { value: Math.round(profile.wallClockSeconds) })}</td>
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
          {busy ? t("workerBudget.busy") : t("workerBudget.notFound")}
        </p>
      )}

      <p className="muted small" style={{ marginBottom: 0 }}>
        {t("workerBudget.limitLead")} <code>ulimit</code> {t("workerBudget.limitMiddle")}{" "}
        <code>E_WORKER_BUDGET</code> {t("workerBudget.limitTail")}
      </p>
    </div>
  );
}
