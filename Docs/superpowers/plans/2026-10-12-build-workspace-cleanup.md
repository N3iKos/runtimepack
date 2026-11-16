# Build Workspace Ownership, Artifact Integrity Gate & Intermediate Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Automatically clean up large build-owned intermediates (staging, ASAR unpack, game.pack, android-project) into an exclusive per-run scratch directory upon passing a strict APK integrity and signature gate, while preserving failed builds and supporting `--keep-build-files`.

**Architecture:** Encapsulate per-run temporary directories into an owned `WorkspaceScratch` and `OwnershipJournal` under `<output>/.work_<run_id>/`; implement a five-stage `ArtifactVerifier` (streaming SHA-256, byte identity, bounded ZIP CRC, and SDK `apksigner` integration) before atomic no-clobber APK publication; execute safe non-traversing recursive unlinking only upon verified artifact and probe success.

**Tech Stack:** Rust (core, cli), `zip = "=8.6.0"` (MIT only, features ["deflate-flate2-zlib-rs"]), Android SDK build-tools `apksigner.jar` / binary, Windows CMD / Linux Bash test runners.

**Spec:** `Docs/superpowers/specs/2026-10-12-build-workspace-cleanup-design.md`

## Global Constraints

- **Status:** APPROVED (Chat approval dated 2026-10-12).
- **Worktree:** `.worktrees/fix-mvmz-logical-root` (Commit `10e2a92` / Branch `fix/mvmz-logical-root-20261009`).
- **Flavor Policy:** `--flavor beta` mandatory for autonomous test builds (`AGENTS.md §23`).
- **Source Immutability:** Input source game directories, RTP directories, backups, reference APKs, and external keystores remain 100% read-only (`AGENTS.md §6.4`).
- **No Broad Deletion:** Recursive deletion is strictly restricted to `<output>/.work_<run_id>/`; no glob deletions, no `rm -rf`, no deletion of historical runs, user files, or global caches (`.runtimepack/cache/audit`, `~/.gradle`, `~/.cargo`) (`AGENTS.md §4`).
- **No-Clobber Publication:** Publishing must use true atomic no-replace commits (hardlink creation on Linux/Windows or native no-replace API). Never plain Unix `fs::rename`.
- **Single Canonical APK:** Cease creating secondary duplicate APKs for titles with apostrophes; emit exactly one canonical sanitized APK to satisfy `RuntimePack-Wizard.bat:467` (`APK_COUNT == 1`).
- **Checksums Policy:** `checksums.txt` includes only surviving durable deliverables; no dangling intermediate hashes.
- **No Emulation of Signed APKs in Production:** Production verification must invoke real `apksigner` (preferring JDK `java.exe -jar apksigner.jar` with direct argument arrays); test doubles are isolated to unit test traits.
- **Diagnostic Code Taxonomies:** Stable error codes `BLD-111` through `BLD-113`, `SIG-103` through `SIG-104`.

## Review Focus

1. **Windows Junction / Reparse Point Prior Recursion:** Prior to any recursive directory traversal, inspect `symlink_metadata` and on Windows `MetadataExt::file_attributes()` checking `FILE_ATTRIBUTE_REPARSE_POINT`. Any unexpected link triggers conservative refusal with `BLD-111`. Owned by Task 1 test `test_scratch_safe_deletion_refuses_unexpected_junctions_and_reparse_points`.
2. **Publish No-Clobber Race & Overwrite Defense:** Publication creates a source-verified temporary file on the SAME output filesystem, then commits via hardlink/no-replace. If the target APK already exists, publication fails closed with `BLD-113` and preserves existing files. Owned by Task 3 test `test_cli_build_rejects_output_collision_without_overwrite`.
3. **Bounded Streaming ZIP Verification:** Verification must enforce streaming CRC to EOF with bounded resource budgets (max expanded bytes budget, max entry count, time budget) to prevent decompression bombs without loading the full APK into memory. Owned by Task 2 test `test_artifact_verifier_rejects_corrupted_zip_crc_and_bounds_resources`.
4. **Post-Copy Incomplete Write Detection:** If copying the APK from project output to destination is truncated or corrupted, byte identity must fail, preventing publication and blocking cleanup. Owned by Task 2 test `test_artifact_verifier_detects_post_copy_hash_mismatch`.
5. **Failed Build / Failed Probe Retention:** If Gradle fails, packaging fails, or `--probe` returns failure, the scratch workspace must be retained intact with its full path printed for developer debugging. Owned by Task 3 test `test_cli_build_retains_workspace_on_failure_or_probe_fail`.

