//! Disk tarafındaki blob artıkları: kasanın `blobs/` klasöründe duran ama
//! veritabanında karşılığı olmayan dosyalar.
//!
//! Neden ayrı bir envanter gerekir: `garbage_collect_blobs` **kayıtları** siler,
//! dosyayı çağıran (Tauri) siler. Yazma sırası önce kayıt sonra dosya olduğunda
//! dosya silinemezse (izin, kilit, süreç ölümü) geriye veritabanında hiçbir izi
//! olmayan bir dosya kalır — `blobs` tablosundan türeyen hiçbir envanter onu
//! göremez, ürün onu bir daha silemez. Aynı sınıf: `write_blob` yarıda kalırsa
//! `.blb.tmp` dosyası diskte kalır (bkz. `dim_vault::write_blob`).
//!
//! Kural: **diskten yalnız kanıtlı artıklar silinir.**
//!
//! - Ad biçimi kapalı küme: `<32 küçük hex>.blb` ve `.<32 küçük hex>.blb.tmp`.
//!   Biçime uymayan her girdi `ignored` — hiçbir zaman silinmez.
//! - Symlink, dizin ve diğer dosya türleri **dokunulmaz** (`ignored`); symlink
//!   hedefi asla izlenmez.
//! - Eşikten **yeni** dosyalar aday olmaz: içe aktarma `rename` ile dosyayı
//!   yerleştirip kaydı hemen sonra yazar; arada kalan milisaniyelerde dosya
//!   "sahipsiz" görünür. Yaş eşiği bu yarışı kapatır.
//! - Silme anında kayıt **yeniden sorulur**: arada yazılmış bir kayıt varsa
//!   dosya silinmez (`keptListed`).
//! - Veritabanında olup diskte olmayan kayıt (`missing`) yalnız **bildirilir**;
//!   bu modül kayıt silmez, "onarmaz".
//!
//! Denetim yalnız sayı taşır (dosya adı yazılmaz).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rusqlite::Row;
use serde::{Deserialize, Serialize};

use crate::{atomic, now_iso, Database, StorageError};

/// Aday listelerinin raporda dönen üst sınırı. Toplam sayı ayrı alanda gelir;
/// kırpma gizlenmez (`truncated`).
pub const MAX_DISK_LIST: usize = 100;

/// Artık dosya sayılmak için gereken en az yaş (saniye). İçe aktarma ile
/// temizlik arasındaki yarışı kapatır.
pub const DEFAULT_MIN_AGE_SECS: i64 = 600;

/// Diskte bulunan tek bir blob dosyası.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskBlobFile {
    /// Diskteki tam ad (`<id>.blb` ya da `.<id>.blb.tmp`). Dosya adı denetime
    /// yazılmaz, yalnız raporda görünür.
    pub file_name: String,
    pub storage_id: String,
    pub bytes: i64,
    /// Son değişiklik zamanı (Unix saniye).
    pub modified_unix: i64,
}

/// Diskteki blob klasörünün envanteri.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobDiskScan {
    pub dir_present: bool,
    /// Klasördeki toplam girdi (her tür).
    pub entries: u64,
    /// Veritabanında karşılığı olan `.blb` dosyası.
    pub listed_files: u64,
    pub listed_bytes: i64,
    /// Veritabanında karşılığı **olmayan** `.blb` (silinebilir aday).
    pub unlisted: Vec<DiskBlobFile>,
    pub unlisted_total: u64,
    pub unlisted_bytes: i64,
    /// Eşikten eski `.blb.tmp` (silinebilir aday).
    pub stale_temps: Vec<DiskBlobFile>,
    pub stale_temp_total: u64,
    pub stale_temp_bytes: i64,
    /// Eşikten yeni olduğu için aday sayılmayan dosya sayısı.
    pub young_protected: u64,
    /// Veritabanında kaydı olup diskte dosyası olmayan blob kimlikleri.
    pub missing: Vec<String>,
    pub missing_total: u64,
    /// Ad biçimine uymayan ya da dosya olmayan (symlink/dizin) girdiler.
    pub ignored: Vec<String>,
    pub ignored_total: u64,
    /// Listeler kırpıldı mı (`MAX_DISK_LIST`).
    pub truncated: bool,
    /// Veritabanı tarafı: toplam blob, referanssız blob, referanssız bayt.
    pub db_total: i64,
    pub db_orphans: i64,
    pub db_orphan_bytes: i64,
}

