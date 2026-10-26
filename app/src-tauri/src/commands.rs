//! Tauri commands for the UI. The WebView only ever receives what it shows:
//! lists carry no passwords; a password crosses only on explicit reveal.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use rahsa_core::model::{AccountMeta, GeneratorOptions, Institution, Settings};
use rahsa_core::vault::{AccountInput, InstitutionInput, Vault};
use rahsa_core::{now_ms, Error};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::state::{AppState, ExtEvent, LoginIntent, INTENT_TTL};
use crate::{chrome, tray};

type R<T> = Result<T, String>;

fn err(e: Error) -> String {
    e.to_string()
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> R<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct AppStatus {
    vault_exists: bool,
    unlocked: bool,
    vault_path: String,
    retry_after_secs: u64,
    language: String,
}

#[tauri::command]
pub async fn app_status(state: State<'_, AppState>) -> R<AppStatus> {
    let g = state.vault.lock().unwrap();
    let retry = state.failed_attempts.lock().unwrap().1.map(|t| t.saturating_duration_since(Instant::now()).as_secs());
    Ok(AppStatus {
        vault_exists: state.vault_path.exists(),
        unlocked: g.is_some(),
        vault_path: state.vault_path.to_string_lossy().into(),
        retry_after_secs: retry.unwrap_or(0),
        language: g.as_ref().map(|v| v.data.settings.language.clone()).unwrap_or_else(|| "id".into()),
    })
}

fn on_unlocked(app: &AppHandle, state: &AppState) {
    *state.failed_attempts.lock().unwrap() = (0, None);
    let _ = state.events.send(ExtEvent::Unlocked);
    let _ = app.emit("vault-unlocked", ());
    tray::refresh(app);
}

#[tauri::command]
pub async fn create_vault(app: AppHandle, state: State<'_, AppState>, password: String) -> R<()> {
    if state.vault_path.exists() {
        return Err("Vault sudah ada di laptop ini.".into());
    }
    let path = state.vault_path.clone();
    let v = blocking(move || Vault::create(&path, &password)).await?.map_err(err)?;
    *state.vault.lock().unwrap() = Some(v);
    on_unlocked(&app, &state);
    Ok(())
}

/// F-02: wrong password → wait 1, 2, 4 … max 30 s before the next attempt.
#[tauri::command]
pub async fn unlock(app: AppHandle, state: State<'_, AppState>, password: String) -> R<()> {
    if let Some(until) = state.failed_attempts.lock().unwrap().1 {
        let left = until.saturating_duration_since(Instant::now());
        if !left.is_zero() {
            return Err(format!("Tunggu {} detik sebelum mencoba lagi.", left.as_secs() + 1));
        }
    }
    let path = state.vault_path.clone();
    match blocking(move || Vault::unlock(&path, &password)).await? {
        Ok(v) => {
            *state.vault.lock().unwrap() = Some(v);
            on_unlocked(&app, &state);
            Ok(())
        }
        Err(e) => {
            let mut f = state.failed_attempts.lock().unwrap();
            f.0 += 1;
            let delay = 1u64 << (f.0 - 1).min(5);
            f.1 = Some(Instant::now() + Duration::from_secs(delay.min(30)));
            Err(match e {
                Error::Decrypt => "Master password salah.".into(),
                other => other.to_string(),
            })
        }
    }
}

#[tauri::command]
pub async fn lock(app: AppHandle, state: State<'_, AppState>) -> R<()> {
    state.lock(&app, "manual");
    Ok(())
}

#[derive(Serialize)]
pub struct Overview {
    institutions: Vec<Institution>,
    accounts: Vec<AccountMeta>,
    trash_count: usize,
    backup_due: bool,
}

#[tauri::command]
pub async fn overview(state: State<'_, AppState>) -> R<Overview> {
    state.with_vault(|v| {
        // Remind 30 days after the last backup, or 30 days after the oldest
        // account if there has never been one.
        let since = v
            .data
            .settings
            .last_backup_at
            .or_else(|| v.data.accounts.iter().map(|a| a.created_at).min());
        Ok(Overview {
            institutions: v.data.institutions.clone(),
            accounts: v.data.accounts.iter().map(AccountMeta::from).collect(),
            trash_count: v.data.trash.len(),
            backup_due: since.is_some_and(|t| now_ms() - t > 30 * 24 * 3600 * 1000),
        })
    })
}

#[tauri::command]
pub async fn reveal_password(state: State<'_, AppState>, id: Uuid) -> R<String> {
    state.with_vault(|v| v.account(id).map(|a| a.password.clone()).ok_or_else(|| "Akun tidak ditemukan".into()))
}

#[tauri::command]
pub async fn reveal_custom_field(state: State<'_, AppState>, id: Uuid, name: String) -> R<String> {
    state.with_vault(|v| {
        v.account(id)
            .and_then(|a| a.custom_fields.iter().find(|f| f.name == name).map(|f| f.value.clone()))
            .ok_or_else(|| "Field tidak ditemukan".into())
    })
}

#[derive(Serialize)]
pub struct HistoryItem {
    password: String,
    changed_at: i64,
}

#[tauri::command]
pub async fn password_history(state: State<'_, AppState>, id: Uuid) -> R<Vec<HistoryItem>> {
    state.with_vault(|v| {
        let a = v.account(id).ok_or("Akun tidak ditemukan")?;
        Ok(a.password_history
            .iter()
            .map(|h| HistoryItem { password: h.password.clone(), changed_at: h.changed_at })
            .collect())
    })
}

fn changed(state: &AppState) {
    let _ = state.events.send(ExtEvent::VaultChanged);
}

#[tauri::command]
pub async fn save_institution(state: State<'_, AppState>, input: InstitutionInput) -> R<Uuid> {
    let id = state.with_vault(|v| v.upsert_institution(input).map_err(err))?;
    changed(&state);
    Ok(id)
}

#[tauri::command]
pub async fn delete_institution(state: State<'_, AppState>, id: Uuid) -> R<()> {
    state.with_vault(|v| v.delete_institution(id).map_err(err))?;
    changed(&state);
    Ok(())
}

#[tauri::command]
pub async fn save_account(state: State<'_, AppState>, input: AccountInput) -> R<Uuid> {
    let id = state.with_vault(|v| v.upsert_account(input).map_err(err))?;
    changed(&state);
    Ok(id)
}

#[tauri::command]
pub async fn delete_account(state: State<'_, AppState>, id: Uuid) -> R<()> {
    state.with_vault(|v| v.trash_account(id).map_err(err))?;
    changed(&state);
    Ok(())
}

#[derive(Serialize)]
pub struct TrashEntry {
    account: AccountMeta,
    deleted_at: i64,
}

#[tauri::command]
pub async fn list_trash(state: State<'_, AppState>) -> R<Vec<TrashEntry>> {
    state.with_vault(|v| {
        Ok(v.data
            .trash
            .iter()
            .map(|t| TrashEntry { account: AccountMeta::from(&t.account), deleted_at: t.deleted_at })
            .collect())
    })
}

#[tauri::command]
pub async fn restore_trash(state: State<'_, AppState>, id: Uuid) -> R<()> {
    state.with_vault(|v| v.restore_from_trash(id).map_err(err))?;
    changed(&state);
    Ok(())
}

#[tauri::command]
pub async fn purge_trash_item(state: State<'_, AppState>, id: Uuid) -> R<()> {
    state.with_vault(|v| v.delete_from_trash(id).map_err(err))
}

#[tauri::command]
pub async fn generate_password(options: GeneratorOptions) -> R<String> {
    rahsa_core::generator::generate(&options).map(|p| p.to_string()).map_err(err)
}

/// F-13: copy username/password; excluded from Win+V history, cleared later.
#[tauri::command]
pub async fn copy_secret(state: State<'_, AppState>, id: Uuid, field: String) -> R<u32> {
    let (text, secs) = state.with_vault(|v| {
        let a = v.account(id).ok_or("Akun tidak ditemukan")?;
        let t = match field.as_str() {
            "username" => a.username.clone(),
            "password" => a.password.clone(),
            _ => return Err("Field tidak dikenal".into()),
        };
        Ok((zeroize::Zeroizing::new(t), v.data.settings.clipboard_clear_seconds))
    })?;
    let seq = rahsa_core::win::clipboard_set_secret(&text).map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(u64::from(secs)));
        rahsa_core::win::clipboard_clear_if(seq);
    });
    Ok(secs)
}

