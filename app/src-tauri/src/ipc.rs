//! Named-pipe server speaking the extension protocol (PRD §9).
//!
//! Each connection is one `rahsa-host.exe` started by Chrome. The pipe's DACL
//! admits only the current user, and the client process must be the
//! `rahsa-host.exe` that ships next to this app.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rahsa_core::mask::username_hint;
use rahsa_core::session::{self, b64, unb64, AAD_APP_TO_EXT, AAD_EXT_TO_APP};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;
use x25519_dalek::PublicKey;
use zeroize::Zeroizing;

use crate::state::{show_main, toast, AppState, ExtEvent, PendingPairing, Toast, ToastAction};

const MAX_FRAME: usize = 1024 * 1024;
const PROTOCOL_VERSION: u32 = 1;
const CREDENTIALS_PER_MINUTE: usize = 10;
const PAIRING_TIMEOUT: Duration = Duration::from_secs(120);

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = serve(app).await {
            eprintln!("pipe server berhenti: {e}");
        }
    });
}

fn create_server(name: &str, first: bool) -> std::io::Result<NamedPipeServer> {
    let sec = rahsa_core::win::OwnerOnlySecurity::new()?;
    let mut opts = ServerOptions::new();
    opts.first_pipe_instance(first).reject_remote_clients(true);
    // SAFETY: `sec` outlives the call; the OS copies the descriptor.
    unsafe { opts.create_with_security_attributes_raw(name, &sec.attrs as *const _ as *mut _) }
}

async fn serve(app: AppHandle) -> std::io::Result<()> {
    let name = rahsa_core::win::pipe_name()?;
    let mut server = create_server(&name, true)?;
    loop {
        server.connect().await?;
        let client = server;
        server = create_server(&name, false)?;
        if !client_is_our_host(&client) {
            drop(client);
            continue;
        }
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = handle_connection(app, client).await;
        });
    }
}

fn client_is_our_host(pipe: &NamedPipeServer) -> bool {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Pipes::GetNamedPipeClientProcessId;
    let mut pid = 0u32;
    // SAFETY: valid pipe handle owned by `pipe`.
    if unsafe { GetNamedPipeClientProcessId(pipe.as_raw_handle() as _, &mut pid) } == 0 {
        return false;
    }
    let (Ok(client), Some(expected)) = (rahsa_core::win::process_image_path(pid), crate::chrome::host_exe()) else {
        return false;
    };
    let canon = |p: &std::path::Path| std::fs::canonicalize(p).ok();
    canon(&client).is_some() && canon(&client) == canon(&expected)
}

async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> std::io::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let n = u32::from_le_bytes(len) as usize;
    if n > MAX_FRAME {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "frame terlalu besar"));
    }
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf).await?;
    Ok(Some(buf))
}

async fn write_frame<W: AsyncWrite + Unpin>(w: &mut W, data: &[u8]) -> std::io::Result<()> {
    w.write_all(&(data.len() as u32).to_le_bytes()).await?;
    w.write_all(data).await?;
    w.flush().await
}

struct Conn {
    id: u64,
    extension_id: Option<String>,
    key: Option<Zeroizing<[u8; 32]>>,
    seen_nonces: HashSet<String>,
    out: mpsc::UnboundedSender<Vec<u8>>,
}

impl Conn {
    fn send_plain(&self, v: Value) {
        let _ = self.out.send(v.to_string().into_bytes());
    }

    fn send_enc(&self, v: Value) {
        let Some(key) = &self.key else { return };
        let pt = Zeroizing::new(v.to_string().into_bytes());
        let (n, c) = session::seal_json(key, AAD_APP_TO_EXT, &pt);
        self.send_plain(json!({ "type": "enc", "n": n, "c": c }));
    }
}

