# Implementation Plan: Game Stabilization Plan 6 — The Witch's House MV

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Clean up the audit errors for *The Witch's House MV* (RPG Maker MV) by resolving case mismatches on encrypted `.rpgmvp` tilesets and verifying non-crashing transparent fallbacks for unused default sideview actor assets.

**Architecture:** 
1. In `engines/rpgmaker_web/src/asset_audit.rs`:
   - Resolve 5 case mismatches on encrypted tilesets (`mori_A2.rpgmvp` vs disk `mori_a2.rpgmvp`) cleanly with case-folded matching.
   - For missing default sideview actors (`Actor1_1`) referenced by default actor database entries when sideview battle is disabled, downgrade to `Optional` or ensure auto-recovery transparent fallback prevents runtime failure.
2. In `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`:
   - Ensure case-insensitive resolution handles `.rpgmvp` and `.rpgmvo` files seamlessly on Android.
3. Re-run audit to verify errors drop to minor/clean, and rebuild `dist/witchs_house/TheWitchsHouseMV.apk`.

**Tech Stack:** Rust (RPG Maker Web asset audit), Kotlin (`VfsResourceInterceptor.kt`).

**Spec:** `Docs/superpowers/specs/2026-09-26-game-stabilization-and-unblock-design.md`

## Global Constraints

- Source directory in `Raw-Game/The_Witchs_House_MV` remains strictly immutable.
- Build uses `--flavor beta`.
- All automated unit/integration tests must pass cleanly.
- Code edits, builds, and tests must be delegated to Sub-Agents.

---

### Task 1: Case-Folded Encrypted Tileset Resolution & Sideview Actor Fallbacks

**Files:**
- Modify: `engines/rpgmaker_web/src/asset_audit.rs`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`
- Test: `tests/integration/web_asset_audit_test.rs`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt`

**Interfaces:**
- Produces:
  - Exact/Case-folded resolution for encrypted tilesets (`mori_A2.rpgmvp` -> `mori_a2.rpgmvp`).
  - Default unused sideview battler graphics handled without crashing or stalling.

- [ ] **Step 1: Write integration tests**:
  - `web_asset_audit_test.rs`: Test encrypted tileset case mismatch resolves with warning.
  - `VfsResourceInterceptorTest.kt`: Test case-insensitive resolution of `.rpgmvp` tilesets.
- [ ] **Step 2: Run tests to verify failure**: `cargo test --test web_asset_audit_test`.
- [ ] **Step 3: Implement encrypted case-folding in `engines/rpgmaker_web/src/asset_audit.rs`**.
- [ ] **Step 4: Update `VfsResourceInterceptor.kt`**.
- [ ] **Step 5: Run tests to verify pass**: `cargo test --test web_asset_audit_test` and `gradlew testDebugUnitTest`.
- [ ] **Step 6: Commit changes**:
  ```powershell
  git add engines/rpgmaker_web/src/asset_audit.rs android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt tests/integration/web_asset_audit_test.rs android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt
  git commit -m "feat(web): resolve encrypted tileset case mismatches and verify sideview fallbacks"
  ```

---

### Task 2: Re-Audit & Rebuild *The Witch's House MV*

**Files:**
- Test target: `Raw-Game\The_Witchs_House_MV`
- Build output: `dist/witchs_house/TheWitchsHouseMV.apk`

- [ ] **Step 1: Run static audit on The Witch's House MV**:
  ```powershell
  cargo run --bin runtimepack -- audit "Raw-Game\The_Witchs_House_MV" --mode standard --output dist/witchs_house/asset-audit-report.json
  ```
  - Verify case mismatches are resolved.
- [ ] **Step 2: Execute build for The Witch's House MV**:
  ```powershell
  cargo run --bin runtimepack -- build "Raw-Game\The_Witchs_House_MV" -o dist/witchs_house --package com.example.witchshouse --name "The Witch's House MV" --flavor beta --accept-audit-fail --accept-audit-unknown
  ```
- [ ] **Step 3: Verify APK generation**:
  - Assert that `dist/witchs_house/TheWitch'sHouseMV.apk` exists, has size > 150 MB, and Gradle build exits with 0.
- [ ] **Step 4: Commit build verification record**:
  ```powershell
  git commit --allow-empty -m "chore: verify The Witch's House MV APK build success and clean audit"
  ```
