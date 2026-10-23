use std::path::PathBuf;
use std::sync::Mutex;

use dim_vault::UnlockedVault;
use sha2::{Digest, Sha256};

/// Kilit açıkken tutulan kasa durumu. Kilitliyken yalnız yol ve kimlik kalır;
/// anahtar `UnlockedVault` düşünce `Zeroizing` ile temizlenir.
#[derive(Default)]
pub struct VaultState {
    pub unlocked: Option<UnlockedVault>,
    pub path: Option<PathBuf>,
    pub vault_id: Option<String>,
}

#[derive(Default)]
pub struct AppState {
    pub vault: Mutex<VaultState>,
}

impl AppState {
    pub fn lock(&self) -> std::sync::MutexGuard<'_, VaultState> {
        // Kilit zehirlenirse durum bilinmiyordur; anahtarı bırakıp hataya dönmek
        // yerine süreci yeniden başlatmak gerekir. Burada güvenli taraf: panic yok,
        // zehirli guard'ı `into_inner` ile al ve kasayı kilitli say.
        match self.vault.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                guard.unlocked = None;
                guard
            }
        }
    }
}

/// Kasa kimliği: ana anahtarın SHA-256 özetinin ilk 16 baytı. Kimlikte dosya adı,
/// taraf veya esas numarası yoktur; yalnız kasayı tanıtır.
pub fn vault_id_from_master(master: &[u8; 32]) -> String {
    let digest = Sha256::digest(master);
    hex::encode(&digest[..16])
}
