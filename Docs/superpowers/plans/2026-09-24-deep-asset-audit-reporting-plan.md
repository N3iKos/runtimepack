# Deep Asset Audit Reporting Implementation Plan (Condensed)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic canonical JSON reporting, content-sensitive audit caching, standalone offline HTML viewer, manifest/provenance links, bounded network auditing, and orphan analysis.

**Architecture:** Canonical JSON is rendered from `AssetAuditReport` with deterministic key/array sorting and sanitized paths. Caching uses SHA-256 of game content and engine metadata. The HTML viewer is a self-contained offline single-file app. Orphan analysis and bounded network auditing enrich the graph without breaking fail-closed policy.

**Tech Stack:** Rust, serde_json, SHA-256, JSON Schema, Cargo integration tests.

**Spec:** `Docs/superpowers/specs/2026-09-24-multi-engine-deep-asset-audit-design.md`

## Global Constraints

- Never execute game scripts or binaries.
- Output paths must not leak host/device absolute paths (sanitized logical paths only).
- Output must never write into immutable source directories.
- Caching hit vs miss must produce byte-identical canonical JSON.
- Orphan findings are strictly `INFO | POSSIBLY_UNUSED` and never alter build policy outcomes.
- No external CDN/network dependencies in offline HTML viewer.

---

### Task 1: Canonical Report Writer & Content-Sensitive Audit Cache

**Files:**
- Create: `core/src/asset_audit/report.rs`
- Create: `core/src/asset_audit/cache.rs`
- Modify: `core/src/asset_audit/mod.rs`
- Modify: `cli/src/main.rs`
- Create: `tests/integration/asset_audit_report_test.rs`

**Interfaces:**
- Produces:
  - `CanonicalAuditArtifacts`, `render_canonical_artifacts(&AssetAuditReport) -> Result<CanonicalAuditArtifacts, Diagnostic>`
  - `write_audit_artifacts(output_dir: &Path, artifacts: &CanonicalAuditArtifacts) -> Result<(), Diagnostic>`
  - `AuditCacheKey`, `AuditCache::get(...)`, `AuditCache::put(...)`

- [ ] **Step 1: Write integration tests** in `tests/integration/asset_audit_report_test.rs`:
  - Verify canonical JSON output is deterministic regardless of in-memory insertion order.
  - Verify absolute paths are sanitized to logical paths.
  - Verify cache key incorporates content hash, engine descriptor, mode, limits, and ruleset version.
  - Verify cache hit produces identical JSON to fresh audit.
  - Verify corrupt cache entries fail closed to fresh audit.
  - Verify atomic file writing uses unique temp files and preserves existing destination on write error.
- [ ] **Step 2: Run test to verify failure:** `cargo test --test asset_audit_report_test`.
- [ ] **Step 3: Implement `core/src/asset_audit/report.rs`**:
  - `render_canonical_artifacts`: canonicalize report, serialize pretty JSON with trailing newline, calculate SHA-256, sanitize paths.
  - `write_audit_artifacts`: atomic write `asset-audit-report.json` and `asset-dependency-graph.json`.
- [ ] **Step 4: Implement `core/src/asset_audit/cache.rs`**:
  - Compute `AuditCacheKey` from `GameSourceIndex.source_hash`, `EngineDescriptor`, `AuditMode`, `AuditLimits`, and toolchain versions.
  - Read/write cache entries atomically under `.runtimepack/cache/audit/` or designated cache dir outside game source.
- [ ] **Step 5: Wire cache and report writer into `cli/src/main.rs` & `cli/src/audit_command.rs`**:
  - CLI `runtimepack audit` checks cache before running scan, writes cache on completion.
  - `runtimepack build` uses canonical writer for output `asset-audit-report.json`.
- [ ] **Step 6: Run tests to verify pass:** `cargo test --test asset_audit_report_test` and `cargo test --workspace`.
- [ ] **Step 7: Commit changes.**

```powershell
git add core/src/asset_audit/report.rs core/src/asset_audit/cache.rs core/src/asset_audit/mod.rs cli/src/main.rs cli/src/audit_command.rs tests/integration/asset_audit_report_test.rs
git commit -m "feat: add canonical report writer and content-sensitive cache"
```

---

### Task 2: Standalone Offline HTML Viewer & Build Manifest/Provenance Integration

**Files:**
- Create: `core/src/asset_audit/html.rs`
- Modify: `core/src/asset_audit/mod.rs`
- Modify: `core/src/build_plan/mod.rs`
- Modify: `core/src/build_plan/generator.rs`
- Modify: `cli/src/main.rs`
- Modify: `cli/src/audit_command.rs`
- Modify: `schemas/build-manifest.schema.json`
- Modify: `tests/integration/asset_audit_report_test.rs`
- Modify: `tests/integration/build_test.rs`

**Interfaces:**
- Produces:
  - `render_audit_html(&CanonicalAuditArtifacts) -> Result<Vec<u8>, Diagnostic>`
  - `write_audit_artifacts` emits `asset-audit-report.html` alongside JSON.
  - Build manifest records `manifest.audit.report_sha256` and matching provenance.