async fn handle_connection(app: AppHandle, pipe: NamedPipeServer) -> std::io::Result<()> {
    let (mut rd, mut wr) = tokio::io::split(pipe);
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (in_tx, mut in_rx) = mpsc::unbounded_channel::<Vec<u8>>();

    let writer = tauri::async_runtime::spawn(async move {
        while let Some(f) = out_rx.recv().await {
            if write_frame(&mut wr, &f).await.is_err() {
                break;
            }
        }
    });
    let reader = tauri::async_runtime::spawn(async move {
        while let Ok(Some(f)) = read_frame(&mut rd).await {
            if in_tx.send(f).is_err() {
                break;
            }
        }
    });

    let mut conn = Conn {
        id: NEXT_SESSION.fetch_add(1, Ordering::Relaxed),
        extension_id: None,
        key: None,
        seen_nonces: HashSet::new(),
        out: out_tx,
    };
    let mut events = app.state::<AppState>().events.subscribe();

    loop {
        tokio::select! {
            frame = in_rx.recv() => {
                let Some(frame) = frame else { break };
                let Ok(msg) = serde_json::from_slice::<Value>(&frame) else { continue };
                if !handle_frame(&app, &mut conn, msg).await {
                    break;
                }
            }
            ev = events.recv() => {
                match ev {
                    Ok(ExtEvent::Locked) => conn.send_enc(json!({ "type": "locked" })),
                    Ok(ExtEvent::Unlocked) => conn.send_enc(json!({ "type": "unlocked" })),
                    Ok(ExtEvent::VaultChanged) => conn.send_enc(json!({ "type": "vault_changed" })),
                    Ok(ExtEvent::OpenLogin { session, url, origins, patterns, fresh }) if session == conn.id => {
                        conn.send_enc(json!({ "type": "open_login", "url": url, "origins": origins, "patterns": patterns, "fresh": fresh }));
                    }
                    Ok(ExtEvent::Revoked(id)) if conn.extension_id.as_deref() == Some(&id) => {
                        conn.send_plain(json!({ "type": "error", "error": "not_paired" }));
                        break;
                    }
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(_) => break,
                }
            }
        }
    }

    app.state::<AppState>().sessions.lock().unwrap().remove(&conn.id);
    drop(conn);
    reader.abort();
    let _ = writer.await;
    Ok(())
}

