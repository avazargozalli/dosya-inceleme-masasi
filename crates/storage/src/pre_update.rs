//! Güncelleme öncesi yedeklerin bulunması ve **uygulama içi** geri dönüş (F16-T03).
//!
//! `rollback_vault` kapalı bir dosyayı geri koyar; bu modül uygulamanın
//! ihtiyaç duyduğu iki ek adımı taşır:
//!
//! 1. `pre_update_backups`: kasanın yanındaki `<kasa>.db.v<N>-pre-update.bak`
//!    dosyalarını **salt okunur** açıp doğrular. Addan okunan sürüm bir
//!    iddiadır; doğrulanmış sürüm dosyanın kendisinden gelir.
//! 2. `rollback_and_resume`: yedeği geri koyar, kasayı yeniden açar ve güncel
//!    şemaya ilerletir. Veri geri döner; **ikili (binary) sürüm düşürülmez**.
//!
//! Geri dönüşün kendisi (güvenlik kopyası, yan dosyalar, son doğrulama)
//! `rollback_vault` içindedir; burada kural kopyalanmaz.

use std::path::{Path, PathBuf};

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::{rollback_vault, Database, DatabaseKey, StorageError, CURRENT_SCHEMA_VERSION};

/// Kasanın yanında bulunan bir güncelleme öncesi yedek.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreUpdateBackup {
    /// Dosyanın tam yolu; geri dönüş bunu kullanır.
    pub path: PathBuf,
    /// Dosya adı. Arayüz seçim anahtarı olarak bunu taşır; yol istemciden gelmez.
    pub file_name: String,
    /// Dosya adından okunan sürüm — **iddia**, doğrulanmış sürüm değil.
    pub claimed_version: u32,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
    /// Dosyanın kendi bildirdiği şema sürümü (açıldı, anahtar ve bütünlük denendi).
    pub verified_version: Option<u32>,
    /// Doğrulama düştüyse `E_…` kodu ve neden; yedek listeden gizlenmez.
    pub problem: Option<String>,
}

impl PreUpdateBackup {
    /// Geri dönüş için kullanılabilir mi? Doğrulanmamış ya da bu uygulamadan
    /// yeni şemalı yedek kullanılamaz (sessizce denenmez).
    #[must_use]
    pub fn usable(&self) -> bool {
        self.problem.is_none()
            && self
                .verified_version
                .is_some_and(|version| version <= CURRENT_SCHEMA_VERSION)
    }

    /// Addan okunan sürüm ile dosyanın bildirdiği sürüm ayrışıyor mu?
    /// Ayrışma bir uyarıdır: dosya adı yeniden adlandırılmış olabilir.
    #[must_use]
    pub fn name_mismatch(&self) -> bool {
        self.verified_version
            .is_some_and(|version| version != self.claimed_version)
    }
}

/// Kasanın yanındaki güncelleme öncesi yedekleri bulur ve doğrular.
///
/// Kapsam: yalnız **bu kasa dosyasının** adını taşıyan, adı birebir
/// `<kasa>.v<rakam>-pre-update.bak` olan normal dosyalar. Sembolik bağlar
/// izlenmez (yedeğin başka yere işaret etmesi geri dönüşte yol değiştirme
/// vektörü olur). Doğrulama dosyayı değiştirmez.
pub fn pre_update_backups(
    vault_db_path: &Path,
    key: &DatabaseKey,
) -> Result<Vec<PreUpdateBackup>, StorageError> {
    let dir = match vault_db_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let name = vault_db_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "vault.db".to_string());
    let prefix = format!("{name}.v");
    const SUFFIX: &str = "-pre-update.bak";

    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        let Some(middle) = file_name
            .strip_prefix(&prefix)
            .and_then(|rest| rest.strip_suffix(SUFFIX))
        else {
            continue;
        };
        if middle.is_empty() || !middle.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Ok(claimed_version) = middle.parse::<u32>() else {
            continue;
        };
        let path = entry.path();
        let meta = entry.metadata()?;
        let (verified_version, problem) = verify_without_residue(&path, key);
        found.push(PreUpdateBackup {
            path,
            file_name,
            claimed_version,
            size_bytes: meta.len(),
            modified_at: meta
                .modified()
                .ok()
                .map(OffsetDateTime::from)
                .and_then(|t| t.format(&Rfc3339).ok()),
            verified_version,
            problem,
        });
    }

    // En yeni (addan okunan) sürüm başta; eşitse dosya adına göre sabit sıra.
    found.sort_by(|a, b| {
        b.claimed_version
            .cmp(&a.claimed_version)
            .then_with(|| a.file_name.cmp(&b.file_name))
    });
    Ok(found)
}

