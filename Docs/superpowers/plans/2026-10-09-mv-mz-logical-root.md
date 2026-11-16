# Implementation Plan: RPG Maker MV/MZ Logical Content Root Discovery & Pipeline Scoping

**Goal:** Implement static discovery, scoped evidence evaluation, generic domain carrying, analyzer/auditor alignment, and direct GPACK packaging for arbitrary nested RPG Maker MV/MZ payloads (e.g. `bin/www/`) when outer desktop game distributions are supplied, maintaining source immutability.  
**Architecture:** Marker-owned candidate derivation from indexed path keys, strict per-root scorer isolation, generic `content_root: Option<String>` domain carrier, fail-closed ambiguity resolution (`DET-110`), cross-platform lexical containment, in-memory index scoping (`scoped_to`), and container packaging without disk re-ingestion.  
**Tech Stack:** Rust 2021 edition workspace (`cli`, `core`, `pack`, `engines/rpgmaker_web`), serde, sha2, WSL2 (`Ubuntu-24.04`).  
**Spec Reference:** `docs/superpowers/specs/2026-10-09-mv-mz-logical-root-design.md`  
**Required Sub-Skill:** `subagent-driven-development`  
**Toolchain Mandate:** All development, testing, and script execution strictly inside WSL2 (`Ubuntu-24.04`). Source files format LF. Non-destructive operations only (mandatory `trash <path>` for file removal).

---

## 1. Global Constraints & Review Focus

### 1.1 Global Constraints
1. **WSL2 Runtime Only:** All shell development and test execution run inside WSL2 via `wsl -d Ubuntu-24.04 -- bash -lc 'cd "$WORKSPACE" && <command>'`. Windows host execution is prohibited.
2. **Strict Immutability:** Game source trees (synthetic fixtures and real game directories) remain 100% read-only. Full file inventory and byte digests verified before and after operations.
3. **Non-Destructive Operations:** Permanent file deletion commands (`rm`, `del`, `git reset --hard`, `git clean -f`) are strictly blocked. File removals must use `trash <path>`.
4. **Anti-Hardcoding & Anti-Overfitting:** Zero proprietary game titles, publisher names, folder names, or machine-specific absolute paths in production code or unit test fixtures.
5. **No Artificial Engine Locks:** Core carriers remain engine-agnostic; general flags (`--overlay-mode`, `--vpad`) remain universal across all engines.

### 1.2 Five Highest-Risk Review Areas (Tied to Owning Tests)
1. **Ambiguity Contract & Resolution Policy:**
   - $\ge 2$ auto-build eligible roots fail closed with `DET-110` (`DET_AMBIGUOUS`).
   - Forced engine flag (`--engine`) on competing same-engine roots does not guess.
   - Multiple Medium roots fail closed with `DET-110` even with `--accept-detection`.
   - *Owning Tests:* `runtimepack::integration_detection` (`TEST-ROOT-NEG-002`, `TEST-ROOT-NEG-003`, `TEST-ROOT-NEG-004`, `TEST-ROOT-NEG-006`).
2. **Lexical Security Containment:**
   - Pre-normalization lexical rejection of Windows drive prefixes (`^[a-zA-Z]:`), UNC (`^//`, `^\\\\`), URLs, NUL bytes (`\0`), and `..` traversal across Linux & Windows syntax.
   - Symlink and junction containment enforced at both ingest and packaging.
   - *Owning Tests:* `runtimepack-core::scoped_index_test` (`TEST-ROOT-SEC-001`, `TEST-ROOT-SEC-002`).
3. **Index Case Collision & Safe Component Re-Rooting:**
   - Case-folding collisions in scoped index fail closed with `CMP-AUD-005` (`CMP_AUD_CASE_COLLISION`).
   - Component stripping uses `Path::strip_prefix` on original components (no string byte slicing).
   - *Owning Tests:* `runtimepack-core::scoped_index_test` (`TEST-ROOT-SEC-003`).
