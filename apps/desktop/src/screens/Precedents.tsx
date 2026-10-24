import { useCallback, useEffect, useState } from "react";
import { open as openFiles } from "@tauri-apps/plugin-dialog";
import {
  api,
  toApiError,
  type ApiError,
  type BulkImportReport,
  type CaseRow,
  type CitationCheckRow,
  type CitationContext,
  type ConnectorCatalog,
  type DocumentRow,
  type PrecedentBibliography,
  type PrecedentNoteRow,
  type PrecedentRow,
  type PrecedentVersionRow,
  type QuoteVerificationRow,
} from "../lib/api";
import { formatTimestamp } from "../lib/format";
import {
  accessBasisLabel,
  bibliographyMissingText,
  bulkOutcomeLabel,
  bulkOutcomeTone,
  bulkReportSummary,
  quoteStateText,
  quoteStateTone,
  quoteToCitationMatch,
  summarizeVersions,
  versionConflictText,
  versionSourceLabel,
  versionStateText,
} from "../lib/precedents";

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

const MEANING_LABEL: Record<string, string> = {
  not_reviewed: "incelenmedi",
  accurate: "kararı yansıtıyor",
  misleading: "yanıltıcı",
  off_context: "bağlam dışı",
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
  const [meaningFor, setMeaningFor] = useState<string | null>(null);
  const [meaningContext, setMeaningContext] = useState<CitationContext | null>(null);
  const [meaningNote, setMeaningNote] = useState("");

  const [versions, setVersions] = useState<PrecedentVersionRow[]>([]);
  const [fullText, setFullText] = useState("");
  const [quoteInput, setQuoteInput] = useState("");
  const [verification, setVerification] = useState<QuoteVerificationRow | null>(null);
  const versionSummary = summarizeVersions(versions);

  // Bibliyografik künye (F12-T12)
  const [bibliography, setBibliography] = useState<PrecedentBibliography | null>(null);
  const [copied, setCopied] = useState(false);

  // Toplu içe alma (F12-T06)
  const [bulkFiles, setBulkFiles] = useState<string[]>([]);
  const [manifestPath, setManifestPath] = useState<string | null>(null);
  const [bulkCourt, setBulkCourt] = useState("Yargıtay");
  const [bulkChamber, setBulkChamber] = useState("");
  const [bulkSourceUrl, setBulkSourceUrl] = useState("");
  const [bulkTags, setBulkTags] = useState("");
  const [bulkReport, setBulkReport] = useState<BulkImportReport | null>(null);
  const [bulkBusy, setBulkBusy] = useState(false);
  const [connectors, setConnectors] = useState<ConnectorCatalog | null>(null);

  const load = useCallback(async () => {
    try {
      setRows(await api.precedentList());
    } catch (e) {
      setError(toApiError(e));
    }
  }, []);

  useEffect(() => {
    void load();
    void api
      .precedentConnectorList()
      .then(setConnectors)
      .catch((e) => setError(toApiError(e)));
    void api.caseList(false).then((r) => {
      setCases(r);
      if (r.length > 0) setCaseId((prev) => prev || r[0]!.id);
    }).catch((e) => setError(toApiError(e)));
  }, [load]);

  useEffect(() => {
    if (selected) {
      void api.precedentNoteList(selected.id).then(setNotes).catch((e) => setError(toApiError(e)));
      void api
        .precedentVersionList(selected.id)
        .then(setVersions)
        .catch((e) => setError(toApiError(e)));
      void api
        .precedentBibliography(selected.id)
        .then(setBibliography)
        .catch((e) => setError(toApiError(e)));
    } else {
      setNotes([]);
      setVersions([]);
      setBibliography(null);
    }
    setCopied(false);
    setVerification(null);
    setQuoteInput("");
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

  async function addFullText() {
    if (!selected) return;
    if (fullText.trim().length === 0) {
      setError({ code: "E_TEXT", message: "Tam metin boş olamaz.", retryable: false });
      return;
    }
    setBusy(true);
    try {
      await api.precedentTextAdd(selected.id, {
        sourceKind: "paste",
        text: fullText,
        documentVersionId: null,
      });
      setFullText("");
      setVersions(await api.precedentVersionList(selected.id));
      setBibliography(await api.precedentBibliography(selected.id));
      // Metin değişti: önceki doğrulama bu sürüme ait değil.
      setVerification(null);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function verifyQuote() {
    if (!selected) return;
    if (quoteInput.trim().length === 0) {
      setError({ code: "E_QUOTE", message: "Doğrulanacak alıntı boş olamaz.", retryable: false });
      return;
    }
    setBusy(true);
    try {
      setVerification(await api.precedentQuoteVerify(selected.id, quoteInput));
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function copyBibliography() {
    if (!bibliography) return;
    try {
      await navigator.clipboard.writeText(bibliography.text);
      setCopied(true);
    } catch {
      setError({
        code: "E_CLIPBOARD",
        message: "Metin panoya kopyalanamadı; künyeyi seçip elle kopyalayabilirsiniz.",
        retryable: false,
      });
    }
  }

  async function pickBulkFiles() {
    try {
      const picked = await openFiles({
        multiple: true,
        title: "Karar dosyalarını seç",
        filters: [{ name: "Karar dosyaları", extensions: ["pdf", "docx", "udf", "txt", "md"] }],
      });
      if (!picked) return;
      setBulkFiles(Array.isArray(picked) ? picked : [picked]);
      setBulkReport(null);
    } catch (e) {
      setError(toApiError(e));
    }
  }

  async function pickManifest() {
    try {
      const picked = await openFiles({
        multiple: false,
        title: "Künye listesi (CSV)",
        filters: [{ name: "Künye listesi", extensions: ["csv", "txt"] }],
      });
      if (!picked) return;
      setManifestPath(Array.isArray(picked) ? (picked[0] ?? null) : picked);
      setBulkReport(null);
    } catch (e) {
      setError(toApiError(e));
    }
  }

  async function runBulkImport() {
    if (bulkFiles.length === 0) {
      setError({ code: "E_BULK_EMPTY", message: "Önce dosya seçin.", retryable: false });
      return;
    }
    setBulkBusy(true);
    setBulkReport(null);
    try {
      const report = await api.precedentBulkImport({
        files: bulkFiles,
        manifestPath,
        defaults: {
          court: bulkCourt.trim(),
          chamber: bulkChamber.trim(),
          sourceUrl: bulkSourceUrl.trim(),
          tags: bulkTags.split(",").map((t) => t.trim()).filter(Boolean),
          legalIssue: "",
        },
      });
      setBulkReport(report);
      await load();
      setVersions((prev) => prev);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBulkBusy(false);
    }
  }

  async function openMeaning(id: string) {
    setMeaningFor((prev) => (prev === id ? null : id));
    setMeaningNote("");
    setMeaningContext(null);
    try {
      setMeaningContext(await api.citationContext(id));
    } catch (e) {
      setError(toApiError(e));
    }
  }

  async function reviewMeaning(id: string, review: string) {
    try {
      await api.citationMeaningReview(id, review, meaningNote);
      setChecks(await api.citationCheckList(versionId));
      setMeaningFor(null);
      setMeaningContext(null);
      setMeaningNote("");
    } catch (e) {
      setError(toApiError(e));
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
        <p className="hint">
          Bu ekran yalnız <strong>emsal kütüphanesini</strong> yönetir; dosya belgelerinde arama
          “Arama” ekranındaki <em>Dosya belgeleri</em> kapsamındadır. Kütüphane araması da ayrı
          kapsamdır (<em>Emsal kütüphanesi</em>); sonuçlar dosyaya delil olarak eklenmez.
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

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Toplu içe alma</h2>
            <p className="card-sub">
              Dosyalar + isteğe bağlı künye listesi (CSV) · metin belge worker'ında çıkarılır, ağ isteği yok
            </p>
          </div>
        </div>
        <div className="row">
          <button className="btn" disabled={bulkBusy} onClick={() => void pickBulkFiles()}>
            {bulkFiles.length === 0 ? "Dosyaları seç…" : `${bulkFiles.length} dosya seçildi`}
          </button>
          <button className="btn ghost" disabled={bulkBusy} onClick={() => void pickManifest()}>
            {manifestPath ? `Künye listesi: ${manifestPath.split("/").pop()}` : "Künye listesi (CSV) seç…"}
          </button>
          {bulkFiles.length > 0 ? (
            <button
              className="btn ghost"
              disabled={bulkBusy}
              onClick={() => {
                setBulkFiles([]);
                setBulkReport(null);
              }}
            >
              Seçimi temizle
            </button>
          ) : null}
        </div>
        <div className="row">
          <div className="field"><label htmlFor="b-court">Varsayılan mahkeme</label><input id="b-court" className="input" value={bulkCourt} onChange={(e) => setBulkCourt(e.target.value)} /></div>
          <div className="field"><label htmlFor="b-chamber">Varsayılan daire</label><input id="b-chamber" className="input" value={bulkChamber} onChange={(e) => setBulkChamber(e.target.value)} /></div>
          <div className="field"><label htmlFor="b-url">Kaynak URL</label><input id="b-url" className="input" value={bulkSourceUrl} onChange={(e) => setBulkSourceUrl(e.target.value)} placeholder="https://…" /></div>
          <div className="field"><label htmlFor="b-tags">Etiketler</label><input id="b-tags" className="input" value={bulkTags} onChange={(e) => setBulkTags(e.target.value)} placeholder="kira, temerrüt" /></div>
        </div>
        <div className="row">
          <button
            className="btn primary"
            disabled={bulkBusy || bulkFiles.length === 0}
            onClick={() => void runBulkImport()}
          >
            {bulkBusy ? "İçe alınıyor…" : "İçe al"}
          </button>
          <span className="hint" style={{ flex: "1 1 280px", margin: 0 }}>
            Liste sütunları: dosya, mahkeme, daire, esas, karar, tarih, başlık, kaynak url, etiketler, konu.
            Liste yoksa varsayılan künye kullanılır; eksik künye “incomplete” kalır, numara tahmin edilmez.
          </span>
        </div>
        {bulkReport ? (
          <div role="status">
            <p className="small" style={{ margin: "4px 0" }}>
              {bulkReportSummary(bulkReport)}
              {bulkReport.manifestHash ? ` · liste sha256 ${bulkReport.manifestHash.slice(0, 12)}…` : ""}
            </p>
            {bulkReport.manifestWarnings.map((warning) => (
              <p key={warning} className="hint" style={{ margin: 0 }}>{warning}</p>
            ))}
            <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
              {bulkReport.items.map((item) => (
                <div
                  key={`${item.fileName}-${item.outcome}-${item.code ?? ""}`}
                  className="row"
                  style={{ justifyContent: "space-between", padding: "6px 8px", border: "1px solid var(--line)", borderRadius: "var(--radius)" }}
                >
                  <span className="small">{item.fileName}</span>
                  <span className={`badge ${bulkOutcomeTone(item)}`}>{bulkOutcomeLabel(item)}</span>
                </div>
              ))}
            </div>
            {bulkReport.conflicts > 0 ? (
              <p className="hint" style={{ margin: 0 }}>
                Çakışan nüshalar ayrı sürüm olarak eklendi ve “farklı metin” rozetiyle işaretlendi;
                sessiz birleştirme yapılmadı. İki metni karşılaştırmadan biri silinmemelidir.
              </p>
            ) : null}
          </div>
        ) : null}
      </div>

      {selected ? (
        <>
        <div className="card">
          <div className="card-head">
            <div>
              <h2>Bibliyografik künye</h2>
              <p className="card-sub">
                Dilekçeye kopyalanabilir künye · kaynak bağlantısı ve erişim tarihi
              </p>
            </div>
            {bibliography ? (
              <button className="btn ghost" onClick={() => void copyBibliography()}>
                {copied ? "Kopyalandı" : "Künyeyi kopyala"}
              </button>
            ) : null}
          </div>
          {bibliography ? (
            <>
              <p
                className="small"
                style={{
                  margin: "0 0 6px",
                  padding: "8px 10px",
                  border: "1px solid var(--line)",
                  borderRadius: "var(--radius)",
                  background: "var(--surface)",
                  userSelect: "text",
                }}
              >
                {bibliography.text}
              </p>
              <div className="row" style={{ gap: 6 }}>
                <span className={`badge ${bibliography.missing.length === 0 ? "ok" : "warn"}`}>
                  {bibliographyMissingText(bibliography.missing)}
                </span>
                <span className="badge plain">erişim: {bibliography.accessedAt}</span>
                <span className={`badge ${bibliography.hasFullText ? "ok" : "warn"}`}>
                  {bibliography.hasFullText ? "tam metin var" : "tam metin yok"}
                </span>
              </div>
              <p className="hint" style={{ margin: "6px 0 0" }}>
                {accessBasisLabel(bibliography.accessBasis)}
              </p>
              {bibliography.warnings.map((warning) => (
                <p key={warning} className="hint" style={{ margin: 0 }}>{warning}</p>
              ))}
              {bibliography.sourceUrl ? (
                <p className="hint" style={{ margin: 0 }}>
                  <a href={bibliography.sourceUrl} target="_blank" rel="noreferrer">
                    Kaynak bağlantısını aç
                  </a>
                </p>
              ) : null}
            </>
          ) : (
            <p className="hint" style={{ margin: 0 }}>Künye yükleniyor…</p>
          )}
        </div>

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

        <div className="card">
          <div className="card-head">
            <div>
              <h2>Tam metin ve alıntı doğrulama</h2>
              <p className="card-sub">
                {versions.length} sürüm · {versionSummary.withText} metinli
                {versions.length > 0 && !versionSummary.latestHasText ? " · en yeni sürümde metin yok" : ""}
              </p>
            </div>
          </div>

          {versions.length === 0 ? (
            <div className="empty"><p style={{ margin: 0 }}>Bu kararın tam metni yok; alıntı doğrulanamaz.</p></div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
              {versions.map((version) => (
                <div
                  key={version.id}
                  className="row"
                  style={{ justifyContent: "space-between", padding: "6px 8px", border: "1px solid var(--line)", borderRadius: "var(--radius)" }}
                >
                  <span className="small">
                    {versionSourceLabel(version)} · {formatTimestamp(version.acquiredAt)}
                    <span className="muted"> · sha256 {version.bodyHash.slice(0, 12)}…</span>
                  </span>
                  <span style={{ display: "flex", gap: 6 }}>
                    {versionConflictText(version) ? (
                      <span className="badge warn">{versionConflictText(version)}</span>
                    ) : null}
                    <span className={`badge ${version.hasText ? "ok" : "warn"}`}>{versionStateText(version)}</span>
                  </span>
                </div>
              ))}
            </div>
          )}

          <div className="field">
            <label htmlFor="p-fulltext">Karar tam metnini yapıştır</label>
            <textarea
              id="p-fulltext"
              className="input"
              rows={5}
              style={{ height: "auto", minHeight: 110, padding: "8px 10px", resize: "vertical" }}
              value={fullText}
              onChange={(e) => setFullText(e.target.value)}
              placeholder="Kararın tam metni (kaynağından kopyalanan hâli)…"
            />
          </div>
          <div className="row">
            <button className="btn" disabled={busy || fullText.trim().length === 0} onClick={() => void addFullText()}>
              Tam metni kaydet
            </button>
            <span className="hint" style={{ flex: "1 1 260px", margin: 0 }}>
              Metin kasanın şifreli veritabanında tutulur; listeye yalnız SHA-256 ve karakter sayısı çıkar.
            </span>
          </div>

          <div className="row">
            <div className="field" style={{ flex: "2 1 320px" }}>
              <label htmlFor="p-quote">Karardan alıntı</label>
              <input
                id="p-quote"
                className="input"
                value={quoteInput}
                onChange={(e) => setQuoteInput(e.target.value)}
                placeholder="Doğrulanacak cümle (en az 8 karakter)"
              />
            </div>
            <button
              className="btn primary"
              disabled={busy || !versionSummary.latestHasText || quoteInput.trim().length === 0}
              onClick={() => void verifyQuote()}
            >
              Alıntıyı doğrula
            </button>
          </div>

          {verification ? (
            <div className={`notice${quoteStateTone(verification) === "ok" ? " ok" : quoteStateTone(verification) === "warn" ? " danger" : ""}`} role="status">
              {quoteStateText(verification)}
              {verification.state !== "not_checked" ? (
                <div style={{ marginTop: 8 }}>
                  <button
                    className="btn ghost"
                    style={{ height: 26 }}
                    onClick={() => setQuoteMatch(quoteToCitationMatch(verification))}
                  >
                    Atıf kontrolü alanına taşı ({quoteToCitationMatch(verification)})
                  </button>
                </div>
              ) : null}
            </div>
          ) : null}
          <p className="hint" style={{ margin: 0 }}>
            Doğrulama yalnız metin eşleşmesidir; anlam/bağlam incelemesi ayrı boyuttur. “Bulunamadı”
            sonucu tek başına “uydurulmuş” hükmü değildir.
          </p>
        </div>
        </>
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
              <div key={check.id} style={{ padding: "6px 8px", border: "1px solid var(--line)", borderRadius: "var(--radius)" }}>
                <div className="row" style={{ justifyContent: "space-between" }}>
                  <span className="small">{check.extractedCitation}</span>
                  <span style={{ display: "flex", gap: 6, alignItems: "center" }}>
                    <span className="badge plain">künye: {check.identityMatch}</span>
                    <span className="badge plain">alıntı: {check.quoteMatch}</span>
                    <span className={`badge ${check.meaningReview === "accurate" ? "ok" : check.meaningReview === "not_reviewed" ? "plain" : "warn"}`}>
                      anlam: {MEANING_LABEL[check.meaningReview] ?? check.meaningReview}
                    </span>
                    <button className="btn ghost" onClick={() => void openMeaning(check.id)}>
                      {meaningFor === check.id ? "Kapat" : "Anlam"}
                    </button>
                  </span>
                </div>
                {meaningFor === check.id ? (
                  <div style={{ marginTop: 8, borderTop: "1px solid var(--line)", paddingTop: 8 }}>
                    {meaningContext ? (
                      meaningContext.found ? (
                        <p className="small" style={{ margin: "0 0 6px" }}>
                          …{meaningContext.before}
                          <mark>{meaningContext.quote}</mark>
                          {meaningContext.after}…
                        </p>
                      ) : (
                        <p className="hint" style={{ margin: "0 0 6px" }}>
                          Tam metin/bağlam yok ({meaningContext.reason ?? meaningContext.state}); uydurma bağlam
                          gösterilmez.
                        </p>
                      )
                    ) : (
                      <p className="hint" style={{ margin: "0 0 6px" }}>Bağlam yükleniyor…</p>
                    )}
                    <div className="field">
                      <label htmlFor={`mn-${check.id}`}>Gerekçe (yanıltıcı/bağlam dışı için zorunlu)</label>
                      <input
                        id={`mn-${check.id}`}
                        className="input"
                        value={meaningNote}
                        onChange={(e) => setMeaningNote(e.target.value)}
                        placeholder="Örn. kararın sonucu tersine çevrilmiş"
                      />
                    </div>
                    <div className="row" style={{ gap: 6 }}>
                      <button className="btn" onClick={() => void reviewMeaning(check.id, "accurate")}>Anlam doğru</button>
                      <button className="btn ghost" onClick={() => void reviewMeaning(check.id, "misleading")}>Yanıltıcı</button>
                      <button className="btn ghost" onClick={() => void reviewMeaning(check.id, "off_context")}>Bağlam dışı</button>
                    </div>
                  </div>
                ) : null}
              </div>
            ))}
          </div>
        ) : null}
        <p className="hint" style={{ margin: 0 }}>
          Karar metni olmadan alıntı doğrulanamaz; bu durum “kontrol edilmedi” olarak kalır ve
          “uydurulmuş” anlamına gelmez.
        </p>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Dış bağlayıcılar</h2>
            <p className="card-sub">
              {connectors
                ? `${connectors.connectors.length} kaynak belgelendi · hiçbiri etkin değil`
                : "sicil yükleniyor…"}
            </p>
          </div>
        </div>
        {connectors ? (
          <>
            {connectors.connectors.map((connector) => (
              <div key={connector.id} className="card tight" style={{ background: "var(--surface)" }}>
                <div className="row" style={{ justifyContent: "space-between" }}>
                  <strong style={{ fontWeight: 500 }}>{connector.name}</strong>
                  <span className="badge warn">etkin değil</span>
                </div>
                <p className="muted small" style={{ margin: "2px 0" }}>{connector.operator}</p>
                <p className="small" style={{ margin: "4px 0" }}>{connector.scope}</p>
                <p className="small" style={{ margin: "4px 0" }}>{connector.reuseNote}</p>
                <p className="hint" style={{ margin: 0 }}>
                  {connector.accessMethod} <a href={connector.termsUrl} target="_blank" rel="noreferrer">koşullar</a>
                </p>
              </div>
            ))}
            <p className="hint" style={{ margin: 0 }}>
              {connectors.note} <span className="muted">sicil sha256 {connectors.catalogHash.slice(0, 12)}…</span>
            </p>
          </>
        ) : null}
      </div>
    </div>
  );
}
