# Implementation Plan: Game Stabilization Phase 0 & Termina

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove artificial build blockers (`TYR-101` and Windows DLLs `CMP-102`), sanitize Gradle project names, and stabilize *Fear & Hunger 2: Termina* to achieve a clean APK build and verified audit.

**Architecture:** 
1. In `core/src/build_plan/audit_policy.rs` and `cli/src/main.rs`, remove `TYR-101` and desktop-wrapper DLLs from hard non-bypassable blockers, treating them as informative warnings overridable by `--accept-audit-fail`.
2. In `core/src/build_plan/generator.rs`, sanitize `rootProject.name` in `settings.gradle.kts` to replace characters like `:`, `/`, `\` with `_`, preventing Gradle `InvalidUserDataException`.
3. In `engines/renpy/src/scanner.rs`, classify `lib/py3-windows-x86_64/*.dll` as excluded desktop artifacts rather than fatal blockers.
4. Execute audit and packaging for *Fear & Hunger 2: Termina*, verifying that `Termina.apk` builds successfully with case mismatches and missing assets handled gracefully.

**Tech Stack:** Rust (core build policy, generator, ingest), Gradle, Kotlin (WebActivity, VFS interceptor).

**Spec:** `Docs/superpowers/specs/2026-09-26-game-stabilization-and-unblock-design.md`

## Global Constraints

- Game sources in `Raw-Game/` remain strictly immutable.
- Autonomous subagents build with `--flavor beta`.
- All tests must pass before completing each task.
- Zero fake progress or simulated `SUCCESS`.

---

### Task 1: Remove Policy Hard Blockers (`TYR-101` & Desktop DLLs)

**Files:**
- Modify: `core/src/build_plan/audit_policy.rs`
- Modify: `cli/src/main.rs`
- Modify: `engines/renpy/src/scanner.rs`
- Modify: `tests/integration/build_test.rs`
- Modify: `tests/integration/compatibility_test.rs`

**Interfaces:**
- Produces:
  - `evaluate_audit_policy`: Does NOT treat `TYR_LEGAL_GATE_BLOCKED` (`TYR-101`) as a non-bypassable hard blocker. When `--accept-audit-fail` is supplied, Tyrano builds proceed.
  - `RenPyAnalyzer`: Windows DLLs located in `lib/py3-windows-x86_64/` are classified as `CMP-DESKTOP-EXCLUDED` (warning), not fatal `CMP-102`.
  - Truly dangerous blockers (container corruption `CNT-*`, path traversal `CMP-AUD-009`) remain strictly enforced.

- [ ] **Step 1: Write integration tests**:
  - `build_test.rs`: Test that a project with `TYR-101` warning can build when `--accept-audit-fail` is passed.
  - `compatibility_test.rs`: Test that Ren'Py game with Windows desktop DLLs in `lib/py3-windows-x86_64/` emits `CMP-DESKTOP-EXCLUDED` warning without returning `CompatibilityStatus::UnsupportedDependency`.
- [ ] **Step 2: Run tests to verify failure**:
  - `cargo test -p runtimepack --test integration_build`
  - `cargo test -p runtimepack --test integration_compatibility`
- [ ] **Step 3: Update `core/src/build_plan/audit_policy.rs`**:
  - In `evaluate_audit_policy`: Remove `TYR_LEGAL_GATE_BLOCKED` from `hard_blockers`. Allow `--accept-audit-fail` to permit packaging.
- [ ] **Step 4: Update `engines/renpy/src/scanner.rs`**:
  - In `RenPyAnalyzer::analyze`: Filter out DLLs in desktop wrapper directory `lib/py3-windows-x86_64/`, emitting `CMP_DESKTOP_EXCLUDED` instead of fatal `CMP-102`.
- [ ] **Step 5: Run tests to verify pass**:
  - `cargo test --test integration_build`
  - `cargo test --test integration_compatibility`
  - `cargo test --workspace`
- [ ] **Step 6: Commit changes**:
  ```powershell
  git add core/src/build_plan/audit_policy.rs cli/src/main.rs engines/renpy/src/scanner.rs tests/integration/build_test.rs tests/integration/compatibility_test.rs
  git commit -m "fix(policy): remove artificial hard blockers for Tyrano and desktop wrapper DLLs"
  ```

---

### Task 2: Gradle Project Name Sanitization in `generator.rs`

**Files:**
- Modify: `core/src/build_plan/generator.rs`
- Modify: `tests/integration/build_test.rs`

**Interfaces:**
- Produces:
  - `AndroidProjectGenerator::sanitize_gradle_project_name(name: &str) -> String`
  - Replaces illegal characters `[/, \, :, <, >, ", ?, *, |]` and whitespace with `_`.
  - Ensures `settings.gradle.kts` uses sanitized name for `rootProject.name = "..."`.

- [ ] **Step 1: Write integration tests** in `build_test.rs`:
  - Test that an app name with colons and special characters like `"Fear & Hunger 2: Termina"` produces sanitized `rootProject.name = "Fear_Hunger_2_Termina"`.
- [ ] **Step 2: Run test to verify failure**:
  - `cargo test -p runtimepack --test integration_build test_gradle_project_name_sanitization`
- [ ] **Step 3: Implement `sanitize_gradle_project_name` in `generator.rs`**:
  - Replace illegal characters with `_` and collapse consecutive underscores.
  - Use sanitized name in `settings.gradle.kts`.
- [ ] **Step 4: Run tests to verify pass**:
  - `cargo test --test integration_build`
  - `cargo test --workspace`
- [ ] **Step 5: Commit changes**:
  ```powershell
  git add core/src/build_plan/generator.rs tests/integration/build_test.rs
  git commit -m "fix(generator): sanitize Gradle rootProject.name to prevent syntax crashes on colons"
  ```

---

### Task 3: Stabilize & Build *Fear & Hunger 2: Termina*

**Files:**
- Test target: `Raw-Game\Fear & Hunger 2 Termina`
- Build output: `dist/termina/Termina.apk`

**Interfaces:**
- Produces:
  - Clean static audit report `dist/termina/asset-audit-report.json`.
  - Fully compiled APK `dist/termina/Termina.apk`.

- [ ] **Step 1: Run static audit on Termina**:
  - Command:
    `cargo run --bin runtimepack -- audit "Raw-Game\Fear & Hunger 2 Termina" --mode standard --output dist/termina/asset-audit-report.json`
  - Verify audit report is written and categorizes missing assets and case mismatches.
- [ ] **Step 2: Execute build for Termina**:
  - Command:
    `cargo run --bin runtimepack -- build "Raw-Game\Fear & Hunger 2 Termina" -o dist/termina --package com.example.termina --name "Fear & Hunger 2: Termina" --flavor beta --accept-audit-fail --accept-audit-unknown`
- [ ] **Step 3: Verify APK generation**:
  - Assert that `dist/termina/Termina.apk` exists, has size > 1 GB, and Gradle build finishes with exit code 0.
- [ ] **Step 4: Commit build documentation / verification record**:
  ```powershell
  git commit --allow-empty -m "chore: verify Fear & Hunger 2 Termina APK build success"
  ```
