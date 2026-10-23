use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use zeroize::Zeroize;

use crate::{constant_time_eq, derive_subkey, VaultError, HEADER_LEN, MASTER_LEN};

const MAGIC: &[u8; 8] = b"DIMVLT01";
const SLOT_LEN: usize = MASTER_LEN + 16;
const MAC_INFO: &[u8] = b"dimvault/header-mac/v1";

type HmacSha256 = Hmac<Sha256>;

/// KDF maliyeti başlıkta saklanır; böylece eski kasa yeni sürümde de açılır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

/// Kasa başlığı. Sabit boyutludur ve doğrudan diske yazılır.
#[derive(Clone)]
pub struct VaultHeader {
    pub version: u8,
    pub kdf: KdfParams,
    pub salt_password: [u8; 16],
    pub nonce_password: [u8; 12],
    pub wrapped_password: [u8; SLOT_LEN],
    pub salt_recovery: [u8; 16],
    pub nonce_recovery: [u8; 12],
    pub wrapped_recovery: [u8; SLOT_LEN],
    pub created_at_unix: i64,
    pub header_mac: [u8; 32],
}

impl std::fmt::Debug for VaultHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VaultHeader")
            .field("version", &self.version)
            .field("kdf", &self.kdf)
            .field("created_at_unix", &self.created_at_unix)
            .field("header_mac", &"<32 bytes>")
            .finish()
    }
}

// 8 + 1 + 1 + 4*3 + 16 + 12 + 48 + 16 + 12 + 48 + 8 + 32
pub const HEADER_LEN: usize = 8 + 2 + 12 + 16 + 12 + 48 + 16 + 12 + 48 + 8 + 32;

impl VaultHeader {
    pub fn to_bytes(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        let mut o = 0;
        out[o..o + 8].copy_from_slice(MAGIC);
        o += 8;
        out[o] = self.version;
        o += 1;
        out[o] = 1; // kdf_alg = argon2id
        o += 1;
        out[o..o + 4].copy_from_slice(&self.kdf.m_cost_kib.to_le_bytes());
        o += 4;
        out[o..o + 4].copy_from_slice(&self.kdf.t_cost.to_le_bytes());
        o += 4;
        out[o..o + 4].copy_from_slice(&self.kdf.p_cost.to_le_bytes());
        o += 4;
        out[o..o + 16].copy_from_slice(&self.salt_password);
        o += 16;
        out[o..o + 12].copy_from_slice(&self.nonce_password);
        o += 12;
        out[o..o + SLOT_LEN].copy_from_slice(&self.wrapped_password);
        o += SLOT_LEN;
        out[o..o + 16].copy_from_slice(&self.salt_recovery);
        o += 16;
        out[o..o + 12].copy_from_slice(&self.nonce_recovery);
        o += 12;
        out[o..o + SLOT_LEN].copy_from_slice(&self.wrapped_recovery);
        o += SLOT_LEN;
        out[o..o + 8].copy_from_slice(&self.created_at_unix.to_le_bytes());
        o += 8;
        out[o..o + 32].copy_from_slice(&self.header_mac);
        o += 32;
        debug_assert_eq!(o, HEADER_LEN);
        out
    }

    pub fn from_bytes(buf: &[u8]) -> Result<Self, VaultError> {
        if buf.len() != HEADER_LEN || &buf[0..8] != MAGIC {
            return Err(VaultError::BadHeader);
        }
        let mut o = 8;
        let version = buf[o];
        o += 1;
        let kdf_alg = buf[o];
        o += 1;
        if version != 1 || kdf_alg != 1 {
            return Err(VaultError::BadHeader);
        }
        let m_cost_kib = read_u32(buf, &mut o)?;
        let t_cost = read_u32(buf, &mut o)?;
        let p_cost = read_u32(buf, &mut o)?;
        let mut salt_password = [0u8; 16];
        salt_password.copy_from_slice(&buf[o..o + 16]);
        o += 16;
        let mut nonce_password = [0u8; 12];
        nonce_password.copy_from_slice(&buf[o..o + 12]);
        o += 12;
        let mut wrapped_password = [0u8; SLOT_LEN];
        wrapped_password.copy_from_slice(&buf[o..o + SLOT_LEN]);
        o += SLOT_LEN;
        let mut salt_recovery = [0u8; 16];
        salt_recovery.copy_from_slice(&buf[o..o + 16]);
        o += 16;
        let mut nonce_recovery = [0u8; 12];
        nonce_recovery.copy_from_slice(&buf[o..o + 12]);
        o += 12;
        let mut wrapped_recovery = [0u8; SLOT_LEN];
        wrapped_recovery.copy_from_slice(&buf[o..o + SLOT_LEN]);
        o += SLOT_LEN;
        let created_at_unix = i64::from_le_bytes(
            buf[o..o + 8]
                .try_into()
                .map_err(|_| VaultError::BadHeader)?,
        );
        o += 8;
        let mut header_mac = [0u8; 32];
        header_mac.copy_from_slice(&buf[o..o + 32]);

        // KDF sınırları: bozuk başlıkla bellek tükenmesi engellenir.
        if !(8_192..=1_048_576).contains(&m_cost_kib)
            || !(1..=10).contains(&t_cost)
            || !(1..=8).contains(&p_cost)
        {
            return Err(VaultError::BadHeader);
        }

        Ok(Self {
            version,
            kdf: KdfParams {
                m_cost_kib,
                t_cost,
                p_cost,
            },
            salt_password,
            nonce_password,
            wrapped_password,
            salt_recovery,
            nonce_recovery,
            wrapped_recovery,
            created_at_unix,
            header_mac,
        })
    }

