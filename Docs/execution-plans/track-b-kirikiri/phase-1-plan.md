# Implementation Plan: Kirikiri Engine Integration — Phase 1 (Static Detection & In-Memory XP3 Parser)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish foundational host-side support for Kirikiri 2 / Kirikiri Z by registering `EngineId::Kirikiri`, implementing an in-memory 11-byte `.xp3` binary archive parser with zlib TOC decompression, and building the static detector.

**Architecture:**
1. In `core/src/domain.rs` and registries, register `EngineId::Kirikiri` and `RuntimeFamily::KirikiriNative`.
2. Create `engines/kirikiri` crate containing `archive.rs` (a robust, zero-disk-extraction binary parser for standard Kirikiri `.xp3` archives based on official `XP3Archive.h` specs).
3. Implement `KirikiriDetector` in `engines/kirikiri` to detect canonical markers (`data.xp3`, `system.xp3`, `startup.tjs`, etc.) with deterministic confidence levels.
4. Integrate into `core::detection` and verify detection and archive parsing through comprehensive test suites.

**Tech Stack:** Rust (core, engines/kirikiri), `flate2`, `serde`.

**Spec:** `Docs/execution-plans/track-b-kirikiri/phase-1-design.md`

## Global Constraints

- Source directories in `Raw-Game/` remain strictly 100% immutable.
- Zero disk extraction: all `.xp3` archive TOC reading must operate in-memory via file slice reads without creating temporary files.
- Binary parsing must strictly adhere to the 11-byte signature `b"XP3\r\n \n\x1a\x8b\x67\x01"`.
- All subagents must run tests and verify zero regressions across the workspace before each commit.

---

### Task 1: Domain & Engine Registration in `core`

**Files:**
- Modify: `core/src/domain.rs`
- Modify: `core/src/diagnostics/mod.rs`
- Modify: `schemas/build-manifest.schema.json`
- Test: `tests/integration/detection_test.rs`

**Interfaces:**
- Produces:
  - `EngineId::Kirikiri` with `as_str() -> "kirikiri"` and display name `"Kirikiri 2 / Kirikiri Z"`.
  - `RuntimeFamily::KirikiriNative` with `as_str() -> "Kirikiri Native"`.
  - `DiagnosticCode::DET_KIRIKIRI_ENCRYPTED_XP3 = "DET-KRK-001"`.

- [ ] **Step 1: Write failing test in `detection_test.rs`**:
  ```rust
  #[test]
  fn test_kirikiri_engine_domain_id_and_runtime_family() {
      use runtimepack_core::domain::{EngineId, RuntimeFamily};
      let engine = EngineId::Kirikiri;
      assert_eq!(engine.as_str(), "kirikiri");
      assert_eq!(engine.runtime_family(), Some(RuntimeFamily::KirikiriNative));
  }
  ```
- [ ] **Step 2: Run test to verify failure**:
  `cargo test --test detection_test test_kirikiri_engine_domain_id_and_runtime_family`
- [ ] **Step 3: Implement domain types**:
  - In `core/src/domain.rs`:
    Add `Kirikiri` to `EngineId` enum (`#[serde(rename = "kirikiri")]`).
    Map `EngineId::Kirikiri` in `Display`, `as_str()`, and `runtime_family()` (`RuntimeFamily::KirikiriNative`).
    Add `KirikiriNative` to `RuntimeFamily` enum (`#[serde(rename = "Kirikiri Native")]`).
  - In `core/src/diagnostics/mod.rs`:
    Add `pub const DET_KIRIKIRI_ENCRYPTED_XP3: &'static str = "DET-KRK-001";`.
  - In `schemas/build-manifest.schema.json`:
    Add `"kirikiri"` to engine enum and `"Kirikiri Native"` to runtime family enum.
- [ ] **Step 4: Run test to verify pass**:
  `cargo test --test detection_test test_kirikiri_engine_domain_id_and_runtime_family`
- [ ] **Step 5: Commit**:
  ```powershell
  git add core/src/domain.rs core/src/diagnostics/mod.rs schemas/build-manifest.schema.json tests/integration/detection_test.rs
  git commit -m "feat(core): register EngineId::Kirikiri and RuntimeFamily::KirikiriNative"
  ```

