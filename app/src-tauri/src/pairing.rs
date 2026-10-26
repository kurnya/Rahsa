//! Pairing store: the app's static X25519 key and the public keys of paired
//! extensions.
//!
//! Deviation from PRD §7 (which lists `PairedExtension` inside the vault): the
//! app must authenticate the extension while the vault is *locked* (for
//! `status` and `request_unlock`), so this lives in its own file protected
//! with DPAPI (current-user scope). It holds no account data or vault keys.

use std::path::{Path, PathBuf};

use rahsa_core::session::{b64, generate_secret, unb64};
use serde::{Deserialize, Serialize};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedExtension {
    pub extension_id: String,
    pub public_key: String,
    pub paired_at: i64,
    pub last_seen_at: Option<i64>,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    app_secret: String,
    extensions: Vec<PairedExtension>,
}

pub struct PairingStore {
    path: PathBuf,
    secret: StaticSecret,
    pub extensions: Vec<PairedExtension>,
}

impl PairingStore {
    pub fn load(path: &Path) -> PairingStore {
        let loaded = std::fs::read(path)
            .ok()
            .and_then(|enc| rahsa_core::win::dpapi_unprotect(&enc).ok())
            .and_then(|pt| serde_json::from_slice::<Stored>(&pt).ok())
            .and_then(|s| {
                let k: [u8; 32] = unb64(&s.app_secret).ok()?;
                Some((StaticSecret::from(k), s.extensions))
            });
        match loaded {
            Some((secret, extensions)) => PairingStore { path: path.to_path_buf(), secret, extensions },
            None => {
                let s = PairingStore { path: path.to_path_buf(), secret: generate_secret(), extensions: vec![] };
                let _ = s.save();
                s
            }
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let stored = Stored { app_secret: b64(self.secret.as_bytes()), extensions: self.extensions.clone() };
        let pt = Zeroizing::new(serde_json::to_vec(&stored).map_err(std::io::Error::other)?);
        let enc = rahsa_core::win::dpapi_protect(&pt)?;
        rahsa_core::storage::write_atomic(&self.path, &enc, false).map_err(std::io::Error::other)
    }

    pub fn secret(&self) -> &StaticSecret {
        &self.secret
    }

    pub fn public(&self) -> [u8; 32] {
        PublicKey::from(&self.secret).to_bytes()
    }

    pub fn find(&self, extension_id: &str) -> Option<&PairedExtension> {
        self.extensions.iter().find(|e| e.extension_id == extension_id)
    }

    pub fn add(&mut self, extension_id: &str, public_key: &[u8; 32]) -> std::io::Result<()> {
        self.extensions.retain(|e| e.extension_id != extension_id);
        self.extensions.push(PairedExtension {
            extension_id: extension_id.to_string(),
            public_key: b64(public_key),
            paired_at: rahsa_core::now_ms(),
            last_seen_at: None,
        });
        self.save()
    }

    pub fn touch(&mut self, extension_id: &str) {
        if let Some(e) = self.extensions.iter_mut().find(|e| e.extension_id == extension_id) {
            e.last_seen_at = Some(rahsa_core::now_ms());
            let _ = self.save();
        }
    }

    pub fn remove(&mut self, extension_id: &str) -> std::io::Result<bool> {
        let before = self.extensions.len();
        self.extensions.retain(|e| e.extension_id != extension_id);
        self.save()?;
        Ok(before != self.extensions.len())
    }
}
