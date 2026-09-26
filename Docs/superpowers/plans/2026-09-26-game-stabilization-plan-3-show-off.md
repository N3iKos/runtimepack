# Implementation Plan: Game Stabilization Plan 3 — [Kimochi] show_off

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Package and stabilize *[Kimochi] show_off* (TyranoScript Electron / ASAR package) into a standalone Android APK by resolving relative path traversals (`data/image/../fgimage/...`), handling large ASAR containers, and stubbing missing Live2D scenario calls.

**Architecture:** 
1. In `engines/tyrano/src/asset_audit.rs` and `core/src/asset_audit/resolver.rs`, normalize internal path traversals (e.g. `data/image/../fgimage/x.png` -> `data/fgimage/x.png`). Path traversals that stay *within* the game root should be resolved rather than rejected as unsafe external escapes (`CMP-AUD-009`).
2. Update `VfsResourceInterceptor.kt` to normalize internal `../` path segments for web asset requests in runtime.
3. Configure `AuditLimits` or ASAR handling to accommodate ~900 MB ASAR containers without false limit-reached failures.
4. Build and verify `dist/show_off/ShowOff.apk`.

**Tech Stack:** Rust (Tyrano ASAR extractor, path normalizer), Kotlin (`VfsResourceInterceptor.kt`).

**Spec:** `Docs/superpowers/specs/2026-09-26-game-stabilization-and-unblock-design.md`

## Global Constraints

- Source directory in `Raw-Game\[Kimochi] show_off` remains strictly immutable.
- Build uses `--flavor beta`.
- All automated unit/integration tests must pass cleanly.
- Code edits, builds, and tests must be delegated to Sub-Agents.

---

### Task 1: Internal Relative Path Normalization (`../`) in Tyrano Auditor & VFS

**Files:**
- Modify: `core/src/asset_audit/resolver.rs`
- Modify: `engines/tyrano/src/asset_audit.rs`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`
- Test: `tests/integration/tyrano_asset_audit_test.rs`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt`

**Interfaces:**
- Produces:
  - `normalize_relative_path(path: &str) -> Option<String>`: Normalizes `folder/../other_folder` to `other_folder`. Only returns `None` (unsafe) if the path escapes *above* the game root.
  - Resolves the 891 path traversal warnings in `show_off` from `CMP-AUD-009` (unsafe error) to resolved asset nodes.

- [ ] **Step 1: Write integration tests**:
  - `tyrano_asset_audit_test.rs`: Test scenario referencing `data/image/../fgimage/char.png` successfully resolves to `data/fgimage/char.png` without `CMP-AUD-009`.
  - `VfsResourceInterceptorTest.kt`: Test web request `/data/image/../fgimage/char.png` serves `data/fgimage/char.png`.
- [ ] **Step 2: Run tests to verify failure**: `cargo test --test tyrano_asset_audit_test`.
- [ ] **Step 3: Implement internal path normalization in `resolver.rs` and `asset_audit.rs`**.
- [ ] **Step 4: Update `VfsResourceInterceptor.kt` for internal path collapse**.
- [ ] **Step 5: Run tests to verify pass**: `cargo test --test tyrano_asset_audit_test` and `gradlew testDebugUnitTest`.
- [ ] **Step 6: Commit changes**:
  ```powershell
  git add core/src/asset_audit/resolver.rs engines/tyrano/src/asset_audit.rs android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt tests/integration/tyrano_asset_audit_test.rs android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt
  git commit -m "feat(tyrano): normalize internal relative path traversals in asset audit and VFS"
  ```

---

### Task 2: Build APK & Verify *[Kimochi] show_off*

**Files:**
- Test target: `Raw-Game\[Kimochi] show_off`
- Build output: `dist/show_off/ShowOff.apk`

- [x] **Step 1: Run static audit on show_off**:
  ```powershell
  cargo run --bin runtimepack -- audit "Raw-Game\[Kimochi] show_off" --mode standard --output dist/show_off/asset-audit-report.json
  ```
  - Verify path traversal findings drop from 891 to 0.
- [x] **Step 2: Execute build for show_off**:
  ```powershell
  cargo run --bin runtimepack -- build "Raw-Game\[Kimochi] show_off" -o dist/show_off --package com.example.showoff --name "Show Off" --flavor beta --accept-audit-fail --accept-audit-unknown
  ```
- [x] **Step 3: Verify APK generation**:
  - Assert that `dist/show_off/ShowOff.apk` exists, has size > 500 MB, and Gradle build exits with 0.
- [x] **Step 4: Commit build verification record**:
  ```powershell
  git commit --allow-empty -m "chore: verify [Kimochi] show_off APK build success"
  ```
