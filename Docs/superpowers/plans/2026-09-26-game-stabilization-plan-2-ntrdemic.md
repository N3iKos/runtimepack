# Implementation Plan: Game Stabilization Plan 2 — [Kimochi] NTRdemic

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Package and stabilize *[Kimochi] NTRdemic* (TyranoScript Electron / ASAR package) into a standalone Android APK, bypassing the artificial `TYR-101` blocker and handling dynamic expressions gracefully.

**Architecture:** 
1. Use the unblocked Tyrano build pipeline from Phase 0: `TYR-101` produces an informational warning, not a build rejection.
2. In `engines/tyrano/src/asset_audit.rs` and `runtimepack_web_shims.js`, handle dynamic storage expressions (`&tf.img_path`, `&tf.btn_path_off`) and ensure missing scenario tags (`cg.ks`, `replay.ks`) do not cause runtime lockups.
3. Package unpacked assets from `resources/app.asar` into `game.pack` and compile `dist/ntrdemic/NTRdemic.apk`.

**Tech Stack:** Rust (Tyrano ASAR extractor, asset audit), JavaScript (`runtimepack_web_shims.js`, Tyrano KAG shims).

**Spec:** `Docs/superpowers/specs/2026-09-26-game-stabilization-and-unblock-design.md`

## Global Constraints

- Source directory in `Raw-Game/[Kimochi] NTRdemic...` remains 100% immutable.
- Build uses `--flavor beta`.
- All automated unit/integration tests must pass cleanly.
- Code edits, builds, and tests must be delegated to Sub-Agents.

---

### Task 1: Audit & Dynamic Reference Handling for NTRdemic

**Files:**
- Modify: `engines/tyrano/src/asset_audit.rs`
- Modify: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
- Test: `tests/integration/tyrano_asset_audit_test.rs`

**Interfaces:**
- Produces:
  - `TyranoAssetAuditor`: Properly classifies dynamic expressions (`&tf.img_path`, `&tf.btn_path_off`) as non-blocking `CMP-AUD-006` (`INDETERMINATE`), not fatal missing assets.
  - Shims in `runtimepack_web_shims.js`: Stub missing replay/CG gallery scenarios (`cg.ks`, `replay.ks`) so tapping optional menu buttons does not crash.

- [ ] **Step 1: Write integration test** in `tyrano_asset_audit_test.rs`:
  - Test scenario with dynamic expressions `&tf.img_path` emits dynamic reference warning without failing other static assets.
- [ ] **Step 2: Run test to verify failure**: `cargo test --test tyrano_asset_audit_test`.
- [ ] **Step 3: Implement dynamic expression handling in `asset_audit.rs`**.
- [ ] **Step 4: Update web shims for optional scenario jumps in `runtimepack_web_shims.js`**.
- [ ] **Step 5: Run tests to verify pass**: `cargo test --test tyrano_asset_audit_test` and `cargo test --workspace`.
- [ ] **Step 6: Commit changes**:
  ```powershell
  git add engines/tyrano/src/asset_audit.rs android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js tests/integration/tyrano_asset_audit_test.rs
  git commit -m "feat(tyrano): handle dynamic scenario expressions and optional jump stubs"
  ```

---

### Task 2: Build APK & Verify *[Kimochi] NTRdemic*

**Files:**
- Test target: `Raw-Game\[Kimochi] NTRdemic - I Will Rescue Her Without Yielding to Any Temptation`
- Build output: `dist/ntrdemic/NTRdemic.apk`

- [ ] **Step 1: Run static audit on NTRdemic**:
  ```powershell
  cargo run --bin runtimepack -- audit "Raw-Game\[Kimochi] NTRdemic - I Will Rescue Her Without Yielding to Any Temptation" --mode standard --output dist/ntrdemic/asset-audit-report.json
  ```
  - Verify audit report is generated and `TYR-101` is classified as informational only.
- [ ] **Step 2: Execute build for NTRdemic**:
  ```powershell
  cargo run --bin runtimepack -- build "Raw-Game\[Kimochi] NTRdemic - I Will Rescue Her Without Yielding to Any Temptation" -o dist/ntrdemic --package com.example.ntrdemic --name "NTRdemic" --flavor beta --accept-audit-fail --accept-audit-unknown
  ```
- [ ] **Step 3: Verify APK generation**:
  - Assert that `dist/ntrdemic/NTRdemic.apk` exists, has size > 100 MB, and Gradle build exits with 0.
- [ ] **Step 4: Commit build verification record**:
  ```powershell
  git commit --allow-empty -m "chore: verify [Kimochi] NTRdemic APK build success"
  ```
