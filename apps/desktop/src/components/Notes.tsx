import { useCallback, useEffect, useState } from "react";
import { api, toApiError, type ApiError, type NoteRow } from "../lib/api";
import { formatTimestamp } from "../lib/format";

export function Notes({ caseId }: { caseId: string }) {
  const [rows, setRows] = useState<NoteRow[]>([]);
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

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

  async function add() {
    if (body.trim().length === 0) {
      setError({ code: "E_NOTE", message: "Not metni boş olamaz.", retryable: false });
      return;
    }
    setBusy(true);
    try {
      await api.noteCreate(caseId, { title: title.trim(), body: body.trim() });
      setTitle("");
      setBody("");
      await load();
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function remove(id: string) {
    try {
      await api.noteDelete(id);
      await load();
    } catch (e) {
      setError(toApiError(e));
    }
  }

  return (
    <div className="card tight" style={{ background: "var(--surface)" }}>
      <div className="card-head">
        <div>
          <h2>Notlar</h2>
          <p className="card-sub">{rows.length} not · kaynak bağlama F08 devamında</p>
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
      <div className="row" style={{ justifyContent: "flex-end" }}>
        <button className="btn primary" disabled={busy} onClick={() => void add()}>Notu ekle</button>
      </div>

      {rows.length > 0 ? (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {rows.map((note) => (
            <div key={note.id} style={{ background: "var(--bg)", border: "1px solid var(--line)", borderRadius: "var(--radius)", padding: "10px 12px" }}>
              <div className="row" style={{ justifyContent: "space-between" }}>
                <strong style={{ fontWeight: 500 }}>{note.title || "Başlıksız not"}</strong>
                <span style={{ display: "flex", gap: 8, alignItems: "center" }}>
                  <span className="muted small">{formatTimestamp(note.updatedAt)}</span>
                  <button className="btn ghost" style={{ height: 24 }} onClick={() => void remove(note.id)}>Sil</button>
                </span>
              </div>
              <p style={{ margin: "6px 0 0", whiteSpace: "pre-wrap" }}>{note.body}</p>
            </div>
          ))}
        </div>
      ) : null}
      <p className="hint" style={{ margin: 0 }}>
        Notlar düz metindir; HTML çalıştırılmaz. Kaynağa bağlı alıntı F08'de eklenecek.
      </p>
    </div>
  );
}