4. **Cache & Hash Identity Isolation:**
   - Original physical `source_hash` preserved for input provenance; distinct `scoped_content_hash` generated for payload container.
   - `AuditCacheKey` incorporates `content_root`. Authoritative audit ruleset version bumped ($12 \rightarrow 13$).
   - *Owning Tests:* `runtimepack-engine-rpgmaker-web::web_asset_audit_test`, `runtimepack::build_test`.
5. **Native Binary Scoping & Container Alignment:**
   - Payload-internal native DLLs/Node addons flagged as `CMP-102`/`CMP-103`.
   - Outer desktop runners excluded by scoping and logged as `CMP-106` (Info), without concealing active dependencies.
   - In-memory GPACK packaging produces root-level `index.html` without re-ingesting disk.
   - *Owning Tests:* `runtimepack-engine-rpgmaker-web::web_asset_audit_test`, `runtimepack::build_test`, `runtimepack::cli_root_discovery_test`.

---

## 2. Preflight Verification & Toolchain Blocker

### 2.1 Observed Environment Status
- **Physical Git Repositories:** Root `.git/HEAD` physically points to `ref: refs/heads/baseline/vnm-v4` (`52f9f2f`). Worktree `codebase-generalization` and `master` point to `10e2a92`.
- **WSL Non-Root (`mhsymr`):** Verified official Rust 1.99.0 toolchain active (`cargo 1.99.0`, `rustc 1.99.0`) via official rustup 1.29.1, Cargo dependencies root lock fetched, `cargo check` exit 0, PATH set via `$HOME/.cargo/bin:$PATH`. (Approved reviewer report: `docs/superpowers/reports/2026-10-09-wsl-user-toolchain.md`).
- **WSL Root (`root`):** Non-root user `mhsymr` is mandatory; root/sudo builds strictly prohibited.
- **Android ADB:** Live device testing via `adb` blocked until independent environment task; host-side static verification and packaging proceed under non-root `mhsymr`.

### 2.2 Toolchain Preflight Status (RESOLVED)
Preflight toolchain blocker resolved via Option B (approved report `docs/superpowers/reports/2026-10-09-wsl-user-toolchain.md`). All build/test commands execute under normal non-root user `mhsymr` via `wsl -d Ubuntu-24.04 -u mhsymr -- bash -lc 'export PATH="$HOME/.cargo/bin:$PATH"; ...'`. Zero unpinned downloads, official 1.99.0 toolchain verified.

### 2.3 Proposed Isolation Target
Create isolated worktree `.worktrees/fix-mvmz-logical-root` on branch `fix/mvmz-logical-root-20261009` branched from `10e2a92`. This preserves uncommitted edits in `.worktrees/codebase-generalization`, leaves Windows launcher binaries (`RuntimePack-Wizard.bat`, `runtimepack.exe`) untouched, and isolates new work.

---

## 3. Implementation Tasks

### Task 1: Generic Core Carrier, Lexical Containment & Scoped Index View
**Role:** `executor` $\rightarrow$ `code-reviewer`  
**Crate:** `runtimepack-core` (`core/`)

#### Interfaces
- `DetectionCandidate`: Add `pub content_root: Option<String>` (`None` for top-level root, canonical normalized relative subpath for nested).
- `EngineDescriptor`: Add `pub content_root: Option<String>` (`None` for top-level root).
- `EngineManifestEntry` (`core/src/build_plan/mod.rs`): Add `pub content_root: Option<String>`.
- `Detector` trait (`core/src/detection/mod.rs`):
  - `fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate;` (required)
  - `fn probe_all(&self, source: &GameSourceIndex) -> Vec<DetectionCandidate> { vec![self.probe(source)] }` (default wrapper)
