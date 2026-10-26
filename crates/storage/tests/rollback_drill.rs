//! F16 — **atomik geri dönüş tatbikatı**: süreç öldürme + gerçek disk dolması.
//!
//! İki ayrı senaryo, ikisi de gerçek dosyalarla:
//!
//! 1. `killing_rollback_before_install_keeps_vault_readable` — geri dönüşün
//!    tam ortasında (güvenlik kopyası hazır, yedek henüz yerine konmamışken)
//!    süreç `SIGKILL` ile öldürülür. Kasa dosyası **yerinde** ve açılabilir
//!    olmalı; kalan geçici dosya bir sonraki denemede temizlenmeli.
//! 2. `rollback_on_full_disk_fails_cleanly_and_vault_stays` (elle, `#[ignore]`)
//!    — gerçek 8 MB APFS imajı doldurulur, geri dönüş `E_DISK_FULL` vermeli,
//!    kasa bozulmamalı, artık kalmamalı; yer açılınca aynı çağrı başarılı olmalı.
//!
//! Tatbikat kancası `atomic::drill_pause` yalnız **hata ayıklama**
//! derlemesinde çalışır; ürün derlemesinde hiçbir şey yapmaz.
//!
//! Koşum:
//! `cargo test -p dim-storage --test rollback_drill`
//! `cargo test -p dim-storage --test rollback_drill -- --ignored --nocapture` (imaj)

#![allow(clippy::disallowed_methods)] // gerekçe: tatbikat kendi sürecini ve `hdiutil`/`df` çağırır; ürün yolunda süreç başlatma yok (ADR-05).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use dim_storage::{
    rollback_vault, AiPolicy, CaseKind, Database, DatabaseKey, NewCase, Sensitivity,
};

fn key() -> DatabaseKey {
    DatabaseKey::new([23u8; 32])
}

fn create_case(db: &Database, title: &str) {
    db.create_case(&NewCase {
        title: title.to_string(),
        case_kind: CaseKind::Review,
        reference_code: None,
        jurisdiction_label: None,
        practice_area: None,
        sensitivity: Sensitivity::Normal,
        default_ai_policy: AiPolicy::Off,
    })
    .expect("dosya kaydı");
}

fn titles(db: &Database) -> Vec<String> {
    let mut stmt = db
        .conn()
        .prepare("SELECT title FROM cases ORDER BY title")
        .expect("hazırla");
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("sorgu");
    rows.map(|row| row.expect("satır")).collect()
}

fn entries(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .expect("liste")
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect()
}

fn tagged_temps(dir: &Path) -> Vec<String> {
    let mut found: Vec<String> = entries(dir)
        .into_iter()
        .filter(|name| name.starts_with(".vault.db.rollback-"))
        .collect();
    found.sort();
    found
}

/// Tatbikat kasası: güncel şemada kasa + **doğrulanmış** yedek + sonradan
/// eklenen kayıt. Geri dönüş yalnız `Eski dosya`yı bırakmalı.
fn seed_vault(dir: &Path) -> (PathBuf, PathBuf) {
    let path = dir.join("vault.db");
    let backup = dir.join("yedek.db");
    let db = Database::open(&path, &key()).expect("kasa açılır");
    create_case(&db, "Eski dosya");
    db.backup_to(&backup, &key()).expect("yedek alınır");
    create_case(&db, "Yeni dosya");
    drop(db);
    (path, backup)
}

/// Tatbikatın çocuk ayağı: gerçek geri dönüşü başlatır ve kancada bekler.
///
/// Doğrudan koşulduğunda (`DIM_DRILL_CHILD` yok) hiçbir şey yapmaz; bu yüzden
/// normal test koşusunda zararsızdır.
#[test]
fn drill_child_rolls_back() {
    if std::env::var("DIM_DRILL_CHILD").is_err() {
        return;
    }
    let path = PathBuf::from(std::env::var("DIM_DRILL_VAULT").expect("kasa yolu"));
    let backup = PathBuf::from(std::env::var("DIM_DRILL_BACKUP").expect("yedek yolu"));
    // `DIM_DRILL_PAUSE=before_install` verildiğinde burada bekler; ebeveyn
    // süreci bu noktada öldürür.
    let _ = rollback_vault(&path, &backup, &key());
}

