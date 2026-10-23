//! Dışa aktarma kayıtları. Bir dosyanın verisini kasa dışına çıkaran her eylem
//! ayrı kayda alınır; kaynak kimlikleri ve çıktı hash'i korunur.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRow {
    pub id: String,
    pub case_id: String,
    pub export_type: String,
    pub status: String,
    pub destination_kind: String,
    pub output_hash: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

impl Database {
    pub fn record_export(
        &self,
        case_id: &str,
        export_type: &str,
        included_ids: &[String],
        destination_kind: &str,
        output_hash: &str,
    ) -> Result<ExportRow, StorageError> {
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
            "INSERT INTO exports (id,case_id,type,included_ids,status,destination_kind,output_hash,created_at,completed_at)
             VALUES (?1,?2,?3,?4,'ready',?5,?6,?7,?7)",
            rusqlite::params![
                id,
                case_id,
                export_type,
                serde_json::to_string(included_ids).unwrap_or_else(|_| "[]".into()),
                destination_kind,
                output_hash,
                now
            ],
        )?;
        self.append_audit(
            "export",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({"type": export_type, "destination": destination_kind}),
        )?;
        self.get_export(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn get_export(&self, id: &str) -> Result<Option<ExportRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,type,status,destination_kind,output_hash,created_at,completed_at
             FROM exports WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_export).optional()?)
    }

    pub fn list_exports(&self, case_id: &str) -> Result<Vec<ExportRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,type,status,destination_kind,output_hash,created_at,completed_at
             FROM exports WHERE case_id=?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([case_id], map_export)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

fn map_export(row: &Row<'_>) -> rusqlite::Result<ExportRow> {
    Ok(ExportRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        export_type: row.get(2)?,
        status: row.get(3)?,
        destination_kind: row.get(4)?,
        output_hash: row.get(5)?,
        created_at: row.get(6)?,
        completed_at: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::DatabaseKey;

    fn db_with_case() -> (Database, String) {
        let db = Database::open_in_memory(&DatabaseKey::new([43u8; 32])).expect("db");
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
    fn export_is_recorded_with_hash() {
        let (db, case_id) = db_with_case();
        let row = db
            .record_export(
                &case_id,
                "json",
                &["d1".into()],
                "user_selected_path",
                &"a".repeat(64),
            )
            .expect("kayıt");
        assert_eq!(row.status, "ready");
        assert_eq!(row.output_hash.as_deref(), Some("a".repeat(64).as_str()));
        assert_eq!(db.list_exports(&case_id).expect("liste").len(), 1);
    }

    #[test]
    fn unknown_case_rejected() {
        let (db, _) = db_with_case();
        assert!(db
            .record_export("yok", "json", &[], "path", &"b".repeat(64))
            .is_err());
    }
}
