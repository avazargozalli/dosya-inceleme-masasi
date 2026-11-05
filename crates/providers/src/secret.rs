//! API anahtarının **kaynağı**. Anahtar hiçbir tabloya, Denetim kaydına veya
//! günlüğe yazılmaz (INV-11); yalnız istek başlığında kullanılır.
//!
//! Sıra: **OS güvenli deposu** (macOS: Anahtar Zinciri, Windows: Kimlik
//! Yöneticisi) → `DIM_API_KEY` ortam değişkeni. Hangi yolun gerçekten
//! kullanıldığı [`SecretTarget`] ile **açıkça** bildirilir; ekran ve profil
//! künyesi bu etiketi yazar, tahmin etmez. OS deposu bu makinede yoksa
//! (bugün: Linux) ya da okuma gerçek bir hatayla düşerse bu **sessizce
//! yutulmaz**: sırasıyla `note` alanına yazılır / çağırana hata döner.

#[cfg(test)]
use std::cell::RefCell;

use serde::{Deserialize, Serialize};

/// Anahtar zinciri kaydının hizmet adı. Uygulamaya özgüdür; başka bir
/// uygulamanın kaydıyla çakışmaz.
pub const KEYCHAIN_SERVICE: &str = "tr.dosya-inceleme-masasi";
/// Anahtar zinciri kaydının hesap adı (tek API profili).
pub const KEYCHAIN_ACCOUNT: &str = "api-key";
/// Ortam değişkeni: yalnız **yedek** yol (CI, geliştirme, elle kurulum).
pub const ENV_VAR: &str = "DIM_API_KEY";

/// Anahtarın **gerçekten** okunduğu yer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretTarget {
    /// OS güvenli deposu (macOS Anahtar Zinciri / Windows Kimlik Yöneticisi).
    OsKeychain,
    /// `DIM_API_KEY` ortam değişkeni.
    EnvVar,
    /// Hiçbir yerde yok.
    Missing,
}

impl SecretTarget {
    /// Profil künyesine/Denetim kaydına yazılan etiket. **Anahtar değeri
    /// değildir**; yalnız kaynağı söyler.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::OsKeychain => "os-keychain",
            Self::EnvVar => "env:DIM_API_KEY",
            Self::Missing => "none",
        }
    }

    /// Anahtar OS güvenli deposunda mı (şifreleme/izin açısından tercih edilen)?
    #[must_use]
    pub const fn is_secure(self) -> bool {
        matches!(self, Self::OsKeychain)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SecretError {
    #[error("bu platformda OS güvenli deposu yok: {0}")]
    Unavailable(&'static str),
    #[error("OS güvenli deposu isteği reddetti: {0}")]
    Rejected(String),
    #[error("anahtar boş olamaz")]
    EmptyKey,
}

impl SecretError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Unavailable(_) => "E_AI_SECRET_UNSUPPORTED",
            Self::Rejected(_) => "E_AI_SECRET",
            Self::EmptyKey => "E_AI_SECRET_EMPTY",
        }
    }
}

/// Anahtarın durumu. **Anahtar değeri bu tipte taşınmaz** (ekrana/IPC'ye
/// sızmaz); yalnız nereden okunduğu ve var olup olmadığı.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyStatus {
    pub target: SecretTarget,
    pub present: bool,
    /// OS deposu bu makinede kullanılabilir mi? `false` ise `note` nedenini
    /// yazar; "anahtar yok" demek **değildir**.
    pub os_store_available: bool,
    /// OS deposunun neden kullanılmadığı (anahtar değeri asla burada olmaz).
    pub note: Option<String>,
}

