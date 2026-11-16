# Multi-Engine Logical Content Root Discovery & Backend Stability — Design Specification

**Status:** Design specification prepared for Supervisor and Human User review; implementation NOT authorized by this document.  
**Date:** 2026-10-10  
**Document ID:** SPEC-20261010-MULTI-ENGINE-ROOT-DISCOVERY  
**Authoritative Worktree Path:** `D:\02_Development\Projects_1\runtimepack-vnm-reconstruction\.worktrees\fix-mvmz-logical-root\Docs\superpowers\specs\2026-10-10-multi-engine-root-discovery-design.md`  
**Toolchain Mandate:** All development, testing, and script execution strictly inside WSL2 (`Ubuntu-24.04`) as non-root user `mhsymr` (`export PATH="$HOME/.cargo/bin:$PATH"`). Source files Unix format LF. Non-destructive operations only. Input sources 100% immutable.

---

## 1. Intent & Architectural Boundaries

### 1.1 Problem Statement
In desktop game distributions, game engines are frequently deployed within nested directory structures alongside desktop runtime wrappers (such as NW.js, Electron, Python, or Ruby executables).
Stage 1 (`SPEC-20261009-MV-MZ-LOGICAL-ROOT`) resolved nested payload discovery, per-root scoring, and scoped GPACK packaging for RPG Maker MV and MZ. However, the remaining supported engine families in RuntimePack still exhibit root-coupling limitations:
1. **Tyrano (`runtimepack-engine-tyrano`):** `TyranoDetector::probe` evaluates top-level root `""` and top-level `"resources/app.asar"` only. Nested web or Electron distributions fail detection with `[DET-100]` (Engine Unknown). In addition, existing ASAR handling lacks extraction security validation and does not integrate into the GPACK packaging pipeline.
2. **Ren'Py (`runtimepack-engine-renpy`):** `RenPyDetector::probe` scans keys globally without root scoping. When a Ren'Py game is nested inside an installer wrapper (e.g. `<game>/release/renpy/` and `<game>/release/game/*.rpy`), detection lacks containment, and packaging must distinguish the distribution root from the child `game/` folder.
3. **RGSS (`runtimepack-engine-rgss`):** `RpgMakerXpDetector`, `RpgMakerVxDetector`, and `RpgMakerVxAceDetector` evaluate top-level `"Game.ini"` and loose scripts (`"Data/Scripts.*"`). Nested distributions fail detection, while loose outer `Game.exe` files could artificially bias scoring across unrelated folders.
4. **Visual Novel Maker (VNM):** Exists as an experimental profile in codebase, but conflicts with root normative specifications (`AGENTS.md`).

### 1.2 User Directives & Long-Horizon Execution Roadmap
Per explicit user delegation, the execution priority is:
1. **Backend Discovery & Stability FIRST:** Full, robust root discovery, strict per-root evidence isolation, fail-closed ambiguity handling, secure ASAR pipeline, and downstream packaging for all supported engines.
2. **Windows Build & Wizard Native Delivery LAST:** Toolchain provisioning, Windows cross-compilation, distribution `.bat` packaging, and manual Windows QA deferred to final stages.
3. **Roadmap Ordering:**
   - **Task 1: Tyrano Complete Pipeline:** Logical root discovery, verified ASAR engine scoring, secure ASAR staging extraction, scoped auditor/analyzer, ruleset bump, and in-memory GPACK packaging.
   - **Task 2: Ren'Py Preserved Distribution Root + Child Separation:** Discovery of nested distribution root (preserving sibling `renpy/` metadata), separate version resolution (`RNP-110`), and child `game/` mounting.
   - **Task 3: RGSS (XP, VX, VX Ace) Same-Root Discovery & RTP Fallback:** Strict same-root co-location of `Game.ini` and script archives, encrypted archive parent detection, outer artifact isolation, and static RTP fallback metadata preservation.
   - **Task 4: Full Backend QA & Cross-Engine Matrix:** Comprehensive synthetic integration matrix covering all engines across flat and nested layouts with 100% source immutability assertions.
   - **Task 5: Windows Binary Provision & Toolchain Preparation:** Official pinned Windows target preparation, cross-compile configuration, and `NOT_WINDOWS_RUNTIME_VALIDATED` disclosure.
   - **Task 6: Wizard Native Delivery & Manual Windows QA:** Distribution `.bat` frontend wrapper (`run_N` output), release payload folder deployment, and preservation of existing user work and font profiles.

