//! Şifreli kasa çekirdeği.
//!
//! İki katman vardır:
//! 1. **Kasa başlığı** — rastgele ana anahtarın parola ve kurtarma anahtarıyla
//!    sarıldığı sürümlü, sabit boyutlu başlık (Argon2id + AES-256-GCM).
//! 2. **Blob** — büyük dosyalar için sabit boyutlu parçalarla akan AEAD;
//!    kesilme ve bit değiştirme tespit edilir.
//!
//! Ana anahtar süreç belleğinde yalnız `Zeroizing` içinde tutulur ve `Debug`
//! çıktısında görünmez.

mod blob;
mod header;
mod recovery;

pub use blob::{decrypt_blob, encrypt_blob, BLOB_CHUNK_SIZE, BLOB_HEADER_LEN};
pub use header::{KdfParams, VaultHeader, HEADER_LEN};
pub use recovery::{format_recovery_key, normalize_recovery_key, RecoveryKey};

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use hkdf::Hkdf;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::Sha256;
use zeroize::Zeroizing;

/// Varsayılan KDF maliyeti: 64 MiB, 3 tur, 1 şerit (F03'te cihaz ölçümüyle ayarlanır).
pub const DEFAULT_KDF: KdfParams = KdfParams {
    m_cost_kib: 65_536,
    t_cost: 3,
    p_cost: 1,
};

const MASTER_LEN: usize = 32;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("kasa dosyası bulunamadı: {0}")]
    NotFound(PathBuf),
    #[error("kasa başlığı geçersiz veya desteklenmeyen sürüm")]
    BadHeader,
    #[error("parola yanlış veya başlık bozulmuş")]
    BadCredentials,
    #[error("kurtarma anahtarı geçersiz")]
    BadRecoveryKey,
    #[error("blob bütünlüğü bozuk: {0}")]
    Corrupt(&'static str),
    #[error("G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("kriptografik işlem başarısız")]
    Crypto,
}

impl VaultError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "E_VAULT_NOT_FOUND",
            Self::BadHeader => "E_VAULT_HEADER",
            Self::BadCredentials => "E_VAULT_CREDENTIALS",
            Self::BadRecoveryKey => "E_VAULT_RECOVERY",
            Self::Corrupt(_) => "E_VAULT_CORRUPT",
            Self::Io(_) => "E_IO",
            Self::Crypto => "E_CRYPTO",
        }
    }
}

/// Kilit açılmış kasa: ana anahtar yalnız burada, bellekte.
pub struct UnlockedVault {
    master_key: Zeroizing<[u8; MASTER_LEN]>,
    header: VaultHeader,
}

impl std::fmt::Debug for UnlockedVault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnlockedVault")
            .field("version", &self.header.version)
            .field("master_key", &"<redacted>")
            .finish()
    }
}

impl UnlockedVault {
    /// Blob ve başlık işlemlerinde kullanılan ana anahtar. Çağıran taraf kopyalamaz.
    #[must_use]
    pub fn master_key(&self) -> &[u8; MASTER_LEN] {
        &self.master_key
    }

    #[must_use]
    pub const fn header(&self) -> &VaultHeader {
        &self.header
    }

    /// Kasa anahtarıyla bir blob'u dosyaya şifreler (atomik: geçici dosya + rename).
    pub fn write_blob(&self, dir: &Path, plain: &[u8], aad: &[u8]) -> Result<PathBuf, VaultError> {
        fs::create_dir_all(dir)?;
        let storage_id = random_storage_id();
        let final_path = dir.join(format!("{storage_id}.blb"));
        let tmp_path = dir.join(format!(".{storage_id}.blb.tmp"));
        {
            let mut out = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&tmp_path)?;
            encrypt_blob(plain, &mut out, &self.master_key, aad)?;
            out.flush()?;
            out.sync_all()?;
        }
        fs::rename(&tmp_path, &final_path)?;
        sync_dir(dir)?;
        Ok(final_path)
    }

    pub fn read_blob(&self, path: &Path, aad: &[u8]) -> Result<Vec<u8>, VaultError> {
        let mut f = File::open(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                VaultError::NotFound(path.to_path_buf())
            } else {
                VaultError::Io(e)
            }
        })?;
        decrypt_blob(&mut f, &self.master_key, aad)
    }
}