/// Copy a freshly generated password with the same protections as `copy_secret`.
#[tauri::command]
pub async fn copy_generated(state: State<'_, AppState>, text: String) -> R<u32> {
    let secs = state.with_vault(|v| Ok(v.data.settings.clipboard_clear_seconds)).unwrap_or(20);
    let text = zeroize::Zeroizing::new(text);
    let seq = rahsa_core::win::clipboard_set_secret(&text).map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(u64::from(secs)));
        rahsa_core::win::clipboard_clear_if(seq);
    });
    Ok(secs)
}

/// F-25: create a one-shot 30 s login intent and open the login page.
///
/// Normally the extension opens the page: it first closes the site's tabs and
/// clears that site's cookies/storage (so whoever was logged in is logged
/// out), then the content script fills and submits for the chosen account.
/// Incognito windows start clean anyway, so those are opened by the app.
#[tauri::command]
pub async fn masuk(state: State<'_, AppState>, account_id: Uuid, incognito: Option<bool>) -> R<()> {
    struct Target {
        url: String,
        name: String,
        incognito: bool,
        fresh: bool,
        origins: Vec<rahsa_core::origin::Origin>,
    }
    let t = state.with_vault(|v| {
        let a = v.account(account_id).ok_or("Akun tidak ditemukan")?;
        let origins = v.effective_origins(a);
        match a.institution_id.and_then(|i| v.institution(i)) {
            Some(i) => Ok(Target {
                url: i.login_url.clone(),
                name: i.name.clone(),
                incognito: i.open_incognito,
                fresh: i.fresh_session,
                origins,
            }),
            None => {
                let o = a.origins.first().ok_or("Akun ini belum punya link login")?;
                Ok(Target { url: o.serialize(), name: a.label.clone(), incognito: false, fresh: true, origins })
            }
        }
    })?;
    if state.pairing.lock().unwrap().extensions.is_empty() {
        return Err("Ekstensi Rahsa belum dipasangkan. Pasang ekstensi di Chrome (chrome://extensions → Load unpacked → folder extension\\dist), klik ikon Rahsa, lalu pilih Pasangkan.".into());
    }
    // Without host permission the extension cannot run on the page Chrome opens.
    let patterns: Vec<String> = t.origins.iter().map(|o| o.match_pattern()).collect();
    if let (Some(p), Some(granted)) = (patterns.first(), state.granted_patterns.lock().unwrap().as_ref()) {
        if !granted.contains(p) {
            return Err(format!(
                "Ekstensi Rahsa belum diizinkan membuka {}. Di Chrome, klik ikon Rahsa di toolbar lalu tekan Izinkan, kemudian coba Masuk lagi.",
                t.name
            ));
        }
    }
    // A new click replaces any earlier intent.
    *state.intent.lock().unwrap() = Some(LoginIntent {
        account_id,
        institution_name: t.name.clone(),
        expires_at: Instant::now() + INTENT_TTL,
        used: false,
    });

    let incognito = incognito.unwrap_or(t.incognito);
    if incognito {
        chrome::open_url(&t.url, true)?;
        return Ok(());
    }

    // Chrome not running (or the extension not yet connected): start Chrome
    // and give the extension a moment to connect through rahsa-host.
    if !state.extension_connected() {
        chrome::start()?;
        for _ in 0..50 {
            if state.extension_connected() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
    let Some(session) = state.latest_session() else {
        *state.intent.lock().unwrap() = None;
        return Err("Chrome sudah dibuka, tetapi ekstensi Rahsa belum tersambung. Pastikan ekstensi terpasang, aktif, dan sudah dipasangkan (Pengaturan → Ekstensi).".into());
    };
    let _ = state.events.send(ExtEvent::OpenLogin {
        session,
        url: t.url,
        origins: t.origins.iter().map(|o| o.serialize()).collect(),
        patterns,
        fresh: t.fresh,
    });
    Ok(())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> R<Settings> {
    state.with_vault(|v| Ok(v.data.settings.clone()))
}

#[tauri::command]
pub async fn update_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings) -> R<()> {
    state.with_vault(|v| v.update_settings(settings).map_err(err))?;
    tray::refresh(&app);
    Ok(())
}

/// F-14: export an encrypted copy. Requires the master password again.
#[tauri::command]
pub async fn export_backup(state: State<'_, AppState>, path: PathBuf, master_password: String) -> R<()> {
    let verify = {
        let g = state.vault.lock().unwrap();
        let v = g.as_ref().ok_or(crate::state::LOCKED)?;
        let path = v.path().to_path_buf();
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        bytes
    };
    blocking(move || Vault::verify_image(&verify, &master_password))
        .await?
        .map_err(|_| "Master password salah.".to_string())?;
    state.with_vault(|v| {
        v.export_to(&path).map_err(err)?;
        let mut s = v.data.settings.clone();
        s.last_backup_at = Some(now_ms());
        v.update_settings(s).map_err(err)
    })
}

/// F-14: restore a backup file, replacing the vault on this laptop. The old
/// vault (if any) is kept next to it as `vault.rahsa.before-restore-<ms>`.
#[tauri::command]
pub async fn restore_backup(app: AppHandle, state: State<'_, AppState>, path: PathBuf, master_password: String) -> R<usize> {
    let bytes = std::fs::read(&path).map_err(|e| format!("File tidak bisa dibaca: {e}"))?;
    let pw = master_password.clone();
    let check = bytes.clone();
    let count = blocking(move || Vault::verify_image(&check, &pw)).await?.map_err(|e| match e {
        Error::Decrypt => "Master password salah atau file backup rusak.".to_string(),
        other => other.to_string(),
    })?;
    state.lock(&app, "restore");
    if state.vault_path.exists() {
        let mut keep = state.vault_path.as_os_str().to_owned();
        keep.push(format!(".before-restore-{}", now_ms()));
        std::fs::copy(&state.vault_path, PathBuf::from(keep)).map_err(|e| e.to_string())?;
    }
    rahsa_core::storage::write_atomic(&state.vault_path, &bytes, true).map_err(err)?;
    let vp = state.vault_path.clone();
    let v = blocking(move || Vault::unlock(&vp, &master_password)).await?.map_err(err)?;
    *state.vault.lock().unwrap() = Some(v);
    on_unlocked(&app, &state);
    Ok(count)
}

/// F-15.
#[tauri::command]
pub async fn change_master_password(state: State<'_, AppState>, old: String, new: String) -> R<()> {
    // Argon2 runs twice here; take the vault out so the UI thread never blocks on the mutex.
    let mut v = state.vault.lock().unwrap().take().ok_or(crate::state::LOCKED)?;
    let (v, r) = blocking(move || {
        let r = v.change_master(&old, &new);
        (v, r)
    })
    .await?;
    *state.vault.lock().unwrap() = Some(v);
    r.map_err(|e| match e {
        Error::Decrypt => "Master password lama salah.".into(),
        other => other.to_string(),
    })
}

#[tauri::command]
pub async fn save_emergency_kit(state: State<'_, AppState>, path: PathBuf) -> R<()> {
    let html = include_str!("../emergency-kit.html")
        .replace("{{VAULT_PATH}}", &html_escape(&state.vault_path.to_string_lossy()))
        .replace("{{DATE}}", &chrono_like_date());
    std::fs::write(path, html).map_err(|e| e.to_string())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn chrono_like_date() -> String {
    // Days since epoch → civil date (Howard Hinnant's algorithm); avoids a date crate.
    let days = now_ms().div_euclid(86_400_000);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{d:02}-{m:02}-{y}")
}

#[derive(Serialize)]
pub struct PairingView {
    extension_id: String,
    paired_at: i64,
    last_seen_at: Option<i64>,
    connected: bool,
}

#[tauri::command]
pub async fn list_pairings(state: State<'_, AppState>) -> R<Vec<PairingView>> {
    let sessions: Vec<String> = state.sessions.lock().unwrap().values().cloned().collect();
    Ok(state
        .pairing
        .lock()
        .unwrap()
        .extensions
        .iter()
        .map(|e| PairingView {
            extension_id: e.extension_id.clone(),
            paired_at: e.paired_at,
            last_seen_at: e.last_seen_at,
            connected: sessions.contains(&e.extension_id),
        })
        .collect())
}

#[tauri::command]
pub async fn revoke_pairing(state: State<'_, AppState>, extension_id: String) -> R<()> {
    state.pairing.lock().unwrap().remove(&extension_id).map_err(|e| e.to_string())?;
    let _ = state.events.send(ExtEvent::Revoked(extension_id));
    Ok(())
}

#[tauri::command]
pub async fn respond_pairing(state: State<'_, AppState>, approve: bool) -> R<()> {
    let mut g = state.pending_pairing.lock().unwrap();
    let p = g.as_mut().ok_or("Tidak ada permintaan pairing")?;
    if let Some(tx) = p.respond.take() {
        let _ = tx.send(approve);
    }
    *g = None;
    Ok(())
}

#[derive(Serialize)]
pub struct ExtensionInfo {
    extension_id: &'static str,
    host_registered: bool,
    connected: bool,
    chrome_found: bool,
    pending_code: Option<String>,
}

#[tauri::command]
pub async fn extension_info(state: State<'_, AppState>) -> R<ExtensionInfo> {
    Ok(ExtensionInfo {
        extension_id: chrome::EXTENSION_ID,
        host_registered: chrome::host_exe().is_some(),
        connected: state.extension_connected(),
        chrome_found: chrome::chrome_exe().is_some(),
        pending_code: state.pending_pairing.lock().unwrap().as_ref().map(|p| p.code.clone()),
    })
}