/// Diskteki artıkların temizlik raporu.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobDiskSweep {
    pub dry_run: bool,
    /// Silinecek adaylar (kuru çalıştırmada da aynı seçim).
    pub planned: Vec<DiskBlobFile>,
    /// Gerçekten silinen dosyalar.
    pub removed: Vec<DiskBlobFile>,
    /// Silme anında veritabanında kaydı görülen, bu yüzden **dokunulmayan**
    /// dosyalar (yarış kapısı).
    pub kept_listed: Vec<String>,
    /// Silinemeyen dosyalar (`ad: neden`).
    pub errors: Vec<String>,
    pub freed_bytes: i64,
    /// Silinemediği için kaydı da **korunan** kuru artık yok; bu alan yalnız
    /// veritabanı tarafındaki eksik dosyaları bildirir.
    pub missing: Vec<String>,
    pub truncated: bool,
}

impl BlobDiskSweep {
    /// Kuru çalıştırma raporundan görünüm.
    #[must_use]
    pub fn planned_only(scan: &BlobDiskScan) -> Self {
        let mut planned = scan.unlisted.clone();
        planned.extend(scan.stale_temps.iter().cloned());
        Self {
            dry_run: true,
            planned,
            removed: Vec::new(),
            kept_listed: Vec::new(),
            errors: Vec::new(),
            freed_bytes: 0,
            missing: scan.missing.clone(),
            truncated: scan.truncated,
        }
    }
}

/// Ad biçimi: 32 küçük hex + `.blb`.
fn parse_blob_name(name: &str) -> Option<&str> {
    let id = name.strip_suffix(".blb")?;
    is_storage_id(id).then_some(id)
}

/// Ad biçimi: `.` + 32 küçük hex + `.blb.tmp`.
fn parse_temp_name(name: &str) -> Option<&str> {
    let id = name.strip_prefix('.')?.strip_suffix(".blb.tmp")?;
    is_storage_id(id).then_some(id)
}

