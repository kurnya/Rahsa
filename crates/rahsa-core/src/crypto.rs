//! Thin wrappers around audited RustCrypto primitives. No custom algorithms.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::rngs::OsRng;
use rand::RngCore;
use zeroize::Zeroizing;

use crate::{Error, Result};

pub const KEY_LEN: usize = 32;
pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 24;
pub const TAG_LEN: usize = 16;

pub type Key = Zeroizing<[u8; KEY_LEN]>;

/// Argon2id parameters stored (unencrypted) in the vault header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl KdfParams {
    /// PRD §7: memory 256 MiB, 3 iterations, parallelism 4.
    pub const DEFAULT: KdfParams = KdfParams { m_cost_kib: 256 * 1024, t_cost: 3, p_cost: 4 };

    /// Bounds accepted when *reading* a file, so a malicious header cannot make
    /// us allocate unbounded memory or spin forever.
    pub fn validate(&self) -> Result<()> {
        let ok = (1..=16).contains(&self.p_cost)
            && (1..=10).contains(&self.t_cost)
            && self.m_cost_kib >= 8 * self.p_cost
            && self.m_cost_kib <= 1024 * 1024;
        if ok { Ok(()) } else { Err(Error::BadKdfParams) }
    }

    /// Cheap parameters for unit tests only.
    #[cfg(test)]
    pub const TEST: KdfParams = KdfParams { m_cost_kib: 1024, t_cost: 1, p_cost: 1 };
}

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    OsRng.fill_bytes(&mut b);
    b
}

pub fn random_key() -> Key {
    let mut k = Zeroizing::new([0u8; KEY_LEN]);
    OsRng.fill_bytes(k.as_mut());
    k
}

/// Derive the key-encryption key from the master password.
pub fn derive_kek(password: &str, salt: &[u8; SALT_LEN], params: KdfParams) -> Result<Key> {
    params.validate()?;
    let p = Params::new(params.m_cost_kib, params.t_cost, params.p_cost, Some(KEY_LEN))
        .map_err(|_| Error::BadKdfParams)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, p);
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    argon
        .hash_password_into(password.as_bytes(), salt, out.as_mut())
        .map_err(|_| Error::BadKdfParams)?;
    Ok(out)
}

pub fn seal(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN], plaintext: &[u8], aad: &[u8]) -> Vec<u8> {
    let cipher = XChaCha20Poly1305::new(key.into());
    cipher
        .encrypt(XNonce::from_slice(nonce), Payload { msg: plaintext, aad })
        .expect("XChaCha20-Poly1305 encryption cannot fail for in-memory buffers")
}

pub fn open(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    ciphertext: &[u8],
    aad: &[u8],
) -> Result<Zeroizing<Vec<u8>>> {
    let cipher = XChaCha20Poly1305::new(key.into());
    cipher
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ciphertext, aad })
        .map(Zeroizing::new)
        .map_err(|_| Error::Decrypt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argon2id_rfc9106_shape_is_deterministic() {
        let salt = [7u8; SALT_LEN];
        let a = derive_kek("correct horse battery", &salt, KdfParams::TEST).unwrap();
        let b = derive_kek("correct horse battery", &salt, KdfParams::TEST).unwrap();
        let c = derive_kek("correct horse batterz", &salt, KdfParams::TEST).unwrap();
        assert_eq!(*a, *b);
        assert_ne!(*a, *c);
    }

    /// RFC 9106 §5.3 Argon2id test vector (exercises the same library and
    /// algorithm/version configuration that `derive_kek` uses).
    #[test]
    fn argon2id_rfc9106_vector() {
        let params = argon2::ParamsBuilder::new()
            .m_cost(32)
            .t_cost(3)
            .p_cost(4)
            .data(argon2::AssociatedData::new(&[0x04; 12]).unwrap())
            .output_len(32)
            .build()
            .unwrap();
        let argon = Argon2::new_with_secret(&[0x03; 8], Algorithm::Argon2id, Version::V0x13, params).unwrap();
        let mut out = [0u8; 32];
        argon.hash_password_into(&[0x01; 32], &[0x02; 16], &mut out).unwrap();
        assert_eq!(out.to_vec(), hex_v("0d640df58d78766c08c037a34a8b53c9d01ef0452d75b65eb52520e96b01e659"));
    }

    /// XChaCha20-Poly1305 test vector from draft-irtf-cfrg-xchacha-03 §A.3.1.
    #[test]
    fn xchacha20poly1305_test_vector() {
        let key: [u8; 32] = hex32("808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f");
        let nonce: [u8; 24] = hex_n("404142434445464748494a4b4c4d4e4f5051525354555657");
        let aad = hex_v("50515253c0c1c2c3c4c5c6c7");
        let pt = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        let ct = seal(&key, &nonce, pt, &aad);
        let expected_tag = hex_v("c0875924c1c7987947deafd8780acf49");
        assert_eq!(&ct[ct.len() - 16..], &expected_tag[..]);
        let expected_ct_prefix = hex_v("bd6d179d3e83d43b9576579493c0e939");
        assert_eq!(&ct[..16], &expected_ct_prefix[..]);
        assert_eq!(&open(&key, &nonce, &ct, &aad).unwrap()[..], &pt[..]);
    }

    #[test]
    fn tamper_detection_every_byte() {
        let key = random_key();
        let nonce = random_bytes::<NONCE_LEN>();
        let ct = seal(&key, &nonce, b"rahasia", b"hdr");
        for i in 0..ct.len() {
            let mut bad = ct.clone();
            bad[i] ^= 0x01;
            assert!(open(&key, &nonce, &bad, b"hdr").is_err(), "byte {i} flip accepted");
        }
        assert!(open(&key, &nonce, &ct, b"hdX").is_err());
    }

    #[test]
    fn rejects_hostile_kdf_params() {
        assert!(KdfParams { m_cost_kib: 4 * 1024 * 1024, t_cost: 3, p_cost: 4 }.validate().is_err());
        assert!(KdfParams { m_cost_kib: 1024, t_cost: 0, p_cost: 1 }.validate().is_err());
        assert!(KdfParams { m_cost_kib: 1024, t_cost: 1, p_cost: 0 }.validate().is_err());
        assert!(KdfParams::DEFAULT.validate().is_ok());
    }

    fn hex_v(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }
    fn hex_n<const N: usize>(s: &str) -> [u8; N] {
        hex_v(s).try_into().unwrap()
    }
    fn hex32(s: &str) -> [u8; 32] {
        hex_n(s)
    }
}
