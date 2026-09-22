# Task 4: Porting & Validation — The Witch's House MV (RPG Maker MV)

## Goal
Port and validate *The Witch's House MV* (`Raw-Game/The_Witchs_House_MV`) on Android using the Web runtime (`WebActivity.kt`). The subagent must inspect the game directory, verify that it normalizes to `www/`, build the standalone APK with `runtimepack build`, install to MuMu emulator (`127.0.0.1:16384`), verify interactive title screen and starting the game, capture a screenshot, write the report in `Docs/test_ports/rpgmv_the_witchs_house_mv/report.md`, and update `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.

## Requirements
1. Inspect `Raw-Game/The_Witchs_House_MV`:
   - Run `cargo run --bin runtimepack -- inspect "Raw-Game\The_Witchs_House_MV"`.
   - Verify detection as RPG Maker MV (`Confidence: EXACT`).
   - Check compatibility report (content root normalized to `www/`).
2. Build the APK:
   - Run: `cargo run --bin runtimepack -- build "Raw-Game\The_Witchs_House_MV" -o dist/witchs_house --package com.example.witchshouse --name "The Witch's House"`
   - Ensure exit status 0 and verify APK size and SHA-256.
3. Install and verify on MuMu emulator (`127.0.0.1:16384`):
   - `adb -s 127.0.0.1:16384 install -r dist/witchs_house/TheWitchsHouse.apk`
   - `adb -s 127.0.0.1:16384 shell pm clear com.example.witchshouse`
   - `adb -s 127.0.0.1:16384 shell am start -n com.example.witchshouse/io.runtimepack.host.WebActivity`
   - Monitor logcat for clean startup (`WebActivity`, `GameConsole`).
   - Advance until Title Screen / New Game is active.
   - Capture screenshot (resized to 960x540) to `Docs/test_ports/rpgmv_the_witchs_house_mv/screenshot.png`.
   - Close the game cleanly: `adb -s 127.0.0.1:16384 shell am force-stop com.example.witchshouse`.
4. Documentation:
   - Create `Docs/test_ports/rpgmv_the_witchs_house_mv/report.md` following `Docs/test_ports/TEMPLATE.md`.
   - Mark `[x] The Witch’s House MV` in `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.
5. Execution Report:
   - Write `.superpowers/sdd/2026-09-23-multi-engine-porting-and-cheat-registry/task-4-report.md`.

## Critical Constraints
- NEVER touch, read, or write any folder named `antigravity` or `.antigravity`.
- Source directory `Raw-Game/The_Witchs_House_MV` is strictly READ-ONLY.
