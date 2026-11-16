# Technical Design Specification: Build Workspace Ownership, Artifact Integrity Gate & Intermediate Cleanup

- **Document ID:** SPEC-2026-10-12-BUILD-WORKSPACE-CLEANUP
- **Status:** APPROVED (Chat approval dated 2026-10-12)
- **Authoritative Worktree:** `.worktrees/fix-mvmz-logical-root` (Commit `10e2a92` / Branch `fix/mvmz-logical-root-20261009`)
- **Canonical Requirement IDs:** `PRD-001`, `PRD-002`, `PRD-010`, `TECH-CORE-001`, `TECH-CORE-002`, `TECH-CORE-004`, `TECH-AND-001`
- **Canonical References:** `S01`, `S02`, `S13`, `S14`, `S17`, `S22`, `S29`
- **Specification IDs:** `CLEANUP-001` through `CLEANUP-024`

---

## 1. Executive Summary & Problem Analysis

### 1.1 Context & Strategic Objective
RuntimePack packages standalone Android APKs for 7 supported engines. On large games (1.0–2.0 GB source assets), the current build pipeline creates multiple massive uncompressed duplicates:
1. Staging directory (`output/staging`): 1.5–2.0 GB.
2. Extracted ASAR staging (`output/staging_content`): 1.0–1.5 GB for Electron-wrapped games.
3. Standalone intermediate container (`output/game.pack`): 1.4–1.5 GB.
4. Android build project (`output/android-project/`): 2.5–4.0 GB (including copied `app/src/main/assets/game.pack`, Gradle build tree, dex files, and unaligned APKs).
5. Final APK (`output/<Game>.apk`) and apostrophe duplicates: 1.5–3.0 GB.

A single 1.5 GB build currently consumes 7.0–11.0 GB of disk space. Because intermediates are never cleaned up, multiple builds rapidly exhaust host storage. The user has approved: **default auto-cleanup of large build-owned intermediates after verified final APK, with a `--keep-build-files` opt-out flag, while retaining workspaces on failed builds or errors, and strictly preserving global/user caches**.

### 1.2 Inspection of Current Pipeline Deficiencies (`cli/src/main.rs:1033-2185`)
An inspection of the authoritative worktree reveals six architectural gaps:
1. **Unowned Workspace & Output Mixing:**
   `cmd_build` creates `staging`, `staging_content`, `game.pack`, `android-project`, final reports, and the final APK all directly under `output/`. There is no boundary separating temporary intermediates from durable deliverables.
2. **Deficient Verification Gate:**
   Gradle `exit 0` and `apk_source.exists()` are the sole exit criteria (`main.rs:2014-2024`). The subsequent SHA-256 calculation (`main.rs:2042-2046`) reads the entire multi-gigabyte APK into memory via `fs::read` and silently ignores errors (`if let Ok(bytes)`). If `fs::read` fails (e.g., OOM), `apk_hash` is left as `None` without stopping the build. There is zero post-copy byte verification, zero ZIP structural or CRC check, and zero APK signature validation.
3. **Premature Success Declaration:**
   `[SUCCESS] Standalone Android packaging complete!` is printed at line 2138 *before* the optional `--probe` runtime verification executes at line 2140.
4. **Duplicate APK Conflict with Wizard Contract:**
   `main.rs:2037-2041` duplicates APKs containing apostrophes (e.g., `The Witch's House MV` produces both `TheWitch'sHouseMV.apk` and `TheWitchsHouseMV.apk`). Meanwhile, `RuntimePack-Wizard.bat:457-470` enforces that *exactly one APK* must exist in `%RUN_OUTPUT_DIR%`, causing the Wizard to report build failure whenever apostrophes are present.
5. **No Intermediate Retention Flag:**
   No `--keep-build-files` flag exists. Furthermore, in project-emission modes (`--no-compile` or `RUNTIMEPACK_SKIP_GRADLE`), intermediate deliverables are needed by the user and must never be deleted.
6. **Global Cache Boundaries:**
   Shared caches (`.runtimepack/cache/audit`, `~/.gradle`, `~/.cargo`, Android SDK caches) must never be cleaned by a per-run cleanup routine.

---

## 2. Architecture & Ownership Model

