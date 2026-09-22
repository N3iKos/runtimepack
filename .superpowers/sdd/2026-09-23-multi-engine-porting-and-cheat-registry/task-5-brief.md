# Task 5: Porting & Validation — Forestia (RPG Maker MZ)

## Goal
Port and validate *Forestia* (`Raw-Game/Forestia1.4.4-IND`) on Android using the Web runtime (`WebActivity.kt`). The subagent must inspect the game directory, handle any RPG Maker MZ specific traits (e.g. desktop NW.js wrapper DLL filtering, `js/rmmz_core.js`, WebGL/WebAudio), build the standalone APK with `runtimepack build`, install to MuMu emulator (`127.0.0.1:16384`), verify interactive title screen and starting the game, capture a screenshot, write the report in `Docs/test_ports/rpgmz_forestia/report.md`, and update `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.

## Requirements
1. Inspect `Raw-Game/Forestia1.4.4-IND`:
   - Run `cargo run --bin runtimepack -- inspect "Raw-Game\Forestia1.4.4-IND"`.
   - Verify detection as RPG Maker MZ (`Confidence: EXACT`).
   - Check if desktop DLLs in the root trigger `CMP-102`. Notice that in Forestia, `package.json` and `index.html` might be in the root directory directly with NW.js DLLs beside them (unlike MV which has a `www/` subfolder).
   - If desktop DLLs in the root trigger `CMP-102` for an MZ project where `index.html` is at the root, ensure the analyzer/scanner safely recognizes desktop NW.js runner DLLs and ignores them.
2. Build the APK:
   - Run: `cargo run --bin runtimepack -- build "Raw-Game\Forestia1.4.4-IND" -o dist/forestia --package com.example.forestia --name "Forestia"`
   - Ensure exit status 0 and verify APK size and SHA-256.
3. Install and verify on MuMu emulator (`127.0.0.1:16384`):
   - `adb -s 127.0.0.1:16384 install -r dist/forestia/Forestia.apk`
   - `adb -s 127.0.0.1:16384 shell pm clear com.example.forestia`
   - `adb -s 127.0.0.1:16384 shell am start -n com.example.forestia/io.runtimepack.host.WebActivity`
   - Monitor logcat for clean startup (`WebActivity`, `GameConsole`).
   - Advance until Title Screen / New Game is active.
   - Capture screenshot (resized to 960x540) to `Docs/test_ports/rpgmz_forestia/screenshot.png`.
   - Close the game cleanly: `adb -s 127.0.0.1:16384 shell am force-stop com.example.forestia`.
4. Documentation:
   - Create `Docs/test_ports/rpgmz_forestia/report.md` following `Docs/test_ports/TEMPLATE.md`.
   - Mark `[x] Forestia` in `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.
5. Execution Report:
   - Write `.superpowers/sdd/2026-09-23-multi-engine-porting-and-cheat-registry/task-5-report.md`.

## Critical Constraints
- NEVER touch, read, or write any folder named `antigravity` or `.antigravity`.
- Source directory `Raw-Game/Forestia1.4.4-IND` is strictly READ-ONLY.
