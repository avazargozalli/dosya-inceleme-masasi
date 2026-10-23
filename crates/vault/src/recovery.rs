use rand::rngs::OsRng;
use rand::RngCore;
use zeroize::Zeroizing;

use crate::VaultError;

const KEY_LEN: usize = 32;

/// Kurtarma anahtarı: 32 rastgele bayt, insan yazımı için Crockford benzeri
/// base32 (tireli gruplar). Parola unutulsa da kasa açılabilir.
pub struct RecoveryKey(Zeroizing<[u8; KEY_LEN]>);

impl std::fmt::Debug for RecoveryKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryKey(<redacted>)")
    }
}

impl RecoveryKey {
    #[must_use]
    pub fn generate() -> Self {
        let mut bytes = Zeroizing::new([0u8; KEY_LEN]);
        OsRng.fill_bytes(bytes.as_mut());
        Self(bytes)
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    /// Kullanıcıya bir kez gösterilecek biçim: `XXXX-XXXX-...` (8 grup).
    #[must_use]
    pub fn to_display_string(&self) -> String {
        format_recovery_key(&self.0)
    }
}

pub fn format_recovery_key(bytes: &[u8]) -> String {
    let encoded = data_encoding::BASE32_NOPAD.encode(bytes).to_ascii_uppercase();
    encoded
        .as_bytes()
        .chunks(4)
        .map(|c| std::str::from_utf8(c).unwrap_or_default().to_string())
        .collect::<Vec<_>>()
        .join("-")
}

/// Kullanıcı girişini normalize eder: küçük harf, boşluk/tire yok, geçersiz
/// uzunluk veya alfabe `None`. Hata mesajında anahtar parçası görünmez.
#[must_use]
pub fn normalize_recovery_key(input: &str) -> Option<String> {
    let cleaned: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if cleaned.len() != 52 {
        return None;
    }
    let decoded = data_encoding::BASE32_NOPAD.decode(cleaned.as_bytes()).ok()?;
    if decoded.len() != KEY_LEN {
        return None;
    }
    Some(cleaned)
}

/// Kurtarma anahtarının geçerliliğini doğrular (kasa açmadan biçim kontrolü).
pub fn parse_recovery_key(input: &str) -> Result<String, VaultError> {
    normalize_recovery_key(input).ok_or(VaultError::BadRecoveryKey)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_roundtrip() {
        let key = RecoveryKey::generate();
        let shown = key.to_display_string();
        let normalized = normalize_recovery_key(&shown).expect("geçerli");
        assert_eq!(format_recovery_key(key.as_bytes()), shown);
        assert_eq!(normalized.len(), 52);
    }

    #[test]
    fn lowercase_and_spaces_accepted() {
        let key = RecoveryKey::generate();
        let shown = key.to_display_string().to_lowercase().replace('-', " ");
        assert!(normalize_recovery_key(&shown).is_some());
    }

    #[test]
    fn short_or_garbage_rejected() {
        assert!(normalize_recovery_key("ABC-DEF").is_none());
        assert!(normalize_recovery_key(&"!".repeat(52)).is_none());
    }
}