```text
Build Run Initialized (exclusive Run ID: <timestamp>_<nanos>)
  │
  ├── Output Directory: <output>/ (Durable Deliverables Root)
  │     ├── <Game>.apk (Atomic verified publish, no-clobber)
  │     ├── build-manifest.json
  │     ├── compatibility-report.json
  │     ├── asset-audit-report.json / html / graph
  │     ├── checksums.txt (Durable deliverables only; no dangling game.pack)
  │     ├── third-party-notices/NOTICES.txt
  │     ├── probe-report.json (if --probe requested)
  │     ├── cleanup-summary.json (Durable deletion report)
  │     │
  │     └── [Owned Private Scratch]: <output>/.work_<run_id>/
  │           ├── staging/            (compatibility profile transform)
  │           ├── staging_content/    (ASAR archive extraction)
  │           ├── game.pack           (intermediate container before APK asset embed)
  │           └── android-project/    (Gradle project tree, dex, caches)
  │
  └── Verification & Cleanup Lifecycle:
        1. Compile APK -> .work_<run_id>/android-project/app/build/.../app-debug.apk
        2. Atomic Publish (No-Clobber) -> temp on SAME filesystem -> hardlink / RENAME_NOREPLACE
        3. Mandatory Artifact Gate:
             - Non-empty streaming size + SHA-256 (64 KiB chunks)
             - Post-copy byte-for-byte identity (source == destination)
             - Bounded ZIP central directory & CRC-32 integrity validation (budget-capped)
             - apksigner verify via configured JDK java.exe -jar apksigner.jar (or direct binary)
        4. Optional Probe Execution (--probe against final APK)
        5. Decision Gate:
             - Succeeded + Verified + Probe PASS + NOT --keep-build-files:
                 => Safe Non-Traversing Recursive Deletion of .work_<run_id>/
             - Any Failure / Probe FAIL / --keep-build-files / --no-compile:
                 => RETAIN entire scratch untouched
        6. Durable Report: emit cleanup-summary.json, finalize checksums.txt, print success banner
```

### 2.1 Specification Rules: Private Scratch & Ownership Journal
- `CLEANUP-001`: **Exclusive Per-Run Scratch Directory.**
  All temporary build intermediates MUST be created inside a private subdirectory under the resolved output root: `<output>/.work_<run_id>/`. The `run_id` is generated deterministically at build start using UTC timestamp and monotonic process nanos (`YYYYMMDD_HHMMSS_<nanos>`).
- `CLEANUP-002`: **Owned Intermediate Relocation.**
  The creation of:
  - Compatibility-profile staging (`staging/`)
  - ASAR extracted staging (`staging_content/`)
  - Intermediate container (`game.pack`)
  - Generated Android project (`android-project/`)
  MUST be relocated into `<output>/.work_<run_id>/`. Path plumbing across `stage_and_apply_profile`, ASAR extractor, and `AndroidProjectGenerator` MUST accept the private scratch path.
- `CLEANUP-003`: **Manifest Logical Provenance Isolation.**
  `build-manifest.json` and reports MUST record logical relative paths and hashes (e.g., `content.content_hash`, `runtime.id`, `engine.content_root`). They MUST NOT leak ephemeral `.work_<run_id>/` absolute filesystem paths. The logical `content.content_hash` serves as provenance record independent of physical intermediate file persistence on disk.
- `CLEANUP-004`: **In-Memory Ownership Journal & Subtree Delegation.**
  The builder MUST maintain an in-memory `OwnershipJournal` recording:
  - `run_id`: string identifier.
  - `scratch_root`: absolute canonical path to `.work_<run_id>/`.
  - `owned_directories`: registered top-level subdirectories (`staging`, `staging_content`, `android-project`).
  - `owned_files`: registered intermediate files (`game.pack`).
  - `created_at`: RFC 3339 timestamp.
  *Delegated Subtree Distinction:* External tools (Gradle) naturally generate unknown descendant files inside `android-project/`. The journal owns the root private scratch subtree created exclusively for this run, while refusing unexpected external junction/reparse links.
- `CLEANUP-005`: **Strict Non-Destruction of Foreign / Historical Files.**
  Cleanup MUST ONLY operate on the validated `scratch_root` belonging to the current `run_id`. Historical directories (e.g., pre-existing `output/staging`, other `.work_*` dirs, or user output files) MUST NOT be touched. No glob-based (`*.pack`, `rm -rf`) heuristics are permitted.
- `CLEANUP-006`: **Exclusion of Shared and Global Caches.**
  Audit cache (`.runtimepack/cache/audit`), Gradle user cache (`~/.gradle`), Cargo cache (`~/.cargo`), and Android SDK directories are shared across runs and MUST NOT be registered in or deleted by the ownership journal.

