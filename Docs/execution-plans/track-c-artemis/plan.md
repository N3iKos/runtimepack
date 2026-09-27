# Implementation Plan: Artemis Engine (PFS / Lua) Support

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement end-to-end support for Artemis Engine visual novels by registering `EngineId::Artemis`, creating an in-memory `.pfs` (PF8, PF6, PF2) binary archive parser with SHA-1 XOR decryption, building the static detector and script auditor, and wiring compilation to the `art3m1s-core` C FFI native runtime (`ArtemisActivity.kt`).

**Architecture:**
1. In `core/src/domain.rs` and registries, register `EngineId::Artemis` with `RuntimeFamily::ArtemisNative`.
2. Create `engines/artemis` crate containing `archive.rs` (zero-disk-extraction parser for PF8, PF6, and PF2 with SHA-1 XOR stream cipher and multi-volume patch chain resolution).
3. Implement `ArtemisDetector` and `ArtemisAssetAuditor` in `engines/artemis` to detect markers (`root.pfs`, `system.ini`) and audit `.ast` script asset references.
4. Integrate native C FFI runtime bindings (`libart3m1s.so` built with NDK r28, 16 KB page alignment) and `ArtemisActivity.kt` in the Android host shell with save data isolation to `filesDir/saves/<title-id>`.

**Tech Stack:** Rust (core, engines/artemis), `sha1` (RustCrypto), Kotlin (`ArtemisActivity.kt`), C FFI (`art3m1s-core` ABI v1).

**Spec:** `Docs/execution-plans/track-c-artemis/design.md`
**Technical Report:** `Docs/research/RuntimePack_Artemis_Research.md`

## Global Constraints

- Source directories in `Raw-Game/` remain strictly 100% immutable.
- Zero disk extraction: all `.pfs` archive TOC reading and payload hashing operates in-memory.
- Native shared libraries must have 16 KB ELF segment alignment (`max-page-size=16384`).
- Save data must be isolated strictly to `Context.filesDir + "/saves/<title-id>"`.
- Autonomous subagents must build with `--flavor beta`.
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
  - `EngineId::Artemis` with `as_str() -> "artemis"` and display name `"Artemis Engine"`.
  - `RuntimeFamily::ArtemisNative` with `as_str() -> "Artemis Native"`.
  - `DiagnosticCode::DET_ART_ENCRYPTED_PFS = "DET-ART-001"`.

- [ ] **Step 1: Write failing test in `detection_test.rs`**:
  ```rust
  #[test]
  fn test_artemis_engine_domain_id_and_runtime_family() {
      use runtimepack_core::domain::{EngineId, RuntimeFamily};
      let engine = EngineId::Artemis;
      assert_eq!(engine.as_str(), "artemis");
      assert_eq!(engine.runtime_family(), Some(RuntimeFamily::ArtemisNative));
  }
  ```
- [ ] **Step 2: Run test to verify failure**:
  `cargo test --test detection_test test_artemis_engine_domain_id_and_runtime_family`
- [ ] **Step 3: Implement domain types**:
  - In `core/src/domain.rs`:
    Add `Artemis` to `EngineId` enum (`#[serde(rename = "artemis")]`).
    Map in `Display`, `as_str()`, and `runtime_family()` (`Some(RuntimeFamily::ArtemisNative)`).
    Add `ArtemisNative` to `RuntimeFamily` enum (`#[serde(rename = "Artemis Native")]`).
  - In `core/src/diagnostics/mod.rs`:
    Add `pub const DET_ART_ENCRYPTED_PFS: &'static str = "DET-ART-001";`.
  - In `schemas/build-manifest.schema.json`:
    Add `"artemis"` and `"Artemis Native"` to respective enum lists.
- [ ] **Step 4: Run test to verify pass**:
  `cargo test --test detection_test test_artemis_engine_domain_id_and_runtime_family`
- [ ] **Step 5: Commit**:
  ```powershell
  git add core/src/domain.rs core/src/diagnostics/mod.rs schemas/build-manifest.schema.json tests/integration/detection_test.rs
  git commit -m "feat(core): register EngineId::Artemis and RuntimeFamily::ArtemisNative"
  ```

---

### Task 2: In-Memory PFS Binary Archive Parser (`engines/artemis/src/archive.rs`)

