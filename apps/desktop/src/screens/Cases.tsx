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
      setError({ code: "E_TITLE", message: "Dosya başlığı zorunlu.", retryable: false });
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
        { name: "Belgeler", extensions: ["pdf", "docx", "udf", "txt", "md", "csv", "png", "jpg", "jpeg", "tif", "tiff"] },
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
        setNotice(
          `Bu dosyada aynı içerik zaten var: “${outcome.displayName}”. Yeni kayıt oluşturulmadı.`,
        );
      } else {
        setNotice(`“${outcome.document.displayName}” içe alındı (sürüm ${outcome.version.versionNo}).`);
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
      const budgetLine = result.budget ? ` · ${budgetSummary(result.budget)}` : "";
      // Başarılı işte ihlal beklenmez (ihlal işi durdurur); yine de görülürse
      // hata olarak değil, açık bir uyarı satırı olarak yazılır.
      const budgetAlert = result.budget ? budgetWarning(result.budget) : null;
      setNotice(
        `Ayrıştırma ${result.parseRun.status}: ${result.pages} sayfa, ${result.storedSegments} metin bloğu` +
          (result.warnings.length ? ` · ${result.warnings.length} uyarı` : "") +
          (encodingNote ? ` · ${encodingNote}` : "") +
          budgetLine +
          (budgetAlert ? ` · ${budgetAlert}` : ""),
      );
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
          message: "Kaynak kaydı bulunamadı; silinmiş olabilir.",
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
        <h1>Dosyalar</h1>
        <p className="lead">
          Her dosya kendi belge, not ve kronolojisini taşır. Bir dosyanın verisi başka dosyanın
          sorgusuna örtük olarak eklenmez (INV-05).
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
            <h2>İnceleme dosyaları</h2>
            <p className="card-sub">{rows.length} kayıt · {showArchived ? "arşiv dahil" : "yalnız aktif"}</p>
          </div>
          <div className="row" style={{ flexWrap: "nowrap" }}>
            <label className="row" style={{ alignItems: "center", gap: 6 }}>
              <input
                type="checkbox"
                checked={showArchived}
                onChange={(e) => setShowArchived(e.target.checked)}
                style={{ width: 15, height: 15 }}
              />
              <span className="small muted">Arşiv</span>
            </label>
            <button className="btn primary" onClick={() => setCreating((v) => !v)}>
              {creating ? "Vazgeç" : "Yeni dosya"}
            </button>
          </div>
        </div>

        {creating ? (
          <div className="card tight" style={{ background: "var(--surface)" }}>
            <div className="row">
              <div className="field" style={{ flex: "2 1 320px" }}>
                <label htmlFor="case-title">Başlık</label>
                <input
                  id="case-title"
                  className="input"
                  value={form.title}
                  onChange={(e) => setForm({ ...form, title: e.target.value })}
                  placeholder="Örn. Kira alacağı — 2026"
                  autoFocus
                />
              </div>
              <div className="field">
                <label htmlFor="case-kind">Tür</label>
                <select
                  id="case-kind"
                  className="input"
                  value={form.caseKind}
                  onChange={(e) => setForm({ ...form, caseKind: e.target.value as NewCaseInput["caseKind"] })}
                >
                  <option value="review">İnceleme dosyası</option>
                  <option value="reference_archive">Kaynak arşivi</option>
                </select>
              </div>
            </div>
            <div className="row">
              <div className="field">
                <label htmlFor="case-ref">Esas / referans</label>
                <input
                  id="case-ref"
                  className="input"
                  value={form.referenceCode ?? ""}
                  onChange={(e) => setForm({ ...form, referenceCode: e.target.value })}
                  placeholder="2026/0000"
                />
              </div>
              <div className="field">
                <label htmlFor="case-area">Alan</label>
                <input
                  id="case-area"
                  className="input"
                  value={form.practiceArea ?? ""}
                  onChange={(e) => setForm({ ...form, practiceArea: e.target.value })}
                  placeholder="Kira, iş, aile…"
                />
              </div>
            </div>
            <div className="row" style={{ justifyContent: "flex-end" }}>
              <button className="btn primary" disabled={busy} onClick={() => void create()}>
                {busy ? "Oluşturuluyor…" : "Dosyayı oluştur"}
              </button>
            </div>
          </div>
        ) : null}

        {rows.length === 0 ? (
          <div className="empty">
            <p style={{ margin: 0 }}>
              {showArchived ? "Bu kasada kayıt yok." : "Henüz aktif dosya yok. “Yeni dosya” ile başlayın."}
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
                    {row.referenceCode ?? "referans yok"} · {row.practiceArea ?? "alan belirtilmedi"}
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
                {CASE_KIND_LABEL[selected.caseKind]} · {selected.status} · sürüm {selected.rowVersion}
              </p>
            </div>
            <button className="btn ghost" onClick={() => setSelected(null)}>Kapat</button>
          </div>
          <dl className="kv">
            <dt>Esas / referans</dt><dd>{selected.referenceCode ?? "—"}</dd>
            <dt>Yargı yeri</dt><dd>{selected.jurisdictionLabel ?? "—"}</dd>
            <dt>Hassasiyet</dt><dd>{SENSITIVITY_LABEL[selected.sensitivity]}</dd>
            <dt>AI politikası</dt><dd>{AI_POLICY_LABEL[selected.defaultAiPolicy]}</dd>
            <dt>Oluşturma</dt><dd>{formatTimestamp(selected.createdAt)}</dd>
            <dt>Son değişiklik</dt><dd>{formatTimestamp(selected.updatedAt)}</dd>
            <dt>Kimlik</dt><dd style={{ fontFamily: "var(--mono)", fontSize: 12 }}>{selected.id}</dd>
          </dl>
          <div className="row" style={{ justifyContent: "flex-end" }}>
            {selected.status === "active" ? (
              <button className="btn" disabled={busy} onClick={() => void mutate("archive")}>Arşivle</button>
            ) : (
              <button className="btn" disabled={busy} onClick={() => void mutate("restore")}>Geri al</button>
            )}
            <button className="btn" disabled={busy} onClick={() => void mutate("delete")}>Sil (geri dönüşümlü)</button>
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
                  aria-label="Ayrıştırma ilerlemesi"
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
                setNotice(`Kaynak kaydedildi: “${anchor.exactQuote.slice(0, 60)}”`);
              }}
              onAnchorRebound={() => {
                // Anchor güncel sürüme taşındı: liste çekirdekten tazelenir.
                void api
                  .anchorList(selected.id)
                  .then(setAnchors)
                  .catch(() => undefined);
                setNotice("Kaynak güncel sürüme bağlandı.");
              }}
              onNote={(note) => {
                setNotice(`Kaynaklı not oluşturuldu: “${note.title || note.body.slice(0, 40)}”`);
              }}
            />
          ) : null}

          <div className="card tight" style={{ background: "var(--surface)" }}>
            <div className="card-head">
              <div>
                <h2>Kaynak alıntılar</h2>
                <p className="card-sub">{anchors.length} bağ · alıntı segmentte doğrulanarak kaydedilir</p>
              </div>
            </div>
            {anchors.length === 0 ? (
              <p className="hint" style={{ margin: 0 }}>
                Henüz kaynak yok. Okuyucuda metin seçip “Seçimi kaynağa bağla” deyin.
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
                      title="Alıntının sayfasını okuyucuda aç"
                    >
                      Okuyucuda aç
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="card tight" style={{ background: "var(--surface)" }}>
            <div className="card-head">
              <div>
                <h2>Belgeler</h2>
                <p className="card-sub">{docs.length} belge · özgün baytlar şifreli saklanır</p>
              </div>
              <div style={{ display: "flex", gap: "var(--space-2)" }}>
                <button
                  className="btn"
                  disabled={busy || batchBusy || docs.length === 0}
                  title="Güncel sürümü ayrıştırılmamış belgeleri en fazla iki worker'la ayrıştırır"
                  onClick={() => void parsePending()}
                >
                  {batchBusy ? "Ayrıştırılıyor…" : "Bekleyenleri ayrıştır"}
                </button>
                <button className="btn" disabled={busy} onClick={() => void addDocument()}>
                  Belge ekle
                </button>
              </div>
            </div>
            {docs.length === 0 ? (
              <div className="empty" style={{ padding: "var(--space-4)" }}>
                <p style={{ margin: 0 }}>Henüz belge yok. PDF, DOCX, TXT veya görüntü ekleyin.</p>
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
                        {doc.currentVersionId?.slice(0, 8) ?? "sürüm yok"}
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
                              title={`“${tag}” etiketini kaldır`}
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
                        placeholder="+ etiket (Enter)"
                        aria-label={`${doc.displayName} için etiket`}
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
                        title="Belgeyi okuyucuda aç"
                      >
                        Oku
                      </button>
                      <button
                        className="btn ghost"
                        disabled={!doc.currentVersionId || busy}
                        onClick={() => void parseDocument(doc.currentVersionId!, false)}
                      >
                        Ayrıştır
                      </button>
                      <button
                        className="btn ghost"
                        disabled={!doc.currentVersionId || busy}
                        onClick={() => void parseDocument(doc.currentVersionId!, true)}
                        title={`Taranmış sayfalar için OCR (${defaultOcrLanguageLabel()})`}
                      >
                        OCR
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
                      {batchResult.succeeded} / {batchResult.requested} belge ayrıştırıldı
                    </strong>
                    <p className="card-sub" style={{ marginBottom: 0 }}>
                      {batchResult.pages} sayfa · eşzamanlılık tepe {batchResult.peakParallel} (en fazla 2)
                      {batchResult.failed > 0 ? ` · ${batchResult.failed} başarısız` : ""}
                    </p>
                  </div>
                </div>
                {batchResult.requested === 0 ? (
                  <p className="muted small" style={{ margin: 0 }}>
                    Bekleyen belge yok: her belgenin güncel sürümü için başarılı bir ayrıştırma var.
                  </p>
                ) : (
                  <ul className="small" style={{ margin: 0, paddingLeft: 18 }}>
                    {batchResult.results.map((item) => (
                      <li key={item.versionId}>
                        <span className={`badge ${item.ok ? "" : "plain"}`}>
                          {item.ok ? `${item.pages} sayfa` : (item.code ?? "E_PARSE")}
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
              <label htmlFor="txt-encoding">TXT kodlaması (düz metin belgeler)</label>
              <select
                id="txt-encoding"
                className="input"
                value={txtEncoding}
                onChange={(e) => setTxtEncoding(e.target.value)}
              >
                <option value="auto">Otomatik (BOM → UTF-8 → Türkçe)</option>
                <option value="utf-8">UTF-8</option>
                <option value="windows-1254">Windows-1254 (Türkçe)</option>
                <option value="iso-8859-9">ISO-8859-9 (Latin-5)</option>
              </select>
            </div>
            <p className="hint" style={{ marginTop: 6 }}>
              Otomatik seçim BOM'u, katı UTF-8'i ve Türkçe tek baytlık kodlamaları dener; hangisinin
              kullanıldığı ayrıştırma uyarısında yazılır. Metin bozuk görünürse kodlamayı açıkça
              seçip yeniden ayrıştırın.
            </p>
            <div className="field" style={{ maxWidth: 320, marginTop: 8 }}>
              <label htmlFor="pdf-password">Parolalı PDF parolası (yalnız bu oturum için)</label>
              <input
                id="pdf-password"
                className="input"
                type="password"
                autoComplete="off"
                value={pdfPassword}
                onChange={(e) => setPdfPassword(e.target.value)}
                placeholder="Parola gerekmiyorsa boş bırakın"
              />
            </div>
            <p className="hint" style={{ margin: 0 }}>
              Aynı içerik ikinci kez eklenirse yeni belge oluşturulmaz; mevcut kayıt gösterilir.
              Parola kaydedilmez, yalnız ayrıştırma isteğinde worker'a iletilir. OCR taranmış
              sayfalar için çalışır ve dil paketi gerektirir.
            </p>
          </div>

          <Notes caseId={selected.id} anchors={anchors} />
        </div>
      ) : null}
    </div>
  );
}