---

## 3. Mandatory Artifact Integrity Gate & Publishing Policy

Before any intermediate deletion or final build success can occur, the produced APK MUST pass five mandatory integrity stages encapsulated in a typed `ArtifactVerification` structure:

### 3.1 Verification Stages (`CLEANUP-007` to `CLEANUP-011`)
1. **Stage 1 — Non-Empty Size & Streaming SHA-256 (`CLEANUP-007`):**
   The produced APK at `apk_source` and `dest_apk` MUST be non-empty (> 0 bytes). The SHA-256 hash MUST be calculated using a streaming buffer (64 KiB chunks) without loading the archive into memory.
2. **Stage 2 — Post-Copy Byte-for-Byte Identity (`CLEANUP-008`):**
   The destination APK hash MUST match the source APK hash exactly: `hash(dest_apk) == hash(apk_source)`. If a mismatch or read error occurs, the build fails with `BLD-112`.
3. **Stage 3 — Bounded ZIP Structural & CRC-32 Validation (`CLEANUP-009`):**
   The destination APK MUST be parsed as a valid ZIP archive without full-file memory loading:
   - Central directory and local file headers MUST be structurally valid.
   - Streaming CRC verification to EOF is mandatory for all entries.
   - Resource bounded execution: enforcement of configurable maximum expanded bytes budget (e.g. 8 GB to accommodate valid 2 GB games while bounding malicious bomb expansion), maximum entry count (e.g. 50,000 entries), and time budget. If any limit is exceeded or CRC fails, verification fails closed and retains workspace.
   - *Dependency Status:* Dependency `zip = "=8.6.0"` (MIT only, features ["deflate-flate2-zlib-rs"]) selected; final independent evidence audit in Task 2 verifies sources before integration. Hand-rolled cryptographic signature routines are strictly forbidden.
4. **Stage 4 — Cryptographic Signature Verification (`CLEANUP-010`):**
   The packager MUST verify APK signatures using the official Android SDK build-tools `apksigner`.
   - *Windows Escaping & Direct Invocation:* To avoid `cmd.exe` batch script argument escaping vulnerabilities (`!`, `%`, `&`, `"`), the preferred invocation on Windows executes the configured JDK `java.exe -jar <sdk_root>/build-tools/34.0.0/lib/apksigner.jar verify --verbose --print-certs <dest_apk>` with direct argument arrays. If `apksigner.jar` is absent, invoke `<sdk_root>/build-tools/34.0.0/apksigner` directly (or `.bat` with documented argument quoting). No passwords or private keys are passed.
   - Verification MUST confirm: exit code is `0`, output contains `Verifies: true` or scheme v2/v3 verified, and a certificate is present.
   - If `apksigner` is missing or fails: emit diagnostic error `SIG-104` (missing tool) or `SIG-103` (verification failed). Cleanup is blocked and success is NOT reported.
   - `--keep-build-files` does NOT bypass this verification.
   - Test doubles are isolated behind `trait ApkVerifier` (`SystemApkVerifier` in production, `MockApkVerifier` in tests).
5. **Stage 5 — Pre-Cleanup Probe Gate (`CLEANUP-011`):**
   If `--probe` is enabled:
   - The probe runs against `dest_apk` *before* cleanup.
   - If the probe executes and fails (`overall != "PASS"`), intermediate files are RETAINED for debugging and build exits `1`.
   - If no device is available (`DeviceResolution::NoneAvailable`), `cmd_build` reports `[probe] no device available — skipping runtime verification`. Cleanup proceeds only if APK verification passed and probe was explicitly skipped due to missing device (not failed).

### 3.2 Safe Atomic Publishing & Collision Policy (`CLEANUP-012` to `CLEANUP-014`)
- `CLEANUP-012`: **Atomic Publication with No-Clobber Semantics.**
  Pre-existence checks (`exists()`) alone are vulnerable to TOCTOU races. Publication MUST follow a true atomic no-replace commit:
  - Copy `apk_source` to a temporary verified file on the SAME output filesystem: `<output>/<sanitized_apk_name>.tmp_<run_id>`.
  - Perform atomic creation of the destination without overwriting:
    - On Linux: create destination via hardlink (`std::fs::hard_link`), which atomically fails if destination exists (`EEXIST`), then unlink the owned temp file. (Or use `renameat2` with `RENAME_NOREPLACE`).
    - On Windows: invoke atomic no-replace filesystem call (or hardlink creation via `CreateHardLinkW`, followed by unlinking temp).
    - NEVER use plain `fs::rename` on Unix, because Unix `rename(2)` silently clobbers existing destination files.
    - If the filesystem does not support hardlinks or atomic no-replace primitives, fail closed and preserve the temp file. Never claim atomicity while streaming directly to destination.
