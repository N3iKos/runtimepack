# Multi-Engine Logical Content Root Discovery & Backend Stability — Implementation Plan

**Skill Required:** Subagent Driven Development  
**Document ID:** PLAN-20261010-MULTI-ENGINE-ROOT-DISCOVERY  
**Design Spec:** `Docs/superpowers/specs/2026-10-10-multi-engine-root-discovery-design.md`  
**SDD Ledger:** `.superpowers/sdd/2026-10-10-multi-engine-root-discovery/progress.md`  
**Authoritative Worktree Path:** `D:\02_Development\Projects_1\runtimepack-vnm-reconstruction\.worktrees\fix-mvmz-logical-root\Docs\superpowers\plans\2026-10-10-multi-engine-root-discovery.md`  
**Baseline Git Branch:** `fix/mvmz-logical-root-20261009`  
**Toolchain Mandate:** All development, testing, and script execution strictly inside WSL2 (`Ubuntu-24.04`) as non-root user `mhsymr` (`export PATH="$HOME/.cargo/bin:$PATH"`). Source files Unix format LF. Non-destructive operations only. Input sources 100% immutable.

---

## 1. Plan Overview & Architectural Rulings

### 1.1 Scope & Roadmap Strategy
This plan delivers universal marker-owned logical content root discovery across all remaining supported engines, prioritizing **backend stability first** and deferring **Windows binary cross-compilation and Wizard delivery to the final stages**.

