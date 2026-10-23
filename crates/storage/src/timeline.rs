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

/// Kullanıcı düzenlemesinde değişebilen alanlar. `assertion_kind` bilinçli
/// olarak dışarıda: kaydın kaynağı (kullanıcı/belge/iddia) düzenlemeyle
/// değişmez, yoksa kaynak izlenebilirliği bozulur.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventPatch {
    pub event_kind: String,
    pub date_value: Option<String>,
    pub date_precision: String,
    pub date_role: String,
    #[serde(default)]
    pub date_raw: Option<String>,
    pub title: String,
    #[serde(default)]
    pub description: String,
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

    /// Kullanıcı düzenlemesi (F08-T04). `expected_row_version` kaydın okunduğu
    /// andaki sürümüdür; başka bir yazma araya girdiyse **yazmaz** ve
    /// `Conflict` döner (F08-T09). Böylece eski sürümle ezme sessizce olmaz.
    ///
    /// Onaylanmış olay da kullanıcı tarafından düzenlenebilir; INV-04 yalnız
    /// AI'ın onaylı kaydı sessizce değiştirmesini yasaklar.
    pub fn update_event(
        &self,
        id: &str,
        expected_row_version: i64,
        patch: &EventPatch,
    ) -> Result<EventRow, StorageError> {
        if !valid_precision(&patch.date_precision) || !valid_role(&patch.date_role) {
            return Err(StorageError::InvalidSetting);
        }
        if patch.title.trim().is_empty() {
            return Err(StorageError::InvalidSetting);
        }
        let current = self.get_event(id)?.ok_or(StorageError::NotFound)?;
        let deleted: bool = self.conn().query_row(
            "SELECT deleted_at IS NOT NULL FROM events WHERE id=?1",
            [id],
            |row| row.get(0),
        )?;
        if deleted {
            return Err(StorageError::NotFound);
        }
        if current.row_version != expected_row_version {
            return Err(StorageError::Conflict {
                expected: expected_row_version,
                actual: current.row_version,
            });
        }
        // Bilinmeyen tarihte sıralama değeri NULL kalır (create ile aynı kural).
        let sortable = if patch.date_precision == "unknown" {
            None
        } else {
            patch
                .date_value
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
        };
        let now = now_iso();
        let changed = self.conn().execute(
            "UPDATE events SET event_kind=?1, date_value=?2, date_precision=?3, date_role=?4,
                 date_raw=?5, sortable_date=?6, title=?7, description=?8, updated_at=?9,
                 row_version=row_version+1
             WHERE id=?10 AND row_version=?11 AND deleted_at IS NULL",
            rusqlite::params![
                patch.event_kind.trim(),
                patch.date_value,
                patch.date_precision,
                patch.date_role,
                patch.date_raw,
                sortable,
                patch.title.trim(),
                patch.description.trim(),
                now,
                id,
                expected_row_version
            ],
        )?;
        if changed == 0 {
            // Okuma ile yazma arasında başka bir yazma oldu: gerçek sürümü bildir.
            let actual = self
                .get_event(id)?
                .map(|row| row.row_version)
                .ok_or(StorageError::NotFound)?;
            return Err(StorageError::Conflict {
                expected: expected_row_version,
                actual,
            });
        }
        self.append_audit(
            "event",
            Some(id),
            "update",
            "local-user",
            &serde_json::json!({"rowVersion": expected_row_version.saturating_add(1)}),
        )?;
        self.get_event(id)?.ok_or(StorageError::NotFound)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSourceRow {
    pub id: String,
    pub event_id: String,
    pub source_anchor_id: String,
    pub relation: String,
    pub user_confirmed: bool,
}

impl Database {
    /// Olayı bir kaynak anchor'ına bağlar. Anchor ve olay aynı dosyada olmalı;
    /// aynı bağ ikinci kez eklenmez (şema UNIQUE kısıtı).
    pub fn link_event_source(
        &self,
        event_id: &str,
        anchor_id: &str,
        relation: &str,
    ) -> Result<EventSourceRow, StorageError> {
        if !matches!(relation, "states" | "supports" | "contradicts" | "context") {
            return Err(StorageError::InvalidSetting);
        }
        let event_case: String = self
            .conn()
            .query_row("SELECT case_id FROM events WHERE id=?1", [event_id], |r| {
                r.get(0)
            })
            .optional()?
            .ok_or(StorageError::NotFound)?;
        let anchor_case: String = self
            .conn()
            .query_row(
                "SELECT case_id FROM source_anchors WHERE id=?1",
                [anchor_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(StorageError::NotFound)?;
        if event_case != anchor_case {
            return Err(StorageError::NotFound);
        }
        if let Some(existing) = self.get_event_source_by_pair(event_id, anchor_id, relation)? {
            return Ok(existing);
        }
        let id = uuid::Uuid::now_v7().to_string();
        self.conn().execute(
            "INSERT INTO event_sources (id,event_id,source_anchor_id,relation,user_confirmed)
             VALUES (?1,?2,?3,?4,1)",
            rusqlite::params![id, event_id, anchor_id, relation],
        )?;
        self.append_audit(
            "event_source",
            Some(&id),
            "link",
            "local-user",
            &serde_json::json!({"eventId": event_id, "anchorId": anchor_id, "relation": relation}),
        )?;
        Ok(EventSourceRow {
            id,
            event_id: event_id.to_string(),
            source_anchor_id: anchor_id.to_string(),
            relation: relation.to_string(),
            user_confirmed: true,
        })
    }

    pub fn get_event_source_by_pair(
        &self,
        event_id: &str,
        anchor_id: &str,
        relation: &str,
    ) -> Result<Option<EventSourceRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,event_id,source_anchor_id,relation,user_confirmed
             FROM event_sources WHERE event_id=?1 AND source_anchor_id=?2 AND relation=?3",
        )?;
        Ok(stmt
            .query_row(
                rusqlite::params![event_id, anchor_id, relation],
                map_event_source,
            )
            .optional()?)
    }

    pub fn list_event_sources(&self, event_id: &str) -> Result<Vec<EventSourceRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,event_id,source_anchor_id,relation,user_confirmed
             FROM event_sources WHERE event_id=?1 ORDER BY rowid",
        )?;
        let rows = stmt.query_map([event_id], map_event_source)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn unlink_event_source(&self, id: &str) -> Result<(), StorageError> {
        let changed = self
            .conn()
            .execute("DELETE FROM event_sources WHERE id=?1", [id])?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        self.append_audit(
            "event_source",
            Some(id),
            "unlink",
            "local-user",
            &serde_json::json!({}),
        )?;
        Ok(())
    }
}

