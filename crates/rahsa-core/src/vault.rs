//! An unlocked vault and the operations on it.
//!
//! The vault key is random and independent of the master password: the
//! password only wraps it. Changing the master password rotates both.

use std::path::{Path, PathBuf};

use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use crate::crypto::{self, KdfParams, Key, KEY_LEN, NONCE_LEN, SALT_LEN};
use crate::format::{Header, HEADER_LEN};
use crate::model::*;
use crate::origin::{MatchMode, Origin};
use crate::{now_ms, storage, Error, Result};

pub const MIN_MASTER_LEN: usize = 12;

pub struct Vault {
    path: PathBuf,
    kdf: KdfParams,
    salt: [u8; SALT_LEN],
    wrap_nonce: [u8; NONCE_LEN],
    wrapped_key: [u8; KEY_LEN + 16],
    key: Key,
    pub data: VaultData,
}

impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vault").field("path", &self.path).finish_non_exhaustive()
    }
}

fn check_master(password: &str) -> Result<()> {
    if password.chars().count() < MIN_MASTER_LEN {
        return Err(Error::WeakMasterPassword);
    }
    Ok(())
}

/// Wrap `key` under a fresh KEK derived from `password`.
fn wrap(password: &str, kdf: KdfParams, key: &Key) -> Result<([u8; SALT_LEN], [u8; NONCE_LEN], [u8; KEY_LEN + 16])> {
    let salt = crypto::random_bytes::<SALT_LEN>();
    let wrap_nonce = crypto::random_bytes::<NONCE_LEN>();
    let kek = crypto::derive_kek(password, &salt, kdf)?;
    let probe = Header { kdf, salt, wrap_nonce, wrapped_key: [0; KEY_LEN + 16], payload_nonce: [0; NONCE_LEN] };
    let ct = crypto::seal(&kek, &wrap_nonce, key.as_ref(), &probe.kdf_section());
    Ok((salt, wrap_nonce, ct.try_into().expect("32-byte key + 16-byte tag")))
}

/// Decrypt a vault file image. Shared by unlock and backup verification.
fn decrypt_image(bytes: &[u8], password: &str) -> Result<(Header, Key, VaultData)> {
    let (header, ct) = Header::parse(bytes)?;
    let kek = crypto::derive_kek(password, &header.salt, header.kdf)?;
    let key_bytes = crypto::open(&kek, &header.wrap_nonce, &header.wrapped_key, &header.kdf_section())?;
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    key.copy_from_slice(&key_bytes);
    let pt = crypto::open(&key, &header.payload_nonce, ct, &header.to_bytes())?;
    let data: VaultData = serde_json::from_slice(&pt).map_err(|_| Error::Corrupt("payload tidak bisa dibaca"))?;
    if data.version != PAYLOAD_VERSION {
        return Err(Error::UnsupportedVersion(data.version as u16));
    }
    Ok((header, key, data))
}

impl Vault {
    pub fn create(path: &Path, password: &str) -> Result<Vault> {
        Self::create_with_params(path, password, KdfParams::DEFAULT)
    }

    pub fn create_with_params(path: &Path, password: &str, kdf: KdfParams) -> Result<Vault> {
        check_master(password)?;
        let key = crypto::random_key();
        let (salt, wrap_nonce, wrapped_key) = wrap(password, kdf, &key)?;
        let v = Vault {
            path: path.to_path_buf(),
            kdf,
            salt,
            wrap_nonce,
            wrapped_key,
            key,
            data: VaultData { version: PAYLOAD_VERSION, ..Default::default() },
        };
        v.save()?;
        Ok(v)
    }

    pub fn unlock(path: &Path, password: &str) -> Result<Vault> {
        let bytes = std::fs::read(path)?;
        let (h, key, data) = decrypt_image(&bytes, password)?;
        let mut v = Vault {
            path: path.to_path_buf(),
            kdf: h.kdf,
            salt: h.salt,
            wrap_nonce: h.wrap_nonce,
            wrapped_key: h.wrapped_key,
            key,
            data,
        };
        if v.purge_trash(now_ms()) > 0 {
            v.save()?;
        }
        Ok(v)
    }

