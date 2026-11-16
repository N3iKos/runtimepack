# Deep Asset Audit Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the engine-neutral audit contract, dependency graph, CLI command, build policy gates, and authoritative static report artifact.

**Architecture:** `core::asset_audit` owns common types, orchestration, deterministic reduction, and policy. Engine modules implement `AssetAuditor`; CLI calls the registry for `audit` and automatically before `build`.

**Tech Stack:** Rust, serde/serde_json, clap, JSON Schema, Cargo integration tests.

**Spec:** `Docs/superpowers/specs/2026-09-24-multi-engine-deep-asset-audit-design.md`

## Global Constraints

- Static audit never executes game scripts, executables, Python, NW.js, or native libraries.
- `PASS/WARNING` permit build; `FAIL/INDETERMINATE` block unless their dedicated override is accepted.
- Audit references are required by default.
- Stable status categories replace percentages.
- Input source is immutable.

---

### Task 1: Audit Domain and Dependency Graph

**Files:**
- Create: `core/src/asset_audit/mod.rs`
- Create: `core/src/asset_audit/graph.rs`
- Modify: `core/src/lib.rs`
- Create: `schemas/asset-audit-report.schema.json`
- Test: `core/src/asset_audit/mod.rs`

**Interfaces:**
- Produces: `AuditMode::{Standard,Deep}`, `AuditOutcome::{Pass,Warning,Fail,Indeterminate}`, `EvidenceStage`, `ReferenceRequiredness`, `AuditFinding`, `AssetNode`, `ReferenceEdge`, `DependencyGraph`, `AuditCompleteness`, `AssetAuditReport`, `AssetAuditOptions`.
- Later tasks consume all names exactly as declared here.

- [ ] **Step 1: Write serialization and ordering tests**

Add unit tests that construct equivalent findings/nodes/edges in different insertion orders and assert identical `canonicalize()` output and JSON. Assert evidence cannot skip from `ReferenceFound` directly to `DecodeSuccess` without intermediate stage results.

- [ ] **Step 2: Run the focused test and verify failure**

Run:

```powershell
cargo test -p runtimepack-core asset_audit
```

Expected: FAIL because `core::asset_audit` does not exist.

- [ ] **Step 3: Implement the minimal domain types**

Required public signatures:

```rust
pub enum AuditMode { Standard, Deep }
pub enum AuditOutcome { Pass, Warning, Fail, Indeterminate }
pub enum EvidenceStage {
    ReferenceFound,
    PathResolved,
    FileExists,
    FileReadable,
    IntegrityVerified,
    DecodeSuccess,
    EngineLoadSuccess,
    RuntimeUseSuccess,
}
pub enum ReferenceRequiredness { Required, Optional }

pub struct AssetAuditOptions {
    pub mode: AuditMode,
    pub allow_network: bool,
    pub limits: AuditLimits,
}

pub struct AssetAuditReport {
    pub schema_version: u32,
    pub engine: EngineId,
    pub outcome: AuditOutcome,
    pub completeness: AuditCompleteness,
    pub graph: DependencyGraph,
    pub findings: Vec<AuditFinding>,
    pub runtime_validation: RuntimeValidationState,
}

impl AssetAuditReport {
    pub fn canonicalize(&mut self);
    pub fn recompute_outcome(&mut self);
}
```

`recompute_outcome` must treat a required blocking finding as `Fail`, incomplete required scope as `Indeterminate`, warning-only findings as `Warning`, and otherwise `Pass`. Orphan findings never escalate outcome.

- [ ] **Step 4: Define the JSON schema**

The schema must require schema version, engine, mode, outcome, completeness, statistics, graph, findings, overrides, and `runtime_validation`. Restrict all enums to the Rust values and reject unknown required fields.

- [ ] **Step 5: Run focused tests**

Run:

```powershell
cargo test -p runtimepack-core asset_audit
```

Expected: PASS.

- [ ] **Step 6: Commit**

```powershell
git add core/src/lib.rs core/src/asset_audit schemas/asset-audit-report.schema.json
git commit -m "feat: add asset audit domain contract"
```

---

### Task 2: Auditor Registry and Incomplete Adapter

**Files:**
- Modify: `core/src/asset_audit/mod.rs`
- Test: `core/src/asset_audit/mod.rs`

