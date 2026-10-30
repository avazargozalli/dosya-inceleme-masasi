import { useCallback, useEffect, useRef, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  toApiError,
  type ApiError,
  type AnchorRow,
  type CaseRow,
  type BatchParseResult,
  type DocumentRow,
  type NewCaseInput,
  type ParseProgressEvent,
} from "../lib/api";
import { AI_POLICY_LABEL, CASE_KIND_LABEL, formatTimestamp, SENSITIVITY_LABEL } from "../lib/format";
import { t } from "../lib/i18n";
import { DEFAULT_OCR_LANG, defaultOcrLanguageLabel } from "../lib/ocr";
import { parseProgressLine, parseProgressWidth } from "../lib/parseProgress";
import { budgetSummary, budgetWarning } from "../lib/budget";
import { Notes } from "../components/Notes";
import { DualReader } from "../components/DualReader";
import { targetFromAnchor, type ReaderTarget } from "../lib/reader";

const EMPTY_FORM: NewCaseInput = {
  title: "",
  caseKind: "review",
  referenceCode: null,
  jurisdictionLabel: null,
  practiceArea: null,
  sensitivity: "normal",
  defaultAiPolicy: "off",
};

export function Cases() {
  const [rows, setRows] = useState<CaseRow[]>([]);
  const [showArchived, setShowArchived] = useState(false);
  const [selected, setSelected] = useState<CaseRow | null>(null);
  const [creating, setCreating] = useState(false);
  const [form, setForm] = useState<NewCaseInput>(EMPTY_FORM);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  const [docs, setDocs] = useState<DocumentRow[]>([]);
  const [tagsByDoc, setTagsByDoc] = useState<Record<string, string[]>>({});
  const [tagDrafts, setTagDrafts] = useState<Record<string, string>>({});
  const [notice, setNotice] = useState<string | null>(null);
  const [reader, setReader] = useState<{ left: ReaderTarget | null; right: ReaderTarget | null } | null>(null);
  const [anchors, setAnchors] = useState<AnchorRow[]>([]);
  const [pdfPassword, setPdfPassword] = useState("");
  // F06-T07: düz metin kodlaması. Varsayılan otomatik; yanlış çözülürse
  // kullanıcı aynı belgeyi açık kodlamayla yeniden ayrıştırabilir.
  const [txtEncoding, setTxtEncoding] = useState("auto");
  // F04-T07: canlı aşama/sayım. `parse-progress` olayı yalnız aşama ve sayaç
  // taşır; satır, geçerli ayrıştırmanın sürümüne aitse gösterilir.
  const [parseProgress, setParseProgress] = useState<ParseProgressEvent | null>(null);
  const activeParseVersion = useRef<string | null>(null);
  // §17.3 toplu ayrıştırma: bekleyen belgeler en fazla iki worker'la. Sonuç
  // belge başına gerçektir; kaç belgenin gerçekten ayrıştığı yazılır.
  const [batchBusy, setBatchBusy] = useState(false);
  const [batchResult, setBatchResult] = useState<BatchParseResult | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    // Tarayıcıda (Tauri yok) dinleyici kurulamaz; hata yutulur, ayrıştırma
    // yalnızca uygulama içinde çalışır.
    void listen<ParseProgressEvent>("parse-progress", (event) => {
      if (disposed) return;
      if (event.payload.versionId !== activeParseVersion.current) return;
      setParseProgress(event.payload);
    })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const load = useCallback(async () => {
    try {
      setError(null);
      setRows(await api.caseList(showArchived));
    } catch (e) {
      setError(toApiError(e));
    }
  }, [showArchived]);

  useEffect(() => {
    void load();
  }, [load]);

  const loadDocs = useCallback(async (caseId: string) => {
    try {
      setDocs(await api.documentList(caseId));
    } catch (e) {
      setError(toApiError(e));
    }
  }, []);

  useEffect(() => {
    if (selected) {
      void loadDocs(selected.id);
      void api.anchorList(selected.id).then(setAnchors).catch((e) => setError(toApiError(e)));
      void api
        .documentTagsForCase(selected.id)
        .then((rows) => {
          const grouped: Record<string, string[]> = {};
          for (const row of rows) {
            (grouped[row.documentId] ??= []).push(row.tag);
          }
          setTagsByDoc(grouped);
        })
        .catch((e) => setError(toApiError(e)));
    } else {
      setDocs([]);
      setAnchors([]);
      setTagsByDoc({});
    }
  }, [selected, loadDocs]);

  // Dosya değişince önceki dosyanın okuyucu hedefleri taşınmaz.
  useEffect(() => {
    setReader(null);
  }, [selected?.id]);

  async function create() {
    if (form.title.trim().length === 0) {
      setError({ code: "E_TITLE", message: t("cases.titleRequired"), retryable: false });
      return;
    }
    setBusy(true);
    try {
      const created = await api.caseCreate({
        ...form,
        title: form.title.trim(),
        referenceCode: form.referenceCode?.trim() || null,
        practiceArea: form.practiceArea?.trim() || null,
      });
      setCreating(false);
      setForm(EMPTY_FORM);
      await load();
      setSelected(created);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function addDocument() {
    const picked = await openFileDialog({
      multiple: false,
      directory: false,
      filters: [
        { name: t("cases.fileFilterDocuments"), extensions: ["pdf", "docx", "udf", "txt", "md", "csv", "png", "jpg", "jpeg", "tif", "tiff"] },
      ],
    });
    if (typeof picked !== "string") return;
    await importFromPath(picked);
  }

  async function importFromPath(path: string) {
    if (!selected) return;
    setBusy(true);
    setNotice(null);
    try {
      const outcome = await api.documentImport(selected.id, path);
      if (outcome.kind === "duplicate") {
        setNotice(t("cases.duplicateNotice", { name: outcome.displayName }));
      } else {
        setNotice(
          t("cases.imported", {
            name: outcome.document.displayName,
            version: outcome.version.versionNo,
          }),
        );
      }
      await loadDocs(selected.id);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  /** Belge etiketi ekler; sunucunun döndürdüğü güncel listeyi yazar.
   *  Etiket aramada filtre olarak kullanılır (F07-T03). */
  async function addTag(documentId: string) {
    const draft = (tagDrafts[documentId] ?? "").trim();
    if (!draft) return;
    try {
      const tags = await api.documentTagAdd(documentId, draft);
      setTagsByDoc((prev) => ({ ...prev, [documentId]: tags }));
      setTagDrafts((prev) => ({ ...prev, [documentId]: "" }));
    } catch (e) {
      setError(toApiError(e));
    }
  }

  async function removeTag(documentId: string, tag: string) {
    try {
      const tags = await api.documentTagRemove(documentId, tag);
      setTagsByDoc((prev) => ({ ...prev, [documentId]: tags }));
    } catch (e) {
      setError(toApiError(e));
    }
  }

  async function parseDocument(versionId: string, ocr = false) {
    setBusy(true);
    setNotice(null);
    setParseProgress(null);
    activeParseVersion.current = versionId;
    try {
      const result = await api.documentParse(
        versionId,
        ocr,
        DEFAULT_OCR_LANG,
        txtEncoding,
        pdfPassword.trim() === "" ? undefined : pdfPassword,
      );
      // Hangi kodlamanın kullanıldığı worker uyarısından okunur; tahmin edilmez.
      const encodingNote = result.warnings.find((warning) => warning.includes("kodlama"));
      // §17.3: uygulanan bütçe ve gözlenen tepe değerler çekirdekten gelir.
      const budgetLine = result.budget ? budgetSummary(result.budget) : "";
      // Başarılı işte ihlal beklenmez (ihlal işi durdurur); yine de görülürse
      // hata olarak değil, açık bir uyarı satırı olarak yazılır.
      const budgetAlert = result.budget ? budgetWarning(result.budget) : null;
      const parts = [
        t("cases.parseSummary", {
          status: result.parseRun.status,
          pages: result.pages,
          segments: result.storedSegments,
        }),
      ];
      if (result.warnings.length > 0) {
        parts.push(t("cases.parseWarnings", { count: result.warnings.length }));
      }
      if (encodingNote) parts.push(encodingNote);
      if (budgetLine) parts.push(budgetLine);
      if (budgetAlert) parts.push(budgetAlert);
      setNotice(parts.join(" · "));
    } catch (e) {
      setError(toApiError(e));
    } finally {
      activeParseVersion.current = null;
      setParseProgress(null);
      setBusy(false);
    }
  }

  /** §17.3: bekleyen belgeleri en fazla iki worker'la ayrıştırır. */
  async function parsePending() {
    if (!selected) return;
    setBatchBusy(true);
    setError(null);
    setBatchResult(null);
    try {
      const result = await api.documentsParsePending(selected.id, 4);
      setBatchResult(result);
      await loadDocs(selected.id);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBatchBusy(false);
    }
  }

  /** Belgeyi sol panelde açar; sağ paneldeki belge korunur (F07-T08). */
  function openDocument(doc: DocumentRow) {
    if (!doc.currentVersionId) return;
    setReader((prev) => ({
      left: {
        versionId: doc.currentVersionId!,
        documentName: doc.displayName,
        pageId: null,
        segmentId: null,
        pageIndex: null,
        startOffset: null,
        endOffset: null,
        quote: null,
        query: null,
        anchorId: null,
        quoteIntact: null,
        quads: [],
      },
      right: prev?.right ?? null,
    }));
  }

  /** Kaynağa geri dönüş: anchor'ın sayfasını ve alıntısını okuyucuda açar.
   *  Alıntı segmentten kopmuşsa okuyucu bunu "kopmuş kaynak" olarak gösterir. */
  async function openAnchor(anchorId: string) {
    try {
      const location = await api.anchorLocate(anchorId);
      if (!location) {
        setError({
          code: "E_ANCHOR_MISSING",
          message: t("cases.anchorMissing"),
          retryable: false,
        });
        return;
      }
      setError(null);
      setReader((prev) => ({ left: targetFromAnchor(location), right: prev?.right ?? null }));
    } catch (e) {
      setError(toApiError(e));
    }
  }

  // Dosya sürükle-bırak: Tauri pencere olayı yol verir, HTML5 drop vermez.
  useEffect(() => {
    if (!selected) return;
    let unlisten: (() => void) | undefined;
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type !== "drop") return;
        const path = event.payload.paths[0];
        if (path) void importFromPath(path);
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => undefined);
    return () => {
      unlisten?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected?.id]);

  async function mutate(action: "archive" | "restore" | "delete") {
    if (!selected) return;
    setBusy(true);
    try {
      if (action === "archive") await api.caseArchive(selected.id);
      if (action === "restore") await api.caseRestore(selected.id);
      if (action === "delete") await api.caseDelete(selected.id);
      setSelected(null);
      await load();
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page" style={{ maxWidth: 980 }}>
      <div className="page-head">
        <h1>{t("nav.files")}</h1>
        <p className="lead">{t("cases.lead")}</p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("cases.listTitle")}</h2>
            <p className="card-sub">
              {t("cases.listSubtitle", {
                count: rows.length,
                scope: showArchived ? t("cases.scopeArchived") : t("cases.scopeActive"),
              })}
            </p>
          </div>
          <div className="row" style={{ flexWrap: "nowrap" }}>
            <label className="row" style={{ alignItems: "center", gap: 6 }}>
              <input
                type="checkbox"
                checked={showArchived}
                onChange={(e) => setShowArchived(e.target.checked)}
                style={{ width: 15, height: 15 }}
              />
              <span className="small muted">{t("cases.archiveToggle")}</span>
            </label>
            <button className="btn primary" onClick={() => setCreating((v) => !v)}>
              {creating ? t("cases.cancel") : t("cases.newCase")}
            </button>
          </div>
        </div>

        {creating ? (
          <div className="card tight" style={{ background: "var(--surface)" }}>
            <div className="row">
              <div className="field" style={{ flex: "2 1 320px" }}>
                <label htmlFor="case-title">{t("cases.fieldTitle")}</label>
                <input
                  id="case-title"
                  className="input"
                  value={form.title}
                  onChange={(e) => setForm({ ...form, title: e.target.value })}
                  placeholder={t("cases.titlePlaceholder")}
                  autoFocus
                />
              </div>
              <div className="field">
                <label htmlFor="case-kind">{t("cases.fieldKind")}</label>
                <select
                  id="case-kind"
                  className="input"
                  value={form.caseKind}
                  onChange={(e) => setForm({ ...form, caseKind: e.target.value as NewCaseInput["caseKind"] })}
                >
                  <option value="review">{t("labels.caseKind.review")}</option>
                  <option value="reference_archive">{t("labels.caseKind.reference_archive")}</option>
                </select>
              </div>
            </div>
            <div className="row">
              <div className="field">
                <label htmlFor="case-ref">{t("cases.fieldReference")}</label>
                <input
                  id="case-ref"
                  className="input"
                  value={form.referenceCode ?? ""}
                  onChange={(e) => setForm({ ...form, referenceCode: e.target.value })}
                  placeholder={t("cases.referencePlaceholder")}
                />
              </div>
              <div className="field">
                <label htmlFor="case-area">{t("cases.fieldArea")}</label>
                <input
                  id="case-area"
                  className="input"
                  value={form.practiceArea ?? ""}
                  onChange={(e) => setForm({ ...form, practiceArea: e.target.value })}
                  placeholder={t("cases.areaPlaceholder")}
                />
              </div>
            </div>
            <div className="row" style={{ justifyContent: "flex-end" }}>
              <button className="btn primary" disabled={busy} onClick={() => void create()}>
                {busy ? t("cases.creating") : t("cases.create")}
              </button>
            </div>
          </div>
        ) : null}

        {rows.length === 0 ? (
          <div className="empty">
            <p style={{ margin: 0 }}>
              {showArchived ? t("cases.emptyArchived") : t("cases.emptyActive")}
            </p>
          </div>
        ) : (
          <div style={{ border: "1px solid var(--line)", borderRadius: "var(--radius)" }}>
            {rows.map((row, index) => (
              <button
                key={row.id}
                onClick={() => setSelected(row)}
                style={{
                  display: "grid",
                  gridTemplateColumns: "1fr 120px 130px 150px",
                  gap: "var(--space-4)",
                  alignItems: "center",
                  width: "100%",
                  textAlign: "left",
                  padding: "10px var(--space-4)",
                  border: 0,
                  borderTop: index === 0 ? 0 : "1px solid var(--line)",
                  background: selected?.id === row.id ? "var(--surface)" : "transparent",
                  cursor: "pointer",
                }}
              >
                <span>
                  <span style={{ fontWeight: 500 }}>{row.title}</span>
                  <span className="muted small" style={{ display: "block" }}>
                    {row.referenceCode ?? t("cases.noReference")} · {row.practiceArea ?? t("cases.noArea")}
                  </span>
                </span>
                <span className="badge plain">{CASE_KIND_LABEL[row.caseKind] ?? row.caseKind}</span>
                <span className="badge plain">{SENSITIVITY_LABEL[row.sensitivity] ?? row.sensitivity}</span>
                <span className="muted small">{formatTimestamp(row.updatedAt)}</span>
              </button>
            ))}
          </div>
        )}
      </div>

      {selected ? (
        <div className="card">
          <div className="card-head">
            <div>
              <h2>{selected.title}</h2>
              <p className="card-sub">
                {CASE_KIND_LABEL[selected.caseKind]} · {selected.status} · {t("cases.versionLabel", { version: selected.rowVersion })}
              </p>
            </div>
            <button className="btn ghost" onClick={() => setSelected(null)}>{t("cases.close")}</button>
          </div>
          <dl className="kv">
            <dt>{t("cases.fieldReference")}</dt><dd>{selected.referenceCode ?? "—"}</dd>
            <dt>{t("cases.fieldJurisdiction")}</dt><dd>{selected.jurisdictionLabel ?? "—"}</dd>
            <dt>{t("cases.fieldSensitivity")}</dt><dd>{SENSITIVITY_LABEL[selected.sensitivity]}</dd>
            <dt>{t("cases.fieldAiPolicy")}</dt><dd>{AI_POLICY_LABEL[selected.defaultAiPolicy]}</dd>
            <dt>{t("cases.fieldCreated")}</dt><dd>{formatTimestamp(selected.createdAt)}</dd>
            <dt>{t("cases.fieldUpdated")}</dt><dd>{formatTimestamp(selected.updatedAt)}</dd>
            <dt>{t("cases.fieldId")}</dt><dd style={{ fontFamily: "var(--mono)", fontSize: 12 }}>{selected.id}</dd>
          </dl>
          <div className="row" style={{ justifyContent: "flex-end" }}>
            {selected.status === "active" ? (
              <button className="btn" disabled={busy} onClick={() => void mutate("archive")}>{t("cases.archive")}</button>
            ) : (
              <button className="btn" disabled={busy} onClick={() => void mutate("restore")}>{t("cases.restore")}</button>
            )}
            <button className="btn" disabled={busy} onClick={() => void mutate("delete")}>{t("cases.delete")}</button>
          </div>
          {notice ? <div className="notice ok">{notice}</div> : null}
          {/* F04-T07: canlı aşama/sayım — yalnız ayrıştırma sürerken görünür. */}
          {busy && parseProgress ? (
            <div className="field" style={{ marginTop: 8 }} aria-live="polite">
              <p className="hint" style={{ margin: "0 0 4px" }}>
                {parseProgressLine(parseProgress)}
              </p>
              {parseProgressWidth(parseProgress) !== null ? (
                <div
                  role="progressbar"
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={parseProgressWidth(parseProgress) ?? 0}
                  aria-label={t("cases.parseProgressAria")}
                  style={{
                    height: 4,
                    borderRadius: 2,
                    background: "var(--line)",
                    overflow: "hidden",
                  }}
                >
                  <div
                    style={{
                      width: `${parseProgressWidth(parseProgress)}%`,
                      height: "100%",
                      background: "var(--accent)",
                    }}
                  />
                </div>
              ) : null}
            </div>
          ) : null}

          {reader ? (
            <DualReader
              caseId={selected.id}
              documents={docs}
              left={reader.left}
              right={reader.right}
              onClose={() => setReader(null)}
              onAnchor={(anchor) => {
                setAnchors((prev) => [anchor, ...prev]);
                setNotice(t("cases.anchorSaved", { quote: anchor.exactQuote.slice(0, 60) }));
              }}
              onAnchorRebound={() => {
                // Anchor güncel sürüme taşındı: liste çekirdekten tazelenir.
                void api
                  .anchorList(selected.id)
                  .then(setAnchors)
                  .catch(() => undefined);
                setNotice(t("cases.anchorRebound"));
              }}
              onNote={(note) => {
                setNotice(
                  t("cases.noteCreated", { title: note.title || note.body.slice(0, 40) }),
                );
              }}
            />
          ) : null}

          <div className="card tight" style={{ background: "var(--surface)" }}>
            <div className="card-head">
              <div>
                <h2>{t("cases.anchorsTitle")}</h2>
                <p className="card-sub">{t("cases.anchorsSubtitle", { count: anchors.length })}</p>
              </div>
            </div>
            {anchors.length === 0 ? (
              <p className="hint" style={{ margin: 0 }}>
                {t("cases.anchorsEmpty")}
              </p>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                {anchors.map((anchor) => (
                  <div
                    key={anchor.id}
                    className="row"
                    style={{ justifyContent: "space-between", alignItems: "center", gap: 8 }}
                  >
                    <span style={{ minWidth: 0 }}>
                      <span style={{ display: "block", fontSize: 13 }}>“{anchor.exactQuote.slice(0, 140)}”</span>
                      <span className="muted small">
                        {formatTimestamp(anchor.createdAt)} · karakter {anchor.startOffset}–{anchor.endOffset}
                      </span>
                    </span>
                    <button
                      className="btn ghost"
                      style={{ height: 26 }}
                      onClick={() => void openAnchor(anchor.id)}
                      title={t("cases.openAnchorTitle")}
                    >
                      {t("cases.openInReader")}
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="card tight" style={{ background: "var(--surface)" }}>
            <div className="card-head">
              <div>
                <h2>{t("cases.documentsTitle")}</h2>
                <p className="card-sub">{t("cases.documentsSubtitle", { count: docs.length })}</p>
              </div>
              <div style={{ display: "flex", gap: "var(--space-2)" }}>
                <button
                  className="btn"
                  disabled={busy || batchBusy || docs.length === 0}
                  title={t("cases.parsePendingTitle")}
                  onClick={() => void parsePending()}
                >
                  {batchBusy ? t("cases.parsing") : t("cases.parsePending")}
                </button>
                <button className="btn" disabled={busy} onClick={() => void addDocument()}>
                  {t("cases.addDocument")}
                </button>
              </div>
            </div>
            {docs.length === 0 ? (
              <div className="empty" style={{ padding: "var(--space-4)" }}>
                <p style={{ margin: 0 }}>{t("cases.documentsEmpty")}</p>
              </div>
            ) : (
              <div style={{ border: "1px solid var(--line)", borderRadius: "var(--radius)", background: "var(--bg)" }}>
                {docs.map((doc, index) => (
                  <div
                    key={doc.id}
                    style={{
                      display: "grid",
                      gridTemplateColumns: "1fr 80px 100px 120px 200px 190px",
                      gap: "var(--space-3)",
                      alignItems: "center",
                      padding: "9px var(--space-3)",
                      borderTop: index === 0 ? 0 : "1px solid var(--line)",
                    }}
                  >
                    <span>
                      <span style={{ fontWeight: 500 }}>{doc.displayName}</span>
                      <span className="muted small" style={{ display: "block", fontFamily: "var(--mono)", fontSize: 11 }}>
                        {doc.currentVersionId?.slice(0, 8) ?? t("cases.noVersion")}
                      </span>
                    </span>
                    <span className="badge plain">{doc.documentType}</span>
                    <span className="muted small">{doc.status}</span>
                    <span className="muted small">{formatTimestamp(doc.updatedAt)}</span>
                    <span style={{ display: "flex", flexDirection: "column", gap: 3 }}>
                      <span style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
                        {(tagsByDoc[doc.id] ?? []).map((tag) => (
                          <span
                            key={tag}
                            className="badge plain"
                            style={{ display: "inline-flex", alignItems: "center", gap: 2 }}
                          >
                            #{tag}
                            <button
                              className="btn ghost"
                              style={{ height: 16, padding: "0 3px", fontSize: 11, lineHeight: 1 }}
                              title={t("cases.removeTagTitle", { tag })}
                              onClick={() => void removeTag(doc.id, tag)}
                            >
                              ×
                            </button>
                          </span>
                        ))}
                      </span>
                      <input
                        className="input"
                        style={{ height: 24, fontSize: 12 }}
                        placeholder={t("cases.tagPlaceholder")}
                        aria-label={t("cases.tagAria", { name: doc.displayName })}
                        value={tagDrafts[doc.id] ?? ""}
                        onChange={(e) => setTagDrafts((prev) => ({ ...prev, [doc.id]: e.target.value }))}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            e.preventDefault();
                            void addTag(doc.id);
                          }
                        }}
                      />
                    </span>
                    <span style={{ display: "flex", gap: 4 }}>
                      <button
                        className="btn ghost"
                        disabled={!doc.currentVersionId || busy}
                        onClick={() => openDocument(doc)}
                        title={t("cases.openDocumentTitle")}
                      >
                        {t("cases.read")}
                      </button>
                      <button
                        className="btn ghost"
                        disabled={!doc.currentVersionId || busy}
                        onClick={() => void parseDocument(doc.currentVersionId!, false)}
                      >
                        {t("cases.parse")}
                      </button>
                      <button
                        className="btn ghost"
                        disabled={!doc.currentVersionId || busy}
                        onClick={() => void parseDocument(doc.currentVersionId!, true)}
                        title={t("cases.ocrTitle", { lang: defaultOcrLanguageLabel() })}
                      >
                        {t("cases.ocr")}
                      </button>
                    </span>
                  </div>
                ))}
              </div>
            )}
            <div className="field" style={{ maxWidth: 320, marginTop: 8 }}>
            {batchResult && (
              <div
                className="card tight"
                style={{ marginTop: "var(--space-3)", background: "var(--surface)" }}
              >
                <div className="card-head" style={{ paddingBottom: 4 }}>
                  <div>
                    <strong>
                      {t("cases.batchResult", {
                        succeeded: batchResult.succeeded,
                        requested: batchResult.requested,
                      })}
                    </strong>
                    <p className="card-sub" style={{ marginBottom: 0 }}>
                      {t("cases.batchDetail", {
                        pages: batchResult.pages,
                        peak: batchResult.peakParallel,
                      })}
                      {batchResult.failed > 0
                        ? ` · ${t("cases.batchFailed", { count: batchResult.failed })}`
                        : ""}
                    </p>
                  </div>
                </div>
                {batchResult.requested === 0 ? (
                  <p className="muted small" style={{ margin: 0 }}>
                    {t("cases.batchNonePending")}
                  </p>
                ) : (
                  <ul className="small" style={{ margin: 0, paddingLeft: 18 }}>
                    {batchResult.results.map((item) => (
                      <li key={item.versionId}>
                        <span className={`badge ${item.ok ? "" : "plain"}`}>
                          {item.ok ? t("cases.batchPages", { count: item.pages }) : (item.code ?? "E_PARSE")}
                        </span>{" "}
                        {item.displayName}
                        {!item.ok && item.message ? (
                          <span className="muted"> — {item.message}</span>
                        ) : null}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )}
              <label htmlFor="txt-encoding">{t("cases.encodingLabel")}</label>
              <select
                id="txt-encoding"
                className="input"
                value={txtEncoding}
                onChange={(e) => setTxtEncoding(e.target.value)}
              >
                <option value="auto">{t("cases.encodingAuto")}</option>
                <option value="utf-8">{t("cases.encodingUtf8")}</option>
                <option value="windows-1254">{t("cases.encodingWindows")}</option>
                <option value="iso-8859-9">{t("cases.encodingIso")}</option>
              </select>
            </div>
            <p className="hint" style={{ marginTop: 6 }}>
              {t("cases.encodingHint")}
            </p>
            <div className="field" style={{ maxWidth: 320, marginTop: 8 }}>
              <label htmlFor="pdf-password">{t("cases.pdfPasswordLabel")}</label>
              <input
                id="pdf-password"
                className="input"
                type="password"
                autoComplete="off"
                value={pdfPassword}
                onChange={(e) => setPdfPassword(e.target.value)}
                placeholder={t("cases.pdfPasswordPlaceholder")}
              />
            </div>
            <p className="hint" style={{ margin: 0 }}>
              {t("cases.footnote")}
            </p>
          </div>

          <Notes caseId={selected.id} anchors={anchors} />
        </div>
      ) : null}
    </div>
  );
}