    /// Check that `bytes` is a vault that `password` opens (used before restore).
    pub fn verify_image(bytes: &[u8], password: &str) -> Result<usize> {
        decrypt_image(bytes, password).map(|(_, _, d)| d.accounts.len())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn encrypt_image(&self) -> Result<Vec<u8>> {
        let header = Header {
            kdf: self.kdf,
            salt: self.salt,
            wrap_nonce: self.wrap_nonce,
            wrapped_key: self.wrapped_key,
            payload_nonce: crypto::random_bytes::<NONCE_LEN>(),
        };
        let hb = header.to_bytes();
        let pt = Zeroizing::new(serde_json::to_vec(&self.data)?);
        let ct = crypto::seal(&self.key, &header.payload_nonce, &pt, &hb);
        let mut out = Vec::with_capacity(HEADER_LEN + ct.len());
        out.extend_from_slice(&hb);
        out.extend_from_slice(&ct);
        Ok(out)
    }

    pub fn save(&self) -> Result<()> {
        let image = self.encrypt_image()?;
        storage::write_atomic(&self.path, &image, true)
    }

    /// Write an encrypted copy elsewhere (backup export). The copy opens with
    /// the current master password.
    pub fn export_to(&self, dest: &Path) -> Result<()> {
        let image = self.encrypt_image()?;
        storage::write_atomic(dest, &image, false)
    }

    pub fn verify_master(&self, password: &str) -> Result<()> {
        let kek = crypto::derive_kek(password, &self.salt, self.kdf)?;
        let probe = Header {
            kdf: self.kdf,
            salt: self.salt,
            wrap_nonce: self.wrap_nonce,
            wrapped_key: self.wrapped_key,
            payload_nonce: [0; NONCE_LEN],
        };
        crypto::open(&kek, &self.wrap_nonce, &self.wrapped_key, &probe.kdf_section()).map(|_| ())
    }

    /// F-15: requires the old password; generates a new vault key and re-encrypts.
    pub fn change_master(&mut self, old: &str, new: &str) -> Result<()> {
        self.verify_master(old)?;
        check_master(new)?;
        let key = crypto::random_key();
        let (salt, wrap_nonce, wrapped_key) = wrap(new, self.kdf, &key)?;
        self.key = key;
        self.salt = salt;
        self.wrap_nonce = wrap_nonce;
        self.wrapped_key = wrapped_key;
        self.save()
    }

    // ---------- institutions ----------

    pub fn upsert_institution(&mut self, input: InstitutionInput) -> Result<Uuid> {
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err(Error::Invalid("nama institusi wajib diisi".into()));
        }
        let login_url = input.login_url.trim().to_string();
        let mut origins = vec![Origin::from_user_input(&login_url, MatchMode::Exact)?];
        for extra in &input.extra_origins {
            let o = Origin::from_user_input(&extra.url, extra.match_mode)?;
            if !origins.contains(&o) {
                origins.push(o);
            }
        }
        let id = input.id.unwrap_or_else(Uuid::new_v4);
        let inst = Institution {
            id,
            name,
            login_url,
            origins,
            icon: input.icon,
            open_incognito: input.open_incognito,
            fresh_session: input.fresh_session,
        };
        match self.data.institutions.iter_mut().find(|i| i.id == id) {
            Some(existing) => *existing = inst,
            None if input.id.is_some() => return Err(Error::NotFound),
            None => self.data.institutions.push(inst),
        }
        self.save()?;
        Ok(id)
    }

    /// Accounts of a deleted institution are kept and become "tanpa institusi".
    pub fn delete_institution(&mut self, id: Uuid) -> Result<()> {
        let before = self.data.institutions.len();
        self.data.institutions.retain(|i| i.id != id);
        if self.data.institutions.len() == before {
            return Err(Error::NotFound);
        }
        for a in self.data.accounts.iter_mut().filter(|a| a.institution_id == Some(id)) {
            a.institution_id = None;
        }
        self.save()
    }

    pub fn institution(&self, id: Uuid) -> Option<&Institution> {
        self.data.institutions.iter().find(|i| i.id == id)
    }

    // ---------- accounts ----------

    pub fn account(&self, id: Uuid) -> Option<&Account> {
        self.data.accounts.iter().find(|a| a.id == id)
    }

    /// All origins at which this account may be filled.
    pub fn effective_origins(&self, account: &Account) -> Vec<Origin> {
        let mut v = account.origins.clone();
        if let Some(inst) = account.institution_id.and_then(|id| self.institution(id)) {
            for o in &inst.origins {
                if !v.contains(o) {
                    v.push(o.clone());
                }
            }
        }
        v
    }

