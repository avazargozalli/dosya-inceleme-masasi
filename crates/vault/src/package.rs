//! Şifreli devir paketi.
//!
//! Paket, kasadan **bağımsız** bir parola ile korunur: alıcıya kasa anahtarı
//! vermeden tek dosya taşınır. Parola Argon2id ile anahtara çevrilir, içerik
//! AES-256-GCM ile şifrelenir. Başlık açık metindir (sürüm, tuz, nonce); içerik
//! ve dosya adları şifrelidir.

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use rand::rngs::OsRng;
use rand::RngCore;
use zeroize::Zeroizing;

use crate::VaultError;
use crate::{derive_kek, DEFAULT_KDF, NONCE_LEN, SALT_LEN};

pub const PACKAGE_MAGIC: &[u8; 8] = b"DIMPAK1\0";
const AAD: &[u8] = b"dimvault/package/v1";

/// `plain` baytlarını parola ile şifreler. Çıktı: magic + tuz + nonce + ciphertext.
pub fn seal(plain: &[u8], password: &str) -> Result<Vec<u8>, VaultError> {
    if password.chars().count() < 10 {
        return Err(VaultError::WeakPassword);
    }
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);

    let kek = Zeroizing::new(derive_kek(password.as_bytes(), &salt, DEFAULT_KDF)?);
    let cipher = Aes256Gcm::new_from_slice(&kek[..]).map_err(|_| VaultError::Crypto)?;
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plain,
                aad: AAD,
            },
        )
        .map_err(|_| VaultError::Crypto)?;

    let mut out = Vec::with_capacity(8 + SALT_LEN + NONCE_LEN + ciphertext.len());
    out.extend_from_slice(PACKAGE_MAGIC);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Şifreli paketi açar. Yanlış parola ile bozuk içerik aynı hatayı verir.
pub fn open(data: &[u8], password: &str) -> Result<Vec<u8>, VaultError> {
    let header = 8 + SALT_LEN + NONCE_LEN;
    if data.len() <= header || &data[..8] != PACKAGE_MAGIC {
        return Err(VaultError::BadHeader);
    }
    let salt = &data[8..8 + SALT_LEN];
    let nonce = &data[8 + SALT_LEN..header];
    let ciphertext = &data[header..];

    let kek = Zeroizing::new(derive_kek(password.as_bytes(), salt, DEFAULT_KDF)?);
    let cipher = Aes256Gcm::new_from_slice(&kek[..]).map_err(|_| VaultError::Crypto)?;
    cipher
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: AAD,
            },
        )
        .map_err(|_| VaultError::BadCredentials)
}