#[test]
fn killing_rollback_before_install_keeps_vault_readable() {
    if !cfg!(debug_assertions) {
        eprintln!("ATLANDI: tatbikat kancası yalnız hata ayıklama derlemesinde etkin");
        return;
    }
    let dir = tempfile::tempdir().expect("dizin");
    let (path, backup) = seed_vault(dir.path());

    let exe = std::env::current_exe().expect("test ikilisi");
    let mut child = Command::new(exe)
        .args(["--exact", "drill_child_rolls_back", "--nocapture"])
        .env("DIM_DRILL_CHILD", "1")
        .env("DIM_DRILL_PAUSE", "before_install")
        .env("DIM_DRILL_PAUSE_MS", "120000")
        .env("DIM_DRILL_VAULT", &path)
        .env("DIM_DRILL_BACKUP", &backup)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("çocuk süreç başlar");

    // Güvenlik kopyası görünene kadar bekle: çocuk tam "yedeği yerine koymadan
    // önce" noktasında duruyor.
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut safety_seen = false;
    while Instant::now() < deadline {
        if entries(dir.path())
            .iter()
            .any(|name| name.ends_with(".safety"))
        {
            safety_seen = true;
            break;
        }
        if let Some(status) = child.try_wait().expect("durum") {
            panic!("çocuk beklenen noktaya gelmeden bitti: {status:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        safety_seen,
        "çocuk güvenlik kopyasına ulaşmadı; tatbikat kancası etkin mi?"
    );

    child.kill().expect("SIGKILL");
    let status = child.wait().expect("çocuk toplanır");
    assert!(
        !status.success(),
        "süreç öldürülmüş olmalı, durum: {status:?}"
    );

    // --- 1) Kasa dosyası **yerinde**: eski sıra burada dosyayı taşımıştı ve
    //        `Database::open` boş bir kasa yaratacaktı.
    assert!(path.is_file(), "kasa dosyası yerinde kalmalı");
    let db = Database::open(&path, &key()).expect("öldürmeden sonra kasa açılır");
    assert_eq!(
        titles(&db),
        vec!["Eski dosya".to_string(), "Yeni dosya".to_string()],
        "yedeği yerine koyma adımı çalışmadı: güncelleme sonrası veri durur"
    );
    assert_eq!(db.schema_version(), dim_storage::CURRENT_SCHEMA_VERSION);
    drop(db);

    // --- 2) Çökme izi: yarım kalan geçici dosya duruyor (kanıt), güvenlik
    //        kopyası da duruyor.
    let leftovers = tagged_temps(dir.path());
    assert!(
        !leftovers.is_empty(),
        "çökme geçici dosya bırakmalı (kanıt); dizin: {:?}",
        entries(dir.path())
    );
    assert!(
        entries(dir.path())
            .iter()
            .any(|name| name.ends_with(".safety")),
        "güvenlik kopyası çökmeden sonra da durmalı"
    );

    // --- 3) Tekrar deneme: artıkları temizler, veriyi geri koyar.
    let outcome = rollback_vault(&path, &backup, &key()).expect("tekrar deneme başarılı");
    assert_eq!(
        outcome.restored_version,
        dim_storage::CURRENT_SCHEMA_VERSION
    );
    assert_eq!(
        outcome.swept_temps,
        leftovers.len(),
        "çökmenin bıraktığı geçici dosyalar temizlenir: {leftovers:?}"
    );
    assert!(
        tagged_temps(dir.path()).is_empty(),
        "geri dönüş sonrası etiketli artık kalmamalı: {:?}",
        tagged_temps(dir.path())
    );
    assert_eq!(
        titles(&Database::open(&path, &key()).expect("geri konan kasa")),
        vec!["Eski dosya".to_string()],
        "geri dönüş yedekteki veriyi getirir"
    );
}

// ---------------------------------------------------------------------------
// Gerçek disk dolması (elle koşulur)
// ---------------------------------------------------------------------------

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
/// Birim adı test başına **tekil** olmalı; aynı ada iki imaj bağlanırsa test
/// yanlış birime bakar (`DIMENOSPC 1`). Bu dosyayı `--test-threads=1` ile koşun.
fn mount_tiny_volume(tag: &str) -> Option<MountedVolume> {
    let volume_name = format!("DIMDRILL{}", tag.to_uppercase());
    let image = std::env::temp_dir().join(format!(
        "dim-drill-{tag}-{}.sparseimage",
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

/// Diski gerçekten doldurur, `leave_free` kadar boşluk bırakır.
///
/// Tamamen dolu APFS'te `set_len` bile `ENOSPC` verdiği için doldurucu silinip
/// biraz küçüğü yeniden yazılır (bkz. `tests/disk_full.rs`).
fn fill_volume(mount: &Path, leave_free: u64) -> u64 {
    use std::io::Write;
    let filler = mount.join("doldur.bin");
    let chunk = vec![0xAB_u8; 64 * 1024];
    let mut written = 0u64;
    {
        let mut file = fs::File::create(&filler).expect("doldurucu");
        loop {
            match file.write_all(&chunk) {
                Ok(()) => written += chunk.len() as u64,
                Err(err) if dim_storage::atomic::is_no_space(&err) => break,
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

#[test]
#[ignore = "gerçek APFS imajı bağlar; elle koşulur"]
fn rollback_on_full_disk_fails_cleanly_and_vault_stays() {
    let Some(volume) = mount_tiny_volume("rollback") else {
        return;
    };
    let mount = volume.mount_point.clone();
    let path = mount.join("vault.db");
    let backup = mount.join("yedek.db");

    let db = Database::open(&path, &key()).expect("kasa açılır");
    for index in 0..300 {
        create_case(&db, &format!("Eski dosya {index:03}"));
    }
    db.backup_to(&backup, &key()).expect("yedek alınır");
    create_case(&db, "Yeni dosya");
    drop(db);

    let vault_bytes = fs::metadata(&path).expect("meta").len();
    let backup_bytes = fs::metadata(&backup).expect("meta").len();
    eprintln!("kasa {vault_bytes} bayt · yedek {backup_bytes} bayt");
    assert!(
        vault_bytes > 256 * 1024,
        "kasa, kalan boşluktan büyük olmalı: {vault_bytes}"
    );
    let before = fs::read(&path).expect("kasa baytları");

    let filled = fill_volume(&mount, 48 * 1024);
    eprintln!("doldurucu: {filled} bayt (48 KiB boşluk)");

    let err = rollback_vault(&path, &backup, &key()).expect_err("disk dolu");
    eprintln!("rollback hatası: {} — {err}", err.code());
    assert_eq!(err.code(), "E_DISK_FULL", "ayrı kod beklenir: {err}");

    // Kasa **yerinde** ve baytları değişmedi. Disk doluyken kasa açılamıyor
    // (SQLite WAL için yer bulamıyor) ama bu "yanlış anahtar" değil,
    // `E_DISK_FULL` olmalı: kullanıcıya "kasanız bozuldu" demek yanıltıcı.
    assert!(path.is_file(), "kasa dosyası yerinde kalmalı");
    let bytes_after = fs::read(&path).expect("kasa okunur");
    assert_eq!(
        bytes_after.len(),
        usize::try_from(vault_bytes).expect("boyut")
    );
    assert_eq!(bytes_after, before, "kasa baytları değişmemeli");
    let err = Database::open(&path, &key()).expect_err("dolu diskte kasa açılamaz");
    eprintln!("dolu diskte açma hatası: {} — {err}", err.code());
    assert_eq!(
        err.code(),
        "E_DB_IO",
        "yanlış anahtar değil, dosya sistemi hatası: {err}"
    );

    // Bizim geçici/ güvenlik artıklarımız kalmamalı.
    let ours: Vec<String> = entries(&mount)
        .into_iter()
        .filter(|name| name.starts_with('.') || name.ends_with(".safety"))
        .collect();
    assert!(ours.is_empty(), "geçici artık kaldı: {ours:?}");
    eprintln!("dizin: {:?}", entries(&mount));

    // Yer açılınca kasa açılır ve **güncelleme sonrası veri yerinde** olur;
    // ardından aynı çağrı başarılı ve veri yedeğe döner.
    fs::remove_file(mount.join("doldur.bin")).expect("doldurucu silinir");
    let db = Database::open(&path, &key()).expect("yer açılınca kasa açılır");
    assert_eq!(
        db.list_cases(true).expect("dosyalar").len(),
        301,
        "kasa bozulmadı: 300 eski + 1 yeni kayıt"
    );
    drop(db);
    let outcome = rollback_vault(&path, &backup, &key()).expect("yer açıldı, geri dönüş");
    assert_eq!(
        outcome.restored_version,
        dim_storage::CURRENT_SCHEMA_VERSION
    );
    assert_eq!(
        titles(&Database::open(&path, &key()).expect("geri konan kasa")).len(),
        300,
        "yalnız yedekteki 300 kayıt kalır"
    );
    assert!(outcome.safety_copy.is_file(), "güvenlik kopyası yazılır");
    eprintln!(
        "yer açıldıktan sonra: {} güvenlik kopyası, {} bayt",
        outcome.safety_copy.display(),
        fs::metadata(&outcome.safety_copy).expect("meta").len()
    );
}
