//! Yedek **saklama** (retention): sınırsız biriken yedek klasörlerini
//! ölçer, "son N günlük + son M haftalık" politikasına göre **silinecekleri**
//! planlar ve yalnız açık istekle siler.
//!
//! Neden bu modül: şartname (Bölüm 05.5, F14-T09) "son 7 günlük ve 4 haftalık
//! yedek saklama önerisi değiştirilebilir; disk doluluğu görünürdür" der.
//! Üretimde her otomatik yedek `DimYedek-…` klasörü **ve** `.dimbak` arşivi
//! bırakıyordu; hiçbir temizlik yolu yoktu. Haftalık yedekleme 8 GB'lık hedef
//! cihazda bir yıl içinde diskini doldurur — ve dolan disk yeni yedeği de
//! engeller.
//!
//! Dürüstlük kuralları (bunlar kaldırılamaz):
//! - **Yalnız kendi yedeğimizi sileriz**: manifestteki `vaultId` bu kasanın
//!   kimliğiyle **birebir** eşleşmelidir. Kimlik taşımayan eski yedekler ve
//!   başka kasanın yedekleri **hiçbir zaman** silinmez; raporda ayrı sayılır.
//! - Tanınmayan ad, symlink, manifesti bozuk/ileri sürüm klasör **yedek
//!   sayılmaz**; taranmaz, silinmez (`ignored`).
//! - Silme **açık istekle** (`dry_run=false`) yapılır; önce `.dimbak` arşivi,
//!   sonra klasör silinir. Arşiv silinemezse klasöre dokunulmaz (yedek
//!   bütünlüğü bozulmasın).
//! - En yeni yedek **her zaman** korunur; politika sayıları ne olursa olsun
//!   "hepsini sil" üretilemez.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time};

use crate::atomic;
use crate::backup::{read_backup_manifest, ARCHIVE_EXT, NAME_PREFIX};
use crate::error::StorageError;

/// Günlük saklama üst sınırı (yıllık planı aşan bir değer kabul edilmez).
pub const MAX_KEEP_DAILY: u32 = 365;
/// Haftalık saklama üst sınırı (bir yıl).
pub const MAX_KEEP_WEEKLY: u32 = 52;
/// Şartnamenin önerdiği varsayılan: son 7 günlük yedek.
pub const DEFAULT_KEEP_DAILY: u32 = 7;
/// Şartnamenin önerdiği varsayılan: son 4 haftalık yedek.
pub const DEFAULT_KEEP_WEEKLY: u32 = 4;

/// Saklama politikası. `keep_daily` en az 1'dir: "hiç yedek tutma" seçeneği
/// yoktur (yanlış yapılandırma tüm yedekleri silmesin).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub keep_daily: u32,
    pub keep_weekly: u32,
}

impl RetentionPolicy {
    pub fn new(keep_daily: u32, keep_weekly: u32) -> Result<Self, StorageError> {
        if keep_daily == 0 || keep_daily > MAX_KEEP_DAILY {
            return Err(StorageError::BackupInvalid {
                reason: format!(
                    "günlük saklama 1..={MAX_KEEP_DAILY} arasında olmalı (verilen {keep_daily})"
                ),
            });
        }
        if keep_weekly > MAX_KEEP_WEEKLY {
            return Err(StorageError::BackupInvalid {
                reason: format!(
                    "haftalık saklama 0..={MAX_KEEP_WEEKLY} arasında olmalı (verilen {keep_weekly})"
                ),
            });
        }
        Ok(Self {
            keep_daily,
            keep_weekly,
        })
    }
}

/// Yedeğin bu kasaya ait olup olmadığı. `Missing`: eski biçim (kimlik yok);
/// `Foreign`: başka kasanın yedeği. İkisi de **silinmez**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupAttribution {
    Own,
    Missing,
    Foreign,
}

/// Taranan tek yedek. `totalBytes` = klasör + varsa `.dimbak` arşivi.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSnapshot {
    pub name: String,
    /// Manifestteki zaman (yoksa adın taşıdığı damga), RFC3339.
    pub created_at: String,
    pub directory_bytes: u64,
    pub archive_bytes: u64,
    pub total_bytes: u64,
    pub attribution: BackupAttribution,
    /// Bu taramada silinecek olarak işaretlendi mi (yalnız `Own` olabilir).
    pub will_delete: bool,
    /// Planlama için çözülmüş zaman; serileştirilmez.
    #[serde(skip)]
    pub at: Option<OffsetDateTime>,
}

