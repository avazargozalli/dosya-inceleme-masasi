//! Kasa yedeği **geçmişi** — şema `backup_runs`.
//!
//! Neden bu modül: `vault_backup` yedeği diske yazıyor, manifesti üretiyor ve
//! yedeğin açıldığını doğruluyordu; ama kasa **kendi geçmişini tutmuyordu**.
//! "Ne zaman, nereye, hangi manifestle, doğrulandı mı" sorusunun tek kanıtı
//! hedef klasördeki `manifest.json` dosyasıydı ve uygulamadan okunamıyordu
//! (`backup_runs` tablosu şemada vardı, hiç yazılmıyordu).
//!
//! Kayıt **yedeğin kendisinden sonra** yazılır: yedek alınamadıysa sahte bir
//! "alındı" satırı oluşmaz. Kayıt yazılamazsa yedek yine de geçerlidir; komut
//! bunu ayrı alanda (`historyRecorded=false` + kod) bildirir.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

/// `backup_runs` tablosunun okunmuş satırı. Alan adları arayüzde camelCase'tir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRunRow {
    pub id: String,
    /// Yedek klasörünün adı (ör. `DimYedek-20261107-1830`) — hangi snapshot.
    pub snapshot_id: String,
    /// Yedeği üreten kasadaki şema sürümü.
    pub schema_version: i64,
    /// `manifest.json` baytlarının sha256'sı (yedeğin künyesi).
    pub manifest_hash: String,
    pub status: String,
    /// Doğrulama kanıtı (ör. `integrity_check`); yoksa `null`.
    pub verification_result: Option<String>,
    /// Kullanıcının seçtiği hedef klasör (kırpılmış).
    pub destination_label: String,
    pub created_at: String,
    pub completed_at: Option<String>,
}

/// Tablodaki `status` CHECK kısıtıyla **aynı** küme. Bilinmeyen durum yazılmaz:
/// arayüz "doğrulandı" ile "sürüyor"u ayırt edemezse yalan söyler.
pub const BACKUP_RUN_STATUSES: [&str; 4] = ["running", "succeeded", "failed", "verified"];

/// Liste üst sınırı (sessizce sınırsız liste dönmez).
pub const MAX_BACKUP_RUN_LIST: usize = 50;

/// Yazılacak yedek kaydı. `status` çağıranın kararıdır ama küme kapalıdır.
pub struct NewBackupRun<'a> {
    pub snapshot_id: &'a str,
    pub schema_version: i64,
    pub manifest_hash: &'a str,
    pub status: &'a str,
    pub verification_result: Option<&'a str>,
    pub destination_label: &'a str,
}

impl Database {
    /// Yedek kaydını yazar ve **geri okur**. Boş kimlik/manifest/hedef ve
    /// tanınmayan durum reddedilir; yarım kayıt oluşmaz.
    pub fn record_backup_run(&self, run: &NewBackupRun<'_>) -> Result<BackupRunRow, StorageError> {
        let snapshot = run.snapshot_id.trim();
        let manifest = run.manifest_hash.trim();
        let destination = run.destination_label.trim();
        if snapshot.is_empty() || manifest.is_empty() || destination.is_empty() {
            return Err(StorageError::BackupInvalid {
                reason: "yedek kaydı eksik: snapshot, manifest ve hedef zorunlu".to_string(),
            });
        }
        if !BACKUP_RUN_STATUSES.contains(&run.status) {
            return Err(StorageError::BackupInvalid {
                reason: format!("bilinmeyen yedek durumu: {}", run.status),
            });
        }
        if run.schema_version < 0 {
            return Err(StorageError::BackupInvalid {
                reason: "şema sürümü negatif olamaz".to_string(),
            });
        }
        let verification = run
            .verification_result
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        // `running` henüz bitmemiştir: tamamlanma zamanı uydurulmaz.
        let completed = if run.status == "running" {
            None
        } else {
            Some(now.clone())
        };
        self.conn().execute(
            "INSERT INTO backup_runs
                (id, snapshot_id, schema_version, manifest_hash, status,
                 verification_result, destination_label, created_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                id,
                snapshot,
                run.schema_version,
                manifest,
                run.status,
                verification,
                destination,
                now,
                completed
            ],
        )?;
        self.get_backup_run(&id)?.ok_or(StorageError::NotFound)
    }

    fn get_backup_run(&self, id: &str) -> Result<Option<BackupRunRow>, StorageError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT id, snapshot_id, schema_version, manifest_hash, status,
                        verification_result, destination_label, created_at, completed_at
                 FROM backup_runs WHERE id = ?1",
                [id],
                map_backup_run,
            )
            .optional()?)
    }

    /// **En yeni önce.** Aynı damgada yazılan iki kayıt `created_at` ile
    /// ayrılmaz; sıra `rowid DESC` ile kararlıdır (kayıt sırası korunur).
    /// `limit` 1..=[`MAX_BACKUP_RUN_LIST`] aralığına kırpılır.
    pub fn list_backup_runs(&self, limit: usize) -> Result<Vec<BackupRunRow>, StorageError> {
        let limit = i64::try_from(limit.clamp(1, MAX_BACKUP_RUN_LIST)).unwrap_or(1);
        let mut stmt = self.conn().prepare(
            "SELECT id, snapshot_id, schema_version, manifest_hash, status,
                    verification_result, destination_label, created_at, completed_at
             FROM backup_runs
             ORDER BY created_at DESC, rowid DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], map_backup_run)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

