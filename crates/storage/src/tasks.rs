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
        if !matches!(new.reminder_policy.as_str(), "none" | "on_day" | "day_before") {
            return Err(StorageError::InvalidSetting);
        }
        let exists: i64 = self
            .conn()
            .query_row("SELECT count(*) FROM cases WHERE id=?1 AND deleted_at IS NULL", [case_id], |r| r.get(0))?;
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
        self.append_audit("task", Some(&id), "create", "local-user", &serde_json::json!({"caseId": case_id}))?;
        self.get_task(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn get_task(&self, id: &str) -> Result<Option<TaskRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,title,status,user_due_date,date_timezone,date_origin,assignee_label,reminder_policy,created_at,updated_at,row_version
             FROM tasks WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_task).optional()?)
    }

    /// Açık görevler tarih sırasında; tarihsizler sonda.
    pub fn list_tasks(&self, case_id: &str, include_done: bool) -> Result<Vec<TaskRow>, StorageError> {
        let sql = if include_done {
            "SELECT id,case_id,title,status,user_due_date,date_timezone,date_origin,assignee_label,reminder_policy,created_at,updated_at,row_version
             FROM tasks WHERE case_id=?1 AND deleted_at IS NULL
             ORDER BY (user_due_date IS NULL), user_due_date ASC, created_at ASC"
        } else {
            "SELECT id,case_id,title,status,user_due_date,date_timezone,date_origin,assignee_label,reminder_policy,created_at,updated_at,row_version
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
        self.append_audit("task", Some(id), "complete", "local-user", &serde_json::json!({}))?;
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
        db.create_task(&case_id, &task("Sonra", Some("2026-06-01"))).expect("t1");
        db.create_task(&case_id, &task("Tarihsiz", None)).expect("t2");
        db.create_task(&case_id, &task("Önce", Some("2026-01-01"))).expect("t3");
        let rows = db.list_tasks(&case_id, false).expect("liste");
        let titles: Vec<_> = rows.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, vec!["Önce", "Sonra", "Tarihsiz"]);
        assert_eq!(rows[0].date_origin, "user_entered");
    }

    #[test]
    fn completing_task_removes_from_open_list() {
        let (db, case_id) = setup();
        let t = db.create_task(&case_id, &task("Tebliğ kontrolü", None)).expect("görev");
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
}