/// Yedek dosyasının yan dosyası (`-wal`/`-shm`).
fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    path.with_file_name(format!("{name}{suffix}"))
}

/// Yedeği salt okunur doğrular, **kendi bıraktığı** yan dosyaları temizler.
///
/// WAL kipinde kapanmış bir dosya salt okunur açıldığında SQLite paylaşımlı
/// bellek indeksi (`-shm`) ve boş bir `-wal` bırakabilir; bunlar yedeğin
/// parçası değildir ve açık bağlantı yokken silinmeleri güvenlidir. Dolu bir
/// `-wal` ise doğrulamanın yedeğe **yazdığı** anlamına gelir: silinmez,
/// sorun olarak bildirilir (sessizce yutulmaz). Önceden var olan yan dosyalara
/// dokunulmaz.
fn verify_without_residue(path: &Path, key: &DatabaseKey) -> (Option<u32>, Option<String>) {
    let wal = sidecar_path(path, "-wal");
    let shm = sidecar_path(path, "-shm");
    let (had_wal, had_shm) = (wal.exists(), shm.exists());

    let result = Database::verify_database_file_readonly(path, key);

    let mut residue = None;
    if !had_shm && shm.exists() {
        let _ = std::fs::remove_file(&shm);
    }
    if !had_wal && wal.exists() {
        match std::fs::metadata(&wal) {
            Ok(meta) if meta.len() == 0 => {
                let _ = std::fs::remove_file(&wal);
            }
            other => {
                residue = Some(format!(
                    "E_DB_VERIFY_RESIDUE: doğrulama yedeğe yazdı (-wal {} bayt)",
                    other.map(|m| m.len()).unwrap_or(0)
                ));
            }
        }
    }

    match result {
        Ok(version) => (Some(version), residue),
        Err(err) => (None, Some(format!("{}: {}", err.code(), err))),
    }
}

/// Uygulama içi geri dönüşün sonucu: geri konan dosya **ve** yeniden açılmış kasa.
#[derive(Debug)]
pub struct InPlaceRollback {
    /// Yedeğin taşıdığı (geri konan) şema sürümü.
    pub restored_version: u32,
    /// Geri dönüş öncesi dosyanın saklandığı güvenlik kopyası.
    pub safety_copy: PathBuf,
    /// Güncel şemaya ilerletilmiş, kullanıma hazır bağlantı.
    pub database: Database,
}

