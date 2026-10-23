//! Sayfa önizleme önbelleği (F07-T11).
//!
//! İki katman vardır:
//! 1. **Disk**: PNG önizlemeleri kasa ana anahtarıyla AEAD şifreli dosyalar
//!    olarak `<kasa>/preview-cache/<sürüm>/<sayfa>_<dpi>.pv` altında tutulur.
//!    Dosya adı ve yol içerik sızdırmaz; içerik anahtar olmadan okunamaz.
//!    `INV-06`: önizleme de şifreleme kapsamındadır.
//! 2. **Bellek**: yalnız kilit açıkken tutulan küçük LRU; kasa kilitlenince
//!    `PreviewMemory::clear()` çağrılır ve çözülmüş PNG baytları düşer.
//!
//! Önbellek "doğruluk kaynağı" değildir: eksik/bozuk kayıt sessizce yok sayılmaz,
//! `load` hata döner ve çağıran yeniden üretir.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use dim_vault::{UnlockedVault, VaultError};

#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error("önizleme önbelleği G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("önizleme çözülemedi (bozuk kayıt veya yanlış anahtar)")]
    Crypto,
}

impl PreviewError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "E_PREVIEW_IO",
            Self::Crypto => "E_PREVIEW_CRYPTO",
        }
    }
}

impl From<VaultError> for PreviewError {
    fn from(value: VaultError) -> Self {
        match value {
            VaultError::Io(error) => Self::Io(error),
            VaultError::NotFound(_) => Self::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "önizleme dosyası yok",
            )),
            _ => Self::Crypto,
        }
    }
}

/// Önbellek anahtarı: belge sürümü + fiziksel sayfa + çözünürlük.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewKey {
    pub version_id: String,
    pub page_index: u32,
    pub dpi: u32,
}

impl PreviewKey {
    #[must_use]
    pub fn new(version_id: impl Into<String>, page_index: u32, dpi: u32) -> Self {
        Self {
            version_id: version_id.into(),
            page_index,
            dpi,
        }
    }

    /// AEAD ek verisi: kayıt başka sürüm/sayfaya taşınamaz (bağlama).
    #[must_use]
    pub fn aad(&self) -> String {
        format!(
            "preview:v1:{}:{}:{}",
            self.version_id, self.page_index, self.dpi
        )
    }

    fn file_name(&self) -> String {
        format!("{}_{}.pv", self.page_index, self.dpi)
    }

    fn cache_key(&self) -> String {
        format!("{}:{}:{}", self.version_id, self.page_index, self.dpi)
    }
}

/// Disk katmanı: `<root>/<sürüm>/<sayfa>_<dpi>.pv` (AEAD şifreli).
#[derive(Debug, Clone)]
pub struct PreviewStore {
    root: PathBuf,
}

impl PreviewStore {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn path_for(&self, key: &PreviewKey) -> PathBuf {
        self.root.join(&key.version_id).join(key.file_name())
    }

    /// Kayıt varsa çözülmüş PNG baytlarını döner; yoksa `Ok(None)`.
    pub fn load(
        &self,
        vault: &UnlockedVault,
        key: &PreviewKey,
    ) -> Result<Option<Vec<u8>>, PreviewError> {
        let path = self.path_for(key);
        if !path.exists() {
            return Ok(None);
        }
        match vault.read_blob(&path, key.aad().as_bytes()) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(VaultError::Crypto) => Err(PreviewError::Crypto),
            Err(error) => Err(error.into()),
        }
    }

    /// Önizlemeyi şifreleyip diske yazar. Aynı anahtar için eski kayıt varsa
    /// atomik olarak değiştirilir: kısmi dosya okunmaz.
    pub fn store(
        &self,
        vault: &UnlockedVault,
        key: &PreviewKey,
        png: &[u8],
    ) -> Result<PathBuf, PreviewError> {
        let dir = self.root.join(&key.version_id);
        let written = vault.write_blob(&dir, png, key.aad().as_bytes())?;
        let target = self.path_for(key);
        if written != target {
            std::fs::rename(&written, &target)?;
        }
        Ok(target)
    }

    /// Sürümün tüm önizlemelerini siler (belge kaldırılınca). Dönen sayı silinen
    /// dosya sayısıdır; dizin yoksa 0.
    pub fn purge_version(&self, version_id: &str) -> Result<usize, PreviewError> {
        let dir = self.root.join(version_id);
        if !dir.exists() {
            return Ok(0);
        }
        let count = std::fs::read_dir(&dir)?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_file())
            .count();
        std::fs::remove_dir_all(&dir)?;
        Ok(count)
    }
}

/// Bellek katmanı: kilit açıkken tutulan, sınırlı LRU. Kilitte `clear()`.
#[derive(Debug)]
pub struct PreviewMemory {
    limit_bytes: usize,
    used: usize,
    order: VecDeque<String>,
    entries: HashMap<String, (PreviewKey, Vec<u8>)>,
}

impl Default for PreviewMemory {
    fn default() -> Self {
        Self::new(32 * 1024 * 1024)
    }
}

impl PreviewMemory {
    #[must_use]
    pub fn new(limit_bytes: usize) -> Self {
        Self {
            limit_bytes,
            used: 0,
            order: VecDeque::new(),
            entries: HashMap::new(),
        }
    }

