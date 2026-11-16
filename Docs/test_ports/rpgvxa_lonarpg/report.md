# Laporan Pengujian Porting: Lona RPG

## Informasi Game
- **Judul Game:** LONA RPG (B.0.10.8.05.11465AD)
- **Engine / Versi:** RPG Maker VX Ace (RGSS3 / Ruby 1.9.3 + mkxp-z Native Player)
- **Tanggal Uji:** 2026-09-20
- **Perangkat / Emulator:** MuMu Player (Android 12 x86_64, Native ARM64 Translation)
- **Versi Build RuntimePack:** APK SHA-256 `6233B7BD86F6C8ED113EBC4171E15477AB9F8282B99B8F81C445E26E76A6F576`

---

## Status Akhir
- **Durasi Playtest:** Sedang Berjalan (Boot, Title Screen, Cheat Menu terverifikasi)
- **Status Akhir:** `KENDALA (IN PROGRESS)` → Menunggu Playtest Lengkap 30 Menit oleh User
> **Update 2026-09-30:** Menunggu playtest manual user (30 menit) sejak 2026-09-20; build tersedia dan stabil menurut audit statis. Laporan ini menunggu konfirmasi gameplay dari pemilik project.

---

## Hasil Evaluasi Kriteria
- [x] **Booting & Title Screen:** Lolos 100%. Game berhasil booting dari `game.pack` (15.789 file) dan merender Title Screen interaktif lengkap.
- [x] **Audio Engine:** Lolos. BGM intro kota berputar normal via OpenAL Soft.
- [x] **Kontrol (Virtual Pad & Fisik):** Lolos. Virtual Pad touch (D-Pad, A, B, Dash, Menu, Skip, Q, E) dan keyboard fisik (mapping Win32 VK buffer) tersinkronisasi.
- [x] **Grafis & Font:** Lolos. Resolusi font Noto Sans CJK TC via `fontSub` sukses memperbaiki karakter tofu/kotak.
- [x] **Save & Load:** Lolos. Fitur *Open Save Menu Anywhere* dan *Quick Save (SavQuick)* telah diintegrasikan.
- [ ] **Stabilitas 30 Menit:** Sedang dalam proses playtest oleh User.

---

## Catatan Kendala Teknis & Solusi yang Diterapkan
1. **JSON Parser Recursion Crash:**
   - *Kendala:* Skrip `101_jsonEnDecoder.rb` rekursif menyebabkan `SystemStackError: stack level too deep` saat membaca konfigurasi.
   - *Solusi:* `GpackExtractor.kt` otomatis mem-patch parser JSON menjadi non-rekursif line-based saat proses ekstraksi internal.
2. **Win32API & Hime AllKey Keyboard Polling:**
   - *Kendala:* Skrip Hime AllKey membaca keyboard Windows via `GetKeyboardState` dari buffer memori pointer Win32, sehingga keyboard Android biasa tidak terbaca.
   - *Solusi:* `runtimepack_bootstrap.rb` mengimplementasikan shimming `DL::CPtr` dan `Win32API#GetKeyboardState`, disinkronkan langsung dengan buffer atomic `rp_input_state.bin` dari `RgssActivity.kt`.
3. **Numpad Arrow "Ngintip" (Camera Peek) vs Jalan:**
   - *Kendala:* Numpad 8/2/4/6 di Android terbaca sebagai keycode numpad sehingga Lona RPG memicu fungsi kamera peek (`Input.numpad_dir4 != 0`).
   - *Solusi:* `RgssActivity.kt` memetakan tombol numpad menjadi Virtual Key arah jalan (`VK_UP/DOWN/LEFT/RIGHT`) kecuali jika tombol ALT sedang ditahan.
4. **God Mode & 1-Hit Kill:**
   - *Kendala:* `immune_damage` bawaan Lona RPG ter-reset saat pindah peta, dan modul pertarungan bernama `Battle_System` (dengan underscore).
   - *Solusi:* Memasang persistent frame-loop hook di `runtimepack_bootstrap.rb` yang mengunci status kebal dan menginjeksi instant death pada musuh saat diserang pemain.
5. **Verifikasi Encrypted-Streaming VFS (Zero-Disk Extraction):**
   - *Build Target:* APK `dist/lona-encrypted/LonaRPG.apk` (SHA-256 `93F5DFA8E653FC12F53A2F5173229728BBBCD0DD97A668CFE87F0F4C0DB99017`, 817.716.388 bytes / 779.84 MB).
   - *Container:* `game.pack` (SHA-256 `A3AC3B36B400D8E7A94B871E5F9389495E652F82A8FE1260E77E36C5DA23C274`, 779.017.590 bytes / 742.93 MB, 15.789 file).
   - *Kriptografi:* ChaCha20-Poly1305 AEAD per-chunk (`encryption: 1`, `stored_size = size + 16`) dengan key terderivasi HKDF-SHA256 package-bound (`com.example.lonarpg::Lona RPG`).
   - *Hasil Uji Storage:* Zero loose files terverifikasi di `/data/user/0/com.example.lonarpg/files/game`. Tidak ada folder `Data/`, `Audio/`, maupun file `Scripts.rvdata2` yang diekstrak ke disk storage.
   - *Temuan Native mkxp-z:* Bypass ekstraksi loose files berfungsi sempurna (`GpackExtractor.prepareStreamingContainer`); runtime native mkxp-z C++ membutuhkan native PhysFS hook provider untuk membaca `Game.ini` langsung dari RAM buffer terdekripsi.
