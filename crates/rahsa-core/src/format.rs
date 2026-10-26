//! Binary layout of `vault.rahsa` (all integers little-endian):
//!
//! ```text
//! off  len  field
//!   0    5  magic "RAHSA"
//!   5    2  format version (u16) = 1
//!   7    1  KDF id (1 = Argon2id v1.3)
//!   8    4  m_cost KiB (u32)
//!  12    4  t_cost (u32)
//!  16    4  p_cost (u32)
//!  20   16  salt
//!  36   24  wrap nonce
//!  60   48  wrapped vault key (32-byte key + 16-byte Poly1305 tag)
//! 108   24  payload nonce (fresh on every save)
//! 132    …  payload ciphertext (XChaCha20-Poly1305 of the JSON vault)
//! ```
//!
//! The wrapped key is authenticated with bytes `0..36` (magic, version, KDF
//! params, salt) as associated data. The payload is authenticated with the full
//! 132-byte header, so any change to any header byte makes decryption fail.

use crate::crypto::{KdfParams, KEY_LEN, NONCE_LEN, SALT_LEN, TAG_LEN};
use crate::{Error, Result};

pub const MAGIC: &[u8; 5] = b"RAHSA";
pub const VERSION: u16 = 1;
pub const KDF_ARGON2ID: u8 = 1;
pub const KDF_SECTION_LEN: usize = 36;
pub const WRAPPED_KEY_LEN: usize = KEY_LEN + TAG_LEN;
pub const HEADER_LEN: usize = KDF_SECTION_LEN + NONCE_LEN + WRAPPED_KEY_LEN + NONCE_LEN;
/// Refuse to even try parsing anything larger (≈ tens of thousands of accounts).
pub const MAX_FILE_LEN: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub kdf: KdfParams,
    pub salt: [u8; SALT_LEN],
    pub wrap_nonce: [u8; NONCE_LEN],
    pub wrapped_key: [u8; WRAPPED_KEY_LEN],
    pub payload_nonce: [u8; NONCE_LEN],
}

impl Header {
    pub fn kdf_section(&self) -> [u8; KDF_SECTION_LEN] {
        let mut b = [0u8; KDF_SECTION_LEN];
        b[0..5].copy_from_slice(MAGIC);
        b[5..7].copy_from_slice(&VERSION.to_le_bytes());
        b[7] = KDF_ARGON2ID;
        b[8..12].copy_from_slice(&self.kdf.m_cost_kib.to_le_bytes());
        b[12..16].copy_from_slice(&self.kdf.t_cost.to_le_bytes());
        b[16..20].copy_from_slice(&self.kdf.p_cost.to_le_bytes());
        b[20..36].copy_from_slice(&self.salt);
        b
    }

    pub fn to_bytes(&self) -> [u8; HEADER_LEN] {
        let mut b = [0u8; HEADER_LEN];
        b[..KDF_SECTION_LEN].copy_from_slice(&self.kdf_section());
        let mut o = KDF_SECTION_LEN;
        b[o..o + NONCE_LEN].copy_from_slice(&self.wrap_nonce);
        o += NONCE_LEN;
        b[o..o + WRAPPED_KEY_LEN].copy_from_slice(&self.wrapped_key);
        o += WRAPPED_KEY_LEN;
        b[o..o + NONCE_LEN].copy_from_slice(&self.payload_nonce);
        b
    }

    /// Parse a whole file into header + ciphertext. Never panics on hostile input.
    pub fn parse(file: &[u8]) -> Result<(Header, &[u8])> {
        if file.len() > MAX_FILE_LEN {
            return Err(Error::Corrupt("file terlalu besar"));
        }
        if file.len() < 7 || &file[0..5] != MAGIC {
            return Err(Error::BadMagic);
        }
        let version = u16::from_le_bytes([file[5], file[6]]);
        if version != VERSION {
            return Err(Error::UnsupportedVersion(version));
        }
        if file.len() < HEADER_LEN + TAG_LEN {
            return Err(Error::Corrupt("file terpotong"));
        }
        if file[7] != KDF_ARGON2ID {
            return Err(Error::BadKdfParams);
        }
        let u32_at = |o: usize| u32::from_le_bytes(file[o..o + 4].try_into().unwrap());
        let kdf = KdfParams { m_cost_kib: u32_at(8), t_cost: u32_at(12), p_cost: u32_at(16) };
        kdf.validate()?;
        let arr = |o: usize, n: usize| &file[o..o + n];
        let mut o = 20;
        let salt = arr(o, SALT_LEN).try_into().unwrap();
        o += SALT_LEN;
        let wrap_nonce = arr(o, NONCE_LEN).try_into().unwrap();
        o += NONCE_LEN;
        let wrapped_key = arr(o, WRAPPED_KEY_LEN).try_into().unwrap();
        o += WRAPPED_KEY_LEN;
        let payload_nonce = arr(o, NONCE_LEN).try_into().unwrap();
        o += NONCE_LEN;
        debug_assert_eq!(o, HEADER_LEN);
        Ok((Header { kdf, salt, wrap_nonce, wrapped_key, payload_nonce }, &file[HEADER_LEN..]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_round_trip() {
        let h = Header {
            kdf: KdfParams::DEFAULT,
            salt: [1; SALT_LEN],
            wrap_nonce: [2; NONCE_LEN],
            wrapped_key: [3; WRAPPED_KEY_LEN],
            payload_nonce: [4; NONCE_LEN],
        };
        let mut file = h.to_bytes().to_vec();
        file.extend_from_slice(&[9; 40]);
        let (p, ct) = Header::parse(&file).unwrap();
        assert_eq!(p, h);
        assert_eq!(ct.len(), 40);
    }

    #[test]
    fn garbage_never_panics() {
        use rand::{Rng, RngCore};
        let mut rng = rand::thread_rng();
        for len in 0..400 {
            let mut buf = vec![0u8; len];
            rng.fill_bytes(&mut buf);
            if len >= 7 && rng.gen_bool(0.5) {
                buf[..5].copy_from_slice(MAGIC);
                buf[5..7].copy_from_slice(&VERSION.to_le_bytes());
            }
            let _ = Header::parse(&buf);
        }
    }
}
