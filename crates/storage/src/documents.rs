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

/// Ayrıştırma bekleyen belge: güncel sürümü var, o sürüm için `succeeded` ya da
/// `partial` bir ayrıştırma koşusu **yok** (§17.3 toplu ayrıştırma girdisi).
///
/// Sıralama `created_at` + kimlik: aynı saniyede açılan belgelerde sıra
/// deterministiktir (zaman damgası çözünürlüğü tek başına yetmez).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingParse {
    pub document_id: String,
    pub display_name: String,
    pub version_id: String,
    pub byte_size: i64,
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
        // Özet **küçük harf** hex olmalı: şema kapısı da (0052) aynı ölçüyü
        // kullanır ve `sha256_hex` zaten küçük harf üretir. Büyük harf kabul
        // edilseydi aynı baytlar iki farklı özetle yazılır, `find_version_by_sha`
        // eşleşmez ve yazma şema kapısında `E_DB_INTEGRITY` ile düşerdi.
        if sha256.len() != 64
            || !sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(StorageError::InvalidSetting);
        }
        // Kapsam **çağıranın sözüne bırakılmaz** (INV-05): sürüm, ancak kendi
        // belgesinin dosyası adına yazılır. Aksi hâlde `document_versions` bir
        // dosyanın sürümünü başka dosya altında kaydeder, belgenin
        // `current_version_id`'si çapraz dosyaya döner ve kapsam raporu boş bir
        // dosyada sürüm sayar. Ölçüldü (2026-11-14): yanlış `case_id` ile
        // yazılan sürüm kabul ediliyordu.
        let owner_case = self.document_case(document_id)?;
        if owner_case != case_id {
            return Err(StorageError::NotFound);
        }
        // Ebeveyn **aynı dosyada** olmalı. Türev sürümün ebeveyni başka bir
        // *belgenin* sürümü olabilir (maskeli kopya yeni belge açar), ama asla
        // başka bir *dosyanın* sürümü olamaz — soy ağacı dosya sınırını aşmaz.
        if let Some(parent_id) = parent_version_id {
            let parent_case: Option<String> = self
                .conn()
                .query_row(
                    "SELECT case_id FROM document_versions WHERE id=?1",
                    [parent_id],
                    |r| r.get(0),
                )
                .optional()?;
            if parent_case.as_deref() != Some(case_id) {
                return Err(StorageError::NotFound);
            }
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
            "SELECT id,case_id,display_name,document_type,status,
                    (SELECT cv.version_id FROM current_document_versions cv WHERE cv.document_id=documents.id),
                    declared_document_date,created_at,updated_at,row_version
             FROM documents WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_document).optional()?)
    }

    /// Belgenin **dosyası** (silinmemiş satırdan). Kapsam kapılarının tek
    /// kaynağı: çağıranın verdiği `case_id` bu değerle karşılaştırılır.
    pub(crate) fn document_case(&self, document_id: &str) -> Result<String, StorageError> {
        self.conn()
            .query_row(
                "SELECT case_id FROM documents WHERE id=?1 AND deleted_at IS NULL",
                [document_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(StorageError::NotFound)
    }

    pub fn list_documents(&self, case_id: &str) -> Result<Vec<Document>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,display_name,document_type,status,
                    (SELECT cv.version_id FROM current_document_versions cv WHERE cv.document_id=documents.id),
                    declared_document_date,created_at,updated_at,row_version
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

    /// Bir belgenin sürümleri, en yeniden eskiye. Arayüz "hangi sürümden metin
    /// aktarılacak" seçimini bu listeden yapar (F12-T13); sürüm seçimi tahmin
    /// edilmez, kullanıcı seçer.
    pub fn list_versions(&self, document_id: &str) -> Result<Vec<DocumentVersion>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,document_id,case_id,version_no,blob_id,original_sha256,byte_size,detected_mime,imported_at
             FROM document_versions WHERE document_id=?1 ORDER BY version_no DESC",
        )?;
        let rows = stmt.query_map([document_id], map_version)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
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
        // Kural tek yerde: kuyruk modülü (varsayılan öncelik 0).
        self.enqueue_job_with_priority(job_type, case_id, input_fingerprint, idempotency_key, 0)
    }

    /// **§17.3 toplu ayrıştırma girdisi:** güncel sürümü olup o sürüm için
    /// `succeeded`/`partial` ayrıştırma koşusu bulunmayan belgeler.
    ///
    /// - Silinmiş belgeler listelenmez.
    /// - `failed` koşusu olan belge **bekliyordur** (yeniden denenebilir).
    /// - Sıra deterministik: `created_at`, sonra belge kimliği.
    /// - `limit` 1..=50 aralığına kırpılır; sessizce sınırsız liste dönmez.
    pub fn documents_pending_parse(
        &self,
        case_id: &str,
        limit: i64,
    ) -> Result<Vec<PendingParse>, StorageError> {
        let limit = limit.clamp(1, 50);
        let mut stmt = self.conn().prepare(
            "SELECT d.id, d.display_name, v.version_id, v.byte_size
             FROM documents d
             JOIN current_document_versions v ON v.document_id = d.id
             WHERE d.case_id = ?1 AND d.deleted_at IS NULL
               AND NOT EXISTS (
                 SELECT 1 FROM parse_runs r
                 WHERE r.document_version_id = v.version_id
                   AND r.status IN ('succeeded','partial')
               )
             ORDER BY d.created_at, d.id
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![case_id, limit], |row| {
            Ok(PendingParse {
                document_id: row.get(0)?,
                display_name: row.get(1)?,
                version_id: row.get(2)?,
                byte_size: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
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

        // F12-T13: sürüm seçimi arayüzde bu listeden yapılır — en yeniden eskiye.
        let listed = db.list_versions(&doc.id).expect("sürümler");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, v2.id);
        assert_eq!(listed[1].id, v.id);
        assert!(db.list_versions("yok").expect("boş").is_empty());
    }

    /// Yardımcı: ikinci bir dosya. Kapsam kapıları iki dosya arasında ölçülür.
    fn second_case(db: &Database) -> String {
        db.create_case(&NewCase {
            title: "Diğer dosya".into(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: crate::Sensitivity::Normal,
            default_ai_policy: crate::AiPolicy::Off,
        })
        .expect("ikinci dosya")
        .id
    }

    fn version_count(db: &Database) -> i64 {
        db.conn()
            .query_row("SELECT count(*) FROM document_versions", [], |r| r.get(0))
            .expect("sayım")
    }

    fn blob_refs(db: &Database, blob_id: &str) -> i64 {
        db.conn()
            .query_row(
                "SELECT reference_count FROM blobs WHERE id=?1",
                [blob_id],
                |r| r.get(0),
            )
            .expect("blob")
    }

    /// **F05-T03b (2026-11-14):** sürüm, ancak kendi belgesinin dosyası adına
    /// yazılır. Ölçülen kusur: yanlış `case_id` ile yazılan sürüm kabul
    /// ediliyor, belgenin `current_version_id`'si o satıra dönüyor ve kapsam
    /// raporu **boş** bir dosyada sürüm sayıyordu.
    #[test]
    fn version_is_written_only_under_its_own_case() {
        let (db, case_id) = db_with_case();
        let other = second_case(&db);
        let doc = db
            .create_document(&case_id, "Belge", "pdf", None, "unknown")
            .expect("belge");
        let b = blob(&db, "s1");
        let v1 = db
            .add_version(
                &doc.id,
                &case_id,
                &b,
                &"a".repeat(64),
                10,
                "application/pdf",
            )
            .expect("v1");
        let versions_before = version_count(&db);
        let refs_before = blob_refs(&db, &b);

        let err = db
            .add_version(&doc.id, &other, &b, &"b".repeat(64), 10, "application/pdf")
            .expect_err("yanlış dosya reddedilir");
        assert_eq!(err.code(), "E_NOT_FOUND");
        // Reddedilen yazma **hiçbir** iz bırakmaz: satır yok, işaretçi yok,
        // blob sayacı artmadı, sürüm numarası tüketilmedi.
        assert_eq!(version_count(&db), versions_before);
        assert_eq!(blob_refs(&db, &b), refs_before);
        let doc_after = db.get_document(&doc.id).expect("get").expect("var");
        assert_eq!(
            doc_after.current_version_id.as_deref(),
            Some(v1.id.as_str())
        );

        // Aynı dosya adına yazmak hâlâ çalışır (kapı yalnız yanlışı keser).
        let v2 = db
            .add_version(
                &doc.id,
                &case_id,
                &b,
                &"c".repeat(64),
                10,
                "application/pdf",
            )
            .expect("v2");
        assert_eq!(v2.version_no, 2);

        // Silinmiş belgeye sürüm eklenemez: kapı silme izini de okur.
        db.conn()
            .execute(
                "UPDATE documents SET deleted_at=?1 WHERE id=?2",
                rusqlite::params![now_iso(), doc.id],
            )
            .expect("sil");
        let err = db
            .add_version(
                &doc.id,
                &case_id,
                &b,
                &"f".repeat(64),
                10,
                "application/pdf",
            )
            .expect_err("silinmiş belge reddedilir");
        assert_eq!(err.code(), "E_NOT_FOUND");
        assert_eq!(version_count(&db), versions_before + 1);
    }

    /// **F05-T03b:** türev sürümün ebeveyni aynı **dosyada** olmalı. Maskeli
    /// kopya yeni bir *belge* açar, yani ebeveyn başka belgenin sürümü
    /// olabilir; ama başka *dosyanın* sürümü olamaz — soy ağacı dosya sınırını
    /// aşmaz.
    #[test]
    fn derived_version_parent_stays_inside_the_case() {
        let (db, case_id) = db_with_case();
        let other = second_case(&db);
        let sha = "a".repeat(64);
        let source = db
            .create_document(&case_id, "Kaynak", "pdf", None, "unknown")
            .expect("kaynak");
        let source_version = db
            .add_version(
                &source.id,
                &case_id,
                &blob(&db, "s1"),
                &sha,
                10,
                "application/pdf",
            )
            .expect("kaynak sürüm");
        let target = db
            .create_document(&case_id, "Maskeli kopya", "pdf", None, "unknown")
            .expect("hedef");
        // 1) Başka dosyanın sürümü ebeveyn olamaz.
        let foreign = db
            .create_document(&other, "Yabancı", "pdf", None, "unknown")
            .expect("yabancı");
        let foreign_version = db
            .add_version(
                &foreign.id,
                &other,
                &blob(&db, "s2"),
                &"b".repeat(64),
                10,
                "application/pdf",
            )
            .expect("yabancı sürüm");
        let err = db
            .add_version_with_parent(
                &target.id,
                &case_id,
                &blob(&db, "s3"),
                &"c".repeat(64),
                10,
                "application/pdf",
                Some(&foreign_version.id),
            )
            .expect_err("çapraz dosya ebeveyn reddedilir");
        assert_eq!(err.code(), "E_NOT_FOUND");
        // 2) Var olmayan ebeveyn de reddedilir (uydurma bağ yazılmaz).
        let err = db
            .add_version_with_parent(
                &target.id,
                &case_id,
                &blob(&db, "s4"),
                &"d".repeat(64),
                10,
                "application/pdf",
                Some("olmayan-sürüm"),
            )
            .expect_err("olmayan ebeveyn reddedilir");
        assert_eq!(err.code(), "E_NOT_FOUND");
        // 3) Aynı dosyadaki başka belgenin sürümü ebeveyn olabilir (maskeli
        //    kopya yolu budur) ve bağ gerçekten yazılır.
        let masked = db
            .add_version_with_parent(
                &target.id,
                &case_id,
                &blob(&db, "s5"),
                &"e".repeat(64),
                10,
                "application/pdf",
                Some(&source_version.id),
            )
            .expect("aynı dosyada türev");
        let parent: Option<String> = db
            .conn()
            .query_row(
                "SELECT parent_version_id FROM document_versions WHERE id=?1",
                [&masked.id],
                |r| r.get(0),
            )
            .expect("ebeveyn");
        assert_eq!(parent.as_deref(), Some(source_version.id.as_str()));
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

    /// **§17.3 toplu ayrıştırma girdisi:** yalnız `succeeded`/`partial` koşusu
    /// olan sürümler "ayrıştırıldı" sayılır. `running` ve `failed` koşular
    /// belgeyi **bekleyen** bırakır (failed yeniden denenebilir).
    #[test]
    fn pending_parse_lists_only_unparsed_current_versions() {
        let (db, case_id) = db_with_case();
        let doc = db
            .create_document(&case_id, "Bekleyen", "contract", None, "unknown")
            .expect("belge");
        let b = blob(&db, "p1");
        let version = db
            .add_version(
                &doc.id,
                &case_id,
                &b,
                &"b".repeat(64),
                2048,
                "application/pdf",
            )
            .expect("sürüm");

        let pending = db.documents_pending_parse(&case_id, 10).expect("liste");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].document_id, doc.id);
        assert_eq!(pending[0].version_id, version.id);
        assert_eq!(pending[0].byte_size, 2048);
        assert_eq!(pending[0].display_name, "Bekleyen");

        let run = db
            .start_parse_run(&version.id, &case_id, "pymupdf", "1.24", "application/pdf")
            .expect("koşu");
        assert_eq!(
            db.documents_pending_parse(&case_id, 10).unwrap().len(),
            1,
            "yarım (running) koşu belgeyi ayrıştırılmış saymaz"
        );
        db.finish_parse_run(&run.id, "succeeded", &[])
            .expect("bitti");
        assert!(
            db.documents_pending_parse(&case_id, 10).unwrap().is_empty(),
            "başarılı koşudan sonra beklemez"
        );

        // Eski **başarılı** koşu dururken yeni bir başarısız deneme belgeyi
        // "bekleyen" yapmaz: belgenin metni var, toplu ayrıştırma onu atlar.
        let failed = db
            .start_parse_run(&version.id, &case_id, "pymupdf", "1.24", "application/pdf")
            .expect("koşu");
        db.finish_parse_run(&failed.id, "failed", &["bozuk".to_string()])
            .expect("bitti");
        assert!(
            db.documents_pending_parse(&case_id, 10).unwrap().is_empty(),
            "başarılı koşu geçmişi varsa belge beklemez"
        );

        // Yalnız **başarısız** koşusu olan belge bekliyordur (yeniden denenebilir).
        let other = db
            .create_document(&case_id, "Bozuk", "contract", None, "unknown")
            .expect("belge");
        let b2 = blob(&db, "p2");
        let other_version = db
            .add_version(
                &other.id,
                &case_id,
                &b2,
                &"c".repeat(64),
                512,
                "application/pdf",
            )
            .expect("sürüm");
        let failed_only = db
            .start_parse_run(
                &other_version.id,
                &case_id,
                "pymupdf",
                "1.24",
                "application/pdf",
            )
            .expect("koşu");
        db.finish_parse_run(&failed_only.id, "failed", &["bozuk".to_string()])
            .expect("bitti");
        let pending = db.documents_pending_parse(&case_id, 10).expect("liste");
        assert_eq!(pending.len(), 1, "yalnız başarısız koşulu belge beklemede");
        assert_eq!(pending[0].document_id, other.id);

        // Limit kırpılır: sessizce sınırsız liste dönmez.
        assert_eq!(db.documents_pending_parse(&case_id, 0).unwrap().len(), 1);
        assert_eq!(db.documents_pending_parse(&case_id, 500).unwrap().len(), 1);
    }

    /// Sürüm satırının **kimlik künyesi** doğuşta tutarlı olmalı ve sonrasında
    /// değişmemeli (v52). Çekirdek kapısı değil **şema** kapısı ölçülür: kusur
    /// yazımları doğrudan SQL iledir (INV-05: doğrudan IPC/SQL yolu).
    ///
    /// Neden gerekli: `redaction_plans.render_hash` doğuşta bu özetle **eşit**
    /// olmak zorunda (0051) ve `render_changed` uyarısı yalnız bu sütunla
    /// hesaplanır; `find_version_by_sha` aynı baytı bu sütunla tanır. Özet,
    /// blob ya da boyut sonradan çevrilebilirse bütün zincir «başka bir
    /// dosya» hakkında konuşur.
    #[test]
    fn document_version_identity_is_frozen_and_well_formed() {
        let (db, case_id) = db_with_case();
        let doc = db
            .create_document(&case_id, "Kira", "contract", None, "unknown")
            .expect("belge");
        let b1 = blob(&db, "v1");
        let b2 = blob(&db, "v2");
        let first = db
            .add_version(
                &doc.id,
                &case_id,
                &b1,
                &"a".repeat(64),
                1024,
                "application/pdf",
            )
            .expect("tohum sürüm");

        let gate = |sql: &str| match db.conn().execute(sql, []) {
            Ok(rows) => panic!("v52 kapısı reddetmeli ({rows} satır): {sql}"),
            Err(err) => {
                let text = err.to_string();
                assert!(
                    text.contains("DIM_INTEGRITY:document_versions"),
                    "ret kimlik kapısından gelmeli: {text} ({sql})"
                );
            }
        };
        // Doğum: her deneme **kendi kusuruyla** reddedilmeli (UNIQUE/FK değil).
        let ins = |id: &str, sha: &str, mime: &str, imported: &str| {
            format!(
                "INSERT INTO document_versions (id,document_id,case_id,version_no,blob_id,\
                 original_sha256,byte_size,detected_mime,imported_at,is_original) \
                 VALUES ('{id}','{doc}','{case}','7','{b1}','{sha}',10,'{mime}','{imported}',0)",
                doc = doc.id,
                case = case_id,
                b1 = b1,
            )
        };
        let hex = "a".repeat(64);
        let born = [
            (
                "boş kimlik",
                ins("", &hex, "application/pdf", "2026-01-01T00:00:00Z"),
            ),
            (
                "özet 64 hex değil",
                ins(
                    "v-bad-1",
                    &"Z".repeat(64),
                    "application/pdf",
                    "2026-01-01T00:00:00Z",
                ),
            ),
            (
                "özet büyük harf hex",
                ins(
                    "v-bad-2",
                    &"A".repeat(64),
                    "application/pdf",
                    "2026-01-01T00:00:00Z",
                ),
            ),
            ("mime boş", ins("v-bad-3", &hex, "", "2026-01-01T00:00:00Z")),
            (
                "mime yalnız boşluk",
                ins("v-bad-4", &hex, "   ", "2026-01-01T00:00:00Z"),
            ),
            (
                "mime kırpılmamış",
                ins("v-bad-5", &hex, " application/pdf", "2026-01-01T00:00:00Z"),
            ),
            ("damga boş", ins("v-bad-6", &hex, "application/pdf", "")),
            (
                "damga ISO değil",
                ins("v-bad-7", &hex, "application/pdf", "dün"),
            ),
        ];
        for (why, sql) in born {
            let written: i64 = db
                .conn()
                .query_row("SELECT count(*) FROM document_versions", [], |r| r.get(0))
                .expect("sayım");
            assert_eq!(written, 1, "{why}: satır yazılmamalı");
            gate(&sql);
        }

        // Güncelleme: kimlik künyesinin dokuz sütunu donar.
        db.conn()
            .execute(
                &ins("v-plain", &hex, "application/pdf", "2026-01-01T00:00:00Z"),
                [],
            )
            .expect("meşru sürüm");
        db.conn()
            .execute(
                &format!(
                    "INSERT INTO document_versions (id,document_id,case_id,version_no,blob_id,\
                     original_sha256,byte_size,detected_mime,imported_at,is_original,\
                     parent_version_id) \
                     VALUES ('v-child','{doc}','{case}','8','{b1}','{hex}',10,\
                     'application/pdf','2026-01-01T00:00:00Z',0,'{parent}')",
                    doc = doc.id,
                    case = case_id,
                    b1 = b1,
                    parent = first.id,
                    hex = hex,
                ),
                [],
            )
            .expect("türev sürüm");
        let attacks = [
            (
                "özeti çevir",
                format!("original_sha256='{}'", "f".repeat(64)),
            ),
            ("blob'u başka baytlara çevir", format!("blob_id='{b2}'")),
            ("boyutu çevir", "byte_size=99".to_string()),
            ("mime'ı çevir", "detected_mime='text/plain'".to_string()),
            (
                "damgayı çevir",
                "imported_at='2026-06-01T00:00:00Z'".to_string(),
            ),
            ("'asıl sürüm' işaretini çevir", "is_original=1".to_string()),
            ("sürüm numarasını çevir", "version_no=9".to_string()),
            (
                "asıl dosya adını sonradan ekle",
                "original_filename_encrypted=X'00'".to_string(),
            ),
            ("sürüm kimliğini çevir", "id='v-swapped'".to_string()),
        ];
        for (why, sets) in attacks {
            match db.conn().execute(
                &format!("UPDATE document_versions SET {sets} WHERE id='v-plain'"),
                [],
            ) {
                Ok(rows) => panic!("v52 {why} yazımını durdurmalı ({rows} satır)"),
                Err(err) => assert!(
                    err.to_string().contains("DIM_INTEGRITY:document_versions"),
                    "ret kimlik kapısından gelmeli ({why}): {err}"
                ),
            }
        }
        // Değişmeyen yazma ve **meşru** ebeveyn çözülmesi takılmaz.
        let same = db
            .conn()
            .execute(
                "UPDATE document_versions SET original_sha256=original_sha256, \
                 detected_mime=detected_mime WHERE id='v-plain'",
                [],
            )
            .expect("no-op");
        assert_eq!(same, 1);
        let released = db
            .conn()
            .execute(
                "UPDATE document_versions SET parent_version_id=NULL WHERE id='v-child'",
                [],
            )
            .expect("ebeveyn çözüldü");
        assert_eq!(released, 1);

        // Saldırılardan sonra satır **hiç değişmemiş** olmalı.
        let (sha, size, mime, no, child_parent): (String, i64, String, i64, Option<String>) = db
            .conn()
            .query_row(
                "SELECT original_sha256, byte_size, detected_mime, version_no, \
                 (SELECT parent_version_id FROM document_versions WHERE id='v-child') \
                 FROM document_versions WHERE id='v-plain'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .expect("satır");
        assert_eq!(sha, hex);
        assert_eq!(size, 10);
        assert_eq!(mime, "application/pdf");
        assert_eq!(no, 7);
        assert_eq!(child_parent, None);

        // Çekirdek kapısı şemayla **aynı** ölçüyü kullanır: büyük harf özet
        // `InvalidSetting` (şema hatası değil, yazma hiç denenmez).
        let err = db
            .add_version(
                &doc.id,
                &case_id,
                &b2,
                &"A".repeat(64),
                10,
                "application/pdf",
            )
            .expect_err("büyük harf özet reddedilmeli");
        assert_eq!(err.code(), "E_DB_SETTING");
    }
}
