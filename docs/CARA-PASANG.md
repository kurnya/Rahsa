# Cara memasang Rahsa

Paket Rahsa berisi dua file:

- `Rahsa_<versi>_x64-setup.exe`: aplikasi desktop untuk Windows 10/11 (64-bit)
- `rahsa-extension-<versi>.zip`: ekstensi untuk Google Chrome

## 1. Pasang aplikasi

1. Jalankan `Rahsa_<versi>_x64-setup.exe`. Tidak perlu hak admin.
2. Jika muncul **"Windows protected your PC"**, klik **More info**, lalu **Run anyway**. Peringatan ini muncul karena installer belum ditandatangani dengan sertifikat berbayar.
3. Buka **Rahsa** dari Start Menu, buat master password, lalu simpan Emergency Kit.

## 2. Pasang ekstensi Chrome

1. Ekstrak `rahsa-extension-<versi>.zip` ke folder yang **tidak akan dihapus atau dipindah**, misalnya `C:\Rahsa\extension`. Chrome memuat ekstensi langsung dari folder ini.
2. Buka `chrome://extensions` di Chrome.
3. Nyalakan **Developer mode** (pojok kanan atas).
4. Klik **Load unpacked**, lalu pilih folder hasil ekstrak tadi (folder yang berisi `manifest.json`).
5. Klik ikon puzzle 🧩 di toolbar, lalu pin 📌 **Rahsa**.

> Chrome kadang menampilkan peringatan tentang ekstensi dalam mode developer saat dibuka. Itu normal untuk ekstensi yang tidak dipasang dari Chrome Web Store.

## 3. Pasangkan ekstensi dengan aplikasi

1. Pastikan aplikasi Rahsa terbuka dan vault sudah dibuka kuncinya.
2. Klik ikon Rahsa di Chrome, lalu **Pasangkan**.
3. Cocokkan kode 6 digit di popup dengan yang tampil di aplikasi, lalu klik **Kodenya sama, pasangkan** di aplikasi.

## 4. Mulai pakai

1. Di aplikasi: **Tambah institusi** (nama + link halaman login), lalu **Tambah akun**.
2. Di popup ekstensi, klik **Izinkan** untuk situs tersebut.
3. Klik **Masuk** pada kartu akun.

Ekstensi hanya bekerja selama aplikasi Rahsa terbuka.

## Memperbarui ke versi baru

- **Aplikasi:** jalankan installer versi baru. Data tetap aman di `%APPDATA%\id.rahsa.desktop\`.
- **Ekstensi:** ekstrak zip baru ke folder yang **sama** (timpa isinya), lalu klik 🔄 pada kartu Rahsa di `chrome://extensions`. Pairing tidak perlu diulang.

## Penting

- Master password **tidak bisa dipulihkan** oleh siapa pun. Simpan Emergency Kit dan buat backup rutin (Pengaturan → Backup & pemulihan).
- Rahsa hanya mengisi password di alamat `https` yang Anda daftarkan persis.
