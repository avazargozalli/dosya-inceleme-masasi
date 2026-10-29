import { useCallback, useEffect, useState } from "react";
import {
  api,
  toApiError,
  type ApiError,
  type CaseChecklistItemRow,
  type CaseRow,
  type ChecklistItemState,
  type ChecklistTemplateRow,
  type DocumentRow,
} from "../lib/api";
import {
  CHECKLIST_STATE_HELP,
  CHECKLIST_STATE_LABEL,
  MISSING_SCOPE_NOTE,
  checklistLicenseLabel,
  copiedLabel,
  isChecklistState,
  shortContentHash,
  summarizeChecklist,
  validateChecklistEntry,
} from "../lib/checklist";
import { t } from "../lib/i18n";

type Draft = {
  state: ChecklistItemState;
  links: string[];
  note: string;
};

const STATE_ORDER: ChecklistItemState[] = ["open", "present", "missing", "not_applicable"];

function draftOf(item: CaseChecklistItemRow): Draft {
  return {
    state: isChecklistState(item.state) ? item.state : "open",
    links: [...item.linkedDocumentIds],
    note: item.note,
  };
}

function sameLinks(a: string[], b: string[]): boolean {
  if (a.length !== b.length) return false;
  const left = [...a].sort();
  const right = [...b].sort();
  return left.every((value, index) => value === right[index]);
}

function ruleFor(draft: Draft): string | null {
  return validateChecklistEntry({
    state: draft.state,
    linkedDocumentIds: draft.links,
    note: draft.note,
  });
}

