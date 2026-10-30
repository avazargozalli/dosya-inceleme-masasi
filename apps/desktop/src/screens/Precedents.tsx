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
import { t } from "../lib/i18n";
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

function emptyDraft(): Draft {
  return {
    court: t("precedents.defaultCourt"),
    chamber: "",
    docketNo: "",
    decisionNo: "",
    decisionDate: "",
    title: "",
    sourceUrl: "",
    tags: "",
    legalIssue: "",
  };
}

/** Anlam incelemesi rozeti; bilinmeyen değer olduğu gibi görünür (uydurulmaz). */
function meaningLabel(value: string): string {
  switch (value) {
    case "not_reviewed":
      return t("precedents.meaningNotReviewed");
    case "accurate":
      return t("precedents.meaningAccurateBadge");
    case "misleading":
      return t("precedents.meaningMisleadingBadge");
    case "off_context":
      return t("precedents.meaningOffContextBadge");
    default:
      return value;
  }
}

export function Precedents() {
  const [rows, setRows] = useState<PrecedentRow[]>([]);
  const [selected, setSelected] = useState<PrecedentRow | null>(null);
  const [notes, setNotes] = useState<PrecedentNoteRow[]>([]);
  const [draft, setDraft] = useState<Draft>(emptyDraft);
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
  const [bulkCourt, setBulkCourt] = useState(t("precedents.defaultCourt"));
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
      setError({ code: "E_COURT", message: t("precedents.errorCourt"), retryable: false });
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
      setDraft(emptyDraft());
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
      setError({ code: "E_TEXT", message: t("precedents.errorEmptyText"), retryable: false });
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
      title: t("precedents.dialogSaveCorpus"),
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
      setError({
        code: "E_QUOTE",
        message: t("precedents.errorEmptyQuote"),
        retryable: false,
      });
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
        message: t("precedents.errorClipboard"),
        retryable: false,
      });
    }
  }

  async function pickBulkFiles() {
    try {
      const picked = await openFiles({
        multiple: true,
        title: t("precedents.dialogPickFiles"),
        filters: [
          {
            name: t("precedents.filterDecisionFiles"),
            extensions: ["pdf", "docx", "udf", "txt", "md"],
          },
        ],
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
        title: t("precedents.dialogManifest"),
        filters: [{ name: t("precedents.filterManifest"), extensions: ["csv", "txt"] }],
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
      setError({
        code: "E_BULK_EMPTY",
        message: t("precedents.errorBulkEmpty"),
        retryable: false,
      });
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
      setError({
        code: "E_CITATION",
        message: t("precedents.errorCitation"),
        retryable: false,
      });
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
        <h1>{t("precedents.title")}</h1>
        <p className="lead">{t("precedents.lead")}</p>
        <p className="hint">
          {t("precedents.hintIntro")} <strong>{t("precedents.hintLibrary")}</strong>{" "}
          {t("precedents.hintMid")} <em>{t("precedents.hintDocScope")}</em>{" "}
          {t("precedents.hintMid2")}
          <em>{t("precedents.hintPrecedentScope")}</em>
          {t("precedents.hintTail")}
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
            <h2>{t("precedents.libraryTitle")}</h2>
            <p className="card-sub">
              {t("precedents.libraryCount", { count: rows.length })}
            </p>
          </div>
        </div>
        <div className="row">
          <div className="field"><label htmlFor="p-court">{t("precedents.fieldCourt")}</label><input id="p-court" className="input" value={draft.court} onChange={(e) => setDraft({ ...draft, court: e.target.value })} /></div>
          <div className="field"><label htmlFor="p-chamber">{t("precedents.fieldChamber")}</label><input id="p-chamber" className="input" value={draft.chamber} onChange={(e) => setDraft({ ...draft, chamber: e.target.value })} /></div>
          <div className="field"><label htmlFor="p-docket">{t("precedents.fieldDocket")}</label><input id="p-docket" className="input" value={draft.docketNo} onChange={(e) => setDraft({ ...draft, docketNo: e.target.value })} placeholder={t("precedents.phDocket")} /></div>
          <div className="field"><label htmlFor="p-decision">{t("precedents.fieldDecision")}</label><input id="p-decision" className="input" value={draft.decisionNo} onChange={(e) => setDraft({ ...draft, decisionNo: e.target.value })} placeholder={t("precedents.phDecision")} /></div>
          <div className="field"><label htmlFor="p-date">{t("precedents.fieldDate")}</label><input id="p-date" className="input" type="date" value={draft.decisionDate} onChange={(e) => setDraft({ ...draft, decisionDate: e.target.value })} /></div>
        </div>
        <div className="row">
          <div className="field" style={{ flex: "2 1 300px" }}><label htmlFor="p-title">{t("precedents.fieldTitle")}</label><input id="p-title" className="input" value={draft.title} onChange={(e) => setDraft({ ...draft, title: e.target.value })} /></div>
          <div className="field" style={{ flex: "2 1 300px" }}><label htmlFor="p-url">{t("precedents.fieldUrl")}</label><input id="p-url" className="input" value={draft.sourceUrl} onChange={(e) => setDraft({ ...draft, sourceUrl: e.target.value })} placeholder={t("precedents.phUrl")} /></div>
          <div className="field"><label htmlFor="p-tags">{t("precedents.fieldTags")}</label><input id="p-tags" className="input" value={draft.tags} onChange={(e) => setDraft({ ...draft, tags: e.target.value })} placeholder={t("precedents.phTags")} /></div>
          <button className="btn primary" disabled={busy} onClick={() => void add()}>{t("precedents.add")}</button>
        </div>

        {rows.length === 0 ? (
          <div className="empty"><p style={{ margin: 0 }}>{t("precedents.emptyLibrary")}</p></div>
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
                    {row.court} {row.chamber ?? ""} {row.docketNo ?? ""}{" "}
                    {row.decisionNo
                      ? t("precedents.decisionShort", { no: row.decisionNo })
                      : ""}
                  </strong>
                  <span style={{ display: "flex", gap: 4 }}>
                    <span className={`badge ${termsBadge(row).tone}`}>{termsBadge(row).text}</span>
                    <span className={`badge ${row.completeness === "complete" ? "ok" : "warn"}`}>{row.completeness}</span>
                  </span>
                </span>
                <span className="muted small">
                  {row.title ?? t("precedents.untitled")} ·{" "}
                  {row.decisionDate ?? t("precedents.noDate")}
                </span>
              </button>
            ))}
          </div>
        )}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("precedents.bulkTitle")}</h2>
            <p className="card-sub">{t("precedents.bulkSub")}</p>
          </div>
        </div>
        <div className="row">
          <button className="btn" disabled={bulkBusy} onClick={() => void pickBulkFiles()}>
            {bulkFiles.length === 0
              ? t("precedents.bulkPick")
              : t("precedents.bulkPicked", { count: bulkFiles.length })}
          </button>
          <button className="btn ghost" disabled={bulkBusy} onClick={() => void pickManifest()}>
            {manifestPath
              ? t("precedents.bulkManifestChosen", {
                  name: manifestPath.split("/").pop() ?? "",
                })
              : t("precedents.bulkPickManifest")}
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
              {t("precedents.bulkClear")}
            </button>
          ) : null}
        </div>
        <div className="row">
          <div className="field"><label htmlFor="b-court">{t("precedents.bulkDefaultCourt")}</label><input id="b-court" className="input" value={bulkCourt} onChange={(e) => setBulkCourt(e.target.value)} /></div>
          <div className="field"><label htmlFor="b-chamber">{t("precedents.bulkDefaultChamber")}</label><input id="b-chamber" className="input" value={bulkChamber} onChange={(e) => setBulkChamber(e.target.value)} /></div>
          <div className="field"><label htmlFor="b-url">{t("precedents.fieldUrl")}</label><input id="b-url" className="input" value={bulkSourceUrl} onChange={(e) => setBulkSourceUrl(e.target.value)} placeholder={t("precedents.phUrl")} /></div>
          <div className="field"><label htmlFor="b-tags">{t("precedents.fieldTags")}</label><input id="b-tags" className="input" value={bulkTags} onChange={(e) => setBulkTags(e.target.value)} placeholder={t("precedents.phTags")} /></div>
        </div>
        <div className="row">
          <button
            className="btn primary"
            disabled={bulkBusy || bulkFiles.length === 0}
            onClick={() => void runBulkImport()}
          >
            {bulkBusy ? t("precedents.bulkBusy") : t("precedents.bulkRun")}
          </button>
          <span className="hint" style={{ flex: "1 1 280px", margin: 0 }}>
            {t("precedents.bulkColumns")}
          </span>
        </div>
        {bulkReport ? (
          <div role="status">
            <p className="small" style={{ margin: "4px 0" }}>
              {bulkReportSummary(bulkReport)}
              {bulkReport.manifestHash
                ? ` · ${t("precedents.bulkManifestHash", {
                    hash: bulkReport.manifestHash.slice(0, 12),
                  })}`
                : ""}
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
                {t("precedents.bulkConflictNote")}
              </p>
            ) : null}
          </div>
        ) : null}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("precedents.corpusTitle")}</h2>
            <p className="card-sub">{t("precedents.corpusSub")}</p>
          </div>
          {coverage ? (
            <span className={`badge ${corpusGateOpen(coverage) ? "ok" : "warn"}`}>
              {corpusGateOpen(coverage)
                ? t("precedents.corpusGateOpen")
                : t("precedents.corpusGateClosed")}
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
                    {t("precedents.corpusMoreBlocked", {
                      count: coverage.blockedTotal - coverage.blocked.length,
                    })}
                  </p>
                ) : null}
              </div>
            ) : null}
            <div className="row">
              <div className="field">
                <label htmlFor="corpus-mode">{t("precedents.modeLabel")}</label>
                <select
                  id="corpus-mode"
                  className="input"
                  value={corpusMode}
                  onChange={(e) => setCorpusMode(e.target.value as "permitted_only" | "all")}
                >
                  <option value="permitted_only">{t("precedents.modePermitted")}</option>
                  <option value="all">{t("precedents.modeAll")}</option>
                </select>
              </div>
              <button
                className="btn primary"
                disabled={corpusBusy || coverage.allowed === 0}
                onClick={() => void exportCorpus()}
              >
                {corpusBusy ? t("precedents.corpusBusy") : t("precedents.corpusExport")}
              </button>
            </div>
            {corpusMode === "all" && !corpusGateOpen(coverage) ? (
              <p className="hint" style={{ margin: 0 }}>
                {t("precedents.corpusClosedNote", { blocked: coverage.blockedTotal })}
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
          <p className="hint" style={{ margin: 0 }}>{t("precedents.coverageLoading")}</p>
        )}
      </div>

      {selected ? (
        <>
        <div className="card">
          <div className="card-head">
            <div>
              <h2>{t("precedents.termsTitle")}</h2>
              <p className="card-sub">{t("precedents.termsSub")}</p>
            </div>
            <span className={`badge ${terms ? termsBadge(terms).tone : "warn"}`}>
              {terms ? termsBadge(terms).text : t("precedents.termsNoRecord")}
            </span>
          </div>
          <div className="row">
            <div className="field">
              <label htmlFor="t-basis">{t("precedents.termsBasis")}</label>
              <select
                id="t-basis"
                className="input"
                value={termsBasis}
                onChange={(e) => setTermsBasis(e.target.value as UsageBasis)}
              >
                <option value="unknown">{t("precedents.basisUnknown")}</option>
                <option value="official_public">{t("precedents.basisOfficial")}</option>
                <option value="licence">{t("precedents.basisLicence")}</option>
                <option value="permission">{t("precedents.basisPermission")}</option>
                <option value="internal_only">{t("precedents.basisInternal")}</option>
              </select>
            </div>
            <div className="field">
              <label htmlFor="t-redistribution">{t("precedents.termsRedistribution")}</label>
              <select
                id="t-redistribution"
                className="input"
                value={termsRedistribution}
                onChange={(e) => setTermsRedistribution(e.target.value as Redistribution)}
              >
                <option value="unknown">{t("precedents.redistributionUnknown")}</option>
                <option value="allowed">{t("precedents.redistributionAllowed")}</option>
                <option value="not_allowed">{t("precedents.redistributionNotAllowed")}</option>
              </select>
            </div>
            <div className="field" style={{ flex: "2 1 300px" }}>
              <label htmlFor="t-url">{t("precedents.termsUrl")}</label>
              <input
                id="t-url"
                className="input"
                value={termsUrl}
                onChange={(e) => setTermsUrl(e.target.value)}
                placeholder={t("precedents.phUrl")}
              />
            </div>
          </div>
          <div className="field">
            <label htmlFor="t-note">{t("precedents.termsNote")}</label>
            <textarea
              id="t-note"
              className="input"
              rows={3}
              style={{ height: "auto", minHeight: 70, padding: "8px 10px", resize: "vertical" }}
              value={termsNote}
              onChange={(e) => setTermsNote(e.target.value)}
              placeholder={t("precedents.phTermsNote")}
            />
          </div>
          <div className="row">
            <button className="btn primary" disabled={busy} onClick={() => void saveTerms()}>
              {t("precedents.termsSave")}
            </button>
            <span className="hint" style={{ flex: "1 1 280px", margin: 0 }}>
              {terms ? termsRecordedText(terms) : t("terms.recordedNone")}
              {terms ? ` · ${usageBasisLabel(terms.usageBasis)} · ${redistributionLabel(terms.redistribution)}` : ""}
            </span>
          </div>
          {needsWrittenBasis(termsRedistribution) && !termsUrl.trim() && !termsNote.trim() ? (
            <p className="hint" style={{ margin: 0 }}>
              {t("precedents.termsBasisNeeded")} <code>E_TERMS</code>{" "}
              {t("precedents.termsBasisNeededTail")}
            </p>
          ) : null}
          <p className="hint" style={{ margin: 0 }}>
            {t("precedents.termsDisclaimer")}
          </p>
        </div>

        <div className="card">
          <div className="card-head">
            <div>
              <h2>{t("precedents.bibTitle")}</h2>
              <p className="card-sub">{t("precedents.bibSub")}</p>
            </div>
            {bibliography ? (
              <button className="btn ghost" onClick={() => void copyBibliography()}>
                {copied ? t("precedents.bibCopied") : t("precedents.bibCopy")}
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
                <span className="badge plain">
                  {t("precedents.bibAccessed", { at: bibliography.accessedAt })}
                </span>
                <span className={`badge ${bibliography.hasFullText ? "ok" : "warn"}`}>
                  {bibliography.hasFullText
                    ? t("precedents.bibHasText")
                    : t("precedents.bibNoText")}
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
                    {t("precedents.bibOpenSource")}
                  </a>
                </p>
              ) : null}
            </>
          ) : (
            <p className="hint" style={{ margin: 0 }}>{t("precedents.bibLoading")}</p>
          )}
        </div>

        <div className="card">
          <div className="card-head">
            <div>
              <h2>{t("precedents.notesTitle")}</h2>
              <p className="card-sub">{t("precedents.notesSub", { count: notes.length })}</p>
            </div>
            <button className="btn ghost" onClick={() => setSelected(null)}>
              {t("precedents.close")}
            </button>
          </div>
          {notes.map((note) => (
            <div key={note.id} className="card tight" style={{ background: "var(--surface)" }}>
              <div className="muted small">
                {note.caseIdOptional
                  ? t("precedents.noteCaseScoped")
                  : t("precedents.noteLibrary")}{" "}
                · {formatTimestamp(note.createdAt)}
              </div>
              {note.similarityNote ? <p style={{ margin: "4px 0" }}><strong>{t("precedents.noteSimilarity")}</strong> {note.similarityNote}</p> : null}
              {note.distinguishingNote ? <p style={{ margin: "4px 0" }}><strong>{t("precedents.noteDistinguishing")}</strong> {note.distinguishingNote}</p> : null}
              {note.userConclusion ? <p style={{ margin: "4px 0" }}><strong>{t("precedents.noteConclusion")}</strong> {note.userConclusion}</p> : null}
            </div>
          ))}
          <div className="row">
            <div className="field"><label htmlFor="n-sim">{t("precedents.fieldSimilarity")}</label><input id="n-sim" className="input" value={similarity} onChange={(e) => setSimilarity(e.target.value)} /></div>
            <div className="field"><label htmlFor="n-dist">{t("precedents.fieldDistinguishing")}</label><input id="n-dist" className="input" value={distinguishing} onChange={(e) => setDistinguishing(e.target.value)} /></div>
            <div className="field"><label htmlFor="n-conc">{t("precedents.fieldConclusion")}</label><input id="n-conc" className="input" value={conclusion} onChange={(e) => setConclusion(e.target.value)} /></div>
            <button className="btn" disabled={busy} onClick={() => void addNote()}>{t("precedents.noteAdd")}</button>
          </div>
          <p className="hint" style={{ margin: 0 }}>{t("precedents.noteDisclaimer")}</p>
        </div>

        <div className="card">
          <div className="card-head">
            <div>
              <h2>{t("precedents.textTitle")}</h2>
              <p className="card-sub">
                {t("precedents.textSub", {
                  versions: versions.length,
                  withText: versionSummary.withText,
                })}
                {versions.length > 0 && !versionSummary.latestHasText
                  ? ` · ${t("precedents.textSubNoLatest")}`
                  : ""}
              </p>
            </div>
          </div>

          {versions.length === 0 ? (
            <div className="empty"><p style={{ margin: 0 }}>{t("precedents.textEmpty")}</p></div>
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
                    <span className="muted">
                      {" "}
                      · {t("precedents.textHash", { hash: version.bodyHash.slice(0, 12) })}
                    </span>
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
              <strong style={{ fontSize: 13 }}>{t("precedents.transferTitle")}</strong>
              <p className="hint" style={{ margin: "2px 0 0" }}>
                {t("precedents.transferLead1")} <b>{t("precedents.transferLeadStrong")}</b>{" "}
                {t("precedents.transferLead2")}
              </p>
            </div>
            <div className="row">
              <div className="field" style={{ flex: "1 1 180px" }}>
                <label htmlFor="src-case">{t("precedents.fieldCase")}</label>
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
                  {cases.length === 0 ? (
                    <option value="">{t("precedents.noCaseOption")}</option>
                  ) : null}
                  {cases.map((row) => (
                    <option key={row.id} value={row.id}>
                      {row.title}
                    </option>
                  ))}
                </select>
              </div>
              <div className="field" style={{ flex: "1 1 200px" }}>
                <label htmlFor="src-doc">{t("precedents.fieldDocument")}</label>
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
                  {srcDocs.length === 0 ? (
                    <option value="">{t("precedents.noDocumentOption")}</option>
                  ) : null}
                  {srcDocs.map((row) => (
                    <option key={row.id} value={row.id}>
                      {row.displayName}
                    </option>
                  ))}
                </select>
              </div>
              <div className="field" style={{ flex: "1 1 170px" }}>
                <label htmlFor="src-version">{t("precedents.fieldVersion")}</label>
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
                  {srcVersions.length === 0 ? (
                    <option value="">{t("precedents.noVersionOption")}</option>
                  ) : null}
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
                {srcBusy ? t("precedents.transferBusy") : t("precedents.transferRun")}
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
                  {srcResult.engine} · {srcResult.parseStatus} ·{" "}
                  {t("precedents.transferRunLabel", {
                    id: srcResult.parseRunId.slice(0, 8),
                  })}
                </span>
                <span className="small">
                  {srcResult.written
                    ? t("precedents.transferWritten")
                    : t("precedents.transferUnchanged")}
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
                }) ?? t("precedents.transferHint")}
              </span>
            )}
          </div>

          <div className="field">
            <label htmlFor="p-fulltext">{t("precedents.fullTextLabel")}</label>
            <textarea
              id="p-fulltext"
              className="input"
              rows={5}
              style={{ height: "auto", minHeight: 110, padding: "8px 10px", resize: "vertical" }}
              value={fullText}
              onChange={(e) => setFullText(e.target.value)}
              placeholder={t("precedents.phFullText")}
            />
          </div>
          <div className="row">
            <button className="btn" disabled={busy || fullText.trim().length === 0} onClick={() => void addFullText()}>
              {t("precedents.fullTextSave")}
            </button>
            <span className="hint" style={{ flex: "1 1 260px", margin: 0 }}>
              {t("precedents.fullTextHint")}
            </span>
          </div>

          <div className="row">
            <div className="field" style={{ flex: "2 1 320px" }}>
              <label htmlFor="p-quote">{t("precedents.quoteLabel")}</label>
              <input
                id="p-quote"
                className="input"
                value={quoteInput}
                onChange={(e) => setQuoteInput(e.target.value)}
                placeholder={t("precedents.phQuote")}
              />
            </div>
            <button
              className="btn primary"
              disabled={busy || versionSummary.withText === 0 || quoteInput.trim().length === 0}
              onClick={() => void verifyQuote()}
            >
              {t("precedents.quoteVerify")}
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
                    {t("precedents.quoteToCitation", {
                      state: quoteToCitationMatch(verification),
                    })}
                  </button>
                </div>
              ) : null}
            </div>
          ) : null}
          <p className="hint" style={{ margin: 0 }}>
            {t("precedents.quoteHint")}
          </p>
        </div>
        </>
      ) : null}

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("precedents.checkTitle")}</h2>
            <p className="card-sub">{t("precedents.checkSub", { count: checks.length })}</p>
          </div>
        </div>
        <div className="row">
          <div className="field">
            <label htmlFor="c-case">{t("precedents.fieldCase")}</label>
            <select id="c-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
              {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
            </select>
          </div>
          <div className="field">
            <label htmlFor="c-doc">{t("precedents.fieldDocVersion")}</label>
            <select id="c-doc" className="input" value={versionId} onChange={(e) => setVersionId(e.target.value)}>
              <option value="">{t("common.select")}</option>
              {docs.filter((d) => d.currentVersionId).map((d) => (
                <option key={d.id} value={d.currentVersionId!}>{d.displayName}</option>
              ))}
            </select>
          </div>
          <div className="field" style={{ flex: "2 1 260px" }}>
            <label htmlFor="c-text">{t("precedents.fieldCitation")}</label>
            <input id="c-text" className="input" value={citation} onChange={(e) => setCitation(e.target.value)} placeholder={t("precedents.phCitation")} />
          </div>
        </div>
        <div className="row">
          <div className="field">
            <label htmlFor="c-cand">{t("precedents.fieldCandidate")}</label>
            <select id="c-cand" className="input" value={candidate} onChange={(e) => setCandidate(e.target.value)}>
              <option value="">{t("precedents.noCandidate")}</option>
              {rows.map((r) => <option key={r.id} value={r.id}>{r.court} {r.docketNo ?? ""}</option>)}
            </select>
          </div>
          <div className="field">
            <label htmlFor="c-id">{t("precedents.fieldIdentity")}</label>
            <select id="c-id" className="input" value={identity} onChange={(e) => setIdentity(e.target.value as typeof identity)}>
              <option value="unknown">{t("precedents.identityUnknown")}</option>
              <option value="match">{t("precedents.identityMatch")}</option>
              <option value="mismatch">{t("precedents.identityMismatch")}</option>
            </select>
          </div>
          <div className="field">
            <label htmlFor="c-quote">{t("precedents.fieldQuote")}</label>
            <select id="c-quote" className="input" value={quoteMatch} onChange={(e) => setQuoteMatch(e.target.value as typeof quoteMatch)}>
              <option value="not_checked">{t("precedents.quoteNotChecked")}</option>
              <option value="unknown">{t("precedents.identityUnknown")}</option>
              <option value="match">{t("precedents.identityMatch")}</option>
              <option value="mismatch">{t("precedents.identityMismatch")}</option>
            </select>
          </div>
          <button className="btn" disabled={busy} onClick={() => void addCheck()}>{t("precedents.checkSave")}</button>
        </div>
        {checks.length > 0 ? (
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            {checks.map((check) => (
              <div key={check.id} style={{ padding: "6px 8px", border: "1px solid var(--line)", borderRadius: "var(--radius)" }}>
                <div className="row" style={{ justifyContent: "space-between" }}>
                  <span className="small">{check.extractedCitation}</span>
                  <span style={{ display: "flex", gap: 6, alignItems: "center" }}>
                    <span className="badge plain">
                      {t("precedents.badgeIdentity", { value: check.identityMatch })}
                    </span>
                    <span className="badge plain">
                      {t("precedents.badgeQuote", { value: check.quoteMatch })}
                    </span>
                    <span className={`badge ${check.meaningReview === "accurate" ? "ok" : check.meaningReview === "not_reviewed" ? "plain" : "warn"}`}>
                      {t("precedents.badgeMeaning", {
                        value: meaningLabel(check.meaningReview),
                      })}
                    </span>
                    <button className="btn ghost" onClick={() => void openMeaning(check.id)}>
                      {meaningFor === check.id
                        ? t("precedents.close")
                        : t("precedents.meaning")}
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
                          {t("precedents.contextMissing", {
                            reason: meaningContext.reason ?? meaningContext.state,
                          })}
                        </p>
                      )
                    ) : (
                      <p className="hint" style={{ margin: "0 0 6px" }}>
                        {t("precedents.contextLoading")}
                      </p>
                    )}
                    <div className="field">
                      <label htmlFor={`mn-${check.id}`}>
                        {t("precedents.meaningNoteLabel")}
                      </label>
                      <input
                        id={`mn-${check.id}`}
                        className="input"
                        value={meaningNote}
                        onChange={(e) => setMeaningNote(e.target.value)}
                        placeholder={t("precedents.phMeaningNote")}
                      />
                    </div>
                    <div className="row" style={{ gap: 6 }}>
                      <button className="btn" onClick={() => void reviewMeaning(check.id, "accurate")}>
                        {t("precedents.meaningAccurate")}
                      </button>
                      <button className="btn ghost" onClick={() => void reviewMeaning(check.id, "misleading")}>
                        {t("precedents.meaningMisleading")}
                      </button>
                      <button className="btn ghost" onClick={() => void reviewMeaning(check.id, "off_context")}>
                        {t("precedents.meaningOffContext")}
                      </button>
                    </div>
                  </div>
                ) : null}
              </div>
            ))}
          </div>
        ) : null}
        <p className="hint" style={{ margin: 0 }}>
          {t("precedents.checkHint")}
        </p>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("precedents.connectorsTitle")}</h2>
            <p className="card-sub">
              {connectors
                ? t("precedents.connectorsCount", {
                    count: connectors.connectors.length,
                  })
                : t("precedents.connectorsLoading")}
            </p>
          </div>
        </div>
        {connectors ? (
          <>
            {connectors.connectors.map((connector) => (
              <div key={connector.id} className="card tight" style={{ background: "var(--surface)" }}>
                <div className="row" style={{ justifyContent: "space-between" }}>
                  <strong style={{ fontWeight: 500 }}>{connector.name}</strong>
                  <span className="badge warn">{t("precedents.connectorInactive")}</span>
                </div>
                <p className="muted small" style={{ margin: "2px 0" }}>{connector.operator}</p>
                <p className="small" style={{ margin: "4px 0" }}>{connector.scope}</p>
                <p className="small" style={{ margin: "4px 0" }}>{connector.reuseNote}</p>
                <p className="hint" style={{ margin: 0 }}>
                  {connector.accessMethod}{" "}
                  <a href={connector.termsUrl} target="_blank" rel="noreferrer">
                    {t("precedents.connectorTerms")}
                  </a>
                </p>
              </div>
            ))}
            <p className="hint" style={{ margin: 0 }}>
              {connectors.note}{" "}
              <span className="muted">
                {t("precedents.connectorHash", {
                  hash: connectors.catalogHash.slice(0, 12),
                })}
              </span>
            </p>
          </>
        ) : null}
      </div>
    </div>
  );
}
