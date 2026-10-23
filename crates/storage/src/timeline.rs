//! Kronoloji: olay kayıtları. Olay tarihi, belge tarihi ve tebliğ tarihi ayrı
//! `date_role` değerleridir; eksik gün uydurulmaz, bilinmeyen tarih sıralamaya
//! zorla sokulmaz (REQ-011, 13.3).

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRow {
    pub id: String,
    pub case_id: String,
    pub event_kind: String,
    pub date_value: Option<String>,
    pub date_precision: String,
    pub date_role: String,
    pub date_raw: Option<String>,
    pub sortable_date: Option<String>,
    pub title: String,
    pub description: String,
    pub assertion_kind: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub row_version: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewEvent {
    pub event_kind: String,
    pub date_value: Option<String>,
    pub date_precision: String,
    pub date_role: String,
    #[serde(default)]
    pub date_raw: Option<String>,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_assertion")]
    pub assertion_kind: String,
}

fn default_assertion() -> String {
    "user".to_string()
}

fn valid_precision(value: &str) -> bool {
    matches!(
        value,
        "day" | "month" | "year" | "range" | "approximate" | "unknown"
    )
}

fn valid_role(value: &str) -> bool {
    matches!(
        value,
        "event"
            | "document"
            | "notification"
            | "contract"
            | "hearing"
            | "deadline_candidate"
            | "unknown"
    )
}

fn valid_assertion(value: &str) -> bool {
    matches!(
        value,
        "user" | "party_claim" | "document" | "system_inference"
    )
}

impl Database {
    pub fn create_event(&self, case_id: &str, new: &NewEvent) -> Result<EventRow, StorageError> {
        if !valid_precision(&new.date_precision)
            || !valid_role(&new.date_role)
            || !valid_assertion(&new.assertion_kind)
        {
            return Err(StorageError::InvalidSetting);
        }
        if new.title.trim().is_empty() {
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
        // Bilinmeyen tarihte sıralama değeri NULL kalır; uydurulmuş gün yazılmaz.
        let sortable = if new.date_precision == "unknown" {
            None
        } else {
            new.date_value
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
        };
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO events (id,case_id,event_kind,date_value,date_precision,date_role,date_raw,sortable_date,title,description,assertion_kind,status,created_at,updated_at,row_version)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'draft',?12,?12,1)",
            rusqlite::params![
                id,
                case_id,
                new.event_kind.trim(),
                new.date_value,
                new.date_precision,
                new.date_role,
                new.date_raw,
                sortable,
                new.title.trim(),
                new.description.trim(),
                new.assertion_kind,
                now
            ],
        )?;
        self.append_audit(
            "event",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({"caseId": case_id}),
        )?;
        self.get_event(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn get_event(&self, id: &str) -> Result<Option<EventRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,event_kind,date_value,date_precision,date_role,date_raw,sortable_date,title,description,assertion_kind,status,created_at,updated_at,row_version
             FROM events WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_event).optional()?)
    }

    /// Kronoloji listesi: tarihi bilinenler sıralı, bilinmeyenler sonda ayrı grup.
    pub fn list_events(&self, case_id: &str) -> Result<Vec<EventRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,event_kind,date_value,date_precision,date_role,date_raw,sortable_date,title,description,assertion_kind,status,created_at,updated_at,row_version
             FROM events WHERE case_id=?1 AND deleted_at IS NULL
             ORDER BY (sortable_date IS NULL), sortable_date ASC, created_at ASC",
        )?;
        let rows = stmt.query_map([case_id], map_event)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Kullanıcı onaylı tarih/başlık güncellemesi. Onaylanmış olay AI tarafından
    /// sessizce değiştirilmez (INV-04); bu fonksiyon yalnız kullanıcı eylemidir.
    pub fn confirm_event(&self, id: &str) -> Result<EventRow, StorageError> {
        let now = now_iso();
        let changed = self.conn().execute(
            "UPDATE events SET status='confirmed', updated_at=?1, row_version=row_version+1 WHERE id=?2 AND status='draft'",
            rusqlite::params![now, id],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        self.append_audit(
            "event",
            Some(id),
            "confirm",
            "local-user",
            &serde_json::json!({}),
        )?;
        self.get_event(id)?.ok_or(StorageError::NotFound)
    }

    pub fn soft_delete_event(&self, id: &str) -> Result<(), StorageError> {
        let now = now_iso();
        self.conn().execute(
            "UPDATE events SET deleted_at=?1, updated_at=?1, row_version=row_version+1 WHERE id=?2",
            rusqlite::params![now, id],
        )?;
        self.append_audit(
            "event",
            Some(id),
            "soft_delete",
            "local-user",
            &serde_json::json!({}),
        )?;
        Ok(())
    }
}

