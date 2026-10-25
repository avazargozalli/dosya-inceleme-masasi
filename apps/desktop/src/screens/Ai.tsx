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
      setConsent(
        await api.aiConsentCreate(caseId, `${provider.adapterId}@${provider.policyVersion}`, purpose, []),
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
      const docs = await api.documentList(caseId);
      await api.aiRunExecute(
        caseId,
        provider.id,
        consent.id,
        "summarize",
        docs.map((doc) => doc.id),
      );
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
            ? "Aday doğrulandı ve kaydedildi (geçerli)."
            : result.state === "partial"
              ? "Aday kaydedildi; kısmi — kaynakları eksik veya alıntılar tam doğrulanmadı."
              : "Aday kaydedildi ama geçersiz; incelemeye gönderilemez.",
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
      setNotice("Aday inceleme kuyruğuna gönderildi; kabul edilene kadar hukuki kayıt değil.");
    } catch (e) {
      setError(toApiError(e));
    }
  }

  return (
    <div className="page" style={{ maxWidth: 1000 }}>
      <div className="page-head">
        <h1>AI inceleme</h1>
        <p className="lead">
          AI yalnız aday üretir. İçerik cihazdan ancak dosya kapsamlı, süreli ve iptal edilebilir
          bir aktarım onayıyla çıkar; her gönderim makbuzla kayda alınır.
        </p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      {notice ? <div className="notice ok">{notice}</div> : null}

      <div className="card">
        <div className="field" style={{ maxWidth: 420 }}>
          <label htmlFor="ai-case">Dosya</label>
          <select id="ai-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
            {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
          </select>
        </div>

        {offOnly ? (
          <div className="notice warn">
            <strong>AI kapalı.</strong> Kayıtlı sağlayıcı yalnız <code>off</code>. Bu durumda hiçbir
            içerik gönderilmez; <code>E_AI_OFF</code> döner.
          </div>
        ) : null}

        <dl className="kv">
          <dt>Sağlayıcı</dt>
          <dd>{provider ? `${provider.adapterId} (${provider.providerType})` : "yok — yalnız off"}</dd>
          <dt>Kapsam</dt>
          <dd>
            {overview
              ? `${overview.coverage.documents} belge · ${overview.coverage.pages} sayfa · ` +
                `${overview.coverage.ocrPages} OCR · ${overview.coverage.emptyPages} boş sayfa · ` +
                `${overview.coverage.unparsedDocuments} ayrıştırılmamış`
              : "—"}
          </dd>
        </dl>

        <div className="row">
          <div className="field" style={{ maxWidth: 220 }}>
            <label htmlFor="ai-purpose">Amaç</label>
            <select id="ai-purpose" className="input" value={purpose} onChange={(e) => setPurpose(e.target.value)}>
              <option value="ozet">Özet</option>
              <option value="olay_cikarimi">Olay çıkarımı</option>
              <option value="iddia_cikarimi">İddia çıkarımı</option>
            </select>
          </div>
          <div className="row" style={{ alignItems: "flex-end", gap: "var(--space-2)" }}>
            <button className="btn" disabled={!provider || busy} onClick={() => void approve()}>
              Aktarım onayı ver
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
              Gönderilecek içeriği göster
            </button>
            <button className="btn primary" disabled={!provider || !consent || busy} onClick={() => void start()}>
              Çalıştır
            </button>
          </div>
        </div>

        {preview ? (
          <div className="card" style={{ marginTop: 8 }}>
            <div className="card-head">
              <div>
                <h3>Gönderilecek içerik</h3>
                <p className="card-sub">
                  {preview.documents} belge · {preview.segments} segment ·{" "}
                  {(preview.bytes / 1024).toFixed(1)} KB
                </p>
              </div>
            </div>
            <pre style={{ maxHeight: 140, overflow: "auto", whiteSpace: "pre-wrap", fontSize: 12 }}>
              {preview.sample || "ayrıştırılmış metin yok — bu dosyada gönderilecek içerik bulunmuyor"}
            </pre>
          </div>
        ) : null}
        {consent ? (
          <p className="hint" style={{ margin: 0 }}>
            Onay {consent.id.slice(0, 8)}… · {consent.providerSnapshot} · iptal edilene kadar geçerli.
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
              İptal et
            </button>
          </p>
        ) : null}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Çalıştırmalar</h2>
            <p className="card-sub">Durum, sağlayıcı ve kullanım raporu · aday doğrulaması seçili çalıştırmada</p>
          </div>
        </div>
        {overview && overview.runs.length > 0 ? (
          <table className="table">
            <thead>
              <tr><th>Tür</th><th>Durum</th><th>Başlangıç</th><th>Adaylar</th></tr>
            </thead>
            <tbody>
              {overview.runs.map((run) => (
                <tr key={run.id}>
                  <td>{run.taskType}</td>
                  <td><span className={`badge ${run.status === "succeeded" ? "ok" : ""}`}>{run.status}</span></td>
                  <td>{new Date(run.createdAt).toLocaleString("tr-TR")}</td>
                  <td>
                    <button className="btn ghost" onClick={() => void selectRun(run.id)}>
                      {runId === run.id ? "Seçili · adayları yenile" : "Seç ve adayları gör"}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p className="hint" style={{ margin: 0 }}>Henüz çalıştırma yok. Sağlayıcı bağlanınca burada görünür.</p>
        )}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Aday doğrula ve kaydet</h2>
            <p className="card-sub">
              Yük şemaya göre doğrulanır, alıntılar belgenin gerçek metninde aranır; durum
              hesaplanır — elle “geçerli” yazılamaz.
            </p>
          </div>
        </div>

        {runId ? (
          <p className="hint" style={{ marginTop: 0 }}>
            Seçili çalıştırma: <code>{runId.slice(0, 8)}…</code>
          </p>
        ) : (
          <div className="notice warn">
            Önce yukarıdaki tablodan bir çalıştırma seçin; aday o çalıştırmaya bağlanır.
          </div>
        )}

        <div className="row">
          <div className="field" style={{ maxWidth: 260 }}>
            <label htmlFor="ai-kind">Aday türü</label>
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
          <label htmlFor="ai-draft">Aday yükü (JSON)</label>
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
            Doğrula
          </button>
          <button
            className="btn primary"
            disabled={!runId || busy || draftError !== null}
            onClick={() => void validateDraft(true)}
          >
            Doğrula ve kaydet
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
                <span className="badge danger">{errorCount(report.issues)} hata</span>
              ) : null}
              {warningCount(report.issues) > 0 ? (
                <span className="badge warn">{warningCount(report.issues)} uyarı</span>
              ) : null}
            </div>

            {report.quotes.length > 0 ? (
              <table className="table" style={{ marginTop: "var(--space-2)" }}>
                <thead>
                  <tr><th>#</th><th>Alıntı</th><th>Durum</th><th>Konum</th><th>Tekrar</th><th>Metin</th></tr>
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
                        {quote.textOrigin ?? "—"}
                        {quote.claimedPageIndex !== null && quote.pageIndex !== null
                          && quote.claimedPageIndex !== quote.pageIndex ? (
                          <span className="hint" style={{ display: "block" }}>
                            model sayfa {quote.claimedPageIndex + 1} dedi
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
                <summary>Doğrulama sorunları ({report.issues.length})</summary>
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

            <p className="hint" style={{ marginBottom: 0 }}>
              Doğrulamanın sınırları: yalnız kasada ayrıştırılmış metin aranır; OCR yazım hatası
              eşleşmeyi etkileyebilir, maskeli bağlamın özgün kaynağa geri eşlenmesi bu adımda
              yapılmaz. Aday metni modelin ürettiği hâliyle saklanır.
            </p>
          </div>
        ) : null}
      </div>

      {candidates.length > 0 ? (
        <div className="card">
          <div className="card-head">
            <div>
              <h2>Adaylar</h2>
              <p className="card-sub">Kabul edilene kadar hukuki kayıt sayılmaz</p>
            </div>
          </div>
          <table className="table">
            <thead>
              <tr><th>Tür</th><th>Şema</th><th>Alıntı</th><th>İçerik</th><th /></tr>
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
                          ? new Date(candidate.checkedAt).toLocaleString("tr-TR")
                          : "denetlenmemiş"}
                      </span>
                    </td>
                    <td>
                      <span className={`badge ${candidateStateTone(candidate.schemaState)}`}>
                        {candidateStateLabel(candidate.schemaState)}
                      </span>
                      <span className={`badge ${candidateStateTone(candidate.validationState)}`} style={{ marginLeft: 4 }}>
                        aday: {candidateStateLabel(candidate.validationState)}
                      </span>
                    </td>
                    <td>
                      <span className={`badge ${quoteStateTone(candidate.quoteState)}`}>
                        {quoteStateLabel(candidate.quoteState)}
                      </span>
                      {candidate.quoteReport.length > 0 ? (
                        <details style={{ marginTop: 4 }}>
                          <summary className="hint">{candidate.quoteReport.length} alıntı</summary>
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
                          <summary className="hint">{issues.length} sorun</summary>
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
                          Yeniden doğrula
                        </button>
                        <button
                          className="btn ghost"
                          disabled={busy || blocked !== null}
                          title={blocked ?? "İnceleme kuyruğuna gönder"}
                          onClick={() => void promote(candidate.id)}
                        >
                          İncelemeye gönder
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
