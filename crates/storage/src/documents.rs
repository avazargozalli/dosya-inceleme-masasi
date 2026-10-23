//! Belge ve sürüm kayıtları. Özgün baytlar `blobs` içinde şifreli durur;
//! `document_versions` yalnız üst veriyi taşır ve immutable'dır.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

pub const MAX_DOCUMENT_BYTES: u64 = 250 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: String,
    pub case_id: String,
    pub display_name: String,
    pub document_type: String,
    pub status: String,
    pub current_version_id: Option<String>,
    pub declared_document_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentVersion {
    pub id: String,
    pub document_id: String,
    pub case_id: String,
    pub version_no: i64,
    pub blob_id: String,
    pub original_sha256: String,
    pub byte_size: i64,
    pub detected_mime: String,
    pub imported_at: String,
}

impl Database {
    /// Blob üst kaydı. Fiziksel dosya çağıran tarafından yazılmıştır; burada
    /// yalnız referans ve bütünlük durumu tutulur.
    pub fn create_blob_record(
        &self,
        storage_id: &str,
        encrypted_size: i64,
        aad_ref: &str,
    ) -> Result<String, StorageError> {
        let id = uuid::Uuid::now_v7().to_string();
        self.conn().execute(
            "INSERT INTO blobs (id,storage_id,cipher_version,encrypted_size,wrapped_key_ref,reference_count,integrity_status,created_at)
             VALUES (?1,?2,1,?3,?4,0,'verified',?5)",
            rusqlite::params![id, storage_id, encrypted_size, aad_ref, now_iso()],
        )?;
        Ok(id)
    }

    pub fn create_document(
        &self,
        case_id: &str,
        display_name: &str,
        document_type: &str,
        declared_document_date: Option<&str>,
        declared_date_precision: &str,
    ) -> Result<Document, StorageError> {
        let name = display_name.trim();
        if name.is_empty() || name.chars().count() > 300 {
            return Err(StorageError::InvalidSetting);
        }
        // Dosya kapsamı: hedef case gerçekten var mı?
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
            "INSERT INTO documents (id,case_id,display_name,document_type,declared_document_date,declared_date_precision,source_origin,status,created_at,updated_at,row_version)
             VALUES (?1,?2,?3,?4,?5,?6,'import','active',?7,?7,1)",
            rusqlite::params![id, case_id, name, document_type, declared_document_date, declared_date_precision, now],
        )?;
        self.append_audit(
            "document",
            Some(&id),
            "create",
            "local-user",
            &serde_json::json!({"caseId": case_id}),
        )?;
        self.get_document(&id)?.ok_or(StorageError::NotFound)
    }

    /// Yeni sürüm ekler ve `current_version_id`'yi günceller. Sürüm numarası
    /// aynı belge içinde artan ve benzersizdir.
    pub fn add_version(
        &self,
        document_id: &str,
        case_id: &str,
        blob_id: &str,
        sha256: &str,
        byte_size: i64,
        mime: &str,
    ) -> Result<DocumentVersion, StorageError> {
        self.add_version_with_parent(document_id, case_id, blob_id, sha256, byte_size, mime, None)
    }

    /// Türev sürüm (ör. maskeli kopya) için ebeveyn bağını korur.
    #[allow(clippy::too_many_arguments)] // alanlar tek tek anlamlı; struct sarmak çağrıyı gizler
    pub fn add_version_with_parent(
        &self,
        document_id: &str,
        case_id: &str,
        blob_id: &str,
        sha256: &str,
        byte_size: i64,
        mime: &str,
        parent_version_id: Option<&str>,
    ) -> Result<DocumentVersion, StorageError> {
        if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(StorageError::InvalidSetting);
        }
        let next: i64 = self.conn().query_row(
            "SELECT COALESCE(MAX(version_no),0)+1 FROM document_versions WHERE document_id=?1",
            [document_id],
            |r| r.get(0),
        )?;
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "INSERT INTO document_versions (id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at,is_original,parent_version_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            rusqlite::params![
                id,
                document_id,
                case_id,
                next,
                blob_id,
                sha256,
                byte_size,
                mime,
                now,
                i64::from(next == 1),
                parent_version_id
            ],
        )?;
        tx.execute(
            "UPDATE documents SET current_version_id=?1, updated_at=?2, row_version=row_version+1 WHERE id=?3",
            rusqlite::params![id, now, document_id],
        )?;
        tx.execute(
            "UPDATE blobs SET reference_count=reference_count+1 WHERE id=?1",
            [blob_id],
        )?;
        tx.commit()?;
        self.append_audit(
            "document",
            Some(document_id),
            "add_version",
            "local-user",
            &serde_json::json!({"version": next}),
        )?;
        self.get_version(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn get_document(&self, id: &str) -> Result<Option<Document>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,display_name,document_type,status,current_version_id,declared_document_date,created_at,updated_at,row_version
             FROM documents WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_document).optional()?)
    }

    pub fn list_documents(&self, case_id: &str) -> Result<Vec<Document>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,display_name,document_type,status,current_version_id,declared_document_date,created_at,updated_at,row_version
             FROM documents WHERE case_id=?1 AND deleted_at IS NULL ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([case_id], map_document)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_version(&self, id: &str) -> Result<Option<DocumentVersion>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at
             FROM document_versions WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_version).optional()?)
    }

    /// Aynı baytlar daha önce içe alınmış mı? Kapsam **aynı case** ile sınırlıdır;
    /// başka dosyadaki aynı içerik bu dosyaya otomatik bağlanmaz.
    pub fn find_version_by_sha(
        &self,
        case_id: &str,
        sha256: &str,
    ) -> Result<Option<DocumentVersion>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at
             FROM document_versions WHERE case_id=?1 AND original_sha256=?2 LIMIT 1",
        )?;
        Ok(stmt.query_row([case_id, sha256], map_version).optional()?)
    }

    /// İdempotent iş kaydı: aynı anahtar ikinci kez eklenmez.
    pub fn enqueue_job(
        &self,
        job_type: &str,
        case_id: Option<&str>,
        input_fingerprint: &str,
        idempotency_key: &str,
    ) -> Result<String, StorageError> {
        let existing: Option<String> = self
            .conn()
            .query_row(
                "SELECT id FROM jobs WHERE idempotency_key=?1",
                [idempotency_key],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            return Ok(id);
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        let scope = if case_id.is_some() { "case" } else { "vault" };
        self.conn().execute(
            "INSERT INTO jobs (id,type,scope,case_id,input_fingerprint,idempotency_key,state,progress,retry_policy,attempt_count,next_run_at,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,'queued',0,'{\"maxAttempts\":3,\"baseDelayMs\":2000}',0,?7,?7,?7)",
            rusqlite::params![id, job_type, scope, case_id, input_fingerprint, idempotency_key, now],
        )?;
        Ok(id)
    }
}

