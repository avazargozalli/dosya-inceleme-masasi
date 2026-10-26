//! F15-T06 — **gerçek** disk dolması tatbikatı (elle koşulur, `#[ignore]`).
//!
//! Kurgu değil: macOS'ta küçük bir APFS imajı bağlanır
//! (`hdiutil create -size 8m -type SPARSE`), dolana kadar doldurulur ve
//! ürünün gerçek yazma yolları oraya yönlendirilir. Amaç üç iddiayı ölçmek:
//!
//! 1. `Database::backup_to` disk dolduğunda `E_DISK_FULL` verir,
//! 2. hedefte **yarım dosya kalmaz** (geçici ad temizlenir),
//! 3. kaynak kasa bozulmadan açılır; yer açılınca aynı çağrı başarılı olur.
//!
//! Neden `#[ignore]`: imaj bağlama ayrıcalık/ortam ister ve CI'da paylaşılan
//! runner'da kırılgan olabilir. Deterministik ENOSPC davranışı birim
//! testlerinde (`atomic::tests::enospc_is_mapped_and_temp_is_removed`)
//! her platformda koşar; bu dosya gerçek işletim sistemi hatasını doğrular.
//!
//! Koşum: `cargo test -p dim-storage --test disk_full -- --ignored --nocapture`

#![allow(clippy::disallowed_methods)] // gerekçe: test kurulumu `hdiutil`/`df` çağırır; ürün yolunda süreç başlatma yok (ADR-05).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use dim_storage::atomic;
use dim_storage::backup;
use dim_storage::{Database, DatabaseKey};

/// Bağlı imajı test biterken (panik dâhil) ayırır.
struct MountedVolume {
    mount_point: PathBuf,
    image: PathBuf,
}

impl Drop for MountedVolume {
    fn drop(&mut self) {
        let _ = Command::new("hdiutil")
            .args(["detach", "-force", "-quiet"])
            .arg(&self.mount_point)
            .status();
        let _ = fs::remove_file(&self.image);
    }
}

/// Küçük bir APFS imajı oluşturup bağlar; ortam desteklemiyorsa `None`.
///
/// Birim adı test başına **tekil** (`DIMENOSPCBACKUP`, `DIMENOSPCATOMIC`):
/// testler paralel koşarsa aynı ada bağlanan iki imaj `DIMENOSPC 1` üretir ve
/// test yanlış birime bakar. Bu dosyayı `--test-threads=1` ile koşun.
fn mount_tiny_volume(tag: &str) -> Option<MountedVolume> {
    let volume_name = format!("DIMENOSPC{}", tag.to_uppercase());
    let image = std::env::temp_dir().join(format!(
        "dim-enospc-{tag}-{}.sparseimage",
        std::process::id()
    ));
    let _ = fs::remove_file(&image);
    let created = Command::new("hdiutil")
        .args([
            "create",
            "-size",
            "8m",
            "-fs",
            "APFS",
            "-volname",
            &volume_name,
            "-type",
            "SPARSE",
            "-quiet",
        ])
        .arg(&image)
        .status();
    if !matches!(created, Ok(status) if status.success()) {
        eprintln!("ATLANDI: hdiutil imaj oluşturamadı");
        return None;
    }
    let attached = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-quiet"])
        .arg(&image)
        .output();
    if !matches!(&attached, Ok(out) if out.status.success()) {
        eprintln!("ATLANDI: imaj bağlanamadı");
        let _ = fs::remove_file(&image);
        return None;
    }
    let mount_point = PathBuf::from(format!("/Volumes/{volume_name}"));
    if !mount_point.is_dir() {
        eprintln!("ATLANDI: bağlama noktası yok ({volume_name})");
        let _ = Command::new("hdiutil")
            .args(["detach", "-force", "-quiet"])
            .arg(&image)
            .status();
        let _ = fs::remove_file(&image);
        return None;
    }
    Some(MountedVolume { mount_point, image })
}

/// Diski gerçekten doldurur, sonra `leave_free` kadar boşluk bırakacak şekilde
/// doldurucuyu yeniden yazar ve ölçümü yazdırır.
///
/// Neden iki adım: **tamamen dolu APFS'te `set_len` bile `ENOSPC` verir**
/// (küçültme meta veri yazar). Bu yüzden doldurucu silinir (silme yer açar)
/// ve biraz küçüğü yeniden yazılır.
fn fill_volume(mount: &Path, leave_free: u64) -> u64 {
    let filler = mount.join("doldur.bin");
    let chunk = vec![0xAB_u8; 64 * 1024];
    let mut written = 0u64;
    {
        let mut file = fs::File::create(&filler).expect("doldurucu");
        loop {
            match file.write_all(&chunk) {
                Ok(()) => written += chunk.len() as u64,
                Err(err) if atomic::is_no_space(&err) => break,
                Err(err) => panic!("beklenmeyen hata: {err}"),
            }
        }
        let _ = file.sync_all();
    }
    assert!(written > 0, "disk gerçekten dolmalı");

    fs::remove_file(&filler).expect("doldurucu silinir");
    let target = written.saturating_sub(leave_free);
    let mut file = fs::File::create(&filler).expect("doldurucu yeniden");
    let mut left = target;
    while left > 0 {
        let take = left.min(chunk.len() as u64) as usize;
        file.write_all(&chunk[..take]).expect("yazılır");
        left -= take as u64;
    }
    file.sync_all().expect("sync");
    written
}

fn print_free(mount: &Path, label: &str) {
    let out = Command::new("df").arg("-h").arg(mount).output();
    if let Ok(out) = out {
        let text = String::from_utf8_lossy(&out.stdout);
        if let Some(line) = text.lines().nth(1) {
            eprintln!("{label}: {line}");
        }
    }
}