---

### Task 1: Workspace Private Scratch, Ownership Journal & Safe Deletion Subsystem

**Files:**
- Create: `core/src/build_plan/scratch.rs`
- Modify: `core/src/build_plan/mod.rs`
- Modify: `core/src/diagnostics/mod.rs`
- Test: `core/tests/workspace_scratch_test.rs`

**Interfaces:**
- Produces:
  - `struct WorkspaceScratch { pub run_id: String, pub scratch_root: PathBuf, ... }`
  - `fn WorkspaceScratch::create(output_dir: &Path) -> Result<Self, DiagnosticError>`
  - `fn WorkspaceScratch::register_owned_dir(&mut self, rel_name: &str) -> PathBuf`
  - `fn WorkspaceScratch::register_owned_file(&mut self, rel_name: &str) -> PathBuf`
  - `fn WorkspaceScratch::safe_cleanup(&self) -> Result<CleanupSummary, DiagnosticError>`
  - `struct CleanupSummary { pub run_id: String, pub status: CleanupStatus, pub estimated_bytes_removed: u64, pub surviving_paths: Vec<PathBuf> }`
  - Diagnostic constants: `BLD_CLEANUP_PARTIAL = "BLD-111"`, `BLD_ARTIFACT_COLLISION = "BLD-113"`.

- [x] **Step 1: Write the failing tests in `core/tests/workspace_scratch_test.rs`**
  ```rust
  #[test]
  fn test_scratch_creation_generates_isolated_directory_under_output();
  #[test]
  fn test_scratch_safe_deletion_unlinks_only_owned_entries();
  #[test]
  fn test_scratch_safe_deletion_refuses_unexpected_junctions_and_reparse_points();
  #[test]
  fn test_scratch_safe_deletion_rejects_disjoint_violation();
  ```

- [x] **Step 2: Run test to verify it fails**
  Run: `cargo test --test workspace_scratch_test`
  Expected: FAIL (types and module not found).

- [x] **Step 3: Implement `DiagnosticCode` additions in `core/src/diagnostics/mod.rs`**
  Add `BLD_CLEANUP_PARTIAL: &'static str = "BLD-111"`, `BLD_ARTIFACT_INTEGRITY_FAILED: &'static str = "BLD-112"`, `BLD_ARTIFACT_COLLISION: &'static str = "BLD-113"`, `SIG_VERIFICATION_FAILED: &'static str = "SIG-103"`, `SIG_TOOLCHAIN_MISSING: &'static str = "SIG-104"`.

- [x] **Step 4: Implement `core/src/build_plan/scratch.rs`**
  Implement `WorkspaceScratch` with monotonic run ID (`YYYYMMDD_HHMMSS_<nanos>`), canonical output validation via `validate_disjoint_output_dir`, pre-recursion reparse point check (`FILE_ATTRIBUTE_REPARSE_POINT` on Windows, symlink check on Unix), conservative refusal policy on unexpected links, and safe unlinking emitting `CleanupSummary`.

- [x] **Step 5: Run tests and verify they pass**
  Run: `cargo test --test workspace_scratch_test`
  Expected: PASS (4 tests passing).

---

### Task 2: Mandatory Artifact Integrity Gate & apksigner Verification (Implemented — Fixround 1 Completed, Pending Review)

