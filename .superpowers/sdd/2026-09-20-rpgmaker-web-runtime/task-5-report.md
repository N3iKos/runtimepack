# Task 5 Execution Report: End-to-End Build & Playtest Verification with Fear & Hunger 2: Termina

## Overview
- **Task**: Task 5 - End-to-End Build & Playtest Verification with Fear & Hunger 2: Termina
- **Status**: DONE
- **Date**: 2026-09-22
- **Emulator Target**: MuMu Player (Android 15 x86_64, Native ARM64 Translation at `127.0.0.1:16384`)

## Verification Steps & Accomplishments
1. **Engine Detection & Inspection (`runtimepack inspect`)**:
   - Command: `cargo run --bin runtimepack -- inspect "Raw-Game\Fear & Hunger 2 Termina"`
   - Result:
     - Engine: RPG Maker MV (Confidence: EXACT, Score: 210)
     - Content Root: Automatically normalized to `www/` (`DET-WEB-MV-001..004`)
     - Compatibility: Zero fatal errors (only `[CMP-104]` Greenworks WARN stubbed, and `[CMP-104-SHIM]` INFO for node.path and nw.Window).

2. **Standalone Android Packaging (`runtimepack build`)**:
   - Command: `cargo run --bin runtimepack -- build "Raw-Game\Fear & Hunger 2 Termina" -o dist/termina --package com.example.termina --name "Termina"`
   - GPACK container: `game.pack` (1,491.45 MB, 5,491 files packaged from `www/`)
   - Standalone APK: `dist/termina/Termina.apk`
     - Exact Size: 1,602,678,669 bytes (1,528.43 MB)
     - SHA-256: `861D75D5BD1640F70C1EE8103EB8A4B5B484CF08FD4CE7F6C6B0F6DE6E03516D`

3. **On-Device Installation & Launch**:
   - Installed via adb to MuMu emulator (`127.0.0.1:16384`):
     - `adb -s 127.0.0.1:16384 install -r dist/termina/Termina.apk` (Exit 0, Success)
     - `adb -s 127.0.0.1:16384 shell pm clear com.example.termina` (Exit 0, Success)
     - `adb -s 127.0.0.1:16384 shell am start -n com.example.termina/io.runtimepack.host.WebActivity` (Launched)

4. **Technical Bugfixes & Runtime Hardening Implemented**:
   - **Fix 1: Android 15 WindowInsetsController NPE**:
     - `PhoneWindow.getInsetsController()` threw NullPointerException when `hideSystemUi()` was called before `setContentView(rootLayout)` initialized `decorView`.
     - Fixed in `WebActivity.kt` by using null-safe `window.peekDecorView()?.windowInsetsController` and calling `hideSystemUi()` after `setContentView`.
   - **Fix 2: Assets Copy in `AndroidProjectGenerator`**:
     - Added `shims_src` (`android/shell/app/src/main/assets/web_shims`) copying to `core/src/build_plan/generator.rs` so `runtimepack_web_shims.js` is automatically included in generated projects.
   - **Fix 3: NW.js Command Line Args Stub (`nw.App.argv`)**:
     - `Utils.isOptionValid` in `rpg_core.js:195` accesses `nw.App.argv.length`.
     - Provided `argv: []`, `dataPath: ""`, `quit: function() {}` on `nwShim.App` in `runtimepack_web_shims.js`.
   - **Fix 4: Transparent RPG Maker Encrypted Asset Decryption**:
     - Games using encrypted assets (`.rpgmvp`, `.rpgmvo`, `.rpgmvm`) failed when plugins like `SRD_MenuBackgrounds` preloaded `.png` files before database load.
     - Extracted `encryptionKey` from `data/System.json` at startup and implemented `VfsResourceInterceptor.decryptRpgMakerAsset` with on-the-fly XOR decryption.
   - **Fix 5: Mobile Audio Extension Fallback (`.m4a` vs `.ogg`)**:
     - RPG Maker MV forces `.m4a` (`.rpgmvm`) when `Utils.isMobileDevice()` is true, but desktop games only bundle `.ogg` (`.rpgmvo`).
     - Added `patchAudioManager` in `runtimepack_web_shims.js` to prefer `.ogg` and added VFS fallback from `.rpgmvm` -> `.rpgmvo` in `VfsResourceInterceptor.kt`.
   - **Fix 6: Non-Intrusive Polling Patches**:
     - Replaced global `Object.defineProperty` on `Graphics` and `SceneManager` with safe polling (`pollPatches()`), avoiding V8 `SyntaxError: Unexpected identifier 'Object'` when script tags declare `function Graphics()`.
     - Added null safety in `Graphics._clearUpperCanvas`.

5. **Validation Artifacts**:
   - Screenshot captured and resized to 960x540: `C:\Users\Nekooz\AppData\Local\Temp\opencode\termina_boot_test.png`.
   - Porting report created: `Docs/test_ports/rpgmv_fear_and_hunger_2_termina/report.md`.
   - Validation matrix updated: `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.

## Test Commands & Results
- `node android/shell/app/src/test/js/web_shims.test.js`:
  ```text
  Running Mobile Web Shims Unit Tests...
  All Mobile Web Shims tests passed successfully!
  ```
- `.\gradlew.bat testDebugUnitTest` (workdir: `android/shell`):
  ```text
  BUILD SUCCESSFUL in 20s
  24 actionable tasks: 6 executed, 18 up-to-date
  ```
- `cargo test --workspace`:
  ```text
  test result: ok. 22 passed (integration_build)
  test result: ok. 7 passed (integration_compatibility)
  test result: ok. 9 passed (integration_detection)
  test result: ok. 7 passed (core and pack units)
  Total: 45 passed; 0 failed
  ```

## Modified Files
- `core/src/build_plan/generator.rs`
- `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`
- `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`
- `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
- `Docs/test_ports/rpgmv_fear_and_hunger_2_termina/report.md`
- `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`
- `.superpowers/sdd/2026-09-20-rpgmaker-web-runtime/task-5-report.md`
