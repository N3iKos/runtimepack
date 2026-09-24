# Deep Asset Audit Reporting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic report persistence, content-sensitive caching, HTML viewing, manifest/provenance integration, bounded network audit, and orphan reporting.

**Architecture:** Canonical JSON is generated from `AssetAuditReport`; cache and viewers consume that canonical representation. HTML never computes outcome. Network evidence and orphan analysis extend the graph without weakening default fail-closed behavior.

**Tech Stack:** Rust, serde_json, SHA-256, existing HTTP capability if present, JSON Schema.

**Spec:** `Docs/superpowers/specs/2026-09-24-multi-engine-deep-asset-audit-design.md`

## Global Constraints

- Requires Foundation Plan; full engine coverage requires Adapter Plan.
- No new HTTP dependency unless license/provenance/version checks are completed.
- No absolute host/device paths in output.
- Cache hit and miss must produce identical authoritative JSON.

---

### Task 1: Canonical Report Writer

**Files:**
- Create: `core/src/asset_audit/report.rs`
- Modify: `core/src/asset_audit/mod.rs`
- Create: `tests/integration/asset_audit_report_test.rs`

**Interfaces:**
- Produces: `CanonicalAuditArtifacts`, `write_audit_artifacts`.

- [ ] **Step 1: Write golden report tests** that randomize input insertion order, redact absolute paths, reject malformed status/stage sequences, and compare byte-identical JSON.
- [ ] **Step 2: Run `cargo test --test asset_audit_report_test` and verify failure.**
- [ ] **Step 3: Implement:**

```rust
pub struct CanonicalAuditArtifacts {
    pub report_json: Vec<u8>,
    pub graph_json: Vec<u8>,
    pub report_sha256: String,
    pub graph_sha256: String,
}

pub fn render_canonical_artifacts(report: &AssetAuditReport)
    -> Result<CanonicalAuditArtifacts, Diagnostic>;

pub fn write_audit_artifacts(
    output_dir: &Path,
    artifacts: &CanonicalAuditArtifacts,
) -> Result<(), Diagnostic>;
```

Use atomic sibling temp files and deterministic formatting. Operational timings may be included in a non-canonical section but excluded from semantic hashes.

- [ ] **Step 4: Run tests and commit.**

```powershell
git add core/src/asset_audit tests/integration/asset_audit_report_test.rs
git commit -m "feat: write canonical asset audit reports"
```

---

### Task 2: Content-Sensitive Audit Cache

**Files:**
- Create: `core/src/asset_audit/cache.rs`
- Modify: `core/src/asset_audit/mod.rs`
- Modify: `cli/src/main.rs`
- Test: `tests/integration/asset_audit_report_test.rs`

**Interfaces:**
- Produces: `AuditCacheKey`, `AuditCache`.

- [ ] **Step 1: Write tests** for cache hit/miss equivalence, same-size content mutation, engine/profile/mode/policy/limit/parser/schema/ruleset version invalidation, corrupt entry recovery, and source-external cache location.
- [ ] **Step 2: Verify failure.**

```powershell
cargo test --test asset_audit_report_test cache
```

- [ ] **Step 3: Implement cache key** from selected content hashes plus all approved dimensions; do not rely solely on existing path/size source hash.
- [ ] **Step 4: Implement atomic cache read/write** under RuntimePack cache/output state, never inside the source.
- [ ] **Step 5: Integrate CLI cache use and emit explicit cache provenance.**
- [ ] **Step 6: Run tests and commit.**

```powershell
git add core/src/asset_audit cli/src/main.rs tests/integration/asset_audit_report_test.rs
git commit -m "feat: cache deterministic asset audits"
```

---

### Task 3: Offline HTML Viewer

**Files:**
- Create: `core/src/asset_audit/html.rs`
- Modify: `core/src/asset_audit/mod.rs`
- Test: `tests/integration/asset_audit_report_test.rs`

**Interfaces:**
- Produces: `render_audit_html(&CanonicalAuditArtifacts) -> Result<Vec<u8>, Diagnostic>`.

- [ ] **Step 1: Write viewer tests** asserting escaped game-controlled strings, no external URLs/scripts/styles/fonts, no independent outcome computation, filters for stage/code/severity/source, and cache hit/miss identity.
- [ ] **Step 2: Verify failure.**
- [ ] **Step 3: Implement self-contained HTML** embedding authoritative report JSON and rendering its provided outcome/stats only.
- [ ] **Step 4: Run tests and commit.**

