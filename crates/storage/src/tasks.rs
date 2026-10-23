//! Görevler. Tarih kullanıcı tarafından girilir ve doğrulanır; uygulama
//! "hukuki son gün hesaplandı" demez (ADR-0008). Hatırlatma yereldir.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRow {
    pub id: String,
    pub case_id: String,
    pub title: String,
    pub status: String,
    pub user_due_date: Option<String>,
    pub date_timezone: Option<String>,
    pub date_origin: String,
    pub assignee_label: Option<String>,
    pub reminder_policy: String,
    /// Kullanıcının tarihi açıkça onayladığı an. Tarih yazmak onay değildir;
    /// hatırlatıcı yalnız bu alan doluyken kurulur (F08-T10).
    pub date_confirmed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub row_version: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTask {
    pub title: String,
    pub user_due_date: Option<String>,
    pub date_timezone: Option<String>,
    pub assignee_label: Option<String>,
    #[serde(default = "default_reminder")]
    pub reminder_policy: String,
}

fn default_reminder() -> String {
    "none".to_string()
}

impl Database {
    pub fn create_task(&self, case_id: &str, new: &NewTask) -> Result<TaskRow, StorageError> {
        if new.title.trim().is_empty() || new.title.chars().count() > 300 {
            return Err(StorageError::InvalidSetting);
        }
        if !matches!(
            new.reminder_policy.as_str(),
            "none" | "on_day" | "day_before"
        ) {
            return Err(StorageError::InvalidSetting);
        }
        let exists: i64 = self.conn().query_row(
            "SELECT count(*) FROM cases WHERE id=?1 AND deleted_at IS NULL",
            [case_id],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Err(StorageError::NotFound);
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO tasks (id,case_id,title,status,user_due_date,date_timezone,date_origin,assignee_label,reminder_policy,created_at,updated_at,row_version)
             VALUES (?1,?2,?3,'open',?4,?5,'user_entered',?6,?7,?8,?8,1)",
            rusqlite::params![
                id,
                case_id,
                new.title.trim(),
                new.user_due_date,
                new.date_timezone,
                new.assignee_label,
                new.reminder_policy,
                now
            ],
        )?;
        self.append_audit(
            "task",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({"caseId": case_id}),
        )?;
        self.get_task(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn get_task(&self, id: &str) -> Result<Option<TaskRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,title,status,user_due_date,date_timezone,date_origin,assignee_label,reminder_policy,created_at,updated_at,row_version,date_confirmed_at
             FROM tasks WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_task).optional()?)
    }

    /// Açık görevler tarih sırasında; tarihsizler sonda.
    pub fn list_tasks(
        &self,
        case_id: &str,
        include_done: bool,
    ) -> Result<Vec<TaskRow>, StorageError> {
        let sql = if include_done {
            "SELECT id,case_id,title,status,user_due_date,date_timezone,date_origin,assignee_label,reminder_policy,created_at,updated_at,row_version,date_confirmed_at
             FROM tasks WHERE case_id=?1 AND deleted_at IS NULL
             ORDER BY (user_due_date IS NULL), user_due_date ASC, created_at ASC"
        } else {
            "SELECT id,case_id,title,status,user_due_date,date_timezone,date_origin,assignee_label,reminder_policy,created_at,updated_at,row_version,date_confirmed_at
             FROM tasks WHERE case_id=?1 AND status='open' AND deleted_at IS NULL
             ORDER BY (user_due_date IS NULL), user_due_date ASC, created_at ASC"
        };
        let mut stmt = self.conn().prepare(sql)?;
        let rows = stmt.query_map([case_id], map_task)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn complete_task(&self, id: &str) -> Result<TaskRow, StorageError> {
        let now = now_iso();
        let changed = self.conn().execute(
            "UPDATE tasks SET status='done', updated_at=?1, row_version=row_version+1 WHERE id=?2 AND status='open'",
            rusqlite::params![now, id],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        self.append_audit(
            "task",
            Some(id),
            "complete",
            "local-user",
            &serde_json::json!({}),
        )?;
        self.get_task(id)?.ok_or(StorageError::NotFound)
    }

    pub fn soft_delete_task(&self, id: &str) -> Result<(), StorageError> {
        let now = now_iso();
        self.conn().execute(
            "UPDATE tasks SET deleted_at=?1, updated_at=?1, row_version=row_version+1 WHERE id=?2",
            rusqlite::params![now, id],
        )?;
        Ok(())
    }

    /// Kullanıcı görev tarihini açıkça onaylar (F08-T10). Onaysız tarih kayıt
    /// olarak durur ama hatırlatıcı üretmez. Geçersiz takvim günü reddedilir;
    /// tarih uydurulmaz.
    pub fn confirm_task_date(&self, id: &str) -> Result<TaskRow, StorageError> {
        let task = self.get_task(id)?.ok_or(StorageError::NotFound)?;
        let Some(due) = task.user_due_date.as_deref() else {
            return Err(StorageError::InvalidSetting);
        };
        if parse_day(due).is_none() || task.status != "open" {
            return Err(StorageError::InvalidSetting);
        }
        let now = now_iso();
        self.conn().execute(
            "UPDATE tasks SET date_confirmed_at=?1, updated_at=?1, row_version=row_version+1
             WHERE id=?2 AND status='open' AND deleted_at IS NULL",
            rusqlite::params![now, id],
        )?;
        self.append_audit(
            "task",
            Some(id),
            "date_confirm",
            "local-user",
            &serde_json::json!({ "dueDate": due }),
        )?;
        self.get_task(id)?.ok_or(StorageError::NotFound)
    }

    /// Bugün (veya `day_before` politikasında yarın) tarihli, kullanıcı onaylı
    /// açık görevler. Saf okuma; yan etki üretmez.
    pub fn due_task_reminders(
        &self,
        case_id: &str,
        today: &str,
    ) -> Result<Vec<ReminderRow>, StorageError> {
        let day = parse_day(today).ok_or(StorageError::InvalidSetting)?;
        let tomorrow = day.next_day().ok_or(StorageError::InvalidSetting)?;
        let tomorrow = tomorrow.to_string();
        let mut stmt = self.conn().prepare(
            "SELECT id,user_due_date,reminder_policy FROM tasks
             WHERE case_id=?1 AND status='open' AND deleted_at IS NULL
               AND date_origin='user_entered' AND date_confirmed_at IS NOT NULL
               AND user_due_date IS NOT NULL AND reminder_policy IN ('on_day','day_before')",
        )?;
        let rows = stmt.query_map([case_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (task_id, due_date, policy) = row?;
            let matches = match policy.as_str() {
                "on_day" => due_date == today,
                "day_before" => due_date == tomorrow,
                _ => false,
            };
            if matches {
                out.push(ReminderRow {
                    task_id,
                    due_date,
                    policy,
                    local_day: today.to_string(),
                });
            }
        }
        Ok(out)
    }

    /// Hatırlatıcıları bildirim outbox'ına yazar. Olay kimliği
    /// `task-reminder:{task}:{date}:{policy}` olduğundan aynı gün tekrar
    /// çağrı çift olay üretmez. Payload **yalnız kimlik ve tarih** taşır:
    /// görev başlığı/dosya adı bildirime sızmaz.
    pub fn enqueue_task_reminders(
        &self,
        case_id: &str,
        today: &str,
    ) -> Result<ReminderEnqueueResult, StorageError> {
        let reminders = self.due_task_reminders(case_id, today)?;
        if reminders.is_empty() {
            return Ok(ReminderEnqueueResult {
                queued: 0,
                reminders,
            });
        }
        let now = now_iso();
        let tx = self.conn().unchecked_transaction()?;
        let mut queued = 0usize;
        for reminder in &reminders {
            let id = format!(
                "task-reminder:{}:{}:{}",
                reminder.task_id, reminder.due_date, reminder.policy
            );
            queued += tx.execute(
                "INSERT OR IGNORE INTO outbox_events
                   (id,aggregate_id,case_id,event_type,payload_version,payload,created_at,delivered_at,attempt_count)
                 VALUES (?1,?2,?3,?4,1,?5,?6,NULL,0)",
                rusqlite::params![
                    id,
                    reminder.task_id,
                    case_id,
                    EVENT_TASK_REMINDER,
                    serde_json::json!({
                        "taskId": reminder.task_id,
                        "dueDate": reminder.due_date,
                        "policy": reminder.policy,
                    })
                    .to_string(),
                    now
                ],
            )?;
        }
        tx.commit()?;
        Ok(ReminderEnqueueResult { queued, reminders })
    }
}

/// Yerel hatırlatıcı satırı. `local_day` kullanıcının bulunduğu gün; sunucu
/// saat dilimi varsayımı yapılmaz. Metin alanı **yok**: bildirim içeriği
/// çağıran tarafta da dosya bilgisinden bağımsız üretilir.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderRow {
    pub task_id: String,
    pub due_date: String,
    pub policy: String,
    pub local_day: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderEnqueueResult {
    pub queued: usize,
    pub reminders: Vec<ReminderRow>,
}

pub const EVENT_TASK_REMINDER: &str = "task.reminder_due";

fn parse_day(value: &str) -> Option<time::Date> {
    time::Date::parse(
        value,
        time::macros::format_description!("[year]-[month]-[day]"),
    )
    .ok()
}

fn map_task(row: &Row<'_>) -> rusqlite::Result<TaskRow> {
    Ok(TaskRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        title: row.get(2)?,
        status: row.get(3)?,
        user_due_date: row.get(4)?,
        date_timezone: row.get(5)?,
        date_origin: row.get(6)?,
        assignee_label: row.get(7)?,
        reminder_policy: row.get(8)?,
        date_confirmed_at: row.get(12)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        row_version: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::DatabaseKey;

    fn setup() -> (Database, String) {
        let db = Database::open_in_memory(&DatabaseKey::new([19u8; 32])).expect("db");
        let case = db
            .create_case(&NewCase {
                title: "Kira".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("case");
        (db, case.id)
    }

    fn task(title: &str, date: Option<&str>) -> NewTask {
        NewTask {
            title: title.into(),
            user_due_date: date.map(str::to_string),
            date_timezone: Some("Europe/Istanbul".into()),
            assignee_label: None,
            reminder_policy: "none".into(),
        }
    }

    #[test]
    fn tasks_sort_by_user_date() {
        let (db, case_id) = setup();
        db.create_task(&case_id, &task("Sonra", Some("2026-06-01")))
            .expect("t1");
        db.create_task(&case_id, &task("Tarihsiz", None))
            .expect("t2");
        db.create_task(&case_id, &task("Önce", Some("2026-01-01")))
            .expect("t3");
        let rows = db.list_tasks(&case_id, false).expect("liste");
        let titles: Vec<_> = rows.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, vec!["Önce", "Sonra", "Tarihsiz"]);
        assert_eq!(rows[0].date_origin, "user_entered");
    }

    #[test]
    fn completing_task_removes_from_open_list() {
        let (db, case_id) = setup();
        let t = db
            .create_task(&case_id, &task("Tebliğ kontrolü", None))
            .expect("görev");
        db.complete_task(&t.id).expect("tamamla");
        assert!(db.list_tasks(&case_id, false).expect("liste").is_empty());
        assert_eq!(db.list_tasks(&case_id, true).expect("liste").len(), 1);
    }

    #[test]
    fn invalid_reminder_policy_rejected() {
        let (db, case_id) = setup();
        let mut t = task("X", None);
        t.reminder_policy = "hergun".into();
        assert!(db.create_task(&case_id, &t).is_err());
    }

    #[test]
    fn cross_case_task_rejected() {
        let (db, _) = setup();
        assert!(db.create_task("yok", &task("X", None)).is_err());
    }

    fn task_with(title: &str, date: &str, policy: &str) -> NewTask {
        NewTask {
            title: title.into(),
            user_due_date: Some(date.into()),
            date_timezone: Some("Europe/Istanbul".into()),
            assignee_label: None,
            reminder_policy: policy.into(),
        }
    }

    #[test]
    fn reminder_needs_explicit_user_confirmation() {
        let (db, case_id) = setup();
        let t = db
            .create_task(&case_id, &task_with("Kira borcu", "2026-10-23", "on_day"))
            .expect("görev");
        // Tarih var ama onay yok: hatırlatıcı kurulmaz.
        assert!(db
            .due_task_reminders(&case_id, "2026-10-23")
            .expect("liste")
            .is_empty());
        let confirmed = db.confirm_task_date(&t.id).expect("onay");
        assert!(confirmed.date_confirmed_at.is_some());
        let reminders = db
            .due_task_reminders(&case_id, "2026-10-23")
            .expect("liste");
        assert_eq!(reminders.len(), 1);
        assert_eq!(reminders[0].task_id, t.id);
        assert_eq!(reminders[0].policy, "on_day");
    }

    #[test]
    fn day_before_policy_matches_tomorrow_only() {
        let (db, case_id) = setup();
        let earlier = db
            .create_task(&case_id, &task_with("Yarın", "2026-10-24", "day_before"))
            .expect("t1");
        let today = db
            .create_task(&case_id, &task_with("Bugün", "2026-10-23", "on_day"))
            .expect("t2");
        let other = db
            .create_task(
                &case_id,
                &task_with("Gelecek hafta", "2026-10-30", "on_day"),
            )
            .expect("t3");
        for id in [&earlier.id, &today.id, &other.id] {
            db.confirm_task_date(id).expect("onay");
        }
        let rows = db
            .due_task_reminders(&case_id, "2026-10-23")
            .expect("liste");
        let ids: Vec<_> = rows.iter().map(|r| r.task_id.as_str()).collect();
        assert_eq!(ids, vec![earlier.id.as_str(), today.id.as_str()]);
        // Gün geçince `day_before` görevi düşer.
        assert_eq!(
            db.due_task_reminders(&case_id, "2026-10-24")
                .expect("liste")
                .len(),
            0
        );
    }

    #[test]
    fn completed_task_produces_no_reminder() {
        let (db, case_id) = setup();
        let t = db
            .create_task(&case_id, &task_with("Bitti", "2026-10-23", "on_day"))
            .expect("görev");
        db.confirm_task_date(&t.id).expect("onay");
        db.complete_task(&t.id).expect("tamamla");
        assert!(db
            .due_task_reminders(&case_id, "2026-10-23")
            .expect("liste")
            .is_empty());
    }

    #[test]
    fn enqueue_is_idempotent_and_leaks_no_title() {
        let (db, case_id) = setup();
        let t = db
            .create_task(
                &case_id,
                &task_with("Müvekkil adı geçen görev", "2026-10-23", "on_day"),
            )
            .expect("görev");
        db.confirm_task_date(&t.id).expect("onay");
        let first = db
            .enqueue_task_reminders(&case_id, "2026-10-23")
            .expect("kuyruk");
        assert_eq!(first.queued, 1);
        let second = db
            .enqueue_task_reminders(&case_id, "2026-10-23")
            .expect("kuyruk");
        assert_eq!(second.queued, 0, "aynı gün ikinci olay yazılmamalı");
        let events = db.list_outbox_events(Some(&case_id), true).expect("outbox");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EVENT_TASK_REMINDER);
        assert_eq!(events[0].aggregate_id, t.id);
        let payload = events[0].payload.to_string();
        assert!(payload.contains(&t.id));
        assert!(
            !payload.contains("Müvekkil"),
            "bildirim görev başlığını taşımamalı"
        );
    }

    #[test]
    fn invalid_or_missing_date_cannot_be_confirmed() {
        let (db, case_id) = setup();
        let no_date = db
            .create_task(&case_id, &task("Tarihsiz", None))
            .expect("görev");
        assert!(db.confirm_task_date(&no_date.id).is_err());
        let impossible = db
            .create_task(&case_id, &task_with("Imkansız", "2026-02-30", "on_day"))
            .expect("görev");
        assert!(db.confirm_task_date(&impossible.id).is_err());
        assert!(db.confirm_task_date("yok").is_err());
    }

    #[test]
    fn invalid_today_rejected_for_reminders() {
        let (db, case_id) = setup();
        assert!(db.due_task_reminders(&case_id, "23.10.2026").is_err());
        assert!(db.enqueue_task_reminders(&case_id, "").is_err());
    }
}
