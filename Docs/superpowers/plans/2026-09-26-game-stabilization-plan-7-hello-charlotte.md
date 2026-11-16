# Implementation Plan: Game Stabilization Plan 7 — Hello Charlotte EP1

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Validate and stabilize *Hello Charlotte EP1* (RPG Maker VX Ace / RGSS3 encrypted archive `Game.rgss3a`) to ensure archive integrity, font bundling, and clean boot execution.

**Architecture:** 
1. In `engines/rgss/src/asset_audit.rs`:
   - Inspect `Game.rgss3a` archive header (Magic `RGSSAD\0`, version 3).
   - If archive is valid and `Fonts/` contains bundled fonts (`VL-PGothic-Regular.ttf`, `VL-Gothic-Regular.ttf`, `coders_crux.ttf`), acknowledge font coverage and validate archive file bounds.
2. In `GpackExtractor.kt`:
   - Verify that `Game.rgss3a` is correctly unpacked and linked as `Game.rgss3a` in `destDir`.
   - Verify that custom font `coders_crux.ttf` and `VL-Gothic-Regular.ttf` are linked into `destDir/Fonts/`.
3. Build and verify `dist/hc_ep1/HelloCharlotteEP1.apk`.

**Tech Stack:** Rust (RGSS asset audit, RGSS3A parser), Kotlin (`GpackExtractor.kt`).

**Spec:** `Docs/superpowers/specs/2026-09-26-game-stabilization-and-unblock-design.md`

## Global Constraints

- Source directory in `Raw-Game/HC_EP1` remains strictly immutable.
- Build uses `--flavor beta`.
- All automated unit/integration tests must pass cleanly.
- Code edits, builds, and tests must be delegated to Sub-Agents.

---

### Task 1: RGSS3A Archive Header Validation & Font Verification

**Files:**
- Modify: `engines/rgss/src/asset_audit.rs`
- Test: `tests/integration/rgss_asset_audit_test.rs`

**Interfaces:**
- Produces:
  - Validates `Game.rgss3a` header and integrity.
  - Recognizes `Fonts/` directory contents in `HC_EP1` (`coders_crux.ttf`, `VL-Gothic-Regular.ttf`).

- [ ] **Step 1: Write integration tests**:
  - `rgss_asset_audit_test.rs`: Test game with valid `Game.rgss3a` and bundled `Fonts/` passes archive check cleanly.
- [ ] **Step 2: Run tests to verify failure**: `cargo test --test rgss_asset_audit_test`.
- [ ] **Step 3: Implement archive header validation in `engines/rgss/src/asset_audit.rs`**.
- [ ] **Step 4: Run tests to verify pass**: `cargo test --test rgss_asset_audit_test` and `cargo test --workspace`.
- [ ] **Step 5: Commit changes**:
  ```powershell
  git add engines/rgss/src/asset_audit.rs tests/integration/rgss_asset_audit_test.rs
  git commit -m "feat(rgss): validate RGSS3A archive header integrity and bundled fonts"
  ```

---

### Task 2: Build & Verify *Hello Charlotte EP1*

**Files:**
- Test target: `Raw-Game\HC_EP1`
- Build output: `dist/hc_ep1/HelloCharlotteEP1.apk`

- [ ] **Step 1: Run static audit on Hello Charlotte EP1**:
  ```powershell
  cargo run --bin runtimepack -- audit "Raw-Game\HC_EP1" --mode standard --output dist/hc_ep1/asset-audit-report.json
  ```
  - Verify archive integrity is validated.
- [ ] **Step 2: Execute build for Hello Charlotte EP1**:
  ```powershell
  cargo run --bin runtimepack -- build "Raw-Game\HC_EP1" -o dist/hc_ep1 --package com.example.hellocharlotte1 --name "Hello Charlotte EP1" --flavor beta --accept-audit-fail --accept-audit-unknown
  ```
- [ ] **Step 3: Verify APK generation**:
  - Assert that `dist/hc_ep1/HelloCharlotteEP1.apk` exists, has size > 300 MB, and Gradle build exits with 0.
- [ ] **Step 4: Commit build verification record**:
  ```powershell
  git commit --allow-empty -m "chore: verify Hello Charlotte EP1 APK build success"
  ```
