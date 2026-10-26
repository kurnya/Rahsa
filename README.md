# Rahsa — password manager lokal (Windows + Chrome)

Implementasi MVP dari [PRD](PRD%20—%20Password%20Manager%20Lokal%20(Windows%20+%20Chrome).md). Semua data tersimpan terenkripsi di laptop; tidak ada server, cloud, telemetri, atau kode jaringan.

```
crates/rahsa-core   Format vault, Argon2id + XChaCha20-Poly1305, pencocokan origin,
                    generator, kripto sesi ekstensi, helper Win32 (DPAPI, clipboard, idle)
crates/rahsa-host   rahsa-host.exe: native messaging host, hanya meneruskan frame
app/                Aplikasi desktop Tauri 2 (Rust + React/TypeScript)
extension/          Ekstensi Chrome Manifest V3 (TypeScript, esbuild)
docs/               Format vault terbuka, checklist keamanan
```

## Arsitektur singkat

```
Chrome ── content script ──► background (service worker)
                                  │  chrome.runtime.connectNative("id.rahsa.host")
                                  ▼
                           rahsa-host.exe   (stdin/stdout ⇄ named pipe, tidak bisa membaca isi)
                                  │  \\.\pipe\rahsa-<SID user>, ACL hanya user aktif
                                  ▼
                           Rahsa (Tauri)    vault terbuka hanya di memori Rust
```

* Setelah pairing, semua pesan ekstensi ⇄ app terenkripsi end-to-end (X25519 statis + efemeral → HKDF-SHA256 → XChaCha20-Poly1305). Native host hanya melihat ciphertext.
* App memverifikasi bahwa klien pipe adalah `rahsa-host.exe` di folder aplikasi sendiri.
* `get_credential` ditolak jika origin halaman tidak terdaftar untuk akun itu, maksimal 10/menit, dengan nonce anti-replay.

## Menyiapkan lingkungan

Dibutuhkan: Rust (MSVC) + Visual Studio Build Tools (C++), Node.js 20+, Google Chrome.

```powershell
winget install Rustlang.Rustup
winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

## Menjalankan saat pengembangan

```powershell
pwsh scripts/build.ps1 -Dev        # build rahsa-host + ekstensi
cd app; npm run tauri dev          # jalankan aplikasi desktop
```

Lalu di Chrome buka `chrome://extensions`, aktifkan *Developer mode*, klik *Load unpacked*, pilih folder `extension/dist`. ID ekstensi selalu `nahifcpmliobngflogfdhojbklmomgad` (dikunci oleh `key` di manifest), dan ID inilah yang diizinkan di manifest native host.

Saat aplikasi dibuka, ia otomatis menulis manifest native host dan mendaftarkannya di `HKCU\Software\Google\Chrome\NativeMessagingHosts\id.rahsa.host`.

## Membuat installer

```powershell
pwsh scripts/build.ps1             # menghasilkan target/release/bundle/nsis/Rahsa_0.1.0_x64-setup.exe
```

Installer NSIS dipasang per user (tanpa admin). Untuk mengurangi peringatan SmartScreen, tandatangani installer dengan sertifikat code signing (lihat pertanyaan terbuka di PRD).

## Pemakaian pertama

1. Buka Rahsa, buat master password (min. 12 karakter), simpan Emergency Kit.
2. Pasang ekstensi, klik ikonnya, pilih **Pasangkan**. Cocokkan kode 6 digit di popup dengan dialog di aplikasi, lalu setujui di aplikasi.
3. Di aplikasi, **Tambah institusi** (nama + link halaman login `https://`), lalu **Tambah akun**.
4. Di popup ekstensi, klik **Izinkan** agar Rahsa boleh membaca halaman login institusi tersebut (izin per situs, bukan `<all_urls>`).
5. Klik **Masuk** pada kartu akun: Chrome membuka halaman login, ekstensi mengisi dan menekan tombol login.

Alternatif: buka web institusi sendiri, klik ikon Rahsa di field username (atau `Ctrl+Shift+L`), pilih akun.

## Pengujian

```powershell
cargo test --workspace             # kripto (vektor RFC), format, tamper, origin, generator, storage, vault
cd extension; npm test             # kecocokan kripto Rust ⇄ TypeScript + halaman jebakan di Chrome headless
```

## Penyimpangan dari PRD (disengaja)

* **Pairing disimpan di `pairing.dat` (DPAPI), bukan di dalam vault.** App harus bisa mengautentikasi ekstensi saat vault terkunci (untuk `status` dan "Buka kunci"). File ini tidak berisi data akun atau rahsa vault.
* **Izin `scripting`** ditambahkan ke ekstensi: diperlukan oleh `chrome.scripting.registerContentScripts` yang diminta PRD §8 prinsip 5.
* **Izin `browsingData`** ditambahkan untuk fitur ganti akun: tombol Masuk menutup tab situs institusi dan menghapus cookie/penyimpanan situs itu saja sebelum login dengan akun yang dipilih (bisa dimatikan per institusi).
* **Verifikasi tanda tangan `rahsa-host.exe`** saat ini berupa pencocokan path biner (harus berada di folder aplikasi). Verifikasi Authenticode ditambahkan setelah ada sertifikat code signing.
* Fitur v1/v2 (Windows Hello, tawarkan simpan akun, impor CSV, audit kesehatan, SSO bertahap, TOTP) belum dikerjakan. Riwayat 5 password terakhir (F-18) sudah tersimpan karena murah dibuat sejak awal.
