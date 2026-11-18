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
    /// Yedek kaydını yazar ve **geri okur**. Boş kimlik/manifest/hedef,
    /// biçimsiz manifest özeti (64 küçük harf hex), yol ayırıcı taşıyan
    /// snapshot kimliği, pozitif olmayan şema sürümü, tanınmayan durum ve
    /// **kanıtsız `verified`** reddedilir; yarım kayıt oluşmaz. Kurallar
    /// 0058 şema kapısıyla **aynı**dır (kapı düşürülse bile çekirdek reddeder).
    pub fn record_backup_run(&self, run: &NewBackupRun<'_>) -> Result<BackupRunRow, StorageError> {
        let snapshot = run.snapshot_id.trim();
        let manifest = run.manifest_hash.trim();
        let destination = run.destination_label.trim();
        if snapshot.is_empty() || manifest.is_empty() || destination.is_empty() {
            return Err(StorageError::BackupInvalid {
                reason: "yedek kaydı eksik: snapshot, manifest ve hedef zorunlu".to_string(),
            });
        }
        // Snapshot kimliği **klasör adıdır** (hedefin altındaki yedek klasörü):
        // yol ayırıcı taşırsa «hangi klasör» sorusu yanıltıcı olur.
        if snapshot.contains(['/', '\\']) || snapshot.chars().any(char::is_control) {
            return Err(StorageError::BackupInvalid {
                reason:
                    "snapshot kimliği klasör adıdır: yol ayırıcı ve kontrol karakteri taşıyamaz"
                        .to_string(),
            });
        }
        // `manifest_hash` = `manifest.json` baytlarının sha256'sı; çekirdek
        // `hex::encode` ile **küçük harf** üretir (0058 aynı biçimi ister).
        let manifest_ok = manifest.len() == 64
            && manifest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !manifest_ok {
            return Err(StorageError::BackupInvalid {
                reason: "manifest özeti 64 küçük harf hex olmalı (manifest.json sha256)"
                    .to_string(),
            });
        }
        if !BACKUP_RUN_STATUSES.contains(&run.status) {
            return Err(StorageError::BackupInvalid {
                reason: format!("bilinmeyen yedek durumu: {}", run.status),
            });
        }
        if run.schema_version < 1 {
            return Err(StorageError::BackupInvalid {
                reason:
                    "şema sürümü pozitif olmalı (yedeği üreten kasa en az bir göç uygulamıştır)"
                        .to_string(),
            });
        }
        let verification = run
            .verification_result
            .map(str::trim)
            .filter(|value| !value.is_empty());
        // «Doğrulandı» iddiasının dayanağı kanıttır: `verified` kanıtsız
        // yazılamaz; `running` ise henüz doğrulanmadı, kanıt taşıyamaz.
        if run.status == "verified" && verification.is_none() {
            return Err(StorageError::BackupInvalid {
                reason: "doğrulanmış yedek kanıtsız yazılamaz (integrity_check zorunlu)"
                    .to_string(),
            });
        }
        if run.status == "running" && verification.is_some() {
            return Err(StorageError::BackupInvalid {
                reason: "süren yedeğe doğrulama kanıtı yazılamaz".to_string(),
            });
        }
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
        // v58 `created_at`i dondurur (0058 BU): sıra sözleşmesi
        // (`created_at DESC, rowid DESC`) eşit damgayla **doğuşta** yazılarak
        // ölçülür — sonradan UPDATE ile damga eşitlemek artık kapalıdır.
        for (id, snapshot) in [("r1", "DimYedek-1"), ("r2", "DimYedek-2")] {
            db.conn()
                .execute(
                    "INSERT INTO backup_runs
                       (id, snapshot_id, schema_version, manifest_hash, status,
                        verification_result, destination_label, created_at, completed_at)
                     VALUES (?1, ?2, 57, ?3, 'verified', 'integrity_check',
                             '/Volumes/Yedek', '2026-11-07T12:00:00.000000Z',
                             '2026-11-07T12:00:00.000000Z')",
                    rusqlite::params![id, snapshot, "b".repeat(64)],
                )
                .expect("eşit damgalı kayıt");
        }
        let rows = db.list_backup_runs(10).expect("liste");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].snapshot_id, "DimYedek-2", "en yeni kayıt önce");
        assert_eq!(rows[1].snapshot_id, "DimYedek-1");
        assert_eq!(rows[0].id, "r2");
        assert_eq!(rows[1].id, "r1");
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

    /// 0058 — **yedek makbuzu**: doğuşta kanıtlı, sonuçlandıktan sonra donmuş.
    ///
    /// Okuyucular bu satıra yaslanır: `loadBackupFreshness` →
    /// `latestBackupSuccess` «son başarılı yedek» diye `verified`/`succeeded`
    /// satırların `completed_at ?? created_at`ini basar; `nextBackupAt` sonraki
    /// otomatik yedeği öteler; yedek geçmişi rozetleri durumu buradan yazar.
    /// Damga/kanıt çevrilebilirse ekran ve zamanlama yalan söyler.
    #[test]
    fn backup_receipt_is_backed_at_birth_and_frozen() {
        let db = db();
        const AT: &str = "2026-11-19T00:00:00.000000Z";
        const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

        let reject = |result: Result<usize, StorageError>, label: &str| match result {
            Err(StorageError::Integrity(message)) => assert!(
                message.contains("backup_runs"),
                "{label}: mesaj tabloyu adlandırmıyor: {message}"
            ),
            other => panic!("{label}: beklenen E_DB_INTEGRITY, ölçülen: {other:?}"),
        };

        /// Ham INSERT: `(id, snapshot, schema, hash, status, kanıt, hedef,
        /// created, completed)`. Kapıyı çekirdekten **bağımsız** ölçer.
        type Row<'a> = (
            &'a str,
            &'a str,
            i64,
            &'a str,
            &'a str,
            Option<&'a str>,
            &'a str,
            &'a str,
            Option<&'a str>,
        );
        fn birth(db: &Database, row: Row<'_>) -> Result<usize, StorageError> {
            let (id, snapshot, schema, hash, status, evidence, dest, created, completed) = row;
            let evidence_sql = evidence
                .map(|value| format!("'{value}'"))
                .unwrap_or_else(|| "NULL".to_string());
            let completed_sql = completed
                .map(|value| format!("'{value}'"))
                .unwrap_or_else(|| "NULL".to_string());
            db.conn()
                .execute(
                    &format!(
                        "INSERT INTO backup_runs
                           (id, snapshot_id, schema_version, manifest_hash, status,
                            verification_result, destination_label, created_at, completed_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, {evidence_sql}, ?6, ?7, {completed_sql})"
                    ),
                    rusqlite::params![id, snapshot, schema, hash, status, dest, created],
                )
                .map_err(StorageError::from)
        }

        // --- doğum: makbuz kanıtsız/biçimsiz doğamaz -------------------------
        let ok = Some("integrity_check");
        let birth_attacks: Vec<(&str, Row<'_>)> = vec![
            (
                "boş kimlik",
                (
                    "",
                    "DimYedek-1",
                    57,
                    HASH,
                    "verified",
                    ok,
                    "/Y",
                    AT,
                    Some(AT),
                ),
            ),
            (
                "snapshot boşluk",
                ("b02", "   ", 57, HASH, "verified", ok, "/Y", AT, Some(AT)),
            ),
            (
                "snapshot kırpılmamış",
                (
                    "b03",
                    " DimYedek-1 ",
                    57,
                    HASH,
                    "verified",
                    ok,
                    "/Y",
                    AT,
                    Some(AT),
                ),
            ),
            (
                "snapshot yol ayırıcı",
                (
                    "b04",
                    "../kacis",
                    57,
                    HASH,
                    "verified",
                    ok,
                    "/Y",
                    AT,
                    Some(AT),
                ),
            ),
            (
                "snapshot kontrol karakteri",
                (
                    "b05",
                    "DimYedek\t1",
                    57,
                    HASH,
                    "verified",
                    ok,
                    "/Y",
                    AT,
                    Some(AT),
                ),
            ),
            (
                "şema sürümü 0",
                (
                    "b06",
                    "DimYedek-1",
                    0,
                    HASH,
                    "verified",
                    ok,
                    "/Y",
                    AT,
                    Some(AT),
                ),
            ),
            (
                "şema sürümü negatif",
                (
                    "b07",
                    "DimYedek-1",
                    -2,
                    HASH,
                    "verified",
                    ok,
                    "/Y",
                    AT,
                    Some(AT),
                ),
            ),
            (
                "manifest 63 hex",
                (
                    "b08",
                    "DimYedek-1",
                    57,
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "verified",
                    ok,
                    "/Y",
                    AT,
                    Some(AT),
                ),
            ),
            (
                "manifest büyük harf",
                (
                    "b09",
                    "DimYedek-1",
                    57,
                    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                    "verified",
                    ok,
                    "/Y",
                    AT,
                    Some(AT),
                ),
            ),
            (
                "manifest hex değil",
                (
                    "b10",
                    "DimYedek-1",
                    57,
                    "ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ",
                    "verified",
                    ok,
                    "/Y",
                    AT,
                    Some(AT),
                ),
            ),
            (
                "manifest 'hash'",
                (
                    "b11",
                    "DimYedek-1",
                    57,
                    "hash",
                    "verified",
                    ok,
                    "/Y",
                    AT,
                    Some(AT),
                ),
            ),
            (
                "hedef boşluk",
                (
                    "b12",
                    "DimYedek-1",
                    57,
                    HASH,
                    "verified",
                    ok,
                    "   ",
                    AT,
                    Some(AT),
                ),
            ),
            (
                "hedef kırpılmamış",
                (
                    "b13",
                    "DimYedek-1",
                    57,
                    HASH,
                    "verified",
                    ok,
                    " /Y ",
                    AT,
                    Some(AT),
                ),
            ),
            (
                "created_at 'dün'",
                (
                    "b14",
                    "DimYedek-1",
                    57,
                    HASH,
                    "verified",
                    ok,
                    "/Y",
                    "dün",
                    Some(AT),
                ),
            ),
            (
                "completed_at 'dün'",
                (
                    "b15",
                    "DimYedek-1",
                    57,
                    HASH,
                    "verified",
                    ok,
                    "/Y",
                    AT,
                    Some("dün"),
                ),
            ),
            (
                "running + tamamlanma damgası",
                (
                    "b16",
                    "DimYedek-1",
                    57,
                    HASH,
                    "running",
                    None,
                    "/Y",
                    AT,
                    Some(AT),
                ),
            ),
            (
                "running + kanıt",
                ("b17", "DimYedek-1", 57, HASH, "running", ok, "/Y", AT, None),
            ),
            (
                "verified + tamamlanma yok",
                (
                    "b18",
                    "DimYedek-1",
                    57,
                    HASH,
                    "verified",
                    ok,
                    "/Y",
                    AT,
                    None,
                ),
            ),
            (
                "verified + kanıtsız",
                (
                    "b19",
                    "DimYedek-1",
                    57,
                    HASH,
                    "verified",
                    None,
                    "/Y",
                    AT,
                    Some(AT),
                ),
            ),
            (
                "verified + kanıt boşluk",
                (
                    "b20",
                    "DimYedek-1",
                    57,
                    HASH,
                    "verified",
                    Some("   "),
                    "/Y",
                    AT,
                    Some(AT),
                ),
            ),
            (
                "bilinmeyen durum",
                ("b21", "DimYedek-1", 57, HASH, "ok", ok, "/Y", AT, Some(AT)),
            ),
            (
                "hedef boş",
                (
                    "b22",
                    "DimYedek-1",
                    57,
                    HASH,
                    "verified",
                    ok,
                    "",
                    AT,
                    Some(AT),
                ),
            ),
            (
                "snapshot boş",
                ("b23", "", 57, HASH, "verified", ok, "/Y", AT, Some(AT)),
            ),
        ];
        for (label, row) in birth_attacks {
            reject(birth(&db, row), label);
        }

        // --- UPDATE: kanıt ve kimlik donar, durum yalnız ileri gider ---------
        let seeds = [
            ("v01", "verified", ok, Some(AT)),
            ("v02", "verified", ok, Some(AT)),
            ("v03", "verified", ok, Some(AT)),
            ("v04", "verified", ok, Some(AT)),
            ("v05", "verified", ok, Some(AT)),
            ("v06", "verified", ok, Some(AT)),
            ("v07", "verified", ok, Some(AT)),
            ("v08", "verified", ok, Some(AT)),
            ("s01", "succeeded", None, Some(AT)),
            ("f01", "failed", Some("disk dolu"), Some(AT)),
            ("r01", "running", None, None),
            ("r02", "running", None, None),
            ("r03", "running", None, None),
            ("r04", "running", None, None),
            ("r05", "running", None, None),
        ];
        for (id, status, evidence, completed) in seeds {
            birth(
                &db,
                (
                    id,
                    &format!("DimYedek-{id}"),
                    57,
                    HASH,
                    status,
                    evidence,
                    "/Y",
                    AT,
                    completed,
                ),
            )
            .expect("tohum satır kapıdan geçmeli");
        }
        let update = |sql: &str| db.conn().execute(sql, []).map_err(StorageError::from);
        let update_attacks: Vec<(&str, &str)> = vec![
            (
                "manifest çevrildi",
                "UPDATE backup_runs SET manifest_hash='bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb' WHERE id='v01'",
            ),
            (
                "snapshot çevrildi",
                "UPDATE backup_runs SET snapshot_id='DimYedek-baska' WHERE id='v02'",
            ),
            (
                "şema sürümü çevrildi",
                "UPDATE backup_runs SET schema_version=3 WHERE id='v03'",
            ),
            (
                "hedef çevrildi",
                "UPDATE backup_runs SET destination_label='/tmp/sahte' WHERE id='v04'",
            ),
            (
                "created_at ileri alındı",
                "UPDATE backup_runs SET created_at='2030-01-01T00:00:00.000000Z' WHERE id='v05'",
            ),
            ("kimlik takası", "UPDATE backup_runs SET id='takas' WHERE id='v06'"),
            (
                "completed_at çevrildi",
                "UPDATE backup_runs SET completed_at='2020-01-01T00:00:00.000000Z' WHERE id='v07'",
            ),
            (
                "kanıt silindi",
                "UPDATE backup_runs SET verification_result=NULL WHERE id='v08'",
            ),
            (
                "succeeded→verified kanıtsız",
                "UPDATE backup_runs SET status='verified' WHERE id='s01'",
            ),
            (
                "failed→verified (başarı uydurma)",
                "UPDATE backup_runs SET status='verified' WHERE id='f01'",
            ),
            (
                "running→succeeded tamamlanma yok",
                "UPDATE backup_runs SET status='succeeded' WHERE id='r01'",
            ),
            (
                "running→succeeded 'dün' damgası",
                "UPDATE backup_runs SET status='succeeded', completed_at='dün' WHERE id='r02'",
            ),
            (
                "bilinmeyen durum",
                "UPDATE backup_runs SET status='sent' WHERE id='r03'",
            ),
            (
                "failed kanıtı değiştirildi",
                "UPDATE backup_runs SET verification_result='baska' WHERE id='f01'",
            ),
            (
                "running→verified kanıtsız",
                "UPDATE backup_runs SET status='verified', completed_at='2026-11-19T01:00:00.000000Z' WHERE id='r04'",
            ),
            (
                "running→succeeded kanıt yalnız boşluk",
                "UPDATE backup_runs SET status='succeeded', completed_at='2026-11-19T01:00:00.000000Z', verification_result='   ' WHERE id='r05'",
            ),
        ];
        for (label, sql) in update_attacks {
            reject(update(sql), label);
        }

        // --- meşru yollar: kanıtlı yedek, iki adımlı tamamlanma, sonradan
        //     doğrulama ve değişmeyen yazma takılmaz --------------------------
        birth(
            &db,
            (
                "l01",
                "DimYedek-l01",
                57,
                HASH,
                "verified",
                ok,
                "/Y",
                AT,
                Some(AT),
            ),
        )
        .expect("kanıtlı verified");
        birth(
            &db,
            (
                "l02",
                "DimYedek-l02",
                57,
                HASH,
                "running",
                None,
                "/Y",
                AT,
                None,
            ),
        )
        .expect("süren yedek");
        birth(
            &db,
            (
                "l03",
                "DimYedek-l03",
                57,
                HASH,
                "failed",
                Some("disk dolu"),
                "/Y",
                AT,
                Some(AT),
            ),
        )
        .expect("gerekçeli başarısız yedek");
        birth(
            &db,
            (
                "l04",
                "DimYedek-l04",
                57,
                HASH,
                "succeeded",
                None,
                "/Y",
                AT,
                Some(AT),
            ),
        )
        .expect("kanıtsız succeeded (doğrulama sonradan)");
        update("UPDATE backup_runs SET status='succeeded', completed_at='2026-11-19T01:00:00.000000Z' WHERE id='l02'")
            .expect("running→succeeded tamamlanma anıyla ilerler");
        update("UPDATE backup_runs SET status='verified', verification_result='integrity_check' WHERE id='l04'")
            .expect("succeeded→verified o anda kanıtla meşru");
        update("UPDATE backup_runs SET verification_result='integrity_check' WHERE id='l01'")
            .expect("değişmeyen yazma takılmaz");
        let (status, completed): (String, Option<String>) = db
            .conn()
            .query_row(
                "SELECT status, completed_at FROM backup_runs WHERE id='l02'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("l02");
        assert_eq!(status, "succeeded");
        assert_eq!(
            completed.as_deref(),
            Some("2026-11-19T01:00:00.000000Z"),
            "tamamlanma anı yazıldı"
        );

        // --- çekirdek kapıyla **aynı** kuralları uygular ----------------------
        let core =
            |manifest: &str, schema: i64, snapshot: &str, status: &str, evidence: Option<&str>| {
                db.record_backup_run(&NewBackupRun {
                    snapshot_id: snapshot,
                    schema_version: schema,
                    manifest_hash: manifest,
                    status,
                    verification_result: evidence,
                    destination_label: "/Volumes/Yedek",
                })
            };
        for (label, result) in [
            (
                "çekirdek: manifest 'hash'",
                core("hash", 57, "DimYedek-c1", "verified", ok),
            ),
            (
                "çekirdek: şema 0",
                core(HASH, 0, "DimYedek-c2", "verified", ok),
            ),
            (
                "çekirdek: yol snapshot",
                core(HASH, 57, "../kacis", "verified", ok),
            ),
            (
                "çekirdek: kanıtsız verified",
                core(HASH, 57, "DimYedek-c3", "verified", None),
            ),
            (
                "çekirdek: süren yedeğe kanıt",
                core(HASH, 57, "DimYedek-c4", "running", ok),
            ),
        ] {
            assert!(
                matches!(result, Err(StorageError::BackupInvalid { .. })),
                "{label}: beklenen E_BACKUP_INVALID, ölçülen: {result:?}"
            );
        }
        let recorded =
            core(HASH, 57, "DimYedek-c5", "verified", ok).expect("kanıtlı çekirdek kaydı");
        assert_eq!(recorded.status, "verified");
        assert_eq!(
            recorded.verification_result.as_deref(),
            Some("integrity_check")
        );
    }
}
