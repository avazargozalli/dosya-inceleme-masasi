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
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
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

/// `seal_dir` çıktısı.
#[derive(Debug, Clone)]
pub struct SealedDir {
    /// Üretilen şifreli paket dosyası (`.dimpak`).
    pub dimpak: PathBuf,
    /// Açık metin klasörü **silinemediyse** yolu; `None` = silindi.
    ///
    /// Silinememesi gizlenmez: çağıran bunu kullanıcıya "açık metin diskte
    /// kaldı" diye bildirmek zorundadır. Sessiz kalıntı, "şifreli" sanılan bir
    /// açık metin klasörü bırakır.
    pub leftover_dir: Option<PathBuf>,
}

/// Açık metin klasörünü zip'e koyup tek şifreli `.dimpak` dosyasına çevirir.
///
/// **Sözleşme:** parola ilk adımda doğrulanır (kısa parolada **hiçbir şey
/// yazılmaz**). Zip, şifreleme ya da yazma düşerse açık metin klasörü ve
/// geçici arşiv **silinir**; silinemezse `VaultError::PlaintextLeft` yol
/// bildirir — yarım iş kalıntısı sessizce bırakılmaz.
///
/// Şifreleme düştüğünde klasörün silinmesi bilinçlidir: kullanıcı "paket
/// oluşturulamadı" hatasını görürken hedefte **açık metin** bir klasörün
/// kalması, parola verdiği hâlde şifresiz veri bırakmak olurdu. Paket
/// yeniden üretilebilir bir çıktıdır; açık metin kalıntısı değildir.
pub fn seal_dir(dir: &Path, password: &str) -> Result<SealedDir, VaultError> {
    check_password(password)?;
    if !dir.is_dir() {
        return Err(VaultError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "paketlenecek klasör yok",
        )));
    }
    let archive = dir.with_extension("zip");
    let target = dir.with_extension("dimpak");

    let built: Result<(), VaultError> = (|| {
        zip_dir(dir, &archive)?;
        let zip_bytes = fs::read(&archive)?;
        let sealed = seal(&zip_bytes, password)?;
        // Paket de **geçici ada** yazılıp yerleştirilir: yarım `.dimpak` hedef
        // adda görünmez. Hedef adda bir **klasör** varsa `rename` düşer —
        // kullanıcının klasörü silinmez, çağrı hata verir. (Var olan aynı adlı
        // *dosya* POSIX `rename` ile değişir; eski `fs::write` da eziyordu.)
        let staging = temp_sibling(&target);
        fs::write(&staging, &sealed)?;
        fs::rename(&staging, &target)?;
        Ok(())
    })();
    // Geçici arşiv ve geçici paket açık metin/yarım iş taşır: her iki yolda silinir.
    let _ = fs::remove_file(&archive);
    let _ = fs::remove_file(temp_sibling(&target));
    if let Err(error) = built {
        // Hata yolunda açık metin bırakılmaz.
        let plaintext_gone = remove_path(dir);
        return Err(if plaintext_gone {
            error
        } else {
            VaultError::PlaintextLeft {
                path: dir.to_path_buf(),
                cause: error.to_string(),
            }
        });
    }
    Ok(SealedDir {
        dimpak: target,
        leftover_dir: (!remove_path(dir)).then(|| dir.to_path_buf()),
    })
}

/// Klasör ağacını tek dosyalık zip arşivine yazar.
///
/// Arşiv **geçici ada** yazılır, bitince yerleştirilir: yarıda kalan bir arşiv
/// geçerli paket/yedek sanılmasın (F15-T06). Hem devir paketi hem tek dosyalık
/// kasa yedeği aynı yardımcıyı kullanır.
pub fn zip_dir(src: &Path, dest: &Path) -> Result<(), VaultError> {
    let io_err = |err: zip::result::ZipError| std::io::Error::other(err.to_string());
    let temp = temp_sibling(dest);
    let written: std::io::Result<()> = (|| {
        let mut writer = zip::ZipWriter::new(fs::File::create(&temp)?);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let mut stack = vec![src.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir)? {
                let path = entry?.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let name = path
                    .strip_prefix(src)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                writer.start_file(name, options).map_err(io_err)?;
                writer.write_all(&fs::read(&path)?)?;
            }
        }
        writer.finish().map(|_| ()).map_err(io_err)?;
        Ok(())
    })();
    if let Err(error) = written {
        let _ = fs::remove_file(&temp);
        return Err(VaultError::Io(error));
    }
    fs::rename(&temp, dest)?;
    Ok(())
}

/// Hedefin yanında geçici ad (aynı dosya sistemi → `rename` atomik).
fn temp_sibling(dest: &Path) -> PathBuf {
    let name = dest
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "package".to_string());
    dest.with_file_name(format!(".{name}.tmp"))
}

/// Yolu (dosya ya da dizin) siler; **artık yok mu** diye döner.
///
/// `false` = hâlâ duruyor (izin yok, meşgul). Sessizce `true` demez: açık
/// metin kalıntısını "temizlendi" saymak bu modülün kapatmaya çalıştığı hata.
fn remove_path(path: &Path) -> bool {
    if path.is_dir() {
        let _ = fs::remove_dir_all(path);
    } else {
        let _ = fs::remove_file(path);
    }
    !path.exists()
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
