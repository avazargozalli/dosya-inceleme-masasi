//! Kasa yedeğinin manifesti: üretim ve doğrulama **tek yerde** (F15-T06).
//!
//! Yedek klasörü açık metin içermez (DB ve blob'lar kasa anahtarıyla şifreli)
//! ama **yarım** olabilir: disk dolması, kopan harici disk, yarıda kesilen
//! kopya, bozulmuş aktarım. Yarım bir yedek "yedek var" sanılır ve geri
//! yükleme sırasında fark edilir; o anda da yarım bir kasa oluşur.
//!
//! Bu yüzden geri yükleme **önce manifesti doğrular** (boyut + sha256). Tek
//! bir dosya uyuşmazsa hiçbir şey kopyalanmaz: kullanıcı "geri yükleme
//! başarısız" görür, yarım kasa görmez.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::atomic;
use crate::error::StorageError;

/// Yedek biçim imzası; tanınmayan klasör yedek sayılmaz.
pub const BACKUP_FORMAT: &str = "dim-backup";
/// Bu sürümün yazabildiği en yüksek yedek biçimi.
pub const BACKUP_VERSION: u32 = 1;
pub const MANIFEST_FILE: &str = "manifest.json";
pub const VAULT_FILE: &str = "vault.dimvault";
pub const DB_FILE: &str = "vault.db";
pub const BLOBS_DIR: &str = "blobs";
/// Kasa künye dosyası bilinçli olarak `vault.dimvault` adını taşır.
pub const REQUIRED_FILES: [&str; 3] = [VAULT_FILE, DB_FILE, MANIFEST_FILE];

/// Tek bir blob (belge/görüntü) girdisi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
}

/// `manifest.json` içeriği. Alan adları **kararlı**: eski yedekler okunmaya
/// devam eder (`vaultBytes`/`dbBytes` sonradan eklendi, `Option`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub format: String,
    pub version: u32,
    pub created_at: String,
    pub vault_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vault_bytes: Option<u64>,
    pub db_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db_bytes: Option<u64>,
    pub blobs: Vec<BackupEntry>,
}

/// Yazılan manifest ve doğrulama için sha256'sı.
#[derive(Debug, Clone)]
pub struct WrittenManifest {
    pub manifest: BackupManifest,
    /// `manifest.json` baytlarının sha256'sı (arayüzde künye olarak gösterilir).
    pub manifest_sha256: String,
    pub manifest_bytes: Vec<u8>,
}

/// Doğrulanmış yedek: hangi dosyalar sayıldı, kaç bayt okundu.
#[derive(Debug, Clone)]
pub struct BackupCheck {
    pub manifest: BackupManifest,
    pub verified_files: usize,
    pub verified_bytes: u64,
    /// `blobs/` içinde manifestte **olmayan** dosyalar (gizli dosyalar hariç).
    /// Geri yükleme bunları kopyalamaz; kullanıcıya sayı olarak bildirilir.
    pub unlisted: Vec<String>,
}

/// Geri yükleme sonucu.
#[derive(Debug, Clone)]
pub struct RestoreOutcome {
    pub verified_files: usize,
    pub verified_bytes: u64,
    /// Doğrulama (sha256) süresi; kopyalama süresi dâhil **değil**.
    pub verify_ms: u64,
    pub blob_count: usize,
    pub unlisted: Vec<String>,
}