- `CLEANUP-013`: **Collision Protection (`BLD-113`).**
  If the destination APK already exists, publication fails closed with `BLD-113` ("Target APK already exists. Preserving existing deliverable without overwrite.").
- `CLEANUP-014`: **Single Canonical Final APK (Removal of Apostrophe Duplication).**
  The legacy behavior of creating an un-apostrophed copy (`alt_apk = sanitized_apk_name.replace('\'', "")`) in `main.rs:2037-2041` is REMOVED for new builds.
  - Exactly one canonical sanitized APK filename is produced.
  - Old existing aliases from prior runs are NOT deleted.
  - Manifest and `checksums.txt` record exactly this single canonical APK path and hash.
  - This directly aligns CLI output with `RuntimePack-Wizard.bat:467` (`APK_COUNT == 1`), eliminating false Wizard failures on titles with apostrophes.

---

## 4. Safe Deletion & Filesystem Protection Policy

### 4.1 Security Invariants (`CLEANUP-015` to `CLEANUP-018`)
- `CLEANUP-015`: **Bidirectional Disjointness Verification.**
  Before performing any recursive deletion, the scratch root MUST be validated using `validate_disjoint_output_dir`:
  - `scratch_root` MUST NOT equal or be an ancestor of the game source root, RTP root, workspace resource root, or user home directory.
  - `scratch_root` MUST be strictly contained within the canonical `output_dir`.
- `CLEANUP-016`: **Anti-Symlink & Anti-Junction Defense (Reparse Point Checking).**
  Recursive removal MUST NOT follow symlinks (Unix) or Directory Junctions / Reparse Points (Windows).
  - Traversal MUST inspect `std::fs::symlink_metadata` for every entry.
  - On Windows, inspect `MetadataExt::file_attributes()` checking `FILE_ATTRIBUTE_REPARSE_POINT (0x00000400)` *prior* to any recursion.
  - **Conservative Refusal Policy:** If any entry inside `.work_<run_id>/` is an unexpected symlink, junction, or reparse point not registered by RuntimePack, the cleanup engine REFUSES to delete or traverse it, leaves the tree intact, and emits diagnostic warning `BLD-111`.
  - Owned links created deliberately by toolchains are unlinked directly without target recursion.
- `CLEANUP-017`: **Permanent Freeing of Regenerable Intermediates.**
  Deletion permanently unlinks files. It does not move files to a recycle bin or quarantine, because intermediates are 100% regenerable from immutable inputs and pinned toolchains.
- `CLEANUP-018`: **Independent Cleanup Warning vs Build Success.**
  If the final APK is verified and probe passes, but cleanup fails (e.g., Windows file lock by an external scanner):
  - The CLI process exits with code `0` (build success).
  - A prominent diagnostic warning `[BLD-111]` is emitted: "Build succeeded but some intermediate files could not be removed."
  - Surviving locked files/directories are listed in the report.
  - The cleanup summary records actual bytes successfully unlinked without exaggerating reclaimed disk space.

---

## 5. Reporting & CLI Interface

### 5.1 CLI Arguments (`CLEANUP-019`)
In `cli/src/main.rs:BuildArgs`:
```rust
/// Keep intermediate build files (scratch workspace, staged assets, and android-project)
#[arg(long)]
pub keep_build_files: bool,
```

### 5.2 Project-Emission Modes (`CLEANUP-020`)
When `--no-compile` or `RUNTIMEPACK_SKIP_GRADLE` is active:
- The build is in *project-emission mode*.
- `android-project` and `game.pack` are deliverables, not temporary intermediates.
- They are placed directly in `output/` (or promoted out of scratch), their paths are printed, and cleanup is skipped entirely.
- The build does NOT report APK success.

### 5.3 Final Checksums & Durable Cleanup Summary (`CLEANUP-021` to `CLEANUP-022`)
- `CLEANUP-021`: **Durable Deliverables Checksum Policy.**
  `checksums.txt` is written AFTER cleanup and reporting are finalized.
  - It hashes ONLY surviving durable deliverables in `<output>/` (e.g., `<Game>.apk`, `build-manifest.json`, `compatibility-report.json`, `cleanup-summary.json`).
  - No dangling `game.pack` entry is written when `game.pack` intermediate is cleaned up.
  - In `--keep-build-files` mode, retained files are noted with actual paths.
  - Text reports do not hash themselves cyclically (`checksums.txt` does not include its own hash).