**Interfaces:**
- Consumes: Task 1 audit types.
- Produces: `AssetAuditor` and `AssetAuditorRegistry`.

- [ ] **Step 1: Write registry tests**

Test that all auditors run only for the selected engine, duplicate adapter IDs are rejected, absent adapters produce `Indeterminate` with stable code `CMP-AUD-001`, and adapter errors remain findings rather than panics.

- [ ] **Step 2: Verify failure**

```powershell
cargo test -p runtimepack-core asset_audit::tests::registry
```

Expected: FAIL because the registry is undefined.

- [ ] **Step 3: Implement interfaces**

```rust
pub trait AssetAuditor: Send + Sync {
    fn id(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn supports(&self, engine: EngineId) -> bool;
    fn audit(
        &self,
        source: &GameSourceIndex,
        engine: &EngineDescriptor,
        options: &AssetAuditOptions,
    ) -> AssetAuditReport;
}

pub struct AssetAuditorRegistry {
    auditors: Vec<Box<dyn AssetAuditor>>,
    registered_ids: BTreeSet<&'static str>,
}

impl AssetAuditorRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, auditor: Box<dyn AssetAuditor>) -> Result<(), Diagnostic>;
    pub fn audit(
        &self,
        source: &GameSourceIndex,
        engine: &EngineDescriptor,
        options: &AssetAuditOptions,
    ) -> AssetAuditReport;
}
```

No `match EngineId` is permitted in the registry.

- [ ] **Step 4: Run tests**

```powershell
cargo test -p runtimepack-core asset_audit
```

Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add core/src/asset_audit/mod.rs
git commit -m "feat: add asset auditor registry"
```

---

### Task 3: Stable Audit Diagnostics

**Files:**
- Modify: `core/src/diagnostics/mod.rs`
- Modify: `core/src/asset_audit/mod.rs`
- Test: `core/src/diagnostics/mod.rs`

**Interfaces:**
- Produces stable codes used by all adapters and policy.

- [ ] **Step 1: Write code uniqueness and severity tests**

Cover:

```text
CMP-AUD-001 AUDITOR_UNAVAILABLE
CMP-AUD-002 REQUIRED_REFERENCE_MISSING
CMP-AUD-003 OPTIONAL_REFERENCE_MISSING
CMP-AUD-004 CASE_MISMATCH
CMP-AUD-005 CASE_COLLISION
CMP-AUD-006 DYNAMIC_REFERENCE_UNRESOLVED
CMP-AUD-007 AUDIT_LIMIT_REACHED
CMP-AUD-008 MALFORMED_RESOURCE
CMP-AUD-009 UNSAFE_PATH
CMP-AUD-010 REPORT_INVALID
CMP-AUD-011 EXTERNAL_RESOURCE_UNVERIFIED
```

Required missing and case collision are errors; optional missing/case mismatch are warnings; orphan findings are informational; unresolved required dynamic/network references produce indeterminate disposition.

- [ ] **Step 2: Run and observe failure**

```powershell
cargo test -p runtimepack-core diagnostics
```

- [ ] **Step 3: Add constants and remediation mappings**

Use existing diagnostic domains and structures; do not add arbitrary percentages or a new undocumented top-level domain.

- [ ] **Step 4: Run tests**

```powershell
cargo test -p runtimepack-core diagnostics asset_audit
```

Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add core/src/diagnostics/mod.rs core/src/asset_audit/mod.rs
git commit -m "feat: define asset audit diagnostics"
```

---

### Task 4: CLI `runtimepack audit`

**Files:**
- Modify: `cli/src/main.rs`
- Create: `tests/integration/asset_audit_cli_test.rs`
- Test: `tests/integration/asset_audit_cli_test.rs`

**Interfaces:**
- Consumes: `AssetAuditorRegistry`, `AssetAuditOptions`.
- Produces CLI `audit`, `--mode standard|deep`, `--allow-network-audit`, `--output`.

- [ ] **Step 1: Write CLI integration tests**

Tests must assert:

```text
runtimepack audit <fixture>
  -> detects engine
  -> does not mutate source
  -> writes asset-audit-report.json
  -> defaults to standard and no network

runtimepack audit <fixture> --mode deep
  -> report mode DEEP
```