---

## 2. Normative Scope & Spec Conflict Rulings

### 2.1 Scope Authority Hierarchy
Per `AGENTS.md` §3 (Documentation Precedence):
1. `AGENTS.md` (Highest authority)
2. `Docs/TECHNICAL_SPEC_V1.md`
3. Accepted ADRs
4. `Docs/ARCHITECTURE.md`
5. Engine-specific specifications
6. `Docs/PRD.md`
7. `Docs/IMPLEMENTATION_PLAN.md`

### 2.2 Formal Ruling on Visual Novel Maker (VNM) — Path BLOCKED
- **Conflict Identification:** `AGENTS.md` §1 and §7 strictly restrict v1 scope to 7 engines: RPG Maker XP, VX, VX Ace, MV, MZ, Ren'Py, and TyranoBuilder/TyranoScript. Conversely, `Docs/ENGINE_DETECTION.md` §10 and `core/src/domain.rs` define an experimental profile for `VisualNovelMaker`.
- **Precedence Enforcement:** Under `AGENTS.md` §1 ("Do not silently extend this scope") and §19 ("Stop affected implementation path and report a blocker for SPEC_CONFLICT"):
  - Implementation of logical root discovery for Visual Novel Maker is **BLOCKED**.
  - **Preservation Policy:** Existing implemented VNM code in `engines/vnmaker`, `core/src/domain.rs`, and test suites MUST be strictly preserved. No deletion, no registry removal, and no breaking mutations.
  - **Zero Scope Invention:** Autonomous agents possess zero authority to redefine v1 scope or invent normative requirements for VNM root discovery without formal written amendment from the human supervisor.
  - **Plan Isolation:** This blockage affects ONLY the VNM path. It does NOT stall or delay the multi-engine root discovery roadmap for the 7 canonical engines.

### 2.3 External Evidence Citations (Canonical Registry)
All external engine claims are bound strictly to verified references in `Docs/REFERENCES.md`:
- **Tyrano:** `S10` (TyranoScript repository layout `data`, `tyrano`, `index.html`), `S11` (TyranoBuilder web export tutorial), `S12` (TyranoScript `index.html` loading KAG stack), `S23` (JoiPlay Tyrano archive template).
- **Ren'Py:** `S07` (Ren'Py Android / RAPT documentation), `S08` (Ren'Py CLI build commands), `S09` (Ren'Py license requirements), `S18` (Ren'Py versioned plugin breakdown), `S28` (Ren'Py syntax and RPA archive specifications).
- **RGSS:** `S06` (mkxp-z open-source RGSS runtime), `S20` (XP `Game.ini` and `.rxdata` template), `S21` (VX `Game.ini` and `.rvdata` template), `S22` (VX Ace `Game.ini` and `.rvdata2` template), `S27` (Ruby Marshal 4.8 format and RGSS data specifications).
- **MV / MZ:** `S13` (MV HTML5 web execution), `S14` (MZ web deployment), `S36` (NW.js v0.114.0 redistributable wrapper DLL baseline).
- **Electron ASAR Archive Format:** Missing formal entry in `Docs/REFERENCES.md`. Registered as explicit spike `SPIKE-ASAR-SPEC-001`. Implementation must not invent archive behaviors beyond standard Electron ASAR v4 format.

---

## 3. Universal Marker-Owned Root Derivation Architecture

### 3.1 Generic Discovery Principle (`TECH-CORE-001`)
The host core (`runtimepack-core`) provides the generic carrier and in-memory scoping engine (`GameSourceIndex::scoped_to`), while engine-specific crates define their authoritative marker derivation strategies.
No recursive filesystem walks, no arbitrary `max_depth` limits, and no hardcoded folder name allowlists (`bin`, `app`, `www`, `release`) are permitted in core logic.

### 3.2 Three Distinct Derivation Strategies by Component Boundary
Because `GameSourceIndex` stores leaf file paths rather than directory entries, candidate roots are derived from in-memory file keys using three distinct, boundary-safe strategies:

