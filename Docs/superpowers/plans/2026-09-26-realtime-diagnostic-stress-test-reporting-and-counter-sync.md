# Implementation Plan: Real-Time Diagnostic Suite Stress Test Reporting & Counter Sync

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the Diagnostic Suite overlay so that stress test progress is truly real-time: streaming individual missing asset failures as they occur, accurately parsing final audit statistics from `DONE:` and `audit_report.json`, reflecting real asset numbers in the UI counters (`Scanned`, `Valid`, `Missing`), and strictly enforcing red `FAILED`/`ISSUES` badges whenever missing assets are detected (eradicating fake `[SUCCESS]` and fake green `COMPLETED`).

**Architecture:** 
1. Ruby (`runtimepack_bootstrap.rb`): In `run_stress_test`, stream real-time `FAIL: Missing <type>: <path>` log lines immediately when assets fail disk lookup, rather than silently accumulating them to the end.
2. JavaScript (`diagnostic_suite.js` & `preview/diagnostic_suite.js`):
   - Stop miscounting `MAP:` and `DATABASE:` progress lines as verified assets.
   - Stream `FAIL:` lines into `missingCount++`, `failureCount++`, updating live counters and setting badge to `ISSUES (N)`.
   - On `DONE:`, parse exact counts (`maps`, `exact assets`, `case mismatches`, `missing`) via regex and query `getAuditReport()`, updating counters to the authoritative totals (`valid = exact`, `missing = missing`, `scanned = exact + missing`).
   - If `missing > 0` or `failureCount > 0`, strictly forbid `[SUCCESS]` or green `COMPLETED`; force red `FAILED` / `ISSUES (N)` badge, warning status text, and `[WARN]` log entry.
3. Kotlin (`RgssActivity.kt`): In `getAuditReport()`, fall back to reading `files/game/audit_report.json` if `AssetAuditService` has no report.

**Tech Stack:** JavaScript (overlay), Ruby (RGSS bootstrap), Kotlin (Android Activity bridge), Node & Ruby test harnesses.

## Global Constraints

- `diagnostic_suite.js` in `android/shell/app/src/main/assets/overlay/` and `preview/` MUST remain 100% byte-for-byte identical.
- Zero fake `[SUCCESS]` or green `COMPLETED` when missing assets exist.
- Source directories in `Raw-Game/` remain strictly immutable.
- All tests must pass before completing each task.

---

### Task 1: Real-Time Missing Asset Streaming & Report Fallback in Ruby & Kotlin

**Files:**
- Modify: `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`
- Modify: `android/shell/app/src/test/ruby/asset_audit_test.rb`

**Interfaces:**
- Produces:
  - `runtimepack_bootstrap.rb` emits `FAIL: Missing <type>: <path>` in real-time during `run_stress_test`.
  - `RgssActivity.kt:getAuditReport()` reads `files/game/audit_report.json` if `AssetAuditService.getAuditReport()` is empty.

- [ ] **Step 1: Write failing Ruby unit test** in `asset_audit_test.rb`:
  - Verify that `RuntimePackCheatRunner.run_stress_test` writes `FAIL:` lines to progress for missing assets.
- [ ] **Step 2: Update `runtimepack_bootstrap.rb`**:
  - In `run_stress_test`, inside `verify_asset`, when `res == "MISSING"`, call:
    `report_stress_progress("FAIL: Missing #{asset_type}: #{base_path}")`
  - In `run_stress_test`, if `missing_assets.size > 0`, emit `DONE: Stress test completed with #{missing_assets.size} missing assets: ...`
- [ ] **Step 3: Update `RgssActivity.kt`**:
  - In `getAuditReport()`: if `AssetAuditService.getAuditReport()` returns `"{}"` or empty, check `File(dir, "audit_report.json")` and `File(File(dir, "game"), "audit_report.json")`, returning file contents if present.
- [ ] **Step 4: Run tests to verify pass**:
  - `ruby android/shell/app/src/test/ruby/asset_audit_test.rb`
  - `.\gradlew.bat testDebugUnitTest` (in `android/shell`)

---

### Task 2: Real-Time Counter Parsing & Strict Failure Badge in Diagnostic Suite Overlay