fn map_backup_run(row: &Row<'_>) -> rusqlite::Result<BackupRunRow> {
    Ok(BackupRunRow {
        id: row.get(0)?,
        snapshot_id: row.get(1)?,
        schema_version: row.get(2)?,
        manifest_hash: row.get(3)?,
        status: row.get(4)?,
        verification_result: row.get(5)?,
        destination_label: row.get(6)?,
        created_at: row.get(7)?,
        completed_at: row.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseKey;

    fn db() -> Database {
        Database::open_in_memory(&DatabaseKey::new([31u8; 32])).expect("db")
    }

    const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn run<'a>(snapshot: &'a str, status: &'a str) -> NewBackupRun<'a> {
        NewBackupRun {
            snapshot_id: snapshot,
            schema_version: 24,
            manifest_hash: HASH,
            status,
            verification_result: Some("integrity_check"),
            destination_label: "  /Volumes/Yedek  ",
        }
    }

    #[test]
    fn backup_run_is_recorded_and_read_back() {
        let db = db();
        let row = db
            .record_backup_run(&NewBackupRun {
                snapshot_id: "DimYedek-20261107-1830",
                schema_version: 24,
                manifest_hash: &"b".repeat(64),
                status: "verified",
                verification_result: Some("integrity_check"),
                destination_label: "  /Volumes/Yedek  ",
            })
            .expect("kayıt");
        assert_eq!(row.snapshot_id, "DimYedek-20261107-1830");
        assert_eq!(row.schema_version, 24);
        assert_eq!(row.status, "verified");
        assert_eq!(row.verification_result.as_deref(), Some("integrity_check"));
        // Hedef kırpılır: boşluklu yol "başka klasör" gibi görünmez.
        assert_eq!(row.destination_label, "/Volumes/Yedek");
        assert!(
            row.completed_at.is_some(),
            "biten kayıt tamamlanma anı taşır"
        );
        assert_eq!(db.list_backup_runs(10).expect("liste").len(), 1);
    }

    #[test]
    fn running_run_has_no_completed_at() {
        let db = db();
        let row = db
            .record_backup_run(&NewBackupRun {
                snapshot_id: "DimYedek-1",
                schema_version: 24,
                manifest_hash: &"c".repeat(64),
                status: "running",
                verification_result: None,
                destination_label: "/Volumes/Yedek",
            })
            .expect("kayıt");
        assert!(row.completed_at.is_none());
        assert!(row.verification_result.is_none());
    }

    #[test]
    fn record_rejects_unknown_status_and_empty_identity() {
        let db = db();
        let err = db
            .record_backup_run(&NewBackupRun {
                snapshot_id: "snap",
                schema_version: 24,
                manifest_hash: &"d".repeat(64),
                status: "ok",
                verification_result: None,
                destination_label: "/Volumes/Yedek",
            })
            .expect_err("bilinmeyen durum yazılmaz");
        assert!(matches!(err, StorageError::BackupInvalid { .. }));

        let err = db
            .record_backup_run(&NewBackupRun {
                snapshot_id: "   ",
                schema_version: 24,
                manifest_hash: &"d".repeat(64),
                status: "verified",
                verification_result: None,
                destination_label: "/Volumes/Yedek",
            })
            .expect_err("boş snapshot kimliği yazılmaz");
        assert!(matches!(err, StorageError::BackupInvalid { .. }));

        let err = db
            .record_backup_run(&NewBackupRun {
                snapshot_id: "snap",
                schema_version: 24,
                manifest_hash: &"d".repeat(64),
                status: "verified",
                verification_result: None,
                destination_label: "  ",
            })
            .expect_err("boş hedef yazılmaz");
        assert!(matches!(err, StorageError::BackupInvalid { .. }));
        assert!(db.list_backup_runs(10).expect("liste").is_empty());
    }

    #[test]
    fn list_is_newest_first_and_survives_equal_timestamps() {
        let db = db();
        let first = db
            .record_backup_run(&run("DimYedek-1", "verified"))
            .expect("1");
        let second = db
            .record_backup_run(&run("DimYedek-2", "verified"))
            .expect("2");
        // Gerçek koşuda `now_iso` mikrosaniyedir; eşitliği zorlayarak sıra
        // sözleşmesi (`created_at DESC, rowid DESC`) ölçülür.
        db.conn()
            .execute(
                "UPDATE backup_runs SET created_at = '2026-11-07T12:00:00.000000Z'",
                [],
            )
            .expect("damga eşitle");
        let rows = db.list_backup_runs(10).expect("liste");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].snapshot_id, "DimYedek-2", "en yeni kayıt önce");
        assert_eq!(rows[1].snapshot_id, "DimYedek-1");
        assert_eq!(rows[0].id, second.id);
        assert_eq!(rows[1].id, first.id);
    }

    #[test]
    fn list_limit_is_clamped() {
        let db = db();
        for n in 1..=3 {
            db.record_backup_run(&run(&format!("DimYedek-{n}"), "succeeded"))
                .expect("kayıt");
        }
        assert_eq!(db.list_backup_runs(0).expect("0").len(), 1);
        assert_eq!(db.list_backup_runs(2).expect("2").len(), 2);
        assert_eq!(
            db.list_backup_runs(usize::MAX).expect("max").len(),
            3,
            "kırpma üst sınırı sessiz sınırsız listeye çevirmez"
        );
    }
}
