//! Background watcher: auto-lock (idle, Windows lock screen, sleep/hibernate)
//! and expiry of unused login intents (PRD F-03, F-25).

use std::time::{Duration, Instant, SystemTime};

use tauri::{AppHandle, Manager};

use crate::state::{toast, AppState, Toast, ToastAction};

const TICK: Duration = Duration::from_secs(1);
/// A wall-clock jump this large between ticks means the machine was asleep.
const SLEEP_JUMP: Duration = Duration::from_secs(15);

pub fn spawn(app: AppHandle) {
    std::thread::Builder::new()
        .name("rahsa-guard".into())
        .spawn(move || {
            let mut last_wall = SystemTime::now();
            loop {
                std::thread::sleep(TICK);
                let st = app.state::<AppState>();
                let now_wall = SystemTime::now();
                let jumped = now_wall.duration_since(last_wall).map(|d| d > SLEEP_JUMP).unwrap_or(true);
                last_wall = now_wall;

                // Bind first: the vault mutex must be released before `lock()` re-takes it.
                let open_settings = st
                    .vault
                    .lock()
                    .unwrap()
                    .as_ref()
                    .map(|v| (v.data.settings.auto_lock_minutes, v.data.settings.lock_on_screen_lock));
                if let Some((minutes, on_screen_lock)) = open_settings {
                    if jumped {
                        st.lock(&app, "sleep");
                    } else if on_screen_lock && rahsa_core::win::is_workstation_locked() {
                        st.lock(&app, "screen");
                    } else if rahsa_core::win::idle_millis() >= u64::from(minutes) * 60_000 {
                        st.lock(&app, "idle");
                    }
                }

                expire_intent(&app, &st);
            }
        })
        .expect("spawn guard thread");
}

fn expire_intent(app: &AppHandle, st: &AppState) {
    let mut g = st.intent.lock().unwrap();
    let Some(intent) = g.as_ref() else { return };
    if Instant::now() < intent.expires_at {
        return;
    }
    let unused = !intent.used;
    let (account_id, name) = (intent.account_id, intent.institution_name.clone());
    *g = None;
    drop(g);
    if unused {
        toast(
            app,
            Toast {
                kind: "warning",
                message: format!(
                    "Halaman login {name} tidak terisi dalam 30 detik. Jika Chrome masih login dengan akun lain, logout dulu atau buka di Incognito."
                ),
                action: Some(ToastAction { kind: "incognito", account_id }),
            },
        );
    }
}

/// Seconds until idle auto-lock, for the extension's `status` message.
pub fn seconds_until_lock(minutes: u32) -> u64 {
    (u64::from(minutes) * 60).saturating_sub(rahsa_core::win::idle_millis() / 1000)
}