1. **Component-Boundary Full-File Suffix (MV, MZ, Tyrano loose):**
   - Authoritative marker suffixes: `js/rpg_core.js`, `js/rpg_managers.js`, `js/rmmz_core.js`, `js/rmmz_managers.js`, `tyrano/tyrano.js`, `tyrano/tyrano.base.js`, `resources/app.asar`.
   - Matching rule: If `file.logical_path` ends with `/marker_suffix`, candidate root is `&path[..path.len() - cut_len]`. If `path == marker_suffix`, candidate root is `""`.
2. **Directory Segments Inferred from Indexed Files (Ren'Py):**
   - Ren'Py stores assets in a `game/` child folder and engine scripts in a `renpy/` sibling folder.
   - Matching rule: Scan components of `file.logical_path`. If path contains `/renpy/` or `/game/` segments, candidate distribution root is the prefix preceding `/renpy/` or `/game/`.
   - If path starts with `renpy/` or `game/`, candidate root is `""`.
3. **Archive Extension Basename Parent (RGSS):**
   - RGSS encrypted archives have file extensions `.rgssad`, `.rgss2a`, `.rgss3a` (e.g. `Game.rgssad`, `Data.rgss2a`, `Encrypted.rgss3a`).
   - Matching rule: For any indexed file ending with `.rgssad`, `.rgss2a`, or `.rgss3a`, candidate root is the **parent directory** of that archive file (NOT naive string matching for `/.rgssad`). If the file is at root, candidate root is `""`.
   - For `Data/Scripts.rxdata`, `Data/Scripts.rvdata`, `Data/Scripts.rvdata2`, the candidate root is the parent directory of `Data`.
   - For `Game.ini`, the candidate root is the parent directory of `Game.ini`.

### 3.3 Normalization, Deduplication & Invariants
- **Base Root Invariant:** Top-level root `""` is ALWAYS inserted into `candidate_roots` to ensure standard flat game distributions continue to be evaluated against the baseline.
- **Entrypoint Protection:** Generic entrypoints such as `index.html` MUST NOT generate candidate roots on their own. They only award points when accompanied by engine-specific marker files within candidate root $R$ (preventing generic HTML false positives per `DET-150`).
- **Lexical Validation & Dedup:** Candidate roots are validated lexically (no `..`, drive letters, UNC, NUL, leading slashes), deduplicated case-insensitively, and sorted deterministically in `BTreeSet<String>` while preserving the original casing of the candidate root from the index.

---

## 4. Strict Root Isolation, Scoring & Ambiguity Contracts

### 4.1 Strict Per-Root Isolation (`ROOT-002`)
Markers located outside candidate root $R$ MUST NOT contribute to the score of $R$.
Cross-root score merging is strictly prohibited. For example:
- An outer `index.html` at `""` and an inner `tyrano/tyrano.js` at `"bin/tyrano/tyrano.js"` DO NOT combine.
- An outer `Game.exe` at `""` and an inner `Data/Scripts.rxdata` at `"payload/Data/Scripts.rxdata"` DO NOT combine.

### 4.2 Scoring Weights & Thresholds
Confidence is derived strictly from score thresholds via `DetectionCandidate::calculate_confidence(score)`:
- $\ge 200$: **`DetectionConfidence::Exact`**
- $\ge 140$: **`DetectionConfidence::High`**
- $\ge 80$: **`DetectionConfidence::Medium`**
- $\ge 30$: **`DetectionConfidence::Low`**
- $< 30$: **`DetectionConfidence::None`**

#### Tyrano Scoped Scoring Matrix
Evaluated strictly within candidate root $R$:
- `index.html`: **+80** (awarded ONLY if Tyrano core or scenario markers exist in $R$)
- `tyrano/tyrano.js` or `tyrano/tyrano.base.js`: **+80**
- `tyrano/plugins/kag/`: **+50**
- `data/scenario/` or `data/system/Config.tjs`: **+30**
- `resources/app.asar`: **+120** (awarded ONLY if valid ASAR contains verified Tyrano engine-defining markers)

**Critical ASAR Verification Rule:**
- Mere presence of `resources/app.asar` scores **0**.
- Presence of generic Electron `resources/app.asar` with `index.html` scores **0** (does not select Tyrano).
- Presence of `Config.tjs` or `scenario` inside ASAR without `tyrano/tyrano.js` or `tyrano/tyrano.base.js` scores **0** for ASAR evidence (weak metadata is not engine-defining proof alone).
- Full ASAR Baseline (ASAR with tyrano.js, kag, scenario, index): $120 + 80 + 80 + 50 + 30 = 360$ (`Exact`, $\ge 200$)
- Minimal ASAR Baseline (ASAR with tyrano.js + index): $120 + 80 + 80 = 280$ (`Exact`, $\ge 200$)
- Flat Loose Baseline (all loose markers, no ASAR): $80 + 80 + 50 + 30 = 240$ (`Exact`, $\ge 200$)
- Minimal Loose Baseline (index + tyrano.js, no ASAR): $80 + 80 = 160$ (`High`, $\ge 140$)
- Partial Script (tyrano.js + kag, no index): $80 + 50 = 130$ (`Medium`, $\ge 80$)

#### RGSS Scoped Scoring Matrix
Evaluated strictly within candidate root $R$:
- `Data/Scripts.rxdata` / `rvdata` / `rvdata2`: **+120**
- Encrypted archive `.rgssad` / `.rgss2a` / `.rgss3a`: **+120**
- `Game.ini` referencing script extension: **+80**
- `Game.ini` referencing RGSS library: **+30**
- `Game.exe`: **+10** (corroborating marker only)
- **Standard Flat Baseline:** $120 + 80 + 30 + 10 = 240$ (`Exact`)
- **Archive-Only Baseline (user provided archives without loose scripts):** $120 + 80 + 30 = 230$ (`Exact`)

#### Ren'Py Scoped Scoring Matrix
Evaluated strictly within distribution root $D$:
- `renpy/` directory: **+90**
- `.rpy` scripts in `game/`: **up to +50** ($10 \times \text{count}$, capped)
- `.rpyc` bytecode in `game/`: **up to +50** ($5 \times \text{count}$, capped)
- `.rpa` archives in `game/`: **up to +60** ($20 \times \text{count}$, capped)
- Conflict: presence of `AndroidManifest.xml` or `classes.dex` flags game as already compiled Android package (fails with conflict).
- Standard distribution: $90 + 50 + 50 + 60 = 250$ (`Exact`)

### 4.3 Fail-Closed Ambiguity Contract (`ROOT-004`)
Auto-build eligibility is strictly defined as `candidate.confidence.allows_auto_build()` (`matches!(Exact | High)` with zero conflicts).
1. **$\ge 2$ Auto-Build Eligible Roots for the Same Engine:** Fails closed with `DET-110` (`DET_AMBIGUOUS`). Forced `--engine` flag CANNOT bypass this ambiguity; the user must specify the explicit payload subfolder as CLI input.
2. **$\ge 2$ Auto-Build Eligible Roots across Different Engines:** Fails closed with `DET-110`. Providing `--engine` filters candidates to that engine; if exactly one eligible root remains, it is selected.
3. **$\ge 2$ Medium Roots:** Fails closed with `DET-110` (no arbitrary pick), even when `--accept-detection` is passed.
4. **0 Eligible, 0 Medium Roots:** Fails closed with `DET-100` (`DET_UNKNOWN`).

---

## 5. Tyrano ASAR Complete Pipeline & Security Specification (Task 1)

### 5.1 Nested ASAR Complete Pipeline
When a Tyrano game is distributed as an Electron package (`resources/app.asar` or nested `wrapper/resources/app.asar`), the Android WebView shell cannot execute from the raw `.asar` blob.
1. **Index Scoping & Physical Path Resolution:**
   - CLI resolves `resources/app.asar` using `IndexedFile.real_path` on the effective scoped index, NOT naive `path.join("resources/app.asar")` which ignores compatibility staging profiles.
2. **Secure Extraction to Staging:**
   - The archive is extracted to a new, unique, disjoint staging directory (`target/staging_asar_<id>`).
   - All entries are prevalidated before writing (see §5.2).
3. **GPACK Packaging Index Precedence:**
   - The packing index (`pack_index`) is ingested from the extracted staging directory, taking precedence over the scoped index holding the raw ASAR blob.
   - Genuine loaded assets (`index.html`, `tyrano/tyrano.js`, `data/`) reside at the root of `game.pack`.
   - Raw `resources/app.asar` is omitted from `game.pack`.
4. **Metadata Preservation:**
   - `descriptor.content_root` tracks the physical distribution root on disk (e.g. `wrapper`), preserving provenance in `build-manifest.json`.
5. **Engine-Agnostic Core Preservation:**
   - ASAR extraction helper resides in `engines/tyrano/src/asar.rs`, orchestrated by the CLI known-engine build pipeline without introducing new engine-specific `if` branches in `core/`.

### 5.2 ASAR Entry Extraction Security Validation
Before joining destination path or writing any file during ASAR extraction, every entry MUST be validated:
1. **Cross-Platform Syntax Validation on `entry.path`:**
   - Reject traversal components: `..`, `/../`, `\..\`, leading `../`, trailing `/..`.
   - Reject absolute paths: leading `/` or `\`.
   - Reject Windows drive letters: `^[a-zA-Z]:`.
   - Reject UNC network prefixes: `^//` or `^\\\\`.
   - Reject URL schemes: `://`.
   - Reject NUL bytes: `\0`.
   - Reject backslash-encoded paths.
2. **Header & Entry Integrity:**
   - Reject header case collisions: fail closed if entries differ only in ASCII case.
   - Bounds checking: verify `data_offset + entry.offset + entry.size <= asar_file_len`.
   - Object `"link"` and `"unpacked"`: reject as unsupported; do not follow symlinks or copy unpacked paths outside source.
3. **Preflight Destination Containment:**
   - Validate that canonicalized target path is strictly contained within `dest_dir`.
   - Any invalid entry causes extraction to REJECT fail-closed immediately; no silent skipping.
   - On error, destination staging is cleaned up, and input source game remains 100% immutable.

### 5.3 Idempotent Scoped Views in Analyzer & Auditor
1. **Idempotent Scoping:**
   - `TyranoAnalyzer::analyze` and `TyranoAssetAuditor::audit` compare `source.content_root() == engine.content_root.as_deref()`.
   - If already scoped, use `source` directly. Do not re-scope or clear.
   - If scoping fails, fail closed: analyzer sets `CompatibilityStatus::UnsupportedDependency` with `CNT-103`; auditor records finding and sets outcome `AuditOutcome::Fail`.
2. **Native Binaries & DLL Classification:**
   - Native DLLs (`.dll`, `.so`) or Node addons (`.node`) located inside `payload_root` (or inside the ASAR archive) fail closed with `CMP-102` (`CMP_NATIVE_DLL_DEPENDENCY`) or `CMP-103` (`CMP_NODE_NATIVE_MODULE`).
   - Known desktop runners (`electron.exe`, `nw.dll`, `ffmpeg.dll`, `node.dll`, `d3dcompiler_*.dll`) strictly outside `payload_root` are classified as `CMP_DESKTOP_EXCLUDED` (`CMP-106`, Info).
   - Unknown DLLs outside payload fail closed with `CMP-102`.
3. **Auditor Trait Parity & Ruleset Bump:**
   - `AssetAuditor::audit` returns `AssetAuditReport` directly, matching trait contract.
   - Increment active ruleset version by +1 in `core/src/asset_audit/cache.rs` ($15 \rightarrow 16$).

---

## 6. Ren'Py & RGSS Pipeline Specifications (Tasks 2 & 3)

### 6.1 Ren'Py Pipeline Specification (Task 2)
- **Distribution Root vs Child Root Invariant:**
  - `content_root` MUST point to the **selected distribution root** (e.g. `nested/release`), NOT `nested/release/game`.
  - Rationale: Downstream build and runtime packaging requires the presence of `renpy/` metadata and launcher files to extract runtime version information and configure Android execution.
- **Decoupled Version Resolution:**
  - Detection identifies `EngineId::RenPy`.
  - Version extraction remains a separate phase. If the exact version generation cannot be extracted from `renpy/` or `.rpyc` headers, the build fails closed with `RNP-110` (`RUNTIME_VERSION_UNRESOLVED`). The toolchain NEVER guesses an unverified runtime version (`TECH-RENPY-002`).

### 6.2 RGSS Pipeline Specification (Task 3)
- **Strict Same-Root Co-location:**
  - Candidate root $R$ is derived from script archives, `Game.ini`, or encrypted archives.
  - A candidate root $R$ is only awarded `Game.ini` points if `Game.ini` exists inside $R$.
  - An outer `Game.exe` cannot produce false high-confidence RGSS scores for nested payloads.
- **RTP Auto-Fallback Metadata Preservation:**
  - When `Game.ini` specifies `RTP1`, `RTP2`, or `RTP`, RTP auto-provisioning preserves asset paths relative to the selected logical content root.
  - User-provided encrypted archive games lacking loose `Data/Scripts.*` files retain full detection capability based on archive signatures.

---

## 7. Windows Binary Provision & Delivery Specifications (Tasks 5 & 6)

### 7.1 Cross-Compilation & Host Security Rules (Task 5)
- **Zero Root / Sudo:** No `sudo apt install` or system-wide toolchain mutation inside WSL2.
- **Toolchain Pinning:** Windows cross-compilation must utilize official rustup target components (`rustup target add x86_64-pc-windows-gnu`) and official pinned LLVM/MinGW linkers with cryptographic SHA-256 validation.
- **Cross-Platform Conditional Compilation:** Ensure all source files use `#[cfg(windows)]` and `#[cfg(unix)]` appropriately. Prohibit Unix command execution (`df`, `sh`) on Windows targets.
- **Mandatory Disclosure:** Artifacts documented as **`NOT_WINDOWS_RUNTIME_VALIDATED`** until tested on a physical Windows host or approved Windows CI pipeline.

### 7.2 Wizard Native Delivery & User Work Preservation (Task 6)
- **Deployment Safety:** Delivery of native `.bat` frontends and CLI binaries must output to isolated release folders (`dist_release/` or `run_N/`).
- **No Overwrite:** Existing user work, old build logs, and font profiles MUST NOT be overwritten or cleaned up.
- **Zero Hardcoding:** All device IDs, package names, and paths must remain dynamic and configurable via CLI or declarative manifests.

---

## 8. Verification & Acceptance Criteria

1. **Tyrano Scoped Pipeline:**
   - Flat Tyrano (`tyrano_min`) detects as `Tyrano` with score 240 (`Exact`), `content_root = None`.
   - Nested Tyrano (`bin/app/tyrano/tyrano.js`, `bin/app/index.html`) detects as `Tyrano` with score 240 (`Exact`), `content_root = Some("bin/app")`.
   - Nested valid ASAR with Tyrano markers detects as `Tyrano` with score $\ge 280$ (`Exact`).
   - Generic ASAR without Tyrano markers scores 0 and is rejected.
   - Malformed ASAR gives stable diagnostic (`DET-100` / `ING-101`), not UNKNOWN compatible.
   - ASAR traversal cases fail closed with zero writes outside staging.
   - Competing identical Tyrano roots fail closed with `DET-110`.
   - Output GPACK contains root `index.html` with authentic bytes, excluding raw ASAR blob.
   - Active ruleset version incremented by +1 ($15 \rightarrow 16$).
2. **Ren'Py Scoped Pipeline:**
   - Nested Ren'Py distribution (`release/renpy/`, `release/game/script.rpy`) selects `content_root = Some("release")`.
   - Unresolved runtime version fails closed with `RNP-110`.
3. **RGSS Scoped Pipeline:**
   - Nested VX Ace (`sub/Game.ini`, `sub/Data/Scripts.rvdata2`) detects as `RpgMakerVxAce` with score 240, `content_root = Some("sub")`.
   - Outer `Game.exe` in root does not elevate nested XP score.
4. **Source Immutability:**
   - All tests assert pre-execution and post-execution tree hashes match byte-for-byte.
5. **No Regressions:**
   - Existing MV/MZ logical root discovery and all existing unit/integration tests continue to pass 100%.
