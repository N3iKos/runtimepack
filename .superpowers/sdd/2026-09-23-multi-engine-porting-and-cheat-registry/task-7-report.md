# Task 7 Execution Report: Porting & Validation — TyranoScript ([Kimochi] NTRdemic)

## Summary
Implemented support for Electron-packaged TyranoScript games (`resources/app.asar`), created pure Rust ASAR archive parsing and extraction (`AsarArchive`), integrated `TyranoAnalyzer` into compatibility diagnostics, handled Node.js desktop environment shims (`process.execPath`, `process.env.HOME`, `$.isNeedClickAudio`, `localStorage` save routing), and fixed binary MIME charset decoding in `VfsResourceInterceptor.kt`. Built standalone APK `dist/ntrdemic/NTRdemic.apk`, verified live execution, audio streaming (`title2.mp3`), and title screen rendering on MuMu Player (`127.0.0.1:16384`), captured 960x540 screenshot to `Docs/test_ports/tyrano_kimochi/screenshot.png`, cleanly stopped the application, created `Docs/test_ports/tyrano_kimochi/report.md`, and updated `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.

---

## 1. Engine Inspection
- **Command:** `cargo run --bin runtimepack -- inspect "Raw-Game\[Kimochi] NTRdemic - I Will Rescue Her Without Yielding to Any Temptation"`
- **Result:**
  - Indexed 77 files (350.56 MB, source_hash: `b63c2d8c`)
  - Engine: TyranoBuilder / TyranoScript
  - Confidence: `EXACT` (Score: 360)
  - Status: `COMPATIBLE`
  - Runtime Family: Tyrano Web
  - Evidence:
    - `DET-TYR-ASAR`: `resources/app.asar` (+120)
    - `DET-TYR-001`: `resources/app.asar::index.html` (+80)
    - `DET-TYR-002`: `resources/app.asar::tyrano/tyrano.js` (+80)
    - `DET-TYR-003`: `resources/app.asar::tyrano/plugins/kag/` (+50)
    - `DET-TYR-004`: `resources/app.asar::data/scenario/` (+30)
  - Desktop wrapper DLLs categorized as `CMP-DESKTOP-EXCLUDED` / `CMP-REDIST-EXCLUDED` (Info severity).

---

## 2. Build Artifacts
- **Command:** `cargo run --bin runtimepack -- build "Raw-Game\[Kimochi] NTRdemic - I Will Rescue Her Without Yielding to Any Temptation" -o dist/ntrdemic --package com.example.ntrdemic --name "NTRdemic"`
- **Package ID:** `com.example.ntrdemic`
- **Output APK:** `dist/ntrdemic/NTRdemic.apk`
- **APK Size:** 166,339,040 bytes (158.63 MB)
- **APK SHA-256:** `F2EEA549C69B91B3E15DB3B7491011E49E71B4E538F6A0E9DE4818B140716733`
- **Container (`game.pack`):** 1,104 files (121.70 MB, hash: `719f2263`) unpacked from `resources/app.asar` to build staging and packed.

---

## 3. Shims & Compatibility Implementations
1. **Pure Rust ASAR Archive Parser & Extractor (`engines/tyrano/src/asar.rs`):**
   - Implemented `AsarArchive` that parses the 16-byte ASAR header (magic: 4, header size, json length) and extracts file metadata (`path`, `offset`, `size`).
   - Supports in-memory byte reading and streaming filesystem extraction (`extract_all`) for build-time content staging.
2. **Static Detection & Analyzer Support (`engines/tyrano/src/lib.rs`, `engines/tyrano/src/scanner.rs`):**
   - Enhanced `TyranoDetector` to probe inside `resources/app.asar` when present, identifying `index.html`, `tyrano.js`, KAG plugins, and scenario data.
   - Implemented `TyranoAnalyzer` checking for unsupported `.node` binary modules and categorizing desktop runner DLLs as excluded.
3. **Web Shims & Process Environment (`runtimepack_web_shims.js`):**
   - Provided `process.execPath: ""` and `process.env: { HOME: "" }` to eliminate `TypeError: Cannot read properties of undefined (reading 'indexOf')` when `tyrano/libs.js` checks for desktop macOS Sierra paths in `$.localFilePath()`.
   - Forwarded `require("jquery")` and `require("./tyrano/libs/jquery-3.6.0.min.js")` to `window.jQuery` to preserve jQuery plugins and DOM event attachment.
   - Routed file-based storage operations (`type == "file"`) directly to `localStorage` (`$.setStorageWeb` / `$.getStorageWeb`).
   - Hooked `$.isNeedClickAudio` to return `false` and initialized `window.TYRANO.kag.tmp.ready_audio = true`, allowing autoplay audio without blocking tag execution.
4. **Binary MIME Charset Handling (`VfsResourceInterceptor.kt`):**
   - Added `getEncoding(mimeType)` to ensure charset `"UTF-8"` is only supplied for text/JSON/JavaScript formats, and `null` for binary images (`.png`, `.gif`), fonts (`.woff`, `.ttf`), and audio (`.mp3`, `.ogg`), resolving image decoding failures.

---

## 4. Emulator Validation & Gameplay Verification
- **Target Device:** MuMu Player (`127.0.0.1:16384`, Android 12 x86_64, Chromium WebView API 34)
- **Installation:** `adb -s 127.0.0.1:16384 install -r dist/ntrdemic/NTRdemic.apk` (Success)
- **Startup:** Clean launch via `adb shell am start -n com.example.ntrdemic/io.runtimepack.host.WebActivity`.
- **Title Screen:** Rendered full resolution with animated heroine background `title03.gif` (3840x2160), title logo `title_logo.png`, "はじめる" (Start), "つづきから" (Continue), and "オプション" (Option) buttons. Title BGM `title2.mp3` played through AAudio/WebAudio.
- **Screenshot:** Captured active game screen and saved clean 960x540 image to `Docs/test_ports/tyrano_kimochi/screenshot.png`.
- **Clean Exit:** Force-stopped cleanly via `adb shell am force-stop com.example.ntrdemic`.

---

## 5. Artifacts & Documentation
- `Docs/test_ports/tyrano_kimochi/report.md` created.
- `Docs/test_ports/tyrano_kimochi/screenshot.png` captured (960x540).
- `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md` updated with `[x] [Kimochi] NTRdemic`.

---

## Status
DONE