---

### Task 2: In-Memory XP3 Binary Archive Parser (`engines/kirikiri/src/archive.rs`)

**Files:**
- Create: `engines/kirikiri/Cargo.toml`
- Create: `engines/kirikiri/src/archive.rs`
- Modify: `Cargo.toml` (add `engines/kirikiri` to workspace members)
- Test: `engines/kirikiri/src/archive.rs` (unit tests)

**Interfaces:**
- Produces:
  - `pub struct Xp3Entry`:
    ```rust
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Xp3Entry {
        pub path: String,
        pub original_size: u64,
        pub archived_size: u64,
        pub offset: u64,
        pub is_compressed: bool,
    }
    ```
  - `pub struct Xp3ArchiveIndex`:
    ```rust
    #[derive(Debug, Clone, Default)]
    pub struct Xp3ArchiveIndex {
        pub entries: HashMap<String, Xp3Entry>,
    }
    ```
  - `Xp3ArchiveIndex::parse(reader: &mut R) -> Result<Xp3ArchiveIndex, Xp3Error>`
  - `Xp3ArchiveIndex::parse_bytes(bytes: &[u8]) -> Result<Xp3ArchiveIndex, Xp3Error>`

- [ ] **Step 1: Create `engines/kirikiri/Cargo.toml` and update root `Cargo.toml`**:
  - Add `"engines/kirikiri"` to root `Cargo.toml` `members`.
  - Create `engines/kirikiri/Cargo.toml`:
    ```toml
    [package]
    name = "runtimepack-engine-kirikiri"
    version = "0.1.0"
    edition.workspace = true
    license.workspace = true

    [dependencies]
    runtimepack-core = { path = "../../core" }
    serde = { workspace = true }
    serde_json = { workspace = true }
    thiserror = { workspace = true }
    flate2 = { workspace = true }
    ```
- [ ] **Step 2: Write failing unit tests in `engines/kirikiri/src/archive.rs`**:
  - `test_xp3_valid_synthetic_archive_uncompressed`: Build synthetic 11-byte header, uncompressed TOC with `b"File"`, `b"info"`, `b"segm"`, `b"adlr"` chunks. Verify parse succeeds and returns exact file paths and sizes.
  - `test_xp3_valid_synthetic_archive_zlib_compressed`: Build synthetic XP3 with zlib-compressed TOC. Verify decompression and parsing.
  - `test_xp3_invalid_magic_rejected`: Verify corrupt header returns `Xp3Error::InvalidSignature`.