    #[must_use]
    pub fn limit_bytes(&self) -> usize {
        self.limit_bytes
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn used_bytes(&self) -> usize {
        self.used
    }

    pub fn get(&mut self, key: &PreviewKey) -> Option<Vec<u8>> {
        let cache_key = key.cache_key();
        let (_, bytes) = self.entries.get(&cache_key)?;
        let bytes = bytes.clone();
        // Dokunuşu sıranın sonuna taşı (LRU).
        self.order.retain(|existing| existing != &cache_key);
        self.order.push_back(cache_key);
        Some(bytes)
    }

    /// Önizlemeyi ekler; bütçe aşılırsa en eski kayıtlar düşer. Tek başına
    /// bütçeden büyük önizleme bellekte tutulmaz (diskte şifreli kalır).
    pub fn put(&mut self, key: &PreviewKey, bytes: Vec<u8>) {
        if bytes.len() > self.limit_bytes {
            return;
        }
        let cache_key = key.cache_key();
        if let Some((_, previous)) = self.entries.remove(&cache_key) {
            self.used = self.used.saturating_sub(previous.len());
            self.order.retain(|existing| existing != &cache_key);
        }
        while self.used + bytes.len() > self.limit_bytes {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some((_, evicted)) = self.entries.remove(&oldest) {
                self.used = self.used.saturating_sub(evicted.len());
            }
        }
        self.used += bytes.len();
        self.order.push_back(cache_key.clone());
        self.entries.insert(cache_key, (key.clone(), bytes));
    }

    /// Bellekteki tüm çözülmüş önizlemeleri bırakır (kasa kilidi).
    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.used = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dim_vault::VaultFile;

    fn vault(dir: &Path) -> UnlockedVault {
        let (file, _recovery, _unlocked) =
            VaultFile::create(&dir.join("kasa.dimvault"), "parola-123").expect("kasa");
        VaultFile::open(&file.path, "parola-123").expect("aç")
    }

    #[test]
    fn stored_preview_is_encrypted_on_disk_and_round_trips() {
        let dir = tempfile::tempdir().expect("dizin");
        let vault = vault(dir.path());
        let store = PreviewStore::new(dir.path().join("preview-cache"));
        let key = PreviewKey::new("v1", 0, 96);
        let png = b"\x89PNG\r\n\x1a\n bu bir onizleme".to_vec();
        let path = store.store(&vault, &key, &png).expect("yaz");
        assert!(path.exists());
        // Diskteki baytlar düz PNG değildir: şifreli kayıt.
        let raw = std::fs::read(&path).expect("oku");
        assert_ne!(&raw[..], &png[..]);
        assert!(!raw.windows(8).any(|window| window == b"\x89PNG\r\n\x1a\n"));
        let loaded = store.load(&vault, &key).expect("oku").expect("var");
        assert_eq!(loaded, png);
    }

    #[test]
    fn preview_bound_to_version_page_and_dpi() {
        let dir = tempfile::tempdir().expect("dizin");
        let vault = vault(dir.path());
        let store = PreviewStore::new(dir.path().join("preview-cache"));
        let key = PreviewKey::new("v1", 2, 96);
        store.store(&vault, &key, b"png").expect("yaz");
        let other = PreviewKey::new("v1", 3, 96);
        // Aynı dosya adı başka sayfa için çağrılırsa AAD uyuşmaz.
        let borrowed = store.path_for(&key);
        assert!(store.load(&vault, &other).expect("yok").is_none());
        assert!(std::fs::copy(&borrowed, store.path_for(&other)).is_ok());
        let error = store.load(&vault, &other).expect_err("aad uyuşmaz");
        assert_eq!(error.code(), "E_PREVIEW_CRYPTO");
    }

    #[test]
    fn purge_removes_version_directory() {
        let dir = tempfile::tempdir().expect("dizin");
        let vault = vault(dir.path());
        let store = PreviewStore::new(dir.path().join("preview-cache"));
        store
            .store(&vault, &PreviewKey::new("v1", 0, 96), b"a")
            .expect("yaz");
        store
            .store(&vault, &PreviewKey::new("v1", 1, 96), b"b")
            .expect("yaz");
        assert_eq!(store.purge_version("v1").expect("sil"), 2);
        assert!(!dir.path().join("preview-cache").join("v1").exists());
        assert_eq!(store.purge_version("v1").expect("sil"), 0);
    }

    #[test]
    fn memory_is_lru_and_bounded() {
        let mut memory = PreviewMemory::new(10);
        let first = PreviewKey::new("v1", 0, 96);
        let second = PreviewKey::new("v1", 1, 96);
        let third = PreviewKey::new("v1", 2, 96);
        memory.put(&first, vec![1; 4]);
        memory.put(&second, vec![2; 4]);
        assert_eq!(memory.used_bytes(), 8);
        // İlk kayda dokun: artık en eski ikinci olur.
        assert!(memory.get(&first).is_some());
        memory.put(&third, vec![3; 4]);
        assert_eq!(memory.len(), 2);
        assert!(memory.get(&first).is_some());
        assert!(memory.get(&second).is_none());
        assert!(memory.get(&third).is_some());
    }

    #[test]
    fn memory_drops_everything_on_clear() {
        let mut memory = PreviewMemory::new(1024);
        memory.put(&PreviewKey::new("v1", 0, 96), vec![7; 32]);
        memory.put(&PreviewKey::new("v1", 1, 96), vec![8; 32]);
        assert_eq!(memory.len(), 2);
        memory.clear();
        assert!(memory.is_empty());
        assert_eq!(memory.used_bytes(), 0);
        assert!(memory.get(&PreviewKey::new("v1", 0, 96)).is_none());
    }

    #[test]
    fn oversized_preview_is_not_kept_in_memory() {
        let mut memory = PreviewMemory::new(8);
        memory.put(&PreviewKey::new("v1", 0, 96), vec![0; 9]);
        assert!(memory.is_empty());
        assert_eq!(memory.used_bytes(), 0);
    }
}