/// Bir sır deposu. Ürün yolu [`os_store`] kullanır; testler `MemoryStore`.
pub trait SecretStore {
    /// Kayıt yoksa `Ok(None)`; okuma hatası `Err` (sessizce "yok" sayılmaz).
    fn read(&self) -> Result<Option<String>, SecretError>;
    fn write(&self, secret: &str) -> Result<(), SecretError>;
    /// Kayıt yoksa başarı (idempotent silme).
    fn clear(&self) -> Result<(), SecretError>;
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{SecretError, SecretStore, KEYCHAIN_ACCOUNT, KEYCHAIN_SERVICE};

    /// macOS Anahtar Zinciri (genel parola kaydı).
    pub struct OsStore {
        service: String,
        account: String,
    }

    impl OsStore {
        #[must_use]
        pub fn new() -> Self {
            Self {
                service: KEYCHAIN_SERVICE.to_string(),
                account: KEYCHAIN_ACCOUNT.to_string(),
            }
        }

        /// Tatbikat/test kaydı: gerçek kullanıcı kaydına dokunmadan aynı
        /// kod yolunu ölçmek için ayrı hizmet adı.
        #[cfg(test)]
        pub fn with_names(service: &str, account: &str) -> Self {
            Self {
                service: service.to_string(),
                account: account.to_string(),
            }
        }
    }

    impl Default for OsStore {
        fn default() -> Self {
            Self::new()
        }
    }

    /// `errSecItemNotFound`: kayıt yok — hata değil.
    const ITEM_NOT_FOUND: i32 = -25300;

    impl SecretStore for OsStore {
        fn read(&self) -> Result<Option<String>, SecretError> {
            match security_framework::passwords::get_generic_password(&self.service, &self.account)
            {
                Ok(bytes) => String::from_utf8(bytes).map(Some).map_err(|_| {
                    SecretError::Rejected("kayıt UTF-8 değil; anahtar kullanılamaz".to_string())
                }),
                Err(error) if error.code() == ITEM_NOT_FOUND => Ok(None),
                Err(error) => Err(SecretError::Rejected(format!(
                    "okunamadı (kod {})",
                    error.code()
                ))),
            }
        }

        fn write(&self, secret: &str) -> Result<(), SecretError> {
            let secret = secret.trim();
            if secret.is_empty() {
                return Err(SecretError::EmptyKey);
            }
            security_framework::passwords::set_generic_password(
                &self.service,
                &self.account,
                secret.as_bytes(),
            )
            .map_err(|error| SecretError::Rejected(format!("yazılamadı (kod {})", error.code())))
        }

