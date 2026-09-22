# Task 3 Execution Report: Porting & Validation — To the Moon (RPG Maker XP / RGSS1)

## Summary
- **Status:** DONE
- **Objective:** Port and validate *To the Moon* on Android using the RGSS (mkxp-z) backend. Inspect the game directory, handle RGSS desktop package PE DLLs, support `.rgssad` encrypted archives and `lang.dat` RTP mounting, implement Ruby 1.8 compatibility and `MKXP` module stubs in `runtimepack_bootstrap.rb`, build standalone APK, install to MuMu emulator (`127.0.0.1:16384`), verify interactive Title Screen, capture screenshot, update test reports and validation matrix.

## Changes Completed

1. **Static Engine Detection Enhancements (`engines/rgss/src/lib.rs`):**
   - Added encrypted archive detection rules for all three RGSS generations:
     - `DET-RGSS-XP-005`: `*.rgssad` encrypted archive check (+120 weight) -> RPG Maker XP detected with `Confidence: EXACT` (Score: 230).
     - `DET-RGSS-VX-005`: `*.rgss2a` encrypted archive check (+120 weight) -> RPG Maker VX.
     - `DET-RGSS-VXA-005`: `*.rgss3a` encrypted archive check (+120 weight) -> RPG Maker VX Ace.

2. **Desktop Runner DLL Filtering (`engines/rgss/src/scanner.rs`):**
   - Implemented `is_desktop_package` detection based on presence of `mkxp.conf`, `mkxp-console.exe`, or `mkxp.json`.
   - Added `is_desktop_runner_dll` to filter desktop-specific wrapper PE DLLs (`steam_api.dll`, `fluidsynth.dll`, `libfluidsynth*.dll`, `utils*.dll`, `discord_game_sdk.dll`) as `CMP-DESKTOP-EXCLUDED` (Info severity).
   - This prevents fatal `CMP-102` rejections while preserving strict rejection for untrusted arbitrary Windows PE DLLs in non-desktop-runner projects.

3. **Dynamic `mkxp.conf` Parsing and Archive Aliasing (`android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt`):**
   - Added parser for `mkxp.conf` to extract `execName`, `RTP` list (`lang.dat`), and `fontSub` rules.
   - Generated dynamic `mkxp.json` with `execName` set to game's archive name (`To the Moon`), `RTP: ["lang.dat"]`, and multilingual font substitutions (`Arial>Open Sans`, `New Times Roman>Open Sans`).
   - Automatically created archive symlink / alias (`To the Moon.rgssad -> Game.rgssad`) and INI fallback (`To the Moon.ini -> Game.ini`).

4. **Ruby 1.8 Compatibility & MKXP Module Stubs (`android/shell/app/src/main/assets/runtimepack_bootstrap.rb`):**
   - Added Ruby 1.8 compatibility polyfills for RGSS1/RGSS2:
     - `NilClass#id` (4), `NilClass#type` ("NilClass")
     - `Hash#index` (delegates to `key`)
     - `Array#to_s` (returns `join`)
   - Implemented `module MKXP` with `data_directory` (`"./."`), `save_directory`, `custom_save_directory`, `raw_key_states`, `puts`/`print`/`p`, `title`, and safe `method_missing` stubs.
   - Updated `Win32API#call` default branch to return `1` (Win32 `TRUE` / success for utility DLLs).
   - Updated script runner to dynamically discover and load scripts from `Data/Scripts.rxdata`, `Data/Scripts.rvdata`, and `Data/Scripts.rvdata2`.
   - Initialized input and cheat hooks prior to evaluating `Main`.
   - Initialized `.lang` default setting file (`"en"`) to smoothly transition straight to the Title Screen.

5. **Standalone APK Build & Live Verification:**
   - Ran `cargo run --bin runtimepack -- build "Raw-Game\To the Moon" -o dist/to_the_moon --package com.example.tothemoon --name "To the Moon"` (exit 0).
   - Generated `dist/to_the_moon/ToTheMoon.apk` (154.45 MB, SHA-256: `5fc35430aad13fa7f3c75c843133ee1133b66d8251811e4ef074c0d490698cf6`).
   - Installed to MuMu Player (`127.0.0.1:16384`), executed `pm clear com.example.tothemoon`, and launched `io.runtimepack.host.RgssActivity`.
   - Verified clean startup via logcat: `GpackExtractor` extracted 310 files, symlinked archive, `mkxp-z` loaded RGSS1 backend, OpenAL Soft audio initialized without crash.
   - Verified Title Screen rendered with full graphics (Lighthouse, Moon, ocean, Begin/Load/Exit options) and active Virtual Gamepad overlay.
   - Captured screenshot, resized to 960x540, and saved to `Docs/test_ports/rpgxp_to_the_moon/screenshot.png`.
   - Force-stopped the activity cleanly.

6. **Documentation & Matrix Updates:**
   - Created `Docs/test_ports/rpgxp_to_the_moon/report.md` with full details.
   - Updated `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`: marked `[x] To the Moon`.

## Modified Files
- `engines/rgss/src/lib.rs`
- `engines/rgss/src/scanner.rs`
- `android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt`
- `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
- `Docs/test_ports/rpgxp_to_the_moon/report.md`
- `Docs/test_ports/rpgxp_to_the_moon/screenshot.png`
- `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`

## Verification
- `cargo test`: 42 passed (Rust workspace unit + integration tests)
- `ruby input_hook_test.rb`: 44 passed, 0 failed
- `ruby cheat_runner_test.rb`: 25 passed, 0 failed
- `ruby font_popup_compatibility_test.rb`: PASS
- Live test on MuMu emulator (`127.0.0.1:16384`): interactive title screen verified.
