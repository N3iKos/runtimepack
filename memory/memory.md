# Quick Memory (baca ini duluan)

## TL;DR project
RuntimePack adalah build-time compatibility packager untuk mengubah game desktop (RPG Maker XP/VX/Ace/MV/MZ, Ren'Py, Tyrano) menjadi APK Android native mandiri tanpa emulasi Windows.

## 3 hal paling penting yang harus diingat
- **Folder Game Input 100% Immutable:** Dilarang keras mengedit atau memodifikasi file game user secara langsung (*in-place*); semua shimming/patching harus lewat container, working directory, atau overlay VFS.
- **Dilarang Broad Storage Permission:** Jangan pernah menambahkan `MANAGE_EXTERNAL_STORAGE` atau izin read/write legacy; game harus berjalan di app-specific internal storage (`Context.filesDir`).
- **Gunakan Systematic Debugging & TDD:** Jangan menebak error; ambil bukti konkret (logcat, dump window, backtrace) dan tulis failing test sebelum menerapkan perbaikan.

## Status terkini (1 baris, detail di progress.md)
Milestone 0-9 selesai; Milestone 4 RGSS native slice aktif; Lona RPG (VX Ace) terbukti sukses booting dan merender Title Screen interaktif di emulator Android.

## Preferensi user / gaya kerja
- Komunikasi langsung ke poin utama (*concise, direct*), hindari penjelasan bertele-tele kecuali diminta detail.
- Utamakan aksi nyata dan perbaikan mandiri sampai tuntas ("langsung selesaikan semua secara iterasi").
- Mengadopsi standar kontrol `rpgmaker-android-suite`: layout `vpad.js` (5 preset, 4 ikon toolbar pojok kiri atas, Clean Slate & Retro OSD theme) dan arsitektur dua lapis Profile-Driven Cheat Engine.
