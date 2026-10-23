use std::path::{Path, PathBuf};
use std::sync::Mutex;

use dim_storage::{Database, DatabaseKey};
use dim_vault::UnlockedVault;
use sha2::{Digest, Sha256};

/// Kilit açıkken tutulan kasa durumu. Kilitliyken yalnız yol ve kimlik kalır;
/// anahtar ve veritabanı bağlantısı düşünce bellekten silinir.
#[derive(Default)]
pub struct VaultState {
    pub unlocked: Option<UnlockedVault>,
    pub db: Option<Database>,
    pub path: Option<PathBuf>,
    pub vault_id: Option<String>,
}

#[derive(Default)]
pub struct AppState {
    pub vault: Mutex<VaultState>,
}

impl AppState {
    pub fn lock(&self) -> std::sync::MutexGuard<'_, VaultState> {
        match self.vault.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                guard.unlocked = None;
                guard.db = None;
                guard
            }
        }
    }
}

/// Kasa kimliği: ana anahtarın SHA-256 özetinin ilk 16 baytı. Kimlikte dosya adı,
/// taraf veya esas numarası yoktur; yalnız kasayı tanıtır.
pub fn vault_id_from_master(master: &[u8; 32]) -> String {
    let digest = Sha256::digest(master.as_slice());
    hex::encode(&digest[..16])
}

/// Kasa dizinindeki şifreli veritabanını açar; anahtar kasa ana anahtarından
/// HKDF ile türetilir (aynı anahtar başka kasada geçersizdir).
pub fn open_database(
    vault_path: &Path,
    unlocked: &UnlockedVault,
    vault_id: &str,
) -> Result<Database, dim_storage::StorageError> {
    let key = DatabaseKey::derive_from_master(unlocked.master_key(), vault_id)?;
    let dir = vault_path.parent().unwrap_or_else(|| Path::new("."));
    Database::open(&dir.join("vault.db"), &key)
}
