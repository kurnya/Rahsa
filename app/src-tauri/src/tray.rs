//! Tray icon: lock state, "Kunci sekarang", "Buka Rahsa", "Keluar" (PRD §10).

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::state::{show_main, AppState};

const TRAY_ID: &str = "rahsa-tray";

fn build_menu(app: &AppHandle, unlocked: bool) -> tauri::Result<Menu<tauri::Wry>> {
    let status = MenuItem::with_id(
        app,
        "status",
        if unlocked { "Vault terbuka" } else { "Vault terkunci" },
        false,
        None::<&str>,
    )?;
    let lock = MenuItem::with_id(app, "lock", "Kunci sekarang", unlocked, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Buka Rahsa", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Keluar", true, None::<&str>)?;
    Menu::with_items(app, &[&status, &PredefinedMenuItem::separator(app)?, &open, &lock, &quit])
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app, false)?;
    let mut b = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Rahsa — terkunci")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, ev| match ev.id().as_ref() {
            "open" => show_main(app),
            "lock" => app.state::<AppState>().lock(app, "manual"),
            "quit" => {
                app.state::<AppState>().lock(app, "quit");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        b = b.icon(icon.clone());
    }
    b.build(app)?;
    Ok(())
}

pub fn refresh(app: &AppHandle) {
    let unlocked = app.state::<AppState>().is_unlocked();
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_menu(app, unlocked) {
            let _ = tray.set_menu(Some(menu));
        }
        let _ = tray.set_tooltip(Some(if unlocked { "Rahsa — terbuka" } else { "Rahsa — terkunci" }));
    }
}
