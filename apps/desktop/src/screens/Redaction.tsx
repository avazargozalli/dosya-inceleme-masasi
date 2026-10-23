import { useEffect, useState } from "react";
import { api, toApiError, type ApiError, type CaseRow, type DocumentRow, type RedactionResult } from "../lib/api";

export function Redaction() {
  const [cases, setCases] = useState<CaseRow[]>([]);
  const [caseId, setCaseId] = useState("");
  const [docs, setDocs] = useState<DocumentRow[]>([]);
  const [versionId, setVersionId] = useState("");
  const [terms, setTerms] = useState("");
  const [result, setResult] = useState<RedactionResult | null>(null);
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
      .documentList(caseId)
      .then((list) => {
        setDocs(list);
        setVersionId(list.find((d) => d.currentVersionId)?.currentVersionId ?? "");
      })
      .catch((e) => setError(toApiError(e)));
  }, [caseId]);

  async function run() {
    const list = terms
      .split(/[\n,]/)
      .map((t) => t.trim())
      .filter(Boolean);
    if (!versionId || list.length === 0) {
      setError({ code: "E_TERMS", message: "Belge ve en az bir terim gerekli.", retryable: false });
      return;
    }
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      setResult(await api.documentRedact(versionId, list));
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page" style={{ maxWidth: 860 }}>
      <div className="page-head">
        <h1>Maskeleme</h1>
        <p className="lead">
          Terimler belgenin metin katmanından ve görüntüsünden gerçekten çıkarılır; siyah kutu
          çizmek yeterli sayılmaz. Çıktı yeniden açılıp doğrulanmadan kaydedilmez.
        </p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="card">
        <div className="row">
          <div className="field">
            <label htmlFor="rd-case">Dosya</label>
            <select id="rd-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
              {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
            </select>
          </div>
          <div className="field">
            <label htmlFor="rd-doc">Belge</label>
            <select id="rd-doc" className="input" value={versionId} onChange={(e) => setVersionId(e.target.value)}>
              <option value="">Seçin…</option>
              {docs.filter((d) => d.currentVersionId).map((d) => (
                <option key={d.id} value={d.currentVersionId!}>{d.displayName}</option>
              ))}
            </select>
          </div>
        </div>
        <div className="field">
          <label htmlFor="rd-terms">Maskelenecek terimler (her satır veya virgül ayrı)</label>
          <textarea
            id="rd-terms"
            className="input"
            style={{ height: 90, padding: 10, fontFamily: "var(--mono)", fontSize: 12 }}
            value={terms}
            onChange={(e) => setTerms(e.target.value)}
            placeholder={"Ahmet Yilmaz\n00000000000"}
          />
          <span className="hint">Terimler birebir aranır; eşleşme yoksa o terim için bölge oluşmaz.</span>
        </div>
        <div className="row" style={{ justifyContent: "flex-end" }}>
          <button className="btn primary" disabled={busy || !versionId} onClick={() => void run()}>
            {busy ? "Maskeleniyor…" : "Maskeli kopya üret"}
          </button>
        </div>
      </div>

      {result ? (
        <div className="card">
          <div className="card-head">
            <div>
              <h2>Maskeli kopya hazır</h2>
              <p className="card-sub">Yeni sürüm olarak kaydedildi; özgün sürüm değişmedi.</p>
            </div>
            <span className="badge ok">doğrulandı</span>
          </div>
          <dl className="kv">
            <dt>Yeni belge</dt><dd>{result.document.displayName}</dd>
            <dt>Sürüm</dt><dd>{result.version.versionNo}</dd>
            <dt>Maskelenen bölge</dt><dd>{result.hits}</dd>
            <dt>Kalan terim</dt><dd>{result.remaining.length === 0 ? "yok" : result.remaining.join(", ")}</dd>
            <dt>Plan</dt><dd style={{ fontFamily: "var(--mono)", fontSize: 11 }}>{result.plan.id}</dd>
          </dl>
          <p className="hint" style={{ margin: 0 }}>
            Bu kopya paylaşım içindir. Özgün belge ve eski sürümler kasadan silinmedi.
          </p>
        </div>
      ) : null}
    </div>
  );
}
