//! Referanssız blob temizliği.
//!
//! Bir blob yalnız hiçbir belge sürümü ve türev sayfa onu göstermiyorsa
//! silinebilir. `document_versions.blob_id` FK'si `RESTRICT` olduğundan yanlış
//! silme veritabanı tarafından da engellenir; yine de GC önce açıkça sayar.
//!
//! **Dosya silme çağıranda (Tauri) yapılır** ve bu yüzden sıra önemlidir:
//! önce dosya, sonra kayıt. Ters sırada dosya silinemezse geriye veritabanında
//! izi olmayan bir dosya kalır — hiçbir envanter onu göremez (bkz.
//! `blob_disk`). Bu yüzden bu modül adayları (`orphan_blob_candidates`) ve kayıt
//! silmeyi (`delete_unreferenced_blobs`) ayrı sunar; `garbage_collect_blobs`
//! ikisini birleştiren eski kapıdır.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::{Database, StorageError};

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

/// Artık blob'ların **dosyalarıyla birlikte** toplanmasının raporu.
///
/// Sıra **dosya → kayıt**tır (bkz. modül dokümanı). Her aday için silme anında
/// referans yeniden sorulur; arada referans doğduysa **ne dosyaya ne kayda**
/// dokunulur ve bu `kept_referenced`ta görünür.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GcFileReport {
    pub dry_run: bool,
    /// Gerçekten silinen kayıtlar.
    pub deleted: Vec<GcBlobRow>,
    /// Diskte dosyası bulunmayan adaylar (`storage_id: neden`); kaydı düşer.
    pub missing_files: Vec<String>,
    /// **Korundu**: arada referans doğdu, dosya silinmedi.
    pub kept_referenced: Vec<String>,
    /// Silinemedi (symlink/dizin/izin) ya da koşullu silme kaydı korudu.
    pub blocked: Vec<String>,
    pub freed_bytes: i64,
}

/// Referanssız blob koşulu: **şemadaki her blob sütunu** bu blob'u göstermiyorsa
/// referanssızdır. **Tek kaynak** — aday seçimi, silme kapısı ve envanter aynı
/// ifadeyi kullanır; kopyalar ayrışırsa "aday" ile "silinebilir" farklı kümeler
/// olur.
///
/// Bugün **dört** sütun blob kimliği taşıyabilir; ikisi FK ile korunur
/// (`document_versions.blob_id` RESTRICT, `pages.derivative_blob_id` SET NULL),
/// ikisinin **FK'si yoktur** (`render_derivatives.blob_id`,
/// `ai_cache_entries.output_blob_id`). FK'siz sütunlar bu koşulda sayılmazsa GC
/// onları "referanssız" sanar: kayıt silinir, disk tarafı da dosyayı süpürür →
/// canlı bir türev/önbellek blob'u sessizce yok olur. Bu yüzden koşul şemanın
/// tamamı üzerinden kapalıdır; yeni bir blob sütunu eklenirse
/// `every_blob_column_is_covered_by_the_reference_gate` testi düşer.
const UNREFERENCED: &str = "NOT EXISTS (SELECT 1 FROM document_versions v WHERE v.blob_id = b.id)
       AND NOT EXISTS (SELECT 1 FROM pages p WHERE p.derivative_blob_id = b.id)
       AND NOT EXISTS (SELECT 1 FROM render_derivatives d WHERE d.blob_id = b.id)
       AND NOT EXISTS (SELECT 1 FROM ai_cache_entries c WHERE c.output_blob_id = b.id)";

