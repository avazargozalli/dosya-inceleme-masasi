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
import { t } from "../lib/i18n";
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

function emptyBodyError(): ApiError {
  return { code: "E_NOTE", message: t("notes.emptyBody"), retryable: false };
}

/** Kaynak seçimi **sıralı** karşılaştırılır: çipler tıklama sırasını korur. */
function sameIds(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((value, index) => value === b[index]);
}

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
        {t("notes.noAnchors")}
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
  /** Silme uçuşu satır kimliğine bağlıdır: ikinci tık ikinci `note_delete` göndermez. */
  const [removingId, setRemovingId] = useState<string | null>(null);

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

  /** Liste okumalarının sırası: geç gelen eski yanıt yenisini ezmez. */
  const listSeq = useRef(0);
  /** Düzenleme paneli isteklerinin sırası: geç gelen taslak okuması paneli ele geçirmez. */
  const editSeq = useRef(0);
  /** Dosya kuşağı: dosya değişince uçuştaki yazmaların durum yazmasını engeller. */
  const caseGen = useRef(0);
  /** Uçuştaki besteci taslağı yazımı: temizlemeden önce beklenir (taslak geri gelmesin). */
  const draftSaveChain = useRef<Promise<unknown>>(Promise.resolve());
  /**
   * Yazma kuşağı: dosya değişimi ve panelin kapanması uçuştaki yanıtı **geçersiz** kılar.
   * Kilit devri de bu sayaçla yapılır: devredilen isteğin `finally`si yeni yazmanın
   * kilidini erken açamaz (`endWrite` yalnız kendi damgasında bırakır).
   */
  const busySeq = useRef(0);
  /** Yanıt geldiğinde "kullanıcı hâlâ gönderilen metni mi görüyor" karşılaştırması. */
  const composerNow = useRef({ title, body, picked });
  const editNow = useRef({ title: editTitle, body: editBody, anchors: editAnchors });

  useEffect(() => {
    composerNow.current = { title, body, picked };
  }, [title, body, picked]);

  useEffect(() => {
    editNow.current = { title: editTitle, body: editBody, anchors: editAnchors };
  }, [editTitle, editBody, editAnchors]);

  /** Yeni bir yazma uçuşu açar; dönen damga yalnız o uçuşta geçerlidir. */
  function beginWrite(): number {
    const seq = busySeq.current + 1;
    busySeq.current = seq;
    setBusy(true);
    return seq;
  }

  /** Kilidi yalnız kendi uçuşunda bırakır (erken açılma kapısı). */
  function endWrite(seq: number): void {
    if (busySeq.current === seq) setBusy(false);
  }

  /** Uçuş hâlâ güncel mi? Dosya değişimi ve panel kapanışı damgayı ilerletir. */
  function stillCurrent(seq: number): boolean {
    return busySeq.current === seq;
  }

  const load = useCallback(async () => {
    const seq = listSeq.current + 1;
    listSeq.current = seq;
    setError(null);
    try {
      const next = await api.noteList(caseId);
      if (listSeq.current !== seq) return;
      setRows(next);
    } catch (e) {
      if (listSeq.current !== seq) return;
      setRows([]);
      setError(toApiError(e));
    }
  }, [caseId]);

  // Dosya değişimi: önceki dosyanın **bütün** türetilmiş durumu istekten önce kalkar (INV-05).
  // Not, düzenleme paneli, çakışma, geçmiş ve besteci metni taşınırsa yanlış dosyaya yazılır.
  useEffect(() => {
    caseGen.current += 1;
    listSeq.current += 1;
    editSeq.current += 1;
    // Yazma kilidi devredilir: uçuştaki eski isteğin yanıtı yeni dosyaya yazmaz.
    busySeq.current += 1;
    setRows([]);
    setError(null);
    setBusy(false);
    setRemovingId(null);
    setTitle("");
    setBody("");
    setPicked([]);
    setComposerStatus("idle");
    setRestoredDraftAt(null);
    composerHydrated.current = false;
    setEditNote(null);
    setEditTitle("");
    setEditBody("");
    setEditAnchors([]);
    setEditStatus("idle");
    setEditPendingDraft(null);
    setEditBaseVersion(null);
    setHistoryNote(null);
    setConflict(null);
    setRevisions(null);
    editHydrated.current = false;
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
    const gen = caseGen.current;
    const timer = setTimeout(() => {
      const write = async () => {
        setComposerStatus("saving");
        try {
          if (!hasContent) {
            await api.noteDraftClear(caseId, null);
            if (caseGen.current !== gen) return;
            setComposerStatus("idle");
            setRestoredDraftAt(null);
          } else {
            const saved = await api.noteDraftSave(caseId, null, {
              title,
              body,
              sourceAnchorIds: picked,
              baseRowVersion: null,
            });
            if (caseGen.current !== gen) return;
            setComposerStatus("saved");
            setRestoredDraftAt(saved.updatedAt);
          }
        } catch {
          if (caseGen.current !== gen) return;
          setComposerStatus("failed");
        }
      };
      // Yazmalar sıraya bağlanır: uçuştaki yazma bitmeden "temizle" çağrılmaz.
      const task = draftSaveChain.current.then(write, write);
      draftSaveChain.current = task.catch(() => undefined);
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
    // Savunma: dosya değiştiyse eski dosyanın notu için taslak **yazılmaz**.
    if (editNote.caseId !== caseId) return;
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
      setError(emptyBodyError());
      return;
    }
    const seq = beginWrite();
    // Ekranda duran **ham** değerler dondurulur; çekirdeğe kırpılmış metin gider.
    const raw = { title, body, picked: [...picked] };
    const submitted = { title: raw.title.trim(), body: raw.body.trim(), picked: raw.picked };
    setError(null);
    try {
      await api.noteCreate(caseId, {
        title: submitted.title,
        body: submitted.body,
        sourceAnchorIds: submitted.picked,
      });
      // Uçuştaki taslak yazımı bitmeden temizleme: geç dönen yazma taslağı geri getirirdi.
      await draftSaveChain.current;
      // Kasadaki taslak **gönderilen** dosyanın taslağıdır; dosya değişse de silinir.
      await api.noteDraftClear(caseId, null);
      // Geç yanıt: dosya değişti ya da başka bir yazma başladı → ekrana yazılmaz.
      if (!stillCurrent(seq)) return;
      // Uçuşta yazılan yeni taslak, gönderilen metin değiştiyse **korunur**.
      const now = composerNow.current;
      if (
        now.title === raw.title &&
        now.body === raw.body &&
        sameIds(now.picked, raw.picked)
      ) {
        setTitle("");
        setBody("");
        setPicked([]);
        setComposerStatus("idle");
        setRestoredDraftAt(null);
      }
      await load();
    } catch (e) {
      if (!stillCurrent(seq)) return;
      setError(toApiError(e));
    } finally {
      endWrite(seq);
    }
  }

  async function discardComposerDraft() {
    setTitle("");
    setBody("");
    setPicked([]);
    setComposerStatus("idle");
    setRestoredDraftAt(null);
    try {
      await draftSaveChain.current;
      await api.noteDraftClear(caseId, null);
    } catch (e) {
      setError(toApiError(e));
    }
  }

  async function openEdit(note: NoteRow) {
    const seq = editSeq.current + 1;
    editSeq.current = seq;
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
    const lock = beginWrite();
    try {
      const draft = await api.noteDraftGet(caseId, note.id);
      // Geç gelen okuma başka bir notun panelini ele geçirmez.
      if (editSeq.current !== seq) return;
      const differs =
        draft !== null && (draft.body !== note.body || draft.title !== note.title);
      setEditPendingDraft(differs ? draft : null);
      if (differs) setEditStatus("saved");
    } catch (e) {
      if (editSeq.current !== seq) return;
      setError(toApiError(e));
    } finally {
      endWrite(lock);
      if (editSeq.current === seq) editHydrated.current = true;
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
        ? t("notes.draftStaleLong")
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
    editSeq.current += 1;
    // Panel kapanışı uçuştaki kaydı geçersiz kılar: yanıt paneli yeniden açmaz.
    busySeq.current += 1;
    setEditNote(null);
    setConflict(null);
    setRevisions(null);
    setHistoryNote(null);
    setEditStatus("idle");
    setEditPendingDraft(null);
    setEditBaseVersion(null);
    setBusy(false);
    editHydrated.current = false;
  }

  async function openConflict(
    noteId: string,
    mine: { title: string; body: string; anchors: string[] },
    seq: number,
  ) {
    try {
      const theirs = await api.noteGet(noteId);
      // Uçuş geçersizse (dosya değişti / panel kapandı / başka yazma başladı) çakışma açılmaz.
      if (!stillCurrent(seq)) return;
      if (!theirs) {
        setError({ code: "E_NOT_FOUND", message: t("notes.notFound"), retryable: false });
        await load();
        return;
      }
      // "Benim metnim" **gönderilen** metindir: başka panelin metni bu nota yazılmaz.
      setConflict({
        noteId,
        mineTitle: mine.title,
        mineBody: mine.body,
        mineAnchors: mine.anchors,
        theirs,
      });
    } catch (e) {
      if (!stillCurrent(seq)) return;
      setError(toApiError(e));
    }
  }

  async function saveEdit() {
    if (!editNote) return;
    if (editBody.trim().length === 0) {
      setError(emptyBodyError());
      return;
    }
    const note = editNote;
    const seq = beginWrite();
    // Panelde duran **ham** değerler dondurulur; çekirdeğe kırpılmış metin gider.
    const raw = { title: editTitle, body: editBody, anchors: [...editAnchors] };
    const submitted = { title: raw.title.trim(), body: raw.body.trim(), anchors: raw.anchors };
    setError(null);
    setHistoryNote(null);
    try {
      const updated = await api.noteUpdate(note.id, note.rowVersion, {
        title: submitted.title,
        body: submitted.body,
        sourceAnchorIds: submitted.anchors,
      });
      await api.noteDraftClear(caseId, note.id);
      // Geç yanıt (dosya değişti / panel kapandı / başka yazma): panel yeniden açılmaz.
      if (!stillCurrent(seq)) return;
      setEditNote(updated);
      // Uçuşta yazılan metin korunur: sunucunun metni yalnız kullanıcı değiştirmediyse yazılır.
      const now = editNow.current;
      if (
        now.title === raw.title &&
        now.body === raw.body &&
        sameIds(now.anchors, raw.anchors)
      ) {
        setEditTitle(updated.title);
        setEditBody(updated.body);
        setEditAnchors(updated.sourceAnchorIds);
      }
      setEditStatus("idle");
      setEditPendingDraft(null);
      setEditBaseVersion(null);
      await load();
      if (revisions?.noteId === updated.id) await showRevisions(updated.id);
    } catch (e) {
      if (!stillCurrent(seq)) return;
      const err = toApiError(e);
      if (err.code === "E_ROW_VERSION") await openConflict(note.id, submitted, seq);
      else setError(err);
    } finally {
      endWrite(seq);
    }
  }

  async function resolveConflict(keepMine: boolean) {
    if (!conflict) return;
    if (keepMine && conflict.mineBody.trim().length === 0) {
      setError(emptyBodyError());
      return;
    }
    const snapshot = conflict;
    const seq = beginWrite();
    setError(null);
    try {
      let updated: NoteRow;
      if (keepMine) {
        updated = await api.noteUpdate(snapshot.noteId, snapshot.theirs.rowVersion, {
          title: snapshot.mineTitle.trim(),
          body: snapshot.mineBody.trim(),
          sourceAnchorIds: snapshot.mineAnchors,
        });
      } else {
        updated = snapshot.theirs;
      }
      await api.noteDraftClear(caseId, snapshot.noteId);
      if (!stillCurrent(seq)) return;
      setEditNote(updated);
      setEditTitle(updated.title);
      setEditBody(updated.body);
      setEditAnchors(updated.sourceAnchorIds);
      setEditStatus("idle");
      setEditBaseVersion(null);
      setConflict(null);
      await load();
    } catch (e) {
      if (!stillCurrent(seq)) return;
      const err = toApiError(e);
      if (err.code === "E_ROW_VERSION") {
        await openConflict(
          snapshot.noteId,
          { title: snapshot.mineTitle, body: snapshot.mineBody, anchors: snapshot.mineAnchors },
          seq,
        );
      } else {
        setError(err);
      }
    } finally {
      endWrite(seq);
    }
  }

  async function stepHistory(direction: "undo" | "redo") {
    if (!editNote || editDirty) return;
    const note = editNote;
    const mine = { title: editTitle, body: editBody, anchors: [...editAnchors] };
    const seq = beginWrite();
    setError(null);
    setHistoryNote(null);
    try {
      const next =
        direction === "undo"
          ? await api.noteUndo(note.id, note.rowVersion)
          : await api.noteRedo(note.id, note.rowVersion);
      if (!stillCurrent(seq)) return;
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
      if (!stillCurrent(seq)) return;
      const err = toApiError(e);
      if (err.code === "E_ROW_VERSION") await openConflict(note.id, mine, seq);
      else if (err.code === "E_NOTE_HISTORY") setHistoryNote(err.message);
      else setError(err);
    } finally {
      endWrite(seq);
    }
  }

  async function showRevisions(noteId: string) {
    setError(null);
    try {
      const list = await api.noteRevisions(noteId);
      setRevisions({ noteId, rows: list });
    } catch (e) {
      setError(toApiError(e));
    }
  }

  async function remove(id: string) {
    const gen = caseGen.current;
    setRemovingId(id);
    setError(null);
    try {
      await api.noteDelete(id);
      if (caseGen.current !== gen) return;
      if (editNote?.id === id) closeEdit();
      await load();
    } catch (e) {
      if (caseGen.current !== gen) return;
      setError(toApiError(e));
    } finally {
      if (caseGen.current === gen) setRemovingId(null);
    }
  }

  const conflictDiff = conflict ? diffNoteText(conflict.mineBody, conflict.theirs.body) : [];
  const conflictStats = diffSummary(conflictDiff);

  return (
    <div className="card tight" style={{ background: "var(--surface)" }}>
      <div className="card-head">
        <div>
          <h2>{t("notes.title")}</h2>
          <p className="card-sub">{t("notes.cardSub", { count: rows.length })}</p>
        </div>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="field">
        <label htmlFor="note-title">{t("notes.titleOptional")}</label>
        <input id="note-title" className="input" value={title} onChange={(e) => setTitle(e.target.value)} />
      </div>
      <div className="field">
        <label>{t("notes.sourceAnchors")}</label>
        <AnchorPicker
          anchors={anchors}
          picked={picked}
          idPrefix="note-new"
          onToggle={(id, checked) => toggleAnchor("composer", id, checked)}
        />
      </div>
      <div className="field">
        <label htmlFor="note-body">{t("notes.body")}</label>
        <textarea
          id="note-body"
          className="input"
          style={{ height: 92, padding: 10, resize: "vertical" }}
          value={body}
          onChange={(e) => setBody(e.target.value)}
          placeholder={t("notes.bodyPlaceholder")}
        />
      </div>
      <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
        <span className="hint" role="status">
          {draftStatusLabel(composerStatus)}
          {restoredDraftAt ? ` · ${formatTimestamp(restoredDraftAt)}` : ""}
          {composerStatus === "failed" ? ` · ${t("notes.lockedDraft")}` : ""}
        </span>
        <span style={{ display: "flex", gap: 8 }}>
          <button
            className="btn"
            disabled={busy || (body.trim().length === 0 && title.trim().length === 0)}
            onClick={() => void discardComposerDraft()}
          >
            {t("notes.discardDraft")}
          </button>
          <button className="btn primary" disabled={busy} onClick={() => void addNote()}>
            {t("notes.add")}
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
                <strong style={{ fontWeight: 500 }}>{note.title || t("notes.untitled")}</strong>
                <span style={{ display: "flex", gap: 8, alignItems: "center" }}>
                  <span className="muted small">
                    {formatTimestamp(note.updatedAt)} · {t("notes.versionLabel", { version: note.rowVersion })}
                  </span>
                  <button className="btn ghost" style={{ height: 24 }} onClick={() => void openEdit(note)}>
                    {t("notes.edit")}
                  </button>
                  <button
                    className="btn ghost"
                    style={{ height: 24 }}
                    disabled={removingId !== null || busy}
                    onClick={() => void remove(note.id)}
                  >
                    {t("notes.delete")}
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
              <strong style={{ fontWeight: 500 }}>{t("notes.editTitle")}</strong>
              <p className="card-sub" style={{ margin: 0 }}>
                {t("notes.editMeta", {
                  version: editNote.rowVersion,
                  revision: editNote.revisionNo,
                  time: formatTimestamp(editNote.updatedAt),
                })}
              </p>
            </div>
            <button className="btn ghost" style={{ height: 24 }} onClick={closeEdit}>
              {t("notes.close")}
            </button>
          </div>

          {editPendingDraft ? (
            <div className="notice" role="status">
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                <span>
                  {t("notes.draftDiffers", { time: formatTimestamp(editPendingDraft.updatedAt) })}
                  {draftIsStale(editPendingDraft.baseRowVersion, editNote.rowVersion)
                    ? ` ${t("notes.draftStaleShort")}`
                    : ""}
                </span>
                <span style={{ display: "flex", gap: 8 }}>
                  <button className="btn" style={{ height: 26 }} onClick={applyPendingDraft}>
                    {t("notes.loadDraft")}
                  </button>
                  <button className="btn ghost" style={{ height: 26 }} onClick={() => void dropPendingDraft()}>
                    {t("notes.discardDraft")}
                  </button>
                </span>
              </div>
            </div>
          ) : null}

          {conflict ? (
            <div className="notice danger" role="alert">
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                <strong>{t("notes.conflictTitle")}</strong>
                <span>{CONFLICT_HINT}</span>
                <span className="small">
                  {t("notes.conflictStats", {
                    mine: conflictStats.mineOnly,
                    theirs: conflictStats.theirsOnly,
                  })}
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
                    {t("notes.keepMine")}
                  </button>
                  <button className="btn" disabled={busy} onClick={() => void resolveConflict(false)}>
                    {t("notes.keepCurrent")}
                  </button>
                </span>
                <span className="small">
                  {t("notes.currentVersion", {
                    version: conflict.theirs.rowVersion,
                    time: formatTimestamp(conflict.theirs.updatedAt),
                  })}
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
            <label htmlFor="note-edit-title">{t("notes.editTitleLabel")}</label>
            <input
              id="note-edit-title"
              className="input"
              value={editTitle}
              disabled={conflict !== null}
              onChange={(e) => setEditTitle(e.target.value)}
            />
          </div>
          <div className="field">
            <label>{t("notes.sourceAnchors")}</label>
            <AnchorPicker
              anchors={anchors}
              picked={editAnchors}
              idPrefix="note-edit"
              onToggle={(id, checked) => toggleAnchor("edit", id, checked)}
            />
          </div>
          <div className="field">
            <label htmlFor="note-edit-body">{t("notes.editBodyLabel")}</label>
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
              {editDirty ? `${t("notes.unsaved")} · ` : ""}
              {draftStatusLabel(editStatus)}
            </span>
            <span style={{ display: "flex", gap: 8 }}>
              <button
                className="btn"
                disabled={busy || editDirty || !editNote.canUndo}
                title={editDirty ? t("notes.saveFirst") : t("notes.undoTitle")}
                onClick={() => void stepHistory("undo")}
              >
                {t("notes.undo")}
              </button>
              <button
                className="btn"
                disabled={busy || editDirty || !editNote.canRedo}
                title={editDirty ? t("notes.saveFirst") : t("notes.redoTitle")}
                onClick={() => void stepHistory("redo")}
              >
                {t("notes.redo")}
              </button>
              <button className="btn" disabled={busy} onClick={() => void showRevisions(editNote.id)}>
                {t("notes.historyTitle")}
              </button>
              <button className="btn primary" disabled={busy || !editDirty} onClick={() => void saveEdit()}>
                {t("common.save")}
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
              <strong style={{ fontWeight: 500, fontSize: 13 }}>{t("notes.historyTitle")}</strong>
              {revisions.rows.length === 0 ? (
                <span className="hint">{t("notes.historyEmpty")}</span>
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
        {t("notes.footnote")}
      </p>
    </div>
  );
}
