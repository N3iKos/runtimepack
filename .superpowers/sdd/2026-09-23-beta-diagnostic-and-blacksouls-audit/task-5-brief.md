# Task 5: Autonomous Benchmark on BLACK SOULS — Stress-Test & Bug Discovery

## Goal
Build *BLACK SOULS* using `--flavor beta`, install to MuMu Player emulator (`127.0.0.1:16384`), verify the permanent Beta Watermark (`Beta Test Veldra Community`), trigger the automated 1-click stress test, extract `audit_report.json`, analyze the output to autonomously identify the bug in BLACK SOULS, apply the fix, and verify resolution.

## Requirements
1. Build BLACK SOULS in `--flavor beta`:
   `cargo run --bin runtimepack -- build "Raw-Game\BLACK SOULS" -o dist/black_souls --package com.example.blacksouls --name "Black Souls" --flavor beta`
   Verify exit status 0.
2. Install to MuMu emulator (`127.0.0.1:16384`):
   - `adb -s 127.0.0.1:16384 install -r dist/black_souls/BlackSouls.apk`
   - `adb -s 127.0.0.1:16384 shell pm clear com.example.blacksouls`
   - `adb -s 127.0.0.1:16384 shell am start -n com.example.blacksouls/io.runtimepack.host.RgssActivity`
3. Verify on-screen:
   - Capture screenshot: `Docs/test_ports/rpgvxa_black_souls/screenshot_beta_wm.png`.
   - Verify permanent top-right watermark `Beta Test Veldra Community` is displayed.
   - Verify floating bug icon `[ 🐞 ]` is present on `#vpad-topbar`.
4. Trigger Stress Test:
   - Trigger `"run_stress_test"` via `adb shell`:
     Write `{"action":"run_stress_test"}` to `/data/user/0/com.example.blacksouls/files/game/rp_cheat_cmd.json` or tap `[ 🐞 ]` -> `[ Run Full Stress Test ]`.
   - Wait 15–20 seconds for the engine to scan all maps, pictures, BGM, and database entries.
   - Pull `audit_report.json` via adb:
     `adb -s 127.0.0.1:16384 exec-out run-as com.example.blacksouls cat files/game/audit_report.json > C:\Users\Nekooz\AppData\Local\Temp\opencode\blacksouls_audit_report.json`.
5. Analyze Bug:
   - Read `blacksouls_audit_report.json` and examine `missing_assets`, `case_mismatches`, `broken_transfers`, or `errors`.
   - Pinpoint the exact bug that would have caused crashes during gameplay!
   - If the bug is a missing asset or case mismatch or script typo, fix it cleanly in `runtimepack_bootstrap.rb` using our Auto-Recovery failsafe or case-insensitive resolver.
6. Documentation:
   - Update `Docs/test_ports/rpgvxa_black_souls/report.md` detailing the stress test findings, the discovered bug, and how it was resolved.
7. Execution Report:
   - Write `.superpowers/sdd/2026-09-23-beta-diagnostic-and-blacksouls-audit/task-5-report.md`.

## Critical Constraints
- NEVER touch, read, or write any folder named `antigravity` or `.antigravity`.
- Source directory `Raw-Game/BLACK SOULS` is strictly READ-ONLY.