- [ ] **Step 3: Implement `engines/kirikiri/src/archive.rs`**:
  - Magic signature: `pub const XP3_MAGIC: &[u8; 11] = b"XP3\r\n \n\x1a\x8b\x67\x01";`.
  - Check 11-byte signature at offset 0 (or at 16-byte boundary if MZ-wrapped).
  - Read `index_offset: u64` (little-endian).
  - Read TOC block flags:
    - Bit 0: raw (`0`) or zlib (`1`).
    - Bit 7 (`0x80`): continuation block.
  - Decompress TOC bytes via `flate2::read::ZlibDecoder`.
  - Parse chunks in loop:
    - If tag is `b"File"`: parse nested chunks `b"info"`, `b"segm"`, `b"adlr"`.
    - In `b"info"`: read `original_size`, `archived_size`, `name_units: u16`, UTF-16LE filename string.
    - Normalize filename (`\` to `/`), check traversal.
    - Store `Xp3Entry` into `entries` map.
- [ ] **Step 4: Run unit tests to verify pass**:
  `cargo test -p runtimepack-engine-kirikiri`
- [ ] **Step 5: Commit**:
  ```powershell
  git add Cargo.toml Cargo.lock engines/kirikiri/Cargo.toml engines/kirikiri/src/archive.rs
  git commit -m "feat(kirikiri): implement in-memory XP3 binary archive parser with zlib TOC decoding"
  ```

---

### Task 3: Kirikiri Static Detector (`engines/kirikiri/src/lib.rs`)

**Files:**
- Create: `engines/kirikiri/src/lib.rs`
- Modify: `core/src/detection/mod.rs`
- Test: `tests/integration/detection_test.rs`

**Interfaces:**
- Produces:
  - `pub struct KirikiriDetector;` implementing `runtimepack_core::detection::Detector`.
  - Registered in `create_default_detector_registry()`.

- [ ] **Step 1: Write integration tests in `detection_test.rs`**:
  - `test_detect_kirikiri_with_data_xp3_and_startup_tjs`: Directory with `data.xp3` and `startup.tjs` -> returns `EngineId::Kirikiri`, `DetectionConfidence::Exact`.
  - `test_detect_kirikiri_with_xp3_only`: Directory with `patch.xp3` -> returns `EngineId::Kirikiri`, `DetectionConfidence::High`.
  - `test_detect_kirikiri_negative`: Directory without any Kirikiri markers -> returns `DetectionConfidence::None`.
- [ ] **Step 2: Run test to verify failure**:
  `cargo test --test detection_test test_detect_kirikiri_with_data_xp3_and_startup_tjs`
- [ ] **Step 3: Implement `KirikiriDetector` in `engines/kirikiri/src/lib.rs`**:
  ```rust
  pub struct KirikiriDetector;

  impl Detector for KirikiriDetector {
      fn id(&self) -> &'static str {
          "kirikiri-static-detector"
      }

      fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate {
          // Score detection:
          // data.xp3 -> +90 pts, DET-KRK-001
          // startup.tjs -> +80 pts, DET-KRK-002
          // any other *.xp3 -> +50 pts, DET-KRK-003
          // tvpwin32.exe / kagex.exe -> +30 pts, DET-KRK-004
          // Confidence: Exact if score >= 90, High if score >= 60, Medium if score >= 30, else None
      }
  }
  ```
  - In `core/src/detection/mod.rs`: Add `KirikiriDetector` to `create_default_detector_registry()`.
  - In `core/Cargo.toml`: Add `runtimepack-engine-kirikiri = { path = "../engines/kirikiri" }`.
- [ ] **Step 4: Run tests to verify pass**:
  `cargo test --test detection_test`
- [ ] **Step 5: Commit**:
  ```powershell
  git add engines/kirikiri/src/lib.rs core/src/detection/mod.rs core/Cargo.toml tests/integration/detection_test.rs
  git commit -m "feat(kirikiri): implement static engine detector for Kirikiri games"
  ```

---

### Task 4: CLI Wiring, Ingest Virtual Discovery, & Workspace Verification

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `core/src/ingest/mod.rs`
- Test: `tests/integration/kirikiri_test.rs`

**Interfaces:**
- Produces:
  - `GameSourceIndex`: When ingesting a game, if `*.xp3` archives are present, optionally parses internal file entries using `Xp3ArchiveIndex` for static asset checking.
  - `runtimepack inspect <game>` correctly recognizes Kirikiri games and displays detected evidence.

- [ ] **Step 1: Write comprehensive integration test in `tests/integration/kirikiri_test.rs`**:
  - Test end-to-end `inspect` workflow on a synthetic Kirikiri game directory containing `data.xp3` (with internal `startup.tjs`, `system.tjs`, and `title.png`).
  - Assert detector outputs `EngineId::Kirikiri` with `DetectionConfidence::Exact`.
  - Assert that internal XP3 file index is parsed and verified.
- [ ] **Step 2: Run test to verify failure**:
  `cargo test --test kirikiri_test`
- [ ] **Step 3: Wire into `cli/src/main.rs` and `core/src/ingest/mod.rs`**:
  - Update `cli/src/main.rs` to include `runtimepack-engine-kirikiri` in default analyzers/detectors.
  - Ensure `GameSourceIndex` or analyzer can access `Xp3ArchiveIndex`.
- [ ] **Step 4: Run full workspace test suite**:
  `cargo test --workspace`
  Ensure 100% of workspace tests pass.
- [ ] **Step 5: Commit and update documentation**:
  ```powershell
  git add cli/src/main.rs core/src/ingest/mod.rs tests/integration/kirikiri_test.rs memory/progress.md memory/decisions_log.md
  git commit -m "feat(cli): wire Kirikiri detector and in-memory XP3 discovery into host CLI"
  ```