    pub fn account_matches(&self, account: &Account, page_origin: &str) -> bool {
        self.effective_origins(account).iter().any(|o| o.matches_str(page_origin))
    }

    /// F-05: accounts for an origin, most recently used first.
    pub fn accounts_for_origin(&self, page_origin: &str) -> Vec<&Account> {
        let mut v: Vec<&Account> = self.data.accounts.iter().filter(|a| self.account_matches(a, page_origin)).collect();
        v.sort_by(|a, b| b.last_used_at.unwrap_or(0).cmp(&a.last_used_at.unwrap_or(0)).then(a.label.cmp(&b.label)));
        v
    }

    pub fn upsert_account(&mut self, mut input: AccountInput) -> Result<Uuid> {
        let label = input.label.trim().to_string();
        if label.is_empty() {
            return Err(Error::Invalid("label akun wajib diisi".into()));
        }
        if let Some(iid) = input.institution_id {
            if self.institution(iid).is_none() {
                return Err(Error::Invalid("institusi tidak ditemukan".into()));
            }
        }
        let mut origins = Vec::new();
        for o in &input.origins {
            let o = Origin::from_user_input(&o.url, o.match_mode)?;
            if !origins.contains(&o) {
                origins.push(o);
            }
        }
        if input.institution_id.is_none() && origins.is_empty() {
            return Err(Error::Invalid("akun tanpa institusi harus punya minimal satu URL".into()));
        }
        let mut tags: Vec<String> = input.tags.iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
        tags.sort();
        tags.dedup();
        let custom_fields: Vec<CustomField> = input
            .custom_fields
            .drain(..)
            .filter(|f| !f.name.trim().is_empty())
            .collect();
        let now = now_ms();

        let id = match input.id {
            Some(id) => {
                let a = self.data.accounts.iter_mut().find(|a| a.id == id).ok_or(Error::NotFound)?;
                if let Some(pw) = input.password.take() {
                    if pw != a.password {
                        let old = std::mem::take(&mut a.password);
                        a.password_history.insert(0, PasswordHistoryEntry { password: old, changed_at: now });
                        a.password_history.truncate(PASSWORD_HISTORY_LEN);
                        a.password = pw;
                    }
                }
                a.institution_id = input.institution_id;
                a.label = label;
                a.username = std::mem::take(&mut input.username);
                a.origins = origins;
                a.notes = std::mem::take(&mut input.notes);
                // Keep a secret custom field's value when the UI sends it back empty.
                let old_fields = std::mem::take(&mut a.custom_fields);
                a.custom_fields = custom_fields
                    .into_iter()
                    .map(|mut f| {
                        if f.secret && f.value.is_empty() {
                            if let Some(old) = old_fields.iter().find(|o| o.secret && o.name == f.name) {
                                f.value = old.value.clone();
                            }
                        }
                        f
                    })
                    .collect();
                a.tags = tags;
                a.auto_submit = input.auto_submit;
                a.updated_at = now;
                id
            }
            None => {
                let id = Uuid::new_v4();
                self.data.accounts.push(Account {
                    id,
                    institution_id: input.institution_id,
                    label,
                    username: std::mem::take(&mut input.username),
                    password: input.password.take().unwrap_or_default(),
                    origins,
                    notes: std::mem::take(&mut input.notes),
                    custom_fields,
                    tags,
                    auto_submit: input.auto_submit,
                    created_at: now,
                    updated_at: now,
                    last_used_at: None,
                    password_history: Vec::new(),
                });
                id
            }
        };
        self.save()?;
        Ok(id)
    }

    pub fn mark_used(&mut self, id: Uuid) -> Result<()> {
        let a = self.data.accounts.iter_mut().find(|a| a.id == id).ok_or(Error::NotFound)?;
        a.last_used_at = Some(now_ms());
        self.save()
    }

    /// F-04: deleting moves the account to the trash for 30 days.
    pub fn trash_account(&mut self, id: Uuid) -> Result<()> {
        let idx = self.data.accounts.iter().position(|a| a.id == id).ok_or(Error::NotFound)?;
        let account = self.data.accounts.remove(idx);
        self.data.trash.push(TrashItem { account, deleted_at: now_ms() });
        self.save()
    }

