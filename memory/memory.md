# Quick Memory (baca ini duluan)

## TL;DR project
RuntimePack adalah build-time compatibility packager untuk mengubah game desktop (RPG Maker XP/VX/Ace/MV/MZ, Ren'Py, Tyrano) menjadi APK Android native mandiri tanpa emulasi Windows.

## 3 hal paling penting yang harus diingat
- **Folder Game Input 100% Immutable:** Dilarang keras mengedit atau memodifikasi file game user secara langsung (*in-place*); semua shimming/patching harus lewat container, working directory, atau overlay VFS.
- **Dilarang Broad Storage Permission:** Jangan pernah menambahkan `MANAGE_EXTERNAL_STORAGE` atau izin read/write legacy; game harus berjalan di app-specific internal storage (`Context.filesDir`).
- **Gunakan Systematic Debugging & TDD:** Jangan menebak error; ambil bukti konkret (logcat, dump window, backtrace) dan tulis failing test sebelum menerapkan perbaikan.

## Status terkini (1 baris, detail di progress.md)
Track D Runtime Fixes selesai: Show Off (Tyrano) & Mizuki And Me (Ren'Py) terbukti playable & stabil di MuMu emulator (title screen render, input & dialog jalan lancar).

## Preferensi user / gaya kerja
- Komunikasi langsung ke poin utama (*concise, direct*), hindari penjelasan bertele-tele kecuali diminta detail.
- **Wajib Superpowers & Sub-Agent Driven:** Selalu gunakan alur superpowers dan delegasikan tugas/inspeksi/eksekusi ke sub-agent untuk efisiensi token context window.
- **Dilarang Asal Build / Eksekusi Tanpa Konfirmasi:** Jangan langsung memicu build berat atau mengambil asumsi sendiri; jika ada instruksi ambigu, wajib klarifikasi/tanya ke user terlebih dahulu.
- Mengadopsi standar kontrol `rpgmaker-android-suite`: arsitektur 3 file (`index.html`, `vpad.css`, `vpad.js`), 5 preset, toolbar mengambang collapsible, 3 tema baku (Minimalist Glass, Minimalist Matte, Retro OSD), Diagnostic Suite Beta on-screen FPS, dan arsitektur dua lapis Profile-Driven Cheat Engine.
