# Implementation Plan: Black Souls RTP Ingestion, Smart Fallbacks & Fog Occlusion Mitigation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve the 4,500+ missing RTP asset errors, pitch-black floor rendering, and missing save/chest sprites in Black Souls and other RGSS games by implementing CLI `--rtp-path` ingestion, smart autotile/character sprite fallback generation, and OpenGL ES subtractive fog mitigations.

**Architecture:**
1. **CLI & Build Layer (`cli/src/main.rs`, `core/src/ingest/`):** Add `--rtp-path <dir>` to `runtimepack audit` and `runtimepack build`. When specified, assets from the RTP directory are indexed into the game source and packaged into `game.pack`.
2. **Ruby Runtime Fallback Layer (`runtimepack_bootstrap.rb`):**
   - When a tileset image fails to load, generate an autotile-compatible grid texture rather than an invisible 32x32 transparent square.
   - When essential interactive character sprites (`!Flame`, `!Chest`, `!Door*`) fail to load, generate a visible labeled sprite grid so save points and doors are never invisible.
   - For `Plane#blend_type = 2` (subtractive) at $Z < 32$, clamp opacity and blend mode to prevent crushing framebuffer pixels to pitch black in OpenGL ES.
3. **Verification Layer:** Run CLI audit on Black Souls, verify error reduction to minor/custom assets only, rebuild APK, and hand off to user for live testing.

**Tech Stack:** Rust (CLI, ingest, packer), Ruby (RGSS bootstrap & smart fallbacks).

## Global Constraints

- Game source in `Raw-Game/BLACK SOULS` remains 100% immutable.
- Build uses `--flavor beta`.
- All automated unit/integration tests must pass cleanly.
- Code edits and builds MUST be performed by Sub-Agents.

---

### Task 1: CLI `--rtp-path` Ingestion for Build & Audit

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `cli/src/audit_command.rs`
- Modify: `core/src/ingest/mod.rs`
- Modify: `tests/integration/asset_audit_cli_test.rs`
- Modify: `tests/integration/build_test.rs`

**Interfaces:**
- Produces:
  - CLI flag `--rtp-path <DIR>` for `runtimepack audit` and `runtimepack build`.
  - In `core/src/ingest/mod.rs`: `GameSourceIndex::build_with_rtp(game_dir, rtp_dir)` merges missing asset paths from RTP without modifying game directory.
  - `ContentPacker` packs merged assets into `game.pack`.

- [ ] **Step 1: Write integration tests**:
  - `asset_audit_cli_test.rs`: test `runtimepack audit` with `--rtp-path` resolves assets present in RTP folder.
  - `build_test.rs`: test `runtimepack build` with `--rtp-path` includes RTP assets in `game.pack`.
- [ ] **Step 2: Implement RTP ingestion in `core/src/ingest/mod.rs`**:
  - If `rtp_dir` is provided, scan `rtp_dir` and for any file whose relative path does not exist in `game_dir`, add it to `GameSourceIndex` with source marked as RTP.
- [ ] **Step 3: Update `cli/src/main.rs` and `cli/src/audit_command.rs`**:
  - Add `--rtp-path` argument.
  - Pass RTP path to audit runner and build planner.
- [ ] **Step 4: Run tests to verify pass**:
  - `cargo test --test asset_audit_cli_test`
  - `cargo test --test integration_build`
  - `cargo test --workspace`

---

### Task 2: Smart Autotile & Interactive Character Fallbacks in RGSS Bootstrap

**Files:**
- Modify: `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
- Modify: `android/shell/app/src/test/ruby/asset_audit_test.rb`

**Interfaces:**
- Produces:
  - `create_smart_tileset_fallback(name)` creates a 512x384 checkered tile texture with distinct sub-tile borders so autotiles render as visible floor/walls rather than transparent void.
  - `create_smart_character_fallback(name)` creates a 128x128 3x4 character sprite sheet with a visible labeled icon (e.g. `[SAVE]`, `[CHEST]`, `[DOOR]`, `[NPC]`) so essential interactables are never invisible.

- [x] **Step 1: Write failing Ruby tests** in `asset_audit_test.rb`:
  - Test that missing tileset returns 512x384 textured bitmap.
  - Test that missing `!Flame` returns 3x4 character sheet with non-zero dimensions.
- [x] **Step 2: Implement smart fallbacks in `runtimepack_bootstrap.rb`**:
  - If `path` matches `Graphics/Tilesets/`, generate 512x384 checkered tile pattern.
  - If `path` matches `Graphics/Characters/`, generate 3x4 sprite grid with visible marker.
- [x] **Step 3: Run Ruby tests to verify pass**:
  - `ruby android/shell/app/src/test/ruby/asset_audit_test.rb`

---

### Task 3: Subtractive Fog Plane Mitigations in OpenGL ES

**Files:**
- Modify: `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
- Modify: `android/shell/app/src/test/ruby/asset_audit_test.rb`

**Interfaces:**
- Produces:
  - Safe hook for `Plane#blend_type=` and `Plane#opacity=`:
    - When `blend_type == 2` (subtractive) and `plane.z < 32` (below characters), clamp maximum opacity to 80 (or adjust blend mode) so OpenGL ES does not clamp the floor pixels to pure pitch black.

- [x] **Step 1: Write Ruby test** in `asset_audit_test.rb`:
  - Test that `Plane#blend_type = 2` with low Z clamps opacity or applies safety adjustment.
- [x] **Step 2: Implement Plane blend clamp in `runtimepack_bootstrap.rb`**.
- [x] **Step 3: Run Ruby tests to verify pass**:
  - `ruby android/shell/app/src/test/ruby/asset_audit_test.rb`

---

### Task 4: CLI Audit Verification & Standalone APK Rebuild

**Files:**
- Build output: `dist/black_souls/BlackSouls.apk`

- [ ] **Step 1: Run CLI audit with RTP path**:
  - `cargo run --bin runtimepack -- audit "Raw-Game\BLACK SOULS" --rtp-path "Raw-Game\HC_EP1" --mode standard`
  - Verify error count drops from 4,525 to ~26 (only custom non-RTP assets remain).
- [ ] **Step 2: Build standalone APK with RTP path**:
  - `cargo run --bin runtimepack -- build "Raw-Game\BLACK SOULS" --rtp-path "Raw-Game\HC_EP1" -o dist/black_souls --package com.example.blacksouls --name "Black Souls" --flavor beta --accept-audit-fail`
- [ ] **Step 3: Hand off to user for live playtesting**.
