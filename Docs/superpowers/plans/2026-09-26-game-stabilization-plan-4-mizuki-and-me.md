# Implementation Plan: Game Stabilization Plan 4 — Mizuki And Me

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Package and stabilize *Mizuki And Me* (Ren'Py 8 / Python 3 visual novel) into a standalone Android APK by excluding Windows desktop wrapper DLLs, mapping standard Ren'Py python module stubs, and handling solid-color hex tags.

**Architecture:** 
1. In `engines/renpy/src/scanner.rs` and `engines/renpy/src/asset_audit.rs`:
   - Treat `lib/py3-windows-x86_64/` and `lib/py2-windows-x86_64/` as desktop runtime libraries to be excluded from Android packaging (`CMP-DESKTOP-EXCLUDED`), not fatal `CMP-102` blockers.
   - Recognize Ren'Py built-in color strings (e.g. `"#000"`, `"#fff"`, `"black"`) as solid-color displayables rather than missing disk file paths.
   - Standard Python modules (`inspect`, `zipfile`, `ast`, `subprocess`) are standard library modules, not external missing assets.
2. In `core/src/build_plan/generator.rs`:
   - Filter desktop libraries when packaging Ren'Py Android project.
3. Build and verify `dist/mizuki_and_me/MizukiAndMe.apk`.

**Tech Stack:** Rust (Ren'Py scanner, asset audit), Python (Ren'Py Android runtime).

**Spec:** `Docs/superpowers/specs/2026-09-26-game-stabilization-and-unblock-design.md`

## Global Constraints

- Source directory in `Raw-Game/Mizuki And Me` remains strictly immutable.
- Build uses `--flavor beta`.
- All automated unit/integration tests must pass cleanly.
- Code edits, builds, and tests must be delegated to Sub-Agents.

---

### Task 1: Desktop Library Exclusion & Ren'Py Built-in Syntax Parsing

**Files:**
- Modify: `engines/renpy/src/scanner.rs`
- Modify: `engines/renpy/src/asset_audit.rs`
- Test: `tests/integration/renpy_asset_audit_test.rs`
- Test: `tests/integration/compatibility_test.rs`

**Interfaces:**
- Produces:
  - `RenPyAnalyzer`: Windows DLLs inside `lib/*windows*` are marked as `CMP-DESKTOP-EXCLUDED` (warning), not `CMP-102` (fatal blocker).
  - `RenPyAssetAuditor`: Color strings (`#000`, `black`, `white`) parsed as color displayables, not disk files.
  - Python standard library modules (`ast`, `inspect`, `zipfile`, `subprocess`, etc.) recognized as standard library imports.

- [ ] **Step 1: Write integration tests**:
  - `renpy_asset_audit_test.rs`: Test script with `scene "#000"` and `import inspect` emits 0 missing asset errors.
  - `compatibility_test.rs`: Test Ren'Py project with `lib/py3-windows-x86_64/` passes compatibility check with desktop exclusion warning.
- [ ] **Step 2: Run tests to verify failure**: `cargo test --test renpy_asset_audit_test`.
- [ ] **Step 3: Implement desktop exclusion in `engines/renpy/src/scanner.rs`**.
- [ ] **Step 4: Implement color displayable and python stdlib parsing in `engines/renpy/src/asset_audit.rs`**.
- [ ] **Step 5: Run tests to verify pass**: `cargo test --test renpy_asset_audit_test` and `cargo test --workspace`.
- [ ] **Step 6: Commit changes**:
  ```powershell
  git add engines/renpy/src/scanner.rs engines/renpy/src/asset_audit.rs tests/integration/renpy_asset_audit_test.rs tests/integration/compatibility_test.rs
  git commit -m "feat(renpy): exclude desktop DLLs and recognize color displayables and stdlib modules"
  ```

---

### Task 2: Build APK & Verify *Mizuki And Me*

**Files:**
- Test target: `Raw-Game\Mizuki And Me`
- Build output: `dist/mizuki_and_me/MizukiAndMe.apk`

- [ ] **Step 1: Run static audit on Mizuki And Me**:
  ```powershell
  cargo run --bin runtimepack -- audit "Raw-Game\Mizuki And Me" --mode standard --output dist/mizuki_and_me/asset-audit-report.json
  ```
  - Verify missing errors drop to 0 and DLLs are classified as desktop-excluded.
- [ ] **Step 2: Execute build for Mizuki And Me**:
  ```powershell
  cargo run --bin runtimepack -- build "Raw-Game\Mizuki And Me" -o dist/mizuki_and_me --package com.cherrykiss.mizukiandme --name "Mizuki And Me" --flavor beta --accept-audit-fail --accept-audit-unknown
  ```
- [ ] **Step 3: Verify APK generation**:
  - Assert that `dist/mizuki_and_me/MizukiAndMe.apk` exists, has size > 500 MB, and Gradle build exits with 0.
- [ ] **Step 4: Commit build verification record**:
  ```powershell
  git commit --allow-empty -m "chore: verify Mizuki And Me APK build success"
  ```
