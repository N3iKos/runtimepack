# RPG Maker MV/MZ Logical Content Root Discovery & Carrier Architecture — Design Specification

**Status:** Design specification prepared for Supervisor and User review; implementation NOT authorized by this document.  
**Date:** 2026-10-09  
**Document ID:** SPEC-20261009-MV-MZ-LOGICAL-ROOT  
**Target Repository:** `D:\02_Development\Projects_1\runtimepack-vnm-reconstruction`  
**Toolchain Mandate:** All development, testing, and script execution strictly inside WSL2 (`Ubuntu-24.04`). Source files format LF. Non-destructive operations only.

---

## 1. Intent & Architectural Boundaries

### 1.1 Problem Statement
Desktop distributions of RPG Maker MV and MZ often package game payloads within nested directories (e.g. `<game>/bin/www/index.html`, `<game>/bin/www/js/rmmz_core.js`) alongside desktop runner binaries (`nw.exe`, `nw.dll`, `ffmpeg.dll`, `package.json`).
Current limitations in `runtimepack`:
1. `engines/rpgmaker_web/src/lib.rs:10-19` (`find_web_file`) checks only physical root `""` and `"www/"`. Nested distributions fail static detection with `[DET-100]` (Engine Unknown), requiring manual subfolder CLI inputs.
2. `find_web_file` cross-merges evidence across roots (e.g. outer launcher `index.html` + inner `www/js/rpg_core.js`), manufacturing inaccurate detection confidence.
3. Downstream packaging (`cli/src/main.rs:985-991` and `cli/src/main.rs:1053-1065`) relies on ad-hoc `path.join("www")` and re-ingests the filesystem from disk, discarding in-memory index state.
4. Core domain models lack a validated carrier for relative logical content roots.

