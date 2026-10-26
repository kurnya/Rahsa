use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Wrong master password *or* tampered file. The two are deliberately
    /// indistinguishable to the caller.
    #[error("master password salah atau file vault rusak")]
    Decrypt,
    #[error("bukan file vault Rahsa")]
    BadMagic,
    #[error("versi format vault {0} tidak didukung")]
    UnsupportedVersion(u16),
    #[error("file vault rusak: {0}")]
    Corrupt(&'static str),
    #[error("parameter KDF tidak valid")]
    BadKdfParams,
    #[error("master password minimal 12 karakter")]
    WeakMasterPassword,
    #[error("{0}")]
    Invalid(String),
    #[error("data tidak ditemukan")]
    NotFound,
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
