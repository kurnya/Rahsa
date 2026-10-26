//! `rahsa-host.exe` — started by Chrome via native messaging.
//!
//! It holds no secrets and understands nothing beyond framing: every frame is
//! a 4-byte little-endian length followed by UTF-8 JSON, both on Chrome's
//! stdin/stdout and on the app's named pipe. After pairing the frames are
//! end-to-end encrypted between the extension and the app, so the host only
//! ever sees ciphertext.
//!
//! The first frame it sends to the app announces which extension started it
//! (Chrome passes the caller origin as the first argument).

#[cfg(windows)]
mod relay {
    use std::path::PathBuf;
    use std::time::Duration;

    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
    use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};

    /// Chrome limits host → extension messages to 1 MB.
    const MAX_FRAME: usize = 1024 * 1024;
    const ERROR_PIPE_BUSY: i32 = 231;
    const ERROR_FILE_NOT_FOUND: i32 = 2;

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

    fn app_exe() -> Option<PathBuf> {
        let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
        ["rahsa.exe", "Rahsa.exe"].iter().map(|n| dir.join(n)).find(|p| p.exists())
    }

    async fn connect(name: &str) -> std::io::Result<NamedPipeClient> {
        let mut launched = false;
        for _ in 0..100 {
            match ClientOptions::new().open(name) {
                Ok(c) => return Ok(c),
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) => {}
                Err(e) if e.raw_os_error() == Some(ERROR_FILE_NOT_FOUND) => {
                    // App not running: start it hidden in the tray, then retry.
                    if !launched {
                        launched = true;
                        if let Some(exe) = app_exe() {
                            let _ = std::process::Command::new(exe).arg("--tray").spawn();
                        }
                    }
                }
                Err(e) => return Err(e),
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "aplikasi Rahsa tidak merespons"))
    }

    pub async fn run() -> std::io::Result<()> {
        let caller = std::env::args().nth(1).unwrap_or_default();
        let mut stdout = tokio::io::stdout();
        let pipe = match connect(&rahsa_core::win::pipe_name()?).await {
            Ok(p) => p,
            Err(e) => {
                let msg = serde_json::json!({ "type": "host_error", "error": "app_unavailable", "detail": e.to_string() });
                write_frame(&mut stdout, msg.to_string().as_bytes()).await?;
                return Err(e);
            }
        };
        let (mut pipe_r, mut pipe_w) = tokio::io::split(pipe);
        let hello = serde_json::json!({ "type": "_host_hello", "caller": caller });
        write_frame(&mut pipe_w, hello.to_string().as_bytes()).await?;

        let up = async {
            let mut stdin = tokio::io::stdin();
            while let Some(f) = read_frame(&mut stdin).await? {
                write_frame(&mut pipe_w, &f).await?;
            }
            Ok::<_, std::io::Error>(())
        };
        let down = async {
            while let Some(f) = read_frame(&mut pipe_r).await? {
                write_frame(&mut stdout, &f).await?;
            }
            Ok::<_, std::io::Error>(())
        };
        // Whichever side closes first ends the relay (Chrome kills us on disconnect).
        tokio::select! {
            r = up => r,
            r = down => r,
        }
    }
}

#[cfg(windows)]
fn main() {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("tokio runtime");
    if rt.block_on(relay::run()).is_err() {
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("rahsa-host hanya untuk Windows");
    std::process::exit(1);
}
