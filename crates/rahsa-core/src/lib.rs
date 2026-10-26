//! Rahsa vault core.
//!
//! Everything security-relevant lives here so it can be unit tested without the
//! desktop UI: the on-disk vault format, key derivation, encryption, origin
//! matching, the password generator and the extension session crypto.

pub mod crypto;
pub mod error;
pub mod format;
pub mod generator;
pub mod mask;
pub mod model;
pub mod origin;
pub mod session;
pub mod storage;
pub mod vault;
#[cfg(windows)]
pub mod win;

pub use error::{Error, Result};

/// Current time as Unix milliseconds.
pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
