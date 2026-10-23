use std::io::{Read, Seek, SeekFrom, Write};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::rngs::OsRng;
use rand::RngCore;

use crate::{derive_subkey, VaultError};

const MAGIC: &[u8; 8] = b"DIMBLB01";
/// 64 KiB parça: bellek kullanımı sabit, büyük dosya akan biçimde işlenir.
pub const BLOB_CHUNK_SIZE: usize = 65_536;
pub const BLOB_HEADER_LEN: usize = 8 + 1 + 4 + 8 + 8 + 4;
const TAG_LEN: usize = 16;

#[derive(Debug, Clone, Copy)]
struct BlobHeader {
    chunk_size: u32,
    nonce_prefix: [u8; 8],
    plain_len: u64,
    chunk_count: u32,
}

impl BlobHeader {
    fn to_bytes(self) -> [u8; BLOB_HEADER_LEN] {
        let mut out = [0u8; BLOB_HEADER_LEN];
        out[0..8].copy_from_slice(MAGIC);
        out[8] = 1;
        out[9..13].copy_from_slice(&self.chunk_size.to_le_bytes());
        out[13..21].copy_from_slice(&self.nonce_prefix);
        out[21..29].copy_from_slice(&self.plain_len.to_le_bytes());
        out[29..33].copy_from_slice(&self.chunk_count.to_le_bytes());
        out
    }

    fn from_bytes(buf: &[u8; BLOB_HEADER_LEN]) -> Result<Self, VaultError> {
        if &buf[0..8] != MAGIC || buf[8] != 1 {
            return Err(VaultError::Corrupt("blob_header"));
        }
        let chunk_size = u32::from_le_bytes(buf[9..13].try_into().map_err(|_| VaultError::Corrupt("chunk_size"))?);
        if chunk_size == 0 || chunk_size > 4 * 1024 * 1024 {
            return Err(VaultError::Corrupt("chunk_size"));
        }
        let mut nonce_prefix = [0u8; 8];
        nonce_prefix.copy_from_slice(&buf[13..21]);
        let plain_len = u64::from_le_bytes(buf[21..29].try_into().map_err(|_| VaultError::Corrupt("plain_len"))?);
        let chunk_count = u32::from_le_bytes(buf[29..33].try_into().map_err(|_| VaultError::Corrupt("chunk_count"))?);
        Ok(Self { chunk_size, nonce_prefix, plain_len, chunk_count })
    }
}

fn parse_header<R: Read>(reader: &mut R) -> Result<BlobHeader, VaultError> {
    let mut buf = [0u8; BLOB_HEADER_LEN];
    reader.read_exact(&mut buf).map_err(|_| VaultError::Corrupt("short_header"))?;
    BlobHeader::from_bytes(&buf)
}

/// Şifreli bir dosyanın beklenen toplam boyutu (kesilme tespiti için).
#[must_use]
pub fn expected_blob_len(plain_len: u64, chunk_size: usize) -> u64 {
    let chunks = plain_len.div_ceil(chunk_size as u64);
    BLOB_HEADER_LEN as u64 + chunks * (chunk_size as u64 + TAG_LEN as u64)
}

/// Girdiyi parçalara bölerek şifreler. Her parça sabit boyutludur; son parça
/// sıfırla doldurulur ve gerçek uzunluk başlıkta taşınır. Böylece dosya boyutu
/// kesilme saldırısını açık eder.
pub fn encrypt_blob<W: Write>(plain: &[u8], writer: &mut W, master: &[u8; 32], aad: &[u8]) -> Result<(), VaultError> {
    let mut nonce_prefix = [0u8; 8];
    OsRng.fill_bytes(&mut nonce_prefix);
    let chunk_size = BLOB_CHUNK_SIZE;
    let plain_len = plain.len() as u64;
    let chunk_count_u64 = plain_len.div_ceil(chunk_size as u64);
    let chunk_count = u32::try_from(chunk_count_u64).map_err(|_| VaultError::Corrupt("too_large"))?;

    let header = BlobHeader { chunk_size: chunk_size as u32, nonce_prefix, plain_len, chunk_count };
    let header_bytes = header.to_bytes();
    writer.write_all(&header_bytes)?;

    let key = derive_subkey(master, &nonce_prefix, b"dimvault/blob-key/v1")?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| VaultError::Crypto)?;

    for index in 0..chunk_count {
        let start = index as usize * chunk_size;
        let end = usize::min(start + chunk_size, plain.len());
        let mut block = vec![0u8; chunk_size];
        block[..end - start].copy_from_slice(&plain[start..end]);

        let nonce = chunk_nonce(&nonce_prefix, index);
        let ct = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload { msg: &block, aad: &chunk_aad(aad, &header_bytes, index) },
            )
            .map_err(|_| VaultError::Crypto)?;
        if ct.len() != chunk_size + TAG_LEN {
            return Err(VaultError::Crypto);
        }
        writer.write_all(&ct)?;
    }
    writer.flush()?;
    Ok(())
}

