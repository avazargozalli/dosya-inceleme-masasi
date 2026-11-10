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
import {
  composeDateValue,
  dateDraftIssue,
  dateInputMode,
  editDateInputs,
  eventDateText,
  type DateDraftIssue,
} from "../lib/timeline-date";


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
  /** Aralık kesinliğinde bitiş tarihi; diğerlerinde boş. */
  dateEnd: string;
  /** Metinde geçen özgün tarih ifadesi (sıralamadan bağımsız, korunur). */
  raw: string;
  precision: string;
  role: string;
  kind: string;
  description: string;
};

const EMPTY: Draft = {
  title: "",
  date: "",
  dateEnd: "",
  raw: "",
  precision: "day",
  role: "event",
  kind: "other",
  description: "",
};

/** Gönderilen taslak hâlâ ekranda mı? Yanıt, uçuşta yazılmış yeni taslağı silmez. */
function sameDraft(a: Draft, b: Draft): boolean {
  return (
    a.title === b.title &&
    a.date === b.date &&
    a.dateEnd === b.dateEnd &&
    a.raw === b.raw &&
    a.precision === b.precision &&
    a.role === b.role &&
    a.kind === b.kind &&
    a.description === b.description
  );
}

/** Tarih taslağı hatasının kullanıcıya yazılan metni (`E_EVENT_DATE`). */
function dateIssueText(issue: DateDraftIssue): string {
  switch (issue) {
    case "needDay":
      return t("timeline.dateNeedDay");
    case "needMonth":
      return t("timeline.dateNeedMonth");
    case "needYear":
      return t("timeline.dateNeedYear");
    case "needRange":
      return t("timeline.dateNeedRange");
    case "rangeOrder":
      return t("timeline.dateRangeOrder");
  }
}

/**
 * Kesinliğe göre tarih girişi (F08-T03): ay/yıl kesinliğinde **gün istenmez**,
 * aralık iki uç ister. Ham metin ayrı alandır ve düzenlemede korunur —
 * kaynak ifade (örn. "10 Nisan 2026") sıralama tarihinden bağımsızdır.
 */
