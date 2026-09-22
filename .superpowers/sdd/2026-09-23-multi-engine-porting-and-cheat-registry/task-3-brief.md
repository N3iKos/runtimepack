# Task 3: Porting & Validation — To the Moon (RPG Maker XP / RGSS1)

## Goal
Port and validate *To the Moon* on Android using the RGSS (mkxp-z) backend. The subagent must inspect the game directory, handle any RPG Maker XP / RGSS1 specific shims or configuration (such as `To the Moon.rgssad` / `Game.ini`), build the APK with `runtimepack build`, install to MuMu emulator (`127.0.0.1:16384`), verify interactive title screen and starting the game, capture a screenshot, write the report in `Docs/test_ports/rpgxp_to_the_moon/report.md`, and update `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.

## Requirements
1. Inspect `Raw-Game/To the Moon`:
   - Run `cargo run --bin runtimepack -- inspect "Raw-Game\To the Moon"`.
   - Engine must be detected as RPG Maker XP (`Confidence: EXACT` or `HIGH`/`MEDIUM` with RGSS1 marker).
   - Verify compatibility issues (e.g. desktop DLLs `fluidsynth.dll`, `steam_api.dll` or `Utils.dll`).
   - If any fatal `CMP-` code blocks the build, add necessary shims or desktop DLL filtering for RGSS desktop packages in `engines/rgss` or `runtimepack_bootstrap.rb`.
2. Build the APK:
   - Run: `cargo run --bin runtimepack -- build "Raw-Game\To the Moon" -o dist/to_the_moon --package com.example.tothemoon --name "To the Moon"`
   - Ensure exit status 0 and verify APK size and SHA-256.
3. Install and verify on MuMu emulator (`127.0.0.1:16384`):
   - `adb -s 127.0.0.1:16384 install -r dist/to_the_moon/ToTheMoon.apk`
   - `adb -s 127.0.0.1:16384 shell pm clear com.example.tothemoon`
   - `adb -s 127.0.0.1:16384 shell am start -n com.example.tothemoon/io.runtimepack.host.RgssActivity`
   - Monitor logcat for clean startup (`mkxp`, `RgssActivity`).
   - Advance until Title Screen / New Game is active.
   - Capture screenshot (resized to 960x540) to `Docs/test_ports/rpgxp_to_the_moon/screenshot.png`.
   - Close the game cleanly: `adb -s 127.0.0.1:16384 shell am force-stop com.example.tothemoon`.
4. Documentation:
   - Create `Docs/test_ports/rpgxp_to_the_moon/report.md` following `Docs/test_ports/TEMPLATE.md`.
   - Mark `[x] To the Moon` in `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.
5. Execution Report:
   - Write `.superpowers/sdd/2026-09-23-multi-engine-porting-and-cheat-registry/task-3-report.md`.

## Critical Constraints
- NEVER touch, read, or write any folder named `antigravity` or `.antigravity`.
- Source directory `Raw-Game/To the Moon` is strictly READ-ONLY.