/// Kasa başlığını içeren dosya (`*.dimvault`).
#[derive(Debug, Clone)]
pub struct VaultFile {
    pub path: PathBuf,
}

impl VaultFile {
    /// Yeni kasa oluşturur. Dosya zaten varsa üzerine yazmaz.
    pub fn create(
        path: &Path,
        password: &str,
    ) -> Result<(Self, RecoveryKey, UnlockedVault), VaultError> {
        if path.exists() {
            return Err(VaultError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "kasa dosyası zaten var",
            )));
        }
        let mut master = Zeroizing::new([0u8; MASTER_LEN]);
        OsRng.fill_bytes(master.as_mut());

        let mut salt_pw = [0u8; SALT_LEN];
        let mut nonce_pw = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut salt_pw);
        OsRng.fill_bytes(&mut nonce_pw);

        let kek_pw = derive_kek(password.as_bytes(), &salt_pw, DEFAULT_KDF)?;
        let wrapped_pw = wrap_key(
            &kek_pw,
            &nonce_pw,
            &master[..],
            b"dimvault/password-slot/v1",
        )?;

        let recovery = RecoveryKey::generate();
        let mut salt_rec = [0u8; SALT_LEN];
        let mut nonce_rec = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut salt_rec);
        OsRng.fill_bytes(&mut nonce_rec);
        let kek_rec = derive_kek(recovery.as_bytes(), &salt_rec, DEFAULT_KDF)?;
        let wrapped_rec = wrap_key(
            &kek_rec,
            &nonce_rec,
            &master[..],
            b"dimvault/recovery-slot/v1",
        )?;

        let mut header = VaultHeader {
            version: 1,
            kdf: DEFAULT_KDF,
            salt_password: salt_pw,
            nonce_password: nonce_pw,
            wrapped_password: wrapped_pw,
            salt_recovery: salt_rec,
            nonce_recovery: nonce_rec,
            wrapped_recovery: wrapped_rec,
            created_at_unix: unix_now(),
            header_mac: [0u8; 32],
        };
        header.header_mac = header.compute_mac(&master)?;

        {
            let mut f = OpenOptions::new().create_new(true).write(true).open(path)?;
            f.write_all(&header.to_bytes())?;
            f.flush()?;
            f.sync_all()?;
        }
        if let Some(parent) = path.parent() {
            sync_dir(parent)?;
        }

        Ok((
            Self {
                path: path.to_path_buf(),
            },
            recovery,
            UnlockedVault {
                master_key: master,
                header,
            },
        ))
    }

    /// Parola ile kasa açar. Yanlış parola ile bozuk başlık aynı hatayı verir;
    /// ayrım sızdırılmaz.
    pub fn open(path: &Path, password: &str) -> Result<UnlockedVault, VaultError> {
        let header = Self::read_header(path)?;
        let kek = derive_kek(password.as_bytes(), &header.salt_password, header.kdf)?;
        let master = unwrap_key(
            &kek,
            &header.nonce_password,
            &header.wrapped_password,
            b"dimvault/password-slot/v1",
        )?;
        // Doğrulama başarısızsa `Zeroizing` anahtarı sıfırlayarak düşürür.
        let master = Zeroizing::new(master);
        header.verify_mac(&master)?;
        Ok(UnlockedVault {
            master_key: master,
            header,
        })
    }

    /// Kurtarma anahtarı ile açma. Kurtarma anahtarı parola değişse de geçerlidir.
    pub fn open_with_recovery(path: &Path, recovery: &str) -> Result<UnlockedVault, VaultError> {
        let header = Self::read_header(path)?;
        let normalized = normalize_recovery_key(recovery).ok_or(VaultError::BadRecoveryKey)?;
        let kek = derive_kek(normalized.as_bytes(), &header.salt_recovery, header.kdf)?;
        let master = unwrap_key(
            &kek,
            &header.nonce_recovery,
            &header.wrapped_recovery,
            b"dimvault/recovery-slot/v1",
        )?;
        let master = Zeroizing::new(master);
        header.verify_mac(&master)?;
        Ok(UnlockedVault {
            master_key: master,
            header,
        })
    }

    fn read_header(path: &Path) -> Result<VaultHeader, VaultError> {
        let mut f = File::open(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                VaultError::NotFound(path.to_path_buf())
            } else {
                VaultError::Io(e)
            }
        })?;
        let mut buf = [0u8; HEADER_LEN];
        f.read_exact(&mut buf).map_err(|_| VaultError::BadHeader)?;
        VaultHeader::from_bytes(&buf)
    }
}