impl Database {
    /// Referanssız blob kayıtlarını **seçer**, hiçbir şey silmez.
    pub fn orphan_blob_candidates(&self) -> Result<Vec<GcBlobRow>, StorageError> {
        let sql = format!(
            "SELECT b.id, b.storage_id, b.encrypted_size FROM blobs b WHERE {UNREFERENCED} ORDER BY b.created_at"
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map([], |row: &Row<'_>| {
            Ok(GcBlobRow {
                id: row.get(0)?,
                storage_id: row.get(1)?,
                encrypted_size: row.get(2)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Verilen kayıtları **yalnız hâlâ referanssızsa** siler; iki adım arasında
    /// referans doğmuşsa satır korunur. Tek işlemde koşar; denetim yalnız sayı
    /// taşır. Gerçekten silinen satır sayısını döndürür.
    pub fn delete_unreferenced_blobs(&self, ids: &[String]) -> Result<usize, StorageError> {
        if ids.is_empty() {
            return Ok(0);
        }
        let lookup =
            format!("SELECT b.encrypted_size FROM blobs b WHERE b.id = ?1 AND {UNREFERENCED}");
        let tx = self.conn().unchecked_transaction()?;
        let mut deleted = 0usize;
        let mut freed = 0i64;
        for id in ids {
            let size: Option<i64> = tx.query_row(&lookup, [id], |row| row.get(0)).optional()?;
            let Some(size) = size else { continue };
            if tx.execute("DELETE FROM blobs WHERE id=?1", [id])? == 1 {
                deleted += 1;
                freed = freed.saturating_add(size);
            }
        }
        tx.commit()?;

        if deleted > 0 {
            self.append_audit(
                "blob_gc",
                None,
                "delete",
                "local-user",
                &serde_json::json!({"count": deleted, "freedBytes": freed}),
            )?;
        }
        Ok(deleted)
    }

    /// Bir blob kaydı **bugün** herhangi bir yerden gösteriliyor mu? Silme anı
    /// kapısı. Ayrı bir SQL yazılmaz: cevap [`UNREFERENCED`]in olumsuzudur, yani
    /// "aday" ile "silinebilir" aynı ifadeyi kullanmaya devam eder.
    pub fn blob_is_referenced(&self, id: &str) -> Result<bool, StorageError> {
        let sql = format!("SELECT count(*) FROM blobs b WHERE b.id = ?1 AND NOT ({UNREFERENCED})");
        let count: i64 = self
            .conn()
            .query_row(&sql, [id], |row: &Row<'_>| row.get(0))?;
        Ok(count > 0)
    }

    /// Artık blob'ları **dosyalarıyla birlikte** toplar: aday seçimi, dosya
    /// silme ve koşullu kayıt silme tek yerde.
    ///
    /// Sıra bilinçli: **önce dosya, sonra kayıt**. Ters sırada dosya
    /// silinemezse geriye veritabanında izi olmayan bir `.blb` kalırdı — hiçbir
    /// envanter onu göremez. Dosyası olmayan adayın kaydı düşer (diskte veri
    /// yok) ama **sessizce** değil: `missing_files`ta bildirilir.
    pub fn collect_orphan_blob_files(
        &self,
        dir: &Path,
        dry_run: bool,
    ) -> Result<GcFileReport, StorageError> {
        let candidates = self.orphan_blob_candidates()?;
        let mut report = GcFileReport {
            dry_run,
            ..GcFileReport::default()
        };
        if dry_run {
            return Ok(report);
        }

        let mut files_removed = Vec::new();
        for blob in &candidates {
            if self.blob_is_referenced(&blob.id)? {
                // Aradaki referans: dosya ve kayıt **korunur**.
                report
                    .kept_referenced
                    .push(format!("{}: referans doğdu", blob.storage_id));
                continue;
            }
            let path = dir.join(format!("{}.blb", blob.storage_id));
            match std::fs::symlink_metadata(&path) {
                Ok(meta) if meta.file_type().is_file() => match std::fs::remove_file(&path) {
                    Ok(()) => {
                        report.freed_bytes = report.freed_bytes.saturating_add(blob.encrypted_size);
                        files_removed.push(blob.clone());
                    }
                    Err(error) => report.blocked.push(format!("{}: {error}", blob.storage_id)),
                },
                // Symlink/dizin: kapsam dışı, dokunulmaz.
                Ok(_) => report
                    .blocked
                    .push(format!("{}: dosya değil", blob.storage_id)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    report
                        .missing_files
                        .push(format!("{}: dosya yok", blob.storage_id));
                    files_removed.push(blob.clone());
                }
                Err(error) => report.blocked.push(format!("{}: {error}", blob.storage_id)),
            }
        }

        let ids: Vec<String> = files_removed.iter().map(|b| b.id.clone()).collect();
        self.delete_unreferenced_blobs(&ids)?;
        // Rapor **gerçekten silineni** yazar. Koşullu silme bir satırı korursa
        // dosyası çoktan silinmiş olabilir; "silindi" denmez, açıkça bildirilir —
        // kaydı olup dosyası olmayan bir satır kullanıcının bilmesi gereken bir
        // tutarsızlıktır.
        for blob in files_removed {
            if self.blob_exists(&blob.id)? {
                report.blocked.push(format!(
                    "{}: kayıt korundu (referans doğdu) — dosya silinmişti",
                    blob.storage_id
                ));
            } else {
                report.deleted.push(blob);
            }
        }
        Ok(report)
    }

    /// Referanssız blob'ları bulur; `dry_run` değilse siler. Hiçbir zaman
    /// referanslı bir blob'a dokunmaz.
    pub fn garbage_collect_blobs(&self, dry_run: bool) -> Result<GcReport, StorageError> {
        let candidates = self.orphan_blob_candidates()?;
        if dry_run || candidates.is_empty() {
            return Ok(GcReport {
                dry_run,
                freed_bytes: 0,
                candidates,
                deleted: Vec::new(),
            });
        }
        let ids: Vec<String> = candidates.iter().map(|b| b.id.clone()).collect();
        self.delete_unreferenced_blobs(&ids)?;
        // Rapor yalnız gerçekten silinenleri gösterir: arada referans doğmuş bir
        // satır silinmez ve `deleted` içinde yer almaz.
        let mut deleted = Vec::new();
        for blob in &candidates {
            if !self.blob_exists(&blob.id)? {
                deleted.push(blob.clone());
            }
        }
        let freed_bytes = deleted.iter().map(|b| b.encrypted_size).sum();
        Ok(GcReport {
            dry_run: false,
            candidates,
            deleted,
            freed_bytes,
        })
    }

    /// Satır hâlâ duruyor mu (GC raporu için).
    fn blob_exists(&self, id: &str) -> Result<bool, StorageError> {
        let count: i64 = self.conn().query_row(
            "SELECT count(*) FROM blobs WHERE id=?1",
            [id],
            |row: &Row<'_>| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Kasa bütünlüğü için özet: toplam blob, referanssız blob ve boyut.
    pub fn blob_inventory(&self) -> Result<(i64, i64, i64), StorageError> {
        let total: i64 = self
            .conn()
            .query_row("SELECT count(*) FROM blobs", [], |r| r.get(0))?;
        let orphans_sql = format!("SELECT count(*) FROM blobs b WHERE {UNREFERENCED}");
        let orphans: i64 = self.conn().query_row(&orphans_sql, [], |r| r.get(0))?;
        let bytes_sql =
            format!("SELECT coalesce(sum(b.encrypted_size),0) FROM blobs b WHERE {UNREFERENCED}");
        let bytes: i64 = self.conn().query_row(&bytes_sql, [], |r| r.get(0))?;
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

    /// Disk adı biçimi `<32 küçük hex>.blb`: test kimlikleri de aynı biçimde
    /// kurulur (gerçek üretim kimliğiyle aynı ad alanı).
    fn hex(digit: &str) -> String {
        std::iter::repeat_n(digit, 32).collect()
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
        // Ölçülen kusur (2026-11-12): bu testin eski hâli "türev korunur" diyordu
        // ama **hiç `pages` satırı kurmuyordu** ve blob'un silinmesini doğruluyordu
        // — yani koruma iddiası ölçülmüyordu. Artık gerçek bir türev sayfası
        // kurulur: blob aday **değildir**; sayfa silinince aday olur (referansın
        // koruyan şey olduğu iki yönlü gösterilir).
        let (db, case_id) = setup();
        let main = db
            .create_blob_record(&hex("1"), 100, "aad-m")
            .expect("ana blob");
        let deriv = db
            .create_blob_record(&hex("2"), 50, "aad-d")
            .expect("türev blob");
        let doc = db
            .create_document(&case_id, "Taranmış", "pdf", None, "unknown")
            .expect("doc");
        let version = db
            .add_version(
                &doc.id,
                &case_id,
                &main,
                &"c".repeat(64),
                100,
                "application/pdf",
            )
            .expect("sürüm");
        let run = db
            .start_parse_run(&version.id, &case_id, "pymupdf", "1.24", "{}")
            .expect("koşu");
        db.conn()
            .execute(
                "INSERT INTO pages (id,parse_run_id,page_index,text_origin,quality_flags,derivative_blob_id)
                 VALUES ('pg-1',?1,0,'native','[]',?2)",
                rusqlite::params![run.id, deriv],
            )
            .expect("sayfa");

        assert!(
            db.orphan_blob_candidates().expect("aday").is_empty(),
            "türev sayfa gösteren blob aday olamaz"
        );
        assert!(db.blob_is_referenced(&deriv).expect("referans"));
        let (_, orphans, _) = db.blob_inventory().expect("envanter");
        assert_eq!(orphans, 0);

        // Sayfa gidince blob referanssız kalır: koruyan şey referanstır.
        db.conn()
            .execute("DELETE FROM pages WHERE id='pg-1'", [])
            .expect("sayfa sil");
        let candidates = db.orphan_blob_candidates().expect("aday");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, deriv);
    }

    #[test]
    fn render_derivative_blob_is_not_collected() {
        // `render_derivatives.blob_id` **FK taşımaz**: koşulda sayılmazsa GC onu
        // referanssız sanıp kaydı ve dosyayı silerdi.
        let (db, _case) = setup();
        let blob = db.create_blob_record(&hex("3"), 64, "aad-r").expect("blob");
        db.conn()
            .execute(
                "INSERT INTO render_derivatives (id,document_version_id,parse_run_id,blob_id,
                   renderer_version,settings_hash,page_manifest,content_hash,created_at)
                 VALUES ('rd-1','v1',NULL,?1,'1.0','h','[]','c','2026-01-01T00:00:00Z')",
                [&blob],
            )
            .expect("türev");
        assert!(db.blob_is_referenced(&blob).expect("referans"));
        assert!(db.orphan_blob_candidates().expect("aday").is_empty());
        let dir = tempfile::tempdir().expect("dizin");
        // Dosya **diskte olsa** korunmalı: referansı olan blob'un dosyasına
        // dokunulmaz (korunan şey yalnız kayıt değil).
        let path = dir.path().join(format!("{}.blb", hex("3")));
        std::fs::write(&path, b"canli").expect("yaz");
        let report = db
            .collect_orphan_blob_files(dir.path(), false)
            .expect("topla");
        assert!(report.deleted.is_empty());
        assert!(path.exists(), "referanslı blob'un dosyası silinmemeli");
        assert!(db.blob_exists(&blob).expect("kayıt"));
    }

    #[test]
    fn cache_entry_blob_is_not_collected() {
        // `ai_cache_entries.output_blob_id` de FK'sizdir; aynı kapı kapsamalı.
        let (db, case_id) = setup();
        let blob = db.create_blob_record(&hex("4"), 32, "aad-c").expect("blob");
        db.conn()
            .execute(
                "INSERT INTO ai_cache_entries (id,case_id,cache_key,ai_run_id,output_blob_id,
                   validation_version,created_at)
                 VALUES ('ac-1',?1,'k',NULL,?2,'1','2026-01-01T00:00:00Z')",
                rusqlite::params![case_id, &blob],
            )
            .expect("önbellek");
        assert!(db.blob_is_referenced(&blob).expect("referans"));
        assert!(db.orphan_blob_candidates().expect("aday").is_empty());
    }

    /// Şemadaki **her** blob kimliği sütunu referans kapısında sayılmalı.
    /// Yeni bir tablo/sütun eklenip kapı güncellenmezse bu test düşer: aksi
    /// hâlde GC canlı bir blob'u "referanssız" sanıp silebilir.
    #[test]
    fn every_blob_column_is_covered_by_the_reference_gate() {
        let (db, _case) = setup();
        let flat = UNREFERENCED
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let tables: Vec<String> = {
            let mut stmt = db
                .conn()
                .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")
                .expect("tablolar");
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .expect("sorgu");
            rows.collect::<Result<_, _>>().expect("satır")
        };
        let mut checked = 0;
        for table in tables {
            if table == "blobs" {
                continue;
            }
            let cols: Vec<String> = {
                let mut stmt = db
                    .conn()
                    .prepare(&format!("PRAGMA table_info({table})"))
                    .expect("pragma");
                let rows = stmt
                    .query_map([], |row| row.get::<_, String>(1))
                    .expect("sorgu");
                rows.collect::<Result<_, _>>().expect("satır")
            };
            for col in cols {
                if col != "blob_id" && !col.ends_with("_blob_id") {
                    continue;
                }
                assert!(
                    flat.contains(&format!("FROM {table} "))
                        && flat.contains(&format!(".{col} = b.id")),
                    "{table}.{col} referans kapısında yok: GC onu referanssız sanıp silebilir"
                );
                checked += 1;
            }
        }
        assert_eq!(
            checked, 4,
            "şemadaki blob sütunu sayısı değişti ({checked}); kapı ve bu sayı birlikte güncellenmeli"
        );
    }

    #[test]
    fn collect_removes_the_file_then_the_record() {
        let (db, _case) = setup();
        let dir = tempfile::tempdir().expect("dizin");
        let storage = hex("a");
        let blob = db.create_blob_record(&storage, 123, "aad").expect("blob");
        let path = dir.path().join(format!("{storage}.blb"));
        std::fs::write(&path, b"govde").expect("yaz");
        let report = db
            .collect_orphan_blob_files(dir.path(), false)
            .expect("topla");
        assert_eq!(report.deleted.len(), 1);
        assert_eq!(report.freed_bytes, 123);
        assert!(report.missing_files.is_empty());
        assert!(report.blocked.is_empty());
        assert!(!path.exists(), "dosya silinmeli");
        assert!(!db.blob_exists(&blob).expect("kayıt"), "kayıt silinmeli");
    }

    #[test]
    fn missing_file_drops_the_record_and_is_reported() {
        let (db, _case) = setup();
        let dir = tempfile::tempdir().expect("dizin");
        let storage = hex("b");
        let _blob = db.create_blob_record(&storage, 9, "aad").expect("blob");
        let report = db
            .collect_orphan_blob_files(dir.path(), false)
            .expect("topla");
        assert_eq!(report.missing_files.len(), 1);
        assert!(report.missing_files[0].starts_with(&storage));
        assert_eq!(report.deleted.len(), 1);
        assert_eq!(report.freed_bytes, 0, "diskte veri yoktu");
        assert_eq!(db.blob_inventory().expect("envanter").0, 0);
    }

    #[test]
    fn unremovable_file_keeps_its_record() {
        let (db, _case) = setup();
        let dir = tempfile::tempdir().expect("dizin");
        let storage = hex("c");
        let blob = db.create_blob_record(&storage, 11, "aad").expect("blob");
        // Aynı adda **dizin**: `remove_file` uygulanmaz, kayıt korunmalı.
        std::fs::create_dir(dir.path().join(format!("{storage}.blb"))).expect("dizin");
        let report = db
            .collect_orphan_blob_files(dir.path(), false)
            .expect("topla");
        assert_eq!(report.blocked.len(), 1);
        assert!(report.blocked[0].contains("dosya değil"));
        assert!(report.deleted.is_empty());
        assert!(db.blob_exists(&blob).expect("kayıt"));
    }

    #[test]
    fn collect_dry_run_touches_nothing() {
        let (db, _case) = setup();
        let dir = tempfile::tempdir().expect("dizin");
        let storage = hex("f");
        let blob = db.create_blob_record(&storage, 5, "aad").expect("blob");
        let path = dir.path().join(format!("{storage}.blb"));
        std::fs::write(&path, b"x").expect("yaz");
        let report = db
            .collect_orphan_blob_files(dir.path(), true)
            .expect("kuru");
        assert!(report.dry_run);
        assert!(report.deleted.is_empty());
        assert_eq!(report.freed_bytes, 0);
        assert!(path.exists(), "kuru çalıştırma dosyaya dokunmaz");
        assert!(db.blob_exists(&blob).expect("kayıt"));
    }

    #[test]
    fn delete_by_id_refuses_a_referenced_row() {
        // Silme iki adımlıdır (dosya → kayıt): araya giren bir referans,
        // kimlikle silme çağrılsa bile satırı korumalı.
        let (db, case_id) = setup();
        let blob = db.create_blob_record("st-late", 10, "aad").expect("blob");
        let doc = db
            .create_document(&case_id, "Belge", "pdf", None, "unknown")
            .expect("doc");
        db.add_version(
            &doc.id,
            &case_id,
            &blob,
            &"b".repeat(64),
            10,
            "application/pdf",
        )
        .expect("sürüm");

        let deleted = db
            .delete_unreferenced_blobs(std::slice::from_ref(&blob))
            .expect("sil");
        assert_eq!(deleted, 0);
        assert!(db.blob_exists(&blob).expect("kayıt"));
    }

    #[test]
    fn candidates_and_inventory_agree() {
        let (db, _) = setup();
        db.create_blob_record("st-a", 5, "aad").expect("a");
        db.create_blob_record("st-b", 7, "aad").expect("b");
        let candidates = db.orphan_blob_candidates().expect("aday");
        let (_, orphans, bytes) = db.blob_inventory().expect("envanter");
        assert_eq!(i64::try_from(candidates.len()).unwrap(), orphans);
        assert_eq!(
            candidates.iter().map(|c| c.encrypted_size).sum::<i64>(),
            bytes
        );
    }
}