- `CLEANUP-022`: **Cleanup Summary Schema (`cleanup-summary.json`).**
  Written to `<output>/cleanup-summary.json`:
  ```json
  {
    "run_id": "20261012_143000_123456789",
    "status": "cleaned",
    "keep_build_files": false,
    "estimated_bytes_removed": 5432109876,
    "removed_paths": [
      ".work_20261012_143000_123456789/staging",
      ".work_20261012_143000_123456789/game.pack",
      ".work_20261012_143000_123456789/android-project"
    ],
    "surviving_paths": []
  }
  ```

---

## 6. Diagnostic Code Allocations

| Code | Domain | Severity | Name | Description |
|---|---|---|---|---|
| `BLD-111` | Build | Warning | `BLD_CLEANUP_PARTIAL` | Intermediate scratch directory could not be fully deleted (e.g., file lock, unexpected reparse point). Surviving files retained. |
| `BLD-112` | Build | Error | `BLD_ARTIFACT_INTEGRITY_FAILED` | Generated APK failed post-copy byte identity, streaming hash, or ZIP structural CRC check. |
| `BLD-113` | Build | Error | `BLD_ARTIFACT_COLLISION` | Target output APK already exists. Preserving existing deliverable without silent overwrite. |
| `SIG-103` | Signing | Error | `SIG_VERIFICATION_FAILED` | APK signature verification failed via `apksigner`. |
| `SIG-104` | Signing | Error | `SIG_TOOLCHAIN_MISSING` | Android SDK `build-tools/apksigner` executable or JAR not found or could not be executed. |

---

## 7. Written User Review List (3 Explicit Decisions)

The following 3 specific decisions are presented for user review before implementation:

1. **Single Canonical APK vs Legacy Apostrophe Duplication:**
   - *Spec Decision:* Emit exactly one canonical sanitized APK (e.g. `TheWitchsHouseMV.apk` or `TheWitch'sHouseMV.apk`) and remove the secondary duplicate copy logic in `main.rs:2037-2041`. Existing prior files on disk are untouched.
   - *Benefit:* Restores full compatibility with `RuntimePack-Wizard.bat:467` (`APK_COUNT == 1`).
2. **Atomic Publication Collision Policy (`BLD-113`):**
   - *Spec Decision:* When `<output>/<Game>.apk` already exists, fail closed with `BLD-113` and preserve the existing file.
   - *Benefit:* Eliminates silent clobbering of previously verified builds.
3. **Mandatory `apksigner` Verification Gate:**
   - *Spec Decision:* APK signature verification via SDK `apksigner` (using direct `java -jar apksigner.jar` or binary) is a non-bypassable prerequisite for declaring verified build success and for deleting intermediates. If `apksigner` is missing or fails, intermediates are retained.
   - *Benefit:* Guarantees that unverified or corrupted packages never trigger irreversible intermediate cleanup.

---

## 8. Acceptance Criteria Checklist

- [ ] All temporary intermediates (`staging`, `staging_content`, `game.pack`, `android-project`) created exclusively in `.work_<run_id>/`.
- [ ] No pre-existing historical files or global caches deleted.
- [ ] Build failure at any step retains `.work_<run_id>/` completely.
- [ ] Passing `--keep-build-files` retains `.work_<run_id>/` completely.
- [ ] `--no-compile` retains project and pack deliverables without reporting APK success.
- [ ] Verification gate enforces streaming hash, post-copy identity, ZIP CRC, and `apksigner` before cleanup.
- [ ] Missing `apksigner` or failed signature blocks cleanup and emits `SIG-103`/`SIG-104`.
- [ ] `--probe` runs before cleanup; probe failure blocks cleanup and exits 1.
- [ ] Symlinks and Windows junction points are detected via reparse point attributes; unexpected links trigger conservative refusal.
- [ ] Final APK publication uses atomic hardlink/no-replace commit and fails closed on collision (`BLD-113`).
- [ ] Single canonical APK published; apostrophe duplication removed.
- [ ] `checksums.txt` contains only surviving durable files; no dangling intermediate hashes.
- [ ] Cleanup failure emits warning `BLD-111` while preserving process exit 0 if APK verified.
- [ ] `cleanup-summary.json` durably records cleanup actions and estimated logical bytes removed.