/// Taranan ama **yedek sayılmayan** girdi (silinmez, sessizce yutulmaz).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupIgnoredEntry {
    pub name: String,
    pub reason: String,
}

/// Saklama tarama/uygulama raporu. Sayılar **ölçülmüştür**; "silindi" yalnız
/// gerçekten diskten kalkan yedekler için yazılır.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRetentionReport {
    pub root: String,
    pub dry_run: bool,
    pub keep_daily: u32,
    pub keep_weekly: u32,
    pub snapshots: Vec<BackupSnapshot>,
    pub ignored: Vec<BackupIgnoredEntry>,
    pub scanned: usize,
    pub total_bytes: u64,
    pub candidate_count: usize,
    pub candidate_bytes: u64,
    pub deleted: Vec<BackupSnapshot>,
    pub freed_bytes: u64,
    pub errors: Vec<String>,
}

/// Ad damgası: `YYYYMMDD-HHMM` veya `YYYYMMDD-HHMMSS` (eski/yeni biçim).
fn parse_snapshot_name(name: &str) -> Option<(Date, Time)> {
    let rest = name.strip_prefix(NAME_PREFIX)?;
    let (date_part, time_part) = rest.split_once('-')?;
    if date_part.len() != 8 || !date_part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if !(time_part.len() == 4 || time_part.len() == 6)
        || !time_part.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let number = |slice: &str| slice.parse::<u32>().ok();
    let year = i32::try_from(number(&date_part[0..4])?).ok()?;
    let month = Month::try_from(u8::try_from(number(&date_part[4..6])?).ok()?).ok()?;
    let day = u8::try_from(number(&date_part[6..8])?).ok()?;
    let hour = u8::try_from(number(&time_part[0..2])?).ok()?;
    let minute = u8::try_from(number(&time_part[2..4])?).ok()?;
    let second = if time_part.len() == 6 {
        u8::try_from(number(&time_part[4..6])?).ok()?
    } else {
        0
    };
    let date = Date::from_calendar_date(year, month, day).ok()?;
    let time = Time::from_hms(hour, minute, second).ok()?;
    Some((date, time))
}

/// Dizin ağacının bayt toplamı (symlink'ler sayılmaz, izlenmez).
fn directory_bytes(path: &Path) -> io::Result<u64> {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push(entry.path());
            } else if file_type.is_file() {
                total += entry.metadata()?.len();
            }
        }
    }
    Ok(total)
}

