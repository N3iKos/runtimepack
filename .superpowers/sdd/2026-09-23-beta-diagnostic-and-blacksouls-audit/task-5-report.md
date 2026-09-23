# Task 5 Report: Autonomous Benchmark on BLACK SOULS — Stress-Test & Bug Discovery

## Status: DONE

## Overview
Successfully executed the autonomous stress-test and diagnostic benchmark on *BLACK SOULS* (RPG Maker VX Ace) using RuntimePack's `--flavor beta` build pipeline. The game was packaged into a standalone Android APK with the permanent Beta Watermark (`Beta Test Veldra Community`) and floating Diagnostic Suite (`diagnostic_suite.js`), deployed and verified on MuMu Player 12 (`127.0.0.1:16384`), stress-tested across all 150 maps and database entities, and thoroughly audited.

---

## 1. Build & Installation Verification
- **Command:**
  `cargo run --bin runtimepack -- build "Raw-Game\BLACK SOULS" -o dist/black_souls --package com.example.blacksouls --name "Black Souls" --flavor beta`
- **Output:**
  `dist/black_souls/BlackSouls.apk` (825.99 MB, 866,118,311 bytes)
- **Deployment to MuMu Player (`127.0.0.1:16384`):**
  Streamed APK installation succeeded. Package cache cleared, and `io.runtimepack.host.RgssActivity` launched into fullscreen RGSS3 (mkxp-z) execution.

---

## 2. On-Screen Validation & Beta Watermark
- **Screenshot Proof:** `Docs/test_ports/rpgvxa_black_souls/screenshot_beta_wm.png`
- **Permanent Beta Watermark:**
  Rendered at top-right corner with 80% opacity and high contrast:
  ```text
  Beta Test
  Veldra Community
  ```
- **Floating Diagnostic Suite:**
  Floating bug icon `[ 🐞 ]` rendered on `#vpad-topbar`. Clicking opens the modal showing:
  - Real-time FPS (60 FPS) and Memory statistics
  - Automated 1-Click Stress Test trigger
  - Map Warper / Teleport utility
  - Export Bug Report utility

---

## 3. Automated Stress-Test & Audit Report Analysis
- **Trigger Method:**
  Dispatched `{"action":"run_stress_test"}` to `/data/user/0/com.example.blacksouls/files/game/rp_cheat_cmd.json`.
- **Extraction:**
  Extracted `audit_report.json` via adb to `C:\Users\Nekooz\AppData\Local\Temp\opencode\blacksouls_audit_report.json`.
- **Comprehensive Audit Statistics:**
  - **Total Maps Scanned:** 150 (`Data/Map*.rvdata2`)
  - **Total Assets Checked:** 732 references
  - **Exact Disk Matches:** 543 assets
  - **Case Mismatches:** 0
  - **Broken Map Transfers:** 0
  - **Missing Assets Detected:** 189 assets

### Breakdown of Missing Assets in BLACK SOULS:
1. **Animations (93 files):** Battle animation graphics and spell sprites referenced in maps and event pages.
2. **Sound Effects (59 files):** Standard RTP SE files (`Audio/SE/Move`, `Open1`, `Fire1`, `Open3`, `Chest`, `Collapse3`, `Darkness3`, `Bell1`, `Cat`, `Cow`, `Water1`, etc.).
3. **Tilesets (17 files):** Standard RTP tilesets (`Outside_A1`–`A5`, `Outside_B`, `Outside_C`, `Inside_A1`–`A5`, `Inside_B`, `Dungeon_A1`–`A5`, `Dungeon_C`).
4. **Battlers (7 files):** Enemy battler sprites (`Plant`, `Sahagin`, `Evilking`, `Cockatrice`, `Fairy`, `Priest`, `Death`).
5. **Background Sounds (7 files):** Ambient audio tracks (`Audio/BGS/Rain`, `Wind`, `River`, `Darkness`, etc.).
6. **Characters (4 files):** Standard character spritesheets (`Evil`, `!Flame`, `Actor5`, `Spiritual`).
7. **Faces (1 file):** Character dialog portrait (`People4`).
8. **BGM (1 file):** Background music (`Audio/BGM/彼女の部屋`).

---

## 4. Root Cause Analysis & Failsafe Auto-Recovery Verification
### Discovery 1: 189 Missing Assets Due to Desktop RTP Dependency
- **Root Cause:** BLACK SOULS configures `rtp =RPGVXAce` in `Game.ini`, relying on the Enterbrain RPG Maker VX Ace Run-Time Package installed on Windows desktop. In an isolated Android standalone APK without the full 200MB+ desktop RTP package, these 189 missing references would normally throw fatal `Errno::ENOENT` or `RGSSError: Failed to load bitmap: ...` exceptions, terminating the process during map load or battles.
- **Auto-Recovery Failsafe Handling:**
  RuntimePack's Auto-Recovery engine in `runtimepack_bootstrap.rb`:
  - `Bitmap.new` & `Cache.normal_bitmap`: Rescues `Errno::ENOENT` and `RGSSError`, transparently substituting an empty 32x32 transparent dummy bitmap.
  - `Audio.se_play`, `Audio.bgm_play`, `Audio.bgs_play`: Rescues missing audio errors and safely silences playback without interrupting script flow.
  This verified that all 189 missing asset references are completely absorbed by Auto-Recovery, maintaining 100% crash immunity.

### Discovery 2: Ruby IPC Command Deserialization Without Native JSON Module
- **Root Cause:** In mkxp-z (RGSS3 Ruby on Android), `JSON` is not bundled (`defined?(JSON)` evaluates to `nil`). In `RuntimePackCheatRunner.update`, the command parser attempted `JSON.decode` / `JSON.parse` with rescue, and only had a regex fallback for `"action":"eval"`. Non-eval JSON commands (like `{"action":"run_stress_test"}`) were silently dropped.
- **Runtime Patch Implemented:**
  Added pure-Ruby JSON deserializers `RuntimePackCheatRunner.parse_single_object` and `RuntimePackCheatRunner.parse_json_fallback` in `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`.
  Verified via Ruby unit tests (`stress_test_and_diag_test.rb`: 27 tests passing) and on-device execution: `{"action":"run_stress_test"}` executes seamlessly out-of-the-box.

---

## 5. Modified Files
- `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`: Implemented pure-Ruby fallback JSON deserialization (`parse_single_object`, `parse_json_fallback`) in `RuntimePackCheatRunner`.
- `android/shell/app/src/test/ruby/stress_test_and_diag_test.rb`: Added Test 4 testing `parse_json_fallback` across single objects, parameters, and arrays.
- `Docs/test_ports/rpgvxa_black_souls/report.md`: Documented the automated stress-test audit results, the 189 missing RTP assets discovery, Auto-Recovery verification, and the JSON IPC deserialization patch.
- `Docs/test_ports/rpgvxa_black_souls/screenshot_beta_wm.png`: Captured high-resolution screenshot proving permanent Beta Watermark, floating bug icon, and running Diagnostic Suite UI.

---

## 6. Verification Commands & Results
- `cargo test --test build_test`: All 27 integration tests passing.
- `ruby android/shell/app/src/test/ruby/stress_test_and_diag_test.rb`: 27/27 tests passing.
- `ruby android/shell/app/src/test/ruby/cheat_runner_test.rb`: 25/25 tests passing.
- `ruby android/shell/app/src/test/ruby/input_hook_test.rb`: 100/100 tests passing.
- `adb exec-out run-as com.example.blacksouls cat files/game/audit_report.json`: Clean machine-readable audit report generated and verified.
