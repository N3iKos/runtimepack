# Decisions Log

## [2026-09-18] JoiPlay / M-Tool Style Dynamic Cheat UI di cheat_menu.js
**Keputusan:** Memperbarui `cheat_menu.js` menjadi dynamic engine-agnostic cheat UI dengan 4 tab inti (`#fnh-tab-variables`, `#fnh-tab-switches`, `#fnh-tab-spawner`, `#fnh-tab-console`), live search/filtering, inline variable/switch mutator, dan bookmark snippet script console berbasis `localStorage` (`vpad_user_cheat_snippets`).
**Alasan:** Pengguna membutuhkan kontrol in-game yang fleksibel untuk memodifikasi state game RPG Maker/RGSS dan Web tanpa hardcoding ID item/variabel spesifik ke game tertentu.
**Alternatif yang dipertimbangkan:** Menjaga cheat menu statis per-game (sulit di-maintain dan tidak scalable lintas ribuan game RPG Maker).
**Dampak:** Menu cheat berfungsi dinamis pada seluruh game RPG Maker (XP/VX/Ace/MV/MZ) dengan antarmuka bergaya JoiPlay & M-Tool, mendukung Clean Slate & Retro OSD theme, dan 100% lulus uji otomatis.

## [2026-09-18] Line-Based JSON Decoder Patching di GpackExtractor
**Keputusan:** Mengganti regex replace dengan pencarian batas method eksplisit (`next_token` hingga `parse`) saat mem-patch `101_jsonEnDecoder.rb` di storage internal Android.
**Alasan:** File asli game mengandung blok kode yang di-comment out dengan komentar `# def self.next_token`, sehingga pencarian berbasis regex regex multiline ceroboh memakan baris komentar dan menghasilkan `SyntaxError: unexpected end`.
**Alternatif yang dipertimbangkan:** Menulis ulang file utuh dari asset patch terpisah (kurang fleksibel bila versi game berbeda).
**Dampak:** Ekstraksi game Lona RPG berjalan bersih tanpa error sintaksis Ruby pada modul JSON.

## [2026-09-18] Penggunaan customScript Bootstrap untuk Mengatasi Override Native Win32API
**Keputusan:** Menggunakan `"customScript": "runtimepack_bootstrap.rb"` pada `mkxp.json` alih-alih `"preloadScript"`.
**Alasan:** Pada arsitektur `mkxp-z`, file-file di `preloadScript` dieksekusi SEBELUM `mriBindingInit()`. Akibatnya, kelas C++ `Win32API` bawaan mkxp-z didefinisikan belakangan dan menimpa shim Ruby yang sudah dibuat di preload.
**Alternatif yang dipertimbangkan:** Memodifikasi dan mengompilasi ulang source C++ mkxp-z (memerlukan setup NDK manual yang berat).
**Dampak:** `runtimepack_bootstrap.rb` dieksekusi setelah seluruh binding native siap, mengizinkan monkey-patching kelas `Win32API`, penanganan case-sensitivity `File.size`, dan `visible=` coercion secara mulus.

## [2026-09-18] Uncompressed (STORED) Asset Packaging untuk game.pack
**Keputusan:** Menambahkan `noCompress += listOf("pack", ...)` pada konfigurasi `build.gradle.kts` Android.
**Alasan:** Android `AssetManager.open()` memiliki batas buffer dekompresi in-memory untuk file terkompresi DEFLATE besar (>100MB), menyebabkan `IOException` saat membaca `game.pack` (778 MB).
**Alternatif yang dipertimbangkan:** Memecah `game.pack` menjadi ribuan file asset kecil (membuat build lambat dan mengurangi proteksi kontainer).
**Dampak:** File `game.pack` disimpan tanpa kompresi ZIP di dalam APK dan bisa di-stream langsung secara instan tanpa limit memori.

## [2026-09-18] Integrasi Native mkxp-z ARM64 dan SDLActivity Bridge
**Keputusan:** Mengadopsi library native C++ `mkxp-z` (`arm64-v8a`), Ruby VM, SDL2, OpenAL Soft, dan `SDLActivity` Java sebagai fondasi runtime RGSS.
**Alasan:** Terbukti stabil, mendukung performa OpenGL ES native tanpa emulasi CPU Windows, dan kompatibel dengan RPG Maker XP/VX/VX Ace.
**Alternatif yang dipertimbangkan:** Porting Wine/Box64 (overhead performa tinggi, butuh resource besar, dan bukan native game feel).
**Dampak:** APK berperilaku seperti game native Android mandiri dengan frame rate 60 FPS.

## [2026-09-18] Unified Web Overlay untuk Kontrol Virtual dan Cheat
**Keputusan:** Menggunakan overlay WebView transparan di atas SurfaceView native untuk menjalankan `vpad.js` dan `cheat_menu.js`.
**Alasan:** Menghadirkan in-game layout editor interaktif bergaya Winlator/JoiPlay, 5 preset bawaan, dual theme (Clean Slate & Retro OSD), serta integrasi cheat tanpa perlu menulis ulang UI sentuh di C++ untuk setiap engine yang berbeda.
**Alternatif yang dipertimbangkan:** Render kontrol sentuh manual lewat skrip Ruby RGSS atau rendering C++ ImGui.
**Dampak:** UI kontrol sentuh seragam dan identik di seluruh engine keluarga RuntimePack (RGSS, Web, Ren'Py, Tyrano).