/// Kök klasörü tarar: bizim ad biçimimizdeki klasörleri manifestle doğrular,
/// boyutlarını ölçer ve aidiyetini yazar. Kök yoksa/klasör değilse hata.
fn scan_with_ignored(
    root: &Path,
    vault_id: &str,
) -> Result<(Vec<BackupSnapshot>, Vec<BackupIgnoredEntry>), StorageError> {
    if !root.is_dir() {
        return Err(StorageError::BackupInvalid {
            reason: format!("yedek klasörü bulunamadı: {}", root.display()),
        });
    }
    let mut snapshots = Vec::new();
    let mut ignored = Vec::new();
    for entry in fs::read_dir(root).map_err(atomic::map_io)? {
        let entry = entry.map_err(atomic::map_io)?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(NAME_PREFIX) {
            continue;
        }
        let file_type = entry.file_type().map_err(atomic::map_io)?;
        if file_type.is_symlink() || !file_type.is_dir() {
            // Klasörün yanındaki `.dimbak` arşivi ayrı bir "yedek" değildir;
            // boyutu klasöre yazılır. Yalnız tanınan ad biçimindeki arşiv
            // sessizce atlanır; başka dosya `ignored` olarak raporlanır.
            let archive = name.strip_suffix(ARCHIVE_EXT);
            if archive.is_some_and(|base| parse_snapshot_name(base).is_some()) {
                continue;
            }
            ignored.push(BackupIgnoredEntry {
                name,
                reason: "klasör değil".to_string(),
            });
            continue;
        }
        let Some((date, time)) = parse_snapshot_name(&name) else {
            ignored.push(BackupIgnoredEntry {
                name,
                reason: "ad biçimi tanınmadı".to_string(),
            });
            continue;
        };
        let dir = entry.path();
        let manifest = match read_backup_manifest(&dir) {
            Ok(manifest) => manifest,
            Err(error) => {
                ignored.push(BackupIgnoredEntry {
                    name,
                    reason: format!("manifest okunamadı: {error}"),
                });
                continue;
            }
        };
        let stamp = PrimitiveDateTime::new(date, time).assume_utc();
        let (at, created_at) = match OffsetDateTime::parse(&manifest.created_at, &Rfc3339) {
            Ok(parsed) => (parsed, parsed.format(&Rfc3339).unwrap_or_default()),
            Err(_) => (stamp, stamp.format(&Rfc3339).unwrap_or_default()),
        };
        let directory_bytes = directory_bytes(&dir).map_err(atomic::map_io)?;
        let archive = root.join(format!("{name}{ARCHIVE_EXT}"));
        let archive_bytes = match fs::symlink_metadata(&archive) {
            Ok(meta) if meta.file_type().is_file() => meta.len(),
            _ => 0,
        };
        let attribution = match manifest.vault_id.as_deref() {
            Some(id) if id == vault_id => BackupAttribution::Own,
            Some(_) => BackupAttribution::Foreign,
            None => BackupAttribution::Missing,
        };
        snapshots.push(BackupSnapshot {
            name,
            created_at,
            directory_bytes,
            archive_bytes,
            total_bytes: directory_bytes + archive_bytes,
            attribution,
            will_delete: false,
            at: Some(at),
        });
    }
    snapshots.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| b.name.cmp(&a.name)));
    Ok((snapshots, ignored))
}

/// Politikaya göre `will_delete` işaretlerini yazar. En yeni yedek her zaman
/// korunur; kimliksiz/yabancı yedekler hiçbir koşulda işaretlenmez.
fn mark_retention(snapshots: &mut [BackupSnapshot], policy: RetentionPolicy) {
    for snapshot in snapshots.iter_mut() {
        snapshot.will_delete = false;
    }
    // `snapshots` zaten en yeniden eskiye sıralı.
    let mut keep: HashSet<usize> = HashSet::new();
    if snapshots.is_empty() {
        return;
    }
    keep.insert(0);
    let mut days: Vec<Date> = Vec::new();
    for (index, snapshot) in snapshots.iter().enumerate() {
        let Some(at) = snapshot.at else { continue };
        let day = at.date();
        if days.contains(&day) {
            continue;
        }
        days.push(day);
        keep.insert(index);
        if days.len() >= policy.keep_daily as usize {
            break;
        }
    }
    if policy.keep_weekly > 0 {
        let mut weeks: Vec<(i32, u8)> = Vec::new();
        for (index, snapshot) in snapshots.iter().enumerate() {
            let Some(at) = snapshot.at else { continue };
            // Takvim yılı + ISO hafta: yıl sınırında aynı ISO haftası iki gruba
            // düşebilir; bu yalnız **fazla** yedek tutar, fazla silmez.
            let week = (at.date().year(), at.date().iso_week());
            if weeks.contains(&week) {
                continue;
            }
            weeks.push(week);
            keep.insert(index);
            if weeks.len() >= policy.keep_weekly as usize {
                break;
            }
        }
    }
    for (index, snapshot) in snapshots.iter_mut().enumerate() {
        snapshot.will_delete = snapshot.attribution == BackupAttribution::Own
            && snapshot.at.is_some()
            && !keep.contains(&index);
    }
}