fn map_event_source(row: &Row<'_>) -> rusqlite::Result<EventSourceRow> {
    Ok(EventSourceRow {
        id: row.get(0)?,
        event_id: row.get(1)?,
        source_anchor_id: row.get(2)?,
        relation: row.get(3)?,
        user_confirmed: row.get::<_, i64>(4)? == 1,
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

    fn patch(title: &str, precision: &str, role: &str, date: Option<&str>) -> EventPatch {
        EventPatch {
            event_kind: "payment".into(),
            date_value: date.map(str::to_string),
            date_precision: precision.into(),
            date_role: role.into(),
            date_raw: date.map(str::to_string),
            title: title.into(),
            description: String::new(),
        }
    }

    #[test]
    fn update_event_writes_fields_and_bumps_row_version() {
        let (db, case_id) = setup();
        let created = db
            .create_event(&case_id, &event("Ödeme", Some("2026-02-02"), "day"))
            .expect("olay");
        assert_eq!(created.row_version, 1);

        let updated = db
            .update_event(
                &created.id,
                created.row_version,
                &patch("Tebliğ", "day", "notification", Some("2026-03-03")),
            )
            .expect("güncelle");

        assert_eq!(updated.title, "Tebliğ");
        assert_eq!(updated.date_role, "notification");
        assert_eq!(updated.sortable_date.as_deref(), Some("2026-03-03"));
        assert_eq!(updated.row_version, 2);
        assert_eq!(
            updated.assertion_kind, created.assertion_kind,
            "kaynak türü değişmemeli"
        );
        assert!(db.verify_audit_chain().expect("zincir") >= 2);
    }

    #[test]
    fn update_event_with_stale_version_conflicts_and_keeps_row() {
        let (db, case_id) = setup();
        let created = db
            .create_event(&case_id, &event("Ödeme", Some("2026-02-02"), "day"))
            .expect("olay");
        db.update_event(&created.id, 1, &patch("Yeni başlık", "day", "event", None))
            .expect("ilk yazma");

        let error = db
            .update_event(
                &created.id,
                1,
                &patch("Eski sürümle ezme", "day", "event", None),
            )
            .expect_err("eski sürüm kabul edilmemeli");

        match error {
            StorageError::Conflict { expected, actual } => {
                assert_eq!(expected, 1);
                assert_eq!(actual, 2);
            }
            other => panic!("çatışma bekleniyordu: {other:?}"),
        }
        let stored = db.get_event(&created.id).expect("oku").expect("var");
        assert_eq!(stored.title, "Yeni başlık", "eski sürüm yazmamalı");
        assert_eq!(stored.row_version, 2);
    }

    #[test]
    fn update_event_to_unknown_precision_clears_sortable_date() {
        let (db, case_id) = setup();
        let created = db
            .create_event(&case_id, &event("Ödeme", Some("2026-02-02"), "day"))
            .expect("olay");
        let updated = db
            .update_event(
                &created.id,
                1,
                &patch("Tarih belirsiz", "unknown", "event", Some("2026-02-02")),
            )
            .expect("güncelle");
        assert_eq!(updated.sortable_date, None);
        assert_eq!(updated.date_precision, "unknown");
        // Bilinmeyen tarih sıralamada sona düşer.
        let rows = db.list_events(&case_id).expect("liste");
        assert_eq!(
            rows.last().map(|row| row.id.as_str()),
            Some(created.id.as_str())
        );
    }

    #[test]
    fn update_event_rejects_invalid_input() {
        let (db, case_id) = setup();
        let created = db
            .create_event(&case_id, &event("Ödeme", Some("2026-02-02"), "day"))
            .expect("olay");
        assert_eq!(
            db.update_event(&created.id, 1, &patch("  ", "day", "event", None))
                .unwrap_err()
                .code(),
            "E_DB_SETTING"
        );
        assert_eq!(
            db.update_event(&created.id, 1, &patch("X", "gun", "event", None))
                .unwrap_err()
                .code(),
            "E_DB_SETTING"
        );
        assert_eq!(
            db.update_event(&created.id, 1, &patch("X", "day", "uydurma", None))
                .unwrap_err()
                .code(),
            "E_DB_SETTING"
        );
        assert_eq!(
            db.update_event("olmayan-olay", 1, &patch("X", "day", "event", None))
                .unwrap_err()
                .code(),
            "E_NOT_FOUND"
        );
    }

    #[test]
    fn update_event_on_deleted_row_is_not_found() {
        let (db, case_id) = setup();
        let created = db
            .create_event(&case_id, &event("Ödeme", Some("2026-02-02"), "day"))
            .expect("olay");
        db.soft_delete_event(&created.id).expect("sil");
        assert_eq!(
            db.update_event(
                &created.id,
                created.row_version,
                &patch("X", "day", "event", None)
            )
            .unwrap_err()
            .code(),
            "E_NOT_FOUND"
        );
    }
}

#[cfg(test)]
mod source_tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::DatabaseKey;

    fn setup() -> (Database, String) {
        let db = Database::open_in_memory(&DatabaseKey::new([53u8; 32])).expect("db");
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

    #[test]
    fn invalid_relation_rejected() {
        let (db, _) = setup();
        assert_eq!(
            db.link_event_source("e1", "a1", "uydurma")
                .unwrap_err()
                .code(),
            "E_DB_SETTING"
        );
    }

    #[test]
    fn missing_anchor_is_not_found() {
        let (db, case_id) = setup();
        let event = db
            .create_event(
                &case_id,
                &NewEvent {
                    event_kind: "event".into(),
                    date_value: Some("2026-01-01".into()),
                    date_precision: "day".into(),
                    date_role: "event".into(),
                    date_raw: None,
                    title: "Ödeme yapılmadı".into(),
                    description: String::new(),
                    assertion_kind: "user".into(),
                },
            )
            .expect("olay");
        assert_eq!(
            db.link_event_source(&event.id, "yok", "supports")
                .unwrap_err()
                .code(),
            "E_NOT_FOUND"
        );
    }
}