- [ ] **Step 1: Write integration tests**:
  - Test HTML generator creates completely self-contained HTML (zero `http://`, `https://`, external CDN links).
  - Test game-controlled text, event names, and error messages are HTML-escaped.
  - Test HTML renders stat counters, status badges (`PASS`/`WARNING`/`FAIL`/`INDETERMINATE`), stage filter, and findings table matching JSON exactly.
  - Test `runtimepack build` embeds `asset-audit-report.json` hash into `build-manifest.json` and `manifest.provenance.audit`.
- [ ] **Step 2: Run test to verify failure:** `cargo test --test asset_audit_report_test`.
- [ ] **Step 3: Implement `core/src/asset_audit/html.rs`**:
  - Embedded CSS and vanilla JS (responsive, dark mode matching RuntimePack theme).
  - Embed the exact canonical JSON inside a `<script type="application/json" id="audit-data">` element.
  - Client-side filtering by severity, category, stage, and text search.
  - Clean collapsible views for map/event/database provenance.
- [ ] **Step 4: Update `write_audit_artifacts` and CLI**:
  - Automatically emit `asset-audit-report.html` in target directory on `runtimepack audit` and `runtimepack build`.
  - Add `--no-html` CLI flag if user wants JSON only.
- [ ] **Step 5: Connect build manifest and provenance**:
  - Ensure `build_plan::generator` and `cmd_build` write matching SHA-256 for `asset-audit-report.json`.
- [ ] **Step 6: Run tests to verify pass:** `cargo test --test asset_audit_report_test` and `cargo test --test integration_build`.
- [ ] **Step 7: Commit changes.**

```powershell
git add core/src/asset_audit/html.rs core/src/asset_audit/mod.rs core/src/build_plan cli/src schemas/build-manifest.schema.json tests/integration/asset_audit_report_test.rs tests/integration/build_test.rs
git commit -m "feat: add standalone offline HTML audit viewer and manifest provenance"
```

---

### Task 3: Orphan Analysis, Bounded Network Audit & Verification Docs

**Files:**
- Create: `core/src/asset_audit/orphans.rs`
- Create: `core/src/asset_audit/network.rs`
- Modify: `core/src/asset_audit/mod.rs`
- Modify: `cli/src/main.rs`
- Modify: `cli/src/audit_command.rs`
- Modify: `Docs/COMPATIBILITY.md`
- Modify: `Docs/CONFIG_AND_SCHEMAS.md`
- Modify: `tests/integration/asset_audit_report_test.rs`

**Interfaces:**
- Produces:
  - `classify_possible_orphans(&GameSourceIndex, &DependencyGraph, &AuditCoverage) -> Vec<AuditFinding>`
  - `audit_network_reference(url: &str, options: &AssetAuditOptions) -> NetworkAuditResult`
  - Canonical docs update.

- [ ] **Step 1: Write integration tests**:
  - Unreachable files in source become `INFO | POSSIBLY_UNUSED` (diagnostic `CMP-AUD-013` or similar informational code).
  - Dynamic expressions or incomplete scope properly mark orphan status as `POSSIBLY_UNUSED`, never altering overall audit outcome.
  - External HTTP(S) URLs tested with local mock server: when `--allow-network-audit` is off -> `INDETERMINATE`. When on -> bounded HEAD/GET check with size/timeout limit; fails gracefully on 404/network error without crashing.
- [ ] **Step 2: Run test to verify failure:** `cargo test --test asset_audit_report_test`.
- [ ] **Step 3: Implement `core/src/asset_audit/orphans.rs`**:
  - Collect all files present in `GameSourceIndex`.
  - Traverse `DependencyGraph` from entrypoints/roots.
  - Unreferenced assets categorized as `POSSIBLY_UNUSED`. Never delete or touch source files.
- [ ] **Step 4: Implement `core/src/asset_audit/network.rs`**:
  - Parse URL, enforce HTTP/HTTPS schemes only.
  - If `--allow-network-audit`: perform bounded request with timeout (max 5s) and redirect cap (max 3). Check MIME and status code. Never execute downloaded payload.
- [ ] **Step 5: Update documentation**:
  - Update `Docs/COMPATIBILITY.md` and `Docs/CONFIG_AND_SCHEMAS.md` with orphan semantics, cache rules, and HTML viewer description.
- [ ] **Step 6: Run full workspace tests and formatting:**
  - `cargo fmt --all -- --check`
  - `cargo test --workspace`
- [ ] **Step 7: Commit changes.**

```powershell
git add core/src/asset_audit/orphans.rs core/src/asset_audit/network.rs core/src/asset_audit/mod.rs cli/src Docs/COMPATIBILITY.md Docs/CONFIG_AND_SCHEMAS.md tests/integration/asset_audit_report_test.rs
git commit -m "feat: add orphan analysis, bounded network audit, and documentation"
```
