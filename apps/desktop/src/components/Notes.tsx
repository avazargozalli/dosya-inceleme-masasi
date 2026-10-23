import { useCallback, useEffect, useState } from "react";
import { api, toApiError, type AnchorRow, type ApiError, type NoteRow } from "../lib/api";
import { formatTimestamp } from "../lib/format";

export function Notes({ caseId, anchors }: { caseId: string; anchors: AnchorRow[] }) {
  const [rows, setRows] = useState<NoteRow[]>([]);
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [picked, setPicked] = useState<string[]>([]);
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
      await api.noteCreate(caseId, {
        title: title.trim(),
        body: body.trim(),
        sourceAnchorIds: picked,
      });
      setTitle("");
      setBody("");
      setPicked([]);
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
      {anchors.length > 0 ? (
        <div className="field">
          <label>Kaynak alıntılar</label>
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {anchors.map((anchor) => (
              <label key={anchor.id} className="row" style={{ alignItems: "flex-start", gap: 8, padding: "6px 8px", border: "1px solid var(--line)", borderRadius: "var(--radius)", background: "var(--bg)" }}>
                <input
                  type="checkbox"
                  checked={picked.includes(anchor.id)}
                  onChange={(e) =>
                    setPicked((prev) => (e.target.checked ? [...prev, anchor.id] : prev.filter((id) => id !== anchor.id)))
                  }
                  style={{ width: 15, height: 15, marginTop: 2 }}
                />
                <span>
                  <span style={{ display: "block", fontSize: 13 }}>“{anchor.exactQuote.slice(0, 140)}”</span>
                  <span className="muted small">{anchor.prefix.slice(-24)} … {anchor.suffix.slice(0, 24)}</span>
                </span>
              </label>
            ))}
          </div>
        </div>
      ) : (
        <p className="hint" style={{ margin: 0 }}>
          Kaynak alıntı yok. Okuyucuda metin seçip “Seçimi kaynak olarak kaydet” deyin.
        </p>
      )}

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