fn map_event(row: &Row<'_>) -> rusqlite::Result<EventRow> {
    Ok(EventRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        event_kind: row.get(2)?,
        date_value: row.get(3)?,
        date_precision: row.get(4)?,
        date_role: row.get(5)?,
        date_raw: row.get(6)?,
        sortable_date: row.get(7)?,
        title: row.get(8)?,
        description: row.get(9)?,
        assertion_kind: row.get(10)?,
        status: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
        row_version: row.get(14)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::DatabaseKey;

    fn setup() -> (Database, String) {
        let db = Database::open_in_memory(&DatabaseKey::new([17u8; 32])).expect("db");
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

    fn event(title: &str, date: Option<&str>, precision: &str) -> NewEvent {
        NewEvent {
            event_kind: "payment".into(),
            date_value: date.map(str::to_string),
            date_precision: precision.into(),
            date_role: "event".into(),
            date_raw: date.map(str::to_string),
            title: title.into(),
            description: String::new(),
            assertion_kind: "user".into(),
        }
    }

    #[test]
    fn events_sort_by_date_and_unknown_last() {
        let (db, case_id) = setup();
        db.create_event(&case_id, &event("Sonra", Some("2026-05-01"), "day"))
            .expect("e1");
        db.create_event(&case_id, &event("Bilinmeyen", None, "unknown"))
            .expect("e2");
        db.create_event(&case_id, &event("Önce", Some("2026-01-15"), "day"))
            .expect("e3");

        let rows = db.list_events(&case_id).expect("liste");
        let titles: Vec<_> = rows.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, vec!["Önce", "Sonra", "Bilinmeyen"]);
        assert_eq!(rows[2].sortable_date, None);
    }

    #[test]
    fn unknown_date_cannot_carry_sortable_value() {
        let (db, case_id) = setup();
        let mut e = event("Bilinmeyen", Some("2026-01-01"), "unknown");
        e.date_value = Some("2026-01-01".into());
        let created = db.create_event(&case_id, &e).expect("olay");
        assert_eq!(created.sortable_date, None);
    }

    #[test]
    fn invalid_precision_rejected() {
        let (db, case_id) = setup();
        assert!(db
            .create_event(&case_id, &event("X", Some("2026-01-01"), "gun"))
            .is_err());
    }

    #[test]
    fn confirm_is_idempotent_and_audited() {
        let (db, case_id) = setup();
        let created = db
            .create_event(&case_id, &event("Ödeme", Some("2026-02-02"), "day"))
            .expect("olay");
        let confirmed = db.confirm_event(&created.id).expect("onayla");
        assert_eq!(confirmed.status, "confirmed");
        assert!(db.confirm_event(&created.id).is_err());
        assert!(db.verify_audit_chain().expect("zincir") >= 2);
    }

    #[test]
    fn cross_case_event_creation_rejected() {
        let (db, _) = setup();
        assert!(db
            .create_event("olmayan-dosya", &event("X", None, "unknown"))
            .is_err());
    }
}
