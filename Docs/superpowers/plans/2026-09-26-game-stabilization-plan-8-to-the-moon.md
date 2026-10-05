# Implementation Plan: Game Stabilization Plan 8 — To the Moon

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Validate and stabilize *To the Moon* (RPG Maker XP / RGSS1 encrypted archive `To the Moon.rgssad`) to ensure archive aliasing, font mapping, and Ruby 1.8 bootstrap execution.

**Architecture:** 
1. In `engines/rgss/src/asset_audit.rs`:
   - Inspect `To the Moon.rgssad` (RGSS1 archive magic `RGSSAD\0`, version 1).
   - Recognize custom fonts (`WenQuanYi_Zen_Hei.ttf`, `NanumBarunGothic.ttf`, `OpenSans.ttf`) in `Fonts/`.
2. In `GpackExtractor.kt`:
   - Parse `mkxp.conf` if present: extract `execName = "To the Moon"` and symlink `To the Moon.rgssad -> Game.rgssad` for mkxp-z discovery.
3. Build and verify `dist/to_the_moon/TotheMoon.apk`.

**Tech Stack:** Rust (RGSS asset audit, RGSSAD parser), Kotlin (`GpackExtractor.kt`), Ruby (`runtimepack_bootstrap.rb` Ruby 1.8 shims).

**Spec:** `Docs/superpowers/specs/2026-09-26-game-stabilization-and-unblock-design.md`

## Global Constraints

- Source directory in `Raw-Game/To the Moon` remains strictly immutable.
- Build uses `--flavor beta`.
- All automated unit/integration tests must pass cleanly.
- Code edits, builds, and tests must be delegated to Sub-Agents.

---

### Task 1: RGSSAD Archive Validation & mkxp.conf Mapping

**Files:**
- Modify: `engines/rgss/src/asset_audit.rs`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt`
- Test: `tests/integration/rgss_asset_audit_test.rs`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/GpackExtractorTest.kt`

**Interfaces:**
- Produces:
  - Validates `*.rgssad` header (Magic `RGSSAD\0`, version 1).
  - Verifies `GpackExtractor.kt` extracts and creates symlink for `To the Moon.rgssad`.

- [ ] **Step 1: Write integration tests**:
  - `rgss_asset_audit_test.rs`: Test game with valid `*.rgssad` passes archive header check.
- [ ] **Step 2: Run tests to verify failure**: `cargo test --test rgss_asset_audit_test`.
- [ ] **Step 3: Implement RGSSAD validation in `engines/rgss/src/asset_audit.rs`**.
- [ ] **Step 4: Run tests to verify pass**: `cargo test --test rgss_asset_audit_test` and `cargo test --workspace`.
- [ ] **Step 5: Commit changes**:
  ```powershell
  git add engines/rgss/src/asset_audit.rs android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt tests/integration/rgss_asset_audit_test.rs
  git commit -m "feat(rgss): validate RGSSAD archive and ensure mkxp.conf symlink mapping"
  ```

---

### Task 2: Build & Verify *To the Moon*

**Files:**
- Test target: `Raw-Game\To the Moon`
- Build output: `dist/to_the_moon/TotheMoon.apk`

- [ ] **Step 1: Run static audit on To the Moon**:
  ```powershell
  cargo run --bin runtimepack -- audit "Raw-Game\To the Moon" --mode standard --output dist/to_the_moon/asset-audit-report.json
  ```
  - Verify archive integrity is validated.
- [ ] **Step 2: Execute build for To the Moon**:
  ```powershell
  cargo run --bin runtimepack -- build "Raw-Game\To the Moon" -o dist/to_the_moon --package com.example.tothemoon --name "To the Moon" --flavor beta --accept-audit-fail --accept-audit-unknown
  ```
- [ ] **Step 3: Verify APK generation**:
  - Assert that `dist/to_the_moon/TotheMoon.apk` exists, has size > 150 MB, and Gradle build exits with 0.
- [ ] **Step 4: Commit build verification record**:
  ```powershell
  git commit --allow-empty -m "chore: verify To the Moon APK build success"
  ```