/// Yedeği geri koyar, kasayı yeniden açar ve güncel şemaya ilerletir.
///
/// Çağıran, kasa dosyasını tutan **tüm bağlantıları kapatmış** olmalıdır
/// (Windows'ta açık dosya yerine konmaz). Bu fonksiyon geri dönüşten sonra
/// kasayı açar; açılış migrasyonu yedeğin sürümünden güncel sürüme ilerler ve
/// aynı ada sahip yedek korunur. Yalnız geri dönüşün kendisi düşerse `Err`
/// döner; geri dönüş başarılı olup yeniden açılış düşerse
/// `E_DB_ROLLBACK_REOPEN` ile **hangi sürümün geri konduğu** bildirilir.
pub fn rollback_and_resume(
    vault_db_path: &Path,
    backup: &Path,
    key: &DatabaseKey,
) -> Result<InPlaceRollback, StorageError> {
    let outcome = rollback_vault(vault_db_path, backup, key)?;
    match Database::open(vault_db_path, key) {
        Ok(database) => {
            let resumed_version = database.schema_version();
            // Denetim yeni (geri konan) kasaya yazılır: kayıt geri dönüşün
            // sonucunu taşır, yol taşımaz.
            let _ = database.append_audit(
                "vault_rollback",
                None,
                "completed",
                "local-user",
                &serde_json::json!({
                    "restoredVersion": outcome.restored_version,
                    "resumedVersion": resumed_version,
                    "safetyCopy": true,
                }),
            );
            Ok(InPlaceRollback {
                restored_version: outcome.restored_version,
                safety_copy: outcome.safety_copy,
                database,
            })
        }
        Err(err) => Err(StorageError::RollbackReopen {
            restored_version: outcome.restored_version,
            safety_copy: outcome.safety_copy.to_string_lossy().to_string(),
            reason: format!("{}: {}", err.code(), err),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> DatabaseKey {
        DatabaseKey::new([41u8; 32])
    }

    fn make_vault(dir: &Path) -> PathBuf {
        let path = dir.join("vault.db");
        let db = Database::open(&path, &key()).expect("kasa");
        drop(db);
        path
    }

    #[test]
    fn lists_only_this_vaults_backups_and_flags_corrupt_ones() {
        let dir = tempfile::tempdir().expect("dizin");
        let path = make_vault(dir.path());
        // Ad **güncel şemadan** türetilir: sabit sürüm yazmak, şema her
        // yükseldiğinde testi yalancı kırmızıya düşürüyordu (v18'de yakalandı).
        let good_name = format!("vault.db.v{CURRENT_SCHEMA_VERSION}-pre-update.bak");
        std::fs::copy(&path, dir.path().join(&good_name)).expect("kopya");
        std::fs::copy(&path, dir.path().join("baska.db.v12-pre-update.bak")).expect("yabancı");
        std::fs::write(dir.path().join("vault.db.v12-pre-update.bak"), b"bozuk")
            .expect("bozuk yedek");
        std::fs::write(dir.path().join("notlar.txt"), b"x").expect("alakasız");
        std::fs::create_dir(dir.path().join("vault.db.v9-pre-update.bak")).expect("dizin");

        let list = pre_update_backups(&path, &key()).expect("liste");
        assert_eq!(list.len(), 2, "yalnız bu kasanın iki yedeği listelenir");

        let good = &list[0];
        assert_eq!(good.file_name, good_name);
        assert_eq!(good.claimed_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(good.verified_version, Some(CURRENT_SCHEMA_VERSION));
        assert_eq!(good.problem, None);
        assert!(good.usable());
        assert!(!good.name_mismatch(), "ad ve dosya aynı sürümü söylüyor");
        assert!(good.size_bytes > 0);
        assert!(good.modified_at.is_some());

        let corrupt = &list[1];
        assert_eq!(corrupt.file_name, "vault.db.v12-pre-update.bak");
        assert_eq!(corrupt.claimed_version, 12);
        assert_eq!(corrupt.verified_version, None);
        assert!(corrupt
            .problem
            .as_deref()
            .is_some_and(|p| p.starts_with("E_DB_MIGRATION_BACKUP")));
        assert!(!corrupt.usable(), "bozuk yedek kullanılabilir sayılmaz");
    }

    #[test]
    fn backup_of_another_vault_is_listed_but_not_usable() {
        let dir = tempfile::tempdir().expect("dizin");
        let path = make_vault(dir.path());
        let name = format!("vault.db.v{CURRENT_SCHEMA_VERSION}-pre-update.bak");
        std::fs::copy(&path, dir.path().join(&name)).expect("kopya");

        let list = pre_update_backups(&path, &DatabaseKey::new([7u8; 32])).expect("liste");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].verified_version, None);
        assert!(list[0].problem.is_some(), "yanlış anahtar sessizce geçmez");
        assert!(!list[0].usable());
    }

    #[test]
    fn renamed_backup_reports_name_mismatch_instead_of_trusting_the_name() {
        let dir = tempfile::tempdir().expect("dizin");
        let path = make_vault(dir.path());
        std::fs::copy(&path, dir.path().join("vault.db.v3-pre-update.bak")).expect("kopya");

        let list = pre_update_backups(&path, &key()).expect("liste");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].claimed_version, 3);
        assert_eq!(list[0].verified_version, Some(CURRENT_SCHEMA_VERSION));
        assert!(list[0].usable(), "doğrulanmış yedek kullanılabilir");
        assert!(
            list[0].name_mismatch(),
            "ad ile dosya sürümü ayrışırsa uyarı verilir"
        );
    }

    #[test]
    fn listing_does_not_modify_the_backup_file() {
        let dir = tempfile::tempdir().expect("dizin");
        let path = make_vault(dir.path());
        let backup_name = format!("vault.db.v{CURRENT_SCHEMA_VERSION}-pre-update.bak");
        let backup = dir.path().join(&backup_name);
        std::fs::copy(&path, &backup).expect("kopya");
        let before = std::fs::metadata(&backup).expect("meta").len();
        let bytes_before = std::fs::read(&backup).expect("bayt");

        let list = pre_update_backups(&path, &key()).expect("liste");
        assert!(list[0].usable());

        let bytes_after = std::fs::read(&backup).expect("bayt");
        assert_eq!(
            std::fs::metadata(&backup).expect("meta").len(),
            before,
            "boyut değişmemeli"
        );
        assert_eq!(bytes_before, bytes_after, "yedek baytları değişmemeli");
        assert!(
            !dir.path().join(format!("{backup_name}-wal")).exists(),
            "doğrulamanın bıraktığı -wal temizlenir"
        );
        assert!(
            !dir.path().join(format!("{backup_name}-shm")).exists(),
            "doğrulamanın bıraktığı -shm temizlenir"
        );
    }
}
