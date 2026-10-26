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
}
