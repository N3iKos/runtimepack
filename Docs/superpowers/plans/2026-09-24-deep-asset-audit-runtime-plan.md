# Deep Asset Audit Runtime Instrumentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Capture actual content resolution/read/decode/load/fallback evidence and run optional bounded sampled or exhaustive runtime probes.

**Architecture:** A versioned runtime event protocol is shared across backends. RGSS Ruby and Web/Kotlin loaders emit events into `AssetAuditService`; optional probes consume the static graph and produce a separate runtime report. Ren'Py remains unavailable until its real backend exposes this contract.

**Tech Stack:** Kotlin, Android app-private storage, Ruby, JavaScript, existing VFS/GPACK APIs, JUnit/Node/Ruby tests.

**Spec:** `Docs/superpowers/specs/2026-09-24-multi-engine-deep-asset-audit-design.md`

## Global Constraints

- Requires Foundation and Reporting plans.
- Runtime evidence never erases static findings.
- Runtime probe is optional after build and defaults to `sampled`.
- Required fallback/suppression is FAIL; optional fallback/suppression is WARNING.
- Do not claim Ren'Py runtime validation before a real pinned backend exists.

---

### Task 1: Runtime Audit Event Protocol

**Files:**
- Create: `android/shell/app/src/main/java/io/runtimepack/host/AssetAuditModels.kt`
- Create: `android/shell/app/src/main/java/io/runtimepack/host/AssetAuditService.kt`
- Create: `android/shell/app/src/test/java/io/runtimepack/host/AssetAuditServiceTest.kt`
- Modify: `Docs/RUNTIME_CONTRACT.md`

**Interfaces:**
- Produces: `RuntimeAuditEvent`, `RuntimeAuditStage`, `RuntimeAuditOutcome`, `AssetAuditService`.

- [ ] **Step 1: Write Kotlin tests** for ordered sequence IDs, bounded storage, malformed event rejection, atomic report persistence, cancellation, process restart recovery, and required/optional fallback outcome reduction.
- [ ] **Step 2: Run and verify failure.**

```powershell
.\gradlew.bat testDebugUnitTest --tests io.runtimepack.host.AssetAuditServiceTest
```

Run from `android/shell`.

- [ ] **Step 3: Implement models and service** with app-private `runtime-state/audit/` storage. Use logical paths only and sanitize payloads.
- [ ] **Step 4: Define optional capability `diagnostics.asset_audit.v1`** without breaking `RuntimeBackend` API v1.
- [ ] **Step 5: Run tests and commit.**

```powershell
git add android/shell/app/src/main/java/io/runtimepack/host/AssetAudit* android/shell/app/src/test/java/io/runtimepack/host/AssetAuditServiceTest.kt Docs/RUNTIME_CONTRACT.md
git commit -m "feat: add runtime asset audit event service"
```

---

### Task 2: Native Content/VFS Evidence

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/GpackStreamReader.kt`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`
- Modify: `android/shell/app/src/test/java/io/runtimepack/host/GpackStreamReaderTest.kt`
- Modify: `android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt`

**Interfaces:**
- Produces metadata lookup and structured read/decrypt/MIME outcomes.

- [ ] **Step 1: Write tests** for exact lookup, folded lookup, collision, absent entry, corrupt bounds, unreadable bytes, encrypted header/key failure, binary charset, and fallback event emission.
- [ ] **Step 2: Verify failure with focused Gradle tests.**
- [ ] **Step 3: Add bounded metadata query APIs** that do not allocate full entries merely for existence checks.
- [ ] **Step 4: Emit runtime events** for selected target, read, integrity/decryption, response delivery, and fallback. Never call transparent PNG/empty audio a successful original load.
- [ ] **Step 5: Run tests and commit.**

```powershell
git add android/shell/app/src/main/java/io/runtimepack/host/GpackStreamReader.kt android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt android/shell/app/src/test/java/io/runtimepack/host
git commit -m "feat: instrument content resolution and reads"
```

---

### Task 3: RGSS Loader Instrumentation

**Files:**
- Create: `android/shell/app/src/main/assets/runtimepack_asset_audit.rb`
- Modify: `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
- Create: `android/shell/app/src/test/ruby/asset_audit_test.rb`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`
- Modify: `core/src/build_plan/generator.rs`

**Interfaces:**
- Produces structured events from `Bitmap`, `Cache`, `Audio`, and case/path resolution.

- [ ] **Step 1: Write Ruby tests** for exact bitmap load, missing bitmap fallback, corrupt/decode exception fallback, cached placeholder, BGM/BGS/ME/SE suppression, case correction, and preservation of original exception/stage/source.
- [ ] **Step 2: Run `ruby android/shell/app/src/test/ruby/asset_audit_test.rb` and verify failure.**
- [ ] **Step 3: Extract audit code into the new Ruby module** and retain a compatibility alias for `run_stress_test` that delegates to structured audit without synthetic claims.
- [ ] **Step 4: Instrument before recovery hooks return fallback.** Emit `PLACEHOLDER_SUBSTITUTED`, `CACHE_FALLBACK_HIT`, or `LOAD_SUPPRESSED` and never mark the original resource as decode/load success.
- [ ] **Step 5: Bridge events atomically to `AssetAuditService` via `RgssActivity`.** Do not persist reports inside immutable game content.
- [ ] **Step 6: Ensure generator copies the module and add integration snapshot assertions.**
- [ ] **Step 7: Run Ruby, Kotlin, and build tests; commit.**