/// Yedeği **yeni** bir kasa dizinine geri yükler.
///
/// Sıra (F15-T06): doğrula → hazırlık dizinine kopyala → tek `rename`.
/// - Bozuk/yarım yedek hiç kopyalanmaz (`E_BACKUP_INVALID`),
/// - disk dolarsa geriye kasa gibi görünen yarım klasör kalmaz
///   (`E_DISK_FULL`) ve işlem tekrar denenebilir,
/// - hedef doluysa hiçbir şey yazılmaz (`E_RESTORE_DEST_NOT_EMPTY`).
pub fn restore_backup_dir(src: &Path, dest: &Path) -> Result<RestoreOutcome, StorageError> {
    let started = std::time::Instant::now();
    let check = verify_backup_dir(src)?;
    let verify_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    if dest.join(VAULT_FILE).exists() {
        return Err(StorageError::RestoreDestNotEmpty);
    }
    if dest.exists() {
        let mut entries = fs::read_dir(dest).map_err(atomic::map_io)?;
        if entries.next().is_some() {
            return Err(StorageError::RestoreDestNotEmpty);
        }
    }

    let staging = atomic::temp_sibling(dest, "dimrestore");
    atomic::remove_tree(&staging);
    let copied = (|| -> Result<(), StorageError> {
        fs::create_dir_all(staging.join(BLOBS_DIR)).map_err(atomic::map_io)?;
        for name in [VAULT_FILE, DB_FILE, MANIFEST_FILE] {
            atomic::copy_atomic(&src.join(name), &staging.join(name))?;
        }
        // Yalnız manifestte **sayılan** blob'lar kopyalanır; fazlalıklar
        // (ör. yarım kalmış bir aktarım) kasaya taşınmaz.
        for blob in &check.manifest.blobs {
            atomic::copy_atomic(
                &src.join(BLOBS_DIR).join(&blob.name),
                &staging.join(BLOBS_DIR).join(&blob.name),
            )?;
        }
        Ok(())
    })();
    if let Err(err) = copied {
        atomic::remove_tree(&staging);
        return Err(err);
    }

    if dest.exists() {
        // Yukarıda boş olduğu doğrulandı; `rename` var olan dizinin üzerine
        // yazmadığı için boş kabuk kaldırılır.
        fs::remove_dir(dest).map_err(atomic::map_io)?;
    }
    if let Err(err) = atomic::commit_dir(&staging, dest) {
        atomic::remove_tree(&staging);
        return Err(err);
    }

    Ok(RestoreOutcome {
        verified_files: check.verified_files,
        verified_bytes: check.verified_bytes,
        verify_ms,
        blob_count: check.manifest.blobs.len(),
        unlisted: check.unlisted,
    })
}

/// Dosyanın sha256'sı (akış hâlinde; büyük blob belleğe alınmaz).
pub fn sha256_file(path: &Path) -> Result<String, StorageError> {
    let mut file = File::open(path).map_err(atomic::map_io)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(atomic::map_io)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Yedek klasörünün manifestini **hesaplayıp** atomik yazar. Klasörde
/// `vault.dimvault` ve `vault.db` bulunmalıdır; yoksa yedek üretilmez.
pub fn write_backup_manifest(dir: &Path) -> Result<WrittenManifest, StorageError> {
    let vault_path = dir.join(VAULT_FILE);
    let db_path = dir.join(DB_FILE);
    for required in [&vault_path, &db_path] {
        if !required.is_file() {
            return Err(StorageError::BackupInvalid {
                reason: format!(
                    "yedek dosyası eksik: {}",
                    required
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                ),
            });
        }
    }

    let mut blobs: Vec<BackupEntry> = Vec::new();
    let blobs_dir = dir.join(BLOBS_DIR);
    if blobs_dir.is_dir() {
        let mut names: Vec<String> = fs::read_dir(&blobs_dir)
            .map_err(atomic::map_io)?
            .map(|entry| {
                entry
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .map_err(atomic::map_io)
            })
            .collect::<Result<_, _>>()?;
        names.sort();
        for name in names {
            let path = blobs_dir.join(&name);
            let meta = fs::metadata(&path).map_err(atomic::map_io)?;
            if !meta.is_file() {
                continue;
            }
            blobs.push(BackupEntry {
                name,
                bytes: meta.len(),
                sha256: sha256_file(&path)?,
            });
        }
    }

    let vault_meta = fs::metadata(&vault_path).map_err(atomic::map_io)?;
    let db_meta = fs::metadata(&db_path).map_err(atomic::map_io)?;
    let manifest = BackupManifest {
        format: BACKUP_FORMAT.to_string(),
        version: BACKUP_VERSION,
        created_at: crate::now_iso(),
        vault_sha256: sha256_file(&vault_path)?,
        vault_bytes: Some(vault_meta.len()),
        db_sha256: sha256_file(&db_path)?,
        db_bytes: Some(db_meta.len()),
        blobs,
    };
    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|_| StorageError::BackupInvalid {
            reason: "manifest üretilemedi".to_string(),
        })?;
    let manifest_sha256 = hex::encode(Sha256::digest(&manifest_bytes));
    atomic::write_atomic(&dir.join(MANIFEST_FILE), &manifest_bytes)?;
    Ok(WrittenManifest {
        manifest,
        manifest_sha256,
        manifest_bytes,
    })
}