**Files:**
- Modify: `android/shell/app/src/main/assets/overlay/diagnostic_suite.js`
- Modify: `preview/diagnostic_suite.js`
- Modify: `android/shell/app/src/test/js/diagnostic_suite.test.js`

**Interfaces:**
- Produces:
  - `diagnostic_suite.js` ignores `MAP:` / `DATABASE:` for asset counter increments.
  - `FAIL:` lines increment `missingCount++`, `scannedCount++`, `failureCount++` live.
  - `DONE:` parses exact stats numbers from the message and/or `getAuditReport()`.
  - Missing count > 0 forces red badge `FAILED` or `ISSUES (N)`, status text `Audit Complete with N missing asset(s)!`, and `[WARN]` log entry.
  - Byte-for-byte synchronization between `assets/overlay/` and `preview/`.

- [ ] **Step 1: Write failing Node test** in `diagnostic_suite.test.js`:
  - Test that receiving `MAP: Scanning Map 1 (1/150)...` does NOT increment `scannedCount` or `validCount`.
  - Test that receiving `FAIL: Missing Tileset: Inside_A2` increments `missingCount`, `scannedCount`, and sets badge to `ISSUES (1)`.
  - Test that receiving `DONE: Stress test completed: 150 maps, 543 exact assets, 0 case mismatches, 189 missing.` sets:
    - `scannedCount = 732`
    - `validCount = 543`
    - `missingCount = 189`
    - Badge = `FAILED` (red `#dc2626`), NOT `COMPLETED` (green)
    - Status text = `Audit Complete with 189 missing asset(s)!`
    - Log entry level = `WARN`, NOT `SUCCESS`
- [ ] **Step 2: Update `diagnostic_suite.js`**:
  - Remove `scannedCount++` and `validCount++` from `MAP:` and `DATABASE:` handlers.
  - In `FAIL:` / `ERROR:` handler: ensure `scannedCount++`, `missingCount++`, `failureCount++` are incremented, UI counters updated, and badge set to `ISSUES (failureCount)`.
  - In `DONE:` handler: parse exact integers from regex `m = line.match(/(\d+)\s+maps.*?(\d+)\s+exact.*?(\d+)\s+case.*?(\d+)\s+missing/i)`:
    - If found: update `validCount = exact`, `missingCount = missing`, `scannedCount = exact + case + missing`, `failureCount = missing + case`.
    - If `failureCount > 0` or `missingCount > 0`: emit `WARN` log, set badge to `FAILED` (red `#dc2626`), and status to `Audit Complete with <N> failure(s).`.
  - Also try `getAuditReport()` to extract `stats.exact_count`, `stats.missing_count`, etc.
  - Synchronize `preview/diagnostic_suite.js` byte-for-byte.
- [ ] **Step 3: Run Node tests**:
  - `node android/shell/app/src/test/js/diagnostic_suite.test.js`

---

### Task 3: Build Verification & Live MuMu Emulator Verification

**Files:**
- Build output: `dist/black_souls/BlackSouls.apk`

- [ ] **Step 1: Run full test suite**:
  - `cargo test --workspace`
  - `node android/shell/app/src/test/js/diagnostic_suite.test.js`
  - `ruby android/shell/app/src/test/ruby/asset_audit_test.rb`
- [ ] **Step 2: Rebuild Black Souls APK with `--flavor beta`**:
  - `cargo run --bin runtimepack -- build "Raw-Game\BLACK SOULS" -o dist/black_souls --package com.example.blacksouls --name "Black Souls" --flavor beta --accept-audit-fail`
- [ ] **Step 3: Install & Launch on MuMu Emulator via ADB / MCP**:
  - Re-install updated APK on MuMu Player (`127.0.0.1:16384`).
  - Open Diagnostic Suite (`[ 🐞 ]`).
  - Run `[ Run Full Stress Test ]`.
  - Verify live counters show actual asset counts (`Scanned: 732 | Valid: 543 | Missing: 189`), badge is red `FAILED` / `ISSUES (189)`, status text reflects 189 missing assets, and log is not marked fake `[SUCCESS]`.
  - Take screenshot to prove fix.