/// Tek yedeği siler: önce `.dimbak`, sonra klasör. Manifest **silme anında**
/// yeniden okunur; kimlik değişmişse hiçbir şey silinmez.
fn delete_snapshot(root: &Path, snapshot: &BackupSnapshot, vault_id: &str) -> Result<(), String> {
    let dir = root.join(&snapshot.name);
    let meta = fs::symlink_metadata(&dir).map_err(|error| format!("klasör okunamadı: {error}"))?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err("klasör değil; dokunulmadı".to_string());
    }
    let manifest = read_backup_manifest(&dir).map_err(|error| format!("manifest: {error}"))?;
    if manifest.vault_id.as_deref() != Some(vault_id) {
        return Err("kasa kimliği eşleşmiyor; dokunulmadı".to_string());
    }
    let archive = root.join(format!("{}{ARCHIVE_EXT}", snapshot.name));
    match fs::symlink_metadata(&archive) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err("arşiv symlink; dokunulmadı".to_string());
        }
        Ok(meta) if meta.is_file() => {
            fs::remove_file(&archive).map_err(|error| format!("arşiv silinemedi: {error}"))?;
        }
        Ok(_) => return Err("arşiv beklenen dosya değil; dokunulmadı".to_string()),
        Err(_) => {}
    }
    fs::remove_dir_all(&dir).map_err(|error| format!("klasör silinemedi: {error}"))?;
    Ok(())
}

fn build_report(
    root: &Path,
    policy: RetentionPolicy,
    dry_run: bool,
    snapshots: Vec<BackupSnapshot>,
    ignored: Vec<BackupIgnoredEntry>,
    deleted: Vec<BackupSnapshot>,
    errors: Vec<String>,
) -> BackupRetentionReport {
    let scanned = snapshots.len();
    let total_bytes = snapshots.iter().map(|snapshot| snapshot.total_bytes).sum();
    let candidate_count = snapshots
        .iter()
        .filter(|snapshot| snapshot.will_delete)
        .count();
    let candidate_bytes = snapshots
        .iter()
        .filter(|snapshot| snapshot.will_delete)
        .map(|snapshot| snapshot.total_bytes)
        .sum();
    BackupRetentionReport {
        root: root.to_string_lossy().to_string(),
        dry_run,
        keep_daily: policy.keep_daily,
        keep_weekly: policy.keep_weekly,
        snapshots,
        ignored,
        scanned,
        total_bytes,
        candidate_count,
        candidate_bytes,
        deleted,
        freed_bytes: 0,
        errors,
    }
}

/// Tarama + plan. Hiçbir dosya silinmez; rapor **ne silineceğini** söyler.
pub fn plan_retention(
    root: &Path,
    vault_id: &str,
    policy: RetentionPolicy,
) -> Result<BackupRetentionReport, StorageError> {
    let (mut snapshots, ignored) = scan_with_ignored(root, vault_id)?;
    mark_retention(&mut snapshots, policy);
    Ok(build_report(
        root,
        policy,
        true,
        snapshots,
        ignored,
        Vec::new(),
        Vec::new(),
    ))
}

