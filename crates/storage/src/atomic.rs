//! Atomik dosya yerleştirme ve disk dolması eşlemesi (F15-T06).
//!
//! Kural: **yarım kalan yazım hedefte görünmez.** Her yazma aynı dizinde
//! geçici bir ada yapılır, `sync_all` ile diske indirilir, sonra `rename` ile
//! yerleştirilir. Hata olursa geçici dosya silinir; hedef ya eskisi gibi kalır
//! ya da hiç oluşmaz. Bu, disk dolduğunda (ENOSPC) yarım kasa/yedek/arşiv
//! bırakılmaması için gerekli: yarım bir yedek "yedek var" sanılır, yarım bir
//! kasa ise açılmaz.
//!
//! Dosya sistemi hatası tek yerde çevrilir: disk dolması `E_DISK_FULL`,
//! diğerleri `E_IO`. Ham `io::Error` metni kullanıcıya gösterilmez.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::StorageError;

/// Aynı süreçte çakışmayan geçici ad üretmek için sayaç.
static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Disk dolması hatası mı? (Unix `ENOSPC` = 28, Windows `ERROR_DISK_FULL`).
///
/// `ErrorKind::StorageFull` iki platformda da eşlenir; ham errno kontrolü
/// eski/özel durumlar için yedek kapıdır.
#[must_use]
pub fn is_no_space(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::StorageFull || err.raw_os_error() == Some(28)
}

/// G/Ç hatasını kasa hatasına çevirir.
#[must_use]
pub fn map_io(err: io::Error) -> StorageError {
    if is_no_space(&err) {
        return StorageError::DiskFull {
            reason: err.to_string(),
        };
    }
    StorageError::Io(err)
}

/// Hedefin **yanında** benzersiz geçici yol üretir (aynı dosya sistemi şartı:
/// `rename` bölümler arası taşıma yapmaz).
#[must_use]
pub fn temp_sibling(path: &Path, tag: &str) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "dim".to_string());
    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(".{name}.{tag}-{}-{seq}", std::process::id()))
}

/// Dosyayı atomik yazar: geçici ada yaz → `sync_all` → `rename`.
///
/// Hata hâlinde geçici dosya silinir, `path` **değişmez**.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    write_atomic_with(path, |out| out.write_all(bytes))
}

/// `write_atomic`in akış hâli: yazma işini çağıran verir (kopya, zip, dışa
/// aktarma). Kapanış `ENOSPC` döndürürse geçici dosya silinir.
pub fn write_atomic_with<F>(path: &Path, write: F) -> Result<(), StorageError>
where
    F: FnOnce(&mut File) -> io::Result<()>,
{
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(map_io)?;
    }
    let temp = temp_sibling(path, "dimtmp");
    let written = (|| -> io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        write(&mut file)?;
        file.sync_all()?;
        Ok(())
    })();

    match written {
        Ok(()) => match fs::rename(&temp, path) {
            Ok(()) => Ok(()),
            Err(err) => {
                remove_file_best_effort(&temp);
                Err(map_io(err))
            }
        },
        Err(err) => {
            remove_file_best_effort(&temp);
            Err(map_io(err))
        }
    }
}

/// Kaynağı hedefe atomik kopyalar; yazılan bayt sayısını döner.
pub fn copy_atomic(src: &Path, dst: &Path) -> Result<u64, StorageError> {
    let mut total: u64 = 0;
    {
        let counter = &mut total;
        write_atomic_with(dst, |out| {
            let mut input = File::open(src)?;
            *counter = io::copy(&mut input, out)?;
            Ok(())
        })?;
    }
    Ok(total)
}

/// Hazırlık dizinini hedef ada taşır. Hedef **varsa** hata döner; üzerine
/// yazmaz (geri yükleme yanlışlıkla mevcut kasayı ezmesin).
pub fn commit_dir(staging: &Path, final_path: &Path) -> Result<(), StorageError> {
    if final_path.exists() {
        return Err(StorageError::Io(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "hedef zaten var",
        )));
    }
    fs::rename(staging, final_path).map_err(map_io)
}

