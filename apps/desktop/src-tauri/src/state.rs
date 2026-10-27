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

/// Süren belge işi: hangi iş/sürüm ve **süreci durdurma** tutamağı
/// (şartname §17.5, F04-T07). İş kasa kilidini tutarken iptal komutu
/// bu yuvaya bakıp süreci öldürür; kasa kilidi beklenmez.
///
/// **§17.3:** aynı anda en fazla iki belge worker'ı koşabildiği için yuva **liste**
/// tutar; iptal, iş kimliğine (yoksa sürüm kimliğine) göre eşleşen süreci
/// durdurur. `kind` hangi işin süreci olduğunu söyler (`parse` · `redact`).
pub struct RunningWorker {
    pub kind: &'static str,
    pub version_id: String,
    pub job_id: Option<String>,
    pub killer: dim_ingest::worker::WorkerKiller,
}

impl std::fmt::Debug for RunningWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunningWorker")
            .field("kind", &self.kind)
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
    /// Süren ayrıştırmalar (§17.3: en fazla iki). Boşken ayrıştırma yok.
    pub workers: Arc<Mutex<Vec<RunningWorker>>>,
    /// Belge worker eşzamanlılık kapısı (§17.3). Kapasite 2; büyük belge tek.
    pub parse_pool: Arc<crate::parse_pool::ParsePool>,
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

    /// Süren belge işleri yuvası (ayrıştırma + maskeleme). Zehirli kilitte yuva temizlenir: panikle
    /// yarıda kalan ayrıştırma yeni ayrıştırmayı kilitlemez.
    pub fn workers_lock(&self) -> MutexGuard<'_, Vec<RunningWorker>> {
        match self.workers.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                guard.clear();
                guard
            }
        }
    }

    /// `spawn_blocking` için kasa tutamacı (kilidi orada alınır).
    #[must_use]
    pub fn vault_handle(&self) -> Arc<Mutex<VaultState>> {
        Arc::clone(&self.vault)
    }

    /// `spawn_blocking` için belge işleri yuvası tutamacı.
    #[must_use]
    pub fn workers_handle(&self) -> Arc<Mutex<Vec<RunningWorker>>> {
        Arc::clone(&self.workers)
    }

    /// `spawn_blocking` için eşzamanlılık kapısı tutamacı.
    #[must_use]
    pub fn parse_pool_handle(&self) -> Arc<crate::parse_pool::ParsePool> {
        Arc::clone(&self.parse_pool)
    }

    /// İş kimliğine uyan süreci durdurur. Dönen değer: süreç gerçekten
    /// durduruldu mu (yuva kilidi kısa tutulur, kasa kilidi **alınmaz** — §17.5).
    pub fn cancel_running_job(&self, job_id: &str) -> bool {
        let killer = {
            let slot = self.workers_lock();
            slot.iter()
                .find(|run| run.job_id.as_deref() == Some(job_id))
                .map(|run| run.killer.clone())
        };
        killer.map(|killer| killer.cancel()).unwrap_or(false)
    }

    /// Kasa kilitlenirken/çıkışta süren bütün belge işlerini durdurur.
    /// Dönen değer: durdurulan süreç sayısı.
    pub fn cancel_all_workers(&self) -> usize {
        let killers: Vec<_> = {
            let slot = self.workers_lock();
            slot.iter().map(|run| run.killer.clone()).collect()
        };
        killers.into_iter().filter(|killer| killer.cancel()).count()
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
pub struct WorkerSlotGuard {
    slot: Arc<Mutex<Vec<RunningWorker>>>,
    version_id: String,
    job_id: Option<String>,
}

impl WorkerSlotGuard {
    /// Süren ayrıştırmayı yuvaya yazar (§17.3: birden çok kayıt olabilir).
    #[must_use]
    pub fn register(slot: Arc<Mutex<Vec<RunningWorker>>>, entry: RunningWorker) -> Self {
        let version_id = entry.version_id.clone();
        let job_id = entry.job_id.clone();
        match slot.lock() {
            Ok(mut guard) => guard.push(entry),
            Err(poisoned) => poisoned.into_inner().push(entry),
        }
        Self {
            slot,
            version_id,
            job_id,
        }
    }
}

impl Drop for WorkerSlotGuard {
    fn drop(&mut self) {
        let version_id = &self.version_id;
        let job_id = &self.job_id;
        match self.slot.lock() {
            Ok(mut guard) => {
                guard.retain(|run| !(run.version_id == *version_id && run.job_id == *job_id))
            }
            Err(poisoned) => poisoned
                .into_inner()
                .retain(|run| !(run.version_id == *version_id && run.job_id == *job_id)),
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