    pub fn restore_from_trash(&mut self, id: Uuid) -> Result<()> {
        let idx = self.data.trash.iter().position(|t| t.account.id == id).ok_or(Error::NotFound)?;
        let item = self.data.trash.remove(idx);
        let mut account = item.account.clone();
        if account.institution_id.is_some_and(|iid| self.institution(iid).is_none()) {
            account.institution_id = None;
        }
        self.data.accounts.push(account);
        self.save()
    }

    pub fn delete_from_trash(&mut self, id: Uuid) -> Result<()> {
        let before = self.data.trash.len();
        self.data.trash.retain(|t| t.account.id != id);
        if before == self.data.trash.len() {
            return Err(Error::NotFound);
        }
        self.save()
    }

    /// Remove trash older than 30 days. Returns how many were purged.
    pub fn purge_trash(&mut self, now: i64) -> usize {
        let before = self.data.trash.len();
        self.data.trash.retain(|t| now - t.deleted_at < TRASH_RETENTION_MS);
        before - self.data.trash.len()
    }

    pub fn update_settings(&mut self, s: Settings) -> Result<()> {
        s.validate()?;
        self.data.settings = s;
        self.save()
    }
}

impl Drop for Vault {
    fn drop(&mut self) {
        // `key` is Zeroizing; also scrub the wrapped copy and salt for tidiness.
        self.wrapped_key.zeroize();
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct OriginInput {
    pub url: String,
    #[serde(default)]
    pub match_mode: MatchMode,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct InstitutionInput {
    pub id: Option<Uuid>,
    pub name: String,
    pub login_url: String,
    #[serde(default)]
    pub extra_origins: Vec<OriginInput>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub open_incognito: bool,
    #[serde(default = "yes")]
    pub fresh_session: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AccountInput {
    pub id: Option<Uuid>,
    pub institution_id: Option<Uuid>,
    pub label: String,
    pub username: String,
    /// `None` on edit means "unchanged".
    pub password: Option<String>,
    #[serde(default)]
    pub origins: Vec<OriginInput>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub custom_fields: Vec<CustomField>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub auto_submit: bool,
}

impl Drop for AccountInput {
    fn drop(&mut self) {
        if let Some(p) = self.password.as_mut() {
            p.zeroize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PW: &str = "kuda-lari-di-pagi-hari";

    fn new_vault(dir: &tempfile::TempDir) -> Vault {
        Vault::create_with_params(&dir.path().join("vault.rahsa"), PW, KdfParams::TEST).unwrap()
    }

    fn siakad(v: &mut Vault) -> Uuid {
        v.upsert_institution(InstitutionInput {
            id: None,
            name: "SIAKAD Kampus".into(),
            login_url: "https://siakad.kampus.ac.id/login".into(),
            extra_origins: vec![],
            icon: None,
            open_incognito: false,
            fresh_session: true,
        })
        .unwrap()
    }

    fn acct(inst: Uuid, label: &str, pw: &str) -> AccountInput {
        AccountInput {
            id: None,
            institution_id: Some(inst),
            label: label.into(),
            username: format!("{label}@kampus.ac.id"),
            password: Some(pw.into()),
            origins: vec![],
            notes: String::new(),
            custom_fields: vec![],
            tags: vec!["kampus".into()],
            auto_submit: false,
        }
    }

    #[test]
    fn round_trip_and_wrong_password() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = new_vault(&dir);
        let i = siakad(&mut v);
        let a = v.upsert_account(acct(i, "Akun Pribadi", "s3cret-pass")).unwrap();
        let path = v.path().to_path_buf();
        drop(v);

        let v = Vault::unlock(&path, PW).unwrap();
        assert_eq!(v.account(a).unwrap().password, "s3cret-pass");
        assert!(matches!(Vault::unlock(&path, "salah-password-nih"), Err(Error::Decrypt)));
    }

    #[test]
    fn file_does_not_leak_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = new_vault(&dir);
        let i = siakad(&mut v);
        v.upsert_account(acct(i, "Admin Prodi", "PlainSecretXYZ")).unwrap();
        let raw = std::fs::read(v.path()).unwrap();
        for needle in ["PlainSecretXYZ", "Admin Prodi", "siakad", "kampus"] {
            assert!(!raw.windows(needle.len()).any(|w| w == needle.as_bytes()), "{needle} leaked");
        }
    }

    #[test]
    fn any_flipped_byte_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let v = new_vault(&dir);
        let raw = std::fs::read(v.path()).unwrap();
        for i in 0..raw.len() {
            let mut bad = raw.clone();
            bad[i] ^= 0x80;
            assert!(Vault::verify_image(&bad, PW).is_err(), "flip at {i} accepted");
        }
        // Truncation.
        for n in [0, 5, 100, raw.len() - 1] {
            assert!(Vault::verify_image(&raw[..n], PW).is_err());
        }
    }

    #[test]
    fn change_master_password() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = new_vault(&dir);
        assert!(v.change_master("bukan-password-lama", "password-baru-yang-panjang").is_err());
        assert!(v.change_master(PW, "pendek").is_err());
        v.change_master(PW, "password-baru-yang-panjang").unwrap();
        let p = v.path().to_path_buf();
        drop(v);
        assert!(Vault::unlock(&p, PW).is_err());
        Vault::unlock(&p, "password-baru-yang-panjang").unwrap();
    }

    #[test]
    fn many_accounts_per_origin_sorted_by_last_used() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = new_vault(&dir);
        let i = siakad(&mut v);
        let a = v.upsert_account(acct(i, "Akun Pribadi", "p1-xxxxxxxx")).unwrap();
        let b = v.upsert_account(acct(i, "Operator", "p2-xxxxxxxx")).unwrap();
        v.mark_used(b).unwrap();
        let list: Vec<Uuid> = v.accounts_for_origin("https://siakad.kampus.ac.id").iter().map(|a| a.id).collect();
        assert_eq!(list, vec![b, a]);
        assert!(v.accounts_for_origin("https://siakad.kampus.ac.id.evil.com").is_empty());
    }

    #[test]
    fn password_history_and_trash() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = new_vault(&dir);
        let i = siakad(&mut v);
        let id = v.upsert_account(acct(i, "Akun", "pw-0-xxxxxxx")).unwrap();
        for n in 1..=7 {
            let mut e = acct(i, "Akun", &format!("pw-{n}-xxxxxxx"));
            e.id = Some(id);
            v.upsert_account(e).unwrap();
        }
        let a = v.account(id).unwrap();
        assert_eq!(a.password, "pw-7-xxxxxxx");
        assert_eq!(a.password_history.len(), PASSWORD_HISTORY_LEN);
        assert_eq!(a.password_history[0].password, "pw-6-xxxxxxx");

        // Editing without a password keeps it.
        let mut e = acct(i, "Akun diganti", "");
        e.id = Some(id);
        e.password = None;
        v.upsert_account(e).unwrap();
        assert_eq!(v.account(id).unwrap().password, "pw-7-xxxxxxx");

        v.trash_account(id).unwrap();
        assert!(v.account(id).is_none());
        v.restore_from_trash(id).unwrap();
        assert!(v.account(id).is_some());
        v.trash_account(id).unwrap();
        assert_eq!(v.purge_trash(now_ms() + TRASH_RETENTION_MS + 1), 1);
    }