**Files:**
- Create: `core/src/build_plan/artifact_verifier.rs`
- Modify: `core/src/build_plan/mod.rs`
- Modify: `core/src/build_plan/signing.rs`
- Test: `core/tests/artifact_verifier_test.rs`

**Interfaces:**
- Consumes: `DiagnosticError`, `DiagnosticCode`.
- Produces:
  - `pub struct ArtifactVerification { pub sha256: String, pub size_bytes: u64, pub zip_entries: usize, pub signature_verified: bool }`
  - `trait ApkVerifier: Send + Sync { fn verify_signature(&self, apk_path: &Path) -> Result<SignatureDetails, DiagnosticError>; }`
  - `trait ZipValidator: Send + Sync { fn validate_zip(&self, apk_path: &Path, limits: &ZipLimits) -> Result<ZipAuditDetails, DiagnosticError>; }`
  - `struct SystemApkVerifier { sdk_root: Option<PathBuf>, java_home: Option<PathBuf> }`
  - `struct ArtifactVerifier<'a> { verifier: &'a dyn ApkVerifier, zip_validator: &'a dyn ZipValidator }`
  - `fn ArtifactVerifier::run_full_gate(&self, source_apk: &Path, dest_apk: &Path) -> Result<ArtifactVerification, DiagnosticError>`

- [x] **Step 1: Write the failing tests in `core/tests/artifact_verifier_test.rs`**
  ```rust
  #[test]
  fn test_artifact_verifier_computes_streaming_sha256_accurately();
  #[test]
  fn test_artifact_verifier_detects_post_copy_hash_mismatch();
  #[test]
  fn test_artifact_verifier_rejects_corrupted_zip_crc_and_bounds_resources();
  #[test]
  fn test_artifact_verifier_validates_apksigner_output_with_mock();
  #[test]
  fn test_artifact_verifier_fails_closed_when_apksigner_returns_signature_invalid();
  ```

- [x] **Step 2: Run test to verify it fails**
  Run: `cargo test --test artifact_verifier_test`
  Expected: FAIL (verifier types not found).

- [x] **Step 3: Implement `core/src/build_plan/artifact_verifier.rs`**
  Implement `ArtifactVerifier`:
  - 64 KiB chunk streaming SHA-256 without loading entire file into memory.
  - Byte-for-byte post-copy verification matching source to destination.
  - Bounded ZIP central directory and CRC-32 extraction via `ZipValidator` trait (with configurable max expanded bytes, max entry count, time budgets).
  - Android SDK `build-tools/34.0.0/lib/apksigner.jar` discovery invoking `java.exe -jar apksigner.jar` directly with argument arrays (or binary fallback).
  - Return `DiagnosticError` with stable codes (`BLD-112`, `SIG-103`, `SIG-104`).

- [x] **Step 4: Run tests and verify they pass**
  Run: `cargo test --test artifact_verifier_test`
  Expected: PASS (5 tests passing).

---

### Task 3: CLI Pipeline Migration, Atomic No-Clobber Publish, Single Canonical APK & Checksums

**Files:**
- Modify: `cli/src/main.rs:149-214` (`BuildArgs`)
- Modify: `cli/src/main.rs:1033-2185` (`cmd_build`)
- Modify: `core/src/ingest/staging.rs:89-115` (`stage_and_apply_profile` parameter)
- Create: `tests/integration/workspace_cleanup_test.rs`

**Interfaces:**
- Consumes: `WorkspaceScratch`, `ArtifactVerifier`, `SystemApkVerifier`, `OwnershipJournal`.
- Produces:
  - `--keep-build-files` flag in `BuildArgs`.
  - Staging, ASAR unpack, `game.pack`, and `android-project` routed into `scratch.scratch_root`.
  - Atomic no-clobber publish via temp file on SAME filesystem commit (hardlink / no-replace), returning `BLD-113` on collision.
  - Single canonical APK published; removal of new apostrophe duplication logic.
  - Checksums written strictly after reports/cleanup, hashing only durable deliverables.
  - Strict lifecycle sequencing: Build -> Verification Gate -> Probe -> Cleanup -> Emit `cleanup-summary.json` -> Final Success Banner.
  - Project emission modes (`--no-compile`) bypass cleanup and retain deliverables.

