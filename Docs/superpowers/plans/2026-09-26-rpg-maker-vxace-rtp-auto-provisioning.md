# Implementation Plan: RPG Maker VX Ace RTP Auto-Provisioning & Native Asset Ingestion

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable RuntimePack to automatically detect RTP requirements (`rtp = RPGVXAce` in `Game.ini`), resolve missing standard assets from a canonical shared RTP repository (`runtimes/rtp/vxace/` or system Enterbrain directory), and package them into `game.pack` at build time so games render 100% identically to PC without requiring manual asset copying or falling back to placeholder textures.

**Architecture:**
1. **RTP Store & Detection (`core/src/ingest/rtp.rs`):**
   - Automatically locate standard Enterbrain RTP directory from Windows standard path (`C:\Program Files (x86)\Common Files\Enterbrain\RGSS3\RPGVXAce`), user-configured environment variable (`RPGVXACE_RTP_PATH`), or fallback repository store (`runtimes/rtp/vxace/`).
   - Parse `Game.ini` during ingest: if `rtp = RPGVXAce` is present and RTP directory exists, activate automatic RTP resolution.
2. **Selective Tree-Shaken Asset Merging (`core/src/ingest/mod.rs` & `cli/src/main.rs`):**
   - Cross-reference assets referenced in game data with RTP assets.
   - Only merge the specific missing RTP assets needed by the game (tree-shaking) into `GameSourceIndex` without modifying the user's source directory (`Raw-Game/BLACK SOULS` remains 100% immutable).
   - Package merged assets into `game.pack` during build.
3. **Validation on Black Souls 2:**
   - Run `runtimepack audit "Raw-Game\BLACK SOULS"`: verify RTP missing count drops from 396 to 0.
   - Compile `dist/black_souls/BlackSouls.apk`: verify title screen renders `Mist.png` brush strokes, and Map 101 renders green carpets, bookshelves, tables, and bonfire save points.

**Tech Stack:** Rust (core ingest, RTP scanner, content packer), Android APK packaging.

## Global Constraints

- Game source directory in `Raw-Game/BLACK SOULS` remains strictly immutable.
- Build flavor defaults to `--flavor beta`.
- All automated unit/integration tests must pass cleanly.
- Code edits, builds, and tests must be delegated to Sub-Agents.

---

### Task 1: Canonical RTP Discovery & Local Store Setup

**Files:**
- Create: `core/src/ingest/rtp.rs`
- Modify: `core/src/ingest/mod.rs`
- Modify: `core/src/lib.rs`
- Test: `tests/integration/asset_audit_cli_test.rs`

**Interfaces:**
- Produces:
  - `pub fn find_system_rtp_path(rtp_name: &str) -> Option<PathBuf>`
    Searches:
    1. CLI explicit flag `--rtp-path <dir>` (if provided).
    2. Environment variable (e.g. `RPGVXACE_RTP_PATH`, `RPGVX_RTP_PATH`, `RPGXP_RTP_PATH`).
    3. Project-local store `runtimes/rtp/<engine_family>/` (e.g. `runtimes/rtp/vxace/`).
    4. Windows registry / Common Files: `CommonProgramFiles(x86)\Enterbrain\RGSS3\RPGVXAce` or `C:\Program Files (x86)\Common Files\Enterbrain\RGSS3\RPGVXAce`.
  - `GameSourceIndex::build_with_auto_rtp(game_dir: &Path, explicit_rtp: Option<&Path>)`:
    Parses `Game.ini` for `rtp = ...`. If found, automatically calls `find_system_rtp_path` and merges missing files.

- [x] **Step 1: Write integration tests** in `asset_audit_cli_test.rs`:
  - Test `find_system_rtp_path` resolves local and standard directories.
  - Test game with `rtp = RPGVXAce` automatically discovers RTP without requiring manual `--rtp-path` flag.
- [x] **Step 2: Implement RTP discovery in `core/src/ingest/rtp.rs`**.
- [x] **Step 3: Update `core/src/ingest/mod.rs`**:
  - Integrate auto-discovery into `GameSourceIndex::build_with_auto_rtp`.
- [x] **Step 4: Run tests to verify pass**:
  - `cargo test --test asset_audit_cli_test`
  - `cargo test --workspace`

---

### Task 2: Selective Tree-Shaken Asset Merging into `game.pack`

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `cli/src/audit_command.rs`
- Modify: `core/src/ingest/mod.rs`
- Modify: `tests/integration/build_test.rs`

**Interfaces:**
- Produces:
  - Selective merging: when RTP is available, only merge files that are actually referenced or needed, avoiding bloat.
  - Build pipeline packages the merged RTP assets directly into `game.pack`.
  - Audit report and build manifest record `rtp_source` and list of merged RTP assets in provenance.

- [x] **Step 1: Write integration tests** in `build_test.rs`:
  - Test that building a game requiring RTP bundles the needed RTP tilesets and character sprites into `game.pack`.
  - Test that source game directory remains 100% untouched.
- [x] **Step 2: Update `cli/src/main.rs` & `cli/src/audit_command.rs`**:
  - Enable auto-RTP resolution in `cmd_build` and `cmd_audit`.
  - Record RTP ingestion provenance in `BuildManifest`.
- [x] **Step 3: Run tests to verify pass**:
  - `cargo test --test build_test`
  - `cargo test --workspace`

---

### Task 3: Provision VX Ace RTP Assets & Verify on Black Souls

**Files:**
- Directory: `runtimes/rtp/vxace/` (or system Enterbrain directory)
- Build Output: `dist/black_souls/BlackSouls.apk`

- [ ] **Step 1: Check/provision RTP assets on host machine**:
  - Locate Enterbrain VX Ace RTP files (`Graphics/Tilesets/Inside_A2.png`, `Inside_B.png`, `Graphics/Titles2/Mist.png`, `Graphics/Characters/!Flame.png`, etc.).
  - If installed on Windows, point RuntimePack to it or link to `runtimes/rtp/vxace/`.
- [ ] **Step 2: Run CLI audit verification**:
  - Run: `cargo run --bin runtimepack -- audit "Raw-Game\BLACK SOULS" --mode standard`
  - Verify that RTP missing count drops to 0 (only the 12 custom non-RTP assets remain).
- [ ] **Step 3: Rebuild Black Souls standalone APK**:
  - Run: `cargo run --bin runtimepack -- build "Raw-Game\BLACK SOULS" -o dist/black_souls --package com.example.blacksouls --name "Black Souls" --flavor beta --accept-audit-fail`
  - Verify `game.pack` includes all required tilesets, characters, and title assets.
- [ ] **Step 4: Install to MuMu Player & hand off to user for live playtesting**:
  - Install updated APK on MuMu Player 12.
  - Verify in-game:
    1. Title screen displays the white ink brush effect (`Mist.png`).
    2. Map 101 displays green carpet, wooden tables, chairs, and bookshelves.
    3. Save point bonfire (`!Flame.png`) is visible.