    #[test]
    fn deleting_institution_keeps_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = new_vault(&dir);
        let i = siakad(&mut v);
        let a = v.upsert_account(acct(i, "Akun", "pw-xxxxxxxxx")).unwrap();
        v.delete_institution(i).unwrap();
        assert_eq!(v.account(a).unwrap().institution_id, None);
    }

    #[test]
    fn rejects_http_and_weak_master() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            Vault::create_with_params(&dir.path().join("x.rahsa"), "short", KdfParams::TEST),
            Err(Error::WeakMasterPassword)
        ));
        let mut v = new_vault(&dir);
        let r = v.upsert_institution(InstitutionInput {
            id: None,
            name: "X".into(),
            login_url: "http://siakad.kampus.ac.id".into(),
            extra_origins: vec![],
            icon: None,
            open_incognito: false,
            fresh_session: true,
        });
        assert!(r.is_err());
    }

    #[test]
    fn export_opens_with_same_password() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = new_vault(&dir);
        let i = siakad(&mut v);
        v.upsert_account(acct(i, "Akun", "pw-xxxxxxxxx")).unwrap();
        let dest = dir.path().join("rahsa-backup-20261026.rahsa");
        v.export_to(&dest).unwrap();
        assert_eq!(Vault::verify_image(&std::fs::read(&dest).unwrap(), PW).unwrap(), 1);
    }
}