/// Yedeği **kopyalamadan önce** doğrular: biçim, sürüm, boyut ve sha256.
///
/// Tek bir uyuşmazlık `E_BACKUP_INVALID` verir; çağıran hiçbir şey
/// kopyalamamalıdır. Manifestte olmayan fazla bloblar **hata değil**, `unlisted`
/// olarak raporlanır (geri yükleme onları kopyalamaz).
pub fn verify_backup_dir(dir: &Path) -> Result<BackupCheck, StorageError> {
    let manifest_path = dir.join(MANIFEST_FILE);
    let raw = fs::read(&manifest_path).map_err(|err| {
        if err.kind() == io::ErrorKind::NotFound {
            return StorageError::BackupInvalid {
                reason: format!("yedek manifesti yok ({MANIFEST_FILE})"),
            };
        }
        atomic::map_io(err)
    })?;
    let manifest: BackupManifest =
        serde_json::from_slice(&raw).map_err(|_| StorageError::BackupInvalid {
            reason: "yedek manifesti okunamadı".to_string(),
        })?;
    if manifest.format != BACKUP_FORMAT {
        return Err(StorageError::BackupInvalid {
            reason: format!("tanınmayan yedek biçimi: {}", manifest.format),
        });
    }
    if manifest.version > BACKUP_VERSION {
        return Err(StorageError::BackupInvalid {
            reason: format!(
                "yedek biçimi bu sürümden yeni ({} > {BACKUP_VERSION})",
                manifest.version
            ),
        });
    }

    let mut verified_files = 0usize;
    let mut verified_bytes = 0u64;

    for (name, expected_sha, expected_bytes) in [
        (VAULT_FILE, &manifest.vault_sha256, manifest.vault_bytes),
        (DB_FILE, &manifest.db_sha256, manifest.db_bytes),
    ] {
        let path = dir.join(name);
        let meta = fs::metadata(&path).map_err(|err| {
            if err.kind() == io::ErrorKind::NotFound {
                return StorageError::BackupInvalid {
                    reason: format!("yedek dosyası eksik: {name}"),
                };
            }
            atomic::map_io(err)
        })?;
        if let Some(bytes) = expected_bytes {
            if meta.len() != bytes {
                return Err(StorageError::BackupInvalid {
                    reason: format!(
                        "{name} boyutu tutmuyor: beklenen {bytes}, bulunan {}",
                        meta.len()
                    ),
                });
            }
        }
        let actual = sha256_file(&path)?;
        if &actual != expected_sha {
            return Err(StorageError::BackupInvalid {
                reason: format!("{name} sha256 tutmuyor (dosya değişmiş veya yarım)"),
            });
        }
        verified_files += 1;
        verified_bytes += meta.len();
    }

    let blobs_dir = dir.join(BLOBS_DIR);
    let mut on_disk: Vec<String> = Vec::new();
    if blobs_dir.is_dir() {
        for entry in fs::read_dir(&blobs_dir).map_err(atomic::map_io)? {
            let entry = entry.map_err(atomic::map_io)?;
            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                on_disk.push(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    on_disk.sort();

    for blob in &manifest.blobs {
        if !is_plain_file_name(&blob.name) {
            return Err(StorageError::BackupInvalid {
                reason: format!("blob adı güvenli değil: {}", blob.name),
            });
        }
        let path = blobs_dir.join(&blob.name);
        let meta = fs::metadata(&path).map_err(|err| {
            if err.kind() == io::ErrorKind::NotFound {
                return StorageError::BackupInvalid {
                    reason: format!("yedekte blob eksik: {}", blob.name),
                };
            }
            atomic::map_io(err)
        })?;
        if meta.len() != blob.bytes {
            return Err(StorageError::BackupInvalid {
                reason: format!(
                    "blob {} boyutu tutmuyor: beklenen {}, bulunan {}",
                    blob.name,
                    blob.bytes,
                    meta.len()
                ),
            });
        }
        let actual = sha256_file(&path)?;
        if actual != blob.sha256 {
            return Err(StorageError::BackupInvalid {
                reason: format!("blob {} sha256 tutmuyor", blob.name),
            });
        }
        verified_files += 1;
        verified_bytes += meta.len();
    }

    let listed: Vec<&str> = manifest.blobs.iter().map(|b| b.name.as_str()).collect();
    let mut unlisted: Vec<String> = on_disk
        .into_iter()
        .filter(|name| !name.starts_with('.') && !listed.contains(&name.as_str()))
        .collect();
    unlisted.sort();

    Ok(BackupCheck {
        manifest,
        verified_files,
        verified_bytes,
        unlisted,
    })
}

/// Blob adı tek bir dosya adı olmalı: dizin ayırıcı, `..`, denetim karakteri
/// ve boş ad reddedilir (yedek klasörü dışına yazma denemesi).
fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && !name.contains('/')
        && !name.contains('\\')
        && name != ".."
        && !name.chars().any(|c| c.is_control())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("dizin");
        fs::write(dir.path().join(VAULT_FILE), b"kasa-kunye").expect("yazıldı");
        fs::write(dir.path().join(DB_FILE), b"sifreli-veritabani").expect("yazıldı");
        fs::create_dir(dir.path().join(BLOBS_DIR)).expect("dizin");
        fs::write(dir.path().join(BLOBS_DIR).join("a.blob"), b"blob-a").expect("yazıldı");
        fs::write(dir.path().join(BLOBS_DIR).join("b.blob"), b"blob-bb").expect("yazıldı");
        dir
    }

    #[test]
    fn written_manifest_verifies_and_counts_files() {
        let dir = fixture();
        let written = write_backup_manifest(dir.path()).expect("manifest");
        assert_eq!(written.manifest.version, BACKUP_VERSION);
        assert_eq!(written.manifest.blobs.len(), 2);
        assert_eq!(written.manifest.vault_bytes, Some(10));
        assert_eq!(written.manifest_sha256.len(), 64);

        let check = verify_backup_dir(dir.path()).expect("doğrulandı");
        assert_eq!(check.verified_files, 4, "2 ana dosya + 2 blob");
        assert_eq!(check.verified_bytes, 10 + 18 + 6 + 7);
        assert!(check.unlisted.is_empty());
        // Manifest gerçekten diskte ve aynı baytlar.
        let on_disk = fs::read(dir.path().join(MANIFEST_FILE)).expect("okundu");
        assert_eq!(on_disk, written.manifest_bytes);
    }

    /// Kesilmiş (yarım kalmış) bir yedek **kabul edilmez**: boyut ve hash
    /// denetimi ayrı ayrı çalışır.
    #[test]
    fn truncated_blob_is_refused() {
        let dir = fixture();
        write_backup_manifest(dir.path()).expect("manifest");
        fs::write(dir.path().join(BLOBS_DIR).join("a.blob"), b"blob").expect("kesildi");
        let err = verify_backup_dir(dir.path()).expect_err("yarım yedek");
        assert_eq!(err.code(), "E_BACKUP_INVALID");
        assert!(err.to_string().contains("boyutu tutmuyor"), "{err}");
    }

    #[test]
    fn same_size_but_changed_blob_is_refused_by_hash() {
        let dir = fixture();
        write_backup_manifest(dir.path()).expect("manifest");
        fs::write(dir.path().join(BLOBS_DIR).join("a.blob"), b"blob-X").expect("değişti");
        let err = verify_backup_dir(dir.path()).expect_err("hash tutmuyor");
        assert!(err.to_string().contains("sha256 tutmuyor"), "{err}");
    }

    #[test]
    fn missing_manifest_and_missing_db_are_refused() {
        let dir = fixture();
        let err = verify_backup_dir(dir.path()).expect_err("manifest yok");
        assert_eq!(err.code(), "E_BACKUP_INVALID");
        assert!(err.to_string().contains("manifesti yok"), "{err}");

        write_backup_manifest(dir.path()).expect("manifest");
        fs::remove_file(dir.path().join(DB_FILE)).expect("silindi");
        let err = verify_backup_dir(dir.path()).expect_err("db yok");
        assert!(err.to_string().contains("eksik"), "{err}");
    }

    #[test]
    fn unknown_format_and_newer_version_are_refused() {
        let dir = fixture();
        let written = write_backup_manifest(dir.path()).expect("manifest");
        let mut value: serde_json::Value =
            serde_json::from_slice(&written.manifest_bytes).expect("json");
        value["format"] = serde_json::json!("baska-yedek");
        fs::write(
            dir.path().join(MANIFEST_FILE),
            serde_json::to_vec(&value).expect("json"),
        )
        .expect("yazıldı");
        let err = verify_backup_dir(dir.path()).expect_err("biçim");
        assert!(err.to_string().contains("tanınmayan yedek biçimi"), "{err}");

        value["format"] = serde_json::json!(BACKUP_FORMAT);
        value["version"] = serde_json::json!(BACKUP_VERSION + 1);
        fs::write(
            dir.path().join(MANIFEST_FILE),
            serde_json::to_vec(&value).expect("json"),
        )
        .expect("yazıldı");
        let err = verify_backup_dir(dir.path()).expect_err("sürüm");
        assert!(err.to_string().contains("bu sürümden yeni"), "{err}");
    }

    #[test]
    fn unlisted_blobs_are_reported_but_not_fatal() {
        let dir = fixture();
        write_backup_manifest(dir.path()).expect("manifest");
        fs::write(dir.path().join(BLOBS_DIR).join("fazla.blob"), b"x").expect("yazıldı");
        fs::write(dir.path().join(BLOBS_DIR).join(".DS_Store"), b"x").expect("yazıldı");
        let check = verify_backup_dir(dir.path()).expect("doğrulandı");
        assert_eq!(
            check.unlisted,
            vec!["fazla.blob".to_string()],
            "gizli sayılmaz"
        );
    }

    #[test]
    fn unsafe_blob_name_is_refused() {
        let dir = fixture();
        let written = write_backup_manifest(dir.path()).expect("manifest");
        let mut value: serde_json::Value =
            serde_json::from_slice(&written.manifest_bytes).expect("json");
        value["blobs"][0]["name"] = serde_json::json!("../kacis.blob");
        fs::write(
            dir.path().join(MANIFEST_FILE),
            serde_json::to_vec(&value).expect("json"),
        )
        .expect("yazıldı");
        let err = verify_backup_dir(dir.path()).expect_err("güvensiz ad");
        assert!(err.to_string().contains("güvenli değil"), "{err}");
    }

    #[test]
    fn legacy_manifest_without_byte_counts_still_verifies() {
        let dir = fixture();
        let written = write_backup_manifest(dir.path()).expect("manifest");
        let mut value: serde_json::Value =
            serde_json::from_slice(&written.manifest_bytes).expect("json");
        let object = value.as_object_mut().expect("nesne");
        object.remove("vaultBytes");
        object.remove("dbBytes");
        fs::write(
            dir.path().join(MANIFEST_FILE),
            serde_json::to_vec(&value).expect("json"),
        )
        .expect("yazıldı");
        let check = verify_backup_dir(dir.path()).expect("eski biçim kabul");
        assert_eq!(check.verified_files, 4);
    }

    /// Bozuk (hash tutmayan) yedek **hiç kopyalanmaz**: hedef klasör oluşmaz.
    #[test]
    fn restore_refuses_tampered_backup_and_creates_nothing() {
        let backup = fixture();
        write_backup_manifest(backup.path()).expect("manifest");
        // Aynı boyut, farklı içerik → yalnız sha256 yakalar.
        fs::write(backup.path().join(BLOBS_DIR).join("a.blob"), b"blob-X").expect("kurcalandı");
        let holder = tempfile::tempdir().expect("dizin");
        let dest = holder.path().join("yeni-kasa");

        let err = restore_backup_dir(backup.path(), &dest).expect_err("bozuk yedek reddedilir");
        assert_eq!(err.code(), "E_BACKUP_INVALID");
        assert!(!dest.exists(), "yarım hedef klasör oluşmamalı");
    }

    /// Yalnız manifestte sayılan blob'lar kopyalanır; doğrulama sayıları döner.
    #[test]
    fn restore_copies_only_listed_blobs_and_reports_verification() {
        let backup = fixture();
        let written = write_backup_manifest(backup.path()).expect("manifest");
        fs::write(backup.path().join(BLOBS_DIR).join("fazla.blob"), b"x").expect("fazla");
        let holder = tempfile::tempdir().expect("dizin");
        let dest = holder.path().join("geri-yuklenen");

        let outcome = restore_backup_dir(backup.path(), &dest).expect("geri yüklendi");
        assert_eq!(outcome.blob_count, 2);
        assert_eq!(outcome.verified_files, 4, "2 ana dosya + 2 blob");
        assert_eq!(
            outcome.verified_bytes,
            written.manifest.vault_bytes.unwrap_or(0)
                + written.manifest.db_bytes.unwrap_or(0)
                + 6
                + 7
        );
        assert_eq!(outcome.unlisted, vec!["fazla.blob".to_string()]);
        assert!(dest.join(VAULT_FILE).is_file() && dest.join(DB_FILE).is_file());
        assert!(
            dest.join(BLOBS_DIR).join("a.blob").is_file(),
            "listelenen blob kopyalanır"
        );
        assert!(
            !dest.join(BLOBS_DIR).join("fazla.blob").exists(),
            "sayılmayan blob kasaya taşınmaz"
        );
        assert!(
            dest.join(MANIFEST_FILE).is_file(),
            "manifest yeni kasaya taşınır"
        );
        // Kopyalanan dosyalar bayt bayt aynı.
        assert_eq!(
            fs::read(dest.join(DB_FILE)).expect("okundu"),
            fs::read(backup.path().join(DB_FILE)).expect("okundu")
        );
    }

    /// Boş olmayan hedefe geri yükleme yazmaz (mevcut kasa/dosyalar ezilmez).
    #[test]
    fn restore_refuses_non_empty_destination_and_keeps_files() {
        let backup = fixture();
        write_backup_manifest(backup.path()).expect("manifest");
        let holder = tempfile::tempdir().expect("dizin");
        let dest = holder.path().join("dolu");
        fs::create_dir_all(&dest).expect("dizin");
        fs::write(dest.join("notlar.txt"), b"x").expect("dosya");

        let err = restore_backup_dir(backup.path(), &dest).expect_err("dolu hedef");
        assert_eq!(err.code(), "E_RESTORE_DEST_NOT_EMPTY");
        assert!(dest.join("notlar.txt").is_file(), "mevcut dosya korunur");
        assert!(!dest.join(VAULT_FILE).exists());
    }

    /// Hedef **boş** bir klasörse geri yükleme yine de çalışır (kullanıcı
    /// boş bir klasör seçmiş olabilir) ve klasör tek `rename` ile dolar.
    #[test]
    fn restore_fills_an_existing_empty_destination() {
        let backup = fixture();
        write_backup_manifest(backup.path()).expect("manifest");
        let holder = tempfile::tempdir().expect("dizin");
        let dest = holder.path().join("bos");
        fs::create_dir_all(&dest).expect("dizin");

        let outcome = restore_backup_dir(backup.path(), &dest).expect("boş hedefe yazılır");
        assert_eq!(outcome.blob_count, 2);
        assert!(dest.join(VAULT_FILE).is_file());
        assert!(dest.join(BLOBS_DIR).join("b.blob").is_file());
    }
}
