//! Notlar. Kaynak bağlantısı opsiyoneldir; kaynaksız kişisel not ile kaynaklı
//! çıkarım aynı etiketi almaz. Ham HTML saklanmaz, düz metin/JSON AST tutulur.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteRow {
    pub id: String,
    pub case_id: String,
    pub title: String,
    pub body: String,
    pub author_kind: String,
    pub visibility_label: String,
    pub approved_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub row_version: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewNote {
    pub title: String,
    pub body: String,
}

impl Database {
    pub fn create_note(&self, case_id: &str, new: &NewNote) -> Result<NoteRow, StorageError> {
        if new.body.trim().is_empty() {
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
        // Gövde düz metin olarak JSON zarfında tutulur; HTML çalıştırılmaz.
        let body_ast = serde_json::json!({"v": 1, "text": new.body.trim()}).to_string();
        self.conn().execute(
            "INSERT INTO notes (id,case_id,title,body_ast,author_kind,visibility_label,source_anchor_ids,created_at,updated_at,row_version)
             VALUES (?1,?2,?3,?4,'user','case_private','[]',?5,?5,1)",
            rusqlite::params![id, case_id, new.title.trim(), body_ast, now],
        )?;
        self.append_audit("note", Some(&id), "create", "local-user", &serde_json::json!({"caseId": case_id}))?;
        self.get_note(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn get_note(&self, id: &str) -> Result<Option<NoteRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,title,body_ast,author_kind,visibility_label,approved_at,created_at,updated_at,row_version
             FROM notes WHERE id=?1 AND deleted_at IS NULL",
        )?;
        Ok(stmt.query_row([id], map_note).optional()?)
    }

    pub fn list_notes(&self, case_id: &str) -> Result<Vec<NoteRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,title,body_ast,author_kind,visibility_label,approved_at,created_at,updated_at,row_version
             FROM notes WHERE case_id=?1 AND deleted_at IS NULL ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([case_id], map_note)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn delete_note(&self, id: &str) -> Result<(), StorageError> {
        let now = now_iso();
        self.conn().execute(
            "UPDATE notes SET deleted_at=?1, updated_at=?1, row_version=row_version+1 WHERE id=?2",
            rusqlite::params![now, id],
        )?;
        self.append_audit("note", Some(id), "soft_delete", "local-user", &serde_json::json!({}))?;
        Ok(())
    }
}

fn map_note(row: &Row<'_>) -> rusqlite::Result<NoteRow> {
    let raw: String = row.get(3)?;
    let body = serde_json::from_str::<serde_json::Value>(&raw)
        .ok()
        .and_then(|v| v.get("text").and_then(|t| t.as_str()).map(str::to_string))
        .unwrap_or_else(|| raw.clone());
    Ok(NoteRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        title: row.get(2)?,
        body,
        author_kind: row.get(4)?,
        visibility_label: row.get(5)?,
        approved_at: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        row_version: row.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::DatabaseKey;

    fn setup() -> (Database, String) {
        let db = Database::open_in_memory(&DatabaseKey::new([23u8; 32])).expect("db");
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
    fn note_roundtrip_keeps_plain_text() {
        let (db, case_id) = setup();
        let note = db
            .create_note(&case_id, &NewNote { title: "İlk izlenim".into(), body: "Ödeme yapılmadı.".into() })
            .expect("not");
        assert_eq!(note.body, "Ödeme yapılmadı.");
        assert_eq!(note.author_kind, "user");
        assert_eq!(db.list_notes(&case_id).expect("liste").len(), 1);
    }

    #[test]
    fn html_is_not_interpreted() {
        let (db, case_id) = setup();
        let note = db
            .create_note(&case_id, &NewNote { title: "X".into(), body: "<img src=x onerror=alert(1)>".into() })
            .expect("not");
        assert_eq!(note.body, "<img src=x onerror=alert(1)>");
    }

    #[test]
    fn empty_body_rejected() {
        let (db, case_id) = setup();
        assert!(db.create_note(&case_id, &NewNote { title: "X".into(), body: "   ".into() }).is_err());
    }

    #[test]
    fn cross_case_note_rejected() {
        let (db, _) = setup();
        assert!(db.create_note("yok", &NewNote { title: "X".into(), body: "metin".into() }).is_err());
    }

    #[test]
    fn delete_hides_note_and_audits() {
        let (db, case_id) = setup();
        let note = db.create_note(&case_id, &NewNote { title: "X".into(), body: "metin".into() }).expect("not");
        db.delete_note(&note.id).expect("sil");
        assert!(db.list_notes(&case_id).expect("liste").is_empty());
        assert!(db.verify_audit_chain().expect("zincir") >= 2);
    }
}
