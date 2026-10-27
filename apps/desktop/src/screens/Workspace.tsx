import type { VaultStatus } from "../lib/api";
import { t } from "../lib/i18n";

export function Workspace({ status }: { status: VaultStatus }) {
  return (
    <div className="page">
      <div className="page-head">
        <div className="steps" aria-label={t("steps.aria")}>
          <span className="step done"><span className="step-dot">✓</span> {t("steps.vault")}</span>
          <span className="step done"><span className="step-dot">✓</span> {t("steps.recovery")}</span>
          <span className="step active"><span className="step-dot">3</span> {t("steps.start")}</span>
        </div>
        <h1>{t("workspace.title")}</h1>
        <p className="lead">{t("workspace.lead")}</p>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("workspace.cardTitle")}</h2>
            <p className="card-sub">{t("workspace.cardSub")}</p>
          </div>
          <span className="badge ok">
            <span className="dot ok" aria-hidden="true" /> {t("workspace.badgeOpen")}
          </span>
        </div>
        <dl className="kv">
          <dt>{t("workspace.idLabel")}</dt>
          <dd>{status.vaultId ?? t("common.unknown")}</dd>
          <dt>{t("workspace.pathLabel")}</dt>
          <dd style={{ fontFamily: "var(--mono)", fontSize: 12 }}>
            {status.vaultPath ?? t("common.unknown")}
          </dd>
          <dt>{t("workspace.encryptionLabel")}</dt>
          <dd>{t("workspace.encryptionValue")}</dd>
          <dt>{t("workspace.recoveryLabel")}</dt>
          <dd>{t("workspace.recoveryValue")}</dd>
        </dl>
      </div>

      <div className="card tight">
        <h2>{t("workspace.nextTitle")}</h2>
        <p className="muted small" style={{ margin: 0 }}>
          {t("workspace.nextBody")}
        </p>
      </div>
    </div>
  );
}
