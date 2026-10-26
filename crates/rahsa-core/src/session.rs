//! Extension ↔ app channel crypto (PRD §9).
//!
//! * Pairing: the extension and the app each hold a static X25519 key. The
//!   six-digit code shown on both sides is derived from both public keys, so a
//!   man in the middle would produce a different code.
//! * Session: every connection mixes the static DH with a fresh ephemeral DH
//!   through HKDF-SHA256, giving a new key per session.
//! * Messages: XChaCha20-Poly1305 with a random 24-byte nonce; the direction is
//!   bound as associated data so a message cannot be reflected back.
//!
//! The extension implements the same construction with @noble (see
//! `extension/src/crypto.ts`); `cross_check_vector` pins it.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::crypto::{self, NONCE_LEN};
use crate::{Error, Result};

pub const SESSION_INFO: &[u8] = b"rahsa-session-v1";
pub const PAIR_DOMAIN: &[u8] = b"rahsa-pair-v1";
pub const AAD_EXT_TO_APP: &[u8] = b"rahsa:e2a";
pub const AAD_APP_TO_EXT: &[u8] = b"rahsa:a2e";

pub fn b64(b: &[u8]) -> String {
    B64.encode(b)
}

pub fn unb64<const N: usize>(s: &str) -> Result<[u8; N]> {
    let v = B64.decode(s).map_err(|_| Error::Invalid("base64".into()))?;
    v.try_into().map_err(|_| Error::Invalid("panjang kunci salah".into()))
}

pub fn generate_secret() -> StaticSecret {
    StaticSecret::random_from_rng(rand::rngs::OsRng)
}

/// Six-digit code: SHA-256(domain ‖ ext_pub ‖ app_pub), first 4 bytes BE mod 10⁶.
pub fn pairing_code(ext_pub: &[u8; 32], app_pub: &[u8; 32]) -> String {
    let mut h = Sha256::new();
    h.update(PAIR_DOMAIN);
    h.update(ext_pub);
    h.update(app_pub);
    let d = h.finalize();
    let n = u32::from_be_bytes([d[0], d[1], d[2], d[3]]) % 1_000_000;
    format!("{n:06}")
}

/// Derive the session key on the app side.
pub fn derive_session_key(
    app_static: &StaticSecret,
    ext_static_pub: &[u8; 32],
    app_eph: &StaticSecret,
    ext_eph_pub: &[u8; 32],
    ext_nonce: &[u8; 16],
    app_nonce: &[u8; 16],
) -> Result<crypto::Key> {
    let dh1 = app_static.diffie_hellman(&PublicKey::from(*ext_static_pub));
    let dh2 = app_eph.diffie_hellman(&PublicKey::from(*ext_eph_pub));
    if !dh1.was_contributory() || !dh2.was_contributory() {
        return Err(Error::Invalid("kunci publik tidak valid".into()));
    }
    let mut ikm = Zeroizing::new([0u8; 64]);
    ikm[..32].copy_from_slice(dh1.as_bytes());
    ikm[32..].copy_from_slice(dh2.as_bytes());
    let mut salt = [0u8; 32];
    salt[..16].copy_from_slice(ext_nonce);
    salt[16..].copy_from_slice(app_nonce);
    let hk = Hkdf::<Sha256>::new(Some(&salt), ikm.as_ref());
    let mut key = Zeroizing::new([0u8; 32]);
    hk.expand(SESSION_INFO, key.as_mut()).expect("32 bytes is a valid HKDF-SHA256 length");
    Ok(key)
}

/// Encrypted frame body: `{ "n": base64(nonce), "c": base64(ciphertext) }`.
pub fn seal_json(key: &[u8; 32], aad: &[u8], plaintext: &[u8]) -> (String, String) {
    let nonce = crypto::random_bytes::<NONCE_LEN>();
    let ct = crypto::seal(key, &nonce, plaintext, aad);
    (b64(&nonce), b64(&ct))
}

pub fn open_json(key: &[u8; 32], aad: &[u8], n: &str, c: &str) -> Result<Zeroizing<Vec<u8>>> {
    let nonce: [u8; NONCE_LEN] = unb64(n)?;
    let ct = B64.decode(c).map_err(|_| Error::Invalid("base64".into()))?;
    crypto::open(key, &nonce, &ct, aad)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_sides_agree() {
        let app_s = generate_secret();
        let ext_s = generate_secret();
        let app_e = generate_secret();
        let ext_e = generate_secret();
        let (en, an) = ([1u8; 16], [2u8; 16]);
        let k_app = derive_session_key(
            &app_s,
            PublicKey::from(&ext_s).as_bytes(),
            &app_e,
            PublicKey::from(&ext_e).as_bytes(),
            &en,
            &an,
        )
        .unwrap();
        // Extension side computes the same DHs with roles swapped.
        let k_ext = derive_session_key(
            &ext_s,
            PublicKey::from(&app_s).as_bytes(),
            &ext_e,
            PublicKey::from(&app_e).as_bytes(),
            &en,
            &an,
        )
        .unwrap();
        assert_eq!(*k_app, *k_ext);

        let (n, c) = seal_json(&k_app, AAD_APP_TO_EXT, b"{\"ok\":true}");
        assert_eq!(&open_json(&k_ext, AAD_APP_TO_EXT, &n, &c).unwrap()[..], b"{\"ok\":true}");
        assert!(open_json(&k_ext, AAD_EXT_TO_APP, &n, &c).is_err(), "reflection must fail");
    }

    #[test]
    fn rejects_low_order_points() {
        let s = generate_secret();
        assert!(derive_session_key(&s, &[0u8; 32], &s, &[0u8; 32], &[0; 16], &[0; 16]).is_err());
    }

    #[test]
    fn pairing_code_is_six_digits_and_key_bound() {
        let a = pairing_code(&[1; 32], &[2; 32]);
        assert_eq!(a.len(), 6);
        assert!(a.chars().all(|c| c.is_ascii_digit()));
        assert_ne!(a, pairing_code(&[1; 32], &[3; 32]));
    }

    /// Fixed inputs whose outputs are asserted again in the extension's test
    /// (`extension/scripts/crypto-vector.mjs`) to prove both stacks agree.
    #[test]
    fn cross_check_vector() {
        let app_s = StaticSecret::from([0x11; 32]);
        let ext_s = StaticSecret::from([0x22; 32]);
        let app_e = StaticSecret::from([0x33; 32]);
        let ext_e = StaticSecret::from([0x44; 32]);
        let k = derive_session_key(
            &app_s,
            PublicKey::from(&ext_s).as_bytes(),
            &app_e,
            PublicKey::from(&ext_e).as_bytes(),
            &[0x55; 16],
            &[0x66; 16],
        )
        .unwrap();
        let code = pairing_code(PublicKey::from(&ext_s).as_bytes(), PublicKey::from(&app_s).as_bytes());
        let ct = crypto::seal(&k, &[0x77; 24], b"rahsa", AAD_APP_TO_EXT);
        println!("VECTOR key={} code={} ct={}", b64(k.as_ref()), code, b64(&ct));
    }
}
