//! Notlar. Kaynak bağlantısı opsiyoneldir; kaynaksız kişisel not ile kaynaklı
//! çıkarım aynı etiketi almaz. Ham HTML saklanmaz, düz metin/JSON AST tutulur.
//!
//! F07-T10: düzenleme iyimser eşzamanlılıkla (`row_version`) korunur; geri/ileri
//! al yığınları ve taslak **kasa içinde** durur (frontend localStorage'a
//! yazılmaz). Her yazma kalıcı bir revizyon bırakır, geri al gerçekten saklanmış
//! sürümü geri getirir.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

/// Geri/ileri al yığınlarında tutulan en fazla sürüm sayısı.
const HISTORY_LIMIT: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteRow {
    pub id: String,
    pub case_id: String,
    pub title: String,
    pub body: String,
    pub author_kind: String,
    pub visibility_label: String,
    pub source_anchor_ids: Vec<String>,
    pub approved_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub row_version: i64,
    /// En son revizyon numarası (1 = oluşturma).
    pub revision_no: i64,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewNote {
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub source_anchor_ids: Vec<String>,
}

/// Var olan notun kullanıcı düzenlemesi.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteEdit {
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub source_anchor_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteRevisionRow {
    pub note_id: String,
    pub revision_no: i64,
    pub title: String,
    pub body: String,
    pub source_anchor_ids: Vec<String>,
    /// `create` | `edit` | `undo` | `redo`
    pub origin: String,
    pub created_at: String,
}

/// Otomatik kaydedilen taslak. `base_row_version` taslak yazılırken notun
/// okunduğu sürümdür; kaydetme sırasında uyuşmazsa çakışma ekranı açılır.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteDraftInput {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub source_anchor_ids: Vec<String>,
    #[serde(default)]
    pub base_row_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteDraftRow {
    pub case_id: String,
    /// `None` = henüz kaydedilmemiş yeni not taslağı.
    pub note_id: Option<String>,
    pub title: String,
    pub body: String,
    pub source_anchor_ids: Vec<String>,
    pub base_row_version: Option<i64>,
    pub updated_at: String,
}

impl Database {
    /// Not yazımlarını tek işlemde uygular. Çağıran zaten bir işlem açmışsa
    /// (ör. inceleme kararı commit'i) iç içe işlem açmaz; aynı işleme katılır.
    /// Aksi hâlde SQLite "cannot start a transaction within a transaction" verir.
    fn note_write<F>(&self, run: F) -> Result<(), StorageError>
    where
        F: FnOnce(&rusqlite::Connection) -> Result<(), StorageError>,
    {
        if self.conn().is_autocommit() {
            let tx = self.conn().unchecked_transaction()?;
            run(&tx)?;
            tx.commit()?;
            Ok(())
        } else {
            run(self.conn())
        }
    }

    pub fn create_note(&self, case_id: &str, new: &NewNote) -> Result<NoteRow, StorageError> {
        if new.body.trim().is_empty() {
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
        // Gövde düz metin olarak JSON zarfında tutulur; HTML çalıştırılmaz.
        let body_ast = encode_body(new.body.trim());
        let anchors = encode_anchors(&new.source_anchor_ids);
        self.note_write(|conn| {
            conn.execute(
                "INSERT INTO notes (id,case_id,title,body_ast,author_kind,visibility_label,source_anchor_ids,created_at,updated_at,row_version)
                 VALUES (?1,?2,?3,?4,'user','case_private',?5,?6,?6,1)",
                rusqlite::params![id, case_id, new.title.trim(), body_ast, anchors, now],
            )?;
            conn.execute(
                "INSERT INTO note_revisions (note_id,revision_no,title,body_ast,source_anchor_ids,origin,created_at)
                 VALUES (?1,1,?2,?3,?4,'create',?5)",
                rusqlite::params![id, new.title.trim(), body_ast, anchors, now],
            )?;
            Ok(())
        })?;
        self.append_audit(
            "note",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({"caseId": case_id}),
        )?;
        self.get_note(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn get_note(&self, id: &str) -> Result<Option<NoteRow>, StorageError> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {NOTE_COLUMNS} FROM notes WHERE id=?1 AND deleted_at IS NULL"
        ))?;
        Ok(stmt.query_row([id], map_note).optional()?)
    }

    pub fn list_notes(&self, case_id: &str) -> Result<Vec<NoteRow>, StorageError> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {NOTE_COLUMNS} FROM notes WHERE case_id=?1 AND deleted_at IS NULL ORDER BY updated_at DESC"
        ))?;
        let rows = stmt.query_map([case_id], map_note)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Kullanıcı düzenlemesi (F07-T10). `expected_row_version` notun okunduğu
    /// sürümdür; araya başka bir yazma girdiyse **yazmaz** ve `Conflict` döner.
    /// Başarılı yazma önceki revizyonu geri al yığınına iter, ileri al yığınını
    /// temizler (yeni dal) ve kalıcı bir `edit` revizyonu bırakır.
    pub fn update_note(
        &self,
        id: &str,
        expected_row_version: i64,
        edit: &NoteEdit,
    ) -> Result<NoteRow, StorageError> {
        if edit.body.trim().is_empty() {
            return Err(StorageError::InvalidSetting);
        }
        let current = self.get_note(id)?.ok_or(StorageError::NotFound)?;
        if current.row_version != expected_row_version {
            return Err(StorageError::Conflict {
                expected: expected_row_version,
                actual: current.row_version,
            });
        }
        let (mut undo, _) = self.history_stacks(id)?;
        undo = push_stack(undo, current.revision_no);
        let now = now_iso();
        let body_ast = encode_body(edit.body.trim());
        let anchors = encode_anchors(&edit.source_anchor_ids);
        let next_rev = current.revision_no.saturating_add(1);
        let mut written = 0usize;
        self.note_write(|conn| {
            written = conn.execute(
                "UPDATE notes SET title=?1, body_ast=?2, source_anchor_ids=?3, updated_at=?4,
                     row_version=row_version+1, undo_stack=?5, redo_stack='[]'
                 WHERE id=?6 AND row_version=?7 AND deleted_at IS NULL",
                rusqlite::params![
                    edit.title.trim(),
                    body_ast,
                    anchors,
                    now,
                    encode_stack(&undo),
                    id,
                    expected_row_version
                ],
            )?;
            if written > 0 {
                conn.execute(
                    "INSERT INTO note_revisions (note_id,revision_no,title,body_ast,source_anchor_ids,origin,created_at)
                     VALUES (?1,?2,?3,?4,?5,'edit',?6)",
                    rusqlite::params![id, next_rev, edit.title.trim(), body_ast, anchors, now],
                )?;
            }
            Ok(())
        })?;
        if written == 0 {
            // Okuma ile yazma arasında başka bir yazma oldu: gerçek sürümü bildir.
            let actual = self
                .get_note(id)?
                .map(|row| row.row_version)
                .ok_or(StorageError::NotFound)?;
            return Err(StorageError::Conflict {
                expected: expected_row_version,
                actual,
            });
        }
        self.append_audit(
            "note",
            Some(id),
            "update",
            "local-user",
            &serde_json::json!({"rowVersion": expected_row_version.saturating_add(1)}),
        )?;
        self.get_note(id)?.ok_or(StorageError::NotFound)
    }

    /// Geri al: yığındaki önceki revizyonun metnini **yeni revizyon** olarak
    /// uygular. Geçmiş silinmez; ileri al aynı sürümü geri getirebilir.
    pub fn undo_note(&self, id: &str, expected_row_version: i64) -> Result<NoteRow, StorageError> {
        self.step_note_history(id, expected_row_version, true)
    }

    /// İleri al: geri al ile bırakılan sürüme döner.
    pub fn redo_note(&self, id: &str, expected_row_version: i64) -> Result<NoteRow, StorageError> {
        self.step_note_history(id, expected_row_version, false)
    }

    pub fn list_note_revisions(&self, id: &str) -> Result<Vec<NoteRevisionRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT note_id,revision_no,title,body_ast,source_anchor_ids,origin,created_at
             FROM note_revisions WHERE note_id=?1 ORDER BY revision_no DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![id, HISTORY_LIMIT as i64], |row| {
            let raw: String = row.get(3)?;
            Ok(NoteRevisionRow {
                note_id: row.get(0)?,
                revision_no: row.get(1)?,
                title: row.get(2)?,
                body: decode_body(&raw),
                source_anchor_ids: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(4)?)
                    .unwrap_or_default(),
                origin: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
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
        self.append_audit(
            "note",
            Some(id),
            "soft_delete",
            "local-user",
            &serde_json::json!({}),
        )?;
        Ok(())
    }

    /// Taslağı kasaya yazar. `note_id = None` yeni not taslağıdır. Gövde boş
    /// olabilir: taslak, kullanıcının yazdığı son hâli birebir korur. Asıl nota
    /// dönüşme kuralı (`create_note`) yine boş gövdeyi reddeder.
    pub fn save_note_draft(
        &self,
        case_id: &str,
        note_id: Option<&str>,
        input: &NoteDraftInput,
    ) -> Result<NoteDraftRow, StorageError> {
        let exists: i64 = self.conn().query_row(
            "SELECT count(*) FROM cases WHERE id=?1 AND deleted_at IS NULL",
            [case_id],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Err(StorageError::NotFound);
        }
        if let Some(nid) = note_id {
            let owner: Option<String> = self
                .conn()
                .query_row(
                    "SELECT case_id FROM notes WHERE id=?1 AND deleted_at IS NULL",
                    [nid],
                    |r| r.get(0),
                )
                .optional()?;
            match owner.as_deref() {
                Some(owner_case) if owner_case == case_id => {}
                _ => return Err(StorageError::NotFound),
            }
        }
        let key = note_id.unwrap_or("");
        let now = now_iso();
        let anchors = encode_anchors(&input.source_anchor_ids);
        self.conn().execute(
            "INSERT INTO note_drafts (case_id,note_id,title,body,source_anchor_ids,base_row_version,updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(case_id,note_id) DO UPDATE SET
               title=excluded.title, body=excluded.body,
               source_anchor_ids=excluded.source_anchor_ids,
               base_row_version=excluded.base_row_version, updated_at=excluded.updated_at",
            rusqlite::params![case_id, key, input.title, input.body, anchors, input.base_row_version, now],
        )?;
        self.get_note_draft(case_id, note_id)?
            .ok_or(StorageError::NotFound)
    }

    pub fn get_note_draft(
        &self,
        case_id: &str,
        note_id: Option<&str>,
    ) -> Result<Option<NoteDraftRow>, StorageError> {
        let key = note_id.unwrap_or("");
        let mut stmt = self.conn().prepare(
            "SELECT case_id,note_id,title,body,source_anchor_ids,base_row_version,updated_at
             FROM note_drafts WHERE case_id=?1 AND note_id=?2",
        )?;
        Ok(stmt
            .query_row(rusqlite::params![case_id, key], |row| {
                let raw_id: String = row.get(1)?;
                Ok(NoteDraftRow {
                    case_id: row.get(0)?,
                    note_id: Some(raw_id).filter(|value| !value.is_empty()),
                    title: row.get(2)?,
                    body: row.get(3)?,
                    source_anchor_ids: serde_json::from_str::<Vec<String>>(
                        &row.get::<_, String>(4)?,
                    )
                    .unwrap_or_default(),
                    base_row_version: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .optional()?)
    }

    /// Taslağı siler. `true` = silinecek taslak vardı.
    pub fn clear_note_draft(
        &self,
        case_id: &str,
        note_id: Option<&str>,
    ) -> Result<bool, StorageError> {
        let key = note_id.unwrap_or("");
        let removed = self.conn().execute(
            "DELETE FROM note_drafts WHERE case_id=?1 AND note_id=?2",
            rusqlite::params![case_id, key],
        )?;
        Ok(removed > 0)
    }

    fn history_stacks(&self, id: &str) -> Result<(Vec<i64>, Vec<i64>), StorageError> {
        let row: Option<(String, String)> = self
            .conn()
            .query_row(
                "SELECT undo_stack, redo_stack FROM notes WHERE id=?1 AND deleted_at IS NULL",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (undo, redo) = row.ok_or(StorageError::NotFound)?;
        Ok((parse_stack(&undo), parse_stack(&redo)))
    }

    fn step_note_history(
        &self,
        id: &str,
        expected_row_version: i64,
        backward: bool,
    ) -> Result<NoteRow, StorageError> {
        let current = self.get_note(id)?.ok_or(StorageError::NotFound)?;
        if current.row_version != expected_row_version {
            return Err(StorageError::Conflict {
                expected: expected_row_version,
                actual: current.row_version,
            });
        }
        let (mut undo, mut redo) = self.history_stacks(id)?;
        let target = if backward { undo.pop() } else { redo.pop() };
        let target = target.ok_or_else(|| {
            StorageError::NoteHistory(if backward {
                "geri alınacak sürüm yok".into()
            } else {
                "ileri alınacak sürüm yok".into()
            })
        })?;
        let (title, body_ast, anchors) = self
            .revision_raw(id, target)?
            .ok_or(StorageError::NotFound)?;
        if backward {
            redo = push_stack(redo, current.revision_no);
        } else {
            undo = push_stack(undo, current.revision_no);
        }
        let origin = if backward { "undo" } else { "redo" };
        let now = now_iso();
        let next_rev = current.revision_no.saturating_add(1);
        let mut written = 0usize;
        self.note_write(|conn| {
            written = conn.execute(
                "UPDATE notes SET title=?1, body_ast=?2, source_anchor_ids=?3, updated_at=?4,
                     row_version=row_version+1, undo_stack=?5, redo_stack=?6
                 WHERE id=?7 AND row_version=?8 AND deleted_at IS NULL",
                rusqlite::params![
                    title,
                    body_ast,
                    anchors,
                    now,
                    encode_stack(&undo),
                    encode_stack(&redo),
                    id,
                    expected_row_version
                ],
            )?;
            if written > 0 {
                conn.execute(
                    "INSERT INTO note_revisions (note_id,revision_no,title,body_ast,source_anchor_ids,origin,created_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7)",
                    rusqlite::params![id, next_rev, title, body_ast, anchors, origin, now],
                )?;
            }
            Ok(())
        })?;
        if written == 0 {
            let actual = self
                .get_note(id)?
                .map(|row| row.row_version)
                .ok_or(StorageError::NotFound)?;
            return Err(StorageError::Conflict {
                expected: expected_row_version,
                actual,
            });
        }
        self.append_audit(
            "note",
            Some(id),
            origin,
            "local-user",
            &serde_json::json!({"restoredRevision": target, "rowVersion": expected_row_version.saturating_add(1)}),
        )?;
        self.get_note(id)?.ok_or(StorageError::NotFound)
    }

    /// Revizyonun ham içeriği (`body_ast` çözülmeden), geri/ileri al için.
    fn revision_raw(
        &self,
        note_id: &str,
        revision_no: i64,
    ) -> Result<Option<(String, String, String)>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT title, body_ast, source_anchor_ids FROM note_revisions
             WHERE note_id=?1 AND revision_no=?2",
        )?;
        Ok(stmt
            .query_row(rusqlite::params![note_id, revision_no], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .optional()?)
    }
}

const NOTE_COLUMNS: &str =
    "id,case_id,title,body_ast,author_kind,visibility_label,source_anchor_ids,\
approved_at,created_at,updated_at,row_version,undo_stack,redo_stack,\
(SELECT COALESCE(max(revision_no),0) FROM note_revisions WHERE note_revisions.note_id = notes.id)";

fn encode_body(text: &str) -> String {
    serde_json::json!({"v": 1, "text": text}).to_string()
}

fn decode_body(raw: &str) -> String {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|v| v.get("text").and_then(|t| t.as_str()).map(str::to_string))
        .unwrap_or_else(|| raw.to_string())
}

fn encode_anchors(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".into())
}

fn parse_stack(raw: &str) -> Vec<i64> {
    serde_json::from_str::<Vec<i64>>(raw).unwrap_or_default()
}

fn encode_stack(values: &[i64]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".into())
}

/// Yığına ekler; `HISTORY_LIMIT` aşılırsa en eski kayıt düşer.
fn push_stack(mut stack: Vec<i64>, value: i64) -> Vec<i64> {
    stack.push(value);
    if stack.len() > HISTORY_LIMIT {
        let drop_count = stack.len() - HISTORY_LIMIT;
        stack.drain(0..drop_count);
    }
    stack
}

fn map_note(row: &Row<'_>) -> rusqlite::Result<NoteRow> {
    let raw: String = row.get(3)?;
    let undo_raw: String = row.get(11)?;
    let redo_raw: String = row.get(12)?;
    Ok(NoteRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        title: row.get(2)?,
        body: decode_body(&raw),
        author_kind: row.get(4)?,
        visibility_label: row.get(5)?,
        source_anchor_ids: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(6)?)
            .unwrap_or_default(),
        approved_at: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        row_version: row.get(10)?,
        revision_no: row.get(13)?,
        can_undo: !parse_stack(&undo_raw).is_empty(),
        can_redo: !parse_stack(&redo_raw).is_empty(),
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

    fn note(db: &Database, case_id: &str, body: &str) -> NoteRow {
        db.create_note(
            case_id,
            &NewNote {
                title: "İlk izlenim".into(),
                body: body.into(),
                source_anchor_ids: vec![],
            },
        )
        .expect("not")
    }

    fn edit(body: &str) -> NoteEdit {
        NoteEdit {
            title: "İlk izlenim".into(),
            body: body.into(),
            source_anchor_ids: vec![],
        }
    }

    #[test]
    fn note_roundtrip_keeps_plain_text() {
        let (db, case_id) = setup();
        let created = note(&db, &case_id, "Ödeme yapılmadı.");
        assert_eq!(created.body, "Ödeme yapılmadı.");
        assert_eq!(created.author_kind, "user");
        assert_eq!(created.revision_no, 1);
        assert!(!created.can_undo);
        assert!(!created.can_redo);
        assert_eq!(db.list_notes(&case_id).expect("liste").len(), 1);
    }

    #[test]
    fn html_is_not_interpreted() {
        let (db, case_id) = setup();
        let created = note(&db, &case_id, "<img src=x onerror=alert(1)>");
        assert_eq!(created.body, "<img src=x onerror=alert(1)>");
    }

    #[test]
    fn empty_body_rejected() {
        let (db, case_id) = setup();
        assert!(db
            .create_note(
                &case_id,
                &NewNote {
                    title: "X".into(),
                    body: "   ".into(),
                    source_anchor_ids: vec![],
                }
            )
            .is_err());
    }

    #[test]
    fn cross_case_note_rejected() {
        let (db, _) = setup();
        assert!(db
            .create_note(
                "yok",
                &NewNote {
                    title: "X".into(),
                    body: "metin".into(),
                    source_anchor_ids: vec![],
                }
            )
            .is_err());
    }

    #[test]
    fn delete_hides_note_and_audits() {
        let (db, case_id) = setup();
        let created = note(&db, &case_id, "metin");
        db.delete_note(&created.id).expect("sil");
        assert!(db.list_notes(&case_id).expect("liste").is_empty());
        assert!(db.verify_audit_chain().expect("zincir") >= 2);
    }

    #[test]
    fn update_bumps_version_and_leaves_revision() {
        let (db, case_id) = setup();
        let created = note(&db, &case_id, "İlk metin");
        let updated = db
            .update_note(&created.id, created.row_version, &edit("Düzeltilmiş metin"))
            .expect("güncelle");
        assert_eq!(updated.body, "Düzeltilmiş metin");
        assert_eq!(updated.row_version, created.row_version + 1);
        assert_eq!(updated.revision_no, 2);
        assert!(updated.can_undo);
        assert!(!updated.can_redo);
        let revisions = db.list_note_revisions(&created.id).expect("revizyonlar");
        assert_eq!(revisions.len(), 2);
        assert_eq!(revisions[0].origin, "edit");
        assert_eq!(revisions[0].body, "Düzeltilmiş metin");
        assert_eq!(revisions[1].origin, "create");
        assert!(db.verify_audit_chain().expect("zincir") >= 3);
    }

    #[test]
    fn stale_update_is_rejected_without_writing() {
        let (db, case_id) = setup();
        let created = note(&db, &case_id, "İlk metin");
        db.update_note(&created.id, created.row_version, &edit("Ara kayıt"))
            .expect("güncelle");
        let stale = db.update_note(&created.id, created.row_version, &edit("Eski görünüm"));
        match stale {
            Err(StorageError::Conflict { expected, actual }) => {
                assert_eq!(expected, created.row_version);
                assert_eq!(actual, created.row_version + 1);
            }
            other => panic!("çakışma bekleniyordu: {other:?}"),
        }
        let current = db.get_note(&created.id).expect("not").expect("var");
        assert_eq!(current.body, "Ara kayıt");
        assert_eq!(current.revision_no, 2);
    }

    #[test]
    fn empty_update_body_rejected() {
        let (db, case_id) = setup();
        let created = note(&db, &case_id, "metin");
        assert!(matches!(
            db.update_note(&created.id, created.row_version, &edit("  ")),
            Err(StorageError::InvalidSetting)
        ));
        let current = db.get_note(&created.id).expect("not").expect("var");
        assert_eq!(current.row_version, created.row_version);
    }

    #[test]
    fn undo_restores_previous_text_and_redo_returns() {
        let (db, case_id) = setup();
        let created = note(&db, &case_id, "A");
        let updated = db
            .update_note(&created.id, created.row_version, &edit("B"))
            .expect("güncelle");
        let undone = db
            .undo_note(&created.id, updated.row_version)
            .expect("geri al");
        assert_eq!(undone.body, "A");
        assert!(!undone.can_undo);
        assert!(undone.can_redo);
        let redone = db
            .redo_note(&created.id, undone.row_version)
            .expect("ileri al");
        assert_eq!(redone.body, "B");
        assert!(redone.can_undo);
        assert!(!redone.can_redo);
        let revisions = db.list_note_revisions(&created.id).expect("revizyonlar");
        assert_eq!(
            revisions
                .iter()
                .map(|r| r.origin.as_str())
                .collect::<Vec<_>>(),
            vec!["redo", "undo", "edit", "create"]
        );
    }

    #[test]
    fn undo_without_history_is_refused_with_code() {
        let (db, case_id) = setup();
        let created = note(&db, &case_id, "tek sürüm");
        match db.undo_note(&created.id, created.row_version) {
            Err(error) => assert_eq!(error.code(), "E_NOTE_HISTORY"),
            Ok(_) => panic!("geri al boş yığınla çalışmamalı"),
        }
        let current = db.get_note(&created.id).expect("not").expect("var");
        assert_eq!(current.row_version, created.row_version);
    }

    #[test]
    fn edit_after_undo_drops_redo_branch() {
        let (db, case_id) = setup();
        let created = note(&db, &case_id, "A");
        let updated = db
            .update_note(&created.id, created.row_version, &edit("B"))
            .expect("güncelle");
        let undone = db
            .undo_note(&created.id, updated.row_version)
            .expect("geri al");
        assert!(undone.can_redo);
        let branched = db
            .update_note(&created.id, undone.row_version, &edit("C"))
            .expect("yeni dal");
        assert!(!branched.can_redo);
        assert!(branched.can_undo);
        assert_eq!(branched.body, "C");
        assert!(matches!(
            db.redo_note(&created.id, branched.row_version),
            Err(StorageError::NoteHistory(_))
        ));
    }

    #[test]
    fn undo_twice_walks_back_two_saves() {
        let (db, case_id) = setup();
        let created = note(&db, &case_id, "A");
        let second = db
            .update_note(&created.id, created.row_version, &edit("B"))
            .expect("güncelle");
        let third = db
            .update_note(&created.id, second.row_version, &edit("C"))
            .expect("güncelle");
        let back_one = db
            .undo_note(&created.id, third.row_version)
            .expect("geri al 1");
        assert_eq!(back_one.body, "B");
        let back_two = db
            .undo_note(&created.id, back_one.row_version)
            .expect("geri al 2");
        assert_eq!(back_two.body, "A");
        assert!(!back_two.can_undo);
    }

    #[test]
    fn history_survives_reopen_of_the_vault() {
        let dir = tempfile::tempdir().expect("dizin");
        let path = dir.path().join("vault.db");
        let (note_id, version_after_undo) = {
            let db = Database::open(&path, &DatabaseKey::new([23u8; 32])).expect("db");
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
            let created = db
                .create_note(
                    &case.id,
                    &NewNote {
                        title: "İlk izlenim".into(),
                        body: "A".into(),
                        source_anchor_ids: vec![],
                    },
                )
                .expect("not");
            let updated = db
                .update_note(&created.id, created.row_version, &edit("B"))
                .expect("güncelle");
            let undone = db
                .undo_note(&created.id, updated.row_version)
                .expect("geri al");
            (created.id, undone.row_version)
        };
        // Yeniden açılışta yığınlar kasa içinden okunur: ileri al hâlâ mümkün.
        let db = Database::open(&path, &DatabaseKey::new([23u8; 32])).expect("yeniden aç");
        let reopened = db.get_note(&note_id).expect("not").expect("var");
        assert_eq!(reopened.body, "A");
        assert!(reopened.can_redo);
        assert!(!reopened.can_undo);
        let redone = db
            .redo_note(&note_id, version_after_undo)
            .expect("ileri al");
        assert_eq!(redone.body, "B");
    }

    #[test]
    fn migration_backfill_sql_restores_first_revision() {
        // 0011 göçü, eski notlar için 1. revizyonu aynı ifadeyle doldurur.
        let (db, case_id) = setup();
        let created = note(&db, &case_id, "eski not");
        db.conn()
            .execute("DELETE FROM note_revisions WHERE note_id=?1", [&created.id])
            .expect("temizle");
        db.conn()
            .execute(
                "INSERT INTO note_revisions (note_id, revision_no, title, body_ast, source_anchor_ids, origin, created_at)
                 SELECT id, 1, title, body_ast, source_anchor_ids, 'create', created_at FROM notes WHERE id=?1",
                [&created.id],
            )
            .expect("doldur");
        let revisions = db.list_note_revisions(&created.id).expect("revizyonlar");
        assert_eq!(revisions.len(), 1);
        assert_eq!(revisions[0].origin, "create");
        assert_eq!(revisions[0].body, "eski not");
    }

    #[test]
    fn draft_roundtrip_is_case_scoped_and_clearable() {
        let (db, case_id) = setup();
        assert!(db.get_note_draft(&case_id, None).expect("taslak").is_none());
        let saved = db
            .save_note_draft(
                &case_id,
                None,
                &NoteDraftInput {
                    title: "Duruşma".into(),
                    body: "Yarım kalan cümle".into(),
                    source_anchor_ids: vec![],
                    base_row_version: None,
                },
            )
            .expect("taslak kaydı");
        assert_eq!(saved.body, "Yarım kalan cümle");
        assert_eq!(saved.note_id, None);

        let updated = db
            .save_note_draft(
                &case_id,
                None,
                &NoteDraftInput {
                    title: "Duruşma".into(),
                    body: "Yarım kalan cümle, devamı".into(),
                    source_anchor_ids: vec![],
                    base_row_version: None,
                },
            )
            .expect("taslak güncelle");
        assert_eq!(updated.body, "Yarım kalan cümle, devamı");
        assert_eq!(
            db.get_note_draft(&case_id, None)
                .expect("taslak")
                .expect("var")
                .body,
            "Yarım kalan cümle, devamı"
        );

        assert!(db.clear_note_draft(&case_id, None).expect("sil"));
        assert!(db.get_note_draft(&case_id, None).expect("taslak").is_none());
        assert!(!db.clear_note_draft(&case_id, None).expect("sil"));
    }

    #[test]
    fn draft_for_existing_note_keeps_base_version_and_rejects_foreign_note() {
        let (db, case_id) = setup();
        let created = note(&db, &case_id, "metin");
        let saved = db
            .save_note_draft(
                &case_id,
                Some(&created.id),
                &NoteDraftInput {
                    title: "İlk izlenim".into(),
                    body: "düzenleniyor".into(),
                    source_anchor_ids: vec![acme_anchor_id()],
                    base_row_version: Some(created.row_version),
                },
            )
            .expect("taslak");
        assert_eq!(saved.note_id.as_deref(), Some(created.id.as_str()));
        assert_eq!(saved.base_row_version, Some(created.row_version));
        assert_eq!(saved.source_anchor_ids.len(), 1);
        assert!(db
            .get_note_draft(&case_id, Some(&created.id))
            .expect("taslak")
            .is_some());

        // Başka dosyanın notu için taslak yazılamaz.
        let other_case = db
            .create_case(&NewCase {
                title: "Diğer".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("case");
        assert!(matches!(
            db.save_note_draft(
                &other_case.id,
                Some(&created.id),
                &NoteDraftInput {
                    title: String::new(),
                    body: "x".into(),
                    source_anchor_ids: vec![],
                    base_row_version: None,
                }
            ),
            Err(StorageError::NotFound)
        ));
    }

    fn acme_anchor_id() -> String {
        // Taslak yalnız kimlik taşır; anchor varlığı bu katmanda doğrulanmaz.
        "anchor-test".to_string()
    }
}
