use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rahsa_core::vault::Vault;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{broadcast, oneshot};
use uuid::Uuid;

use crate::pairing::PairingStore;

/// PRD §7 `LoginIntent` — memory only, never written to disk.
#[derive(Debug, Clone)]
pub struct LoginIntent {
    pub account_id: Uuid,
    pub institution_name: String,
    pub expires_at: Instant,
    pub used: bool,
}

pub const INTENT_TTL: Duration = Duration::from_secs(30);

pub struct PendingPairing {
    pub extension_id: String,
    pub ext_pub: [u8; 32],
    pub code: String,
    pub respond: Option<oneshot::Sender<bool>>,
}

/// Events pushed to connected extensions.
#[derive(Debug, Clone)]
pub enum ExtEvent {
    Locked,
    Unlocked,
    VaultChanged,
    Revoked(String),
    /// Ask one extension session to open a login page (optionally after
    /// logging the site out by clearing its cookies and storage).
    OpenLogin { session: u64, url: String, origins: Vec<String>, patterns: Vec<String>, fresh: bool },
}

pub struct AppState {
    pub data_dir: PathBuf,
    pub vault_path: PathBuf,
    pub vault: Mutex<Option<Vault>>,
    pub failed_attempts: Mutex<(u32, Option<Instant>)>,
    pub intent: Mutex<Option<LoginIntent>>,
    pub pairing: Mutex<PairingStore>,
    pub pending_pairing: Mutex<Option<PendingPairing>>,
    /// Active extension sessions: session id → extension id.
    pub sessions: Mutex<HashMap<u64, String>>,
    pub credential_log: Mutex<VecDeque<Instant>>,
    pub events: broadcast::Sender<ExtEvent>,
    /// Match patterns the extension holds host permission for (reported by it).
    /// `None` until a connected extension reports.
    pub granted_patterns: Mutex<Option<Vec<String>>>,
}

impl AppState {
    pub fn new(data_dir: &Path) -> AppState {
        let (events, _) = broadcast::channel(32);
        AppState {
            data_dir: data_dir.to_path_buf(),
            vault_path: data_dir.join("vault.rahsa"),
            vault: Mutex::new(None),
            failed_attempts: Mutex::new((0, None)),
            intent: Mutex::new(None),
            pairing: Mutex::new(PairingStore::load(&data_dir.join("pairing.dat"))),
            pending_pairing: Mutex::new(None),
            sessions: Mutex::new(HashMap::new()),
            credential_log: Mutex::new(VecDeque::new()),
            events,
            granted_patterns: Mutex::new(None),
        }
    }

    pub fn is_unlocked(&self) -> bool {
        self.vault.lock().unwrap().is_some()
    }

    /// Run `f` against the unlocked vault.
    pub fn with_vault<T>(&self, f: impl FnOnce(&mut Vault) -> Result<T, String>) -> Result<T, String> {
        let mut g = self.vault.lock().unwrap();
        match g.as_mut() {
            Some(v) => f(v),
            None => Err(LOCKED.into()),
        }
    }

    /// PRD F-03: drop the vault (keys are zeroized on drop) and tell everyone.
    pub fn lock(&self, app: &AppHandle, reason: &str) {
        let was_open = self.vault.lock().unwrap().take().is_some();
        *self.intent.lock().unwrap() = None;
        if was_open {
            let _ = self.events.send(ExtEvent::Locked);
            let _ = app.emit("vault-locked", reason);
            crate::tray::refresh(app);
        }
    }

    pub fn extension_connected(&self) -> bool {
        !self.sessions.lock().unwrap().is_empty()
    }

    /// The most recently connected extension session (one Chrome profile).
    pub fn latest_session(&self) -> Option<u64> {
        self.sessions.lock().unwrap().keys().max().copied()
    }
}

pub const LOCKED: &str = "Vault terkunci. Buka kunci dulu.";

#[derive(Serialize, Clone)]
pub struct Toast {
    pub kind: &'static str,
    pub message: String,
    /// Optional follow-up the UI can offer, e.g. reopening in Incognito.
    pub action: Option<ToastAction>,
}

#[derive(Serialize, Clone)]
pub struct ToastAction {
    pub kind: &'static str,
    pub account_id: Uuid,
}

pub fn toast(app: &AppHandle, t: Toast) {
    let _ = app.emit("toast", t);
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}
