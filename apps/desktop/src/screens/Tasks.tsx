import { useCallback, useEffect, useState } from "react";
import {
  api,
  toApiError,
  type ApiError,
  type CaseRow,
  type ReminderRow,
  type TaskRow,
} from "../lib/api";
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
    try {
      const [taskRows, reminderRows] = await Promise.all([
        api.taskList(id, done),
        api.taskReminders(id, localIsoDay()),
      ]);
      setRows(taskRows);
      setReminders(reminderRows);
    } catch (e) {
      setError(toApiError(e));
    }
  }, []);

  useEffect(() => {
    if (caseId) void load(caseId, includeDone);
  }, [caseId, includeDone, load]);

  async function add() {
    if (!caseId || title.trim().length === 0) {
      setError({ code: "E_TITLE", message: "Görev başlığı zorunlu.", retryable: false });
      return;
    }
    setBusy(true);
    try {
      await api.taskCreate(caseId, {
        title: title.trim(),
        userDueDate: due || null,
        dateTimezone: "Europe/Istanbul",
        assigneeLabel: null,
        reminderPolicy: policy,
      });
      setTitle("");
      setDue("");
      setPolicy("none");
      await load(caseId, includeDone);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  /** Tarih yazmak onay değildir: kullanıcı ayrıca onaylar (F08-T10). */
  async function confirmDate(id: string) {
    setBusy(true);
    try {
      await api.taskConfirmDate(id);
      await load(caseId, includeDone);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function enqueueReminders() {
    setBusy(true);
    try {
      const result = await api.taskRemindersEnqueue(caseId, localIsoDay());
      setReminderNote(
        result.queued === 0
          ? "Yeni hatırlatıcı yok (bugün için zaten kuyrukta)."
          : `${result.queued} hatırlatıcı yerel bildirim kuyruğuna yazıldı.`,
      );
      await load(caseId, includeDone);
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  }

  async function complete(id: string) {
    try {
      await api.taskComplete(id);
      await load(caseId, includeDone);
    } catch (e) {
      setError(toApiError(e));
    }
  }

  async function remove(id: string) {
    try {
      await api.taskDelete(id);
      await load(caseId, includeDone);
    } catch (e) {
      setError(toApiError(e));
    }
  }

  return (
    <div className="page" style={{ maxWidth: 900 }}>
      <div className="page-head">
        <h1>Görevler</h1>
        <p className="lead">
          Tarihi kullanıcı girer ve doğrular. Uygulama hukuki son gün hesaplamaz; tarih kaynağı
          “kullanıcı girdisi” olarak işaretlenir.
        </p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      <div className="card">
        <div className="row">
          <div className="field" style={{ flex: "1 1 240px" }}>
            <label htmlFor="task-case">Dosya</label>
            <select id="task-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
              {cases.length === 0 ? <option value="">Dosya yok</option> : null}
              {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
            </select>
          </div>
          <div className="field" style={{ flex: "2 1 280px" }}>
            <label htmlFor="task-title">Görev</label>
            <input
              id="task-title"
              className="input"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void add()}
              placeholder="Örn. Tebliğ tarihini kontrol et"
            />
          </div>
          <div className="field" style={{ flex: "0 1 170px" }}>
            <label htmlFor="task-due">Tarih (isteğe bağlı)</label>
            <input id="task-due" className="input" type="date" value={due} onChange={(e) => setDue(e.target.value)} />
          </div>
          <div className="field" style={{ flex: "0 1 170px" }}>
            <label htmlFor="task-policy">Hatırlatma</label>
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
          <button className="btn primary" disabled={busy} onClick={() => void add()}>Ekle</button>
        </div>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Görev listesi</h2>
            <p className="card-sub">{rows.length} kayıt</p>
          </div>
          <label className="row" style={{ alignItems: "center", gap: 6 }}>
            <input
              type="checkbox"
              checked={includeDone}
              onChange={(e) => setIncludeDone(e.target.checked)}
              style={{ width: 15, height: 15 }}
            />
            <span className="small muted">Tamamlananlar</span>
          </label>
        </div>

        {rows.length === 0 ? (
          <div className="empty"><p style={{ margin: 0 }}>Görev yok.</p></div>
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
                  {task.userDueDate ?? "tarih yok"}
                  {task.userDueDate ? (
                    <span className={`badge ${task.dateConfirmedAt ? "ok" : "warn"}`} style={{ marginLeft: 6 }}>
                      {task.dateConfirmedAt ? "onaylı" : "onay bekliyor"}
                    </span>
                  ) : null}
                </span>
                <span className={`badge ${task.status === "done" ? "ok" : "plain"}`}>
                  {task.status === "done" ? "tamamlandı" : "açık"}
                </span>
                <span style={{ display: "flex", gap: 4, justifyContent: "flex-end" }}>
                  {task.status === "open" && task.userDueDate && !task.dateConfirmedAt ? (
                    <button className="btn ghost" disabled={busy} onClick={() => void confirmDate(task.id)}>
                      Tarihi onayla
                    </button>
                  ) : null}
                  {task.status === "open" ? (
                    <button className="btn ghost" onClick={() => void complete(task.id)}>Bitti</button>
                  ) : null}
                  <button className="btn ghost" onClick={() => void remove(task.id)}>Sil</button>
                </span>
              </div>
            ))}
          </div>
        )}
        <p className="hint" style={{ margin: 0 }}>
          Hatırlatıcı yalnız kullanıcının açıkça onayladığı tarih için kurulur; “Tarihi onayla”
          denmedikçe bildirim üretilmez.
        </p>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>Yerel hatırlatıcılar</h2>
            <p className="card-sub">{reminders.length} hatırlatıcı · {localIsoDay()}</p>
          </div>
          <button className="btn" disabled={busy || !caseId} onClick={() => void enqueueReminders()}>
            Bildirim kuyruğuna yaz
          </button>
        </div>

        {reminders.length === 0 ? (
          <div className="empty">
            <p style={{ margin: 0 }}>
              Bugün için hatırlatıcı yok. Göreve tarih ve hatırlatma seçilip tarih onaylanmalı.
            </p>
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
                    {reminderLabel(r.policy)} · {r.dueDate} · {task?.title ?? "görev"}
                  </span>
                </div>
              );
            })}
          </div>
        )}
        {reminderNote ? <p className="hint" style={{ margin: 0 }}>{reminderNote}</p> : null}
        <p className="hint" style={{ margin: 0 }}>
          Bildirim metni müvekkil adı, dosya adı veya görev başlığı taşımaz; ayrıntı yalnız kasa
          açıkken uygulamada görünür.
        </p>
      </div>
    </div>
  );
}