export function Checklists() {
  const [cases, setCases] = useState<CaseRow[]>([]);
  const [caseId, setCaseId] = useState("");
  const [documents, setDocuments] = useState<DocumentRow[]>([]);
  const [templates, setTemplates] = useState<ChecklistTemplateRow[]>([]);
  const [items, setItems] = useState<CaseChecklistItemRow[]>([]);
  const [drafts, setDrafts] = useState<Record<string, Draft>>({});
  const [error, setError] = useState<ApiError | null>(null);
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const [conflictItem, setConflictItem] = useState<string | null>(null);

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

  const load = useCallback(async (id: string) => {
    if (!id) return;
    try {
      const [docRows, templateRows, itemRows] = await Promise.all([
        api.documentList(id),
        api.checklistTemplates(id),
        api.checklistItems(id),
      ]);
      setDocuments(docRows);
      setTemplates(templateRows);
      setItems(itemRows);
      setDrafts(Object.fromEntries(itemRows.map((item) => [item.id, draftOf(item)])));
      setConflictItem(null);
    } catch (e) {
      setError(toApiError(e));
    }
  }, []);

  useEffect(() => {
    if (caseId) void load(caseId);
  }, [caseId, load]);

  function updateDraft(item: CaseChecklistItemRow, patch: Partial<Draft>) {
    setDrafts((prev) => {
      const current = prev[item.id] ?? draftOf(item);
      return { ...prev, [item.id]: { ...current, ...patch } };
    });
  }

  function changeState(item: CaseChecklistItemRow, state: ChecklistItemState) {
    const current = drafts[item.id] ?? draftOf(item);
    // Belge bağı yalnız "Bulundu" durumunda anlamlı; durum değişince temizlenir.
    updateDraft(item, { state, links: state === "present" ? current.links : [] });
  }

  function toggleDocument(item: CaseChecklistItemRow, documentId: string, checked: boolean) {
    const current = drafts[item.id] ?? draftOf(item);
    const links = checked
      ? [...current.links, documentId]
      : current.links.filter((id) => id !== documentId);
    updateDraft(item, { links });
  }

  async function copyTemplate(template: ChecklistTemplateRow) {
    setBusy(true);
    setError(null);
    try {
      const result = await api.checklistCopy(caseId, template.id);
      setNotice(
        result.created
          ? t("checklists.noticeCopied", {
              title: result.templateTitle,
              version: result.templateVersion,
              count: result.itemCount,
              license: result.licenseId ?? t("checklists.licenseUnknown"),
            })
          : t("checklists.noticeAlready", { title: result.templateTitle }),
      );
      await load(caseId);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function save(item: CaseChecklistItemRow) {
    const draft = drafts[item.id] ?? draftOf(item);
    const rule = ruleFor(draft);
    if (rule) {
      setError({ code: "E_CHECKLIST_RULE", message: rule, retryable: false });
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await api.checklistItemState(
        item.id,
        draft.state,
        draft.links,
        draft.note,
        item.rowVersion,
      );
      await load(caseId);
      setNotice(t("checklists.noticeUpdated", { item: item.itemText }));
    } catch (e) {
      const apiError = toApiError(e);
      if (apiError.code === "E_ROW_VERSION") setConflictItem(item.id);
      setError(apiError);
    } finally {
      setBusy(false);
    }
  }

  const summary = summarizeChecklist(items);

  return (
    <div className="page" style={{ maxWidth: 1000 }}>
      <div className="page-head">
        <h1>{t("nav.checklists")}</h1>
        <p className="lead">{t("checklists.lead")}</p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
          {conflictItem ? (
            <button
              className="btn ghost"
              style={{ marginLeft: 8 }}
              onClick={() => void load(caseId)}
            >
              {t("checklists.reload")}
            </button>
          ) : null}
        </div>
      ) : null}
      {notice ? <div className="notice ok">{notice}</div> : null}

      <div className="card">
        <div className="row">
          <div className="field" style={{ flex: "1 1 320px" }}>
            <label htmlFor="checklist-case">{t("common.case")}</label>
            <select
              id="checklist-case"
              className="input"
              value={caseId}
              onChange={(e) => setCaseId(e.target.value)}
            >
              {cases.length === 0 ? <option value="">{t("common.noCase")}</option> : null}
              {cases.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.title}
                </option>
              ))}
            </select>
          </div>
          <div className="field" style={{ flex: "0 1 auto" }}>
            <label>{t("checklists.state")}</label>
            <span className="small muted">
              {t("checklists.summary", {
                total: summary.total,
                open: summary.open,
                present: summary.present,
                missing: summary.missing,
                notApplicable: summary.notApplicable,
              })}
            </span>
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("checklists.catalogTitle")}</h2>
            <p className="card-sub">{t("checklists.catalogSub")}</p>
          </div>
        </div>

        {templates.length === 0 ? (
          <div className="empty">
            <p style={{ margin: 0 }}>{t("checklists.catalogEmpty")}</p>
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {templates.map((template) => {
              const copied = copiedLabel(template);
              return (
                <div
                  key={template.id}
                  style={{
                    border: "1px solid var(--line)",
                    borderRadius: "var(--radius)",
                    padding: "10px var(--space-3)",
                  }}
                >
                  <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
                    <div>
                      <strong>{template.title}</strong>
                      <div className="small muted">
                        {template.practiceArea ?? t("checklists.practiceUnknown")} ·{" "}
                        {checklistLicenseLabel(template)}
                      </div>
                      <div className="small muted">
                        {t("checklists.templateItems", { count: template.itemCount })}{" "}
                        <span style={{ fontFamily: "var(--mono)" }}>
                          {shortContentHash(template.contentHash)}
                        </span>
                        {template.sourceUrl ? (
                          <>
                            {" "}
                            · {t("checklists.sourceLabel")}{" "}
                            <span style={{ fontFamily: "var(--mono)", fontSize: 11 }}>
                              {template.sourceUrl}
                            </span>
                          </>
                        ) : null}
                      </div>
                    </div>
                    <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                      {copied ? <span className="badge ok">{copied}</span> : null}
                      <button
                        className="btn primary"
                        disabled={busy || !caseId}
                        onClick={() => void copyTemplate(template)}
                      >
                        {copied ? t("checklists.verifyCopy") : t("checklists.copyToCase")}
                      </button>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
        <p className="hint" style={{ margin: 0 }}>
          {t("checklists.copyHint")}
        </p>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("checklists.caseListTitle")}</h2>
            <p className="card-sub">{t("checklists.itemsSub", { count: items.length })}</p>
          </div>
          {items.length > 0 ? (
            <div style={{ display: "flex", gap: 6 }}>
              <span className="badge ok">
                {t("checklists.foundBadge", { count: summary.present })}
              </span>
              <span className="badge warn">
                {t("checklists.missingBadge", { count: summary.missing })}
              </span>
              <span className="badge plain">
                {t("checklists.notApplicableBadge", { count: summary.notApplicable })}
              </span>
            </div>
          ) : null}
        </div>

        {items.length === 0 ? (
          <div className="empty">
            <p style={{ margin: 0 }}>{t("checklists.empty")}</p>
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {items.map((item) => {
              const draft = drafts[item.id] ?? draftOf(item);
              const rule = ruleFor(draft);
              const knownDocs = new Set(documents.map((document) => document.id));
              const staleLinks = draft.state === "present" ? draft.links.filter((id) => !knownDocs.has(id)) : [];
              const dirty =
                draft.state !== item.state ||
                draft.note.trim() !== item.note ||
                !sameLinks(draft.links, item.linkedDocumentIds);
              return (
                <div
                  key={item.id}
                  style={{
                    border: "1px solid var(--line)",
                    borderRadius: "var(--radius)",
                    padding: "10px var(--space-3)",
                    display: "flex",
                    flexDirection: "column",
                    gap: 8,
                  }}
                >
                  <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
                    <div style={{ flex: "1 1 420px" }}>
                      <div className="small muted">
                        {t("checklists.itemMeta", {
                          ordinal: item.ordinal + 1,
                          category: item.category || t("checklists.categoryGeneral"),
                        })}
                        {item.template ? (
                          <>
                            {" "}
                            ·{" "}
                            {t("checklists.templateMeta", {
                              title: item.template.title,
                              version: item.template.version,
                              license: item.template.licenseId ?? t("checklists.licenseUnknown"),
                            })}{" "}
                            {shortContentHash(item.template.contentHash)}
                          </>
                        ) : (
                          <>
                            {" · "}
                            {t("checklists.noTemplateMeta")}
                          </>
                        )}
                      </div>
                      <div style={{ marginTop: 2 }}>{item.itemText}</div>
                    </div>
                    <span
                      className={`badge ${
                        item.state === "present"
                          ? "ok"
                          : item.state === "missing"
                            ? "warn"
                            : "plain"
                      }`}
                    >
                      {CHECKLIST_STATE_LABEL[isChecklistState(item.state) ? item.state : "open"]}
                    </span>
                  </div>

                  <div className="row" style={{ alignItems: "flex-start" }}>
                    <div className="field" style={{ flex: "0 1 240px" }}>
                      <label htmlFor={`state-${item.id}`}>{t("checklists.state")}</label>
                      <select
                        id={`state-${item.id}`}
                        className="input"
                        value={draft.state}
                        onChange={(e) => {
                          if (isChecklistState(e.target.value)) changeState(item, e.target.value);
                        }}
                      >
                        {STATE_ORDER.map((state) => (
                          <option key={state} value={state}>
                            {CHECKLIST_STATE_LABEL[state]}
                          </option>
                        ))}
                      </select>
                      <span className="hint">{CHECKLIST_STATE_HELP[draft.state]}</span>
                    </div>

                    {draft.state === "present" ? (
                      <div className="field" style={{ flex: "1 1 320px" }}>
                        <label>
                          {t("checklists.linkedDocuments", { count: draft.links.length })}
                        </label>
                        {documents.length === 0 ? (
                          <span className="hint">{t("checklists.noDocuments")}</span>
                        ) : (
                          <div
                            style={{
                              maxHeight: 140,
                              overflow: "auto",
                              border: "1px solid var(--line)",
                              borderRadius: "var(--radius-sm)",
                              padding: 6,
                              display: "flex",
                              flexDirection: "column",
                              gap: 4,
                            }}
                          >
                            {documents.map((document) => (
                              <label key={document.id} className="small" style={{ display: "flex", gap: 6 }}>
                                <input
                                  type="checkbox"
                                  checked={draft.links.includes(document.id)}
                                  onChange={(e) =>
                                    toggleDocument(item, document.id, e.target.checked)
                                  }
                                />
                                <span>{document.displayName}</span>
                              </label>
                            ))}
                          </div>
                        )}
                      </div>
                    ) : null}

                    <div className="field" style={{ flex: "1 1 260px" }}>
                      <label htmlFor={`note-${item.id}`}>
                        {draft.state === "missing"
                          ? t("checklists.noteMissing")
                          : draft.state === "not_applicable"
                            ? t("checklists.noteNotApplicable")
                            : t("checklists.noteOptional")}
                      </label>
                      <input
                        id={`note-${item.id}`}
                        className="input"
                        value={draft.note}
                        onChange={(e) => updateDraft(item, { note: e.target.value })}
                        placeholder={
                          draft.state === "missing"
                            ? t("checklists.notePlaceholderMissing")
                            : t("checklists.notePlaceholder")
                        }
                      />
                    </div>

                    <button
                      className="btn"
                      disabled={busy || rule !== null || !dirty}
                      onClick={() => void save(item)}
                      title={rule ?? undefined}
                    >
                      {t("common.save")}
                    </button>
                  </div>

                  {rule ? <p className="hint error" style={{ margin: 0 }}>{rule}</p> : null}
                  {staleLinks.length > 0 ? (
                    <div className="hint">
                      {t("checklists.staleLinks")}{" "}
                      {staleLinks.map((id) => {
                        const name =
                          item.linkedDocuments.find((document) => document.id === id)?.displayName ??
                          id;
                        return (
                          <span key={id} style={{ marginRight: 8 }}>
                            {name}{" "}
                            <button
                              className="btn ghost"
                              style={{ padding: "0 4px" }}
                              onClick={() => toggleDocument(item, id, false)}
                            >
                              {t("checklists.removeLink")}
                            </button>
                          </span>
                        );
                      })}
                    </div>
                  ) : null}
                  {item.note && draft.note.trim() !== item.note ? (
                    <p className="hint" style={{ margin: 0 }}>
                      {t("checklists.savedNote", { note: item.note })}
                    </p>
                  ) : null}
                </div>
              );
            })}
          </div>
        )}

        <p className="hint" style={{ margin: 0 }}>
          {MISSING_SCOPE_NOTE}
        </p>
      </div>
    </div>
  );
}
