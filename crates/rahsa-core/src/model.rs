//! Entities stored inside the encrypted payload (PRD §7).

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroize;

use crate::origin::Origin;

pub const PAYLOAD_VERSION: u32 = 1;
pub const TRASH_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1000;
pub const PASSWORD_HISTORY_LEN: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VaultData {
    pub version: u32,
    #[serde(default)]
    pub institutions: Vec<Institution>,
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub trash: Vec<TrashItem>,
    #[serde(default)]
    pub settings: Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Institution {
    pub id: Uuid,
    pub name: String,
    /// Always `https://…`; validated on save.
    pub login_url: String,
    /// Derived from `login_url` plus any extra origins the user registers.
    pub origins: Vec<Origin>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub open_incognito: bool,
    /// "Masuk" logs the site out first (clears its cookies/storage in Chrome).
    #[serde(default = "yes")]
    pub fresh_session: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: Uuid,
    #[serde(default)]
    pub institution_id: Option<Uuid>,
    pub label: String,
    pub username: String,
    pub password: String,
    /// Extra origins for this account in addition to the institution's.
    #[serde(default)]
    pub origins: Vec<Origin>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub custom_fields: Vec<CustomField>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub auto_submit: bool,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub last_used_at: Option<i64>,
    #[serde(default)]
    pub password_history: Vec<PasswordHistoryEntry>,
}

impl Drop for Account {
    fn drop(&mut self) {
        self.username.zeroize();
        self.password.zeroize();
        self.notes.zeroize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordHistoryEntry {
    pub password: String,
    pub changed_at: i64,
}

impl Drop for PasswordHistoryEntry {
    fn drop(&mut self) {
        self.password.zeroize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomField {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub secret: bool,
}

impl Drop for CustomField {
    fn drop(&mut self) {
        if self.secret {
            self.value.zeroize();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashItem {
    pub account: Account,
    pub deleted_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub auto_lock_minutes: u32,
    pub lock_on_screen_lock: bool,
    pub clipboard_clear_seconds: u32,
    pub hello_enabled: bool,
    pub generator_defaults: GeneratorOptions,
    pub language: String,
    pub last_backup_at: Option<i64>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            auto_lock_minutes: 10,
            lock_on_screen_lock: true,
            clipboard_clear_seconds: 20,
            hello_enabled: false,
            generator_defaults: GeneratorOptions::default(),
            language: "id".into(),
            last_backup_at: None,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> crate::Result<()> {
        if !(1..=60).contains(&self.auto_lock_minutes) {
            return Err(crate::Error::Invalid("auto-lock harus 1–60 menit".into()));
        }
        if !(5..=120).contains(&self.clipboard_clear_seconds) {
            return Err(crate::Error::Invalid("pengosongan clipboard harus 5–120 detik".into()));
        }
        if !matches!(self.language.as_str(), "id" | "en") {
            return Err(crate::Error::Invalid("bahasa tidak dikenal".into()));
        }
        self.generator_defaults.validate()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GeneratorOptions {
    pub length: u32,
    pub lower: bool,
    pub upper: bool,
    pub digits: bool,
    pub symbols: bool,
    pub min_digits: u32,
    pub min_symbols: u32,
    pub min_upper: u32,
    pub min_lower: u32,
    pub avoid_ambiguous: bool,
    /// Restrict symbols to the set an institution accepts, e.g. `"!@#$"`.
    pub symbol_set: Option<String>,
}

impl Default for GeneratorOptions {
    fn default() -> Self {
        GeneratorOptions {
            length: 20,
            lower: true,
            upper: true,
            digits: true,
            symbols: true,
            min_digits: 1,
            min_symbols: 1,
            min_upper: 1,
            min_lower: 1,
            avoid_ambiguous: true,
            symbol_set: None,
        }
    }
}

/// Fields the UI may see in lists; never contains a password.
#[derive(Debug, Clone, Serialize)]
pub struct AccountMeta {
    pub id: Uuid,
    pub institution_id: Option<Uuid>,
    pub label: String,
    pub username: String,
    pub origins: Vec<Origin>,
    pub notes: String,
    pub custom_fields: Vec<CustomFieldMeta>,
    pub tags: Vec<String>,
    pub auto_submit: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_used_at: Option<i64>,
    pub password_changed_at: Option<i64>,
    pub history_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CustomFieldMeta {
    pub name: String,
    /// Empty for secret fields.
    pub value: String,
    pub selector: Option<String>,
    pub secret: bool,
}

impl From<&Account> for AccountMeta {
    fn from(a: &Account) -> Self {
        AccountMeta {
            id: a.id,
            institution_id: a.institution_id,
            label: a.label.clone(),
            username: a.username.clone(),
            origins: a.origins.clone(),
            notes: a.notes.clone(),
            custom_fields: a
                .custom_fields
                .iter()
                .map(|f| CustomFieldMeta {
                    name: f.name.clone(),
                    value: if f.secret { String::new() } else { f.value.clone() },
                    selector: f.selector.clone(),
                    secret: f.secret,
                })
                .collect(),
            tags: a.tags.clone(),
            auto_submit: a.auto_submit,
            created_at: a.created_at,
            updated_at: a.updated_at,
            last_used_at: a.last_used_at,
            password_changed_at: a.password_history.first().map(|h| h.changed_at),
            history_count: a.password_history.len(),
        }
    }
}