/// Planı uygular. `dry_run` iken `plan_retention` ile aynı raporu döner.
/// Silme hataları tek tek `errors` içinde raporlanır; diğer yedekler denenir.
pub fn apply_retention(
    root: &Path,
    vault_id: &str,
    policy: RetentionPolicy,
    dry_run: bool,
) -> Result<BackupRetentionReport, StorageError> {
    if dry_run {
        return plan_retention(root, vault_id, policy);
    }
    let (mut snapshots, ignored) = scan_with_ignored(root, vault_id)?;
    mark_retention(&mut snapshots, policy);
    let mut deleted = Vec::new();
    let mut errors = Vec::new();
    let mut freed_bytes = 0u64;
    // En eskiden yeniye sileriz: bir hata yeni yedekleri engellemez.
    let mut order: Vec<usize> = (0..snapshots.len())
        .filter(|index| snapshots[*index].will_delete)
        .collect();
    order.sort_by(|a, b| snapshots[*a].at.cmp(&snapshots[*b].at));
    for index in order {
        let snapshot = snapshots[index].clone();
        match delete_snapshot(root, &snapshot, vault_id) {
            Ok(()) => {
                freed_bytes += snapshot.total_bytes;
                deleted.push(snapshot);
            }
            Err(error) => errors.push(format!("{}: {error}", snapshot.name)),
        }
    }
    let mut report = build_report(root, policy, false, snapshots, ignored, deleted, errors);
    report.freed_bytes = freed_bytes;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::{BACKUP_FORMAT, BACKUP_VERSION, DB_FILE, MANIFEST_FILE, VAULT_FILE};
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const VAULT: &str = "vault-A";

    /// Elle manifest yazar: `created_at` ve `vaultId` testin denetiminde olur
    /// (üretim yolu `write_backup_manifest_with_vault`, zamanı `now`dan alır).
    fn write_snapshot(
        root: &Path,
        name: &str,
        created_at: &str,
        vault_id: Option<&str>,
        payload: usize,
    ) -> PathBuf {
        let dir = root.join(name);
        fs::create_dir_all(dir.join("blobs")).expect("dizin");
        fs::write(dir.join(VAULT_FILE), vec![0u8; payload]).expect("vault");
        fs::write(dir.join(DB_FILE), vec![1u8; payload]).expect("db");
        fs::write(dir.join("blobs").join("b1"), vec![2u8; payload]).expect("blob");
        let manifest = crate::backup::BackupManifest {
            format: BACKUP_FORMAT.to_string(),
            version: BACKUP_VERSION,
            created_at: created_at.to_string(),
            vault_id: vault_id.map(str::to_string),
            vault_sha256: "0".repeat(64),
            vault_bytes: None,
            db_sha256: "1".repeat(64),
            db_bytes: None,
            blobs: Vec::new(),
        };
        fs::write(
            dir.join(MANIFEST_FILE),
            serde_json::to_vec(&manifest).expect("manifest"),
        )
        .expect("yaz");
        dir
    }

    fn policy(daily: u32, weekly: u32) -> RetentionPolicy {
        RetentionPolicy::new(daily, weekly).expect("politika")
    }

    fn names(report: &BackupRetentionReport) -> Vec<&str> {
        report
            .snapshots
            .iter()
            .filter(|snapshot| snapshot.will_delete)
            .map(|snapshot| snapshot.name.as_str())
            .collect()
    }

    #[test]
    fn scan_measures_owned_snapshots_and_archives() {
        let root = TempDir::new().expect("kök");
        write_snapshot(
            root.path(),
            "DimYedek-20261111-090000",
            "2026-11-11T09:00:00Z",
            Some(VAULT),
            10,
        );
        let dir = write_snapshot(
            root.path(),
            "DimYedek-20261110-090000",
            "2026-11-10T09:00:00Z",
            Some(VAULT),
            20,
        );
        fs::write(
            root.path().join("DimYedek-20261110-090000.dimbak"),
            vec![9u8; 100],
        )
        .expect("arşiv");
        // Başka ad (bizim değil) taranmaz.
        fs::create_dir_all(root.path().join("başka-klasör")).expect("dizin");

        let report = plan_retention(root.path(), VAULT, policy(7, 4)).expect("plan");
        assert_eq!(report.scanned, 2);
        assert_eq!(report.ignored.len(), 0);
        // 3 dosya × payload + manifest; arşiv ayrı sayılır.
        let older = report
            .snapshots
            .iter()
            .find(|snapshot| snapshot.name == "DimYedek-20261110-090000")
            .expect("eski");
        assert_eq!(older.directory_bytes, directory_bytes(&dir).expect("boyut"));
        assert_eq!(older.archive_bytes, 100);
        assert_eq!(older.total_bytes, older.directory_bytes + 100);
        assert_eq!(older.attribution, BackupAttribution::Own);
        assert!(older.created_at.starts_with("2026-11-10T09:00:00Z"));
        // En yeni her zaman korunur.
        assert!(!report.snapshots[0].will_delete);
        assert_eq!(
            report.total_bytes,
            report
                .snapshots
                .iter()
                .map(|snapshot| snapshot.total_bytes)
                .sum::<u64>()
        );
    }

    #[test]
    fn plan_keeps_last_days_and_weeks_and_flags_older_owned() {
        let root = TempDir::new().expect("kök");
        for (name, at) in [
            ("DimYedek-20261111-090000", "2026-11-11T09:00:00Z"),
            ("DimYedek-20261110-090000", "2026-11-10T09:00:00Z"),
            ("DimYedek-20261109-090000", "2026-11-09T09:00:00Z"),
            ("DimYedek-20261108-090000", "2026-11-08T09:00:00Z"),
            ("DimYedek-20261101-090000", "2026-11-01T09:00:00Z"),
            ("DimYedek-20261020-090000", "2026-10-20T09:00:00Z"),
            ("DimYedek-20261001-090000", "2026-10-01T09:00:00Z"),
        ] {
            write_snapshot(root.path(), name, at, Some(VAULT), 5);
        }

        // 2 gün + 2 hafta: en yeni gün (11), ikinci gün (10), son iki haftanın
        // en yenisi (11 ve 8) tutulur; 9 aynı haftada daha eski olduğu için düşer.
        let report = plan_retention(root.path(), VAULT, policy(2, 2)).expect("plan");
        let mut flagged = names(&report);
        flagged.sort_unstable();
        assert_eq!(
            flagged,
            vec![
                "DimYedek-20261001-090000",
                "DimYedek-20261020-090000",
                "DimYedek-20261101-090000",
                "DimYedek-20261109-090000",
            ]
        );
        assert_eq!(report.candidate_count, 4);
        assert!(report.dry_run);
    }

    #[test]
    fn newest_survives_even_when_daily_budget_is_one() {
        let root = TempDir::new().expect("kök");
        write_snapshot(
            root.path(),
            "DimYedek-20261110-090000",
            "2026-11-10T09:00:00Z",
            Some(VAULT),
            5,
        );
        write_snapshot(
            root.path(),
            "DimYedek-20261111-090000",
            "2026-11-11T09:00:00Z",
            Some(VAULT),
            5,
        );
        let report = plan_retention(root.path(), VAULT, policy(1, 0)).expect("plan");
        assert!(!report.snapshots[0].will_delete, "en yeni korunur");
        assert_eq!(report.candidate_count, 1);
    }

    #[test]
    fn missing_or_foreign_vault_id_is_never_deleted() {
        let root = TempDir::new().expect("kök");
        write_snapshot(
            root.path(),
            "DimYedek-20250101-090000",
            "2025-01-01T09:00:00Z",
            None,
            5,
        );
        write_snapshot(
            root.path(),
            "DimYedek-20250102-090000",
            "2025-01-02T09:00:00Z",
            Some("vault-B"),
            5,
        );
        let report = plan_retention(root.path(), VAULT, policy(1, 0)).expect("plan");
        assert_eq!(report.scanned, 2);
        assert_eq!(report.candidate_count, 0);
        assert!(report
            .snapshots
            .iter()
            .any(|snapshot| snapshot.attribution == BackupAttribution::Missing));
        assert!(report
            .snapshots
            .iter()
            .any(|snapshot| snapshot.attribution == BackupAttribution::Foreign));
        assert!(report
            .snapshots
            .iter()
            .all(|snapshot| !snapshot.will_delete));
    }

    #[test]
    fn apply_deletes_archive_then_directory_and_reports_freed_bytes() {
        let root = TempDir::new().expect("kök");
        write_snapshot(
            root.path(),
            "DimYedek-20261101-090000",
            "2026-11-01T09:00:00Z",
            Some(VAULT),
            7,
        );
        write_snapshot(
            root.path(),
            "DimYedek-20261102-090000",
            "2026-11-02T09:00:00Z",
            Some(VAULT),
            7,
        );
        fs::write(
            root.path().join("DimYedek-20261101-090000.dimbak"),
            vec![3u8; 50],
        )
        .expect("arşiv");

        let before = plan_retention(root.path(), VAULT, policy(1, 0)).expect("plan");
        assert_eq!(before.candidate_count, 1);
        let planned = before.candidate_bytes;

        let report = apply_retention(root.path(), VAULT, policy(1, 0), false).expect("uygula");
        assert_eq!(report.deleted.len(), 1);
        assert_eq!(report.deleted[0].name, "DimYedek-20261101-090000");
        assert_eq!(report.freed_bytes, planned);
        assert!(!root.path().join("DimYedek-20261101-090000").exists());
        assert!(!root.path().join("DimYedek-20261101-090000.dimbak").exists());
        // Yeni yedek yerinde.
        assert!(root.path().join("DimYedek-20261102-090000").is_dir());
        // İkinci uygulama silmez (idempotent).
        let again = apply_retention(root.path(), VAULT, policy(1, 0), false).expect("tekrar");
        assert_eq!(again.deleted.len(), 0);
        assert_eq!(again.freed_bytes, 0);
    }

    #[test]
    fn delete_refuses_when_manifest_identity_changed_after_scan() {
        let root = TempDir::new().expect("kök");
        let dir = write_snapshot(
            root.path(),
            "DimYedek-20261101-090000",
            "2026-11-01T09:00:00Z",
            Some(VAULT),
            5,
        );
        write_snapshot(
            root.path(),
            "DimYedek-20261102-090000",
            "2026-11-02T09:00:00Z",
            Some(VAULT),
            5,
        );
        let plan = plan_retention(root.path(), VAULT, policy(1, 0)).expect("plan");
        let victim = plan
            .snapshots
            .iter()
            .find(|snapshot| snapshot.will_delete)
            .expect("silinecek")
            .clone();
        // Tarama ile silme arasında manifest başka kasaya dönmüş gibi.
        write_snapshot(
            root.path(),
            "DimYedek-20261101-090000",
            "2026-11-01T09:00:00Z",
            Some("vault-B"),
            5,
        );
        let error = delete_snapshot(root.path(), &victim, VAULT).expect_err("kimlik kapısı");
        assert!(error.contains("eşleşmiyor"), "{error}");
        assert!(dir.is_dir(), "klasör silinmemeli");
    }

    #[test]
    fn archive_that_is_not_a_file_blocks_directory_deletion() {
        let root = TempDir::new().expect("kök");
        let dir = write_snapshot(
            root.path(),
            "DimYedek-20261101-090000",
            "2026-11-01T09:00:00Z",
            Some(VAULT),
            5,
        );
        write_snapshot(
            root.path(),
            "DimYedek-20261102-090000",
            "2026-11-02T09:00:00Z",
            Some(VAULT),
            5,
        );
        // `.dimbak` bir **klasör**: arşiv beklenen dosya değil. Yedek bütünlüğü
        // bozulmasın diye klasöre de dokunulmaz (önce arşiv, sonra klasör).
        fs::create_dir(root.path().join("DimYedek-20261101-090000.dimbak")).expect("klasör");

        let report = apply_retention(root.path(), VAULT, policy(1, 0), false).expect("uygula");
        assert_eq!(report.deleted.len(), 0);
        assert_eq!(report.freed_bytes, 0);
        assert_eq!(report.errors.len(), 1);
        assert!(
            report.errors[0].contains("arşiv beklenen dosya değil"),
            "{:?}",
            report.errors
        );
        assert!(dir.is_dir(), "klasör silinmemeli");
    }

    #[test]
    fn invalid_policy_and_missing_root_are_rejected() {
        assert!(RetentionPolicy::new(0, 4).is_err());
        assert!(RetentionPolicy::new(366, 4).is_err());
        assert!(RetentionPolicy::new(7, 53).is_err());
        assert!(RetentionPolicy::new(7, 0).is_ok());

        let root = TempDir::new().expect("kök");
        let missing = root.path().join("yok");
        let error = plan_retention(&missing, VAULT, policy(7, 4)).expect_err("kök yok");
        assert_eq!(error.code(), "E_BACKUP_INVALID");
    }

    #[test]
    fn symlinks_and_non_directories_are_ignored_not_deleted() {
        let root = TempDir::new().expect("kök");
        write_snapshot(
            root.path(),
            "DimYedek-20261101-090000",
            "2026-11-01T09:00:00Z",
            Some(VAULT),
            5,
        );
        fs::write(root.path().join("DimYedek-20261102-090000"), b"dosya").expect("dosya");
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            root.path().join("DimYedek-20261101-090000"),
            root.path().join("DimYedek-20261103-090000"),
        )
        .expect("symlink");

        let report = apply_retention(root.path(), VAULT, policy(1, 0), false).expect("uygula");
        assert_eq!(report.scanned, 1, "yalnız gerçek klasör yedek sayılır");
        assert_eq!(report.ignored.len(), if cfg!(unix) { 2 } else { 1 });
        assert_eq!(report.candidate_count, 0, "en yeni korunur");
        assert!(root.path().join("DimYedek-20261103-090000").exists());
    }
}