/// Bir kasa kurup içine yazı yazar (yedek boyutu sıfır olmasın).
fn seeded_vault(dir: &Path, key: &DatabaseKey) -> Database {
    let db = Database::open(&dir.join("vault.db"), key).expect("kasa açılır");
    db.create_case(&dim_storage::NewCase {
        title: "Disk dolması tatbikatı".to_string(),
        case_kind: dim_storage::CaseKind::Review,
        reference_code: None,
        jurisdiction_label: None,
        practice_area: None,
        sensitivity: dim_storage::Sensitivity::Normal,
        default_ai_policy: dim_storage::AiPolicy::Off,
    })
    .expect("dosya");
    db
}

#[test]
#[ignore = "gerçek imaj bağlar; elle koşulur"]
fn backup_into_full_disk_fails_cleanly_and_keeps_vault_usable() {
    let Some(volume) = mount_tiny_volume("backup") else {
        return;
    };
    let work = tempfile::tempdir().expect("çalışma dizini");
    let key = DatabaseKey::new([9u8; 32]);
    let db = seeded_vault(work.path(), &key);
    let db_bytes = fs::metadata(work.path().join("vault.db"))
        .expect("meta")
        .len();

    let filled = fill_volume(&volume.mount_point, 96 * 1024);
    eprintln!("doldurucu: {filled} bayt (96 KiB boşluk bırakıldı)");
    print_free(&volume.mount_point, "dolu disk");
    eprintln!("kaynak kasa: {db_bytes} bayt");

    // --- 1) Yedek: E_DISK_FULL, hedefte dosya kalmaz ------------------------
    let dest = volume.mount_point.join("vault.db");
    let err = db.backup_to(&dest, &key).expect_err("disk dolu");
    eprintln!("backup_to hatası: {} — {err}", err.code());
    if let dim_storage::StorageError::Sql(inner) = &err {
        eprintln!(
            "  ayrıntı: {inner:?} · kod: {:?}",
            inner.sqlite_error_code()
        );
    }
    assert_eq!(err.code(), "E_DISK_FULL", "ayrı kod beklenir: {err}");
    assert!(!dest.exists(), "yarım yedek dosyası kalmamalı");
    let leftovers: Vec<String> = fs::read_dir(&volume.mount_point)
        .expect("liste")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|name| name != "doldur.bin")
        .collect();
    assert!(leftovers.is_empty(), "geçici dosya kaldı: {leftovers:?}");

    // --- 2) Kaynak kasa bozulmadı ------------------------------------------
    db.integrity_check().expect("kasa bütün");
    assert_eq!(db.list_cases(true).expect("dosyalar").len(), 1);

    // --- 3) Yer açılınca aynı çağrı başarılı, manifest doğrulanıyor --------
    fs::remove_file(volume.mount_point.join("doldur.bin")).expect("doldurucu silinir");
    db.backup_to(&dest, &key).expect("yer açıldı, yedek alınır");
    fs::write(volume.mount_point.join(backup::VAULT_FILE), b"kasa-kunye").expect("künye");
    let written = backup::write_backup_manifest(&volume.mount_point).expect("manifest yazılır");
    let check = backup::verify_backup_dir(&volume.mount_point).expect("yeniden doğrulanmadı");
    assert_eq!(
        check.manifest.db_bytes,
        Some(fs::metadata(&dest).expect("meta").len())
    );
    assert_eq!(check.manifest.vault_sha256, written.manifest.vault_sha256);
    eprintln!(
        "yer açıldıktan sonra: {} dosya, {} bayt doğrulandı",
        check.verified_files, check.verified_bytes
    );
}

#[test]
#[ignore = "gerçek imaj bağlar; elle koşulur"]
fn atomic_writes_leave_nothing_behind_on_full_disk() {
    let Some(volume) = mount_tiny_volume("atomic") else {
        return;
    };
    fill_volume(&volume.mount_point, 96 * 1024);
    print_free(&volume.mount_point, "dolu disk");

    // 4 MiB'lik atomik yazma: sığmaz → hata, hedef yok, geçici yok.
    let big = vec![7u8; 4 * 1024 * 1024];
    let target = volume.mount_point.join("manifest.json");
    let err = atomic::write_atomic(&target, &big).expect_err("sığmaz");
    eprintln!("write_atomic hatası: {} — {err}", err.code());
    assert_eq!(err.code(), "E_DISK_FULL");
    assert!(!target.exists(), "yarım hedef kalmamalı");
    let leftovers: Vec<String> = fs::read_dir(&volume.mount_point)
        .expect("liste")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|name| name != "doldur.bin")
        .collect();
    assert!(leftovers.is_empty(), "geçici dosya kaldı: {leftovers:?}");

    // Kopya yolu da aynı sözleşmeyi tutmalı.
    let src = std::env::temp_dir().join(format!("dim-kaynak-{}.bin", std::process::id()));
    fs::write(&src, vec![1u8; 2 * 1024 * 1024]).expect("kaynak");
    let copy_dest = volume.mount_point.join("kopya.bin");
    let err = atomic::copy_atomic(&src, &copy_dest).expect_err("sığmaz");
    eprintln!("copy_atomic hatası: {} — {err}", err.code());
    assert_eq!(err.code(), "E_DISK_FULL");
    assert!(!copy_dest.exists());
    let leftovers: Vec<String> = fs::read_dir(&volume.mount_point)
        .expect("liste")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|name| name != "doldur.bin")
        .collect();
    assert!(leftovers.is_empty(), "geçici dosya kaldı: {leftovers:?}");
    let _ = fs::remove_file(&src);
}