```powershell
git add core/src/asset_audit tests/integration/asset_audit_report_test.rs
git commit -m "feat: add offline asset audit viewer"
```

---

### Task 4: Manifest, Compatibility, and Provenance Links

**Files:**
- Modify: `core/src/build_plan/mod.rs`
- Modify: `core/src/build_plan/generator.rs`
- Modify: `schemas/build-manifest.schema.json`
- Modify: `schemas/compatibility-report.schema.json`
- Modify: `tests/integration/build_test.rs`

**Interfaces:**
- Produces manifest `asset_audit` metadata containing mode, outcome, schema version, report path/hash, graph path/hash, cache provenance, override records, and runtime status.

- [ ] **Step 1: Write build snapshot tests** for PASS, WARNING, accepted FAIL, accepted INDETERMINATE, and runtime `NOT_RUN`.
- [ ] **Step 2: Verify failure.**
- [ ] **Step 3: Add optional schema/model fields** without embedding the full graph in compatibility report.
- [ ] **Step 4: Emit artifacts before Android project generation** and copy immutable report metadata into generated assets only when runtime viewer needs it.
- [ ] **Step 5: Run tests and commit.**

```powershell
git add core/src/build_plan schemas tests/integration/build_test.rs
git commit -m "feat: link audit reports to build provenance"
```

---

### Task 5: Bounded Network Audit

**Files:**
- Create: `core/src/asset_audit/network.rs`
- Modify: `core/src/asset_audit/mod.rs`
- Modify: `cli/src/main.rs`
- Test: `tests/integration/asset_audit_report_test.rs`
- Modify: `Docs/REFERENCES.md` if an HTTP implementation dependency is introduced

**Interfaces:**
- Produces: `NetworkAuditPolicy` and `NetworkEvidence`.

- [ ] **Step 1: Write tests with a local deterministic HTTP fixture** for disabled-by-default, timeout, redirect limit, maximum body/header bytes, MIME mismatch, unavailable resource, and no content execution.
- [ ] **Step 2: Verify failure.**
- [ ] **Step 3: Implement policy** using an already pinned repository HTTP facility; if none exists, stop with dependency/license review before adding one.
- [ ] **Step 4: Keep required external resources `INDETERMINATE` when network audit is disabled.** Successful bounded reachability proves only network read/metadata, not engine decode/use.
- [ ] **Step 5: Run tests and commit.**

```powershell
git add core/src/asset_audit cli/src/main.rs tests/integration/asset_audit_report_test.rs Docs/REFERENCES.md
git commit -m "feat: add opt-in bounded network audit"
```

---

### Task 6: Orphan Analysis

**Files:**
- Create: `core/src/asset_audit/orphans.rs`
- Modify: `core/src/asset_audit/mod.rs`
- Test: `tests/integration/asset_audit_report_test.rs`

**Interfaces:**
- Produces: `classify_possible_orphans(&DependencyGraph, &AuditCoverage)`.

- [ ] **Step 1: Write tests** proving unreachable assets become `INFO | POSSIBLY_UNUSED`, dynamic/unsupported coverage prevents definitive unused claims, generated/runtime files are excluded, and outcome never escalates.
- [ ] **Step 2: Verify failure.**
- [ ] **Step 3: Implement graph reachability from declared roots.** Never delete or modify assets.
- [ ] **Step 4: Run tests and commit.**

```powershell
git add core/src/asset_audit tests/integration/asset_audit_report_test.rs
git commit -m "feat: report possible orphan assets"
```

---

### Task 7: Reporting Verification and Documentation

**Files:**
- Modify: `Docs/COMPATIBILITY.md`
- Modify: `Docs/CONFIG_AND_SCHEMAS.md`
- Modify: `Docs/TEST_PLAN.md`

- [ ] **Step 1: Document canonical/non-canonical fields, cache key, sanitization, HTML authority, network limits, and orphan semantics.**
- [ ] **Step 2: Run verification.**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 3: Commit.**

```powershell
git add Docs/COMPATIBILITY.md Docs/CONFIG_AND_SCHEMAS.md Docs/TEST_PLAN.md
git commit -m "docs: define audit reporting and cache"
```
