//! Crash-safe file writes (PRD §10 "Keandalan").
//!
//! 1. write `vault.rahsa.tmp` and flush it to disk,
//! 2. rotate the previous versions `vault.rahsa.1` → `.2` → `.3`,
//! 3. copy the current file to `.1`,
//! 4. atomically rename the temp file over `vault.rahsa`.
//!
//! A power cut at any point leaves either the old or the new file in place.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::Result;

pub const KEEP_VERSIONS: usize = 3;

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

pub fn version_path(path: &Path, n: usize) -> PathBuf {
    sibling(path, &format!(".{n}"))
}

pub fn write_atomic(path: &Path, bytes: &[u8], keep_versions: bool) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = sibling(path, ".tmp");
    {
        let mut f = OpenOptions::new().write(true).create(true).truncate(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    if keep_versions && path.exists() {
        for n in (1..KEEP_VERSIONS).rev() {
            let from = version_path(path, n);
            if from.exists() {
                fs::rename(&from, version_path(path, n + 1))?;
            }
        }
        fs::copy(path, version_path(path, 1))?;
    }
    // std::fs::rename uses MoveFileExW(MOVEFILE_REPLACE_EXISTING) on Windows.
    fs::rename(&tmp, path)?;
    if let Some(dir) = path.parent() {
        // Best effort: persist the directory entry (no-op/unsupported on Windows).
        let _ = File::open(dir).and_then(|d| d.sync_all());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_three_versions() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("vault.rahsa");
        for i in 0..6u8 {
            write_atomic(&p, &[i], true).unwrap();
        }
        assert_eq!(fs::read(&p).unwrap(), vec![5]);
        assert_eq!(fs::read(version_path(&p, 1)).unwrap(), vec![4]);
        assert_eq!(fs::read(version_path(&p, 2)).unwrap(), vec![3]);
        assert_eq!(fs::read(version_path(&p, 3)).unwrap(), vec![2]);
        assert!(!version_path(&p, 4).exists());
        assert!(!sibling(&p, ".tmp").exists());
    }

    #[test]
    fn leftover_tmp_does_not_affect_vault() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("vault.rahsa");
        write_atomic(&p, b"good", true).unwrap();
        // Simulate a crash mid-write: a truncated tmp file remains.
        fs::write(sibling(&p, ".tmp"), b"go").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"good");
        write_atomic(&p, b"newer", true).unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"newer");
    }
}