- `GameSourceIndex::scoped_to(&self, subpath: &str) -> Result<GameSourceIndex, IngestError>`:
  - Input: relative subpath string.
  - Validation: rejects Windows drive prefixes (`^[a-zA-Z]:`), UNC (`^//`, `^\\\\`), URLs, NUL bytes, `..` traversal components, and leading `/` or `\`.
  - Invariants: preserves `root_path` as physical input root, preserves `IndexedFile.real_path` physical targeting, re-roots logical keys, preserves `source_hash`, computes `scoped_content_hash`, fails closed on case collisions (`CMP_AUD_CASE_COLLISION`).

- [ ] **Step 1: Write Failing Tests**  
  Add `[[test]] name = "scoped_index_test" path = "tests/scoped_index_test.rs"` to `core/Cargo.toml`.  
  Create `core/tests/scoped_index_test.rs`:
  - `test_scoped_index_lexical_traversal_rejection`: asserts `scoped_to("../outside")`, `scoped_to("C:\\game")`, `scoped_to("\\\\server\\share")`, and `scoped_to("dir\0name")` return `CNT_TRAVERSAL_REJECTED` (`CNT-103`).
  - `test_scoped_index_symlink_escape_rejection`: asserts symlinks escaping input root return `ING_SYMLINK_ESCAPE` (`ING-102`).
  - `test_scoped_index_component_rerooting`: asserts `scoped_to("bin/www")` remaps `"bin/www/index.html"` to `"index.html"`, while `real_path` resolves to physical disk path.
  - `test_scoped_index_case_collision_fails_closed`: asserts conflicting entries differing only in case fail closed with `CMP-AUD-005`.
- [ ] **Step 2: Run Failing Test**  
  Command: `wsl -d Ubuntu-24.04 -u mhsymr -- bash -lc 'export PATH="$HOME/.cargo/bin:$PATH"; cd "$WORKSPACE" && cargo test --offline --locked -p runtimepack-core --test scoped_index_test'`
- [ ] **Step 3: Implement Minimal Changes**  
  - Update `core/src/detection/mod.rs` with `content_root` on `DetectionCandidate` and `EngineDescriptor`.
  - Update `Detector` trait in `core/src/detection/mod.rs` with required `probe` and default `probe_all`.
  - Implement `GameSourceIndex::scoped_to` in `core/src/ingest/mod.rs` with pre-normalization lexical validation and `scoped_content_hash`.
  - Update `EngineManifestEntry` in `core/src/build_plan/mod.rs` and `schemas/build-manifest.schema.json`.
- [ ] **Step 4: Verify Passing Test**  
  Command: `wsl -d Ubuntu-24.04 -u mhsymr -- bash -lc 'export PATH="$HOME/.cargo/bin:$PATH"; cd "$WORKSPACE" && cargo test --offline --locked -p runtimepack-core --test scoped_index_test'`
- [ ] **Step 5: Code Review**  
  Subagent `code-reviewer` reviews lexical security, invariant preservation, and trait compatibility.

---

### Task 2: MV/MZ Marker-Owned Discovery, Per-Root Scorer & Ambiguity Contract
**Role:** `executor` $\rightarrow$ `code-reviewer`  
**Crates:** `runtimepack-engine-rpgmaker-web` (`engines/rpgmaker_web/`), `runtimepack-core` (`core/`), `runtimepack` (`cli/`)

#### Interfaces
- `RpgMakerMvDetector::probe_all(&self, source: &GameSourceIndex) -> Vec<DetectionCandidate>`
- `RpgMakerMzDetector::probe_all(&self, source: &GameSourceIndex) -> Vec<DetectionCandidate>`
- `DetectorRegistry::detect_all(&self, source: &GameSourceIndex) -> Vec<DetectionCandidate>`
- `DetectorRegistry::resolve(&self, source: &GameSourceIndex) -> EngineDescriptor`
  - Invariant: uses `candidate.confidence.allows_auto_build()` (`matches!(Exact | High)` with zero conflicts) to prevent `Ord` trap.
  - Invariant: $\ge 2$ auto-build eligible candidates fail closed with `DET-110` (`DET_AMBIGUOUS`). Forced `--engine` on same-engine candidates fails closed.
  - Invariant: candidate scorer awards `index.html` (+40) and `System.json` (+30) ONLY if core/managers marker exists in candidate root $R$.

- [ ] **Step 1: Write Failing Tests**  
  Add tests to `tests/integration/detection_test.rs` (registered as `[[test]] name = "integration_detection"` in `cli/Cargo.toml`):
  - `test_det_mz_nested_arbitrary`: `bin/www/` discovered dynamically, score 210 `EXACT`, `content_root = Some("bin/www")`.
  - `test_det_split_marker_isolation`: `<root>/index.html` + `bin/www/js/rmmz_core.js` does NOT merge scores (inner score 100 `MEDIUM`, root score 0).
  - `test_det_competing_identical_mz_ambiguous`: two MZ payloads scoring 210 fail closed with `DET-110`.
  - `test_det_forced_engine_competing_same_engine`: `--engine rpgmaker-mz` on competing MZ roots still fails closed with `DET-110`.
  - `test_det_mixed_engine_resolution`: MV at root, MZ at `bin/www` disambiguated when `--engine rpgmaker-mz` supplied.
  - `test_det_intra_root_conflict`: candidate root with both `rpg_core.js` and `rmmz_core.js` fails with candidate conflict `DET-110`.
  - `test_det_multiple_medium_roots_accept_detection_fails`: two Medium roots ($80 \le \text{score} < 140$) fail closed even with `--accept-detection`.
- [ ] **Step 2: Run Failing Test**  
  Command: `wsl -d Ubuntu-24.04 -u mhsymr -- bash -lc 'export PATH="$HOME/.cargo/bin:$PATH"; cd "$WORKSPACE" && cargo test --offline --locked -p runtimepack --test integration_detection'`
- [ ] **Step 3: Implement Minimal Changes**  
  - In `engines/rpgmaker_web/src/lib.rs`, replace `find_web_file` with marker-owned candidate derivation scanning `source.files` keys for authoritative suffixes (`rmmz_core.js`, `rmmz_managers.js`, `rpg_core.js`, `rpg_managers.js`) and base root `""`.
  - Implement per-root candidate scoring with intra-root conflict checks.
  - Implement `probe_all` and `probe` on `RpgMakerMvDetector` and `RpgMakerMzDetector`.
  - In `DetectorRegistry::resolve` (`core/src/detection/mod.rs`), enforce `allows_auto_build()` checks and fail-closed ambiguity resolution with `DET-110`.
- [ ] **Step 4: Verify Passing Test**  
  Command: `wsl -d Ubuntu-24.04 -u mhsymr -- bash -lc 'export PATH="$HOME/.cargo/bin:$PATH"; cd "$WORKSPACE" && cargo test --offline --locked -p runtimepack --test integration_detection'`
- [ ] **Step 5: Code Review**  
  Subagent `code-reviewer` reviews scoring weights, conflict isolation, and regression on RGSS/Ren'Py/Tyrano/VNM detectors.

---

### Task 3: Downstream Pipeline Integration — Analyzer, Auditor, Ruleset Bump & GPACK Packaging
**Role:** `executor` $\rightarrow$ `code-reviewer`  
**Crates:** `runtimepack-engine-rpgmaker-web`, `runtimepack-core`, `runtimepack-pack`, `runtimepack`

#### Interfaces
- `RpgMakerWebAnalyzer::analyze(&self, source: &GameSourceIndex, engine: &EngineDescriptor) -> CompatibilityReport`:
  - Operates on scoped index for game scripts.
  - Scans `payload_root` for native DLLs/addons (`CMP-102`/`CMP-103`).
  - Scans outer parent index to classify desktop runners as `CMP-106` (Info).
- `RpgMakerWebAssetAuditor::audit(&self, source: &GameSourceIndex, engine: &EngineDescriptor, options: &AssetAuditOptions) -> AssetAuditReport`:
  - Operates directly on re-rooted paths (`index.html`, `data/System.json`). Hardcoded `www/` prefix removed.
- `AuditCacheKey`:
  - Adds `pub content_root: Option<String>`.
  - Increments `CURRENT_AUDIT_RULESET_VERSION: u32 = 13` (in `core/src/asset_audit/cache.rs`).
- `GpackPacker::pack_from_index_with_options(index: &GameSourceIndex, output_file: &Path, encryption_key: Option<&[u8; 32]>) -> Result<u64, DiagnosticError>`:
  - Packs directly from scoped in-memory index. Discards ad-hoc disk re-ingestion.

- [ ] **Step 1: Write Failing Tests**  
  - Add test in `tests/integration/web_asset_audit_test.rs` (registered in `engines/rpgmaker_web/Cargo.toml`) asserting asset audit operates on scoped index without hardcoded `www/` prefixes.
  - Add test in `tests/integration/build_test.rs` (registered in `cli/Cargo.toml`) asserting GPACK contains root-level `index.html` and excludes outer `bin/nw.exe`/`bin/nw.dll`.
- [ ] **Step 2: Run Failing Tests**  
  Commands:  
  `wsl -d Ubuntu-24.04 -u mhsymr -- bash -lc 'export PATH="$HOME/.cargo/bin:$PATH"; cd "$WORKSPACE" && cargo test --offline --locked -p runtimepack-engine-rpgmaker-web --test web_asset_audit_test'`  
  `wsl -d Ubuntu-24.04 -u mhsymr -- bash -lc 'export PATH="$HOME/.cargo/bin:$PATH"; cd "$WORKSPACE" && cargo test --offline --locked -p runtimepack --test build_test'`
- [ ] **Step 3: Implement Minimal Changes**  
  - Refactor `RpgMakerWebAnalyzer` (`engines/rpgmaker_web/src/scanner.rs`) to evaluate payload files via scoped index and log outer runners via parent index.
  - Remove `content_prefix = "www/"` in `engines/rpgmaker_web/src/asset_audit.rs`.
  - Add `content_root` to `AuditCacheKey` and bump `CURRENT_AUDIT_RULESET_VERSION = 13` in `core/src/asset_audit/cache.rs`.
  - In `cli/src/main.rs:985-991` and `1053-1065`, replace ad-hoc `path.join("www")` and disk re-ingestion with `source_index.scoped_to` and direct `pack_from_index_with_options`.
- [ ] **Step 4: Verify Passing Tests**  
  Commands:  
  `wsl -d Ubuntu-24.04 -u mhsymr -- bash -lc 'export PATH="$HOME/.cargo/bin:$PATH"; cd "$WORKSPACE" && cargo test --offline --locked -p runtimepack-engine-rpgmaker-web --test web_asset_audit_test'`  
  `wsl -d Ubuntu-24.04 -u mhsymr -- bash -lc 'export PATH="$HOME/.cargo/bin:$PATH"; cd "$WORKSPACE" && cargo test --offline --locked -p runtimepack --test build_test'`
- [ ] **Step 5: Code Review**  
  Subagent `code-reviewer` reviews cache key invalidation, in-memory packaging alignment, and outer wrapper logging.

---

### Task 4: CLI Reporting & Host-Side Synthetic Integration Matrix
**Role:** `executor` $\rightarrow$ `code-reviewer`  
**Crate:** `runtimepack` (`cli/`)

#### Interfaces
- `cmd_inspect(path: &Path)`: Logs `Content Root: <path>` or `Content Root: <root>`.
- `cmd_explain(path: &Path)`: Displays candidate roots, per-root marker breakdown, and rejection reasons.
- `cmd_build(args: BuildArgs)`: Emits build manifest containing `engine.content_root`.

- [ ] **Step 1: Write Failing Tests**  
  Add `[[test]] name = "cli_root_discovery_test" path = "../tests/integration/cli_root_discovery_test.rs"` to `cli/Cargo.toml`.  
  Create `tests/integration/cli_root_discovery_test.rs`:
  - Asserts `inspect` on nested synthetic MZ fixture reports `Content Root: bin/www`.
  - Asserts `explain` on nested synthetic fixture logs candidate `bin/www` as selected and outer root as rejected.
  - Asserts regression on XP, VX, VX Ace, Ren'Py, Tyrano, and VNM synthetic fixtures.
- [ ] **Step 2: Run Failing Test**  
  Command: `wsl -d Ubuntu-24.04 -u mhsymr -- bash -lc 'export PATH="$HOME/.cargo/bin:$PATH"; cd "$WORKSPACE" && cargo test --offline --locked -p runtimepack --test cli_root_discovery_test'`
- [ ] **Step 3: Implement Minimal Changes**  
  - Update `cmd_inspect` in `cli/src/main.rs` to print resolved content root.
  - Update `cmd_explain` in `cli/src/main.rs` to display evaluated candidate roots.
  - Ensure build manifest provenance reflects scoped content root.
- [ ] **Step 4: Verify Passing Test**  
  Command: `wsl -d Ubuntu-24.04 -u mhsymr -- bash -lc 'export PATH="$HOME/.cargo/bin:$PATH"; cd "$WORKSPACE" && cargo test --offline --locked -p runtimepack --test cli_root_discovery_test'`
- [ ] **Step 5: Code Review**  
  Subagent `code-reviewer` reviews CLI output formatting, backward compatibility, and error codes.

---

### Task 5: Real Game Static Inspection, Explain, and Container Validation
**Role:** `qa-tester`  
**Execution Harness:** Injectable environment variable `RUNTIMEPACK_TEST_GAME_ROOT` for opt-in host integration harness (zero hardcoded paths or titles in production code).  
**Subject Game Location:** `/mnt/d/02_Development/Raw-Games/A_Simple_Life_With_My_Unobtrusive_Sister_Ver.1.3_for_windows`

#### Interfaces
- QA Test Driver: reads `std::env::var("RUNTIMEPACK_TEST_GAME_ROOT")`, verifies baseline file inventory, executes CLI commands, and asserts output file integrity outside game directory.

- [ ] **Step 1: Compute Pre-Check Baseline (Immutability Gate)**  
  Compute recursive file inventory count and SHA-256 digests of all files in subject directory. Write to `target/qa_reports/pre_check.json`.
- [ ] **Step 2: Run Host `inspect` on Subject Game**  
  Command:  
  `wsl -d Ubuntu-24.04 -u mhsymr -- bash -lc 'export PATH="$HOME/.cargo/bin:$PATH"; cd "$WORKSPACE" && cargo run --offline --locked --bin runtimepack -- inspect "$RUNTIMEPACK_TEST_GAME_ROOT"'`  
  Assert: Engine identified as `RPG Maker MZ`, Confidence `EXACT`, Score `210`, Content Root `bin/www`.
- [ ] **Step 3: Run Host `explain` on Subject Game**  
  Command:  
  `wsl -d Ubuntu-24.04 -u mhsymr -- bash -lc 'export PATH="$HOME/.cargo/bin:$PATH"; cd "$WORKSPACE" && cargo run --offline --locked --bin runtimepack -- explain "$RUNTIMEPACK_TEST_GAME_ROOT"'`  
  Assert: Candidate `bin/www` selected with full marker set; outer candidate scored 0.
- [ ] **Step 4: Execute Static Packaging & Audit**  
  Command:  
  `wsl -d Ubuntu-24.04 -u mhsymr -- bash -lc 'export PATH="$HOME/.cargo/bin:$PATH"; cd "$WORKSPACE" && cargo run --offline --locked --bin runtimepack -- audit "$RUNTIMEPACK_TEST_GAME_ROOT" --output target/qa_reports/audit.json'`  
  Verify audit report outside source directory. If unknown native plugins are reported, disclose them transparently without auto-bypassing gates.
- [ ] **Step 5: Assert Post-Check Baseline (Immutability Gate)**  
  Compute recursive file inventory count and SHA-256 digests. Assert 100% byte equality with `pre_check.json` (zero input mutations).
- [ ] **Step 6: Runtime Status Disclosure**  
  Record QA report: Static detection, scoping, and packaging: **VERIFIED**. Android runtime playability: **`NOT_TESTED`** (APK compile and on-device execution require ADB bridge authorization).

---

## 4. Deliverable Verification Checklist

- [ ] All 5 tasks completed and verified with passing test suites.
- [ ] All 19 rows of the acceptance matrix in `docs/superpowers/specs/2026-10-09-mv-mz-logical-root-design.md` satisfied.
- [ ] Source directories remain 100% unmodified (asserted by byte digests).
- [ ] Existing Windows active binaries (`RuntimePack-Wizard.bat`, `runtimepack.exe`) left intact.
- [ ] Final QA verification report delivered to supervisor and user.

*End of Implementation Plan.*
