import { useCallback, useEffect, useMemo, useState } from "react";
import {
  api,
  toApiError,
  type AnchorRow,
  type ApiError,
  type CaseRow,
  type EventRow,
  type EventSourceRow,
} from "../lib/api";


const DATE_ROLE_LABEL: Record<string, string> = {
  event: "Olay tarihi",
  document: "Belge tarihi",
  notification: "Tebliğ tarihi",
  contract: "Sözleşme tarihi",
  hearing: "Duruşma",
  deadline_candidate: "Süre adayı",
  unknown: "Rol bilinmiyor",
};

const PRECISION_LABEL: Record<string, string> = {
  day: "Gün",
  month: "Ay",
  year: "Yıl",
  range: "Aralık",
  approximate: "Yaklaşık",
  unknown: "Bilinmiyor",
};

type Draft = {
  title: string;
  date: string;
  precision: string;
  role: string;
  kind: string;
  description: string;
};

const EMPTY: Draft = {
  title: "",
  date: "",
  precision: "day",
  role: "event",
  kind: "other",
  description: "",
};

export function Timeline() {
  const [cases, setCases] = useState<CaseRow[]>([]);
  const [caseId, setCaseId] = useState("");
  const [events, setEvents] = useState<EventRow[]>([]);
  const [linkFor, setLinkFor] = useState<string | null>(null);
  const [anchors, setAnchors] = useState<AnchorRow[]>([]);
  const [links, setLinks] = useState<EventSourceRow[]>([]);
  const [linkAnchor, setLinkAnchor] = useState("");
  const [linkRelation, setLinkRelation] = useState("states");
  const [draft, setDraft] = useState<Draft>(EMPTY);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

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
      setEvents(await api.eventList(id));
    } catch (e) {
      setError(toApiError(e));
    }
  }, []);

  useEffect(() => {
    if (caseId) void load(caseId);
  }, [caseId, load]);

  const known = useMemo(() => events.filter((e) => e.sortableDate), [events]);
  const unknown = useMemo(() => events.filter((e) => !e.sortableDate), [events]);

  async function add() {
    if (!caseId || draft.title.trim().length === 0) {
      setError({ code: "E_TITLE", message: "Olay başlığı zorunlu.", retryable: false });
      return;
    }
    setBusy(true);
    try {
      await api.eventCreate(caseId, {
        eventKind: draft.kind,
        dateValue: draft.precision === "unknown" ? null : draft.date || null,
        datePrecision: draft.precision,
        dateRole: draft.role,
        dateRaw: draft.date || null,
        title: draft.title.trim(),
        description: draft.description.trim(),
        assertionKind: "user",
      });
      setDraft(EMPTY);
      await load(caseId);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function confirm(id: string) {
    try {
      await api.eventConfirm(id);
      await load(caseId);
    } catch (e) {
      setError(toApiError(e));
    }
  }

  async function remove(id: string) {
    try {
      await api.eventDelete(id);
      await load(caseId);
    } catch (e) {
      setError(toApiError(e));
    }
  }

  async function openLink(eventId: string) {
    setLinkFor((prev) => (prev === eventId ? null : eventId));
    try {
      const [anchorRows, linkRows] = await Promise.all([
        api.anchorList(caseId),
        api.eventSources(eventId),
      ]);
      setAnchors(anchorRows);
      setLinks(linkRows);
      setLinkAnchor(anchorRows[0]?.id ?? "");
    } catch (e) {
      setError(toApiError(e));
    }
  }

  async function attach(eventId: string) {
    if (!linkAnchor) return;
    try {
      await api.eventLinkSource(eventId, linkAnchor, linkRelation);
      setLinks(await api.eventSources(eventId));
    } catch (e) {
      setError(toApiError(e));
    }
  }

  async function detach(id: string, eventId: string) {
    try {
      await api.eventUnlinkSource(id);
      setLinks(await api.eventSources(eventId));
    } catch (e) {
      setError(toApiError(e));
    }
  }

  function row(event: EventRow) {
    return (
      <div key={event.id} className="card tight" style={{ background: "var(--surface)" }}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
          <div>
            <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
              <strong style={{ fontWeight: 500 }}>{event.title}</strong>
              <span className="badge plain">{DATE_ROLE_LABEL[event.dateRole] ?? event.dateRole}</span>
              <span className={`badge ${event.status === "confirmed" ? "ok" : "warn"}`}>
                {event.status === "confirmed" ? "onaylandı" : "taslak"}
              </span>
            </div>
            <div className="muted small" style={{ marginTop: 4 }}>
              {event.sortableDate ?? "tarih bilinmiyor"} · {PRECISION_LABEL[event.datePrecision] ?? event.datePrecision}
              {event.dateRaw && event.dateRaw !== event.sortableDate ? ` · ham: ${event.dateRaw}` : ""}
            </div>
            {event.description ? <p style={{ margin: "6px 0 0" }}>{event.description}</p> : null}
          </div>
          <div style={{ display: "flex", gap: 6, flexShrink: 0 }}>
            {event.status === "draft" ? (
              <button className="btn ghost" onClick={() => void confirm(event.id)}>Onayla</button>
            ) : null}
            <button className="btn ghost" onClick={() => void openLink(event.id)}>
              {linkFor === event.id ? "Kaynakları kapat" : "Kaynaklar"}
            </button>
            <button className="btn ghost" onClick={() => void remove(event.id)}>Sil</button>
          </div>
        </div>

        {linkFor === event.id ? (
          <div style={{ marginTop: 10, borderTop: "1px solid var(--border)", paddingTop: 10 }}>
            {links.length > 0 ? (
              <ul style={{ listStyle: "none", padding: 0, margin: "0 0 8px", display: "grid", gap: 4 }}>
                {links.map((link) => (
                  <li key={link.id} className="muted small" style={{ display: "flex", gap: 8, alignItems: "center" }}>
                    <span className="badge plain">{link.relation}</span>
                    <code style={{ fontSize: 11 }}>{link.sourceAnchorId.slice(0, 8)}…</code>
                    <button className="btn ghost" onClick={() => void detach(link.id, event.id)}>Kaldır</button>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="hint" style={{ margin: "0 0 8px" }}>Bu olaya bağlı kaynak yok.</p>
            )}
            <div className="row" style={{ gap: 6, alignItems: "flex-end" }}>
              <div className="field" style={{ flex: "1 1 260px" }}>
                <label htmlFor={`src-${event.id}`}>Kaynak alıntı</label>
                <select
                  id={`src-${event.id}`}
                  className="input"
                  value={linkAnchor}
                  onChange={(e) => setLinkAnchor(e.target.value)}
                >
                  {anchors.length === 0 ? <option value="">Anchor yok — okuyucuda alıntı oluşturun</option> : null}
                  {anchors.map((anchor) => (
                    <option key={anchor.id} value={anchor.id}>
                      {anchor.exactQuote.slice(0, 60)}
                    </option>
                  ))}
                </select>
              </div>
              <div className="field" style={{ flex: "0 0 160px" }}>
                <label htmlFor={`rel-${event.id}`}>İlişki</label>
                <select
                  id={`rel-${event.id}`}
                  className="input"
                  value={linkRelation}
                  onChange={(e) => setLinkRelation(e.target.value)}
                >
                  <option value="states">belirtir</option>
                  <option value="supports">destekler</option>
                  <option value="contradicts">çelişir</option>
                  <option value="context">bağlam</option>
                </select>
              </div>
              <button className="btn" disabled={!linkAnchor} onClick={() => void attach(event.id)}>Bağla</button>
            </div>
          </div>
        ) : null}
      </div>
    );
  }

  return (
    <div className="page" style={{ maxWidth: 940 }}>
      <div className="page-head">
        <h1>Kronoloji</h1>
        <p className="lead">
          Olay, belge ve tebliğ tarihleri ayrı türlerdir. Ay/yıl veya bilinmeyen tarih desteklenir;
          eksik gün uydurulmaz. Tarihi bilinmeyen olaylar ayrı grupta durur.
        </p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="card">
        <div className="row">
          <div className="field" style={{ flex: "1 1 260px" }}>
            <label htmlFor="tl-case">Dosya</label>
            <select id="tl-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
              {cases.length === 0 ? <option value="">Dosya yok</option> : null}
              {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
            </select>
          </div>
          <div className="field" style={{ flex: "2 1 300px" }}>
            <label htmlFor="tl-title">Olay başlığı</label>
            <input
              id="tl-title"
              className="input"
              value={draft.title}
              onChange={(e) => setDraft({ ...draft, title: e.target.value })}
              placeholder="Örn. Kira ödemesi yapılmadı"
            />
          </div>
        </div>
        <div className="row">
          <div className="field">
            <label htmlFor="tl-date">Tarih</label>
            <input
              id="tl-date"
              className="input"
              type="date"
              value={draft.date}
              disabled={draft.precision === "unknown"}
              onChange={(e) => setDraft({ ...draft, date: e.target.value })}
            />
          </div>
          <div className="field">
            <label htmlFor="tl-precision">Kesinlik</label>
            <select
              id="tl-precision"
              className="input"
              value={draft.precision}
              onChange={(e) => setDraft({ ...draft, precision: e.target.value })}
            >
              {Object.entries(PRECISION_LABEL).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
            </select>
          </div>
          <div className="field">
            <label htmlFor="tl-role">Tarih türü</label>
            <select id="tl-role" className="input" value={draft.role} onChange={(e) => setDraft({ ...draft, role: e.target.value })}>
              {Object.entries(DATE_ROLE_LABEL).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
            </select>
          </div>
          <button className="btn primary" disabled={busy} onClick={() => void add()}>Olay ekle</button>
        </div>
        <p className="hint" style={{ margin: 0 }}>
          Bu kayıtlar kullanıcı beyanıdır; otomatik hukuki süre hesabı yapılmaz (ADR-0008).
        </p>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Tarihli olaylar</h2>
            <p className="card-sub">{known.length} olay · eskiden yeniye</p>
          </div>
        </div>
        {known.length === 0 ? (
          <div className="empty"><p style={{ margin: 0 }}>Henüz tarihli olay yok.</p></div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>{known.map(row)}</div>
        )}
      </div>

      {unknown.length > 0 ? (
        <div className="card">
          <div className="card-head">
            <div>
              <h2>Tarihi bilinmeyenler</h2>
              <p className="card-sub">{unknown.length} olay · sıralamaya sokulmaz</p>
            </div>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>{unknown.map(row)}</div>
        </div>
      ) : null}
    </div>
  );
}
