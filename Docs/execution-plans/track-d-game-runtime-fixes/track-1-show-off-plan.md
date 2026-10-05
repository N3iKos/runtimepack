# Implementation Plan: Track 1 — Show Off (TyranoScript) Runtime Fix

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the ASAR data-offset bug that shifts every file in `game.pack` by padding bytes, rebuild `ShowOff.apk`, and verify the game renders and plays on the MuMu emulator.

**Architecture:** Correct `engines/tyrano/src/asar.rs` to use `data_offset = 8 + header_size` (4-byte-aligned ASAR header), add unit tests for both aligned and unaligned `json_len`, add a post-pack SHA-256 round-trip integrity gate, rebuild, and verify on-device.

**Tech Stack:** Rust (`engines/tyrano`, `cli`, `pack`), Gradle, MuMu emulator (`127.0.0.1:16384`).

**Spec:** `Docs/execution-plans/track-d-game-runtime-fixes/design.md`

## Global Constraints

- Source game in `Raw-Game/[Kimochi] show_off` remains strictly 100% immutable.
- Builds use `--flavor beta`.
- NTRdemic (aligned `json_len`) must remain working — no regression.
- Root cause is proven: `data_offset` must be `8 + header_size`, not `16 + json_len`.

---

### Task 1: Fix ASAR Data Offset & Add Padding Unit Tests

**Files:**
- Modify: `engines/tyrano/src/asar.rs` (line ~30 `_header_size`, line ~59 `data_offset`)
- Test: `engines/tyrano/src/asar.rs` (unit tests module)

**Interfaces:**
- Produces:
  - `AsarArchive::load` uses `data_offset = 8 + header_size` for every entry.
  - `AsarArchive::read_file_bytes` / `read_file_string_bounded` return exact original bytes.
  - Unit test: archive with `json_len % 4 == 0` (delta 0) AND `json_len % 4 != 0` (delta > 0) both round-trip byte-exact.

- [ ] **Step 1: Write failing unit tests**
  In `engines/tyrano/src/asar.rs` tests module, add:
  ```rust
  #[test]
  fn test_asar_data_offset_with_4byte_padding() {
      // Build synthetic asar where json_len % 4 != 0 (needs padding),
      // assert read_file_bytes returns the exact original payload.
  }

  #[test]
  fn test_asar_data_offset_without_padding() {
      // Build synthetic asar where json_len % 4 == 0,
      // assert read_file_bytes returns the exact original payload.
  }
  ```
- [ ] **Step 2: Run tests to verify failure**
  Run: `cargo test -p runtimepack-engine-tyrano asar`
  Expected: the padded test FAILS (payload shifted).
- [ ] **Step 3: Implement the fix**
  In `engines/tyrano/src/asar.rs`:
  - Keep the real header size value (currently `_header_size`).
  - Compute `let data_offset = 8 + header_size as u64;` for entry payloads.
- [ ] **Step 4: Run tests to verify pass**
  Run: `cargo test -p runtimepack-engine-tyrano`
  Expected: PASS, including both new tests.
- [ ] **Step 5: Commit**
  ```powershell
  git add engines/tyrano/src/asar.rs
  git commit -m "fix(tyrano): correct ASAR data offset to account for 4-byte header padding"
  ```

---

### Task 2: Post-Pack Integrity Gate (SHA-256 Round-Trip)

**Files:**
- Modify: `cli/src/main.rs` (asar extraction → staging → pack pipeline, ~763-796)
- Test: `tests/integration/build_test.rs`

**Interfaces:**
- Produces:
  - After building `game.pack` from an extracted ASAR, verify a sample of entries round-trip byte-exact (SHA-256 match) against the original archive payloads; abort the build with a stable diagnostic on mismatch.

- [ ] **Step 1: Write failing integration test**
  In `tests/integration/build_test.rs`, add a test that builds a synthetic Tyrano asar with unaligned `json_len`, runs the pack pipeline, and asserts the packed entries match the source bytes.
- [ ] **Step 2: Run test to verify failure (before gate)**
  Run: `cargo test --test build_test test_asar_pack_roundtrip_integrity`
- [ ] **Step 3: Implement the integrity gate**
  Add post-pack verification comparing source ASAR payload hashes with `game.pack` entry hashes; emit `CNT-102`-class diagnostic on mismatch.
- [ ] **Step 4: Run test to verify pass**
  Run: `cargo test --test build_test` and `cargo test --workspace`
- [ ] **Step 5: Commit**
  ```powershell
  git add cli/src/main.rs tests/integration/build_test.rs
  git commit -m "feat(pack): add post-pack SHA-256 round-trip integrity gate for ASAR-sourced content"
  ```

---

### Task 3: Rebuild & Verify Show Off on MuMu

**Files:**
- Output: `dist/show_off/ShowOff.apk`

- [ ] **Step 1: Rebuild**
  Run: `cargo run --bin runtimepack -- build "Raw-Game\[Kimochi] show_off" -o dist/show_off --package com.example.showoff --name "Show Off" --flavor beta --accept-audit-fail --accept-audit-unknown`
  Verify Gradle exit 0 and `ShowOff.apk` produced.
- [ ] **Step 2: Install on MuMu**
  Run: `adb -s 127.0.0.1:16384 install -r dist/show_off/ShowOff.apk`
- [ ] **Step 3: Launch & capture**
  Launch `com.example.showoff`, capture logcat + screenshot. Verify: title screen renders (not black), no `Uncaught SyntaxError`, `TYRANO defined` is truthy, images load (`Total imgs > 0`).
- [ ] **Step 4: Verify stability**
  Confirm audio, tap input, and a scene transition work without crash.
- [ ] **Step 5: Commit verification record**
  ```powershell
  git commit --allow-empty -m "chore: verify Show Off Tyrano runtime fix on MuMu emulator"
  ```