/// Şifreli blob'u çözer. Süreç: başlık doğrula → beklenen boyut → parça AEAD.
/// Herhangi bir parça bozuksa hiçbir çıktı kullanılmaz (çağıran `Vec` alır).
pub fn decrypt_blob<R: Read + Seek>(reader: &mut R, master: &[u8; 32], aad: &[u8]) -> Result<Vec<u8>, VaultError> {
    let header = parse_header(reader)?;
    let chunk_size = header.chunk_size as usize;
    let expected = chunk_size
        .checked_add(TAG_LEN)
        .and_then(|per| (header.chunk_count as usize).checked_mul(per))
        .and_then(|total| total.checked_add(BLOB_HEADER_LEN))
        .ok_or(VaultError::Corrupt("size_overflow"))?;

    let actual = {
        let start = reader.stream_position()?;
        let end = reader.seek(SeekFrom::End(0))?;
        reader.seek(SeekFrom::Start(start))?;
        end
    };
    if actual != expected as u64 {
        return Err(VaultError::Corrupt(if actual < expected as u64 { "truncated" } else { "trailing_data" }));
    }
    if (header.plain_len as usize) > chunk_size.saturating_mul(header.chunk_count as usize) {
        return Err(VaultError::Corrupt("plain_len"));
    }

    let header_bytes = header.to_bytes();
    let key = derive_subkey(master, &header.nonce_prefix, b"dimvault/blob-key/v1")?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| VaultError::Crypto)?;

    let mut out = Vec::with_capacity(usize::min(header.plain_len as usize, 64 * 1024 * 1024));
    for index in 0..header.chunk_count {
        let mut buf = vec![0u8; chunk_size + TAG_LEN];
        reader.read_exact(&mut buf).map_err(|_| VaultError::Corrupt("short_chunk"))?;
        let nonce = chunk_nonce(&header.nonce_prefix, index);
        let pt = cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload { msg: &buf, aad: &chunk_aad(aad, &header_bytes, index) },
            )
            .map_err(|_| VaultError::Corrupt("aead_tag"))?;
        if pt.len() != chunk_size {
            return Err(VaultError::Corrupt("chunk_len"));
        }
        let take = usize::min(chunk_size, (header.plain_len as usize).saturating_sub(out.len()));
        out.extend_from_slice(&pt[..take]);
    }
    if out.len() != header.plain_len as usize {
        return Err(VaultError::Corrupt("plain_len"));
    }
    Ok(out)
}

fn chunk_nonce(prefix: &[u8; 8], index: u32) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[..8].copy_from_slice(prefix);
    nonce[8..].copy_from_slice(&index.to_be_bytes());
    nonce
}

fn chunk_aad(user: &[u8], header: &[u8], index: u32) -> Vec<u8> {
    let mut aad = Vec::with_capacity(user.len() + header.len() + 4);
    aad.extend_from_slice(user);
    aad.extend_from_slice(header);
    aad.extend_from_slice(&index.to_le_bytes());
    aad
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn roundtrip(data: &[u8], aad: &[u8]) {
        let key = [7u8; 32];
        let mut sealed = Vec::new();
        encrypt_blob(data, &mut sealed, &key, aad).expect("şifrele");
        assert_eq!(sealed.len() as u64, expected_blob_len(data.len() as u64, BLOB_CHUNK_SIZE));
        let opened = decrypt_blob(&mut Cursor::new(&sealed), &key, aad).expect("çöz");
        assert_eq!(opened, data);
    }

    #[test]
    fn empty_blob_roundtrip() {
        roundtrip(b"", b"case-1");
    }

    #[test]
    fn single_chunk_roundtrip() {
        roundtrip(b"kira sozlesmesi", b"case-1");
    }

    #[test]
    fn multi_chunk_roundtrip() {
        let data: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        roundtrip(&data, b"case-2");
    }

    #[test]
    fn wrong_master_key_rejected() {
        let mut sealed = Vec::new();
        encrypt_blob(b"gizli", &mut sealed, &[1u8; 32], b"a").expect("şifrele");
        let err = decrypt_blob(&mut Cursor::new(&sealed), &[2u8; 32], b"a").unwrap_err();
        assert_eq!(err.code(), "E_VAULT_CORRUPT");
    }

    #[test]
    fn different_aad_rejected() {
        let mut sealed = Vec::new();
        encrypt_blob(b"gizli", &mut sealed, &[1u8; 32], b"case-a").expect("şifrele");
        assert!(decrypt_blob(&mut Cursor::new(&sealed), &[1u8; 32], b"case-b").is_err());
    }

    #[test]
    fn truncation_detected() {
        let mut sealed = Vec::new();
        encrypt_blob(b"0123456789", &mut sealed, &[1u8; 32], b"a").expect("şifrele");
        sealed.truncate(sealed.len() - 1);
        let err = decrypt_blob(&mut Cursor::new(&sealed), &[1u8; 32], b"a").unwrap_err();
        assert!(err.to_string().contains("truncated"));
    }

    #[test]
    fn bit_flip_detected() {
        let mut sealed = Vec::new();
        encrypt_blob(b"0123456789", &mut sealed, &[1u8; 32], b"a").expect("şifrele");
        let last = sealed.len() - 1;
        sealed[last] ^= 0x01;
        assert!(decrypt_blob(&mut Cursor::new(&sealed), &[1u8; 32], b"a").is_err());
    }

    #[test]
    fn trailing_data_detected() {
        let mut sealed = Vec::new();
        encrypt_blob(b"0123456789", &mut sealed, &[1u8; 32], b"a").expect("şifrele");
        sealed.push(0);
        assert!(decrypt_blob(&mut Cursor::new(&sealed), &[1u8; 32], b"a").is_err());
    }
}