### 1.2 Binding Architectural Rulings
- **Ruling 1 (Visual Novel Maker BLOCKED):** Per `AGENTS.md` §1 and §19, v1 engine scope is limited to 7 canonical engines. VNM logical root discovery is formally **BLOCKED** due to normative specification conflict. Existing VNM code in `engines/vnmaker` and `core/src/domain.rs` is strictly preserved without modification or deletion. Only VNM is affected; the 7 supported engines proceed without delay.
- **Ruling 2 (Auto-Build Eligibility & Ambiguity):** Auto-build eligibility is strictly defined as `candidate.confidence.allows_auto_build()` (`matches!(Exact | High)` with zero conflicts). Competing same-engine eligible roots fail closed with `DET-110` (`DET_AMBIGUOUS`) even if `--engine` is provided. Multiple medium roots fail closed with `DET-110` even if `--accept-detection` is provided.
- **Ruling 3 (Strict Root Isolation):** Evidence cannot be merged across candidate roots. Markers in candidate root $R$ contribute only to the score of $R$.
- **Ruling 4 (Ren'Py Distribution Root Preservation):** In Ren'Py distributions, `content_root` must point to the *distribution container* (e.g. `nested/release`), preserving sibling `renpy/` metadata alongside `game/`. Version extraction remains decoupled; failure to extract version results in fail-closed `RNP-110` (`RUNTIME_VERSION_UNRESOLVED`).
- **Ruling 5 (Toolchain & Security Invariants):** No `sudo`, no host package installations, no destructive deletion commands (`rm`, `del`, `git reset --hard`, `git clean -f`).
- **Ruling 6 (Windows Validation Evidence Level):** Cross-compilation on Linux host cannot prove native Windows runtime execution. Artifacts carry `NOT_WINDOWS_RUNTIME_VALIDATED` until tested on physical Windows host or approved Windows CI.

---

## 2. Work Breakdown & Task Assignments

### Task 1: Tyrano Complete Pipeline — Marker Discovery, Scoped Scorer, Auditor/Analyzer & GPACK Packaging
- **Role:** `executor` $\rightarrow$ `code-reviewer`
- **Crates:** `runtimepack-engine-tyrano` (`engines/tyrano/`), `runtimepack-core`, `runtimepack-pack`, `cli/`
- **Scope & Interfaces:**
  1. `engines/tyrano/src/lib.rs`:
     - Implement `derive_candidate_roots` using marker suffixes: `tyrano/tyrano.js`, `tyrano/tyrano.base.js`, `data/system/Config.tjs`, and `resources/app.asar`. Base root `""` always included.
     - Implement `score_tyrano_candidate_root(source, root)`:
       - `index.html`: +80 (awarded ONLY if Tyrano core or scenario markers exist in $R$).
       - `tyrano/tyrano.js` or `tyrano/tyrano.base.js`: +80.
       - `tyrano/plugins/kag/`: +50.
       - `data/scenario/` or `data/system/Config.tjs`: +30.
       - `resources/app.asar`: +120 (awarded ONLY if valid ASAR contains verified `tyrano/tyrano.js` or `tyrano/tyrano.base.js`). Generic ASAR or ASAR without core script scores 0 for ASAR evidence.
       - Returns `DetectionCandidate` with isolated evidence, score, and `content_root`.
     - Implement `aggregate_candidates(candidates)`: fails closed with `DET-110` on multiple eligible roots or multiple medium roots.
     - Implement `Detector::probe_all` and `Detector::probe`.
  2. `engines/tyrano/src/asar.rs`:
     - Harden `AsarArchive::extract_all` with pre-join path validation: reject `..`, absolute paths, drive letters, UNC, NUL bytes, backslash-encoded paths. Reject header case collisions, object `"link"`, and out-of-bounds offsets. Validate destination preflight containment. Fail-closed immediately on invalid entry.
  3. `engines/tyrano/src/scanner.rs`:
     - `TyranoAnalyzer::analyze`: idempotent scoping check (`source.content_root() == engine.content_root.as_deref()`). Scopes via `source.scoped_to(root)` fail-closed.
     - Inside payload: native DLLs or Node modules fail closed (`CMP-102`, `CMP-103`).
     - Outside payload: desktop runners classified as `CMP_DESKTOP_EXCLUDED` (`CMP-106`, Info). Unknown outer DLLs fail closed with `CMP-102`.
  4. `engines/tyrano/src/asset_audit.rs`:
     - `TyranoAssetAuditor::audit`: idempotent scoping check. Consumes scoped index directly. Scenario parsing in `data/scenario/` and `data/system/Config.tjs` operates relative to payload root. Returns `AssetAuditReport` directly.
  5. `core/src/asset_audit/cache.rs`:
     - Increment active ruleset version by +1 ($15 \rightarrow 16$).
  6. `cli/src/main.rs`:
     - When packaging Tyrano with ASAR, resolve `resources/app.asar` via `IndexedFile.real_path`, extract to unique disjoint staging directory, and build `pack_index` from extracted staging so genuine assets reside at GPACK root.
  7. Integration Tests:
     - Tests covering nested Tyrano, ASAR verification, split marker isolation, competing identical roots ambiguity, and GPACK TOC re-rooting.
- **TDD Workflow:**
  - Step 1: Write failing integration tests.
  - Step 2: Run failing tests in WSL2 to confirm red phase.
  - Step 3: Implement candidate derivation, scoped scoring, ASAR security hardening, analyzer/auditor updates, and ruleset bump.
  - Step 4: Run tests in WSL2 to confirm green phase.
  - Step 5: Subagent `code-reviewer` reviews code quality, security, and invariant preservation.

---

### Task 2: Ren'Py Preserved Distribution Root & Game Child Separation
- **Role:** `executor` $\rightarrow$ `code-reviewer`
- **Crates:** `runtimepack-engine-renpy` (`engines/renpy/`), `runtimepack-core`, `cli/`
- **Scope & Interfaces:**
  1. `engines/renpy/src/lib.rs`:
     - Implement `derive_candidate_roots`: infer distribution root from file components `/renpy/` and `/game/`. Base root `""` always included.
     - Candidate root $D$ represents the *distribution container* containing sibling folders `renpy/` and `game/`.
     - Scoped scoring per root $D$: `renpy/` (+90), `.rpy` scripts in `game/` (+50 max), `.rpyc` bytecode in `game/` (+50 max), `.rpa` archives in `game/` (+60 max).
     - Intra-root conflicts: `AndroidManifest.xml` or `classes.dex` inside $D$ flags game as already compiled Android package (fails with conflict).
     - Returns `DetectionCandidate` with `content_root: Some(D)` (or `None` for flat).
     - Implement `Detector::probe_all` and `Detector::probe` with fail-closed ambiguity aggregation.
  2. `engines/renpy/src/scanner.rs` & `engines/renpy/src/asset_audit.rs`:
     - Scoped to distribution root $D$. Analyzer and auditor inspect assets within `game/` subpath.
  3. Version Resolution Invariant:
     - Detection confidence does not imply resolved runtime version. If version extraction fails, resolution fails closed with `RNP-110` (`RUNTIME_VERSION_UNRESOLVED`).
  4. Integration Tests:
     - Nested Ren'Py distribution (`dist/renpy/`, `dist/game/script.rpy`) selects `content_root = Some("dist")`.
     - Multiple identical nested Ren'Py distributions fail closed with `DET-110`.
- **TDD Workflow:**
  - Step 1: Write failing tests for nested Ren'Py distribution root discovery.
  - Step 2: Run failing tests to confirm red phase.
  - Step 3: Implement candidate derivation, scoped scoring, and trait methods.
  - Step 4: Run tests to confirm green phase.
  - Step 5: Subagent `code-reviewer` review.

---

### Task 3: RGSS XP/VX/Ace Same-Root Discovery & RTP Fallback
- **Role:** `executor` $\rightarrow$ `code-reviewer`
- **Crates:** `runtimepack-engine-rgss` (`engines/rgss/`), `runtimepack-core`, `cli/`
- **Scope & Interfaces:**
  1. `engines/rgss/src/lib.rs`:
     - Implement `derive_candidate_roots`:
       - XP: `Data/Scripts.rxdata`, `Game.ini`, `.rgssad` (parent directory of archive file).
       - VX: `Data/Scripts.rvdata`, `Game.ini`, `.rgss2a` (parent directory of archive file).
       - VX Ace: `Data/Scripts.rvdata2`, `Game.ini`, `.rgss3a` (parent directory of archive file).
       - Base root `""` always included.
     - Scoped scoring per root $R$:
       - Loose script archive: +120.
       - Encrypted archive: +120.
       - `Game.ini` inside $R$: +80 (script ref) and +30 (library ref).
       - `Game.exe` inside $R$: +10 (corroboration only).
       - Strict same-root requirement: outer `Game.exe` or `Game.ini` cannot score candidate in $R$.
     - Implement `Detector::probe_all` and `Detector::probe` for `RpgMakerXpDetector`, `RpgMakerVxDetector`, and `RpgMakerVxAceDetector`.
  2. `engines/rgss/src/scanner.rs` & `engines/rgss/src/asset_audit.rs`:
     - Evaluate payload within scoped root. RTP path resolution preserves relative offsets.
  3. Integration Tests:
     - Nested XP, VX, and VX Ace layouts detect with score 240 (`Exact`) and correct `content_root`.
     - Archive-only games (without loose `Data/Scripts.*`) detect correctly without bypassing policy.
     - Competing identical RGSS roots fail closed with `DET-110`.
- **TDD Workflow:**
  - Step 1: Write failing tests for nested RGSS discovery and split marker isolation.
  - Step 2: Run failing tests to confirm red phase.
  - Step 3: Implement candidate derivation, scoped scoring, and trait methods.
  - Step 4: Run tests to confirm green phase.
  - Step 5: Subagent `code-reviewer` review.

---

### Task 4: Full Backend QA & Cross-Engine Matrix
- **Role:** `qa-tester`
- **Scope:**
  1. Execute full workspace test suite in WSL2 across all 7 supported engines.
  2. Comprehensive matrix testing:
     - Flat vs nested discovery for MV, MZ, Tyrano, Ren'Py, XP, VX, VX Ace.
     - Ambiguity fail-closed validation for each engine family.
     - Split marker score isolation tests.
     - Lexical containment tests (rejection of `..`, drive letters, UNC, NUL).
  3. Pre/post streaming SHA-256 tree hashing on all fixtures to assert 100% source immutability.
  4. Emit comprehensive QA report: `.superpowers/sdd/2026-10-10-multi-engine-root-discovery/backend-qa-report.md`.
  5. Playability status reported honestly as `NOT_TESTED` (zero simulated passes).

---

### Task 5: Windows Binary Provision & Cross-Compile Preparation
- **Role:** `executor` $\rightarrow$ `code-reviewer`
- **Scope:**
  1. Toolchain Planning & Documentation:
     - Document official Rust Windows target setup (`x86_64-pc-windows-gnu` / `x86_64-pc-windows-msvc`).
     - Pinned linker requirements (official LLVM/MinGW release with verified SHA-256).
     - Strict policy: no unauthorized downloads, no root/sudo inside WSL.
  2. Codebase Portability Verification:
     - Verify all source files use `#[cfg(windows)]` and `#[cfg(unix)]` correctly.
     - Verify zero Unix command invocations (`df`, `sh`) on Windows targets.
  3. Build Execution:
     - Cross-compile `runtimepack` binary for target `x86_64-pc-windows-gnu`.
  4. Mandatory Disclosure:
     - Artifacts documented as **`NOT_WINDOWS_RUNTIME_VALIDATED`** until tested on native Windows.

---

### Task 6: Wizard Native Delivery & Manual Windows QA
- **Role:** `executor` $\rightarrow$ `code-reviewer` $\rightarrow$ Human Manual Verification
- **Scope:**
  1. Native Delivery Packaging:
     - Generate distribution `.bat` frontend wrapper (`run_N` output directory format).
     - Embed validated CLI binary with verified SHA-256 checksums.
     - Prohibit fallback to binaries from other worktrees.
  2. Workspace & Asset Preservation:
     - Deliver to isolated release payload directory (`dist_release/` or `release/`).
     - Protect existing user work, backups, and configurations.
     - Strictly preserve font profiles without modification.
  3. Verification Checklist:
     - Manual Windows QA execution checklist provided for human user verification.

---

## 3. Coherence & Dependency Matrix

| Task | Primary Artifacts | Depends On | Unblocks |
|---|---|---|---|
| **Task 1** | `engines/tyrano/src/lib.rs`, `asar.rs`, `scanner.rs`, `asset_audit.rs`, cache ruleset +1 | Stage 1 `GameSourceIndex::scoped_to` | Task 4, Task 6 |
| **Task 2** | `engines/renpy/src/lib.rs`, `scanner.rs`, `asset_audit.rs` | Stage 1 `GameSourceIndex::scoped_to` | Task 4, Task 6 |
| **Task 3** | `engines/rgss/src/lib.rs`, `scanner.rs`, `asset_audit.rs` | Stage 1 `GameSourceIndex::scoped_to` | Task 4, Task 6 |
| **Task 4** | Integration test matrix, full workspace suite | Tasks 1, 2, 3 | Task 5 |
| **Task 5** | Windows cross-compile configuration, binary artifact | Task 4 | Task 6 |
| **Task 6** | Distribution `.bat` frontend, release package | Task 5 | Final Delivery |

---

## 4. Acceptance Criteria & Definition of Done

A task is complete only when:
1. Implementation satisfies all normative requirements and invariants in `SPEC-20261010-MULTI-ENGINE-ROOT-DISCOVERY`.
2. Tests pass 100% in WSL2 (serial and parallel runs with native exit code 0).
3. No permanent file deletion commands were executed.
4. Input game fixtures remain 100% byte-for-byte immutable.
5. All diagnostics use stable error codes (`DET-110`, `DET-100`, `CNT-103`, `ING-102`).
6. Zero hardcoding of game titles, credentials, or absolute host paths.
7. Independent review by `code-reviewer` is performed and approved before closing the task.
