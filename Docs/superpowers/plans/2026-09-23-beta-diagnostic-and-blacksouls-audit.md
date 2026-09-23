# Implementation Plan: Beta Diagnostic Suite, Touch Pass-Through & BLACK SOULS Bug Analysis

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement native touchscreen pass-through in `WebActivity`, the universal Auto-Recovery engine, the `--flavor beta` build pipeline with permanent watermark and diagnostic suite, and autonomously run the stress test on *BLACK SOULS* to detect and analyze its bug.

**Architecture:** 
- In `WebActivity.kt`, override `dispatchTouchEvent` to pass touches outside active controls directly to `gameWebView`, enabling direct screen taps and mouse cursors for Web games (like *Forestia*).
- In `runtimepack_bootstrap.rb` and `VfsResourceInterceptor.kt`, implement the universal Auto-Recovery engine (dummy transparent bitmaps, silent audio fallbacks on missing assets).
- In `cli/src/main.rs` and `generator.rs`, implement `--flavor beta` (default) vs `--flavor release`.
- In `android/shell`, inject the Beta Diagnostic Suite (floating HUD, 1-click stress test generating `audit_report.json`, live error logger, map warper).
- Autonomous Benchmark: Rebuild *BLACK SOULS* with `--flavor beta`, launch on MuMu emulator, run the automated stress test, extract `audit_report.json`, and identify the root cause of the bug.

**Tech Stack:** Rust (CLI, generator), Kotlin (`WebActivity.kt`, `RgssActivity.kt`), JavaScript (`vpad.js`, diagnostic overlay), Ruby (`runtimepack_bootstrap.rb`), Android ADB.

**Spec:** `Docs/superpowers/specs/2026-09-23-beta-diagnostic-suite-design.md`

## Global Constraints
- Target minSdk: 26, targetSdk: 34.
- Subagents MUST NOT touch, inspect, or modify any directory named `antigravity` or `.antigravity`.
- Source directories in `Raw-Game/` are STRICTLY READ-ONLY.
- Autonomous agent policy: Default build flavor is `--flavor beta`.

---

### Task 1: Native Touchscreen & TouchInput Pass-Through in `WebActivity.kt`

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/KeyMappingTest.kt`

- [ ] **Step 1: Write unit test in `KeyMappingTest.kt`** verifying touch forwarding bounding logic.
- [ ] **Step 2: Implement `dispatchTouchEvent` in `WebActivity.kt`**:
  Forward touches occurring outside the topbar or when gamepad is hidden directly to `gameWebView.dispatchTouchEvent(event)`.
- [ ] **Step 3: Run Android tests:** `.\gradlew.bat testDebugUnitTest`.
- [ ] **Step 4: Commit.**

---

### Task 2: Universal Auto-Recovery Engine (RGSS & Web)

**Files:**
- Modify: `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`
- Test: `android/shell/app/src/test/ruby/input_hook_test.rb`

- [ ] **Step 1: In `runtimepack_bootstrap.rb`, implement Auto-Recovery for RGSS**:
  Hook `Bitmap.new`, `Audio.bgm_play`, `Audio.se_play`, `Audio.bgs_play`, and `Cache.*`: if an asset cannot be opened or does not exist, return a 1x1 dummy bitmap or silence without terminating the thread.
- [ ] **Step 2: In `VfsResourceInterceptor.kt`, implement 404 transparent fallback for Web**:
  If a requested image asset (`.png`, `.jpg`) returns null, supply a 1x1 transparent PNG byte stream.
- [ ] **Step 3: Run tests:** `ruby input_hook_test.rb` and `.\gradlew.bat testDebugUnitTest`.
- [ ] **Step 4: Commit.**

---

### Task 3: Build Flavor Pipeline (`--flavor beta` vs `--flavor release`) & Permanent Beta Watermark

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `core/src/build_plan/generator.rs`
- Modify: `android/shell/app/src/main/assets/overlay/vpad.js`
- Test: `tests/integration/build_test.rs`

- [ ] **Step 1: Write integration test for build flavor** in `tests/integration/build_test.rs`.
- [ ] **Step 2: Add `--flavor` flag in `cli/src/main.rs`** (default: `"beta"`).
- [ ] **Step 3: In `vpad.js` & `generator.rs`, implement flavor-conditional watermark**:
  In `beta` flavor, inject the permanent top-right watermark (`Beta Test Veldra Community`). In `release` flavor, omit the watermark completely.
- [ ] **Step 4: Run tests:** `cargo test`.
- [ ] **Step 5: Commit.**

---

### Task 4: Floating Diagnostic & Automated Stress-Test Suite

**Files:**
- Create: `android/shell/app/src/main/assets/overlay/diagnostic_suite.js`
- Modify: `android/shell/app/src/main/assets/overlay/index.html`
- Modify: `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
- Test: `android/shell/app/src/test/js/web_shims.test.js`

- [ ] **Step 1: Create `diagnostic_suite.js`**:
  Provide floating `[ 🐞 ]` toggle button, live error display, `[ Run Full Stress Test ]` button, `[ Teleport Map ]` input, and `[ Export Bug Report ]`.
- [ ] **Step 2: In `runtimepack_bootstrap.rb`, add automated RGSS stress-test runner**:
  Implement `run_stress_test`: scans `Data/Map*.rvdata2`, verifies all referenced event pictures, battlers, BGM, and tilesets against storage, and writes `audit_report.json`.
- [ ] **Step 3: Verify tests and commit.**

---

### Task 5: Autonomous Benchmark on BLACK SOULS — Stress-Test & Bug Discovery

**Files:**
- Execute: `cargo run --bin runtimepack -- build "Raw-Game\BLACK SOULS" -o dist/black_souls --package com.example.blacksouls --name "Black Souls" --flavor beta`
- Target: MuMu emulator (`127.0.0.1:16384`)
- Document: `Docs/test_ports/rpgvxa_black_souls/report.md`

- [ ] **Step 1: Build BLACK SOULS in `--flavor beta`**.
- [ ] **Step 2: Install and launch on MuMu emulator via subagent**.
- [ ] **Step 3: Execute automated stress test and pull `audit_report.json` via adb**.
- [ ] **Step 4: Identify the exact bug**, document findings, and report to user.
