# Implementation Plan: Kirikiri Engine Integration — Phase 2 (Compatibility Scanner & Plugin Validator)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement static compatibility analysis for Kirikiri games by scanning `.tjs` / `.ks` scripts for `Plugins.link(...)` calls, validating against native C++ shims in `libkrkr2.so`, detecting custom XP3 encryption filters, and establishing the Kirikiri compatibility database.

**Architecture:**
1. In `engines/kirikiri/src/scanner.rs`, implement `KirikiriAnalyzer` implementing `runtimepack_core::compatibility::Analyzer`.
2. Parse TJS2 script files (extracted in-memory from `.xp3` or loose files) using regex and token scanning to detect calls like `Plugins.link("csvParser.dll")` and `Plugins.link("windowEx.tpm")`.
3. In `compatibility-db/runtimes/kirikiri-native.json`, define the authoritative capability matrix for `libkrkr2.so` (documenting supported vs unsupported Windows plugins).
4. Detect custom encryption filters (e.g. `XP3ArchiveExtractionFilter` or corrupt headers) and emit structured diagnostics (`CMP-KRK-001`).

**Tech Stack:** Rust (core, engines/kirikiri), regex, JSON schemas.

**Spec:** `Docs/superpowers/specs/2026-09-27-kirikiri-complete-architecture-spec.md`

## Global Constraints

- Source directories in `Raw-Game/` remain strictly 100% immutable.
- Zero disk extraction: all script inspection and archive analysis operates in-memory.
- Unknown native DLL dependencies must never silently succeed without a verified shim.
- All subagents must run tests and verify zero regressions across the workspace before each commit.

---

### Task 1: Kirikiri Native Compatibility Database Definition

**Files:**
- Create: `compatibility-db/runtimes/kirikiri-native.json`
- Modify: `schemas/runtime-manifest.schema.json`
- Test: `tests/integration/compatibility_test.rs`

**Interfaces:**
- Produces:
  - Declarative runtime manifest for `kirikiri-native` runtime:
    - Supported plugins: `csvParser`, `dirlist`, `fftgraph`, `addFont`, `getSample`, `getabout`, `saveStruct`, `varfile`, `wutcwf`, `win32dialog`, `xp3filter`, `layerExMovie`, `perspective`, `scriptsEx`, `windowEx`, `fstat`, `TextRender`, `layerExDraw`, `motionplayer`, `emoteplayer`.
    - Blocked/Unsupported plugins: `DrawDeviceD3D`, `wuvorbis`, `extNag`.

- [ ] **Step 1: Write integration test in `compatibility_test.rs`**:
  ```rust
  #[test]
  fn test_kirikiri_runtime_manifest_in_database() {
      let db = load_compatibility_db();
      let runtime = db.get_runtime("kirikiri-native").expect("kirikiri-native runtime must be registered");
      assert!(runtime.supported_capabilities.contains(&"kirikiri.plugin.fstat".to_string()));
      assert!(runtime.supported_capabilities.contains(&"kirikiri.plugin.windowEx".to_string()));
  }
  ```
- [ ] **Step 2: Run test to verify failure**:
  `cargo test --test integration_compatibility test_kirikiri_runtime_manifest_in_database`
- [ ] **Step 3: Create `compatibility-db/runtimes/kirikiri-native.json`**:
  Populate with id `"kirikiri-native"`, engine `"kirikiri"`, version `"1.0.0"`, capabilities, and supported plugin mappings.
- [ ] **Step 4: Run test to verify pass**:
  `cargo test --test integration_compatibility test_kirikiri_runtime_manifest_in_database`
- [ ] **Step 5: Commit**:
  ```powershell
  git add compatibility-db/runtimes/kirikiri-native.json tests/integration/compatibility_test.rs
  git commit -m "feat(compatibility): add declarative runtime manifest for kirikiri-native"
  ```

---

### Task 2: Script Plugin Scanner & Windows DLL Detection (`engines/kirikiri/src/scanner.rs`)