function DateFields({
  dateId,
  endId,
  rawId,
  precision,
  date,
  dateEnd,
  raw,
  disabled,
  onDate,
  onDateEnd,
  onRaw,
}: {
  dateId: string;
  endId: string;
  rawId: string;
  precision: string;
  date: string;
  dateEnd: string;
  raw: string;
  disabled: boolean;
  onDate: (value: string) => void;
  onDateEnd: (value: string) => void;
  onRaw: (value: string) => void;
}) {
  const mode = dateInputMode(precision);
  const dateDisabled = disabled || mode === "none";
  return (
    <>
      <div className="field">
        <label htmlFor={dateId}>{t("timeline.fieldDate")}</label>
        {mode === "month" ? (
          <input
            id={dateId}
            className="input"
            type="month"
            value={date}
            disabled={dateDisabled}
            onChange={(e) => onDate(e.target.value)}
          />
        ) : mode === "year" ? (
          <input
            id={dateId}
            className="input"
            inputMode="numeric"
            maxLength={4}
            placeholder="YYYY"
            value={date}
            disabled={dateDisabled}
            onChange={(e) => onDate(e.target.value.replace(/[^0-9]/g, "").slice(0, 4))}
          />
        ) : (
          <input
            id={dateId}
            className="input"
            type="date"
            value={date}
            disabled={dateDisabled}
            onChange={(e) => onDate(e.target.value)}
          />
        )}
        {mode === "month" ? (
          <span className="hint">{t("timeline.dateMonthHint")}</span>
        ) : mode === "range" ? (
          <span className="hint">{t("timeline.dateRangeHint")}</span>
        ) : null}
      </div>
      {mode === "range" ? (
        <div className="field">
          <label htmlFor={endId}>{t("timeline.fieldDateEnd")}</label>
          <input
            id={endId}
            className="input"
            type="date"
            value={dateEnd}
            disabled={disabled}
            onChange={(e) => onDateEnd(e.target.value)}
          />
        </div>
      ) : null}
      <div className="field" style={{ flex: "1 1 220px" }}>
        <label htmlFor={rawId}>{t("timeline.fieldDateRaw")}</label>
        <input
          id={rawId}
          className="input"
          maxLength={120}
          placeholder={t("timeline.fieldDateRawPlaceholder")}
          value={raw}
          disabled={disabled}
          onChange={(e) => onRaw(e.target.value)}
        />
      </div>
    </>
  );
}

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
  // Yazma nesli: yanıt **yalnız kendi** neslinde kilidi bırakır. Dosya değişimi
  // kilidi devreder; devredilen isteğin kapanışı yeni dosyada başlatılan yazmanın
  // kilidini erken açmaz.
  const busySeq = useRef(0);

  function beginWrite(): number {
    busySeq.current += 1;
    setBusy(true);
    return busySeq.current;
  }

  function endWrite(seq: number): void {
    if (busySeq.current === seq) setBusy(false);
  }

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
    // Uçuş kilidi **devredilir** (INV-05): yeni dosyanın düğmeleri/alanları eski
    // isteğin yanıtını beklemez; yanıt hiç gelmezse süresiz kilitli kalırdı.
    busySeq.current += 1;
    setBusy(false);
    // Kaynak paneli nesli de devralınır: eski dosyanın uçuştaki bağlama/kaldırma
    // yanıtı (ve hatası) yeni dosyanın panelinde görünmez.
    linkSeq.current += 1;
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
    // Tarih, **kesinliğe** göre doğrulanır: ay/yıl için gün istenmez, gün için
    // tam tarih. Eksik gün uydurulmaz (F08-T03).
    const issue = dateDraftIssue(draft.precision, draft.date, draft.dateEnd);
    if (issue !== null) {
      setError({ code: "E_EVENT_DATE", message: dateIssueText(issue), retryable: false });
      return;
    }
    // Gönderilen dosya ve taslak tıklama anında donar: uçuşta dosya değişip
    // yeni taslak yazılırsa yanıt **onu** silmez.
    const asked = caseId;
    const submitted = draft;
    const seq = beginWrite();
    try {
      await api.eventCreate(asked, {
        eventKind: submitted.kind,
        dateValue: composeDateValue(submitted.precision, submitted.date, submitted.dateEnd),
        datePrecision: submitted.precision,
        dateRole: submitted.role,
        // Ham metin ayrı alandır: kaynak ifade ("10 Nisan 2026") sıralama
        // tarihinden bağımsızdır ve kullanıcı yazmadıysa uydurulmaz.
        dateRaw: submitted.raw.trim().length > 0 ? submitted.raw.trim() : null,
        title: submitted.title.trim(),
        description: submitted.description.trim(),
        assertionKind: "user",
      });
      setDraft((current) => (sameDraft(current, submitted) ? EMPTY : current));
      await load(asked);
    } catch (e) {
      // Eski dosyanın geç hatası yeni dosyanın ekranına yazılmaz.
      if (caseRef.current === asked) setError(toApiError(e));
    } finally {
      endWrite(seq);
    }
  }

  function startEdit(event: EventRow) {
    const inputs = editDateInputs(event.datePrecision, event.dateValue);
    setEditing({
      id: event.id,
      rowVersion: event.rowVersion,
      draft: {
        title: event.title,
        date: inputs.date,
        dateEnd: inputs.dateEnd,
        // Ham metin **korunur**: yalnız başlığı düzelten kullanıcı kaynak
        // ifadeyi kaybetmez (eski davranış ISO tarihi yazıp eziyordu).
        raw: event.dateRaw ?? "",
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
    const asked = caseId;
    setError(null);
    if (target.draft.title.trim().length === 0) {
      setError({ code: "E_TITLE", message: t("timeline.titleRequired"), retryable: false });
      return;
    }
    const issue = dateDraftIssue(target.draft.precision, target.draft.date, target.draft.dateEnd);
    if (issue !== null) {
      setError({ code: "E_EVENT_DATE", message: dateIssueText(issue), retryable: false });
      return;
    }
    const seq = beginWrite();
    try {
      await api.eventUpdate(target.id, target.rowVersion, {
        eventKind: target.draft.kind,
        dateValue: composeDateValue(target.draft.precision, target.draft.date, target.draft.dateEnd),
        datePrecision: target.draft.precision,
        dateRole: target.draft.role,
        dateRaw: target.draft.raw.trim().length > 0 ? target.draft.raw.trim() : null,
        title: target.draft.title.trim(),
        description: target.draft.description.trim(),
      });
      setEditing((current) => (current && current.id === target.id ? null : current));
      await load(asked);
    } catch (e) {
      const failure = toApiError(e);
      // Yalnız **çakışmada** kayıt sunucuda değişmiştir: panel kapanır ve taze
      // sürüm yüklenir. Geçici hatada kullanıcının yazdığı metin **korunur**
      // (paneli kapatmak sessiz kayıp olurdu), hata görünür kalır.
      if (failure.code === "E_CONFLICT") {
        setEditing((current) => (current && current.id === target.id ? null : current));
      }
      await load(asked);
      if (caseRef.current === asked) setError(failure);
    } finally {
      endWrite(seq);
    }
  }

  async function confirm(id: string) {
    const asked = caseId;
    setError(null);
    const seq = beginWrite();
    try {
      await api.eventConfirm(id);
      await load(asked);
    } catch (e) {
      if (caseRef.current === asked) setError(toApiError(e));
    } finally {
      endWrite(seq);
    }
  }

  async function remove(id: string) {
    const asked = caseId;
    setError(null);
    const seq = beginWrite();
    try {
      await api.eventDelete(id);
      await load(asked);
    } catch (e) {
      if (caseRef.current === asked) setError(toApiError(e));
    } finally {
      endWrite(seq);
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
    const write = beginWrite();
    try {
      await api.eventLinkSource(eventId, linkAnchor, linkRelation);
      const rows = await api.eventSources(eventId);
      if (seq === linkSeq.current) setLinks(rows);
    } catch (e) {
      if (seq === linkSeq.current) setError(toApiError(e));
    } finally {
      endWrite(write);
    }
  }

  async function detach(id: string, eventId: string) {
    const seq = linkSeq.current;
    setError(null);
    const write = beginWrite();
    try {
      await api.eventUnlinkSource(id);
      const rows = await api.eventSources(eventId);
      if (seq === linkSeq.current) setLinks(rows);
    } catch (e) {
      if (seq === linkSeq.current) setError(toApiError(e));
    } finally {
      endWrite(write);
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
              {eventDateText(event) ?? t("dates.missing")} ·{" "}
              {PRECISION_LABEL[event.datePrecision] ?? event.datePrecision}
              {event.dateRaw && event.dateRaw !== (eventDateText(event) ?? "")
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
              <DateFields
                dateId={`edit-date-${event.id}`}
                endId={`edit-date-end-${event.id}`}
                rawId={`edit-date-raw-${event.id}`}
                precision={editing.draft.precision}
                date={editing.draft.date}
                dateEnd={editing.draft.dateEnd}
                raw={editing.draft.raw}
                disabled={busy}
                onDate={(value) =>
                  setEditing({ ...editing, draft: { ...editing.draft, date: value } })
                }
                onDateEnd={(value) =>
                  setEditing({ ...editing, draft: { ...editing.draft, dateEnd: value } })
                }
                onRaw={(value) =>
                  setEditing({ ...editing, draft: { ...editing.draft, raw: value } })
                }
              />
              <div className="field">
                <label htmlFor={`edit-precision-${event.id}`}>{t("timeline.fieldPrecision")}</label>
                <select
                  id={`edit-precision-${event.id}`}
                  className="input"
                  value={editing.draft.precision}
                  disabled={busy}
                  onChange={(e) => {
                    const precision = e.target.value;
                    const inputs = editDateInputs(
                      precision,
                      editing.draft.date || editing.draft.dateEnd,
                    );
                    setEditing({
                      ...editing,
                      draft: { ...editing.draft, precision, ...inputs },
                    });
                  }}
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
          <DateFields
            dateId="tl-date"
            endId="tl-date-end"
            rawId="tl-date-raw"
            precision={draft.precision}
            date={draft.date}
            dateEnd={draft.dateEnd}
            raw={draft.raw}
            disabled={busy}
            onDate={(value) => setDraft({ ...draft, date: value })}
            onDateEnd={(value) => setDraft({ ...draft, dateEnd: value })}
            onRaw={(value) => setDraft({ ...draft, raw: value })}
          />
          <div className="field">
            <label htmlFor="tl-precision">{t("timeline.fieldPrecision")}</label>
            <select
              id="tl-precision"
              className="input"
              value={draft.precision}
              disabled={busy}
              onChange={(e) => {
                // Kesinlik değişince tarih alanı yeni çözünürlüğe kırpılır:
                // "2026-04-10" + Ay → "2026-04" (gün saklanmaz, F08-T03).
                const precision = e.target.value;
                const inputs = editDateInputs(precision, draft.date);
                setDraft({ ...draft, precision, date: inputs.date, dateEnd: inputs.dateEnd });
              }}
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