Also verify invalid mode returns stable diagnostic and no report.

- [ ] **Step 2: Run test and verify failure**

```powershell
cargo test --test asset_audit_cli_test
```

Expected: FAIL because the command is missing.

- [ ] **Step 3: Implement command parsing and registry construction**

Add `AuditArgs`, `Commands::Audit`, `cmd_audit`, and `create_default_asset_auditor_registry`. Until adapters are implemented, each registered placeholder adapter returns `Indeterminate/AUDITOR_UNAVAILABLE`; never return PASS.

- [ ] **Step 4: Emit authoritative JSON atomically**

Write to a temporary sibling file, flush, then rename. Output paths must be outside the source unless explicitly equal to a user-selected external path; reject output within the source root.

- [ ] **Step 5: Run focused tests**

```powershell
cargo test --test asset_audit_cli_test
```

Expected: PASS.

- [ ] **Step 6: Commit**

```powershell
git add cli/src/main.rs tests/integration/asset_audit_cli_test.rs
git commit -m "feat: add runtimepack audit command"
```

---

### Task 5: Build Audit Policy Gate

**Files:**
- Create: `core/src/build_plan/audit_policy.rs`
- Modify: `core/src/build_plan/mod.rs`
- Modify: `cli/src/main.rs`
- Modify: `schemas/build-manifest.schema.json`
- Modify: `tests/integration/build_test.rs`

**Interfaces:**
- Produces: `AuditPolicyDecision`, `AuditOverrideRecord`, `evaluate_audit_policy`.

- [ ] **Step 1: Add table-driven policy tests**

Required matrix:

| Outcome | No override | Fail override | Unknown override |
|---|---|---|---|
| PASS | allow | allow | allow |
| WARNING | allow | allow | allow |
| FAIL | block | allow | block |
| INDETERMINATE | block | block | allow |

Add tests proving neither override bypasses security, container corruption, Tyrano legal gate, ambiguous engine, unresolved runtime, or unsupported native dependency.

- [ ] **Step 2: Run tests and observe failure**

```powershell
cargo test --test build_test audit_policy
```

- [ ] **Step 3: Implement pure policy evaluator**

```rust
pub fn evaluate_audit_policy(
    report: &AssetAuditReport,
    accept_fail: bool,
    accept_unknown: bool,
    hard_blockers: &[Diagnostic],
) -> AuditPolicyDecision;
```

The evaluator performs no filesystem or build side effects.

- [ ] **Step 4: Add CLI flags and pre-build ordering**

Add:

```text
--audit-mode standard|deep
--accept-audit-fail
--accept-audit-unknown
```

Run audit and evaluate policy before content packing, Android generation, Gradle, or output artifact creation.

- [ ] **Step 5: Record overrides**

Add audit report hash/path, outcome, mode, and override records to build manifest/provenance. Override records include flag, original outcome, accepted finding IDs, and timestamp-independent rationale.

- [ ] **Step 6: Run tests**

```powershell
cargo test --test build_test audit_policy
cargo test --test asset_audit_cli_test
```

Expected: PASS and rejected builds leave no side-effect artifacts.

- [ ] **Step 7: Commit**

```powershell
git add core/src/build_plan cli/src/main.rs schemas/build-manifest.schema.json tests/integration/build_test.rs
git commit -m "feat: gate builds on static asset audit"
```

---

### Task 6: Foundation Documentation and Verification

**Files:**
- Modify: `Docs/COMPATIBILITY.md`
- Modify: `Docs/CONFIG_AND_SCHEMAS.md`
- Modify: `Docs/TEST_PLAN.md`
- Modify: `Docs/IMPLEMENTATION_PLAN.md`

- [ ] **Step 1: Document authoritative terminology**

Define stages, outcomes, standard/deep, override restrictions, static/runtime separation, and source immutability. Explicitly state that PASS means only declared static scope passed.

- [ ] **Step 2: Run formatting, lint, type checks, and tests**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: all pass.

- [ ] **Step 3: Commit**

```powershell
git add Docs/COMPATIBILITY.md Docs/CONFIG_AND_SCHEMAS.md Docs/TEST_PLAN.md Docs/IMPLEMENTATION_PLAN.md
git commit -m "docs: define deep asset audit policy"
```