    fn mac_input(&self) -> [u8; HEADER_LEN] {
        let mut copy = self.clone();
        copy.header_mac = [0u8; 32];
        let bytes = copy.to_bytes();
        copy.header_mac.zeroize();
        bytes
    }

    pub fn compute_mac(&self, master: &[u8; 32]) -> Result<[u8; 32], VaultError> {
        let key = derive_subkey(master, &self.salt_password, MAC_INFO)?;
        let mut mac = HmacSha256::new_from_slice(&key).map_err(|_| VaultError::Crypto)?;
        mac.update(&self.mac_input());
        Ok(mac.finalize().into_bytes().into())
    }

    pub fn verify_mac(&self, master: &[u8; 32]) -> Result<(), VaultError> {
        let expected = self.compute_mac(master)?;
        if constant_time_eq(&expected, &self.header_mac) {
            Ok(())
        } else {
            Err(VaultError::BadCredentials)
        }
    }

    /// Parola değişiminde yalnız parola yuvası yeniden sarılır; ana anahtar korunur.
    pub fn rewrap_password_slot(
        &mut self,
        master: &[u8; 32],
        kek: &[u8; 32],
    ) -> Result<(), VaultError> {
        let cipher = Aes256Gcm::new_from_slice(kek).map_err(|_| VaultError::Crypto)?;
        let wrapped = cipher
            .encrypt(
                Nonce::from_slice(&self.nonce_password),
                Payload {
                    msg: master,
                    aad: b"dimvault/password-slot/v1",
                },
            )
            .map_err(|_| VaultError::Crypto)?;
        if wrapped.len() != SLOT_LEN {
            return Err(VaultError::Crypto);
        }
        self.wrapped_password.copy_from_slice(&wrapped);
        self.header_mac = self.compute_mac(master)?;
        Ok(())
    }
}

fn read_u32(buf: &[u8], o: &mut usize) -> Result<u32, VaultError> {
    let v = u32::from_le_bytes(
        buf[*o..*o + 4]
            .try_into()
            .map_err(|_| VaultError::BadHeader)?,
    );
    *o += 4;
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> VaultHeader {
        VaultHeader {
            version: 1,
            kdf: KdfParams {
                m_cost_kib: 65_536,
                t_cost: 3,
                p_cost: 1,
            },
            salt_password: [1u8; 16],
            nonce_password: [2u8; 12],
            wrapped_password: [3u8; SLOT_LEN],
            salt_recovery: [4u8; 16],
            nonce_recovery: [5u8; 12],
            wrapped_recovery: [6u8; SLOT_LEN],
            created_at_unix: 1_700_000_000,
            header_mac: [7u8; 32],
        }
    }

    #[test]
    fn header_roundtrip_is_stable() {
        let h = header();
        let bytes = h.to_bytes();
        let parsed = VaultHeader::from_bytes(&bytes).expect("geçerli başlık");
        assert_eq!(parsed.to_bytes(), bytes);
        assert_eq!(parsed.kdf, h.kdf);
    }

    #[test]
    fn bad_magic_rejected() {
        let mut bytes = header().to_bytes();
        bytes[0] = b'X';
        assert_eq!(
            VaultHeader::from_bytes(&bytes).unwrap_err().code(),
            "E_VAULT_HEADER"
        );
    }

    #[test]
    fn absurd_kdf_cost_rejected() {
        let mut h = header();
        h.kdf.m_cost_kib = 8_000_000;
        let bytes = h.to_bytes();
        assert!(VaultHeader::from_bytes(&bytes).is_err());
    }

    #[test]
    fn mac_detects_header_tamper() {
        let master = [9u8; 32];
        let mut h = header();
        h.header_mac = h.compute_mac(&master).expect("mac");
        h.verify_mac(&master).expect("doğru");
        h.created_at_unix += 1;
        assert_eq!(
            h.verify_mac(&master).unwrap_err().code(),
            "E_VAULT_CREDENTIALS"
        );
    }
}
