# Laporan Pengujian Porting: Fear & Hunger 2: Termina

## Informasi Game
- **Judul Game:** Fear & Hunger 2: Termina (Version 1.9.1)
- **Engine / Versi:** RPG Maker MV (Web / Chromium WebView + Canvas/WebGL + WebAudio)
- **Tanggal Uji:** 2026-09-22
- **Perangkat / Emulator:** MuMu Player (Android 15 x86_64, Native ARM64 Translation)
- **Versi Build RuntimePack:** APK SHA-256 `861D75D5BD1640F70C1EE8103EB8A4B5B484CF08FD4CE7F6C6B0F6DE6E03516D` (1,602,678,669 bytes / 1,528.43 MB)

---

## Status Akhir
- **Durasi Playtest:** Sedang Berjalan (Boot, Title Screen, Audio, Virtual Pad terverifikasi)
- **Status Akhir:** `KENDALA (IN PROGRESS)` → Menunggu Playtest Lengkap 30 Menit oleh User

---

## Hasil Evaluasi Kriteria
- [x] **Booting & Title Screen:** Lolos 100%. Game berhasil booting dari `game.pack` (5.491 file terpaket) dan merender Title Screen interaktif lengkap ("Permainan Baru", "Lanjutkan", "Options") via WebGL/Canvas.
- [x] **Audio Engine:** Lolos. Audio BGM/SE termuat tanpa crash setelah penyesuaian extension `.ogg` (`.rpgmvo`) pada mobile environment via `AudioManager.audioFileExt` dan VFS transparent decryption fallback.
- [x] **Kontrol (Virtual Pad & Fisik):** Lolos. Virtual Gamepad overlay `vpad.js` (D-Pad, A, B, DASH, MENU, Q, SKIP, E, dan 4 tombol toolbar navigasi) ter-render di atas layer game dan sinkron dengan event keyboard game.
- [x] **Grafis & Font:** Lolos. Title screen dan teks menu bahasa Indonesia ("Permainan Baru", "Lanjutkan") ter-render bersih tanpa distorsi atau karakter tofu/glitch.
- [ ] **Save & Load:** Sedang dalam proses playtest oleh User.
- [ ] **Stabilitas 30 Menit:** Sedang dalam proses playtest oleh User.

---

## Catatan Kendala Teknis & Solusi yang Diterapkan
1. **WindowInsetsController NullPointerException pada Android 15:**
   - *Kendala:* `PhoneWindow.getInsetsController()` melempar NPE saat `hideSystemUi()` dipanggil sebelum `setContentView` terpasang.
   - *Solusi:* Null-safe check via `window.peekDecorView()?.windowInsetsController` dan penataan urutan inisialisasi setelah `setContentView`.
2. **Missing Asset Shims Copy pada Android Project Generator:**
   - *Kendala:* `runtimepack_web_shims.js` tidak tersalin ke direktori build Android assets, menyebabkan file web shims tidak ditemukan.
   - *Solusi:* Menambahkan `shims_src` (`android/shell/app/src/main/assets/web_shims`) ke proses copy di `core/src/build_plan/generator.rs`.
3. **Crash `TypeError: Cannot read properties of undefined (reading 'length')` pada `Utils.isOptionValid`:**
   - *Kendala:* `Utils.isOptionValid` memeriksa `nw.App.argv.length` ketika `nw` terdefinisi. Karena shim `nw.App` belum mendefinisikan `argv: []`, Chromium melempar TypeError dan menghentikan eksekusi boot script.
   - *Solusi:* Menambahkan `argv: []`, `dataPath: ""`, `quit: function() {}` pada objek `nwShim.App` di `runtimepack_web_shims.js`.
4. **Crash `TypeError: Cannot read properties of undefined (reading 'getContext')` pada `Graphics._clearUpperCanvas`:**
   - *Kendala:* Script plugin error handler (seperti `YEP_CoreEngine` / `Olivia_AntiPlayerStress`) memanggil `Graphics.printFullError` saat boot sebelum canvas layer atas diinstansiasi, memicu exception berantai yang membungkam stack trace asli.
   - *Solusi:* Menambahkan monkey-patch failsafe pada `Graphics._clearUpperCanvas` agar memeriksa keberadaan `this._upperCanvas` sebelum memanggil `getContext('2d')`.
5. **Dukungan Transparan Aset Terenkripsi RPG Maker MV (`.rpgmvp` / `.rpgmvo`):**
   - *Kendala:* Plugin seperti `SRD_MenuBackgrounds` memuat aset gambar `menu.png` saat boot sebelum database `data/System.json` termuat, sedangkan file di kontainer bernama `menu.rpgmvp` terenkripsi dengan kunci 16-byte XOR.
   - *Solusi:* `WebActivity` mendeteksi `encryptionKey` dari `data/System.json` saat startup dan `VfsResourceInterceptor` mengimplementasikan fallback on-the-fly decryption (`decryptRpgMakerAsset`) untuk file `.png` ⇄ `.rpgmvp` serta pemetaan ekstensi `.ogg` ⇄ `.rpgmvo` / `.m4a` ⇄ `.rpgmvm`.
6. **Ekstensi Audio Mobile `.m4a` vs `.ogg`:**
   - *Kendala:* RPG Maker MV secara default memaksa extension `.m4a` saat `Utils.isMobileDevice()` bernilai `true`, padahal game versi desktop PC hanya menyediakan aset audio `.ogg` / `.rpgmvo`.
   - *Solusi:* `patchAudioManager` mem-patch `AudioManager.audioFileExt` agar mengutamakan `.ogg` saat WebAudio mendukung OGG di WebView modern Android.