fn derive_kek(
    secret: &[u8],
    salt: &[u8],
    params: KdfParams,
) -> Result<[u8; MASTER_LEN], VaultError> {
    let p = Params::new(
        params.m_cost_kib,
        params.t_cost,
        params.p_cost,
        Some(MASTER_LEN),
    )
    .map_err(|_| VaultError::BadHeader)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, p);
    let mut out = [0u8; MASTER_LEN];
    argon
        .hash_password_into(secret, salt, &mut out)
        .map_err(|_| VaultError::Crypto)?;
    Ok(out)
}

fn wrap_key(
    kek: &[u8; 32],
    nonce: &[u8; NONCE_LEN],
    key: &[u8],
    aad: &[u8],
) -> Result<[u8; MASTER_LEN + TAG_LEN], VaultError> {
    let cipher = Aes256Gcm::new_from_slice(kek).map_err(|_| VaultError::Crypto)?;
    let ct = cipher
        .encrypt(Nonce::from_slice(nonce), Payload { msg: key, aad })
        .map_err(|_| VaultError::Crypto)?;
    let mut out = [0u8; MASTER_LEN + TAG_LEN];
    if ct.len() != out.len() {
        return Err(VaultError::Crypto);
    }
    out.copy_from_slice(&ct);
    Ok(out)
}

fn unwrap_key(
    kek: &[u8; 32],
    nonce: &[u8; NONCE_LEN],
    wrapped: &[u8; MASTER_LEN + TAG_LEN],
    aad: &[u8],
) -> Result<[u8; MASTER_LEN], VaultError> {
    let cipher = Aes256Gcm::new_from_slice(kek).map_err(|_| VaultError::Crypto)?;
    let pt = cipher
        .decrypt(Nonce::from_slice(nonce), Payload { msg: wrapped, aad })
        .map_err(|_| VaultError::BadCredentials)?;
    let mut out = [0u8; MASTER_LEN];
    if pt.len() != out.len() {
        return Err(VaultError::BadCredentials);
    }
    out.copy_from_slice(&pt);
    Ok(out)
}

/// Blob ve başlık anahtarlarını ana anahtardan ayırır (HKDF-SHA256).
pub(crate) fn derive_subkey(
    master: &[u8; 32],
    salt: &[u8],
    info: &[u8],
) -> Result<[u8; 32], VaultError> {
    let hk = Hkdf::<Sha256>::new(Some(salt), master);
    let mut out = [0u8; 32];
    hk.expand(info, &mut out).map_err(|_| VaultError::Crypto)?;
    Ok(out)
}

fn random_storage_id() -> String {
    let mut buf = [0u8; 16];
    OsRng.fill_bytes(&mut buf);
    data_encoding::HEXLOWER.encode(&buf)
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// Dizin girdisini diske senkronlar (rename sonrası dayanıklılık).
fn sync_dir(dir: &Path) -> Result<(), VaultError> {
    #[cfg(unix)]
    {
        let f = File::open(dir)?;
        f.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
    }
    Ok(())
}

/// Sabit zamanlı karşılaştırma; MAC doğrulamasında kullanılır.
pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    use subtle::ConstantTimeEq;
    a.len() == b.len() && bool::from(a.ct_eq(b))
}
