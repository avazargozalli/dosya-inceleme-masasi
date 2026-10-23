import { useEffect, useState } from "react";
import {
  api,
  toApiError,
  type AiCandidateRow,
  type AiOverview,
  type ApiError,
  type CaseRow,
  type TransferConsentRow,
} from "../lib/api";

export function Ai() {
  const [cases, setCases] = useState<CaseRow[]>([]);
  const [caseId, setCaseId] = useState("");
  const [overview, setOverview] = useState<AiOverview | null>(null);
  const [consent, setConsent] = useState<TransferConsentRow | null>(null);
  const [candidates, setCandidates] = useState<AiCandidateRow[]>([]);
  const [purpose, setPurpose] = useState("ozet");
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

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
      await api.aiRunStart(caseId, provider.id, consent.id, "summarize");
      setOverview(await api.aiOverview(caseId));
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function loadCandidates(runId: string) {
    try {
      setCandidates(await api.aiCandidatesList(runId));
    } catch (e) {
      setError(toApiError(e));
    }
  }

  return (
    <div className="page" style={{ maxWidth: 900 }}>
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
            <button className="btn primary" disabled={!provider || !consent || busy} onClick={() => void start()}>
              Çalıştır
            </button>
          </div>
        </div>

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
            <p className="card-sub">Durum, sağlayıcı ve kullanım raporu</p>
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
                    <button className="btn ghost" onClick={() => void loadCandidates(run.id)}>Adayları gör</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p className="hint" style={{ margin: 0 }}>Henüz çalıştırma yok. Sağlayıcı bağlanınca burada görünür.</p>
        )}
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
            <thead><tr><th>Tür</th><th>Doğrulama</th><th>İçerik</th><th /></tr></thead>
            <tbody>
              {candidates.map((candidate) => (
                <tr key={candidate.id}>
                  <td>{candidate.candidateType}</td>
                  <td><span className={`badge ${candidate.validationState === "valid" ? "ok" : "warn"}`}>{candidate.validationState}</span></td>
                  <td style={{ fontFamily: "var(--mono)", fontSize: 11 }}>{candidate.structuredPayload.slice(0, 80)}</td>
                  <td>
                    <button
                      className="btn ghost"
                      onClick={() =>
                        void api
                          .aiCandidatePromote(candidate.id, caseId)
                          .then(() => setError(null))
                          .catch((e) => setError(toApiError(e)))
                      }
                    >
                      İncelemeye gönder
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
    </div>
  );
}