```powershell
ruby android/shell/app/src/test/ruby/asset_audit_test.rb
cargo test --test build_test
```

```powershell
git add android/shell/app/src/main/assets/runtimepack_asset_audit.rb android/shell/app/src/main/assets/runtimepack_bootstrap.rb android/shell/app/src/test/ruby/asset_audit_test.rb android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt core/src/build_plan/generator.rs tests/integration/build_test.rs
git commit -m "feat: instrument RGSS asset loaders"
```

---

### Task 4: Web Loader Instrumentation

**Files:**
- Create: `android/shell/app/src/main/assets/web_shims/runtimepack_asset_audit.js`
- Modify: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`
- Modify: `android/shell/app/src/test/js/web_shims.test.js`
- Modify: `core/src/build_plan/generator.rs`

**Interfaces:**
- Produces structured image/audio/fetch/plugin load events for MV/MZ and Tyrano Web runtime.

- [ ] **Step 1: Write Node tests** for successful image/audio requests, HTTP/VFS miss, decode rejection, encrypted resolution, fallback, dynamic plugin request, duplicate request deduplication, and Tyrano-specific resource path.
- [ ] **Step 2: Verify failure with `node android/shell/app/src/test/js/web_shims.test.js`.**
- [ ] **Step 3: Implement hooks** around image/audio/fetch/XHR/engine loaders without changing game semantics. Events must distinguish delivery from browser decode and engine acceptance.
- [ ] **Step 4: Connect game WebView and overlay bridge to one `AssetAuditService`.**
- [ ] **Step 5: Update generator and tests; run Node/Kotlin/Rust tests.**
- [ ] **Step 6: Commit.**

```powershell
git add android/shell/app/src/main/assets/web_shims android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt android/shell/app/src/test/js/web_shims.test.js core/src/build_plan/generator.rs tests/integration/build_test.rs
git commit -m "feat: instrument web asset loaders"
```

---

### Task 5: Probe Planner and Sampled Mode

**Files:**
- Create: `android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt`
- Create: `android/shell/app/src/test/java/io/runtimepack/host/AssetProbeRunnerTest.kt`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/AssetAuditService.kt`
- Modify: `schemas/runtime-manifest.schema.json`

**Interfaces:**
- Produces: `ProbeMode::{Sampled,Exhaustive}`, `ProbeLimits`, deterministic sample selection.

- [ ] **Step 1: Write tests** for stable sampling across asset kinds/sizes/sources, required-first selection, unsupported kind reporting, cancellation, timeout, memory budget, and `NOT_RUN` default.
- [ ] **Step 2: Verify failure.**
- [ ] **Step 3: Implement sampled runner** consuming bundled static graph and backend capability. Release each decoded resource immediately after evidence capture.
- [ ] **Step 4: Add runtime capability metadata** without requiring probe for build success.
- [ ] **Step 5: Run tests and commit.**

```powershell
git add android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt android/shell/app/src/main/java/io/runtimepack/host/AssetAuditService.kt android/shell/app/src/test/java/io/runtimepack/host/AssetProbeRunnerTest.kt schemas/runtime-manifest.schema.json
git commit -m "feat: add sampled runtime asset probe"
```

---

### Task 6: Exhaustive Probe, Checkpoints, and Combined Summary

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/AssetAuditService.kt`
- Modify: `android/shell/app/src/test/java/io/runtimepack/host/AssetProbeRunnerTest.kt`
- Create: `schemas/runtime-validation-report.schema.json`
- Create: `schemas/combined-validation-summary.schema.json`

**Interfaces:**
- Produces runtime report and combined summary linked by static audit ID/hash.

- [ ] **Step 1: Write tests** for exhaustive unique traversal, checkpoint/resume, process restart, bounded concurrency, oversized image/audio rejection, per-resource timeout, global timeout, and static PASS/runtime FAIL separation.
- [ ] **Step 2: Verify failure.**
- [ ] **Step 3: Implement exhaustive mode** with configurable concurrency, decoded-pixel/audio limits, checkpoint sequence, and immediate release.
- [ ] **Step 4: Implement separate report reduction.** Probe failure must not alter historical build status; combined summary reports both statuses.
- [ ] **Step 5: Validate schemas and run tests.**
- [ ] **Step 6: Commit.**

```powershell
git add android/shell/app/src/main/java/io/runtimepack/host android/shell/app/src/test/java/io/runtimepack/host schemas/runtime-validation-report.schema.json schemas/combined-validation-summary.schema.json
git commit -m "feat: add exhaustive runtime asset probe"
```

---

### Task 7: Runtime Verification

- [ ] **Step 1: Run all local verification commands.**

```powershell
ruby android/shell/app/src/test/ruby/asset_audit_test.rb
node android/shell/app/src/test/js/web_shims.test.js
cargo test --test build_test
```

From `android/shell`:

```powershell
.\gradlew.bat testDebugUnitTest
```

- [ ] **Step 2: Run Rust lint/type checks.**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 3: Document Ren'Py runtime instrumentation as unavailable until its backend is integrated; do not simulate success.**
