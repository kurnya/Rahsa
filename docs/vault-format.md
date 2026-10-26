# Format file `vault.rahsa` (versi 1)

Dokumen ini sengaja terbuka (PRD §8 prinsip 7) supaya isi vault tetap bisa dibaca dengan alat lain jika aplikasi Rahsa berhenti dikembangkan. Implementasi rujukan ada di [crates/rahsa-core/src/format.rs](../crates/rahsa-core/src/format.rs) dan [vault.rs](../crates/rahsa-core/src/vault.rs).

## Susunan byte

Semua bilangan bulat little-endian.

| Offset | Panjang | Isi |
| --- | --- | --- |
| 0 | 5 | Magic ASCII `RAHSA` |
| 5 | 2 | Versi format (u16) = `1` |
| 7 | 1 | ID KDF: `1` = Argon2id v1.3 |
| 8 | 4 | Memori Argon2id dalam KiB (u32), default 262144 (256 MiB) |
| 12 | 4 | Iterasi (u32), default 3 |
| 16 | 4 | Paralelisme (u32), default 4 |
| 20 | 16 | Salt acak |
| 36 | 24 | Nonce pembungkus kunci |
| 60 | 48 | Kunci vault terbungkus: 32 byte kunci + 16 byte tag Poly1305 |
| 108 | 24 | Nonce payload (baru setiap kali disimpan) |
| 132 | sisa | Ciphertext payload + tag 16 byte |

## Cara membuka

1. `KEK = Argon2id(password = master password UTF-8, salt, m, t, p, panjang 32)`.
2. `vault_key = XChaCha20-Poly1305-Decrypt(KEK, nonce pembungkus, kunci terbungkus, AAD = byte 0..36)`.
3. `json = XChaCha20-Poly1305-Decrypt(vault_key, nonce payload, ciphertext, AAD = byte 0..132)`.

Karena seluruh header menjadi associated data, perubahan satu byte di mana pun membuat pembukaan gagal. Password salah dan file rusak sengaja tidak dibedakan.

Saat membaca, parameter KDF dibatasi (memori ≤ 1 GiB, iterasi 1–10, paralelisme 1–16) supaya file jahat tidak bisa memaksa alokasi memori tak terbatas.

## Payload JSON

```json
{
  "version": 1,
  "institutions": [
    { "id": "uuid", "name": "SIAKAD Kampus", "login_url": "https://siakad.kampus.ac.id/login",
      "origins": [{ "scheme": "https", "host": "siakad.kampus.ac.id", "port": null, "match": "exact" }],
      "icon": null, "open_incognito": false, "fresh_session": true }
  ],
  "accounts": [
    { "id": "uuid", "institution_id": "uuid", "label": "Admin Prodi", "username": "…", "password": "…",
      "origins": [], "notes": "", "custom_fields": [{ "name": "role", "value": "Operator", "selector": "#role", "secret": false }],
      "tags": ["kampus"], "auto_submit": false,
      "created_at": 1793000000000, "updated_at": 1793000000000, "last_used_at": null,
      "password_history": [{ "password": "…", "changed_at": 1793000000000 }] }
  ],
  "trash": [{ "account": { "…": "…" }, "deleted_at": 1793000000000 }],
  "settings": { "auto_lock_minutes": 10, "lock_on_screen_lock": true, "clipboard_clear_seconds": 20,
                "hello_enabled": false, "generator_defaults": { "…": "…" }, "language": "id", "last_backup_at": null }
}
```

Waktu disimpan sebagai milidetik Unix. `match` bernilai `exact` (default) atau `subdomain`.

## Penyimpanan

* Lokasi: `%APPDATA%\id.rahsa.desktop\vault.rahsa`.
* Setiap simpan: tulis `vault.rahsa.tmp` → `fsync` → geser `vault.rahsa.1..3` → salin versi aktif ke `.1` → rename atomik. Jadi selalu ada 3 versi sebelumnya.
* Backup (`rahsa-backup-YYYYMMDD.rahsa`) memakai format yang sama persis dan terbuka dengan master password yang berlaku saat backup dibuat.

## File lain

`pairing.dat` (folder yang sama) berisi kunci X25519 statis aplikasi dan public key ekstensi yang sudah dipasangkan, dilindungi DPAPI (lingkup user Windows). File ini tidak berisi data akun. Lihat catatan di [app/src-tauri/src/pairing.rs](../app/src-tauri/src/pairing.rs).
