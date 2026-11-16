# Implementation Plan: Game Stabilization Plan 5 — Forestia

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Clean up the audit errors for *Forestia* (RPG Maker MZ) by enabling recursive subfolder plugin resolution (`js/plugins/**/`) and case-insensitive encrypted audio extension resolution.

**Architecture:** 
1. In `engines/rpgmaker_web/src/asset_audit.rs`:
   - When resolving plugins declared in `js/plugins.js` or transitive `@base` dependencies, search subdirectories under `js/plugins/` (e.g. `js/plugins/develop/PluginCommonBase.js`) if not found directly in the root of `js/plugins/`.
   - In `AssetResolver`, ensure case-insensitive matching works seamlessly for MZ encrypted extensions (`.ogg_`, `.png_`).
2. Update `VfsResourceInterceptor.kt` to support subfolder plugin fallbacks if requested by WebView.
3. Re-run audit to verify 22 plugin errors drop to 0, and re-compile `dist/forestia/Forestia.apk`.

**Tech Stack:** Rust (RPG Maker Web asset audit), Kotlin (`VfsResourceInterceptor.kt`).

**Spec:** `Docs/superpowers/specs/2026-09-26-game-stabilization-and-unblock-design.md`

## Global Constraints

- Source directory in `Raw-Game/Forestia1.4.4-IND` remains strictly immutable.
- Build uses `--flavor beta`.
- All automated unit/integration tests must pass cleanly.
- Code edits, builds, and tests must be delegated to Sub-Agents.

---

### Task 1: Subfolder Plugin Search & Encrypted Audio Case Resolution

**Files:**
- Modify: `engines/rpgmaker_web/src/asset_audit.rs`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`
- Test: `tests/integration/web_asset_audit_test.rs`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt`

**Interfaces:**
- Produces:
  - Plugin resolver searches `js/plugins/**/*.js` if `js/plugins/{name}.js` is not found, resolving the 22 missing plugin findings in *Forestia*.
  - Case-folded lookup for encrypted extensions (`.ogg_` / `.png_`) resolves `audio/se/jump1.ogg_` -> `Jump1.ogg_`.

- [ ] **Step 1: Write integration tests**:
  - `web_asset_audit_test.rs`: Test game with plugin in `js/plugins/subfolder/MyPlugin.js` resolves correctly without missing error.
  - `web_asset_audit_test.rs`: Test encrypted audio case mismatch resolves with warning.
- [ ] **Step 2: Run tests to verify failure**: `cargo test --test web_asset_audit_test`.
- [ ] **Step 3: Implement subfolder plugin resolution in `engines/rpgmaker_web/src/asset_audit.rs`**.
- [ ] **Step 4: Update `VfsResourceInterceptor.kt` for subfolder plugin fallback**.
- [ ] **Step 5: Run tests to verify pass**: `cargo test --test web_asset_audit_test` and `gradlew testDebugUnitTest`.
- [ ] **Step 6: Commit changes**:
  ```powershell
  git add engines/rpgmaker_web/src/asset_audit.rs android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt tests/integration/web_asset_audit_test.rs android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt
  git commit -m "feat(web): support subfolder plugin resolution and case-folded encrypted audio"
  ```

---

### Task 2: Re-Audit & Rebuild *Forestia*

**Files:**
- Test target: `Raw-Game\Forestia1.4.4-IND`
- Build output: `dist/forestia/Forestia.apk`

- [ ] **Step 1: Run static audit on Forestia**:
  ```powershell
  cargo run --bin runtimepack -- audit "Raw-Game\Forestia1.4.4-IND" --mode standard --output dist/forestia/asset-audit-report.json
  ```
  - Verify missing plugin errors drop from 22 to 0 (only minor/optional findings remain).
- [ ] **Step 2: Execute build for Forestia**:
  ```powershell
  cargo run --bin runtimepack -- build "Raw-Game\Forestia1.4.4-IND" -o dist/forestia --package com.example.forestia --name "Forestia" --flavor beta --accept-audit-fail --accept-audit-unknown
  ```
- [ ] **Step 3: Verify APK generation**:
  - Assert that `dist/forestia/Forestia.apk` exists, has size > 800 MB, and Gradle build exits with 0.
- [ ] **Step 4: Commit build verification record**:
  ```powershell
  git commit --allow-empty -m "chore: verify Forestia APK build success and clean audit"
  ```
