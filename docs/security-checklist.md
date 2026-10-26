# Checklist keamanan (PRD §8, syarat lulus Fase 3)

Status per butir. `Uji` menunjuk ke test otomatis yang membuktikannya.

| # | Prinsip / ancaman | Implementasi | Uji |
| --- | --- | --- | --- |
| 1 | Master password & kunci vault tidak pernah ke disk/log | Kunci hanya di `Vault.key` (`Zeroizing`), tidak ada logging; password UI dikirim sekali ke command `unlock` | Review kode |
| 2 | Semua kripto di Rust | WebView hanya memanggil command; ekstensi hanya kripto sesi (bukan vault) | — |
| 3 | Tauri dikunci | CSP ketat tanpa sumber eksternal, `freezePrototype`, capability hanya event + dialog file | `tauri.conf.json`, `capabilities/default.json` |
| 4 | Tanpa kode jaringan | Tidak ada dependensi HTTP; app hanya named pipe lokal (`reject_remote_clients`) | `cargo tree` |
| 5 | Izin ekstensi minimum | `nativeMessaging`, `activeTab`, `storage`, `scripting`; host `https://*/*` hanya opsional dan diminta per situs | `manifest.json` |
| 6 | Dependensi diaudit & dikunci | `Cargo.lock`, `package-lock.json`; jalankan `cargo audit` dan `npm audit` sebelum rilis | Manual |
| 7 | Format vault terbuka | [vault-format.md](vault-format.md) | — |
| A | File vault dicuri | Argon2id 256 MiB/t=3/p=4, XChaCha20-Poly1305, header sebagai AAD; label/username/domain ikut terenkripsi | `vault::file_does_not_leak_plaintext`, `any_flipped_byte_is_rejected` |
| B | Phishing | Origin persis, wajib https, tanpa trailing dot/IDN mirip/port lain; dicek di ekstensi **dan** app | `origin::origin_table_from_prd` |
| C | Form tersembunyi / iframe | Tidak mengisi field tak terlihat, off-screen, opacity 0, atau di iframe beda origin; klik sintetis (`isTrusted=false`) diabaikan | `extension/test/form-cases.ts` |
| D | Ekstensi palsu | `allowed_origins` hanya ID Rahsa; app menolak `_host_hello` dari ID lain; pairing X25519 + kode 6 digit | `session::*` |
| E | Proses lain ke pipe | DACL hanya SID user aktif; PID klien harus `rahsa-host.exe` di folder app | Review kode `ipc.rs` |
| F | Tebak master password | Jeda 1, 2, 4 … 30 detik; auto-lock idle/layar terkunci/sleep | `commands::unlock`, `guard.rs` |
| G | Clipboard bocor | Format `ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory=0`, `CanUploadToCloudClipboard=0`; dikosongkan setelah 20 detik bila belum diganti | `win::clipboard_set_secret` |
| H | Replay / flood | Nonce unik per request, AEAD per frame, arah pesan terikat di AAD, `get_credential` maks 10/menit | `session::both_sides_agree` |
| I | Mati listrik saat simpan | Tulis tmp → fsync → rename atomik + 3 versi sebelumnya | `storage::*` |
| J | File vault jahat | Parser tanpa panic, batas ukuran, batas parameter KDF | `format::garbage_never_panics`, `crypto::rejects_hostile_kdf_params` |

Sebelum rilis:

- [ ] `cargo audit` dan `npm audit` bersih
- [ ] Installer ditandatangani (atau keputusan tertulis untuk pemakaian pribadi)
- [ ] Install bersih di laptop kedua, restore dari backup berhasil
- [ ] Login ke semua web institusi berhasil; halaman phishing lokal ditolak
- [ ] Review kode oleh pihak kedua untuk `rahsa-core` dan `ipc.rs`
