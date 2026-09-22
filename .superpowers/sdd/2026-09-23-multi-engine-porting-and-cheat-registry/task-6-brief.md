# Task 6: Porting & Validation — BLACK SOULS (RPG Maker VX Ace / RGSS3)

## Goal
Port and validate *BLACK SOULS* (`Raw-Game/BLACK SOULS`) on Android using the RGSS backend (`RgssActivity.kt` / `libmkxp-z.so`). The subagent must inspect the game directory, check engine markers (`Game.ini`, `Data/Scripts.rvdata2`), build the standalone APK with `runtimepack build`, install to MuMu emulator (`127.0.0.1:16384`), verify interactive title screen and starting the game, capture a screenshot, write the report in `Docs/test_ports/rpgvxa_black_souls/report.md`, and update `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.

## Requirements
1. Inspect `Raw-Game/BLACK SOULS`:
   - Run `cargo run --bin runtimepack -- inspect "Raw-Game\BLACK SOULS"`.
   - Verify detection as RPG Maker VX Ace (`Confidence: EXACT`).
   - Check compatibility report (status should be `COMPATIBLE` or `COMPATIBLE_WITH_SHIMS`).
2. Build the APK:
   - Run: `cargo run --bin runtimepack -- build "Raw-Game\BLACK SOULS" -o dist/black_souls --package com.example.blacksouls --name "Black Souls"`
   - Ensure exit status 0 and verify APK size and SHA-256.
3. Install and verify on MuMu emulator (`127.0.0.1:16384`):
   - `adb -s 127.0.0.1:16384 install -r dist/black_souls/BlackSouls.apk`
   - `adb -s 127.0.0.1:16384 shell pm clear com.example.blacksouls`
   - `adb -s 127.0.0.1:16384 shell am start -n com.example.blacksouls/io.runtimepack.host.RgssActivity`
   - Monitor logcat for clean startup (`mkxp`, `RgssActivity`).
   - Advance until Title Screen / New Game is active.
   - Capture screenshot (resized to 960x540) to `Docs/test_ports/rpgvxa_black_souls/screenshot.png`.
   - Close the game cleanly: `adb -s 127.0.0.1:16384 shell am force-stop com.example.blacksouls`.
4. Documentation:
   - Create `Docs/test_ports/rpgvxa_black_souls/report.md` following `Docs/test_ports/TEMPLATE.md`.
   - Mark `[x] Black Souls` in `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.
5. Execution Report:
   - Write `.superpowers/sdd/2026-09-23-multi-engine-porting-and-cheat-registry/task-6-report.md`.

## Critical Constraints
- NEVER touch, read, or write any folder named `antigravity` or `.antigravity`.
- Source directory `Raw-Game/BLACK SOULS` is strictly READ-ONLY.
