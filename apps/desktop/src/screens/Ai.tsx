import { useEffect, useState } from "react";
import {
  api,
  toApiError,
  type AiCandidateRow,
  type AiOverview,
  type ApiError,
  type CandidateReport,
  type CaseRow,
  type ConsentPreview,
  type TransferConsentRow,
} from "../lib/api";
import { formatDateTime, formatSize } from "../lib/format";
import { t } from "../lib/i18n";
import { consentScopeLine, skippedSummary } from "../lib/transferScope";
import {
  CANDIDATE_KINDS,
  candidateKindLabel,
  candidateStateLabel,
  candidateStateTone,
  draftJsonError,
  errorCount,
  parseIssues,
  promoteBlockedReason,
  quoteLineStateLabel,
  quoteLineTone,
  quotePositionLabel,
  quoteStateLabel,
  quoteStateTone,
  quoteSummary,
  warningCount,
} from "../lib/candidate";

export function Ai() {
  const [cases, setCases] = useState<CaseRow[]>([]);
  const [caseId, setCaseId] = useState("");
  const [overview, setOverview] = useState<AiOverview | null>(null);
  const [consent, setConsent] = useState<TransferConsentRow | null>(null);
  const [preview, setPreview] = useState<ConsentPreview | null>(null);
  const [candidates, setCandidates] = useState<AiCandidateRow[]>([]);
  const [purpose, setPurpose] = useState("ozet");
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

  // F11-T05 — aday doğrulama paneli
  const [runId, setRunId] = useState("");
  const [kind, setKind] = useState("claim_candidate");
  const [draft, setDraft] = useState(
    CANDIDATE_KINDS.find((option) => option.value === "claim_candidate")!.template,
  );
  const [report, setReport] = useState<CandidateReport | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const rows = await api.caseList(false);
        setCases(rows);
        if (rows.length > 0) setCaseId((prev) => prev || rows[0]!.id);
      } catch (e) {
        setError(toApiError(e));
      }
    })();
  }, []);

  useEffect(() => {
    if (!caseId) return;
    void api
      .aiOverview(caseId)
      .then(setOverview)
      .catch((e) => setError(toApiError(e)));
  }, [caseId]);

  const provider = overview?.providers.find((p) => p.providerType !== "off") ?? null;
  const offOnly = overview !== null && overview.providers.every((p) => p.providerType === "off");
  const selectedKind = CANDIDATE_KINDS.find((option) => option.value === kind) ?? CANDIDATE_KINDS[0]!;
  const draftError = draftJsonError(draft);

  async function approve() {
    if (!provider) return;
    setBusy(true);
    setError(null);
    try {
      // Onay **somut belge kümesine** bağlanır: kullanıcı neyi onayladıysa
      // gönderim yalnız onu kapsar; sonradan belge eklenirse yeniden onay gerekir.
      const docs = await api.documentList(caseId);
      const ids = docs.map((doc) => doc.id);
      setPreview(await api.aiConsentPreview(caseId, ids));
      setConsent(
        await api.aiConsentCreate(caseId, `${provider.adapterId}@${provider.policyVersion}`, purpose, ids),
      );
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function start() {
    if (!provider || !consent) return;
    setBusy(true);
    setError(null);
    try {
      // Gönderim kapsamı **onaydan** çözülür; burada belge listesi verilmez.
      await api.aiRunExecute(caseId, provider.id, consent.id, "summarize");
      setOverview(await api.aiOverview(caseId));
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function loadCandidates(id: string) {
    try {
      setCandidates(await api.aiCandidatesList(id));
    } catch (e) {
      setError(toApiError(e));
    }
  }

  async function selectRun(id: string) {
    setRunId(id);
    setReport(null);
    setNotice(null);
    await loadCandidates(id);
  }

  async function validateDraft(persist: boolean) {
    if (!runId || draftError) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const result = await api.aiCandidateValidate(runId, kind, draft);
      setReport(result);
      if (persist) {
        await api.aiCandidateRecord(runId, kind, draft);
        await loadCandidates(runId);
        setNotice(
          result.state === "valid"
            ? t("ai.noticeValid")
            : result.state === "partial"
              ? t("ai.noticePartial")
              : t("ai.noticeInvalid"),
        );
      }
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function revalidate(candidateId: string) {
    setBusy(true);
    setError(null);
    try {
      const refreshed = await api.aiCandidateRevalidate(candidateId);
      setCandidates((rows) =>
        rows.map((row) => (row.id === refreshed.id ? refreshed : row)),
      );
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function promote(candidateId: string) {
    setError(null);
    setNotice(null);
    try {
      await api.aiCandidatePromote(candidateId, caseId);
      setNotice(t("ai.noticePromoted"));
    } catch (e) {
      setError(toApiError(e));
    }
  }

  return (
    <div className="page" style={{ maxWidth: 1000 }}>
      <div className="page-head">
        <h1>{t("nav.aiReview")}</h1>
        <p className="lead">{t("ai.lead")}</p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      {notice ? <div className="notice ok">{notice}</div> : null}

      <div className="card">
        <div className="field" style={{ maxWidth: 420 }}>
          <label htmlFor="ai-case">{t("common.case")}</label>
          <select id="ai-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
            {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
          </select>
        </div>

        {offOnly ? (
          <div className="notice warn">
            <strong>{t("ai.offTitle")}</strong> {t("ai.offLead")} <code>off</code>.{" "}
            {t("ai.offTail")} <code>E_AI_OFF</code> {t("ai.offEnd")}
          </div>
        ) : null}

        <dl className="kv">
          <dt>{t("ai.provider")}</dt>
          <dd>{provider ? `${provider.adapterId} (${provider.providerType})` : t("ai.providerNone")}</dd>
          <dt>{t("ai.coverage")}</dt>
          <dd>
            {overview
              ? t("ai.coverageLine", {
                  documents: overview.coverage.documents,
                  pages: overview.coverage.pages,
                  ocrPages: overview.coverage.ocrPages,
                  emptyPages: overview.coverage.emptyPages,
                  unparsed: overview.coverage.unparsedDocuments,
                })
              : t("common.unknown")}
          </dd>
        </dl>

        <div className="row">
          <div className="field" style={{ maxWidth: 220 }}>
            <label htmlFor="ai-purpose">{t("ai.purpose")}</label>
            <select id="ai-purpose" className="input" value={purpose} onChange={(e) => setPurpose(e.target.value)}>
              <option value="ozet">{t("ai.purposeSummary")}</option>
              <option value="olay_cikarimi">{t("ai.purposeEvents")}</option>
              <option value="iddia_cikarimi">{t("ai.purposeClaims")}</option>
            </select>
          </div>
          <div className="row" style={{ alignItems: "flex-end", gap: "var(--space-2)" }}>
            <button className="btn" disabled={!provider || busy} onClick={() => void approve()}>
              {t("ai.approve")}
            </button>
            <button
              className="btn ghost"
              onClick={async () => {
                try {
                  const docs = await api.documentList(caseId);
                  setPreview(await api.aiConsentPreview(caseId, docs.map((doc) => doc.id)));
                } catch {
                  setPreview(null);
                }
              }}
            >
              {t("ai.showContent")}
            </button>
            <button className="btn primary" disabled={!provider || !consent || busy} onClick={() => void start()}>
              {t("ai.run")}
            </button>
          </div>
        </div>

        {preview ? (
          <div className="card" style={{ marginTop: 8 }}>
            <div className="card-head">
              <div>
                <h3>{t("ai.previewTitle")}</h3>
                <p className="card-sub">
                  {t("ai.previewSub", {
                    documents: preview.documents,
                    segments: preview.segments,
                    bytes: formatSize(preview.bytes),
                  })}
                </p>
              </div>
            </div>
            {skippedSummary(preview.skipped) ? (
              <p className="hint" style={{ margin: 0 }}>
                {skippedSummary(preview.skipped)} {t("ai.skippedSuffix")}
              </p>
            ) : null}
            <pre style={{ maxHeight: 140, overflow: "auto", whiteSpace: "pre-wrap", fontSize: 12 }}>
              {preview.sample || t("ai.previewEmpty")}
            </pre>
          </div>
        ) : null}
        {consent ? (
          <p className="hint" style={{ margin: 0 }}>
            {consentScopeLine(consent.selectedDocumentIds.length, preview)}{" · "}
            {t("ai.consentTail", {
              id: consent.id.slice(0, 8),
              provider: consent.providerSnapshot,
            })}
            <button
              className="btn ghost"
              style={{ marginLeft: 8 }}
              onClick={() =>
                void api
                  .aiConsentRevoke(consent.id)
                  .then(() => setConsent(null))
                  .catch((e) => setError(toApiError(e)))
              }
            >
              {t("ai.revoke")}
            </button>
          </p>
        ) : null}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("ai.runsTitle")}</h2>
            <p className="card-sub">{t("ai.runsSub")}</p>
          </div>
        </div>
        {overview && overview.runs.length > 0 ? (
          <table className="table">
            <thead>
              <tr>
                <th>{t("ai.colKind")}</th>
                <th>{t("ai.colState")}</th>
                <th>{t("ai.colStarted")}</th>
                <th>{t("ai.colCandidates")}</th>
              </tr>
            </thead>
            <tbody>
              {overview.runs.map((run) => (
                <tr key={run.id}>
                  <td>{run.taskType}</td>
                  <td><span className={`badge ${run.status === "succeeded" ? "ok" : ""}`}>{run.status}</span></td>
                  <td>{formatDateTime(run.createdAt)}</td>
                  <td>
                    <button className="btn ghost" onClick={() => void selectRun(run.id)}>
                      {runId === run.id ? t("ai.selectedRun") : t("ai.selectRun")}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p className="hint" style={{ margin: 0 }}>{t("ai.runsEmpty")}</p>
        )}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("ai.validateTitle")}</h2>
            <p className="card-sub">{t("ai.validateSub")}</p>
          </div>
        </div>

        {runId ? (
          <p className="hint" style={{ marginTop: 0 }}>
            {t("ai.currentRun")} <code>{runId.slice(0, 8)}…</code>
          </p>
        ) : (
          <div className="notice warn">{t("ai.noRun")}</div>
        )}

        <div className="row">
          <div className="field" style={{ maxWidth: 260 }}>
            <label htmlFor="ai-kind">{t("ai.kindLabel")}</label>
            <select
              id="ai-kind"
              className="input"
              value={kind}
              onChange={(e) => {
                const next = e.target.value;
                setKind(next);
                setReport(null);
                const option = CANDIDATE_KINDS.find((item) => item.value === next);
                if (option) setDraft(option.template);
              }}
            >
              {CANDIDATE_KINDS.map((option) => (
                <option key={option.value} value={option.value}>{option.label}</option>
              ))}
            </select>
          </div>
          <p className="hint" style={{ alignSelf: "flex-end", margin: 0 }}>{selectedKind.hint}</p>
        </div>

        <div className="field">
          <label htmlFor="ai-draft">{t("ai.draftLabel")}</label>
          <textarea
            id="ai-draft"
            className="input"
            style={{ minHeight: 180, fontFamily: "var(--mono)", fontSize: 12 }}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
          />
        </div>

        {draftError ? <p className="hint" style={{ color: "var(--danger)" }}>{draftError}</p> : null}

        <div className="row">
          <button
            className="btn"
            disabled={!runId || busy || draftError !== null}
            onClick={() => void validateDraft(false)}
          >
            {t("ai.validate")}
          </button>
          <button
            className="btn primary"
            disabled={!runId || busy || draftError !== null}
            onClick={() => void validateDraft(true)}
          >
            {t("ai.validateAndSave")}
          </button>
        </div>

        {report ? (
          <div style={{ marginTop: "var(--space-3)" }}>
            <div className="row" style={{ gap: 8, alignItems: "center" }}>
              <span className={`badge ${candidateStateTone(report.state)}`}>
                {candidateStateLabel(report.state)}
              </span>
              <span className={`badge ${quoteStateTone(report.quoteState)}`}>
                {quoteStateLabel(report.quoteState)}
              </span>
              <span className="badge plain">{quoteSummary(report)}</span>
              {errorCount(report.issues) > 0 ? (
                <span className="badge danger">
                  {t("ai.errorBadge", { count: errorCount(report.issues) })}
                </span>
              ) : null}
              {warningCount(report.issues) > 0 ? (
                <span className="badge warn">
                  {t("ai.warningBadge", { count: warningCount(report.issues) })}
                </span>
              ) : null}
            </div>

            {report.quotes.length > 0 ? (
              <table className="table" style={{ marginTop: "var(--space-2)" }}>
                <thead>
                  <tr>
                    <th>#</th>
                    <th>{t("ai.colQuote")}</th>
                    <th>{t("ai.colState")}</th>
                    <th>{t("ai.colPosition")}</th>
                    <th>{t("ai.colOccurrences")}</th>
                    <th>{t("ai.colText")}</th>
                  </tr>
                </thead>
                <tbody>
                  {report.quotes.map((quote) => (
                    <tr key={`${quote.documentVersionId}-${quote.index}`}>
                      <td>{quote.index + 1}</td>
                      <td style={{ maxWidth: 320 }}>“{quote.quote.slice(0, 120)}”</td>
                      <td>
                        <span className={`badge ${quoteLineTone(quote.state)}`}>
                          {quoteLineStateLabel(quote)}
                        </span>
                      </td>
                      <td>{quotePositionLabel(quote)}</td>
                      <td>{quote.occurrences}</td>
                      <td>
                        {quote.textOrigin ?? t("common.unknown")}
                        {quote.claimedPageIndex !== null && quote.pageIndex !== null
                          && quote.claimedPageIndex !== quote.pageIndex ? (
                          <span className="hint" style={{ display: "block" }}>
                            {t("ai.claimedPage", { page: quote.claimedPageIndex + 1 })}
                          </span>
                        ) : null}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : null}

            {report.issues.length > 0 ? (
              <details style={{ marginTop: "var(--space-2)" }} open>
                <summary>{t("ai.issuesSummary", { count: report.issues.length })}</summary>
                <ul className="hint" style={{ margin: "var(--space-1) 0 0", paddingLeft: 18 }}>
                  {report.issues.map((issue, index) => (
                    <li key={`${issue.code}-${issue.field}-${index}`}>
                      <code>{issue.field}</code> · {issue.code} — {issue.detail}
                    </li>
                  ))}
                </ul>
              </details>
            ) : null}

            {report.notes.length > 0 ? (
              <ul className="hint" style={{ margin: "var(--space-2) 0 0", paddingLeft: 18 }}>
                {report.notes.map((note, index) => <li key={index}>{note}</li>)}
              </ul>
            ) : null}

            <p className="hint" style={{ marginBottom: 0 }}>{t("ai.limits")}</p>
          </div>
        ) : null}
      </div>

      {candidates.length > 0 ? (
        <div className="card">
          <div className="card-head">
            <div>
              <h2>{t("ai.candidatesTitle")}</h2>
              <p className="card-sub">{t("ai.candidatesSub")}</p>
            </div>
          </div>
          <table className="table">
            <thead>
              <tr>
                <th>{t("ai.colKind")}</th>
                <th>{t("ai.colSchema")}</th>
                <th>{t("ai.colQuote")}</th>
                <th>{t("ai.colContent")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {candidates.map((candidate) => {
                const blocked = promoteBlockedReason(candidate);
                const issues = parseIssues(candidate.validationErrors);
                return (
                  <tr key={candidate.id}>
                    <td>
                      {candidateKindLabel(candidate.candidateType)}
                      <span className="hint" style={{ display: "block" }}>
                        {candidate.checkedAt
                          ? formatDateTime(candidate.checkedAt)
                          : t("ai.neverChecked")}
                      </span>
                    </td>
                    <td>
                      <span className={`badge ${candidateStateTone(candidate.schemaState)}`}>
                        {candidateStateLabel(candidate.schemaState)}
                      </span>
                      <span className={`badge ${candidateStateTone(candidate.validationState)}`} style={{ marginLeft: 4 }}>
                        {t("ai.candidateBadge", {
                          state: candidateStateLabel(candidate.validationState),
                        })}
                      </span>
                    </td>
                    <td>
                      <span className={`badge ${quoteStateTone(candidate.quoteState)}`}>
                        {quoteStateLabel(candidate.quoteState)}
                      </span>
                      {candidate.quoteReport.length > 0 ? (
                        <details style={{ marginTop: 4 }}>
                          <summary className="hint">
                            {t("ai.quotesToggle", { count: candidate.quoteReport.length })}
                          </summary>
                          <ul className="hint" style={{ margin: "4px 0 0", paddingLeft: 16 }}>
                            {candidate.quoteReport.map((quote) => (
                              <li key={`${quote.documentVersionId}-${quote.index}`}>
                                <span className={`badge ${quoteLineTone(quote.state)}`}>
                                  {quoteLineStateLabel(quote)}
                                </span>{" "}
                                {quotePositionLabel(quote)} · “{quote.quote.slice(0, 48)}”
                              </li>
                            ))}
                          </ul>
                        </details>
                      ) : null}
                    </td>
                    <td style={{ fontFamily: "var(--mono)", fontSize: 11 }}>
                      {candidate.structuredPayload.slice(0, 80)}
                      {issues.length > 0 ? (
                        <details style={{ marginTop: 4 }}>
                          <summary className="hint">
                            {t("ai.issuesToggle", { count: issues.length })}
                          </summary>
                          <ul className="hint" style={{ margin: "4px 0 0", paddingLeft: 16 }}>
                            {issues.map((issue, index) => (
                              <li key={`${issue.code}-${index}`}>
                                <code>{issue.field}</code> · {issue.code}
                              </li>
                            ))}
                          </ul>
                        </details>
                      ) : null}
                    </td>
                    <td>
                      <div className="row" style={{ gap: 6 }}>
                        <button className="btn ghost" disabled={busy} onClick={() => void revalidate(candidate.id)}>
                          {t("ai.revalidate")}
                        </button>
                        <button
                          className="btn ghost"
                          disabled={busy || blocked !== null}
                          title={blocked ?? t("ai.promoteTitle")}
                          onClick={() => void promote(candidate.id)}
                        >
                          {t("ai.promote")}
                        </button>
                      </div>
                      {blocked ? <span className="hint">{blocked}</span> : null}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      ) : null}
    </div>
  );
}