**Files:**
- Create: `engines/artemis/Cargo.toml`
- Create: `engines/artemis/src/archive.rs`
- Modify: `Cargo.toml` (add `engines/artemis` to workspace members)
- Test: `engines/artemis/src/archive.rs` (unit tests)

**Interfaces:**
- Produces:
  - `pub struct PfsEntry`:
    ```rust
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PfsEntry {
        pub path: String,
        pub offset: u64,
        pub stored_size: u64,
        pub is_encrypted: bool,
    }
    ```
  - `pub struct PfsArchiveIndex`:
    ```rust
    #[derive(Debug, Clone, Default)]
    pub struct PfsArchiveIndex {
        pub version: u8,
        pub entries: HashMap<String, PfsEntry>,
        pub key: Option<[u8; 20]>,
    }
    ```
  - `PfsArchiveIndex::parse(reader: &mut R) -> Result<PfsArchiveIndex, PfsError>`
  - `PfsArchiveIndex::parse_bytes(bytes: &[u8]) -> Result<PfsArchiveIndex, PfsError>`
  - Decrypt helper: `decrypt_payload(bytes: &[u8], key: &[u8; 20], start_offset: usize) -> Vec<u8>`.

- [ ] **Step 1: Create `engines/artemis/Cargo.toml` and update root `Cargo.toml`**:
  - Add `"engines/artemis"` to root `Cargo.toml` `members`.
  - Create `engines/artemis/Cargo.toml`:
    ```toml
    [package]
    name = "runtimepack-engine-artemis"
    version = "0.1.0"
    edition.workspace = true
    license.workspace = true

    [dependencies]
    runtimepack-core = { path = "../../core" }
    serde = { workspace = true }
    serde_json = { workspace = true }
    thiserror = { workspace = true }
    sha1 = "0.10"
    encoding_rs = { workspace = true }
    ```
- [ ] **Step 2: Write failing unit tests in `engines/artemis/src/archive.rs`**:
  - `test_pfs_pf8_with_sha1_xor_encryption`:
    Build synthetic PF8 archive with 2 files, compute SHA-1 of index block, encrypt payload with rolling 20-byte XOR key.
    Verify `parse_bytes` successfully decodes entry names, offsets, and correctly decrypts payload plaintext.
  - `test_pfs_pf6_and_pf2_unencrypted`:
    Verify PF6 and PF2 synthetic archives parse entries with correct plaintext offsets.
  - `test_pfs_invalid_magic_rejected`:
    Verify non-PFS magic returns `PfsError::InvalidMagic`.
- [ ] **Step 3: Implement `engines/artemis/src/archive.rs`**:
  - Verify magic (`b"pf8"`, `b"pf6"`, `b"pf2"`).
  - Parse `index_size` at `0x03..0x07`.
  - Parse `entry_count` at `0x07` (PF8/PF6) or `0x0B` (PF2).
  - Loop entries, decode name bytes with CP932 / Shift-JIS fallback to UTF-8.
  - For PF8: compute `K = Sha1::digest(&index_bytes)`, store key.
  - Implement multi-volume merger: `merge_volumes(volumes: &[PfsArchiveIndex]) -> PfsArchiveIndex` (highest volume index wins).
- [ ] **Step 4: Run unit tests to verify pass**:
  `cargo test -p runtimepack-engine-artemis`
- [ ] **Step 5: Commit**:
  ```powershell
  git add Cargo.toml Cargo.lock engines/artemis/Cargo.toml engines/artemis/src/archive.rs
  git commit -m "feat(artemis): implement in-memory PFS (PF8/PF6/PF2) binary archive parser with SHA-1 XOR stream cipher"
  ```

---

### Task 3: Static Detector & Script Auditor (`engines/artemis/src/lib.rs`)

**Files:**
- Create: `engines/artemis/src/lib.rs`
- Create: `engines/artemis/src/asset_audit.rs`
- Modify: `core/src/detection/mod.rs`
- Modify: `core/src/asset_audit/mod.rs`
- Test: `tests/integration/artemis_test.rs`

**Interfaces:**
- Produces:
  - `pub struct ArtemisDetector;` implementing `runtimepack_core::detection::Detector`.
  - `pub struct ArtemisAssetAuditor;` implementing `runtimepack_core::asset_audit::AssetAuditor`.