### 1.2 User Decision & Integrated Stage 1 Scope
Per user directive (*"yaudah bertahap dulu"*), Stage 1 delivers an **integrated MV/MZ solution**: marker-owned discovery, strict root isolation, domain carrier, scoped logical index, analyzer/auditor alignment, and GPACK packaging.
- **Stage 1 Engines:** RPG Maker MV (`EngineId::RpgMakerMv`) and RPG Maker MZ (`EngineId::RpgMakerMz`) only.
- **Generic Core Carrier:** Designed in `core` to be reusable for future engine extensions without breaking domain contracts.
- **Unchanged Engines:** Detection and packaging for RGSS, Ren'Py, Tyrano, and VNM remain untouched.
- **Decoupled Responsibilities:** Static engine detection, content root discovery, and compatibility analysis remain distinct. Detection never infers compatibility; analyzer status remains independent.
- **Deferred to Future Stages:** Generalization to other engines (Ren'Py/Tyrano/RGSS), manifest (`package.json`) hint parsing, and archive inspection.
- **Strict Invariants:** No AI/probabilistic heuristics, zero source modifications (source immutability), no execution of game binaries, no hardcoded folder allowlists (anti-overfitting), and no artificial engine locks on general flags.

---

## 2. Source Provenance & Workspace Status

### 2.1 Physical Workspace Verification
Static verification of the host and WSL environments reveals:
- **Root `.git`:** Physically present at `.git/HEAD` pointing to `ref: refs/heads/baseline/vnm-v4`. (Harness banner `"Is directory a git repo: no"` reflects container/subagent execution limits; `.git` exists on disk).
- **Worktree Executable:** `.worktrees/codebase-generalization/target/debug/runtimepack.exe` physically exists (114 MB, Oct 8). It MUST NOT be executed directly on the Windows host per WSL2 execution rules.
- **Source Divergence:** `.worktrees/codebase-generalization` contains anti-hardcode changes (`core/src/profile/`, `GameProfileCatalog`) unmerged into the root workspace. Both branches currently share the identical `find_web_file` limitation and ad-hoc `www/` packaging.
- **Test Corroboration:** Reviewer testing confirmed an outer desktop distribution with `bin/www/` failed detection (`DET-100`, MZ score 0), whereas supplying `bin/www` produced MZ score 210 (`EXACT`). Commercial game paths remain strictly read-only; test fixtures use synthetic project-owned paths.

---

## 3. Normative Requirements

### 3.1 Canonical Requirements (Quoted Verbatim)
- **`TECH-CORE-001`:** "The host core MUST be engine-agnostic. Engine-specific logic belongs in detectors, analyzers, adapters, or backends."
- **`TECH-CORE-002`:** "The generated APK MUST contain exactly one `BuildManifest` describing the engine, runtime, content format, permissions, ABI set, and tool versions."
- **`TECH-CORE-003`:** "`inspect` MUST NOT execute game scripts, load native libraries from the game, launch NW.js, launch Python game code, or invoke an RPG Maker executable."
- **`TECH-CORE-004`:** "The input directory MUST be treated as immutable unless an explicit `--in-place` mode is added in a future specification."
- **`TECH-INPUT-001`:** "Symlinks escaping the source root MUST be rejected during ingestion."
- **`TECH-INPUT-002`:** "Paths MUST be normalized into a platform-neutral logical path using `/` separators while preserving original path spelling metadata for case-sensitive diagnostics."
- **`TECH-INPUT-003`:** "Source hashing SHOULD use SHA-256 for build provenance."

### 3.2 Specific Design Requirements
- **`ROOT-001` (Marker-Owned Discovery):** Candidate content roots are derived exclusively from indexed authoritative engine markers. Zero redundant filesystem I/O.
- **`ROOT-002` (Strict Root Isolation):** Markers outside candidate root $R$ MUST NOT contribute to the score of $R$. Cross-root score merging is prohibited.
- **`ROOT-003` (Canonical Carrier):** Content root is carried in domain descriptors as `Option<String>`. Canonical top-level root is strictly `None`.
- **`ROOT-004` (Fail-Closed Ambiguity):** Multiple auto-build eligible roots fail closed with `DET-110` (`DET_AMBIGUOUS`). Forced engine flags cannot silently select between multiple roots of that engine.
- **`ROOT-005` (Lexical Containment):** Candidate roots must pass lexical validation (no drive letters, UNC, URLs, NUL, or traversal components) before normalization.
- **`ROOT-006` (Scoped Index Packaging):** Downstream consumers (Analyzer, Auditor, GPACK) consume the in-memory scoped index directly without re-ingesting disk paths.

---

## 4. Root Discovery & Scorer Architecture

### 4.1 Marker-Owned Discovery Algorithm
Candidate roots are discovered purely by scanning the in-memory keys of `GameSourceIndex.files` ($O(N)$ key matching, zero disk reads):
1. **Authoritative Markers:**
   - MZ: Suffixes `js/rmmz_core.js` and `js/rmmz_managers.js`.
   - MV: Suffixes `js/rpg_core.js` and `js/rpg_managers.js`.
   - Base: Top-level root `""` is always evaluated as the base candidate.
2. **Derivation Rule:**
   For any file key ending in a marker suffix:
   - Strip the marker suffix and trailing slash to produce candidate root $R$.
   - Generic `index.html` matches throughout arbitrary directories DO NOT generate candidate roots (prevents treating random HTML docs as game roots per `DET-150`).
3. **Deduplication:**
   Candidate roots are stored in a `BTreeSet<String>` ensuring deterministic ordering.

### 4.2 Scoped Scorer & Weightings
Every candidate root $R$ is evaluated strictly within its own scope:
- Entrypoint: if $R$ is empty then `"index.html"` else `format!("{}/index.html", R)`
- Core script: if $R$ is empty then `"js/rmmz_core.js"` else `format!("{}/js/rmmz_core.js", R)`
- Managers script: if $R$ is empty then `"js/rmmz_managers.js"` else `format!("{}/js/rmmz_managers.js", R)`
- Database system: if $R$ is empty then `"data/System.json"` else `format!("{}/data/System.json", R)`

**Scoring Weights (Preserving Canonical `ENGINE_DETECTION.md` Rules):**
- Core script (`core.js`): **+100**
- Managers script (`managers.js`): **+40**
- Entrypoint (`index.html`): **+40** (awarded ONLY if core or managers script exists in $R$)
- Database system (`System.json`): **+30** (awarded ONLY if core or managers script exists in $R$)

**Score-to-Confidence Thresholds (`calculate_confidence`):**
- Score 210: **`DetectionConfidence::Exact`** ($\ge 200$)
- Score 140–180: **`DetectionConfidence::High`** ($\ge 140$)
- Score 80–110: **`DetectionConfidence::Medium`** ($\ge 80$) *(Note: Score 100 is Medium, not High)*
- Score 30–70: **`DetectionConfidence::Low`** ($\ge 30$)
- Score $< 30$: **`DetectionConfidence::None`**

If candidate $R$ contains both MV core (`rpg_core.js`) and MZ core (`rmmz_core.js`), it records an explicit conflict and confidence caps at `DetectionConfidence::Ambiguous`.

---

## 5. Candidate Ambiguity & Selection Contract

### 5.1 Auto-Build Eligibility Trap
In `core/src/domain.rs`, `DetectionConfidence` derives `Ord` with `Ambiguous` as the final variant (`None < Low < Medium < High < Exact < Ambiguous`).
**Ordinal comparison (`candidate.confidence >= High`) is strictly forbidden** because it incorrectly treats `Ambiguous` as eligible.
Auto-build eligibility MUST use `candidate.confidence.allows_auto_build()` (`matches!(confidence, Exact | High)` with zero conflicts).

### 5.2 Resolution Matrix
1. **Exactly 1 Auto-Build Eligible candidate:** Selected deterministically. Weak/medium alternative candidates do not create ambiguity; they are logged in `explain` diagnostics.
2. **$\ge 2$ Auto-Build Eligible candidates (same engine):** Fails closed with `DET-110` (`DET_AMBIGUOUS`). `--engine` flag CANNOT bypass this ambiguity; user must specify the exact subfolder as CLI input.
3. **$\ge 2$ Auto-Build Eligible candidates (different engines):** Fails closed with `DET-110`. If user passes `--engine`, candidates of other engines are filtered out; if exactly one candidate of the specified engine remains, it is selected.
4. **0 Auto-Build Eligible, $\ge 2$ Medium candidates:** Fails closed with `DET-110`. `--accept-detection` CANNOT arbitrarily choose between competing Medium roots.
5. **0 Auto-Build Eligible, 0 Medium candidates:** Fails closed with `DET-100` (`DET_UNKNOWN`).

---

## 6. Domain Carrier & Interface Contracts

### 6.1 Domain Struct Definitions
In `core/src/detection/mod.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionCandidate {
    pub engine: EngineId,
    pub confidence: DetectionConfidence,
    pub score: i32,
    pub evidence: Vec<Evidence>,
    pub conflicts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineDescriptor {
    pub id: EngineId,
    pub confidence: DetectionConfidence,
    pub score: i32,
    pub evidence: Vec<Evidence>,
    pub conflicts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_version: Option<String>,
    pub overridden: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_root: Option<String>,
}
```
- **Carrier Invariant:** Top-level source root is canonicalized to `None`. Any `Some("")` or `Some("/")` is normalized to `None` upon ingestion. Non-empty string contains the canonical relative subpath.
- **Serialization Policy:** `#[serde(default, skip_serializing_if = "Option::is_none")]` allows new consumers to read legacy manifests. `schemas/build-manifest.schema.json` is updated simultaneously.

### 6.2 Detector Trait Contract
To prevent reciprocal recursion and arbitrary selection, `Detector` trait in `core/src/detection/mod.rs` is defined as:
```rust
pub trait Detector: Send + Sync {
    fn id(&self) -> &'static str;
    
    /// Required base probe method.
    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate;

    /// Evaluates all candidate roots. Default implementation wraps probe.
    fn probe_all(&self, source: &GameSourceIndex) -> Vec<DetectionCandidate> {
        vec![self.probe(source)]
    }
}
```
- Unmodified detectors (`RgssDetector`, `RenPyDetector`, `TyranoDetector`, `VnMakerDetector`) keep their required `probe` implementation; `probe_all` defaults to wrapping `probe`.
- `RpgMakerMvDetector` and `RpgMakerMzDetector` implement `probe_all` to evaluate all candidate roots. Their `probe` implementation aggregates results using the fail-closed ambiguity resolution contract (not arbitrary `.max()`).

---

## 7. Security, Lexical Containment & Scoped View

### 7.1 Cross-Platform Lexical Validation
Candidate root paths MUST be validated lexically BEFORE normalization. Cross-platform validation accounts for Linux/WSL environments where `Path::components()` does not treat Windows drive letters or backslashes as root components:
1. **Explicit Lexical Rejections:**
   - Windows drive prefixes: `^[a-zA-Z]:`
   - UNC prefixes: `^//` or `^\\\\`
   - URL schemes: `^[a-zA-Z][a-zA-Z0-9+.-]*://`
   - Null bytes: `\0`
   - Path traversal components: `/../`, `\..\`, leading `../`, trailing `/..`
   - Absolute paths: leading `/` or `\`
2. **Containment Enforcement:**
   - Traversal attempt: Fails with `DiagnosticCode::CNT_TRAVERSAL_REJECTED` (`CNT-103`).
   - Symlink escaping input root: Fails with `DiagnosticCode::ING_SYMLINK_ESCAPE` (`ING-102`) at both ingest and packing stages.
   - Case-folding collisions: If two distinct files map to the same lowercase key in the scoped index, fail closed with `DiagnosticCode::CMP_AUD_CASE_COLLISION` (`CMP-AUD-005`).

### 7.2 Scoped Logical Index (`GameSourceIndex::scoped_to`)
In `core/src/ingest/mod.rs`:
- `scoped_to(&self, subpath: &str) -> Result<GameSourceIndex, IngestError>` validates `subpath` lexically.
- Distinct path identities:
  - `physical_input_root`: Host path passed to CLI.
  - `payload_root`: Validated relative subpath (`None` for top-level).
  - `IndexedFile.real_path(&default_root)`: Unchanged, preserving exact physical file location on disk.
- Re-rooting uses `Path::strip_prefix` on original components (no unsafe string byte slicing).
- **Hashing Semantics:** Preserves original `source_hash` representing physical source provenance (paths + sizes, not a cryptographic byte digest). Introduces `scoped_content_hash` representing the scoped payload index for cache keying.

---

## 8. Downstream Pipeline Integration

### 8.1 Compatibility Scanner (`RpgMakerWebAnalyzer`)
- Evaluates files using the scoped logical index.
- **Payload-Internal Native Binaries:** Unsupported DLLs (`.dll`) or Node addons (`.node`) located inside `payload_root` are diagnosed as `CMP_NATIVE_DLL_DEPENDENCY` (`CMP-102`) or `CMP_NODE_NATIVE_MODULE` (`CMP-103`), failing the build gate.
- **Outer Wrapper Binaries:** Binaries outside `payload_root` are excluded from the package by scoping. The analyzer inspects parent context to log known runners (`nw.dll`, `node.dll`, `ffmpeg.dll`) as `CMP-DESKTOP-EXCLUDED` (`CMP-106`, Info). Unknown outer binaries are excluded from packaging without claiming proof of safety.

### 8.2 Asset Auditor (`RpgMakerWebAssetAuditor`) & Ruleset Bump
- Hardcoded `content_prefix = "www/"` in `engines/rpgmaker_web/src/asset_audit.rs` is removed; the auditor directly accesses re-rooted paths (`index.html`, `data/System.json`).
- `AuditCacheKey` in `core/src/asset_audit/cache.rs` incorporates `content_root`.
- `CURRENT_AUDIT_RULESET_VERSION` is bumped by `+1` relative to the active authoritative version ($12 \rightarrow 13$).

### 8.3 Content Packaging (`GpackPacker`) & Build Manifest
- In `cli/src/main.rs:985-991` and `1053-1065`, delete ad-hoc `path.join("www")` and disk re-ingestion.
- Package directly from the in-memory scoped index via `GpackPacker::pack_from_index_with_options`.
- Android runtime serves `https://game.runtimepack.local/index.html` from `game.pack`, which directly matches the scoped index root.
- `BuildManifest` records `engine.content_root`. `schemas/build-manifest.schema.json` updated accordingly.

### 8.4 CLI Reporting
- **`runtimepack inspect <dir>`:**
  ```text
  Engine:         RPG Maker MZ
  Confidence:     EXACT
  Score:          210
  Content Root:   bin/www
  Status:         COMPATIBLE_WITH_SHIMS
  Runtime Family: RPG Maker Web
  ```
- **`runtimepack explain <dir>`:** Logs all evaluated candidate roots, per-root marker breakdown, and marks non-selected roots as rejected or shadowed.

---

## 9. Acceptance & Regression Test Matrix

All tests run host-side in WSL2 without compiling APKs. Physical byte immutability of test fixtures is asserted before and after execution.

| Test ID | Fixture Description | Expected Outcome | Verification Assertion |
|---|---|---|---|
| `TEST-ROOT-001` | Flat MZ (`index.html`, `js/rmmz_core.js`, `js/rmmz_managers.js`, `data/System.json` at root) | MZ score 210 `EXACT`, `content_root = None` | Matches existing baseline bit-for-bit. |
| `TEST-ROOT-002` | Standard `www/` MZ (`www/index.html`, `www/js/rmmz_core.js`, etc.) | MZ score 210 `EXACT`, `content_root = Some("www")` | Validates standard deployed layout. |
| `TEST-ROOT-003` | Arbitrary nested MZ (`bin/www/index.html`, `bin/www/js/rmmz_core.js`, outer `bin/nw.exe`) | MZ score 210 `EXACT`, `content_root = Some("bin/www")` | Resolves `DET-100` on nested distributions. |
| `TEST-ROOT-004` | Flat MV (`index.html`, `js/rpg_core.js`, `js/rpg_managers.js`, `data/System.json` at root) | MV score 210 `EXACT`, `content_root = None` | Confirms MV parity with MZ. |
| `TEST-ROOT-005` | Arbitrary nested MV (`app/package/index.html`, `app/package/js/rpg_core.js`, etc.) | MV score 210 `EXACT`, `content_root = Some("app/package")` | Proves arbitrary folder name discovery. |
| `TEST-ROOT-006` | Arbitrary multi-level Unicode/case root (`ゲーム/Data_MZ/js/rmmz_core.js`, etc.) | MZ score 210 `EXACT`, `content_root = Some("ゲーム/Data_MZ")` | Proves Unicode path handling. |
| `TEST-ROOT-NEG-001` | Split-Marker Negative: `<root>/index.html`, but `bin/www/js/rmmz_core.js` | Root score 0; `bin/www` score 100 `MEDIUM`. Overall: 100 `MEDIUM` | Proves zero cross-root score merging. |
| `TEST-ROOT-NEG-002` | Competing Identical MZ Payloads: `payload_a/www` (MZ 210) and `payload_b/www` (MZ 210) | Resolution fails closed: `DET-110` (`DET_AMBIGUOUS`) | Proves no arbitrary "first root wins". |
| `TEST-ROOT-NEG-003` | Competing Payloads with `--engine rpgmaker-mz` on identical MZ roots | Fails closed with `DET-110` listing both roots | Proves forced engine cannot guess root. |
| `TEST-ROOT-NEG-004` | Mixed Engine Competing Payloads: `p1/www` (MZ 210) and `p2/www` (MV 210) | Without `--engine`: `DET-110`. With `--engine rpgmaker-mz`: selects `p1/www`. | Proves engine filter disambiguates mixed types. |
| `TEST-ROOT-NEG-005` | Same Root MV/MZ Conflict: candidate root contains both `rpg_core.js` and `rmmz_core.js` | Fails closed with candidate conflict `DET-110` | Proves intra-root conflict detection. |
| `TEST-ROOT-NEG-006` | Multiple Medium Candidates ($80 \le \text{score} < 140$) with `--accept-detection` | Fails closed with `DET-110` | Proves `--accept-detection` cannot guess root. |
| `TEST-ROOT-SEC-001` | Traversal candidate: `"../../etc"`, `"C:\Windows"`, `"\\server\share"`, `"\0"` | Rejected lexically: `CNT_TRAVERSAL_REJECTED` (`CNT-103`) | Proves component traversal rejection. |
| `TEST-ROOT-SEC-002` | Escaping symlink or junction in candidate root | Rejected: `ING_SYMLINK_ESCAPE` (`ING-102`) | Proves containment on links/junctions. |
| `TEST-ROOT-SEC-003` | Case collision in scoped index (`Index.html` vs `index.html`) | Fails closed: `CMP_AUD_CASE_COLLISION` (`CMP-AUD-005`) | Proves no silent case overwrite. |
| `TEST-ROOT-PKG-001` | Packaging nested payload `bin/www/` into `game.pack` | `index.html` at GPACK root; outer `nw.exe`/`nw.dll` excluded | Proves container re-rooting. |
| `TEST-ROOT-PKG-002` | Payload-internal native DLL (`bin/www/plugin.dll`) | Emits `CMP_NATIVE_DLL_DEPENDENCY` (`CMP-102`), fails gate | Proves internal DLLs are not hidden. |
| `TEST-ROOT-REG-001` | RPG Maker XP / VX / VX Ace minimal fixtures | XP, VX, VX Ace scores and confidence remain 100% bit-exact | Zero regression on RGSS family. |
| `TEST-ROOT-REG-002` | Ren'Py, Tyrano, and VNM minimal fixtures | Scores and confidence remain 100% bit-exact | Zero regression on other engines. |

---

## 10. Workspace Decisions & Implementation Prerequisites

1. **Workspace Reconciliation:** Root `.git` is physically on `baseline/vnm-v4`. Implementation should proceed in a clean worktree branching from baseline, with awareness of unmerged profile catalog changes in `.worktrees/codebase-generalization`.
2. **Environment Compliance:** All future implementation, builds, and test runs MUST execute via WSL2 (`wsl -d Ubuntu-24.04 -- <command>`). Host Windows execution is prohibited.

*End of Design Specification.*