/// Returns false to close the connection.
async fn handle_frame(app: &AppHandle, conn: &mut Conn, msg: Value) -> bool {
    let st = app.state::<AppState>();
    let ty = msg["type"].as_str().unwrap_or_default();
    let id = msg["id"].clone();

    match ty {
        "_host_hello" => {
            // Sent by rahsa-host with Chrome's caller origin; only honoured once.
            if conn.extension_id.is_some() {
                return false;
            }
            let caller = msg["caller"].as_str().unwrap_or_default();
            let ext = caller.strip_prefix("chrome-extension://").map(|s| s.trim_end_matches('/').to_string());
            match ext {
                Some(e) if crate::chrome::allowed_extension_ids().contains(&e) => conn.extension_id = Some(e),
                _ => return false,
            }
        }
        _ if conn.extension_id.is_none() => return false,
        "hello" => {
            let paired = st.pairing.lock().unwrap().find(conn.extension_id.as_ref().unwrap()).is_some();
            conn.send_plain(json!({
                "id": id, "type": "hello",
                "protocol": PROTOCOL_VERSION,
                "app_version": env!("CARGO_PKG_VERSION"),
                "state": if st.is_unlocked() { "unlocked" } else { "locked" },
                "paired": paired,
            }));
        }
        "pair_start" => {
            let Ok(ext_pub) = unb64::<32>(msg["ext_pub"].as_str().unwrap_or_default()) else {
                conn.send_plain(json!({ "id": id, "type": "error", "error": "bad_request" }));
                return true;
            };
            let app_pub = st.pairing.lock().unwrap().public();
            let code = session::pairing_code(&ext_pub, &app_pub);
            let (tx, rx) = oneshot::channel();
            *st.pending_pairing.lock().unwrap() = Some(PendingPairing {
                extension_id: conn.extension_id.clone().unwrap(),
                ext_pub,
                code: code.clone(),
                respond: Some(tx),
            });
            show_main(app);
            let _ = app.emit("pairing-request", json!({ "code": code }));
            conn.send_plain(json!({ "id": id, "type": "pair_pending", "app_pub": b64(&app_pub), "code": code }));

            // The approval arrives on this request's own oneshot: a newer
            // pair_start replaces the pending entry and drops our sender, so
            // an approval can never apply to a different key.
            let out = conn.out.clone();
            let app2 = app.clone();
            let ext_id = conn.extension_id.clone().unwrap();
            tauri::async_runtime::spawn(async move {
                let approved = matches!(tokio::time::timeout(PAIRING_TIMEOUT, rx).await, Ok(Ok(true)));
                let st = app2.state::<AppState>();
                let ok = approved && st.pairing.lock().unwrap().add(&ext_id, &ext_pub).is_ok();
                let mut pending = st.pending_pairing.lock().unwrap();
                if pending.as_ref().is_some_and(|p| p.ext_pub == ext_pub) {
                    *pending = None;
                }
                drop(pending);
                let _ = app2.emit("pairing-finished", ok);
                let _ = out.send(json!({ "type": "pair_result", "ok": ok }).to_string().into_bytes());
            });
        }
        "session_init" => {
            let ext_id = conn.extension_id.clone().unwrap();
            let parsed = (|| {
                Some((
                    unb64::<32>(msg["ext_pub"].as_str()?).ok()?,
                    unb64::<32>(msg["eph_pub"].as_str()?).ok()?,
                    unb64::<16>(msg["nonce"].as_str()?).ok()?,
                ))
            })();
            let Some((ext_pub, eph_pub, ext_nonce)) = parsed else {
                conn.send_plain(json!({ "id": id, "type": "error", "error": "bad_request" }));
                return true;
            };
            let mut pairing = st.pairing.lock().unwrap();
            let paired = pairing.find(&ext_id).is_some_and(|e| e.public_key == b64(&ext_pub));
            if !paired {
                conn.send_plain(json!({ "id": id, "type": "error", "error": "not_paired" }));
                return true;
            }
            let app_eph = session::generate_secret();
            let app_nonce = rahsa_core::crypto::random_bytes::<16>();
            match session::derive_session_key(pairing.secret(), &ext_pub, &app_eph, &eph_pub, &ext_nonce, &app_nonce) {
                Ok(key) => {
                    pairing.touch(&ext_id);
                    drop(pairing);
                    conn.key = Some(key);
                    conn.seen_nonces.clear();
                    st.sessions.lock().unwrap().insert(conn.id, ext_id);
                    conn.send_plain(json!({
                        "id": id, "type": "session",
                        "eph_pub": b64(PublicKey::from(&app_eph).as_bytes()),
                        "nonce": b64(&app_nonce),
                        "state": if st.is_unlocked() { "unlocked" } else { "locked" },
                    }));
                }
                Err(_) => conn.send_plain(json!({ "id": id, "type": "error", "error": "bad_request" })),
            }
        }
        "enc" => {
            let Some(key) = &conn.key else {
                conn.send_plain(json!({ "type": "error", "error": "no_session" }));
                return true;
            };
            let opened = session::open_json(
                key,
                AAD_EXT_TO_APP,
                msg["n"].as_str().unwrap_or_default(),
                msg["c"].as_str().unwrap_or_default(),
            );
            let Ok(pt) = opened else { return false };
            let Ok(req) = serde_json::from_slice::<Value>(&pt) else { return true };
            let resp = handle_request(app, conn, &req).await;
            let mut resp = resp;
            resp["id"] = req["id"].clone();
            conn.send_enc(resp);
        }
        _ => conn.send_plain(json!({ "id": id, "type": "error", "error": "unknown_type" })),
    }
    true
}

fn error(code: &str) -> Value {
    json!({ "type": "error", "error": code })
}

