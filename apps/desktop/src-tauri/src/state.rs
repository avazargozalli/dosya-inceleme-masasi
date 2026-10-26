use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

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

/// Süren belge ayrıştırması: hangi iş/sürüm ve **süreci durdurma** tutamağı
/// (şartname §17.5, F04-T07). Ayrıştırma kasa kilidini tutarken iptal komutu
/// bu yuvaya bakıp süreci öldürür; kasa kilidi beklenmez.
pub struct RunningParse {
    pub version_id: String,
    pub job_id: Option<String>,
    pub killer: dim_ingest::worker::WorkerKiller,
}

impl std::fmt::Debug for RunningParse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunningParse")
            .field("version_id", &self.version_id)
            .field("job_id", &self.job_id)
            .field("canceled", &self.killer.is_canceled())
            .finish()
    }
}

#[derive(Default)]
pub struct AppState {
    /// Kasa durumu paylaşılır (`Arc`): uzun komutlar `spawn_blocking` içinde
    /// çalışıp kilidi orada tutar, arayüz iş parçacığı beklemez.
    pub vault: Arc<Mutex<VaultState>>,
    /// Çözülmüş sayfa önizlemeleri; yalnız kilit açıkken tutulur (F07-T11).
    pub preview: Mutex<dim_preview::PreviewMemory>,
    /// Süren CLI sınama işareti (F10). `None` iken sınama yok; aynı anda tek
    /// sınama çalışır ve iptal düğmesi bu bayrağı işaretler.
    pub cli_probe: Mutex<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>>,
    /// Süren ayrıştırma yuvası (F04-T07). `None` iken ayrıştırma yok.
    pub parse: Arc<Mutex<Option<RunningParse>>>,
}

impl AppState {
    /// Önizleme belleği. Kasa kilidinde `clear()` çağrılır; çözülmüş PNG
    /// baytları bellekte kalmaz.
    pub fn preview_lock(&self) -> std::sync::MutexGuard<'_, dim_preview::PreviewMemory> {
        match self.preview.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                guard.clear();
                guard
            }
        }
    }

    /// Süren CLI sınama yuvası (F10). Zehirli kilitten sonra bayrak temizlenir:
    /// panikle yarıda kalan bir sınama yeni sınamayı kalıcı olarak engellemez.
    pub fn cli_probe_lock(
        &self,
    ) -> std::sync::MutexGuard<'_, Option<std::sync::Arc<std::sync::atomic::AtomicBool>>> {
        match self.cli_probe.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                *guard = None;
                guard
            }
        }
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, VaultState> {
        lock_vault(&self.vault)
    }

    /// Süren ayrıştırma yuvası. Zehirli kilitte yuva temizlenir: panikle
    /// yarıda kalan ayrıştırma yeni ayrıştırmayı kilitlemez.
    pub fn parse_lock(&self) -> MutexGuard<'_, Option<RunningParse>> {
        match self.parse.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                *guard = None;
                guard
            }
        }
    }

    /// `spawn_blocking` için kasa tutamacı (kilidi orada alınır).
    #[must_use]
    pub fn vault_handle(&self) -> Arc<Mutex<VaultState>> {
        Arc::clone(&self.vault)
    }

    /// `spawn_blocking` için ayrıştırma yuvası tutamacı.
    #[must_use]
    pub fn parse_handle(&self) -> Arc<Mutex<Option<RunningParse>>> {
        Arc::clone(&self.parse)
    }
}

/// Kasa kilidini alır; zehirli kilitte anahtar ve bağlantı **bellekten
/// silinir** (kilit açık sanılmaz).
pub fn lock_vault(handle: &Mutex<VaultState>) -> MutexGuard<'_, VaultState> {
    match handle.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            let mut guard = poisoned.into_inner();
            guard.unlocked = None;
            guard.db = None;
            guard
        }
    }
}

/// Ayrıştırma yuvasını temizleyen nöbetçi: hata veya panik yolunda da yuva
/// `None` kalır (yarım ayrıştırma yeni ayrıştırmayı engellemez).
pub struct ParseSlotGuard {
    slot: Arc<Mutex<Option<RunningParse>>>,
}

impl ParseSlotGuard {
    #[must_use]
    pub fn register(slot: Arc<Mutex<Option<RunningParse>>>, entry: RunningParse) -> Self {
        match slot.lock() {
            Ok(mut guard) => *guard = Some(entry),
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                *guard = Some(entry);
            }
        }
        Self { slot }
    }
}

impl Drop for ParseSlotGuard {
    fn drop(&mut self) {
        match self.slot.lock() {
            Ok(mut guard) => *guard = None,
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                *guard = None;
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

// `AtomicBool` doğrudan içe aktarılmaz: `cli_probe` alanı tam yoluyla yazılı.
