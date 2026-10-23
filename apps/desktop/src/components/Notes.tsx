import { useCallback, useEffect, useRef, useState } from "react";
import {
  api,
  toApiError,
  type AnchorRow,
  type ApiError,
  type NoteDraftRow,
  type NoteRevisionRow,
  type NoteRow,
} from "../lib/api";
import { formatTimestamp } from "../lib/format";
import {
  CONFLICT_HINT,
  diffNoteText,
  diffSummary,
  draftIsStale,
  draftStatusLabel,
  noteEditDirty,
  revisionOriginLabel,
  type DraftStatus,
} from "../lib/notes";

type ConflictState = {
  noteId: string;
  mineTitle: string;
  mineBody: string;
  mineAnchors: string[];
  theirs: NoteRow;
};

const EMPTY_BODY_ERROR: ApiError = {
  code: "E_NOTE",
  message: "Not metni boş olamaz.",
  retryable: false,
};

function AnchorPicker({
  anchors,
  picked,
  onToggle,
  idPrefix,
}: {
  anchors: AnchorRow[];
  picked: string[];
  onToggle: (id: string, checked: boolean) => void;
  idPrefix: string;
}) {
  if (anchors.length === 0) {
    return (
      <p className="hint" style={{ margin: 0 }}>
        Kaynak alıntı yok. Okuyucuda metin seçip “Seçimi kaynak olarak kaydet” deyin.
      </p>
    );
  }
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      {anchors.map((anchor, index) => (
        <label
          key={anchor.id}
          className="row"
          style={{
            alignItems: "flex-start",
            gap: 8,
            padding: "6px 8px",
            border: "1px solid var(--line)",
            borderRadius: "var(--radius)",
            background: "var(--bg)",
          }}
        >
          <input
            id={`${idPrefix}-anchor-${index}`}
            type="checkbox"
            checked={picked.includes(anchor.id)}
            onChange={(e) => onToggle(anchor.id, e.target.checked)}
            style={{ width: 15, height: 15, marginTop: 2 }}
          />
          <span>
            <span style={{ display: "block", fontSize: 13 }}>“{anchor.exactQuote.slice(0, 140)}”</span>
            <span className="muted small">
              {anchor.prefix.slice(-24)} … {anchor.suffix.slice(0, 24)}
            </span>
          </span>
        </label>
      ))}
    </div>
  );
}

