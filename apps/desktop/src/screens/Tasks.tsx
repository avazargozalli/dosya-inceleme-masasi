import { useCallback, useEffect, useRef, useState } from "react";
import {
  api,
  toApiError,
  type ApiError,
  type CaseRow,
  type ReminderRow,
  type TaskRow,
} from "../lib/api";
import { t } from "../lib/i18n";
import { localIsoDay, REMINDER_POLICY_LABEL, REMINDER_TEXT, reminderLabel } from "../lib/reminders";

export function Tasks() {
  const [cases, setCases] = useState<CaseRow[]>([]);
  const [caseId, setCaseId] = useState("");
  const [rows, setRows] = useState<TaskRow[]>([]);
  const [includeDone, setIncludeDone] = useState(false);
  const [title, setTitle] = useState("");
  const [due, setDue] = useState("");
  const [policy, setPolicy] = useState("none");
  const [reminders, setReminders] = useState<ReminderRow[]>([]);
  const [reminderNote, setReminderNote] = useState("");
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  /** Askıdaki liste yanıtlarının sırası: geç gelen **eski** dosya yanıtı
   *  yeni dosyanın listesini ezmemeli (bkz. `load`). */
  const loadSeq = useRef(0);
  /** Uçuştaki yazmanın kilidini devretmek için nesil sayacı: dosya değişince
   *  yeni dosya eski isteği beklemez, eski kapanış yeni kilidi açamaz. */
  const busySeq = useRef(0);
  /** Yanıt anında dosya/süzgeç karşılaştırması; render değeri ref'e yazılır ki
   *  effect'i beklemek arada gelen yanıtı eski dosyaya yazmasın. */
  const caseRef = useRef(caseId);
  caseRef.current = caseId;
  const doneRef = useRef(includeDone);
  doneRef.current = includeDone;

  useEffect(() => {
    (async () => {
      try {
        const list = await api.caseList(false);
        setCases(list);
        if (list.length > 0) setCaseId((prev) => prev || list[0]!.id);
      } catch (e) {
        setError(toApiError(e));
      }
    })();
  }, []);

  const load = useCallback(async (id: string, done: boolean) => {
    if (!id) return;
    const seq = (loadSeq.current += 1);
    try {
      const [taskRows, reminderRows] = await Promise.all([
        api.taskList(id, done),
        api.taskReminders(id, localIsoDay()),
      ]);
      if (seq !== loadSeq.current) return;
      setRows(taskRows);
      setReminders(reminderRows);
    } catch (e) {
      if (seq !== loadSeq.current) return;
      // Düşen yükleme **önceki** listeyi güncel gibi bırakmaz.
      setRows([]);
      setReminders([]);
      setError(toApiError(e));
    }
  }, []);

  useEffect(() => {
    // Dosya ya da süzgeç değişince önceki dosyanın görevleri, hatırlatıcıları ve
    // bildirimi ekranda kalmamalı; istek askıdaysa eski veri yeni dosyanın
    // altında görünürdü (INV-05).
    // Askıdaki eski yanıtları geçersiz kıl.
    loadSeq.current += 1;
    // Uçuştaki yazmanın kilidi devredilir: yeni dosyanın düğmeleri eski isteğin
    // dönmesini beklemez. Eski yanıt `caseRef` kapısından yazmaz; eski kapanış
    // kendi nesli (`busySeq`) yüzünden yeni kilidi açamaz.
    setBusy(false);
    setRows([]);
    setReminders([]);
    setReminderNote("");
    setError(null);
    if (caseId) void load(caseId, includeDone);
  }, [caseId, includeDone, load]);

  async function add() {
    // Kilit yalnız düğmede değil işleyicide de tutulur: Enter yolu düğmeyi atlar.
    if (busy) return;
    setError(null);
    if (!caseId) {
      // Eksik olan başlık değil dosyadır: gerekçe doğru yazılır.
      setError({ code: "E_NO_CASE", message: t("tasks.caseRequired"), retryable: false });
      return;
    }
    if (title.trim().length === 0) {
      setError({ code: "E_TITLE", message: t("tasks.titleRequired"), retryable: false });
      return;
    }
    const target = caseId;
    const submitted = { title: title.trim(), due, policy };
    const seq = ++busySeq.current;
    setBusy(true);
    try {
      await api.taskCreate(target, {
        title: submitted.title,
        userDueDate: submitted.due || null,
        dateTimezone: "Europe/Istanbul",
        assigneeLabel: null,
        // Tarih yoksa hatırlatıcı kurulamaz; seçim tarih silinince "none"a döner
        // (aşağıdaki tarih alanı) — tarihsiz göreve politika taşınmaz.
        reminderPolicy: submitted.policy,
      });
      // Yanıt, isteğin başladığı dosya artık seçili değilse hiçbir yere yazılmaz.
      if (caseRef.current !== target) return;
      // Form yalnız **gönderilen** değerler hâlâ ekrandaysa temizlenir: yanıt
      // beklerken yazılan yeni görev taslağı yanıtla birlikte silinmez.
      setTitle((prev) => (prev.trim() === submitted.title ? "" : prev));
      setDue((prev) => (prev === submitted.due ? "" : prev));
      setPolicy((prev) => (prev === submitted.policy ? "none" : prev));
      // Tazeleme o anki süzgeçle koşar: uçuşta "Tamamlananlar" değiştiyse eski
      // süzgeç listeyi ezmez.
      await load(target, doneRef.current);
    } catch (e) {
      if (caseRef.current !== target) return;
      setError(toApiError(e));
    } finally {
      if (seq === busySeq.current) setBusy(false);
    }
  }

  /** Tarih yazmak onay değildir: kullanıcı ayrıca onaylar (F08-T10). */
  async function confirmDate(id: string) {
    const target = caseId;
    const seq = ++busySeq.current;
    setError(null);
    setBusy(true);
    try {
      await api.taskConfirmDate(id);
      if (caseRef.current !== target) return;
      await load(target, doneRef.current);
    } catch (e) {
      if (caseRef.current !== target) return;
      setError(toApiError(e));
    } finally {
      if (seq === busySeq.current) setBusy(false);
    }
  }

  async function enqueueReminders() {
    const target = caseId;
    const seq = ++busySeq.current;
    setError(null);
    setReminderNote("");
    setBusy(true);
    try {
      const result = await api.taskRemindersEnqueue(target, localIsoDay());
      if (caseRef.current !== target) return;
      setReminderNote(
        result.queued === 0
          ? t("tasks.noNewReminders")
          : t("tasks.remindersQueued", { queued: result.queued }),
      );
      await load(target, doneRef.current);
    } catch (e) {
      if (caseRef.current !== target) return;
      setError(toApiError(e));
    } finally {
      if (seq === busySeq.current) setBusy(false);
    }
  }

  async function sendReminders() {
    const target = caseId;
    const seq = ++busySeq.current;
    setError(null);
    setReminderNote("");
    setBusy(true);
    try {
      // Gerçek teslim: OS bildirimi kabul etmezse çekirdek satırı kapatmaz
      // (`failed` artar, satır kuyrukta kalır). İzin yoksa hata kodu döner ve
      // "gönderildi" yazılmaz.
      const result = await api.notificationsDeliver(target);
      if (caseRef.current !== target) return;
      setReminderNote(
        result.failed > 0
          ? t("tasks.notifyPartial", {
              sent: result.sent,
              failed: result.failed,
              remaining: result.remaining,
            })
          : result.sent === 0
            ? t("tasks.notifyNone", { remaining: result.remaining })
            : t("tasks.notifySent", {
                sent: result.sent,
                remaining: result.remaining,
              }),
      );
    } catch (e) {
      if (caseRef.current !== target) return;
      setError(toApiError(e));
    } finally {
      if (seq === busySeq.current) setBusy(false);
    }
  }

  async function complete(id: string) {
    const target = caseId;
    const seq = ++busySeq.current;
    setError(null);
    setBusy(true);
    try {
      await api.taskComplete(id);
      if (caseRef.current !== target) return;
      await load(target, doneRef.current);
    } catch (e) {
      if (caseRef.current !== target) return;
      setError(toApiError(e));
    } finally {
      if (seq === busySeq.current) setBusy(false);
    }
  }

  async function remove(id: string) {
    const target = caseId;
    const seq = ++busySeq.current;
    setError(null);
    setBusy(true);
    try {
      await api.taskDelete(id);
      if (caseRef.current !== target) return;
      await load(target, doneRef.current);
    } catch (e) {
      if (caseRef.current !== target) return;
      setError(toApiError(e));
    } finally {
      if (seq === busySeq.current) setBusy(false);
    }
  }

  return (
    <div className="page" style={{ maxWidth: 900 }}>
      <div className="page-head">
        <h1>{t("nav.tasks")}</h1>
        <p className="lead">{t("tasks.lead")}</p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="card">
        <div className="row">
          <div className="field" style={{ flex: "1 1 240px" }}>
            <label htmlFor="task-case">{t("common.case")}</label>
            <select id="task-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
              {cases.length === 0 ? <option value="">{t("common.noCase")}</option> : null}
              {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
            </select>
          </div>
          <div className="field" style={{ flex: "2 1 280px" }}>
            <label htmlFor="task-title">{t("tasks.taskLabel")}</label>
            <input
              id="task-title"
              className="input"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void add()}
              placeholder={t("tasks.titlePlaceholder")}
            />
          </div>
          <div className="field" style={{ flex: "0 1 170px" }}>
            <label htmlFor="task-due">{t("tasks.dueLabel")}</label>
            <input id="task-due" className="input" type="date" value={due} onChange={(e) => {
              setDue(e.target.value);
              // Tarih silinince hatırlatma seçimi anlamsız kalır; değeri
              // "none"a döner ve tarihsiz göreve politika taşınmaz.
              if (!e.target.value) setPolicy("none");
            }} />
          </div>
          <div className="field" style={{ flex: "0 1 170px" }}>
            <label htmlFor="task-policy">{t("tasks.reminderLabel")}</label>
            <select
              id="task-policy"
              className="input"
              value={policy}
              onChange={(e) => setPolicy(e.target.value)}
              disabled={!due}
            >
              {Object.entries(REMINDER_POLICY_LABEL).map(([k, v]) => (
                <option key={k} value={k}>{v}</option>
              ))}
            </select>
          </div>
          <button className="btn primary" disabled={busy} onClick={() => void add()}>{t("tasks.add")}</button>
        </div>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("tasks.listTitle")}</h2>
            <p className="card-sub">{t("tasks.recordCount", { count: rows.length })}</p>
          </div>
          <label className="row" style={{ alignItems: "center", gap: 6 }}>
            <input
              type="checkbox"
              checked={includeDone}
              onChange={(e) => setIncludeDone(e.target.checked)}
              style={{ width: 15, height: 15 }}
            />
            <span className="small muted">{t("tasks.includeDone")}</span>
          </label>
        </div>

        {rows.length === 0 ? (
          <div className="empty"><p style={{ margin: 0 }}>{t("tasks.empty")}</p></div>
        ) : (
          <div style={{ border: "1px solid var(--line)", borderRadius: "var(--radius)" }}>
            {rows.map((task, index) => (
              <div
                key={task.id}
                style={{
                  display: "grid",
                  gridTemplateColumns: "1fr 150px 120px 90px",
                  gap: "var(--space-3)",
                  alignItems: "center",
                  padding: "10px var(--space-3)",
                  borderTop: index === 0 ? 0 : "1px solid var(--line)",
                }}
              >
                <span style={{ textDecoration: task.status === "done" ? "line-through" : "none" }}>
                  {task.title}
                </span>
                <span className="muted small">
                  {task.userDueDate ?? t("tasks.noDate")}
                  {task.userDueDate ? (
                    <span className={`badge ${task.dateConfirmedAt ? "ok" : "warn"}`} style={{ marginLeft: 6 }}>
                      {task.dateConfirmedAt ? t("tasks.confirmed") : t("tasks.pendingConfirm")}
                    </span>
                  ) : null}
                </span>
                <span className={`badge ${task.status === "done" ? "ok" : "plain"}`}>
                  {task.status === "done" ? t("tasks.done") : t("tasks.open")}
                </span>
                <span style={{ display: "flex", gap: 4, justifyContent: "flex-end" }}>
                  {task.status === "open" && task.userDueDate && !task.dateConfirmedAt ? (
                    <button className="btn ghost" disabled={busy} onClick={() => void confirmDate(task.id)}>
                      {t("tasks.confirmDate")}
                    </button>
                  ) : null}
                  {task.status === "open" ? (
                    <button className="btn ghost" disabled={busy} onClick={() => void complete(task.id)}>{t("tasks.complete")}</button>
                  ) : null}
                  <button className="btn ghost" disabled={busy} onClick={() => void remove(task.id)}>{t("tasks.remove")}</button>
                </span>
              </div>
            ))}
          </div>
        )}
        <p className="hint" style={{ margin: 0 }}>
          {t("tasks.reminderHint")}
        </p>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("tasks.remindersTitle")}</h2>
            <p className="card-sub">
              {t("tasks.remindersSub", { count: reminders.length, day: localIsoDay() })}
            </p>
          </div>
          <div className="row" style={{ gap: 6 }}>
            <button
              className="btn"
              disabled={busy || !caseId}
              onClick={() => void enqueueReminders()}
            >
              {t("tasks.enqueue")}
            </button>
            <button
              className="btn"
              disabled={busy || !caseId}
              onClick={() => void sendReminders()}
            >
              {t("tasks.notify")}
            </button>
          </div>
        </div>

        {reminders.length === 0 ? (
          <div className="empty">
            <p style={{ margin: 0 }}>{t("tasks.noReminders")}</p>
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {reminders.map((r) => {
              const task = rows.find((t) => t.id === r.taskId);
              return (
                <div
                  key={`${r.taskId}-${r.policy}`}
                  className="row"
                  style={{
                    justifyContent: "space-between",
                    border: "1px solid var(--line)",
                    borderRadius: "var(--radius)",
                    padding: "6px 8px",
                  }}
                >
                  <span className="small">{REMINDER_TEXT}</span>
                  <span className="muted small">
                    {reminderLabel(r.policy)} · {r.dueDate} · {task?.title ?? t("tasks.taskFallback")}
                  </span>
                </div>
              );
            })}
          </div>
        )}
        {reminderNote ? <p className="hint" style={{ margin: 0 }}>{reminderNote}</p> : null}
        <p className="hint" style={{ margin: 0 }}>
          {t("tasks.privacyHint")}
        </p>
      </div>
    </div>
  );
}
