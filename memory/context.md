# Project Context

## Apa project ini
RuntimePack adalah build-time compatibility-runtime packager yang mengonversi game desktop/web (RPG Maker XP, VX, VX Ace, MV, MZ, Ren'Py, TyranoBuilder/TyranoScript) menjadi aplikasi Android mandiri (APK/AAB) tanpa emulasi Windows atau launcher eksternal.

## Tech stack
- **Host CLI & Core:** Rust (cargo workspace: `cli`, `core`, `pack`, `engines/*`)
- **Container & Kriptografi:** GPACK/1 binary container dengan streaming AEAD ChaCha20-Poly1305 & SHA-256
- **Android Shell:** Kotlin, Android SDK API 34 (minSdk 26), Gradle 8.7, JDK 17
- **Native RGSS Runtime:** C++ (lineage mkxp-z), Embedded Ruby VM (`libruby.so`), SDL2, OpenAL Soft (`arm64-v8a`)
- **Virtual Controller & UI Overlay:** HTML5/CSS3/JavaScript (`vpad.js`, `cheat_menu.js`, `cheat_engine.js` di atas Android WebView transparan)

## Struktur folder utama
- `cli/`: Binary CLI `runtimepack` (`doctor`, `inspect`, `explain`, `build`, `runtime list`, `licenses`)
- `core/`: Logika domain, ingest, static detector, compatibility analyzer, build planner, diagnostics
- `pack/`: Implementasi container GPACK/1 (`GpackPacker`, `GpackReader`, `ContentCrypto`)
- `engines/`: Modul scanner & detector spesifik engine (`rgss`, `rpgmaker_web`, `renpy`, `tyrano`)
- `android/shell/`: Template project Android (Kotlin, SDLActivity bridge, assets overlay, jniLibs)
- `compatibility-db/`: Database deklaratif penanganan kompatibilitas & shim
- `schemas/`: JSON Schemas untuk config, build-manifest, runtime-manifest, compatibility-report
- `runtimes/`: Manifest runtime backend terdaftar dan native binary libraries
- `tests/`: Fixture corpus dan integration tests (`detection_test`, `build_test`, `compatibility_test`)
- `Docs/`: Spesifikasi teknis kanonikal (PRD, Technical Spec, Architecture, Engine Detection, dll.)
- `memory/`: Memori kerja persisten dan protokol lintasan sesi

## Konvensi coding (naming, style, dll)
- **Rust:** Standard Rust formatting, `snake_case` untuk function/modul, `PascalCase` untuk struct/enum, error handling via `thiserror` & domain codes
- **Kotlin:** Official Kotlin code style, CamelCase, coroutines untuk async IO
- **Ruby (Shims):** Monkey-patch non-intrusif dengan `alias_method` dan safety checks
- **Error Taxonomy:** Wajib menggunakan kode terstruktur stabil (`ING-xxx`, `DET-xxx`, `CMP-xxx`, `RT-xxx`, `RGS-xxx`, `WEB-xxx`, `RNP-xxx`, `TYR-xxx`, `CNT-xxx`, `AND-xxx`, `BLD-xxx`, `SIG-xxx`, `LIC-xxx`)

## Constraint / batasan (misal: gak boleh pakai library X, harus support Y)
- **Game Asli Immutable:** Folder input game user TIDAK BOLEH dimodifikasi sama sekali (in-place). Semua patch/shim diterapkan di working directory, container, atau overlay
- **Zero Broad Storage Permissions:** DILARANG menggunakan `MANAGE_EXTERNAL_STORAGE` atau broad legacy read/write storage permissions; aplikasi wajib menggunakan app-specific internal storage (`filesDir`)
- **Legal Gate Tyrano:** Pembundelan engine TyranoScript diblokir (`TYR-101`) sampai izin redistribusi/legalitas disetujui secara tertulis
- **No Floating Dependencies:** Dilarang menggunakan dependency "latest"; semua versi toolchain dan runtime wajib di-pin di `toolchain.lock` dan manifest
