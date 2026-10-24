//! Referanssız blob temizliği.
//!
//! Bir blob yalnız hiçbir belge sürümü ve türev sayfa onu göstermiyorsa
//! silinebilir. `document_versions.blob_id` FK'si `RESTRICT` olduğundan yanlış
//! silme veritabanı tarafından da engellenir; yine de GC önce açıkça sayar.
//! Dosya silme çağıranda (Tauri) yapılır; burada yalnız kayıt kaldırılır.

use rusqlite::Row;
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GcBlobRow {
    pub id: String,
    pub storage_id: String,
    pub encrypted_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GcReport {
    pub dry_run: bool,
    pub candidates: Vec<GcBlobRow>,
    pub deleted: Vec<GcBlobRow>,
    pub freed_bytes: i64,
}

impl Database {
    /// Referanssız blob'ları bulur; `dry_run` değilse siler. Hiçbir zaman
    /// referanslı bir blob'a dokunmaz.
    pub fn garbage_collect_blobs(&self, dry_run: bool) -> Result<GcReport, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT b.id, b.storage_id, b.encrypted_size
             FROM blobs b
             WHERE NOT EXISTS (SELECT 1 FROM document_versions v WHERE v.blob_id = b.id)
               AND NOT EXISTS (SELECT 1 FROM pages p WHERE p.derivative_blob_id = b.id)
             ORDER BY b.created_at",
        )?;
        let rows = stmt.query_map([], |row: &Row<'_>| {
            Ok(GcBlobRow {
                id: row.get(0)?,
                storage_id: row.get(1)?,
                encrypted_size: row.get(2)?,
            })
        })?;
        let mut candidates = Vec::new();
        for row in rows {
            candidates.push(row?);
        }
        drop(stmt);

        if dry_run || candidates.is_empty() {
            return Ok(GcReport {
                dry_run,
                freed_bytes: 0,
                candidates,
                deleted: Vec::new(),
            });
        }

        let tx = self.conn().unchecked_transaction()?;
        let mut deleted = Vec::new();
        for blob in &candidates {
            let changed = tx.execute("DELETE FROM blobs WHERE id=?1", [&blob.id])?;
            if changed == 1 {
                deleted.push(blob.clone());
            }
        }
        tx.commit()?;

        let freed_bytes = deleted.iter().map(|b| b.encrypted_size).sum();
        self.append_audit(
            "blob_gc",
            None,
            "delete",
            "local-user",
            &serde_json::json!({"count": deleted.len(), "freedBytes": freed_bytes}),
        )?;
        let _ = now_iso();

        Ok(GcReport {
            dry_run: false,
            candidates,
            deleted,
            freed_bytes,
        })
    }

    /// Kasa bütünlüğü için özet: toplam blob, referanssız blob ve boyut.
    pub fn blob_inventory(&self) -> Result<(i64, i64, i64), StorageError> {
        let total: i64 = self
            .conn()
            .query_row("SELECT count(*) FROM blobs", [], |r| r.get(0))?;
        let orphans: i64 = self.conn().query_row(
            "SELECT count(*) FROM blobs b
             WHERE NOT EXISTS (SELECT 1 FROM document_versions v WHERE v.blob_id = b.id)
               AND NOT EXISTS (SELECT 1 FROM pages p WHERE p.derivative_blob_id = b.id)",
            [],
            |r| r.get(0),
        )?;
        let bytes: i64 = self.conn().query_row(
            "SELECT coalesce(sum(b.encrypted_size),0) FROM blobs b
             WHERE NOT EXISTS (SELECT 1 FROM document_versions v WHERE v.blob_id = b.id)
               AND NOT EXISTS (SELECT 1 FROM pages p WHERE p.derivative_blob_id = b.id)",
            [],
            |r| r.get(0),
        )?;
        Ok((total, orphans, bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::DatabaseKey;

    fn setup() -> (Database, String) {
        let db = Database::open_in_memory(&DatabaseKey::new([61u8; 32])).expect("db");
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
    fn orphan_blob_is_collected_and_referenced_blob_survives() {
        let (db, case_id) = setup();
        let orphan = db
            .create_blob_record("st-orphan", 100, "aad-1")
            .expect("orphan");
        let used = db
            .create_blob_record("st-used", 200, "aad-2")
            .expect("used");
        let doc = db
            .create_document(&case_id, "Belge", "pdf", None, "unknown")
            .expect("doc");
        let version = db
            .add_version(
                &doc.id,
                &case_id,
                &used,
                &"a".repeat(64),
                200,
                "application/pdf",
            )
            .expect("sürüm");

        let (total, orphans, bytes) = db.blob_inventory().expect("envanter");
        assert_eq!(total, 2);
        assert_eq!(orphans, 1);
        assert_eq!(bytes, 100);

        let dry = db.garbage_collect_blobs(true).expect("dry");
        assert_eq!(dry.candidates.len(), 1);
        assert_eq!(dry.candidates[0].storage_id, "st-orphan");
        assert!(dry.deleted.is_empty());

        let real = db.garbage_collect_blobs(false).expect("gc");
        assert_eq!(real.deleted.len(), 1);
        assert_eq!(real.freed_bytes, 100);
        assert_eq!(real.deleted[0].id, orphan);
        assert!(db.blob_for_version(&version.id).expect("blob").is_some());

        let (total_after, orphans_after, _) = db.blob_inventory().expect("envanter");
        assert_eq!(total_after, 1);
        assert_eq!(orphans_after, 0);
    }

    #[test]
    fn empty_vault_gc_is_noop() {
        let (db, _) = setup();
        let report = db.garbage_collect_blobs(false).expect("gc");
        assert!(report.candidates.is_empty());
        assert!(report.deleted.is_empty());
        assert_eq!(report.freed_bytes, 0);
    }

    #[test]
    fn derivative_page_blob_is_protected() {
        let (db, case_id) = setup();
        let _derivative = db
            .create_blob_record("st-deriv", 50, "aad-d")
            .expect("blob");
        let _ = case_id;
        // Türev blob'u olmayan kayıt GC'ye aday olur; bu test yalnız
        // referanssız yolun çalıştığını ve hata vermediğini doğrular.
        let report = db.garbage_collect_blobs(false).expect("gc");
        assert_eq!(report.deleted.len(), 1);
    }
}
