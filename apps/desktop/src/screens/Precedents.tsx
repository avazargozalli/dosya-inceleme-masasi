import { useCallback, useEffect, useState } from "react";
import {
  api,
  toApiError,
  type ApiError,
  type CaseRow,
  type CitationCheckRow,
  type DocumentRow,
  type PrecedentNoteRow,
  type PrecedentRow,
} from "../lib/api";
import { formatTimestamp } from "../lib/format";

type Draft = {
  court: string;
  chamber: string;
  docketNo: string;
  decisionNo: string;
  decisionDate: string;
  title: string;
  sourceUrl: string;
  tags: string;
  legalIssue: string;
};

const EMPTY: Draft = {
  court: "Yargıtay",
  chamber: "",
  docketNo: "",
  decisionNo: "",
  decisionDate: "",
  title: "",
  sourceUrl: "",
  tags: "",
  legalIssue: "",
};

export function Precedents() {
  const [rows, setRows] = useState<PrecedentRow[]>([]);
  const [selected, setSelected] = useState<PrecedentRow | null>(null);
  const [notes, setNotes] = useState<PrecedentNoteRow[]>([]);
  const [draft, setDraft] = useState<Draft>(EMPTY);
  const [similarity, setSimilarity] = useState("");
  const [distinguishing, setDistinguishing] = useState("");
  const [conclusion, setConclusion] = useState("");
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

  const [cases, setCases] = useState<CaseRow[]>([]);
  const [caseId, setCaseId] = useState("");
  const [docs, setDocs] = useState<DocumentRow[]>([]);
  const [versionId, setVersionId] = useState("");
  const [citation, setCitation] = useState("");
  const [candidate, setCandidate] = useState("");
  const [identity, setIdentity] = useState<"match" | "mismatch" | "unknown">("unknown");
  const [quoteMatch, setQuoteMatch] = useState<"match" | "mismatch" | "not_checked" | "unknown">("not_checked");
  const [checks, setChecks] = useState<CitationCheckRow[]>([]);

  const load = useCallback(async () => {
    try {
      setRows(await api.precedentList());
    } catch (e) {
      setError(toApiError(e));
    }
  }, []);

  useEffect(() => {
    void load();
    void api.caseList(false).then((r) => {
      setCases(r);
      if (r.length > 0) setCaseId((prev) => prev || r[0]!.id);
    }).catch((e) => setError(toApiError(e)));
  }, [load]);

  useEffect(() => {
    if (selected) {
      void api.precedentNoteList(selected.id).then(setNotes).catch((e) => setError(toApiError(e)));
    } else {
      setNotes([]);
    }
  }, [selected]);

  useEffect(() => {
    if (!caseId) return;
    void api.documentList(caseId).then((list) => {
      setDocs(list);
      setVersionId(list.find((d) => d.currentVersionId)?.currentVersionId ?? "");
    }).catch((e) => setError(toApiError(e)));
  }, [caseId]);

  useEffect(() => {
    if (!versionId) {
      setChecks([]);
      return;
    }
    void api.citationCheckList(versionId).then(setChecks).catch((e) => setError(toApiError(e)));
  }, [versionId]);

  async function add() {
    if (draft.court.trim().length === 0) {
      setError({ code: "E_COURT", message: "Mahkeme zorunlu.", retryable: false });
      return;
    }
    setBusy(true);
    try {
      await api.precedentCreate({
        court: draft.court.trim(),
        chamber: draft.chamber.trim() || null,
        docketNo: draft.docketNo.trim() || null,
        decisionNo: draft.decisionNo.trim() || null,
        applicationNo: null,
        decisionDate: draft.decisionDate || null,
        title: draft.title.trim() || null,
        sourceUrl: draft.sourceUrl.trim() || null,
        userTags: draft.tags.split(",").map((t) => t.trim()).filter(Boolean),
        legalIssue: draft.legalIssue.trim() || null,
      });
      setDraft(EMPTY);
      await load();
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function addNote() {
    if (!selected) return;
    setBusy(true);
    try {
      await api.precedentNoteCreate(selected.id, {
        caseIdOptional: caseId || null,
        usefulPassageAnchor: null,
        similarityNote: similarity.trim(),
        distinguishingNote: distinguishing.trim(),
        userConclusion: conclusion.trim(),
      });
      setSimilarity("");
      setDistinguishing("");
      setConclusion("");
      setNotes(await api.precedentNoteList(selected.id));
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function addCheck() {
    if (!versionId || citation.trim().length === 0) {
      setError({ code: "E_CITATION", message: "Belge sürümü ve atıf metni zorunlu.", retryable: false });
      return;
    }
    setBusy(true);
    try {
      await api.citationCheckCreate({
        inputDocumentVersionId: versionId,
        extractedCitation: citation.trim(),
        candidatePrecedentId: candidate || null,
        identityMatch: identity,
        quoteMatch,
        meaningReview: "not_reviewed",
        reasonCodes: quoteMatch === "not_checked" ? ["text_not_checked"] : [],
      });
      setCitation("");
      setChecks(await api.citationCheckList(versionId));
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page" style={{ maxWidth: 1000 }}>
      <div className="page-head">
        <h1>Emsal ve atıf kontrolü</h1>
        <p className="lead">
          Künye eksikse karar “incomplete” işaretlenir. “Bulunamadı” ile “uydurulmuş” aynı şey
          değildir; hiçbir kontrol tek başına sahte hükmü üretmez.
        </p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Emsal kütüphanesi</h2>
            <p className="card-sub">{rows.length} karar</p>
          </div>
        </div>
        <div className="row">
          <div className="field"><label htmlFor="p-court">Mahkeme</label><input id="p-court" className="input" value={draft.court} onChange={(e) => setDraft({ ...draft, court: e.target.value })} /></div>
          <div className="field"><label htmlFor="p-chamber">Daire</label><input id="p-chamber" className="input" value={draft.chamber} onChange={(e) => setDraft({ ...draft, chamber: e.target.value })} /></div>
          <div className="field"><label htmlFor="p-docket">Esas</label><input id="p-docket" className="input" value={draft.docketNo} onChange={(e) => setDraft({ ...draft, docketNo: e.target.value })} placeholder="2024/123" /></div>
          <div className="field"><label htmlFor="p-decision">Karar</label><input id="p-decision" className="input" value={draft.decisionNo} onChange={(e) => setDraft({ ...draft, decisionNo: e.target.value })} placeholder="2025/456" /></div>
          <div className="field"><label htmlFor="p-date">Tarih</label><input id="p-date" className="input" type="date" value={draft.decisionDate} onChange={(e) => setDraft({ ...draft, decisionDate: e.target.value })} /></div>
        </div>
        <div className="row">
          <div className="field" style={{ flex: "2 1 300px" }}><label htmlFor="p-title">Başlık</label><input id="p-title" className="input" value={draft.title} onChange={(e) => setDraft({ ...draft, title: e.target.value })} /></div>
          <div className="field" style={{ flex: "2 1 300px" }}><label htmlFor="p-url">Kaynak URL</label><input id="p-url" className="input" value={draft.sourceUrl} onChange={(e) => setDraft({ ...draft, sourceUrl: e.target.value })} placeholder="https://…" /></div>
          <div className="field"><label htmlFor="p-tags">Etiketler</label><input id="p-tags" className="input" value={draft.tags} onChange={(e) => setDraft({ ...draft, tags: e.target.value })} placeholder="kira, temerrüt" /></div>
          <button className="btn primary" disabled={busy} onClick={() => void add()}>Ekle</button>
        </div>

        {rows.length === 0 ? (
          <div className="empty"><p style={{ margin: 0 }}>Henüz emsal yok.</p></div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {rows.map((row) => (
              <button
                key={row.id}
                onClick={() => setSelected(row)}
                style={{
                  textAlign: "left",
                  padding: "9px 12px",
                  border: "1px solid var(--line)",
                  borderRadius: "var(--radius)",
                  background: selected?.id === row.id ? "var(--surface)" : "var(--bg)",
                  cursor: "pointer",
                }}
              >
                <span style={{ display: "flex", justifyContent: "space-between", gap: 8 }}>
                  <strong style={{ fontWeight: 500 }}>
                    {row.court} {row.chamber ?? ""} {row.docketNo ?? ""} {row.decisionNo ? `K.${row.decisionNo}` : ""}
                  </strong>
                  <span className={`badge ${row.completeness === "complete" ? "ok" : "warn"}`}>{row.completeness}</span>
                </span>
                <span className="muted small">{row.title ?? "başlıksız"} · {row.decisionDate ?? "tarih yok"}</span>
              </button>
            ))}
          </div>
        )}
      </div>

      {selected ? (
        <div className="card">
          <div className="card-head">
            <div>
              <h2>Kullanım notu</h2>
              <p className="card-sub">{notes.length} not · dosyaya özel veya kütüphane notu</p>
            </div>
            <button className="btn ghost" onClick={() => setSelected(null)}>Kapat</button>
          </div>
          {notes.map((note) => (
            <div key={note.id} className="card tight" style={{ background: "var(--surface)" }}>
              <div className="muted small">{note.caseIdOptional ? "dosyaya özel" : "kütüphane notu"} · {formatTimestamp(note.createdAt)}</div>
              {note.similarityNote ? <p style={{ margin: "4px 0" }}><strong>Benzerlik:</strong> {note.similarityNote}</p> : null}
              {note.distinguishingNote ? <p style={{ margin: "4px 0" }}><strong>Ayrışan:</strong> {note.distinguishingNote}</p> : null}
              {note.userConclusion ? <p style={{ margin: "4px 0" }}><strong>Sonuç:</strong> {note.userConclusion}</p> : null}
            </div>
          ))}
          <div className="row">
            <div className="field"><label htmlFor="n-sim">Benzerlik</label><input id="n-sim" className="input" value={similarity} onChange={(e) => setSimilarity(e.target.value)} /></div>
            <div className="field"><label htmlFor="n-dist">Ayrışan nokta</label><input id="n-dist" className="input" value={distinguishing} onChange={(e) => setDistinguishing(e.target.value)} /></div>
            <div className="field"><label htmlFor="n-conc">Sonuç</label><input id="n-conc" className="input" value={conclusion} onChange={(e) => setConclusion(e.target.value)} /></div>
            <button className="btn" disabled={busy} onClick={() => void addNote()}>Not ekle</button>
          </div>
          <p className="hint" style={{ margin: 0 }}>Hukuki uygunluk değerlendirmesi kullanıcıya aittir.</p>
        </div>
      ) : null}

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Atıf kontrolü</h2>
            <p className="card-sub">{checks.length} kayıt · boyutlar ayrı tutulur</p>
          </div>
        </div>
        <div className="row">
          <div className="field">
            <label htmlFor="c-case">Dosya</label>
            <select id="c-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
              {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
            </select>
          </div>
          <div className="field">
            <label htmlFor="c-doc">Belge sürümü</label>
            <select id="c-doc" className="input" value={versionId} onChange={(e) => setVersionId(e.target.value)}>
              <option value="">Seçin…</option>
              {docs.filter((d) => d.currentVersionId).map((d) => (
                <option key={d.id} value={d.currentVersionId!}>{d.displayName}</option>
              ))}
            </select>
          </div>
          <div className="field" style={{ flex: "2 1 260px" }}>
            <label htmlFor="c-text">Atıf metni</label>
            <input id="c-text" className="input" value={citation} onChange={(e) => setCitation(e.target.value)} placeholder="Yargıtay 1. HD 2024/123" />
          </div>
        </div>
        <div className="row">
          <div className="field">
            <label htmlFor="c-cand">Aday karar</label>
            <select id="c-cand" className="input" value={candidate} onChange={(e) => setCandidate(e.target.value)}>
              <option value="">Eşleşme yok</option>
              {rows.map((r) => <option key={r.id} value={r.id}>{r.court} {r.docketNo ?? ""}</option>)}
            </select>
          </div>
          <div className="field">
            <label htmlFor="c-id">Künye</label>
            <select id="c-id" className="input" value={identity} onChange={(e) => setIdentity(e.target.value as typeof identity)}>
              <option value="unknown">Bilinmiyor</option>
              <option value="match">Eşleşti</option>
              <option value="mismatch">Eşleşmedi</option>
            </select>
          </div>
          <div className="field">
            <label htmlFor="c-quote">Alıntı</label>
            <select id="c-quote" className="input" value={quoteMatch} onChange={(e) => setQuoteMatch(e.target.value as typeof quoteMatch)}>
              <option value="not_checked">Kontrol edilmedi</option>
              <option value="unknown">Bilinmiyor</option>
              <option value="match">Eşleşti</option>
              <option value="mismatch">Eşleşmedi</option>
            </select>
          </div>
          <button className="btn" disabled={busy} onClick={() => void addCheck()}>Kontrolü kaydet</button>
        </div>
        {checks.length > 0 ? (
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            {checks.map((check) => (
              <div key={check.id} className="row" style={{ justifyContent: "space-between", padding: "6px 8px", border: "1px solid var(--line)", borderRadius: "var(--radius)" }}>
                <span className="small">{check.extractedCitation}</span>
                <span style={{ display: "flex", gap: 6 }}>
                  <span className="badge plain">künye: {check.identityMatch}</span>
                  <span className="badge plain">alıntı: {check.quoteMatch}</span>
                </span>
              </div>
            ))}
          </div>
        ) : null}
        <p className="hint" style={{ margin: 0 }}>
          Karar metni olmadan alıntı doğrulanamaz; bu durum “kontrol edilmedi” olarak kalır ve
          “uydurulmuş” anlamına gelmez.
        </p>
      </div>
    </div>
  );
}