- [x] **Step 1: Write the failing integration tests in `tests/integration/workspace_cleanup_test.rs`**
  ```rust
  #[test]
  fn test_cli_build_default_cleans_intermediates_and_emits_summary();
  #[test]
  fn test_cli_build_keep_build_files_preserves_intermediates();
  #[test]
  fn test_cli_build_retains_workspace_on_failure_or_probe_fail();
  #[test]
  fn test_cli_build_no_compile_retains_project_deliverables();
  #[test]
  fn test_cli_build_rejects_output_collision_without_overwrite();
  #[test]
  fn test_cli_build_emits_single_canonical_apk_for_apostrophe_title();
  ```

- [x] **Step 2: Run test to verify it fails**
  Run: `cargo test --test workspace_cleanup_test`
  Expected: FAIL (`--keep-build-files` not recognized, scratch paths unmigrated).

- [x] **Step 3: Update `staging.rs` and `cli/src/main.rs`**
  1. Add `keep_build_files: bool` to `BuildArgs`.
  2. Instantiate `WorkspaceScratch::create(output)` at start of `cmd_build`.
  3. Route `staging_path`, `staging_content`, `pack_path`, and `android_project_dir` into `scratch.scratch_root`.
  4. Perform atomic no-clobber publish: create temp on same filesystem, commit via hardlink/no-replace. If destination exists, fail closed with `BLD-113`.
  5. Remove apostrophe duplicate copy logic (`main.rs:2037-2041`).
  6. Run `ArtifactVerifier::run_full_gate` on destination APK.
  7. Run `--probe` against destination APK if requested.
  8. If verified, probe passed (or skipped due to no device), not `--keep-build-files`, and not `--no-compile`: invoke `scratch.safe_cleanup()` and write `cleanup-summary.json`.
  9. Finalize `checksums.txt` hashing only surviving durable deliverables.
  10. Print final success message only *after* probe, verification, and cleanup are finalized.

- [x] **Step 4: Run tests and verify they pass**
  Run: `cargo test --test workspace_cleanup_test`
  Run: `cargo test --test build_test`
  Expected: PASS (all workspace cleanup tests and existing regression build tests pass).

- [x] **Step 5: Verify Wizard batch script compatibility**
  Verify `RuntimePack-Wizard.bat` against the updated cleanup output: confirm that titles with apostrophes produce exactly 1 APK, satisfying `%APK_COUNT% == 1`.

---

## Acceptance & Review Checklist

- [x] `WorkspaceScratch` encapsulates all ephemeral intermediates into `<output>/.work_<run_id>/`.
- [x] No pre-existing files, ancestor paths, or global caches (`.runtimepack/cache/audit`, `~/.gradle`) can be touched.
- [x] Symlinks and Windows junctions inspected via reparse point attributes; unexpected links trigger conservative refusal (`BLD-111`).
- [x] Artifact integrity gate requires streaming SHA-256, byte identity, bounded ZIP CRC, and `apksigner`.
- [x] Missing `apksigner` or failed verification halts publication, halts cleanup, and emits `SIG-104`/`SIG-103`.
- [x] Any build error, crash, or probe failure preserves the workspace for debugging.
- [x] `--keep-build-files` completely skips unlinking.
- [x] `--no-compile` preserves project and pack files as user deliverables.
- [x] Atomic no-clobber publication prevents partial writes and rejects collisions (`BLD-113`).
- [x] Apostrophe duplicate copy removed; single canonical APK emitted aligning with Wizard contract.
- [x] `checksums.txt` hashes only surviving durable deliverables without cyclic self-hashing.
- [x] `cleanup-summary.json` records run details and estimated logical bytes removed.
