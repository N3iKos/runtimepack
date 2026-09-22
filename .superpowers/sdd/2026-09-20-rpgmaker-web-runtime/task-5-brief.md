# Task 5: End-to-End Build & Playtest Verification with Fear & Hunger 2: Termina

## Goal
Execute end-to-end packaging and on-device playtest verification of *Fear & Hunger 2: Termina* on the MuMu Player Android emulator (`127.0.0.1:16384`), confirming:
1. `runtimepack inspect` normalizes the content root to `www/` and raises zero fatal `CMP-102` errors.
2. `runtimepack build` compiles `dist/termina/Termina.apk` (Exit status 0, packaged from `www/`).
3. Clean installation (`pm clear com.example.termina`) and launch of `io.runtimepack.host.WebActivity`.
4. Verification of title screen render, audio, touch virtual pad, and new game start.
5. Create documentation report in `Docs/test_ports/rpgmv_fear_and_hunger_2_termina/report.md` and check item in `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.

## Requirements
1. Run `cargo run --bin runtimepack -- inspect "Raw-Game\Fear & Hunger 2 Termina"`. Verify output.
2. Run `cargo run --bin runtimepack -- build "Raw-Game\Fear & Hunger 2 Termina" -o dist/termina --package com.example.termina --name "Termina"`. Verify output APK.
3. Install to MuMu emulator (`127.0.0.1:16384`):
   - `adb -s 127.0.0.1:16384 install -r dist/termina/Termina.apk`
   - `adb -s 127.0.0.1:16384 shell pm clear com.example.termina`
   - `adb -s 127.0.0.1:16384 shell am start -n com.example.termina/io.runtimepack.host.WebActivity`
4. Wait 15 seconds for assets extraction and Chromium WebView initialization.
5. Capture screenshot resized to 960x540: `C:\Users\Nekooz\AppData\Local\Temp\opencode\termina_boot_test.png`.
6. Document findings in `Docs/test_ports/rpgmv_fear_and_hunger_2_termina/report.md`.
7. Write execution report to `.superpowers/sdd/2026-09-20-rpgmaker-web-runtime/task-5-report.md`.