fn map_document(row: &Row<'_>) -> rusqlite::Result<Document> {
    Ok(Document {
        id: row.get(0)?,
        case_id: row.get(1)?,
        display_name: row.get(2)?,
        document_type: row.get(3)?,
        status: row.get(4)?,
        current_version_id: row.get(5)?,
        declared_document_date: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        row_version: row.get(9)?,
    })
}

fn map_version(row: &Row<'_>) -> rusqlite::Result<DocumentVersion> {
    Ok(DocumentVersion {
        id: row.get(0)?,
        document_id: row.get(1)?,
        case_id: row.get(2)?,
        version_no: row.get(3)?,
        blob_id: row.get(4)?,
        original_sha256: row.get(5)?,
        byte_size: row.get(6)?,
        detected_mime: row.get(7)?,
        imported_at: row.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::DatabaseKey;

    fn db_with_case() -> (Database, String) {
        let db = Database::open_in_memory(&DatabaseKey::new([5u8; 32])).expect("db");
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

    fn blob(db: &Database, tag: &str) -> String {
        db.create_blob_record(tag, 128, tag).expect("blob")
    }

    #[test]
    fn document_and_version_lifecycle() {
        let (db, case_id) = db_with_case();
        let doc = db
            .create_document(&case_id, "Kira sözleşmesi", "contract", None, "unknown")
            .expect("belge");
        let b = blob(&db, "s1");
        let sha = "a".repeat(64);
        let v = db
            .add_version(&doc.id, &case_id, &b, &sha, 1024, "application/pdf")
            .expect("sürüm");
        assert_eq!(v.version_no, 1);

        let updated = db.get_document(&doc.id).expect("get").expect("var");
        assert_eq!(updated.current_version_id.as_deref(), Some(v.id.as_str()));

        let b2 = blob(&db, "s2");
        let v2 = db
            .add_version(
                &doc.id,
                &case_id,
                &b2,
                &"b".repeat(64),
                2048,
                "application/pdf",
            )
            .expect("sürüm 2");
        assert_eq!(v2.version_no, 2);
    }

    #[test]
    fn duplicate_content_is_found_only_within_case() {
        let (db, case_id) = db_with_case();
        let doc = db
            .create_document(&case_id, "Belge", "unknown", None, "unknown")
            .expect("belge");
        let b = blob(&db, "s1");
        let sha = "c".repeat(64);
        db.add_version(&doc.id, &case_id, &b, &sha, 10, "text/plain")
            .expect("sürüm");

        assert!(db
            .find_version_by_sha(&case_id, &sha)
            .expect("ara")
            .is_some());

        let other = db
            .create_case(&NewCase {
                title: "Başka".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("case");
        assert!(db
            .find_version_by_sha(&other.id, &sha)
            .expect("ara")
            .is_none());
    }

    #[test]
    fn unknown_case_rejected() {
        let (db, _) = db_with_case();
        assert!(db
            .create_document("yok", "Belge", "unknown", None, "unknown")
            .is_err());
    }

    #[test]
    fn bad_sha_rejected() {
        let (db, case_id) = db_with_case();
        let doc = db
            .create_document(&case_id, "Belge", "unknown", None, "unknown")
            .expect("belge");
        let b = blob(&db, "s1");
        assert!(db
            .add_version(&doc.id, &case_id, &b, "kisa", 1, "text/plain")
            .is_err());
    }

    #[test]
    fn job_enqueue_is_idempotent() {
        let (db, case_id) = db_with_case();
        let a = db
            .enqueue_job("parse_document", Some(&case_id), "fp", "key-1")
            .expect("iş");
        let b = db
            .enqueue_job("parse_document", Some(&case_id), "fp", "key-1")
            .expect("iş");
        assert_eq!(a, b);
    }
}
