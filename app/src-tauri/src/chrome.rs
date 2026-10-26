//! Chrome integration: native-host registration and launching login pages.

use std::path::{Path, PathBuf};

/// Fixed by the `key` in `extension/manifest.json`.
pub const EXTENSION_ID: &str = "nahifcpmliobngflogfdhojbklmomgad";
pub const HOST_NAME: &str = "id.rahsa.host";

pub fn host_exe() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let p = dir.join("rahsa-host.exe");
    p.exists().then_some(p)
}

/// Extension IDs allowed to talk to the host. `RAHSA_EXTRA_EXTENSION_ID` lets a
/// developer add the ID of a differently-keyed build without recompiling.
pub fn allowed_extension_ids() -> Vec<String> {
    let mut v = vec![EXTENSION_ID.to_string()];
    if let Ok(extra) = std::env::var("RAHSA_EXTRA_EXTENSION_ID") {
        let extra = extra.trim().to_string();
        if extra.len() == 32 && extra.chars().all(|c| ('a'..='p').contains(&c)) {
            v.push(extra);
        }
    }
    v
}

/// Write the host manifest and point `HKCU\…\NativeMessagingHosts` at it.
/// Runs on every start so moving/reinstalling the app self-heals.
pub fn register_native_host(data_dir: &Path) -> Result<(), String> {
    let exe = host_exe().ok_or("rahsa-host.exe tidak ditemukan di folder aplikasi")?;
    let manifest = serde_json::json!({
        "name": HOST_NAME,
        "description": "Rahsa native messaging host",
        "path": exe,
        "type": "stdio",
        "allowed_origins": allowed_extension_ids()
            .iter()
            .map(|id| format!("chrome-extension://{id}/"))
            .collect::<Vec<_>>(),
    });
    let manifest_path = data_dir.join(format!("{HOST_NAME}.json"));
    std::fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest).unwrap()).map_err(|e| e.to_string())?;

    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    for browser in [r"Software\Google\Chrome\NativeMessagingHosts", r"Software\Microsoft\Edge\NativeMessagingHosts"] {
        let (key, _) = hkcu.create_subkey(format!(r"{browser}\{HOST_NAME}")).map_err(|e| e.to_string())?;
        key.set_value("", &manifest_path.to_string_lossy().to_string()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn chrome_exe() -> Option<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let app_paths = r"Software\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe";
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        if let Ok(k) = RegKey::predef(root).open_subkey(app_paths) {
            if let Ok(p) = k.get_value::<String, _>("") {
                let p = PathBuf::from(p.trim_matches('"'));
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }
    let mut candidates = vec![
        PathBuf::from(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
        PathBuf::from(r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe"),
    ];
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local).join(r"Google\Chrome\Application\chrome.exe"));
    }
    candidates.into_iter().find(|p| p.exists())
}

/// Start Chrome (or open a new window if it is running) without a URL, so the
/// extension wakes up and connects.
pub fn start() -> Result<(), String> {
    let exe = chrome_exe().ok_or("Google Chrome tidak ditemukan di laptop ini")?;
    std::process::Command::new(exe).spawn().map(|_| ()).map_err(|e| format!("Gagal membuka Chrome: {e}"))
}

/// PRD §9 "Aturan tombol Masuk": only a stored `https://` URL, passed to
/// `chrome.exe` as a single argument — never through a shell.
pub fn open_url(url: &str, incognito: bool) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|_| "Link login tidak valid".to_string())?;
    if parsed.scheme() != "https" {
        return Err("Link login harus diawali https://".into());
    }
    let exe = chrome_exe().ok_or("Google Chrome tidak ditemukan di laptop ini")?;
    let mut cmd = std::process::Command::new(exe);
    if incognito {
        cmd.arg("--incognito");
    }
    cmd.arg(parsed.as_str());
    cmd.spawn().map(|_| ()).map_err(|e| format!("Gagal membuka Chrome: {e}"))
}
