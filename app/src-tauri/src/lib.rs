//! Rahsa desktop app: holds the unlocked vault (in Rust memory only), serves
//! the UI over Tauri IPC and the extension over a per-user named pipe.

mod chrome;
mod commands;
mod guard;
mod ipc;
mod pairing;
mod state;
mod tray;

use tauri::Manager;

pub use state::AppState;

pub fn run() {
    let start_hidden = std::env::args().any(|a| a == "--tray");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            state::show_main(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let mut data_dir = app.path().app_data_dir()?;
            // Debug builds only: a throwaway data folder for UI testing.
            if cfg!(debug_assertions) {
                if let Ok(dir) = std::env::var("RAHSA_DATA_DIR") {
                    data_dir = dir.into();
                }
            }
            std::fs::create_dir_all(&data_dir)?;
            let st = AppState::new(&data_dir);
            app.manage(st);

            if let Err(e) = chrome::register_native_host(&data_dir) {
                eprintln!("registrasi native host gagal: {e}");
            }
            tray::create(app.handle())?;
            guard::spawn(app.handle().clone());
            ipc::spawn(app.handle().clone());

            if !start_hidden {
                state::show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps Rahsa in the tray so the extension keeps working.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_status,
            commands::create_vault,
            commands::unlock,
            commands::lock,
            commands::overview,
            commands::reveal_password,
            commands::reveal_custom_field,
            commands::password_history,
            commands::save_institution,
            commands::delete_institution,
            commands::save_account,
            commands::delete_account,
            commands::list_trash,
            commands::restore_trash,
            commands::purge_trash_item,
            commands::generate_password,
            commands::copy_secret,
            commands::copy_generated,
            commands::masuk,
            commands::get_settings,
            commands::update_settings,
            commands::export_backup,
            commands::restore_backup,
            commands::change_master_password,
            commands::save_emergency_kit,
            commands::list_pairings,
            commands::revoke_pairing,
            commands::respond_pairing,
            commands::extension_info,
        ])
        .run(tauri::generate_context!())
        .expect("gagal menjalankan Rahsa");
}