        fn clear(&self) -> Result<(), SecretError> {
            match security_framework::passwords::delete_generic_password(
                &self.service,
                &self.account,
            ) {
                Ok(()) => Ok(()),
                Err(error) if error.code() == ITEM_NOT_FOUND => Ok(()),
                Err(error) => Err(SecretError::Rejected(format!(
                    "silinemedi (kod {})",
                    error.code()
                ))),
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{SecretError, SecretStore, KEYCHAIN_ACCOUNT, KEYCHAIN_SERVICE};
    use keyring::{Entry, Error as KeyringError};

    /// Windows **Kimlik Yöneticisi** (Credential Manager). Kayıt genel
    /// türdedir; hedef adı [`KEYCHAIN_SERVICE`], kullanıcı adı
    /// [`KEYCHAIN_ACCOUNT`]tır. `keyring` Win32 çağrılarını **güvenli** API ile
    /// sarar; bu crate `unsafe` kullanmaz (çalışma alanı kapısı
    /// `unsafe_code = "forbid"`).
    pub struct OsStore {
        service: String,
        account: String,
    }

    impl OsStore {
        #[must_use]
        pub fn new() -> Self {
            Self {
                service: KEYCHAIN_SERVICE.to_string(),
                account: KEYCHAIN_ACCOUNT.to_string(),
            }
        }

        /// Tatbikat/test kaydı: gerçek ürün kaydına dokunmadan aynı kod
        /// yolunu ölçmek için ayrı hizmet adı.
        #[cfg(test)]
        pub fn with_names(service: &str, account: &str) -> Self {
            Self {
                service: service.to_string(),
                account: account.to_string(),
            }
        }

        fn entry(&self) -> Result<Entry, SecretError> {
            Entry::new(&self.service, &self.account)
                .map_err(|error| SecretError::Rejected(format!("depo açılamadı: {error}")))
        }
    }

    impl Default for OsStore {
        fn default() -> Self {
            Self::new()
        }
    }

    impl SecretStore for OsStore {
        fn read(&self) -> Result<Option<String>, SecretError> {
            match self.entry()?.get_password() {
                Ok(secret) => Ok(Some(secret)),
                // Kayıt yok — hata değil (macOS `errSecItemNotFound` karşılığı).
                Err(KeyringError::NoEntry) => Ok(None),
                // Gerçek erişim/bozulma hatası sessizce yedeğe düşmez.
                Err(error) => Err(SecretError::Rejected(format!("okunamadı: {error}"))),
            }
        }

        fn write(&self, secret: &str) -> Result<(), SecretError> {
            let secret = secret.trim();
            if secret.is_empty() {
                return Err(SecretError::EmptyKey);
            }
            self.entry()?
                .set_password(secret)
                .map_err(|error| SecretError::Rejected(format!("yazılamadı: {error}")))
        }

        fn clear(&self) -> Result<(), SecretError> {
            match self.entry()?.delete_credential() {
                Ok(()) => Ok(()),
                // Idempotent silme: kayıt yoksa başarı.
                Err(KeyringError::NoEntry) => Ok(()),
                Err(error) => Err(SecretError::Rejected(format!("silinemedi: {error}"))),
            }
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    use super::{SecretError, SecretStore};

    /// Henüz uygulanmadı: Linux Secret Service. **Sessizce yok sayılmaz** —
    /// çağıran `Unavailable` alır, ekran ortam değişkenine düştüğünü yazar.
    #[derive(Debug, Default)]
    pub struct OsStore;

    impl OsStore {
        #[must_use]
        pub fn new() -> Self {
            Self
        }

        /// Tatbikat/test kaydı. Bu platformda depo olmadığı için adlar yok
        /// sayılır — kullanılmayan alan tutmak `dead_code` kapısını düşürürdü.
        #[cfg(test)]
        pub fn with_names(_service: &str, _account: &str) -> Self {
            Self
        }
    }

    impl SecretStore for OsStore {
        fn read(&self) -> Result<Option<String>, SecretError> {
            Err(SecretError::Unavailable(
                "yalnız macOS Anahtar Zinciri ve Windows Kimlik Yöneticisi uygulandı",
            ))
        }

        fn write(&self, _secret: &str) -> Result<(), SecretError> {
            Err(SecretError::Unavailable(
                "yalnız macOS Anahtar Zinciri ve Windows Kimlik Yöneticisi uygulandı",
            ))
        }

        fn clear(&self) -> Result<(), SecretError> {
            Err(SecretError::Unavailable(
                "yalnız macOS Anahtar Zinciri ve Windows Kimlik Yöneticisi uygulandı",
            ))
        }
    }
}

pub use platform::OsStore;

/// Ürün yolu: OS güvenli deposu.
#[must_use]
pub fn os_store() -> OsStore {
    OsStore::new()
}

/// Ortam değişkeni yalnız **boş olmayan** değeriyle yedek yoldur.
#[must_use]
pub fn env_key() -> Option<String> {
    std::env::var(ENV_VAR)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn lookup(
    store: &dyn SecretStore,
    env: Option<&str>,
) -> Result<(Option<String>, ApiKeyStatus), SecretError> {
    let mut note = None;
    let mut os_store_available = true;
    match store.read() {
        Ok(Some(key)) if !key.trim().is_empty() => {
            return Ok((
                Some(key),
                ApiKeyStatus {
                    target: SecretTarget::OsKeychain,
                    present: true,
                    os_store_available: true,
                    note: None,
                },
            ));
        }
        // Boş kayıt "anahtar var" sayılmaz; aşağıda ortam değişkenine düşer.
        Ok(_) => {}
        Err(SecretError::Unavailable(reason)) => {
            os_store_available = false;
            note = Some(format!("OS güvenli deposu kullanılamıyor: {reason}"));
        }
        // Gerçek okuma hatası (izin/bozuk kayıt) sessizce yedeğe düşmez:
        // kullanıcı anahtarını sandığı yerde bulamıyorsa bunu bilmeli.
        Err(error) => return Err(error),
    }
    let env_key = env
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let status = match env_key {
        Some(_) => ApiKeyStatus {
            target: SecretTarget::EnvVar,
            present: true,
            os_store_available,
            note,
        },
        None => ApiKeyStatus {
            target: SecretTarget::Missing,
            present: false,
            os_store_available,
            note,
        },
    };
    Ok((env_key, status))
}

/// Anahtarın durumu (değeri **döndürmez**).
pub fn status(store: &dyn SecretStore, env: Option<&str>) -> Result<ApiKeyStatus, SecretError> {
    Ok(lookup(store, env)?.1)
}

/// Anahtar ve kaynağı. `Missing` ise anahtar `None`dur.
pub fn resolve(
    store: &dyn SecretStore,
    env: Option<&str>,
) -> Result<(Option<String>, SecretTarget), SecretError> {
    let (key, status) = lookup(store, env)?;
    Ok((key, status.target))
}

/// Ürün yolu: OS deposu → `DIM_API_KEY`.
pub fn resolve_default() -> Result<(Option<String>, SecretTarget), SecretError> {
    resolve(&os_store(), env_key().as_deref())
}

/// Ürün yolu: durum sorgusu.
pub fn status_default() -> Result<ApiKeyStatus, SecretError> {
    status(&os_store(), env_key().as_deref())
}

/// Anahtarı OS güvenli deposuna yazar; durumu döndürür. Düz metin dosya,
/// tablo veya Denetim kaydı **yazılmaz**.
pub fn store_key(store: &dyn SecretStore, secret: &str) -> Result<ApiKeyStatus, SecretError> {
    store.write(secret)?;
    status(store, env_key().as_deref())
}

pub fn clear_key(store: &dyn SecretStore) -> Result<ApiKeyStatus, SecretError> {
    store.clear()?;
    status(store, env_key().as_deref())
}

#[cfg(test)]
pub struct MemoryStore {
    slot: RefCell<Option<String>>,
    mode: MemoryMode,
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MemoryMode {
    Working,
    Unavailable,
    Broken,
}

#[cfg(test)]
impl MemoryStore {
    pub fn with_key(key: &str) -> Self {
        Self {
            slot: RefCell::new(Some(key.to_string())),
            mode: MemoryMode::Working,
        }
    }

    pub fn empty() -> Self {
        Self {
            slot: RefCell::new(None),
            mode: MemoryMode::Working,
        }
    }

    pub fn unavailable() -> Self {
        Self {
            slot: RefCell::new(None),
            mode: MemoryMode::Unavailable,
        }
    }

    pub fn broken() -> Self {
        Self {
            slot: RefCell::new(Some("yazılabilir-gorunuyor".to_string())),
            mode: MemoryMode::Broken,
        }
    }
}

#[cfg(test)]
impl SecretStore for MemoryStore {
    fn read(&self) -> Result<Option<String>, SecretError> {
        match self.mode {
            MemoryMode::Unavailable => Err(SecretError::Unavailable("test: depo yok")),
            MemoryMode::Broken => Err(SecretError::Rejected("test: okuma reddedildi".into())),
            MemoryMode::Working => Ok(self.slot.borrow().clone()),
        }
    }

    fn write(&self, secret: &str) -> Result<(), SecretError> {
        let secret = secret.trim();
        if secret.is_empty() {
            return Err(SecretError::EmptyKey);
        }
        match self.mode {
            MemoryMode::Unavailable => Err(SecretError::Unavailable("test: depo yok")),
            MemoryMode::Broken => Err(SecretError::Rejected("test: yazma reddedildi".into())),
            MemoryMode::Working => {
                *self.slot.borrow_mut() = Some(secret.to_string());
                Ok(())
            }
        }
    }

    fn clear(&self) -> Result<(), SecretError> {
        match self.mode {
            MemoryMode::Unavailable => Err(SecretError::Unavailable("test: depo yok")),
            MemoryMode::Broken => Err(SecretError::Rejected("test: silme reddedildi".into())),
            MemoryMode::Working => {
                *self.slot.borrow_mut() = None;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keychain_wins_over_env() {
        let store = MemoryStore::with_key("anahtar-zinciri");
        let (key, target) = resolve(&store, Some("ortam-anahtari")).expect("çözüm");
        assert_eq!(key.as_deref(), Some("anahtar-zinciri"));
        assert_eq!(target, SecretTarget::OsKeychain);
        assert!(target.is_secure());
        assert_eq!(target.label(), "os-keychain");
    }

    #[test]
    fn env_is_used_when_store_is_empty() {
        let store = MemoryStore::empty();
        let (key, target) = resolve(&store, Some("  ortam-anahtari  ")).expect("çözüm");
        assert_eq!(key.as_deref(), Some("ortam-anahtari"));
        assert_eq!(target, SecretTarget::EnvVar);
        assert_eq!(target.label(), "env:DIM_API_KEY");
    }

    #[test]
    fn missing_when_neither_source_has_a_key() {
        let store = MemoryStore::empty();
        let (key, target) = resolve(&store, Some("   ")).expect("çözüm");
        assert!(key.is_none());
        assert_eq!(target, SecretTarget::Missing);
        assert_eq!(target.label(), "none");
    }

    #[test]
    fn unavailable_store_falls_back_to_env_and_says_so() {
        let store = MemoryStore::unavailable();
        let (key, target) = resolve(&store, Some("ortam-anahtari")).expect("çözüm");
        assert_eq!(key.as_deref(), Some("ortam-anahtari"));
        assert_eq!(target, SecretTarget::EnvVar);
        let status = status(&store, Some("ortam-anahtari")).expect("durum");
        assert!(!status.os_store_available);
        let note = status.note.unwrap_or_default();
        assert!(note.contains("kullanılamıyor"), "{note}");
        // Not anahtar değerini taşımaz.
        assert!(!note.contains("ortam-anahtari"), "{note}");
    }

    #[test]
    fn blank_keychain_record_falls_back_to_env() {
        // Boş kayıt "anahtar var" sayılmaz: `present` yalan olmamalı.
        let store = MemoryStore::with_key("   ");
        let (key, target) = resolve(&store, Some("ortam-anahtari")).expect("çözüm");
        assert_eq!(key.as_deref(), Some("ortam-anahtari"));
        assert_eq!(target, SecretTarget::EnvVar);
        let (key, target) = resolve(&store, None).expect("çözüm");
        assert!(key.is_none());
        assert_eq!(target, SecretTarget::Missing);
    }

    #[test]
    fn rejected_read_is_not_silently_replaced_by_env() {
        let store = MemoryStore::broken();
        let error = resolve(&store, Some("ortam-anahtari")).unwrap_err();
        assert_eq!(error.code(), "E_AI_SECRET");
    }

    #[test]
    fn empty_key_is_refused_with_its_own_code() {
        let store = MemoryStore::empty();
        let error = store_key(&store, "   ").unwrap_err();
        assert_eq!(error.code(), "E_AI_SECRET_EMPTY");
        // Yazma denenmedi: depo boş kaldı.
        assert!(store.read().expect("okuma").is_none());
    }

    #[test]
    fn stored_key_round_trips_and_clear_falls_back_to_env() {
        let store = MemoryStore::empty();
        let status = store_key(&store, "  yeni-anahtar  ").expect("yazma");
        assert!(status.present);
        assert_eq!(status.target, SecretTarget::OsKeychain);
        // Kırpılarak yazıldı: yapıştırma artığı başlığa sızmasın.
        assert_eq!(
            store.read().expect("okuma").as_deref(),
            Some("yeni-anahtar")
        );

        let status = clear_key(&store).expect("silme");
        assert_eq!(status.target, SecretTarget::Missing);
        assert!(!status.present);

        // Silme idempotent: ikinci çağrı hata vermez.
        assert!(clear_key(&store).is_ok());
    }

    #[test]
    fn status_never_carries_the_key() {
        let store = MemoryStore::with_key("cok-gizli-anahtar");
        let status = status(&store, None).expect("durum");
        let json = serde_json::to_string(&status).expect("json");
        assert!(!json.contains("cok-gizli-anahtar"), "{json}");
        assert!(json.contains("\"target\":\"os_keychain\""), "{json}");
    }

    /// Gerçek Anahtar Zinciri kaydı: aynı kod yolu, ayrı hizmet adı (kullanıcı
    /// kaydına dokunmaz). Elle koşulur — CI'da anahtar zinciri etkileşimi
    /// olmayabilir. Windows karşılığı aşağıda **CI'da koşar**.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "gerçek Anahtar Zinciri kaydı yazar; `--ignored` ile elle koşulur"]
    fn os_keychain_round_trip() {
        let store = OsStore::with_names(
            &format!("{KEYCHAIN_SERVICE}.test"),
            &format!("api-key-{}", std::process::id()),
        );
        let _ = store.clear();
        assert!(store.read().expect("ilk okuma").is_none());

        let status = store_key(&store, "  zincir-anahtari  ").expect("yazma");
        assert!(status.present);
        assert_eq!(status.target, SecretTarget::OsKeychain);
        assert!(status.os_store_available);
        assert_eq!(
            store.read().expect("okuma").as_deref(),
            Some("zincir-anahtari")
        );

        let status = clear_key(&store).expect("silme");
        assert_eq!(status.target, SecretTarget::Missing);
        assert!(!status.present);
        assert!(store.read().expect("son okuma").is_none());
    }

    /// Gerçek **Windows Kimlik Yöneticisi** kaydı: ürün kaydına dokunmaz
    /// (ayrı hizmet adı + süreç kimliği). Yaz → oku → üzerine yaz → sil →
    /// yok sırasını **gerçek** Credential Manager üzerinde doğrular.
    /// `windows-latest` CI işinde koşar; diğer platformlarda derlenmez.
    #[cfg(target_os = "windows")]
    #[test]
    #[serial_test::serial]
    fn windows_credential_manager_round_trip() {
        let store = OsStore::with_names(
            &format!("{KEYCHAIN_SERVICE}.test"),
            &format!("api-key-{}", std::process::id()),
        );
        let _ = store.clear();
        assert!(store.read().expect("ilk okuma").is_none());

        let status = store_key(&store, "  kimlik-yoneticisi-anahtari  ").expect("yazma");
        assert!(status.present);
        assert_eq!(status.target, SecretTarget::OsKeychain);
        assert!(status.os_store_available);
        assert_eq!(
            store.read().expect("okuma").as_deref(),
            Some("kimlik-yoneticisi-anahtari")
        );

        // Üzerine yazma tek kayıt bırakır (rotasyon).
        store_key(&store, "rotasyon-anahtari").expect("rotasyon");
        assert_eq!(
            store.read().expect("rotasyon okuması").as_deref(),
            Some("rotasyon-anahtari")
        );

        let status = clear_key(&store).expect("silme");
        assert_eq!(status.target, SecretTarget::Missing);
        assert!(store.read().expect("son okuma").is_none());
        // Idempotent silme.
        assert!(store.clear().is_ok());
    }
}