- [ ] **Step 1: Write integration tests in `artemis_test.rs`**:
  - `test_detect_artemis_game_with_root_pfs_and_system_ini`:
    Directory with `root.pfs` and `system.ini` -> returns `EngineId::Artemis`, `DetectionConfidence::Exact`.
  - `test_artemis_audit_resolves_ast_script_assets`:
    Synthetic `.pfs` containing `init.ast` referencing `bg/classroom.png` and `bgm/theme.ogg` -> verifies assets and returns `AuditOutcome::Pass`.
- [ ] **Step 2: Run test to verify failure**:
  `cargo test --test artemis_test`
- [ ] **Step 3: Implement `ArtemisDetector` in `engines/artemis/src/lib.rs`**:
  - Score detection:
    - `root.pfs` or `*.pfs` -> +90 pts, `DET-ART-001`.
    - `system.ini` -> +80 pts, `DET-ART-002`.
    - `*.ast`, `*.asb`, `*.iet` -> +60 pts, `DET-ART-003`.
- [ ] **Step 4: Implement `ArtemisAssetAuditor` in `engines/artemis/src/asset_audit.rs`**:
  - Read all `.pfs` archives into merged `PfsArchiveIndex`.
  - Parse `.ast` script tokens for background, foreground, BGM, and voice references.
  - Verify asset existence in the merged PFS index.
- [ ] **Step 5: Run tests to verify pass**:
  `cargo test --test artemis_test`
- [ ] **Step 6: Commit**:
  ```powershell
  git add engines/artemis/src/lib.rs engines/artemis/src/asset_audit.rs core/src/detection/mod.rs core/src/asset_audit/mod.rs tests/integration/artemis_test.rs
  git commit -m "feat(artemis): implement static engine detector and AST script asset auditor"
  ```

---

### Task 4: Android Shell Activity, Generator & CLI Wiring

**Files:**
- Create: `runtimes/artemis/runtime-manifest.json`
- Create: `android/shell/app/src/main/java/io/runtimepack/host/ArtemisActivity.kt`
- Modify: `core/src/build_plan/generator.rs`
- Modify: `cli/src/main.rs`
- Test: `tests/integration/build_test.rs`

**Interfaces:**
- Produces:
  - Runtime manifest for `artemis-native` based on `art3m1s-core` C FFI API v1.
  - `ArtemisActivity.kt`: manages C FFI runtime lifecycle, sets save root to `filesDir/saves/<title-id>`, and mounts `filesDir/game`.
  - `AndroidProjectGenerator`: configures `ArtemisActivity` for `EngineId::Artemis` and copies native binaries.

- [ ] **Step 1: Write integration build test in `build_test.rs`**:
  ```rust
  #[test]
  fn test_build_artemis_project_emits_artemis_activity_and_save_isolation() {
      let temp = test_temp_dir("artemis-build");
      let manifest = create_test_manifest_for_artemis();
      AndroidProjectGenerator::emit_project(&temp, &manifest, None).unwrap();
      
      let manifest_xml = std::fs::read_to_string(temp.join("app/src/main/AndroidManifest.xml")).unwrap();
      assert!(manifest_xml.contains("io.runtimepack.host.ArtemisActivity"));
  }
  ```
- [ ] **Step 2: Run test to verify failure**:
  `cargo test --test build_test test_build_artemis_project_emits_artemis_activity_and_save_isolation`
- [ ] **Step 3: Implement runtime manifest, shell activity, and generator updates**:
  - In `runtimes/artemis/runtime-manifest.json`: register `artemis-native`.
  - In `ArtemisActivity.kt`: create shell activity embedding C FFI calls with direct boot.
  - In `generator.rs`: match `"artemis" | "Artemis Native" => "io.runtimepack.host.ArtemisActivity"`.
  - In `cli/src/main.rs`: wire `runtimepack-engine-artemis` into detector, analyzer, and auditor registries.
- [ ] **Step 4: Run full workspace test suite**:
  `cargo test --workspace`
- [ ] **Step 5: Commit**:
  ```powershell
  git add runtimes/artemis/ android/shell/app/src/main/java/io/runtimepack/host/ArtemisActivity.kt core/src/build_plan/generator.rs cli/src/main.rs tests/integration/build_test.rs
  git commit -m "feat(artemis): support Artemis Engine project generation and ArtemisActivity shell"
  ```
