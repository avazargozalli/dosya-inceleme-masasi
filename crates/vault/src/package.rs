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

/// Paket parolasının **alt sınırı** (karakter, UTF-16 birimi değil).
///
/// Arayüz aynı sayıyı kendi kapısında kullanır; iki taraf da `chars().count()`
/// ile sayar. Sınırın tek kaynağı burasıdır: arayüz `trim()` uygulasa ya da
/// `.length` (UTF-16) saysa iki kapı ayrışır ve kısa parola sessizce açık
/// metne düşer.
pub const MIN_PASSWORD_CHARS: usize = 10;

/// Parola sözleşmesini denetler.
///
/// `seal` de bu kapıdan geçer: kısa parola **açık metne düşmez**, çağrı hata
/// alır. Boş dize "parola verilmedi" demek değildir — parola verilmediği
/// `None` ile anlatılır; boş dize kısa paroladır ve reddedilir.
pub fn check_password(password: &str) -> Result<(), VaultError> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(VaultError::WeakPassword);
    }
    Ok(())
}

/// `plain` baytlarını parola ile şifreler. Çıktı: magic + tuz + nonce + ciphertext.
pub fn seal(plain: &[u8], password: &str) -> Result<Vec<u8>, VaultError> {
    check_password(password)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Sınır **karakter** sayısıdır: JS'in `.length`i (UTF-16 birimi) değil.
    /// 5 emoji tarayıcıda 10 birim görünür ama çekirdek 5 karakter sayar —
    /// eski kod bu parolayı `filter` ile düşürüp paketi sessizce **açık metin**
    /// yazıyordu.
    #[test]
    fn password_limit_counts_chars_not_utf16_units() {
        assert!(matches!(
            check_password("kisa"),
            Err(VaultError::WeakPassword)
        ));
        assert!(matches!(
            check_password("123456789"),
            Err(VaultError::WeakPassword)
        ));
        assert!(check_password("1234567890").is_ok());
        assert!(matches!(
            check_password("🙂🙂🙂🙂🙂"),
            Err(VaultError::WeakPassword)
        ));
        assert!(check_password("🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂").is_ok());
        // Boş dize "parola verilmedi" değildir: açıkça verilmiş kısa paroladır.
        assert!(matches!(check_password(""), Err(VaultError::WeakPassword)));
        // Kırpma yok: çekirdek ham dizeyi sayar (arayüz de kırpmıyor).
        assert!(check_password(" 12345678 ").is_ok());
        assert!(matches!(
            check_password(" 1234567 "),
            Err(VaultError::WeakPassword)
        ));
    }

    /// `seal` kısa parolada **bayt üretmez**; geçerli parolada üretilen paket
    /// aynı parolayla açılır, yanlış parolayla açılmaz.
    #[test]
    fn seal_refuses_short_password_and_roundtrips() {
        assert!(matches!(
            seal(b"govde", "kisa"),
            Err(VaultError::WeakPassword)
        ));
        let sealed = seal(b"govde", "0123456789").expect("paket üretilmeli");
        assert_eq!(&sealed[..8], PACKAGE_MAGIC);
        assert_eq!(open(&sealed, "0123456789").expect("açılmalı"), b"govde");
        assert!(matches!(
            open(&sealed, "yanlis-parola"),
            Err(VaultError::BadCredentials)
        ));
    }
}
