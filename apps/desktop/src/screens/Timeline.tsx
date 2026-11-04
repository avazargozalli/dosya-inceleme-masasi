import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  api,
  toApiError,
  type AnchorRow,
  type ApiError,
  type CaseRow,
  type EventRow,
  type EventSourceRow,
} from "../lib/api";
import { t } from "../lib/i18n";


const DATE_ROLE_LABEL: Record<string, string> = {
  event: t("timeline.role.event"),
  document: t("timeline.role.document"),
  notification: t("timeline.role.notification"),
  contract: t("timeline.role.contract"),
  hearing: t("timeline.role.hearing"),
  deadline_candidate: t("timeline.role.deadline_candidate"),
  unknown: t("timeline.role.unknown"),
};

const PRECISION_LABEL: Record<string, string> = {
  day: t("timeline.precision.day"),
  month: t("timeline.precision.month"),
  year: t("timeline.precision.year"),
  range: t("timeline.precision.range"),
  approximate: t("timeline.precision.approximate"),
  unknown: t("timeline.precision.unknown"),
};

// Bağlantı rozeti ile seçim aynı kaynaktan: ham anahtar ("contradicts") kullanıcıya
// gösterilmez; sözlükte karşılığı olmayan değer dürüstçe ham yazılır.
const RELATION_LABEL: Record<string, string> = {
  states: t("timeline.relationKind.states"),
  supports: t("timeline.relationKind.supports"),
  contradicts: t("timeline.relationKind.contradicts"),
  context: t("timeline.relationKind.context"),
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
  // Düzenleme, kaydın okunduğu sürümü taşır: eski sürümle yazma reddedilir.
  const [editing, setEditing] = useState<{ id: string; rowVersion: number; draft: Draft } | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  // Seçili dosya ve kaynak paneli istek sürerken değişebilir; yanıtlar **o anda
  // ekranda duran** bağlama yazılır. Panel kimliği değil, nesil sayacı: aynı olay
  // paneli kapatılıp yeniden açılsa bile eski isteğin geç yanıtı yenisini ezmez.
  const caseRef = useRef("");
  const linkSeq = useRef(0);

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
    // Yazma sonrası tazeleme eski dosyanın kimliğiyle çağrılabilir (uçuşta dosya
    // değiştiyse): o istek hiç gönderilmez, yeni dosyanın listesi ezilmez.
    if (!id || id !== caseRef.current) return;
    try {
      const rows = await api.eventList(id);
      if (id === caseRef.current) setEvents(rows);
    } catch (e) {
      if (id === caseRef.current) setError(toApiError(e));
    }
  }, []);

  useEffect(() => {
    if (!caseId) return;
    caseRef.current = caseId;
    // Dosya değişince türetilmiş durum **istekten önce** temizlenir (INV-05):
    // önceki dosyanın olayları, açık düzenleme/kaynak paneli ve taşınan çapa
    // yeni dosyada görünmez; "Kaydet"/"Bağla" eski dosyanın kaydına yazamaz.
    setEvents([]);
    setEditing(null);
    setLinkFor(null);
    setAnchors([]);
    setLinks([]);
    setLinkAnchor("");
    setError(null);
    void load(caseId);
  }, [caseId, load]);

  const known = useMemo(() => events.filter((e) => e.sortableDate), [events]);
  const unknown = useMemo(() => events.filter((e) => !e.sortableDate), [events]);

  async function add() {
    setError(null);
    if (!caseId || draft.title.trim().length === 0) {
      setError({ code: "E_TITLE", message: t("timeline.titleRequired"), retryable: false });
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

  function startEdit(event: EventRow) {
    setEditing({
      id: event.id,
      rowVersion: event.rowVersion,
      draft: {
        title: event.title,
        date: event.dateValue ?? "",
        precision: event.datePrecision,
        role: event.dateRole,
        kind: event.eventKind,
        description: event.description,
      },
    });
  }

  async function saveEdit() {
    if (!editing) return;
    // Gönderilen kayıt ve değerler tıklama anında donar; uçuşta başka satırın
    // düzenleyicisi açılırsa yanıt **onu** kapatmaz.
    const target = editing;
    setError(null);
    if (target.draft.title.trim().length === 0) {
      setError({ code: "E_TITLE", message: t("timeline.titleRequired"), retryable: false });
      return;
    }
    setBusy(true);
    try {
      await api.eventUpdate(target.id, target.rowVersion, {
        eventKind: target.draft.kind,
        dateValue: target.draft.precision === "unknown" ? null : target.draft.date || null,
        datePrecision: target.draft.precision,
        dateRole: target.draft.role,
        dateRaw: target.draft.date || null,
        title: target.draft.title.trim(),
        description: target.draft.description.trim(),
      });
      setEditing((current) => (current && current.id === target.id ? null : current));
      await load(caseId);
    } catch (e) {
      // Çatışmada kayıt ezilmedi: taze sürümü yükle, kullanıcı yeniden düzenlesin.
      setEditing((current) => (current && current.id === target.id ? null : current));
      await load(caseId);
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function confirm(id: string) {
    setError(null);
    setBusy(true);
    try {
      await api.eventConfirm(id);
      await load(caseId);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function remove(id: string) {
    setError(null);
    setBusy(true);
    try {
      await api.eventDelete(id);
      await load(caseId);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function openLink(eventId: string) {
    if (linkFor === eventId) {
      setLinkFor(null);
      return;
    }
    const seq = ++linkSeq.current;
    setLinkFor(eventId);
    // Panel açılırken önceki olayın bağlantıları/çapası taşınmaz: düşen istek
    // eski kaynağı yeni olayın kaynağı gibi göstermez, "Bağla" eski çapayla açılmaz.
    setAnchors([]);
    setLinks([]);
    setLinkAnchor("");
    setError(null);
    try {
      const [anchorRows, linkRows] = await Promise.all([
        api.anchorList(caseId),
        api.eventSources(eventId),
      ]);
      // Yanıt yalnız panel hâlâ aynı açılışta ise yazılır: geç yanıt sonradan
      // açılan (ya da yeniden açılan) paneli ezmez.
      if (seq !== linkSeq.current) return;
      setAnchors(anchorRows);
      setLinks(linkRows);
      setLinkAnchor(anchorRows[0]?.id ?? "");
    } catch (e) {
      if (seq === linkSeq.current) setError(toApiError(e));
    }
  }

  async function attach(eventId: string) {
    if (!linkAnchor) return;
    const seq = linkSeq.current;
    setError(null);
    setBusy(true);
    try {
      await api.eventLinkSource(eventId, linkAnchor, linkRelation);
      const rows = await api.eventSources(eventId);
      if (seq === linkSeq.current) setLinks(rows);
    } catch (e) {
      if (seq === linkSeq.current) setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function detach(id: string, eventId: string) {
    const seq = linkSeq.current;
    setError(null);
    setBusy(true);
    try {
      await api.eventUnlinkSource(id);
      const rows = await api.eventSources(eventId);
      if (seq === linkSeq.current) setLinks(rows);
    } catch (e) {
      if (seq === linkSeq.current) setError(toApiError(e));
    } finally {
      setBusy(false);
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
                {event.status === "confirmed" ? t("timeline.confirmed") : t("timeline.draft")}
              </span>
            </div>
            <div className="muted small" style={{ marginTop: 4 }}>
              {event.sortableDate ?? t("dates.missing")} ·{" "}
              {PRECISION_LABEL[event.datePrecision] ?? event.datePrecision}
              {event.dateRaw && event.dateRaw !== event.sortableDate
                ? ` · ${t("timeline.rawNote", { raw: event.dateRaw })}`
                : ""}
            </div>
            {event.description ? <p style={{ margin: "6px 0 0" }}>{event.description}</p> : null}
          </div>
          <div style={{ display: "flex", gap: 6, flexShrink: 0 }}>
            {event.status === "draft" ? (
              <button className="btn ghost" disabled={busy} onClick={() => void confirm(event.id)}>
                {t("timeline.confirm")}
              </button>
            ) : null}
            <button
              className="btn ghost"
              onClick={() => (editing?.id === event.id ? setEditing(null) : startEdit(event))}
            >
              {editing?.id === event.id ? t("timeline.closeEdit") : t("timeline.edit")}
            </button>
            <button className="btn ghost" onClick={() => void openLink(event.id)}>
              {linkFor === event.id ? t("timeline.closeSources") : t("timeline.sources")}
            </button>
            <button className="btn ghost" disabled={busy} onClick={() => void remove(event.id)}>
              {t("timeline.remove")}
            </button>
          </div>
        </div>

        {editing?.id === event.id ? (
          <div style={{ marginTop: 10, borderTop: "1px solid var(--border)", paddingTop: 10 }}>
            <div className="row">
              <div className="field" style={{ flex: "2 1 240px" }}>
                <label htmlFor={`edit-title-${event.id}`}>{t("timeline.fieldTitle")}</label>
                <input
                  id={`edit-title-${event.id}`}
                  className="input"
                  value={editing.draft.title}
                  disabled={busy}
                  onChange={(e) =>
                    setEditing({ ...editing, draft: { ...editing.draft, title: e.target.value } })
                  }
                />
              </div>
              <div className="field">
                <label htmlFor={`edit-date-${event.id}`}>{t("timeline.fieldDate")}</label>
                <input
                  id={`edit-date-${event.id}`}
                  className="input"
                  type="date"
                  value={editing.draft.date}
                  disabled={busy || editing.draft.precision === "unknown"}
                  onChange={(e) =>
                    setEditing({ ...editing, draft: { ...editing.draft, date: e.target.value } })
                  }
                />
              </div>
              <div className="field">
                <label htmlFor={`edit-precision-${event.id}`}>{t("timeline.fieldPrecision")}</label>
                <select
                  id={`edit-precision-${event.id}`}
                  className="input"
                  value={editing.draft.precision}
                  disabled={busy}
                  onChange={(e) =>
                    setEditing({ ...editing, draft: { ...editing.draft, precision: e.target.value } })
                  }
                >
                  {Object.entries(PRECISION_LABEL).map(([key, label]) => (
                    <option key={key} value={key}>
                      {label}
                    </option>
                  ))}
                </select>
              </div>
              <div className="field">
                <label htmlFor={`edit-role-${event.id}`}>{t("timeline.fieldRole")}</label>
                <select
                  id={`edit-role-${event.id}`}
                  className="input"
                  value={editing.draft.role}
                  disabled={busy}
                  onChange={(e) =>
                    setEditing({ ...editing, draft: { ...editing.draft, role: e.target.value } })
                  }
                >
                  {Object.entries(DATE_ROLE_LABEL).map(([key, label]) => (
                    <option key={key} value={key}>
                      {label}
                    </option>
                  ))}
                </select>
              </div>
            </div>
            <div className="row">
              <div className="field" style={{ flex: "1 1 320px" }}>
                <label htmlFor={`edit-desc-${event.id}`}>{t("timeline.fieldDescription")}</label>
                <input
                  id={`edit-desc-${event.id}`}
                  className="input"
                  value={editing.draft.description}
                  disabled={busy}
                  onChange={(e) =>
                    setEditing({
                      ...editing,
                      draft: { ...editing.draft, description: e.target.value },
                    })
                  }
                />
              </div>
              <button className="btn primary" disabled={busy} onClick={() => void saveEdit()}>
                {t("common.save")}
              </button>
              <button
                className="btn ghost"
                onClick={() => {
                  setEditing(null);
                  setError(null);
                }}
              >
                {t("timeline.cancel")}
              </button>
            </div>
            <p className="hint" style={{ margin: 0 }}>
              {t("timeline.versionHint", { version: editing.rowVersion })}
            </p>
          </div>
        ) : null}

        {linkFor === event.id ? (
          <div style={{ marginTop: 10, borderTop: "1px solid var(--border)", paddingTop: 10 }}>
            {links.length > 0 ? (
              <ul style={{ listStyle: "none", padding: 0, margin: "0 0 8px", display: "grid", gap: 4 }}>
                {links.map((link) => (
                  <li key={link.id} className="muted small" style={{ display: "flex", gap: 8, alignItems: "center" }}>
                    <span className="badge plain">
                      {RELATION_LABEL[link.relation] ?? link.relation}
                    </span>
                    <code style={{ fontSize: 11 }}>{link.sourceAnchorId.slice(0, 8)}…</code>
                    <button className="btn ghost" disabled={busy} onClick={() => void detach(link.id, event.id)}>
                      {t("timeline.removeLink")}
                    </button>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="hint" style={{ margin: "0 0 8px" }}>{t("timeline.noSources")}</p>
            )}
            <div className="row" style={{ gap: 6, alignItems: "flex-end" }}>
              <div className="field" style={{ flex: "1 1 260px" }}>
                <label htmlFor={`src-${event.id}`}>{t("timeline.sourceQuote")}</label>
                <select
                  id={`src-${event.id}`}
                  className="input"
                  value={linkAnchor}
                  onChange={(e) => setLinkAnchor(e.target.value)}
                >
                  {anchors.length === 0 ? <option value="">{t("timeline.noAnchors")}</option> : null}
                  {anchors.map((anchor) => (
                    <option key={anchor.id} value={anchor.id}>
                      {anchor.exactQuote.slice(0, 60)}
                    </option>
                  ))}
                </select>
              </div>
              <div className="field" style={{ flex: "0 0 160px" }}>
                <label htmlFor={`rel-${event.id}`}>{t("timeline.relation")}</label>
                <select
                  id={`rel-${event.id}`}
                  className="input"
                  value={linkRelation}
                  onChange={(e) => setLinkRelation(e.target.value)}
                >
                  {Object.entries(RELATION_LABEL).map(([key, label]) => (
                    <option key={key} value={key}>
                      {label}
                    </option>
                  ))}
                </select>
              </div>
              <button className="btn" disabled={!linkAnchor || busy} onClick={() => void attach(event.id)}>
                {t("timeline.attach")}
              </button>
            </div>
          </div>
        ) : null}
      </div>
    );
  }

  return (
    <div className="page" style={{ maxWidth: 940 }}>
      <div className="page-head">
        <h1>{t("nav.timeline")}</h1>
        <p className="lead">{t("timeline.lead")}</p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="card">
        <div className="row">
          <div className="field" style={{ flex: "1 1 260px" }}>
            <label htmlFor="tl-case">{t("common.case")}</label>
            <select id="tl-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
              {cases.length === 0 ? <option value="">{t("common.noCase")}</option> : null}
              {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
            </select>
          </div>
          <div className="field" style={{ flex: "2 1 300px" }}>
            <label htmlFor="tl-title">{t("timeline.eventTitle")}</label>
            <input
              id="tl-title"
              className="input"
              value={draft.title}
              disabled={busy}
              onChange={(e) => setDraft({ ...draft, title: e.target.value })}
              placeholder={t("timeline.eventTitlePlaceholder")}
            />
          </div>
        </div>
        <div className="row">
          <div className="field">
            <label htmlFor="tl-date">{t("timeline.fieldDate")}</label>
            <input
              id="tl-date"
              className="input"
              type="date"
              value={draft.date}
              disabled={busy || draft.precision === "unknown"}
              onChange={(e) => setDraft({ ...draft, date: e.target.value })}
            />
          </div>
          <div className="field">
            <label htmlFor="tl-precision">{t("timeline.fieldPrecision")}</label>
            <select
              id="tl-precision"
              className="input"
              value={draft.precision}
              disabled={busy}
              onChange={(e) => setDraft({ ...draft, precision: e.target.value })}
            >
              {Object.entries(PRECISION_LABEL).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
            </select>
          </div>
          <div className="field">
            <label htmlFor="tl-role">{t("timeline.fieldRole")}</label>
            <select id="tl-role" className="input" value={draft.role} disabled={busy} onChange={(e) => setDraft({ ...draft, role: e.target.value })}>
              {Object.entries(DATE_ROLE_LABEL).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
            </select>
          </div>
          <button className="btn primary" disabled={busy} onClick={() => void add()}>
            {t("timeline.addEvent")}
          </button>
        </div>
        <p className="hint" style={{ margin: 0 }}>
          {t("timeline.userAssertionHint")}
        </p>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("timeline.knownTitle")}</h2>
            <p className="card-sub">{t("timeline.knownSub", { count: known.length })}</p>
          </div>
        </div>
        {known.length === 0 ? (
          <div className="empty"><p style={{ margin: 0 }}>{t("timeline.knownEmpty")}</p></div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>{known.map(row)}</div>
        )}
      </div>

      {unknown.length > 0 ? (
        <div className="card">
          <div className="card-head">
            <div>
              <h2>{t("timeline.unknownTitle")}</h2>
              <p className="card-sub">{t("timeline.unknownSub", { count: unknown.length })}</p>
            </div>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>{unknown.map(row)}</div>
        </div>
      ) : null}
    </div>
  );
}