/// Dizin ağacını sessizce siler: yalnız temizlik içindir, hata yutulur.
///
/// Yarıda kalan hazırlık dizinleri kullanıcıya "yarım yedek" olarak
/// görünmesin diye çağrılır; silinemezse asıl hata yine döner.
pub fn remove_tree(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

/// Kaynağı hedefe **sabit bağlantıyla** bağlar; desteklenmiyorsa tam kopya alır.
///
/// `true` = bağlantı kuruldu (ek yer kaplamaz, aynı dosya). `false` = kopya
/// yazıldı (hedef ayrı bir dosya; çağıran doğrulamalıdır).
///
/// Geri dönüşte güvenlik kopyası için kullanılır: eski kod dosyayı `rename` ile
/// taşıyordu ve süreç o arada ölürse kasa dosyası ortadan kalkıyordu. Bağlantı
/// kurulduğunda hedefin yerine yazmak (`rename`) eski içeriği bozmaz, çünkü
/// eski içerik artık ikinci adla da erişilebilir.
pub fn hard_link_or_copy(src: &Path, dst: &Path) -> Result<bool, StorageError> {
    match fs::hard_link(src, dst) {
        Ok(()) => Ok(true),
        Err(err) if link_unsupported(&err) => {
            copy_atomic(src, dst)?;
            Ok(false)
        }
        Err(err) => Err(map_io(err)),
    }
}

/// Sabit bağlantı kurulamayan durumlar (farklı birim, izin, bağlantı sınırı,
/// dosya sistemi desteklemiyor). Bunlarda tam kopya doğru yoldur.
fn link_unsupported(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::Unsupported | io::ErrorKind::PermissionDenied
    ) || matches!(
        err.raw_os_error(),
        // EPERM, EXDEV, EMLINK, ENOTSUP (macOS), EOPNOTSUPP (Linux)
        Some(1 | 18 | 31 | 45 | 95)
    )
}

/// `.{ad}.{tag}-<pid>-<sıra>` geçici dosyalarını siler (yarıda kalan yazımların
/// artıkları). `older_than` sıfırsa hepsi silinir.
///
/// Kural: bu adlar **yalnız** aynı dizinde çalışan tek yazıcıya aittir; kasa
/// kilidi altında canlı bir geçici dosya bulunamaz. Yaş eşiği, kilitsiz
/// kullanımda (ör. iki ayrı kasa klasörü paylaşılıyorsa) yanlışlıkla canlı bir
/// dosyayı silmemek için vardır.
pub fn sweep_tagged_temps(path: &Path, tag: &str, older_than: std::time::Duration) -> usize {
    let dir = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "dim".to_string());
    let prefix = format!(".{name}.{tag}-");
    let now = std::time::SystemTime::now();
    let mut removed = 0;
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().to_string();
        if !file_name.starts_with(&prefix) {
            continue;
        }
        let fresh = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .map(|modified| now.duration_since(modified).unwrap_or_default() < older_than)
            .unwrap_or(false);
        if fresh {
            continue;
        }
        if fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Tatbikat kancası: hata ayıklama derlemesinde `DIM_DRILL_PAUSE` bu noktaya