**Files:**
- Create: `engines/kirikiri/src/scanner.rs`
- Modify: `engines/kirikiri/src/lib.rs`
- Test: `tests/integration/kirikiri_compatibility_test.rs`

**Interfaces:**
- Produces:
  - `pub struct KirikiriAnalyzer;` implementing `runtimepack_core::compatibility::Analyzer`.
  - Scans `startup.tjs`, `*.tjs`, `*.ks` for `Plugins.link("...")`.
  - Evaluates plugin calls:
    - If plugin in supported list -> emits `CompatibleWithShims` with info/shim notice.
    - If plugin is `DrawDeviceD3D` or unmapped Windows DLL -> emits `UnsupportedDependency` with `CMP-102`.

- [ ] **Step 1: Write integration tests in `kirikiri_compatibility_test.rs`**:
  - `test_kirikiri_supported_plugins_resolve_compatible_with_shims`: Script calling `Plugins.link("csvParser.dll")` and `Plugins.link("fstat.tpm")` evaluates to `CompatibleWithShims`.
  - `test_kirikiri_unsupported_plugin_emits_fatal_dependency`: Script calling `Plugins.link("DrawDeviceD3D.dll")` evaluates to `UnsupportedDependency`.
- [ ] **Step 2: Run tests to verify failure**:
  `cargo test --test kirikiri_compatibility_test`
- [ ] **Step 3: Implement `KirikiriAnalyzer` in `scanner.rs`**:
  - Implement plugin extraction regex: `r#"Plugins\.link\s*\(\s*["']([^"']+)["']\s*\)"#`.
  - Normalize plugin name (strip `.dll` and `.tpm`, case-insensitive).
  - Categorize based on supported C++ shim list from Domain D.
  - Return `CompatibilityReport`.
- [ ] **Step 4: Run tests to verify pass**:
  `cargo test --test kirikiri_compatibility_test`
- [ ] **Step 5: Commit**:
  ```powershell
  git add engines/kirikiri/src/scanner.rs engines/kirikiri/src/lib.rs tests/integration/kirikiri_compatibility_test.rs
  git commit -m "feat(kirikiri): implement KirikiriAnalyzer with TJS Plugins.link scanner"
  ```

---

### Task 3: Custom XP3 Encryption & Extraction Filter Probe

**Files:**
- Modify: `engines/kirikiri/src/archive.rs`
- Modify: `engines/kirikiri/src/scanner.rs`
- Test: `tests/integration/kirikiri_compatibility_test.rs`

**Interfaces:**
- Produces:
  - `Xp3ArchiveIndex::probe_encryption(&[u8]) -> EncryptionProbeResult`:
    - `StandardReadable`
    - `FilterSuspected` (TOC readable but files flagged protected or known proprietary header)
    - `CorruptedOrEncryptedHeader` (fails 11-byte magic on `.xp3` file)
  - In `KirikiriAnalyzer`: Emits `CMP-KRK-001` (`"Encrypted or proprietary XP3 archive detected; requires custom extraction filter"`) and marks status `Unknown` or `Blocked`.

- [ ] **Step 1: Write test in `kirikiri_compatibility_test.rs`**:
  - `test_kirikiri_corrupted_or_encrypted_xp3_emits_diagnostic`: Synthetic archive with corrupted header magic emits `CMP-KRK-001`.
- [ ] **Step 2: Run test to verify failure**:
  `cargo test --test kirikiri_compatibility_test test_kirikiri_corrupted_or_encrypted_xp3_emits_diagnostic`
- [ ] **Step 3: Implement `probe_encryption` and diagnostic emission**:
  - In `archive.rs`, add `probe_encryption`.
  - In `scanner.rs`, check all `.xp3` files in `source.files`.
- [ ] **Step 4: Run tests to verify pass**:
  `cargo test --test kirikiri_compatibility_test`
  `cargo test --workspace`
- [ ] **Step 5: Commit**:
  ```powershell
  git add engines/kirikiri/src/archive.rs engines/kirikiri/src/scanner.rs tests/integration/kirikiri_compatibility_test.rs
  git commit -m "feat(kirikiri): add custom XP3 encryption and extraction filter probe"
  ```