async fn handle_request(app: &AppHandle, conn: &mut Conn, req: &Value) -> Value {
    let st = app.state::<AppState>();

    // Anti-replay: each request carries a unique nonce within the session.
    let Some(nonce) = req["nonce"].as_str().filter(|n| (8..=64).contains(&n.len())) else {
        return error("bad_request");
    };
    if !conn.seen_nonces.insert(nonce.to_string()) {
        return error("replay");
    }
    if conn.seen_nonces.len() > 10_000 {
        return error("session_exhausted");
    }

    let ty = req["type"].as_str().unwrap_or_default();
    let origin = req["origin"].as_str().unwrap_or_default();

    match ty {
        "status" => {
            let g = st.vault.lock().unwrap();
            json!({
                "type": "status",
                "state": if g.is_some() { "unlocked" } else { "locked" },
                "auto_lock_secs": g.as_ref().map(|v| crate::guard::seconds_until_lock(v.data.settings.auto_lock_minutes)),
            })
        }
        "request_unlock" => {
            show_main(app);
            let _ = app.emit("unlock-requested", ());
            json!({ "type": "ok" })
        }
        "list_origins" => st
            .with_vault(|v| {
                let mut patterns: Vec<String> = Vec::new();
                let mut push = |o: &rahsa_core::origin::Origin| {
                    let p = o.match_pattern();
                    if !patterns.contains(&p) {
                        patterns.push(p);
                    }
                };
                v.data.institutions.iter().flat_map(|i| i.origins.iter()).for_each(&mut push);
                v.data.accounts.iter().flat_map(|a| a.origins.iter()).for_each(&mut push);
                Ok(json!({ "type": "origins", "patterns": patterns }))
            })
            .unwrap_or_else(|_| error("locked")),
        "list_accounts" => st
            .with_vault(|v| {
                let list: Vec<Value> = v
                    .accounts_for_origin(origin)
                    .iter()
                    .map(|a| json!({ "id": a.id, "label": a.label, "username_hint": username_hint(&a.username) }))
                    .collect();
                Ok(json!({ "type": "accounts", "accounts": list }))
            })
            .unwrap_or_else(|_| error("locked")),
        "get_credential" => get_credential(app, req, origin),
        "pending_login" => {
            if !st.is_unlocked() {
                return error("locked");
            }
            let mut g = st.intent.lock().unwrap();
            let Some(intent) = g.as_mut().filter(|i| !i.used && Instant::now() < i.expires_at) else {
                return json!({ "type": "pending_login" });
            };
            let matches = st
                .with_vault(|v| Ok(v.account(intent.account_id).is_some_and(|a| v.account_matches(a, origin))))
                .unwrap_or(false);
            if !matches {
                return json!({ "type": "pending_login" });
            }
            intent.used = true;
            json!({ "type": "pending_login", "account_id": intent.account_id, "submit": true })
        }
        "login_report" => {
            let ok = req["ok"].as_bool().unwrap_or(false);
            if !ok {
                let reason = req["reason"].as_str().unwrap_or_default();
                let account_id = req["account_id"].as_str().and_then(|s| Uuid::parse_str(s).ok());
                let message = match reason {
                    "no_form" => "Form login tidak ditemukan di halaman. Mungkin Chrome masih login dengan akun lain: logout dulu, atau buka di Incognito.",
                    "captcha" => "Form terisi, tetapi halaman meminta captcha. Selesaikan captcha lalu tekan login sendiri.",
                    _ => "Pengisian otomatis tidak selesai. Periksa halaman login di Chrome.",
                };
                toast(app, Toast {
                    kind: "warning",
                    message: message.into(),
                    action: account_id.filter(|_| reason == "no_form").map(|account_id| ToastAction { kind: "incognito", account_id }),
                });
            }
            json!({ "type": "ok" })
        }
        "permissions" => {
            let granted: Vec<String> = req["granted"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            *st.granted_patterns.lock().unwrap() = Some(granted);
            json!({ "type": "ok" })
        }
        "open_app" => {
            show_main(app);
            json!({ "type": "ok" })
        }
        _ => error("unknown_type"),
    }
}

/// PRD §9 rules: refused when the origin is not registered for that account,
/// and at most 10 per minute.
fn get_credential(app: &AppHandle, req: &Value, origin: &str) -> Value {
    let st = app.state::<AppState>();
    let Some(account_id) = req["account_id"].as_str().and_then(|s| Uuid::parse_str(s).ok()) else {
        return error("bad_request");
    };
    {
        let mut log = st.credential_log.lock().unwrap();
        let cutoff = Instant::now() - Duration::from_secs(60);
        while log.front().is_some_and(|t| *t < cutoff) {
            log.pop_front();
        }
        if log.len() >= CREDENTIALS_PER_MINUTE {
            return error("rate_limited");
        }
        log.push_back(Instant::now());
    }
    let r = st.with_vault(|v| {
        let a = v.account(account_id).ok_or("not_found")?;
        if !v.account_matches(a, origin) {
            return Err("origin_mismatch".into());
        }
        let fields: Vec<Value> = a
            .custom_fields
            .iter()
            .map(|f| json!({ "name": f.name, "value": f.value, "selector": f.selector }))
            .collect();
        let resp = json!({
            "type": "credential",
            "username": a.username,
            "password": a.password,
            "custom_fields": fields,
            "auto_submit": a.auto_submit,
        });
        v.mark_used(account_id).map_err(|e| e.to_string())?;
        Ok(resp)
    });
    match r {
        Ok(v) => v,
        Err(e) if e == crate::state::LOCKED => error("locked"),
        Err(e) => error(&e),
    }
}