export function Notes({ caseId, anchors }: { caseId: string; anchors: AnchorRow[] }) {
  const [rows, setRows] = useState<NoteRow[]>([]);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

  // Yeni not taslağı
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [picked, setPicked] = useState<string[]>([]);
  const [composerStatus, setComposerStatus] = useState<DraftStatus>("idle");
  const [restoredDraftAt, setRestoredDraftAt] = useState<string | null>(null);
  const composerHydrated = useRef(false);

  // Var olan notun düzenlenmesi
  const [editNote, setEditNote] = useState<NoteRow | null>(null);
  const [editTitle, setEditTitle] = useState("");
  const [editBody, setEditBody] = useState("");
  const [editAnchors, setEditAnchors] = useState<string[]>([]);
  const [editStatus, setEditStatus] = useState<DraftStatus>("idle");
  const [editPendingDraft, setEditPendingDraft] = useState<NoteDraftRow | null>(null);
  /** Taslak hangi sürüme dayanıyorsa o; `null` = notun şu anki sürümü. */
  const [editBaseVersion, setEditBaseVersion] = useState<number | null>(null);
  const [historyNote, setHistoryNote] = useState<string | null>(null);
  const [conflict, setConflict] = useState<ConflictState | null>(null);
  const [revisions, setRevisions] = useState<{ noteId: string; rows: NoteRevisionRow[] } | null>(null);
  const editHydrated = useRef(false);

  const load = useCallback(async () => {
    try {
      setRows(await api.noteList(caseId));
    } catch (e) {
      setError(toApiError(e));
    }
  }, [caseId]);

  useEffect(() => {
    void load();
  }, [load]);

  // Kasadaki taslağı geri yükle: localStorage değil, açık kasadaki gerçek kayıt.
  useEffect(() => {
    composerHydrated.current = false;
    let cancelled = false;
    void (async () => {
      try {
        const draft = await api.noteDraftGet(caseId, null);
        if (cancelled) return;
        if (draft && (draft.body.trim().length > 0 || draft.title.trim().length > 0)) {
          setTitle(draft.title);
          setBody(draft.body);
          setPicked(draft.sourceAnchorIds);
          setComposerStatus("saved");
          setRestoredDraftAt(draft.updatedAt);
        } else {
          setTitle("");
          setBody("");
          setPicked([]);
          setComposerStatus("idle");
          setRestoredDraftAt(null);
        }
      } catch (e) {
        if (!cancelled) setError(toApiError(e));
      } finally {
        if (!cancelled) composerHydrated.current = true;
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [caseId]);

  // Yeni not taslağını kasaya otomatik kaydet (900 ms sonra).
  useEffect(() => {
    if (!composerHydrated.current) return;
    const hasContent = body.trim().length > 0 || title.trim().length > 0;
    const timer = setTimeout(() => {
      void (async () => {
        setComposerStatus("saving");
        try {
          if (!hasContent) {
            await api.noteDraftClear(caseId, null);
            setComposerStatus("idle");
            setRestoredDraftAt(null);
          } else {
            const saved = await api.noteDraftSave(caseId, null, {
              title,
              body,
              sourceAnchorIds: picked,
              baseRowVersion: null,
            });
            setComposerStatus("saved");
            setRestoredDraftAt(saved.updatedAt);
          }
        } catch {
          setComposerStatus("failed");
        }
      })();
    }, 900);
    return () => clearTimeout(timer);
  }, [caseId, title, body, picked]);

  const editDirty =
    editNote !== null &&
    noteEditDirty(
      { title: editTitle, body: editBody, sourceAnchorIds: editAnchors },
      { title: editNote.title, body: editNote.body, sourceAnchorIds: editNote.sourceAnchorIds },
    );

  // Düzenleme taslağını kasaya kaydet: not kimliği + okunan sürümle birlikte.
  useEffect(() => {
    if (!editNote || !editHydrated.current) return;
    if (!editDirty) return;
    const note = editNote;
    const timer = setTimeout(() => {
      void (async () => {
        setEditStatus("saving");
        try {
          await api.noteDraftSave(caseId, note.id, {
            title: editTitle,
            body: editBody,
            sourceAnchorIds: editAnchors,
            baseRowVersion: editBaseVersion ?? note.rowVersion,
          });
          setEditStatus("saved");
        } catch {
          setEditStatus("failed");
        }
      })();
    }, 900);
    return () => clearTimeout(timer);
  }, [caseId, editNote, editDirty, editTitle, editBody, editAnchors, editBaseVersion]);

  function toggleAnchor(target: "composer" | "edit", id: string, checked: boolean) {
    const apply = (prev: string[]) => (checked ? [...prev, id] : prev.filter((value) => value !== id));
    if (target === "composer") setPicked(apply);
    else setEditAnchors(apply);
  }

  async function addNote() {
    if (body.trim().length === 0) {
      setError(EMPTY_BODY_ERROR);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await api.noteCreate(caseId, { title: title.trim(), body: body.trim(), sourceAnchorIds: picked });
      await api.noteDraftClear(caseId, null);
      setTitle("");
      setBody("");
      setPicked([]);
      setComposerStatus("idle");
      setRestoredDraftAt(null);
      await load();
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function discardComposerDraft() {
    setTitle("");
    setBody("");
    setPicked([]);
    setComposerStatus("idle");
    setRestoredDraftAt(null);
    try {
      await api.noteDraftClear(caseId, null);
    } catch (e) {
      setError(toApiError(e));
    }
  }

  async function openEdit(note: NoteRow) {
    setError(null);
    setConflict(null);
    setRevisions(null);
    setHistoryNote(null);
    setEditNote(note);
    setEditTitle(note.title);
    setEditBody(note.body);
    setEditAnchors(note.sourceAnchorIds);
    setEditStatus("idle");
    setEditPendingDraft(null);
    setEditBaseVersion(null);
    editHydrated.current = false;
    setBusy(true);
    try {
      const draft = await api.noteDraftGet(caseId, note.id);
      const differs =
        draft !== null && (draft.body !== note.body || draft.title !== note.title);
      setEditPendingDraft(differs ? draft : null);
      if (differs) setEditStatus("saved");
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
      editHydrated.current = true;
    }
  }

  function applyPendingDraft() {
    if (!editPendingDraft) return;
    setEditTitle(editPendingDraft.title);
    setEditBody(editPendingDraft.body);
    setEditAnchors(editPendingDraft.sourceAnchorIds);
    setEditBaseVersion(editPendingDraft.baseRowVersion);
    setHistoryNote(
      draftIsStale(editPendingDraft.baseRowVersion, editNote?.rowVersion ?? 0)
        ? "Taslak daha eski bir sürüme dayanıyor; kaydederken çakışma ekranı açılabilir."
        : null,
    );
    setEditPendingDraft(null);
  }

  async function dropPendingDraft() {
    setEditPendingDraft(null);
    if (!editNote) return;
    try {
      await api.noteDraftClear(caseId, editNote.id);
      setEditStatus("idle");
    } catch (e) {
      setError(toApiError(e));
    }
  }

  function closeEdit() {
    setEditNote(null);
    setConflict(null);
    setRevisions(null);
    setHistoryNote(null);
    setEditStatus("idle");
    setEditPendingDraft(null);
    setEditBaseVersion(null);
    editHydrated.current = false;
  }

  async function openConflict(noteId: string) {
    try {
      const theirs = await api.noteGet(noteId);
      if (!theirs) {
        setError({ code: "E_NOT_FOUND", message: "Not bulunamadı; liste yenilendi.", retryable: false });
        await load();
        return;
      }
      setConflict({
        noteId,
        mineTitle: editTitle,
        mineBody: editBody,
        mineAnchors: editAnchors,
        theirs,
      });
    } catch (e) {
      setError(toApiError(e));
    }
  }

  async function saveEdit() {
    if (!editNote) return;
    if (editBody.trim().length === 0) {
      setError(EMPTY_BODY_ERROR);
      return;
    }
    setBusy(true);
    setError(null);
    setHistoryNote(null);
    try {
      const updated = await api.noteUpdate(editNote.id, editNote.rowVersion, {
        title: editTitle.trim(),
        body: editBody.trim(),
        sourceAnchorIds: editAnchors,
      });
      await api.noteDraftClear(caseId, editNote.id);
      setEditNote(updated);
      setEditTitle(updated.title);
      setEditBody(updated.body);
      setEditAnchors(updated.sourceAnchorIds);
      setEditStatus("idle");
      setEditPendingDraft(null);
      setEditBaseVersion(null);
      await load();
      if (revisions?.noteId === updated.id) await showRevisions(updated.id);
    } catch (e) {
      const err = toApiError(e);
      if (err.code === "E_ROW_VERSION") await openConflict(editNote.id);
      else setError(err);
    } finally {
      setBusy(false);
    }
  }

  async function resolveConflict(keepMine: boolean) {
    if (!conflict) return;
    setBusy(true);
    setError(null);
    try {
      if (keepMine) {
        if (conflict.mineBody.trim().length === 0) {
          setError(EMPTY_BODY_ERROR);
          return;
        }
        const updated = await api.noteUpdate(conflict.noteId, conflict.theirs.rowVersion, {
          title: conflict.mineTitle.trim(),
          body: conflict.mineBody.trim(),
          sourceAnchorIds: conflict.mineAnchors,
        });
        await api.noteDraftClear(caseId, conflict.noteId);
        setEditNote(updated);
        setEditTitle(updated.title);
        setEditBody(updated.body);
        setEditAnchors(updated.sourceAnchorIds);
        setEditStatus("idle");
        setEditBaseVersion(null);
        setConflict(null);
        await load();
      } else {
        const theirs = conflict.theirs;
        await api.noteDraftClear(caseId, conflict.noteId);
        setEditNote(theirs);
        setEditTitle(theirs.title);
        setEditBody(theirs.body);
        setEditAnchors(theirs.sourceAnchorIds);
        setEditStatus("idle");
        setEditBaseVersion(null);
        setConflict(null);
        await load();
      }
    } catch (e) {
      const err = toApiError(e);
      if (err.code === "E_ROW_VERSION") await openConflict(conflict.noteId);
      else setError(err);
    } finally {
      setBusy(false);
    }
  }

  async function stepHistory(direction: "undo" | "redo") {
    if (!editNote || editDirty) return;
    setBusy(true);
    setError(null);
    setHistoryNote(null);
    try {
      const next =
        direction === "undo"
          ? await api.noteUndo(editNote.id, editNote.rowVersion)
          : await api.noteRedo(editNote.id, editNote.rowVersion);
      setEditNote(next);
      setEditTitle(next.title);
      setEditBody(next.body);
      setEditAnchors(next.sourceAnchorIds);
      setEditStatus("idle");
      setEditPendingDraft(null);
      setEditBaseVersion(null);
      await load();
      if (revisions?.noteId === next.id) await showRevisions(next.id);
    } catch (e) {
      const err = toApiError(e);
      if (err.code === "E_ROW_VERSION") await openConflict(editNote.id);
      else if (err.code === "E_NOTE_HISTORY") setHistoryNote(err.message);
      else setError(err);
    } finally {
      setBusy(false);
    }
  }

  async function showRevisions(noteId: string) {
    try {
      const list = await api.noteRevisions(noteId);
      setRevisions({ noteId, rows: list });
    } catch (e) {
      setError(toApiError(e));
    }
  }

  async function remove(id: string) {
    try {
      await api.noteDelete(id);
      if (editNote?.id === id) closeEdit();
      await load();
    } catch (e) {
      setError(toApiError(e));
    }
  }

  const conflictDiff = conflict ? diffNoteText(conflict.mineBody, conflict.theirs.body) : [];
  const conflictStats = diffSummary(conflictDiff);

  return (
    <div className="card tight" style={{ background: "var(--surface)" }}>
      <div className="card-head">
        <div>
          <h2>Notlar</h2>
          <p className="card-sub">
            {rows.length} not · taslak ve düzenleme geçmişi kasa içinde tutulur
          </p>
        </div>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="field">
        <label htmlFor="note-title">Başlık (isteğe bağlı)</label>
        <input id="note-title" className="input" value={title} onChange={(e) => setTitle(e.target.value)} />
      </div>
      <div className="field">
        <label>Kaynak alıntılar</label>
        <AnchorPicker
          anchors={anchors}
          picked={picked}
          idPrefix="note-new"
          onToggle={(id, checked) => toggleAnchor("composer", id, checked)}
        />
      </div>
      <div className="field">
        <label htmlFor="note-body">Not</label>
        <textarea
          id="note-body"
          className="input"
          style={{ height: 92, padding: 10, resize: "vertical" }}
          value={body}
          onChange={(e) => setBody(e.target.value)}
          placeholder="Gözlem, eksik belge, duruşma notu…"
        />
      </div>
      <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
        <span className="hint" role="status">
          {draftStatusLabel(composerStatus)}
          {restoredDraftAt ? ` · ${formatTimestamp(restoredDraftAt)}` : ""}
          {composerStatus === "failed" ? " · kasa kilitliyse taslak yazılamaz" : ""}
        </span>
        <span style={{ display: "flex", gap: 8 }}>
          <button
            className="btn"
            disabled={busy || (body.trim().length === 0 && title.trim().length === 0)}
            onClick={() => void discardComposerDraft()}
          >
            Taslağı sil
          </button>
          <button className="btn primary" disabled={busy} onClick={() => void addNote()}>
            Notu ekle
          </button>
        </span>
      </div>

      {rows.length > 0 ? (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {rows.map((note) => (
            <div
              key={note.id}
              style={{
                background: "var(--bg)",
                border: "1px solid var(--line)",
                borderRadius: "var(--radius)",
                padding: "10px 12px",
              }}
            >
              <div className="row" style={{ justifyContent: "space-between" }}>
                <strong style={{ fontWeight: 500 }}>{note.title || "Başlıksız not"}</strong>
                <span style={{ display: "flex", gap: 8, alignItems: "center" }}>
                  <span className="muted small">
                    {formatTimestamp(note.updatedAt)} · sürüm {note.rowVersion}
                  </span>
                  <button className="btn ghost" style={{ height: 24 }} onClick={() => void openEdit(note)}>
                    Düzenle
                  </button>
                  <button className="btn ghost" style={{ height: 24 }} onClick={() => void remove(note.id)}>
                    Sil
                  </button>
                </span>
              </div>
              <p style={{ margin: "6px 0 0", whiteSpace: "pre-wrap" }}>{note.body}</p>
            </div>
          ))}
        </div>
      ) : null}

      {editNote ? (
        <div
          style={{
            border: "1px solid var(--line)",
            borderRadius: "var(--radius)",
            padding: "12px 14px",
            background: "var(--surface-2)",
            display: "flex",
            flexDirection: "column",
            gap: 10,
          }}
        >
          <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
            <div>
              <strong style={{ fontWeight: 500 }}>Notu düzenle</strong>
              <p className="card-sub" style={{ margin: 0 }}>
                sürüm {editNote.rowVersion} · revizyon {editNote.revisionNo} · {formatTimestamp(editNote.updatedAt)}
              </p>
            </div>
            <button className="btn ghost" style={{ height: 24 }} onClick={closeEdit}>
              Kapat
            </button>
          </div>

          {editPendingDraft ? (
            <div className="notice" role="status">
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                <span>
                  Kasadaki taslak bu nottan farklı ({formatTimestamp(editPendingDraft.updatedAt)}).
                  {draftIsStale(editPendingDraft.baseRowVersion, editNote.rowVersion)
                    ? " Taslak eski bir sürüme dayanıyor."
                    : ""}
                </span>
                <span style={{ display: "flex", gap: 8 }}>
                  <button className="btn" style={{ height: 26 }} onClick={applyPendingDraft}>
                    Taslağı yükle
                  </button>
                  <button className="btn ghost" style={{ height: 26 }} onClick={() => void dropPendingDraft()}>
                    Taslağı sil
                  </button>
                </span>
              </div>
            </div>
          ) : null}

          {conflict ? (
            <div className="notice danger" role="alert">
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                <strong>Sürüm çakışması</strong>
                <span>{CONFLICT_HINT}</span>
                <span className="small">
                  Benim metnim {conflictStats.mineOnly} satırda farklı, kasadaki metin{" "}
                  {conflictStats.theirsOnly} satırda farklı.
                </span>
                <div
                  style={{
                    maxHeight: 190,
                    overflow: "auto",
                    background: "var(--bg)",
                    border: "1px solid var(--line)",
                    borderRadius: "var(--radius)",
                    padding: "8px 10px",
                    fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
                    fontSize: 12,
                  }}
                >
                  {conflictDiff.map((line, index) => (
                    <div
                      key={`${line.kind}-${index}`}
                      style={{
                        whiteSpace: "pre-wrap",
                        background:
                          line.kind === "mine"
                            ? "var(--danger-soft)"
                            : line.kind === "theirs"
                              ? "var(--ok-soft)"
                              : "transparent",
                        color: line.kind === "mine" ? "var(--danger)" : line.kind === "theirs" ? "var(--ok)" : "var(--ink-2)",
                        padding: "1px 4px",
                      }}
                    >
                      <span aria-hidden="true">
                        {line.kind === "mine" ? "− " : line.kind === "theirs" ? "+ " : "  "}
                      </span>
                      {line.text.length === 0 ? "\u00a0" : line.text}
                    </div>
                  ))}
                </div>
                <span style={{ display: "flex", gap: 8 }}>
                  <button className="btn" disabled={busy} onClick={() => void resolveConflict(true)}>
                    Benim metnimi kaydet
                  </button>
                  <button className="btn" disabled={busy} onClick={() => void resolveConflict(false)}>
                    Güncel metni koru
                  </button>
                </span>
                <span className="small">
                  Kasadaki güncel sürüm {conflict.theirs.rowVersion} · {formatTimestamp(conflict.theirs.updatedAt)}
                </span>
              </div>
            </div>
          ) : null}

          {historyNote ? (
            <p className="hint" role="status" style={{ margin: 0 }}>
              {historyNote}
            </p>
          ) : null}

          <div className="field">
            <label htmlFor="note-edit-title">Başlık</label>
            <input
              id="note-edit-title"
              className="input"
              value={editTitle}
              disabled={conflict !== null}
              onChange={(e) => setEditTitle(e.target.value)}
            />
          </div>
          <div className="field">
            <label>Kaynak alıntılar</label>
            <AnchorPicker
              anchors={anchors}
              picked={editAnchors}
              idPrefix="note-edit"
              onToggle={(id, checked) => toggleAnchor("edit", id, checked)}
            />
          </div>
          <div className="field">
            <label htmlFor="note-edit-body">Metin</label>
            <textarea
              id="note-edit-body"
              className="input"
              style={{ height: 110, padding: 10, resize: "vertical" }}
              value={editBody}
              disabled={conflict !== null}
              onChange={(e) => setEditBody(e.target.value)}
            />
          </div>
          <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
            <span className="hint" role="status">
              {editDirty ? "Kaydedilmemiş değişiklik · " : ""}
              {draftStatusLabel(editStatus)}
            </span>
            <span style={{ display: "flex", gap: 8 }}>
              <button
                className="btn"
                disabled={busy || editDirty || !editNote.canUndo}
                title={editDirty ? "Önce kaydedin" : "Son kaydı geri al"}
                onClick={() => void stepHistory("undo")}
              >
                Geri al
              </button>
              <button
                className="btn"
                disabled={busy || editDirty || !editNote.canRedo}
                title={editDirty ? "Önce kaydedin" : "Geri alınan kaydı ileri al"}
                onClick={() => void stepHistory("redo")}
              >
                İleri al
              </button>
              <button className="btn" disabled={busy} onClick={() => void showRevisions(editNote.id)}>
                Sürüm geçmişi
              </button>
              <button className="btn primary" disabled={busy || !editDirty} onClick={() => void saveEdit()}>
                Kaydet
              </button>
            </span>
          </div>

          {revisions && revisions.noteId === editNote.id ? (
            <div
              style={{
                border: "1px solid var(--line)",
                borderRadius: "var(--radius)",
                background: "var(--bg)",
                padding: "8px 10px",
                display: "flex",
                flexDirection: "column",
                gap: 4,
              }}
            >
              <strong style={{ fontWeight: 500, fontSize: 13 }}>Sürüm geçmişi</strong>
              {revisions.rows.length === 0 ? (
                <span className="hint">Kayıt yok.</span>
              ) : (
                revisions.rows.map((revision) => (
                  <span key={revision.revisionNo} className="small" style={{ display: "flex", gap: 8 }}>
                    <span className="muted" style={{ minWidth: 34 }}>
                      #{revision.revisionNo}
                    </span>
                    <span style={{ minWidth: 120 }}>{revisionOriginLabel(revision.origin)}</span>
                    <span className="muted">{formatTimestamp(revision.createdAt)}</span>
                  </span>
                ))
              )}
            </div>
          ) : null}
        </div>
      ) : null}

      <p className="hint" style={{ margin: 0 }}>
        Notlar düz metindir; HTML çalıştırılmaz. Taslak yalnız açık kasaya yazılır, tarayıcı
        deposuna kaydedilmez. Geri/ileri al kasa içindeki gerçek revizyonları kullanır.
      </p>
    </div>
  );
}