/// `random_storage_id()` biçimi: 32 küçük hex. Başka her şey ad biçimi dışıdır.
fn is_storage_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn modified_unix(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

impl Database {
    /// Diskteki blob klasörünü veritabanıyla karşılaştırır. Hiçbir dosya ve
    /// kayıt değiştirilmez; yalnız rapor üretir.
    pub fn scan_blob_dir(
        &self,
        dir: &Path,
        min_age_secs: i64,
        now: i64,
    ) -> Result<BlobDiskScan, StorageError> {
        let (db_total, db_orphans, db_orphan_bytes) = self.blob_inventory()?;
        let mut scan = BlobDiskScan {
            db_total,
            db_orphans,
            db_orphan_bytes,
            ..BlobDiskScan::default()
        };

        let ids = self.blob_storage_ids()?;
        let min_age = min_age_secs.max(0);

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => {
                scan.dir_present = true;
                entries
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // Klasör yok = hiç blob yazılmamış. Eksik dosyalar yine bildirilir.
                scan.missing = ids.iter().take(MAX_DISK_LIST).cloned().collect();
                scan.missing_total = u64::try_from(ids.len()).unwrap_or(u64::MAX);
                scan.truncated = ids.len() > MAX_DISK_LIST;
                return Ok(scan);
            }
            Err(error) => return Err(atomic::map_io(error)),
        };

        for entry in entries {
            let entry = entry.map_err(atomic::map_io)?;
            scan.entries += 1;
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = entry.file_type().map_err(atomic::map_io)?;
            if !file_type.is_file() {
                // Symlink ve dizin: **dokunulmaz**. Symlink hedefi izlenmez.
                scan.ignored_total += 1;
                push_capped(&mut scan.ignored, name, MAX_DISK_LIST, &mut scan.truncated);
                continue;
            }
            let meta = match entry.metadata() {
                Ok(meta) => meta,
                Err(_) => {
                    scan.ignored_total += 1;
                    push_capped(&mut scan.ignored, name, MAX_DISK_LIST, &mut scan.truncated);
                    continue;
                }
            };
            let bytes = i64::try_from(meta.len()).unwrap_or(i64::MAX);
            let modified = modified_unix(&meta);
            let old_enough = now.saturating_sub(modified) >= min_age;

            if let Some(id) = parse_blob_name(&name) {
                if ids.contains(id) {
                    scan.listed_files += 1;
                    scan.listed_bytes = scan.listed_bytes.saturating_add(bytes);
                } else if old_enough {
                    scan.unlisted_total += 1;
                    scan.unlisted_bytes = scan.unlisted_bytes.saturating_add(bytes);
                    if scan.unlisted.len() < MAX_DISK_LIST {
                        let storage_id = id.to_string();
                        scan.unlisted.push(DiskBlobFile {
                            file_name: name,
                            storage_id,
                            bytes,
                            modified_unix: modified,
                        });
                    } else {
                        scan.truncated = true;
                    }
                } else {
                    scan.young_protected += 1;
                }
                continue;
            }

            if let Some(id) = parse_temp_name(&name) {
                if old_enough {
                    scan.stale_temp_total += 1;
                    scan.stale_temp_bytes = scan.stale_temp_bytes.saturating_add(bytes);
                    if scan.stale_temps.len() < MAX_DISK_LIST {
                        let storage_id = id.to_string();
                        scan.stale_temps.push(DiskBlobFile {
                            file_name: name,
                            storage_id,
                            bytes,
                            modified_unix: modified,
                        });
                    } else {
                        scan.truncated = true;
                    }
                } else {
                    scan.young_protected += 1;
                }
                continue;
            }

            scan.ignored_total += 1;
            push_capped(&mut scan.ignored, name, MAX_DISK_LIST, &mut scan.truncated);
        }

        // Disk üzerinde dosyası olmayan kayıtlar: yalnız bildirilir.
        let mut missing: Vec<String> = ids
            .iter()
            .filter(|id| !dir.join(format!("{id}.blb")).exists())
            .cloned()
            .collect();
        missing.sort();
        scan.missing_total = u64::try_from(missing.len()).unwrap_or(u64::MAX);
        scan.truncated |= missing.len() > MAX_DISK_LIST;
        missing.truncate(MAX_DISK_LIST);
        scan.missing = missing;

        Ok(scan)
    }

    /// Diskteki artıkları tarar ve `dry_run` değilse **yalnız** kanıtlı
    /// artıkları siler. Kayıtlara dokunmaz.
    pub fn sweep_blob_orphans(
        &self,
        dir: &Path,
        dry_run: bool,
        min_age_secs: i64,
    ) -> Result<BlobDiskSweep, StorageError> {
        let now = now_unix();
        let scan = self.scan_blob_dir(dir, min_age_secs, now)?;
        let mut candidates = scan.unlisted.clone();
        candidates.extend(scan.stale_temps.iter().cloned());
        let mut report = self.sweep_entries(dir, &candidates, dry_run)?;
        report.missing = scan.missing;
        report.truncated |= scan.truncated;
        Ok(report)
    }

    /// Aday listesini siler. Silme anında her dosya için:
    /// (a) veritabanında kaydı var mı — varsa **dokunulmaz** (yarış kapısı),
    /// (b) hâlâ normal dosya mı — symlink/dizin ise **dokunulmaz**,
    /// (c) yalnız o zaman `remove_file`.
    fn sweep_entries(
        &self,
        dir: &Path,
        candidates: &[DiskBlobFile],
        dry_run: bool,
    ) -> Result<BlobDiskSweep, StorageError> {
        let mut report = BlobDiskSweep {
            dry_run,
            planned: candidates.to_vec(),
            ..BlobDiskSweep::default()
        };
        if dry_run {
            return Ok(report);
        }

        for candidate in candidates {
            let path = dir.join(&candidate.file_name);
            if is_temp_file(&candidate.file_name) {
                // Geçici dosyanın kaydı olmaz; yalnız tür kapısı.
            } else if self.file_has_blob_record(&candidate.storage_id)? {
                report.kept_listed.push(candidate.storage_id.clone());
                continue;
            }

            match std::fs::symlink_metadata(&path) {
                Ok(meta) if meta.file_type().is_file() => {}
                Ok(_) => {
                    report
                        .errors
                        .push(format!("{}: dosya değil", candidate.file_name));
                    continue;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    // Arada silinmiş: kayıp değil, artık yok.
                    continue;
                }
                Err(error) => {
                    report
                        .errors
                        .push(format!("{}: {error}", candidate.file_name));
                    continue;
                }
            }

            match std::fs::remove_file(&path) {
                Ok(()) => {
                    report.freed_bytes = report.freed_bytes.saturating_add(candidate.bytes);
                    report.removed.push(candidate.clone());
                }
                Err(error) => report
                    .errors
                    .push(format!("{}: {error}", candidate.file_name)),
            }
        }

        if !report.removed.is_empty() {
            self.append_audit(
                "blob_disk_sweep",
                None,
                "delete",
                "local-user",
                &serde_json::json!({
                    "count": report.removed.len(),
                    "freedBytes": report.freed_bytes,
                    "keptListed": report.kept_listed.len(),
                    "errors": report.errors.len(),
                    "at": now_iso(),
                }),
            )?;
        }
        Ok(report)
    }

    /// Bir blob kaydı bu depolama kimliğiyle var mı? Silme anındaki yarış
    /// kapısı: `true` ise dosya referanslıdır, silinmez.
    pub fn file_has_blob_record(&self, storage_id: &str) -> Result<bool, StorageError> {
        let count: i64 = self.conn().query_row(
            "SELECT count(*) FROM blobs WHERE storage_id=?1",
            [storage_id],
            |row: &Row<'_>| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Veritabanındaki tüm blob depolama kimlikleri (disk karşılaştırması için).
    pub fn blob_storage_ids(&self) -> Result<HashSet<String>, StorageError> {
        let mut stmt = self.conn().prepare("SELECT storage_id FROM blobs")?;
        let rows = stmt.query_map([], |row: &Row<'_>| row.get::<_, String>(0))?;
        let mut ids = HashSet::new();
        for row in rows {
            ids.insert(row?);
        }
        Ok(ids)
    }
}

fn is_temp_file(name: &str) -> bool {
    parse_temp_name(name).is_some()
}

fn push_capped(list: &mut Vec<String>, value: String, cap: usize, truncated: &mut bool) {
    if list.len() < cap {
        list.push(value);
    } else {
        *truncated = true;
    }
}

/// Kasa klasörünün yanındaki `blobs/` dizini.
#[must_use]
pub fn blob_dir_for_vault(vault_path: &Path) -> PathBuf {
    vault_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("blobs")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::DatabaseKey;
    use std::fs;

    const AGE: i64 = 3600;

    fn setup() -> (Database, String, tempfile::TempDir) {
        let db = Database::open_in_memory(&DatabaseKey::new([73u8; 32])).expect("db");
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
        let dir = tempfile::tempdir().expect("dizin");
        (db, case.id, dir)
    }

    fn id(byte: char) -> String {
        std::iter::repeat_n(byte, 32).collect()
    }

    fn write(dir: &Path, name: &str, bytes: usize) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, vec![7u8; bytes]).expect("yaz");
        path
    }

    fn old(path: &Path) {
        let file = fs::OpenOptions::new().write(true).open(path).expect("aç");
        let age = u64::try_from(2 * AGE).expect("yaş");
        file.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(age))
            .expect("damga");
    }

    #[test]
    fn scan_classifies_listed_unlisted_temp_and_ignored() {
        let (db, _case, tmp) = setup();
        let dir = tmp.path();
        let listed = id('a');
        let unlisted = id('b');
        let temp = id('c');
        db.create_blob_record(&listed, 100, "aad").expect("kayıt");
        let p = write(dir, &format!("{listed}.blb"), 100);
        old(&p);
        let p = write(dir, &format!("{unlisted}.blb"), 40);
        old(&p);
        let p = write(dir, &format!(".{temp}.blb.tmp"), 10);
        old(&p);
        // Ad biçimine uymayanlar: hepsi dokunulmaz.
        write(dir, "notlar.txt", 5);
        write(dir, "deadbeef.blb", 5);
        write(dir, &format!("{}.blb", id('Z')), 5); // büyük harf = biçim dışı
        fs::create_dir(dir.join(format!("{}.blb", id('d')))).expect("dizin");

        let scan = db.scan_blob_dir(dir, AGE, now_unix()).expect("tarama");
        assert!(scan.dir_present);
        assert_eq!(scan.listed_files, 1);
        assert_eq!(scan.listed_bytes, 100);
        assert_eq!(scan.unlisted_total, 1);
        assert_eq!(scan.unlisted[0].storage_id, unlisted);
        assert_eq!(scan.unlisted_bytes, 40);
        assert_eq!(scan.stale_temp_total, 1);
        assert_eq!(scan.stale_temp_bytes, 10);
        assert_eq!(scan.entries, 7);
        assert_eq!(scan.ignored_total, 4);
        assert_eq!(scan.young_protected, 0);
        assert_eq!(scan.db_total, 1);
        // Kayıt hiçbir sürüme bağlı değil: **veritabanı tarafında** referanssız,
        // ama diskte dosyası var → iki görünüm ayrı şeyi söyler (GC bu satırı
        // siler, disk taraması "kayıtlı dosya" der).
        assert_eq!(scan.db_orphans, 1);
        assert_eq!(scan.missing_total, 0);
    }

    #[test]
    fn young_files_are_not_candidates() {
        let (db, _case, tmp) = setup();
        let dir = tmp.path();
        write(dir, &format!("{}.blb", id('a')), 20);
        write(dir, &format!(".{}.blb.tmp", id('b')), 20);

        let scan = db.scan_blob_dir(dir, AGE, now_unix()).expect("tarama");
        assert!(scan.unlisted.is_empty());
        assert!(scan.stale_temps.is_empty());
        assert_eq!(scan.young_protected, 2);

        let sweep = db.sweep_blob_orphans(dir, false, AGE).expect("sil");
        assert!(sweep.removed.is_empty());
        assert_eq!(fs::read_dir(dir).expect("oku").count(), 2);
    }

    #[test]
    fn dry_run_deletes_nothing_and_real_run_removes_only_orphans() {
        let (db, _case, tmp) = setup();
        let dir = tmp.path();
        let listed = id('a');
        let unlisted = id('b');
        let temp = id('c');
        db.create_blob_record(&listed, 100, "aad").expect("kayıt");
        old(&write(dir, &format!("{listed}.blb"), 100));
        old(&write(dir, &format!("{unlisted}.blb"), 40));
        old(&write(dir, &format!(".{temp}.blb.tmp"), 10));

        let dry = db.sweep_blob_orphans(dir, true, AGE).expect("kuru");
        assert_eq!(dry.planned.len(), 2);
        assert!(dry.removed.is_empty());
        assert_eq!(dry.freed_bytes, 0);
        assert_eq!(fs::read_dir(dir).expect("oku").count(), 3);

        let real = db.sweep_blob_orphans(dir, false, AGE).expect("sil");
        assert_eq!(real.removed.len(), 2);
        assert_eq!(real.freed_bytes, 50);
        assert!(real.errors.is_empty());
        assert!(dir.join(format!("{listed}.blb")).exists());
        assert!(!dir.join(format!("{unlisted}.blb")).exists());
        assert!(!dir.join(format!(".{temp}.blb.tmp")).exists());
    }

    #[test]
    fn a_row_written_after_the_scan_keeps_the_file() {
        // Üretimde olmayan ama mümkün olan yarış: tarama ile silme arasında
        // aynı kimliğe kayıt düşerse dosya referanslıdır, silinmez.
        let (db, _case, tmp) = setup();
        let dir = tmp.path();
        let late = id('e');
        old(&write(dir, &format!("{late}.blb"), 30));

        let scan = db.scan_blob_dir(dir, AGE, now_unix()).expect("tarama");
        assert_eq!(scan.unlisted_total, 1);
        db.create_blob_record(&late, 30, "aad").expect("kayıt");

        let report = db.sweep_entries(dir, &scan.unlisted, false).expect("silme");
        assert!(report.removed.is_empty());
        assert_eq!(report.kept_listed, vec![late.clone()]);
        assert!(dir.join(format!("{late}.blb")).exists());
    }

    #[test]
    fn symlink_outside_the_blob_dir_is_never_followed() {
        let (db, _case, tmp) = setup();
        let dir = tmp.path().join("blobs");
        fs::create_dir(&dir).expect("dizin");
        let outside = tmp.path().join("disarida.bin");
        fs::write(&outside, b"gizli").expect("yaz");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, dir.join(format!("{}.blb", id('f'))))
            .expect("symlink");

        let scan = db.scan_blob_dir(&dir, AGE, now_unix()).expect("tarama");
        assert_eq!(scan.unlisted_total, 0);
        assert_eq!(scan.ignored_total, 1);

        let report = db.sweep_blob_orphans(&dir, false, AGE).expect("sil");
        assert!(report.removed.is_empty());
        assert!(outside.exists());
    }

    #[test]
    fn missing_files_are_reported_and_never_repaired() {
        let (db, _case, tmp) = setup();
        let dir = tmp.path();
        let missing = id('9');
        db.create_blob_record(&missing, 77, "aad").expect("kayıt");

        let scan = db.scan_blob_dir(dir, AGE, now_unix()).expect("tarama");
        assert_eq!(scan.missing_total, 1);
        assert_eq!(scan.missing, vec![missing.clone()]);
        // Kayıt yerinde durur: bu modül "onarmaz".
        assert!(db.file_has_blob_record(&missing).expect("kayıt"));
    }

    #[test]
    fn absent_directory_is_reported_without_error() {
        let (db, _case, tmp) = setup();
        let dir = tmp.path().join("yok");
        let scan = db.scan_blob_dir(&dir, AGE, now_unix()).expect("tarama");
        assert!(!scan.dir_present);
        assert_eq!(scan.entries, 0);
        assert_eq!(scan.unlisted_total, 0);
    }

    #[test]
    fn lists_are_capped_and_the_total_is_kept() {
        let (db, _case, tmp) = setup();
        let dir = tmp.path();
        for n in 0..(MAX_DISK_LIST + 5) {
            let name = format!("{n:032x}.blb");
            old(&write(dir, &name, 1));
        }
        let scan = db.scan_blob_dir(dir, AGE, now_unix()).expect("tarama");
        assert_eq!(scan.unlisted.len(), MAX_DISK_LIST);
        assert_eq!(scan.unlisted_total, (MAX_DISK_LIST + 5) as u64);
        assert!(scan.truncated);
    }

    #[test]
    fn sweep_audits_only_counts() {
        let (db, _case, tmp) = setup();
        let dir = tmp.path();
        old(&write(dir, &format!("{}.blb", id('7')), 12));
        db.sweep_blob_orphans(dir, false, AGE).expect("sil");

        let events = db
            .list_audit_events(10, Some("blob_disk_sweep"))
            .expect("denetim");
        let event = events.first().expect("kayıt");
        let metadata = event.metadata.as_ref().expect("meta");
        let text = metadata.to_string();
        assert!(!text.contains(&id('7')));
        assert!(text.contains("\"count\":1"));
    }
}