/// eşitse süreci bekletir (`DIM_DRILL_PAUSE_MS`, varsayılan 60 sn).
///
/// Yalnız süreç öldürme tatbikatında kullanılır (`tests/rollback_drill.rs`):
/// çökme penceresini deterministik hâle getirir. **Ürün derlemesinde hiçbir
/// şey yapmaz** (`cfg(not(debug_assertions))`), yani ürün yolunda davranış
/// değiştirmez.
#[cfg(debug_assertions)]
pub fn drill_pause(point: &str) {
    let Ok(wanted) = std::env::var("DIM_DRILL_PAUSE") else {
        return;
    };
    if wanted != point {
        return;
    }
    let millis = std::env::var("DIM_DRILL_PAUSE_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(60_000);
    std::thread::sleep(std::time::Duration::from_millis(millis));
}

/// Bkz. `drill_pause`; ürün derlemesinde boş.
#[cfg(not(debug_assertions))]
pub fn drill_pause(point: &str) {
    let _ = point;
}

/// Dosyayı ve varsa yan dosyalarını (`-journal`, `-wal`, `-shm`) siler.
pub fn remove_file_best_effort(path: &Path) {
    let _ = fs::remove_file(path);
    let base = path.to_string_lossy().to_string();
    for suffix in ["-journal", "-wal", "-shm"] {
        let _ = fs::remove_file(format!("{base}{suffix}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_atomic_leaves_no_temp_file() {
        let dir = tempfile::tempdir().expect("dizin");
        let target = dir.path().join("x.bin");
        write_atomic(&target, b"abc").expect("yazıldı");
        assert_eq!(fs::read(&target).expect("okundu"), b"abc");
        let names: Vec<String> = fs::read_dir(dir.path())
            .expect("liste")
            .map(|e| e.expect("girdi").file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["x.bin".to_string()], "geçici dosya kalmamalı");
    }

    /// Disk dolduğunda hedef dosya **oluşmamalı** ve geçici dosya silinmeli.
    #[test]
    fn enospc_is_mapped_and_temp_is_removed() {
        let dir = tempfile::tempdir().expect("dizin");
        let target = dir.path().join("y.bin");
        let err = write_atomic_with(&target, |out| {
            out.write_all(b"ilk parca")?;
            Err(io::Error::from_raw_os_error(28))
        })
        .expect_err("disk dolu");
        assert_eq!(err.code(), "E_DISK_FULL", "ayrı kod: {err}");
        assert!(!target.exists(), "yarım hedef kalmamalı");
        assert_eq!(
            fs::read_dir(dir.path()).expect("liste").count(),
            0,
            "geçici dosya kalmamalı"
        );
    }

    #[test]
    fn io_error_stays_io_error_and_keeps_old_content() {
        let dir = tempfile::tempdir().expect("dizin");
        let target = dir.path().join("z.bin");
        write_atomic(&target, b"eski").expect("ilk yazım");
        let err = write_atomic_with(&target, |out| {
            out.write_all(b"yeni")?;
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "koptu"))
        })
        .expect_err("hata");
        assert_eq!(err.code(), "E_IO");
        assert_eq!(
            fs::read(&target).expect("okundu"),
            b"eski",
            "eski içerik korunur"
        );
    }

    #[test]
    fn copy_atomic_reports_bytes_and_commit_dir_refuses_existing() {
        let dir = tempfile::tempdir().expect("dizin");
        let src = dir.path().join("kaynak.bin");
        fs::write(&src, vec![7u8; 4096]).expect("yazıldı");
        let dst = dir.path().join("hedef.bin");
        assert_eq!(copy_atomic(&src, &dst).expect("kopya"), 4096);
        assert_eq!(fs::metadata(&dst).expect("meta").len(), 4096);

        let staging = dir.path().join("hazırlık");
        fs::create_dir(&staging).expect("dizin");
        let existing = dir.path().join("var");
        fs::create_dir(&existing).expect("dizin");
        let err = commit_dir(&staging, &existing).expect_err("var olan hedefe yazmaz");
        assert_eq!(err.code(), "E_IO");
        assert!(staging.exists(), "hazırlık dizini yerinde kalır");
    }

    #[test]
    fn temp_sibling_is_unique_and_hidden() {
        let target = Path::new("/tmp/kasa/vault.db");
        let a = temp_sibling(target, "dimtmp");
        let b = temp_sibling(target, "dimtmp");
        assert_ne!(a, b);
        assert_eq!(a.parent(), target.parent());
        assert!(a
            .file_name()
            .expect("ad")
            .to_string_lossy()
            .starts_with(".vault.db.dimtmp-"));
    }

    /// Güvenlik kopyası **taşınmaz, bağlanır**: hedefin yerine yazmak eski
    /// içeriği bozmaz. Eski `rename` sırası kasa dosyasını bir an yok ediyordu.
    #[test]
    fn hard_link_keeps_old_bytes_after_target_is_replaced() {
        let dir = tempfile::tempdir().expect("dizin");
        let src = dir.path().join("kasa.db");
        fs::write(&src, b"eski icerik").expect("yazıldı");
        let link = dir.path().join("kasa.db.rollback-1.safety");
        assert!(
            hard_link_or_copy(&src, &link).expect("bağ"),
            "aynı birimde sabit bağlantı kurulur"
        );
        assert_eq!(fs::read(&link).expect("okundu"), b"eski icerik");

        let replacement = dir.path().join("yeni.tmp");
        fs::write(&replacement, b"yeni icerik").expect("yeni");
        fs::rename(&replacement, &src).expect("yerine");

        assert_eq!(fs::read(&src).expect("okundu"), b"yeni icerik");
        assert_eq!(
            fs::read(&link).expect("okundu"),
            b"eski icerik",
            "güvenlik kopyası eski baytları korur"
        );
    }

    #[test]
    fn sweep_removes_only_tagged_temps() {
        use std::time::{Duration, SystemTime};
        let dir = tempfile::tempdir().expect("dizin");
        let vault = dir.path().join("vault.db");
        fs::write(&vault, b"kasa").expect("kasa");

        let stale = dir.path().join(".vault.db.rollback-111-0");
        let fresh = dir.path().join(".vault.db.rollback-222-1");
        let other_tag = dir.path().join(".vault.db.dimtmp-333-0");
        let safety = dir.path().join("vault.db.rollback-1.safety");
        for file in [&stale, &fresh, &other_tag, &safety] {
            fs::write(file, b"x").expect("yaz");
        }
        let old = SystemTime::now() - Duration::from_secs(7200);
        fs::OpenOptions::new()
            .write(true)
            .open(&stale)
            .expect("aç")
            .set_times(fs::FileTimes::new().set_modified(old))
            .expect("mtime");

        let removed = sweep_tagged_temps(&vault, "rollback", Duration::from_secs(3600));
        assert_eq!(removed, 1, "yalnız yaşlı etiketli artık silinir");
        assert!(!stale.exists());
        assert!(fresh.exists(), "taze geçici dosya korunur");
        assert!(other_tag.exists(), "başka etiket korunur");
        assert!(safety.exists(), "güvenlik kopyası silinmez");

        assert_eq!(
            sweep_tagged_temps(&vault, "rollback", Duration::ZERO),
            1,
            "sıfır eşik: kalan etiketli artık"
        );
        assert!(!fresh.exists());
        assert!(other_tag.exists());
    }
}
