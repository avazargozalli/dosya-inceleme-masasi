import { useCallback, useEffect, useState } from "react";
import { open as openFiles, save as saveFile } from "@tauri-apps/plugin-dialog";
import {
  api,
  toApiError,
  type ApiError,
  type BulkImportReport,
  type CaseRow,
  type CitationCheckRow,
  type CitationContext,
  type ConnectorCatalog,
  type CorpusCoverage,
  type CorpusExportResult,
  type DocumentRow,
  type DocumentVersionRow,
  type PrecedentBibliography,
  type PrecedentDocumentText,
  type PrecedentNoteRow,
  type PrecedentRow,
  type PrecedentUsageTerms,
  type PrecedentVersionRow,
  type QuoteVerificationRow,
  type Redistribution,
  type UsageBasis,
} from "../lib/api";
import { formatSize, formatTimestamp } from "../lib/format";
import {
  accessBasisLabel,
  bibliographyMissingText,
  bulkOutcomeLabel,
  bulkOutcomeTone,
  bulkReportSummary,
  quoteScanNote,
  quoteStateText,
  quoteStateTone,
  quoteToCitationMatch,
  summarizeVersions,
  versionConflictText,
  versionSourceLabel,
  versionStateText,
} from "../lib/precedents";
import {
  corpusGateOpen,
  corpusGateText,
  corpusResultText,
  coverageText,
  exclusionReasonLabel,
  needsWrittenBasis,
  redistributionLabel,
  termsBadge,
  termsRecordedText,
  usageBasisLabel,
} from "../lib/precedent-terms";
import {
  documentTextSummary,
  documentTextWarnings,
  transferBlockedReason,
  versionOptionLabel,
} from "../lib/precedent-document";

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

  // Kasadaki belgeden tam metin (F12-T13)
  const [srcCaseId, setSrcCaseId] = useState("");
  const [srcDocs, setSrcDocs] = useState<DocumentRow[]>([]);
  const [srcDocId, setSrcDocId] = useState("");
  const [srcVersions, setSrcVersions] = useState<DocumentVersionRow[]>([]);
  const [srcVersionId, setSrcVersionId] = useState("");
  const [srcResult, setSrcResult] = useState<PrecedentDocumentText | null>(null);
  const [srcBusy, setSrcBusy] = useState(false);

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
  const [terms, setTerms] = useState<PrecedentUsageTerms | null>(null);
  const [termsBasis, setTermsBasis] = useState<UsageBasis>("unknown");
  const [termsRedistribution, setTermsRedistribution] = useState<Redistribution>("unknown");
  const [termsUrl, setTermsUrl] = useState("");
  const [termsNote, setTermsNote] = useState("");
  const [coverage, setCoverage] = useState<CorpusCoverage | null>(null);
  const [corpusMode, setCorpusMode] = useState<"permitted_only" | "all">("permitted_only");
  const [corpusResult, setCorpusResult] = useState<CorpusExportResult | null>(null);
  const [corpusBusy, setCorpusBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setRows(await api.precedentList());
    } catch (e) {
      setError(toApiError(e));
    }
  }, []);

  const loadCoverage = useCallback(async () => {
    try {
      setCoverage(await api.precedentCorpusCoverage());
    } catch (e) {
      setError(toApiError(e));
    }
  }, []);

  useEffect(() => {
    void load();
    void loadCoverage();
    void api
      .precedentConnectorList()
      .then(setConnectors)
      .catch((e) => setError(toApiError(e)));
    void api.caseList(false).then((r) => {
      setCases(r);
      if (r.length > 0) setCaseId((prev) => prev || r[0]!.id);
    }).catch((e) => setError(toApiError(e)));
  }, [load, loadCoverage]);

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
      void api
        .precedentTermsGet(selected.id)
        .then((loaded) => {
          setTerms(loaded);
          setTermsBasis(loaded.usageBasis);
          setTermsRedistribution(loaded.redistribution);
          setTermsUrl(loaded.termsUrl ?? "");
          setTermsNote(loaded.termsNote);
        })
        .catch((e) => setError(toApiError(e)));
    } else {
      setNotes([]);
      setVersions([]);
      setBibliography(null);
      setTerms(null);
    }
    setCopied(false);
    setVerification(null);
    setQuoteInput("");
    setSrcResult(null);
  }, [selected]);

  useEffect(() => {
    if (!caseId) return;
    void api.documentList(caseId).then((list) => {
      setDocs(list);
      setVersionId(list.find((d) => d.currentVersionId)?.currentVersionId ?? "");
    }).catch((e) => setError(toApiError(e)));
  }, [caseId]);

  // F12-T13: kaynak belge seçimi — dosyalar `cases`, sürümler gerçek
  // `document_versions` kayıtlarından gelir. Liste boşsa arayüz nedenini yazar.
  useEffect(() => {
    setSrcCaseId((prev) =>
      prev && cases.some((c) => c.id === prev) ? prev : (cases[0]?.id ?? ""),
    );
  }, [cases]);

  useEffect(() => {
    if (!srcCaseId) {
      setSrcDocs([]);
      setSrcDocId("");
      return;
    }
    void api
      .documentList(srcCaseId)
      .then((list) => {
        setSrcDocs(list);
        setSrcDocId((prev) => (list.some((d) => d.id === prev) ? prev : (list[0]?.id ?? "")));
      })
      .catch((e) => setError(toApiError(e)));
  }, [srcCaseId]);

  useEffect(() => {
    if (!srcDocId) {
      setSrcVersions([]);
      setSrcVersionId("");
      return;
    }
    void api
      .documentVersionList(srcDocId)
      .then((list) => {
        setSrcVersions(list);
        setSrcVersionId((prev) => (list.some((v) => v.id === prev) ? prev : (list[0]?.id ?? "")));
      })
      .catch((e) => setError(toApiError(e)));
  }, [srcDocId]);

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
      // Yeni tam metin dağıtım kapısının sayılarını değiştirir.
      await loadCoverage();
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  /**
   * F12-T13 — kasadaki belge sürümünün çıkarılmış metnini emsale aktarır.
   * Kaynak (hangi belge/sürüm) kullanıcı seçer; çekirdek ayrıştırma koşusunu
   * bulamazsa metin yazılmaz ve hata kodu gösterilir.
   */
  async function transferFromDocument() {
    if (!selected) return;
    const blocked = transferBlockedReason({
      hasPrecedent: true,
      documentCount: srcDocs.length,
      versionCount: srcVersions.length,
      selectedVersionId: srcVersionId,
    });
    if (blocked) {
      setError({ code: "E_TEXT_SOURCE", message: blocked, retryable: false });
      return;
    }
    setSrcBusy(true);
    setError(null);
    setSrcResult(null);
    try {
      const result = await api.precedentTextFromDocument(selected.id, srcVersionId);
      setSrcResult(result);
      setVersions(await api.precedentVersionList(selected.id));
      setBibliography(await api.precedentBibliography(selected.id));
      // Metin değişti: önceki alıntı doğrulaması bu sürüme ait değil.
      setVerification(null);
      await loadCoverage();
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setSrcBusy(false);
    }
  }

  async function saveTerms() {
    if (!selected) return;
    setBusy(true);
    setError(null);
    try {
      const saved = await api.precedentTermsSet(selected.id, {
        usageBasis: termsBasis,
        redistribution: termsRedistribution,
        termsUrl: termsUrl.trim() || null,
        termsNote: termsNote.trim(),
      });
      setTerms(saved);
      setTermsBasis(saved.usageBasis);
      setTermsRedistribution(saved.redistribution);
      setTermsUrl(saved.termsUrl ?? "");
      setTermsNote(saved.termsNote);
      await Promise.all([load(), loadCoverage()]);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function exportCorpus() {
    const target = await saveFile({
      title: "Karar korpusunu kaydet",
      defaultPath: "emsal-korpusu.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (typeof target !== "string") return;
    setCorpusBusy(true);
    setError(null);
    setCorpusResult(null);
    try {
      const result = await api.precedentCorpusExport(corpusMode, target);
      setCorpusResult(result);
      await loadCoverage();
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setCorpusBusy(false);
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
                  <span style={{ display: "flex", gap: 4 }}>
                    <span className={`badge ${termsBadge(row).tone}`}>{termsBadge(row).text}</span>
                    <span className={`badge ${row.completeness === "complete" ? "ok" : "warn"}`}>{row.completeness}</span>
                  </span>
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

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Toplu korpus dağıtımı</h2>
            <p className="card-sub">
              Yalnız yeniden dağıtım izni kayıtlı tam metinler · ağ isteği yok, dosya seçilen konuma yazılır
            </p>
          </div>
          {coverage ? (
            <span className={`badge ${corpusGateOpen(coverage) ? "ok" : "warn"}`}>
              {corpusGateOpen(coverage) ? "tümü açık" : "yalnız izinliler"}
            </span>
          ) : null}
        </div>
        {coverage ? (
          <>
            <p className="small" style={{ margin: "0 0 4px" }}>{coverageText(coverage)}</p>
            <p className="hint" style={{ margin: "0 0 8px" }}>{corpusGateText(coverage)}</p>
            {coverage.blocked.length > 0 ? (
              <div style={{ display: "flex", flexDirection: "column", gap: 4, marginBottom: 8 }}>
                {coverage.blocked.map((blocked) => (
                  <div
                    key={blocked.precedentId}
                    className="row"
                    style={{ justifyContent: "space-between", padding: "6px 8px", border: "1px solid var(--line)", borderRadius: "var(--radius)" }}
                  >
                    <span className="small">{blocked.court} {blocked.docketNo ?? ""}</span>
                    <span className="badge warn">{exclusionReasonLabel(blocked.reason)}</span>
                  </div>
                ))}
                {coverage.blockedTotal > coverage.blocked.length ? (
                  <p className="hint" style={{ margin: 0 }}>
                    {coverage.blockedTotal - coverage.blocked.length} kayıt daha listede gösterilmiyor.
                  </p>
                ) : null}
              </div>
            ) : null}
            <div className="row">
              <div className="field">
                <label htmlFor="corpus-mode">Kip</label>
                <select
                  id="corpus-mode"
                  className="input"
                  value={corpusMode}
                  onChange={(e) => setCorpusMode(e.target.value as "permitted_only" | "all")}
                >
                  <option value="permitted_only">Yalnız izinli kararlar</option>
                  <option value="all">Tümü (hepsi izinliyse)</option>
                </select>
              </div>
              <button
                className="btn primary"
                disabled={corpusBusy || coverage.allowed === 0}
                onClick={() => void exportCorpus()}
              >
                {corpusBusy ? "Aktarılıyor…" : "Korpusu dışa aktar…"}
              </button>
            </div>
            {corpusMode === "all" && !corpusGateOpen(coverage) ? (
              <p className="hint" style={{ margin: 0 }}>
                “Tümü” kipi kapalı: {coverage.blockedTotal} tam metinli kayıt için izin kaydı yok.
                İzin kaydı olmadan hiçbir metin dosyaya yazılmaz.
              </p>
            ) : null}
            {corpusResult ? (
              <div className="notice ok" role="status">
                {corpusResultText(corpusResult)}
                <div className="hint" style={{ marginTop: 4 }}>
                  {corpusResult.path} · {formatSize(corpusResult.bytes)}
                </div>
              </div>
            ) : null}
          </>
        ) : (
          <p className="hint" style={{ margin: 0 }}>kapsam yükleniyor…</p>
        )}
      </div>

      {selected ? (
        <>
        <div className="card">
          <div className="card-head">
            <div>
              <h2>Kaynak kullanım koşulu</h2>
              <p className="card-sub">
                Kullanım dayanağı ve yeniden dağıtım izni ayrı kaydedilir · kayıt yokluğu izin sayılmaz
              </p>
            </div>
            <span className={`badge ${terms ? termsBadge(terms).tone : "warn"}`}>
              {terms ? termsBadge(terms).text : "kayıt yok"}
            </span>
          </div>
          <div className="row">
            <div className="field">
              <label htmlFor="t-basis">Kullanım dayanağı</label>
              <select
                id="t-basis"
                className="input"
                value={termsBasis}
                onChange={(e) => setTermsBasis(e.target.value as UsageBasis)}
              >
                <option value="unknown">Bilinmiyor</option>
                <option value="official_public">Resmî yayın (kamuya açık)</option>
                <option value="licence">Açık lisans</option>
                <option value="permission">Yazılı izin</option>
                <option value="internal_only">Yalnız iç kullanım</option>
              </select>
            </div>
            <div className="field">
              <label htmlFor="t-redistribution">Yeniden dağıtım</label>
              <select
                id="t-redistribution"
                className="input"
                value={termsRedistribution}
                onChange={(e) => setTermsRedistribution(e.target.value as Redistribution)}
              >
                <option value="unknown">İzin bilinmiyor</option>
                <option value="allowed">İzinli</option>
                <option value="not_allowed">İzinsiz</option>
              </select>
            </div>
            <div className="field" style={{ flex: "2 1 300px" }}>
              <label htmlFor="t-url">Koşul bağlantısı</label>
              <input
                id="t-url"
                className="input"
                value={termsUrl}
                onChange={(e) => setTermsUrl(e.target.value)}
                placeholder="https://…"
              />
            </div>
          </div>
          <div className="field">
            <label htmlFor="t-note">Yazılı dayanak notu</label>
            <textarea
              id="t-note"
              className="input"
              rows={3}
              style={{ height: "auto", minHeight: 70, padding: "8px 10px", resize: "vertical" }}
              value={termsNote}
              onChange={(e) => setTermsNote(e.target.value)}
              placeholder="Örn. Resmî Gazete 12.05.2025; kurumdan yazılı izin 03.06.2026"
            />
          </div>
          <div className="row">
            <button className="btn primary" disabled={busy} onClick={() => void saveTerms()}>
              Koşulu kaydet
            </button>
            <span className="hint" style={{ flex: "1 1 280px", margin: 0 }}>
              {terms ? termsRecordedText(terms) : "koşul kaydı yok — izin varsayılmaz"}
              {terms ? ` · ${usageBasisLabel(terms.usageBasis)} · ${redistributionLabel(terms.redistribution)}` : ""}
            </span>
          </div>
          {needsWrittenBasis(termsRedistribution) && !termsUrl.trim() && !termsNote.trim() ? (
            <p className="hint" style={{ margin: 0 }}>
              “İzinli” işaretlemek için kaynak bağlantısı veya not gerekir; aksi hâlde kayıt{" "}
              <code>E_TERMS</code> ile reddedilir ve izin bilinmiyor kalır.
            </p>
          ) : null}
          <p className="hint" style={{ margin: 0 }}>
            Bu kayıt hukuki görüş değildir; hangi dayanakla kütüphaneye alındığını belgeler. “İzinli”
            yalnız resmî yayın, açık lisans veya yazılı izinle işaretlenebilir; iç kullanım kararı dağıtılamaz.
          </p>
        </div>

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

          <div
            style={{
              borderTop: "1px solid var(--line)",
              paddingTop: 10,
              display: "flex",
              flexDirection: "column",
              gap: 8,
            }}
          >
            <div>
              <strong style={{ fontSize: 13 }}>Kasadaki belgeden aktar</strong>
              <p className="hint" style={{ margin: "2px 0 0" }}>
                Belgenin <b>ayrıştırılmış sayfa metni</b> birleştirilir; dosyanın kendisi
                kopyalanmaz. Hangi sürümden aktarılacağını sen seçersin.
              </p>
            </div>
            <div className="row">
              <div className="field" style={{ flex: "1 1 180px" }}>
                <label htmlFor="src-case">Dosya</label>
                <select
                  id="src-case"
                  className="input"
                  value={srcCaseId}
                  onChange={(e) => {
                    setSrcCaseId(e.target.value);
                    setSrcResult(null);
                  }}
                  disabled={cases.length === 0}
                >
                  {cases.length === 0 ? <option value="">Dosya yok</option> : null}
                  {cases.map((row) => (
                    <option key={row.id} value={row.id}>
                      {row.title}
                    </option>
                  ))}
                </select>
              </div>
              <div className="field" style={{ flex: "1 1 200px" }}>
                <label htmlFor="src-doc">Belge</label>
                <select
                  id="src-doc"
                  className="input"
                  value={srcDocId}
                  onChange={(e) => {
                    setSrcDocId(e.target.value);
                    setSrcResult(null);
                  }}
                  disabled={srcDocs.length === 0}
                >
                  {srcDocs.length === 0 ? <option value="">Belge yok</option> : null}
                  {srcDocs.map((row) => (
                    <option key={row.id} value={row.id}>
                      {row.displayName}
                    </option>
                  ))}
                </select>
              </div>
              <div className="field" style={{ flex: "1 1 170px" }}>
                <label htmlFor="src-version">Sürüm</label>
                <select
                  id="src-version"
                  className="input"
                  value={srcVersionId}
                  onChange={(e) => {
                    setSrcVersionId(e.target.value);
                    setSrcResult(null);
                  }}
                  disabled={srcVersions.length === 0}
                >
                  {srcVersions.length === 0 ? <option value="">Sürüm yok</option> : null}
                  {srcVersions.map((row) => (
                    <option key={row.id} value={row.id}>
                      {versionOptionLabel(row)}
                    </option>
                  ))}
                </select>
              </div>
              <button
                className="btn"
                disabled={
                  srcBusy ||
                  transferBlockedReason({
                    hasPrecedent: Boolean(selected),
                    documentCount: srcDocs.length,
                    versionCount: srcVersions.length,
                    selectedVersionId: srcVersionId,
                  }) !== null
                }
                onClick={() => void transferFromDocument()}
              >
                {srcBusy ? "Aktarılıyor…" : "Bu belgeden tam metni aktar"}
              </button>
            </div>
            {srcResult ? (
              <div
                style={{
                  border: "1px solid var(--line)",
                  borderRadius: "var(--radius)",
                  padding: "8px 10px",
                  display: "flex",
                  flexDirection: "column",
                  gap: 4,
                }}
              >
                <span className="small">{documentTextSummary(srcResult)}</span>
                <span className="small muted">
                  {srcResult.engine} · {srcResult.parseStatus} · ayrıştırma koşusu{" "}
                  {srcResult.parseRunId.slice(0, 8)}…
                </span>
                <span className="small">
                  {srcResult.written
                    ? "Yeni sürüm yazıldı ve alıntı doğrulaması bu metne göre yapılır."
                    : "Yeni sürüm yazılmadı; var olan sürüm gösteriliyor."}
                </span>
                {documentTextWarnings(srcResult).map((warning) => (
                  <span key={warning} className="small" style={{ color: "var(--warn)" }}>
                    {warning}
                  </span>
                ))}
                {srcResult.notes.map((note) => (
                  <span key={note} className="small muted">
                    {note}
                  </span>
                ))}
              </div>
            ) : (
              <span className="hint" style={{ margin: 0 }}>
                {transferBlockedReason({
                  hasPrecedent: Boolean(selected),
                  documentCount: srcDocs.length,
                  versionCount: srcVersions.length,
                  selectedVersionId: srcVersionId,
                }) ?? "Seçili sürümün sayfa metni aktarılacak; ayrıştırma yoksa metin yazılmaz."}
              </span>
            )}
          </div>

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
              disabled={busy || versionSummary.withText === 0 || quoteInput.trim().length === 0}
              onClick={() => void verifyQuote()}
            >
              Alıntıyı doğrula
            </button>
          </div>

          {verification ? (
            <div className={`notice${quoteStateTone(verification) === "ok" ? " ok" : quoteStateTone(verification) === "warn" ? " danger" : ""}`} role="status">
              {quoteStateText(verification)}
              {quoteScanNote(verification) ? (
                <div className="hint" style={{ marginTop: 6 }}>{quoteScanNote(verification)}</div>
              ) : null}
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
