# Deep Asset Audit Overlay and Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the in-game Diagnostic Suite into a structured, non-authoritative audit viewer/probe controller and validate the complete system on synthetic device fixtures.

**Architecture:** Overlay reads authoritative static/runtime reports and versioned progress events through native bridges. It never simulates completion or computes outcome. Device tests prove actual loader failures, fallback evidence, and report persistence.

**Tech Stack:** JavaScript, Kotlin JS bridge, Node tests, Android instrumentation/ADB, synthetic fixtures.

**Spec:** `Docs/superpowers/specs/2026-09-24-multi-engine-deep-asset-audit-design.md`

## Global Constraints

- Requires Reporting and Runtime plans.
- Overlay is a viewer/controller only.
- No synthetic SUCCESS or percentage-based authority.
- Device builds require explicit user confirmation and `--flavor beta`.
- Commercial games are not committed as fixtures.

---

### Task 1: Structured Overlay State Machine

**Files:**
- Modify: `android/shell/app/src/main/assets/overlay/diagnostic_suite.js`
- Modify: `preview/diagnostic_suite.js`
- Modify: `android/shell/app/src/test/js/diagnostic_suite.test.js`

**Interfaces:**
- Consumes: versioned static report, runtime progress events, runtime report.
- Produces UI states: `UNAVAILABLE`, `IDLE`, `RUNNING`, `COMPLETED`, `FAILED`, `CANCELLED`.

- [ ] **Step 1: Write failing Node tests** asserting empty progress never advances status, missing bridge becomes `UNAVAILABLE`, malformed report becomes `FAILED`, `DONE` text alone is ignored, authoritative PASS/WARNING/FAIL/INDETERMINATE render exactly, and `NOT_RUN` is visible.
- [ ] **Step 2: Run `node android/shell/app/src/test/js/diagnostic_suite.test.js` and verify failure.**
- [ ] **Step 3: Replace text-prefix polling** with structured JSON parsing and monotonic sequence acknowledgement.
- [ ] **Step 4: Delete simulated progress/success logic** currently used when progress is empty.
- [ ] **Step 5: Synchronize production and preview copies byte-for-byte.**
- [ ] **Step 6: Run tests and commit.**

```powershell
git add android/shell/app/src/main/assets/overlay/diagnostic_suite.js preview/diagnostic_suite.js android/shell/app/src/test/js/diagnostic_suite.test.js
git commit -m "fix: remove synthetic audit success"
```

---

### Task 2: Audit Viewer and Multi-Source Findings

**Files:**
- Modify: `android/shell/app/src/main/assets/overlay/diagnostic_suite.js`
- Modify: `android/shell/app/src/main/assets/overlay/vpad.css`
- Modify: `preview/diagnostic_suite.js`
- Modify: `preview/vpad.css`
- Modify: `android/shell/app/src/test/js/diagnostic_suite.test.js`

- [ ] **Step 1: Write tests** for statistics, completeness/limits, stage badges, finding grouping by asset, expansion of multiple source references, case/path/decode/load categories, sanitized paths, orphan info, and HTML/JSON export links.
- [ ] **Step 2: Implement viewer** that displays values supplied by the report; it must not infer outcome from counters.
- [ ] **Step 3: Add accessibility and bounded rendering** for large finding lists using pagination/virtual batching.
- [ ] **Step 4: Synchronize preview and run tests.**

```powershell
node android/shell/app/src/test/js/diagnostic_suite.test.js
node android/shell/app/src/test/js/overlay_interaction_regression.test.js
```

- [ ] **Step 5: Commit.**

```powershell
git add android/shell/app/src/main/assets/overlay preview android/shell/app/src/test/js
git commit -m "feat: show structured asset audit findings"
```

---

### Task 3: Probe Controller and Native Bridges

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`
- Modify: `android/shell/app/src/main/assets/overlay/diagnostic_suite.js`
- Modify: `preview/diagnostic_suite.js`
- Modify: `android/shell/app/src/test/js/diagnostic_suite.test.js`
- Modify: `android/shell/app/src/test/java/io/runtimepack/host/CheatBridgeTest.kt`

**Interfaces:**
- Produces bridges: `startAssetProbe(requestJson)`, `getAssetAuditEvents(afterSequence)`, `getAssetAuditReport()`, `cancelAssetProbe()`.

- [ ] **Step 1: Write JS/Kotlin bridge tests** for sampled/exhaustive requests, cancellation, unavailable backend, duplicate starts, sequence acknowledgement, report retrieval, and share/export.
- [ ] **Step 2: Verify failure.**
- [ ] **Step 3: Implement identical bridge semantics** for RGSS and Web activities through `AssetAuditService`; retain legacy aliases only as deprecation adapters.
- [ ] **Step 4: Add overlay controls** for sampled/exhaustive, limits summary, cancel, resume checkpoint, and report export.
- [ ] **Step 5: Run Node/Kotlin tests and commit.**

```powershell
git add android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt android/shell/app/src/main/assets/overlay/diagnostic_suite.js preview/diagnostic_suite.js android/shell/app/src/test
git commit -m "feat: control runtime asset probes from overlay"
```

---

### Task 4: Generator and Build Artifact Contract

**Files:**
- Modify: `core/src/build_plan/generator.rs`
- Modify: `tests/integration/build_test.rs`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/BuildManifestLoader.kt`
- Create: `android/shell/app/src/test/java/io/runtimepack/host/BuildManifestLoaderTest.kt`

- [ ] **Step 1: Write tests** asserting beta builds contain viewer/probe tooling and authoritative static report metadata, release builds follow approved inclusion policy, runtime status begins `NOT_RUN`, and generated overlay files are synchronized.
- [ ] **Step 2: Verify failure.**
- [ ] **Step 3: Implement manifest loader fields and generator copying.**
- [ ] **Step 4: Run Rust/Kotlin tests and commit.**

```powershell
git add core/src/build_plan/generator.rs tests/integration/build_test.rs android/shell/app/src/main/java/io/runtimepack/host/BuildManifestLoader.kt android/shell/app/src/test/java/io/runtimepack/host/BuildManifestLoaderTest.kt
git commit -m "feat: package audit viewer and probe metadata"
```

---

### Task 5: Android Instrumentation Fixtures

**Files:**
- Create: `android/shell/app/src/androidTest/java/io/runtimepack/host/AssetAuditInstrumentedTest.kt`
- Create synthetic fixture assets under: `android/shell/app/src/androidTest/assets/asset_audit/`
- Modify: `android/shell/app/build.gradle.kts` only if instrumentation dependencies are already pinned or can be approved

- [ ] **Step 1: Add synthetic files:** valid PNG/audio, corrupt PNG/audio, missing logical targets, case mismatch/collision, fallback-triggering request, and multi-source report.
- [ ] **Step 2: Write instrumentation tests** for actual AssetManager/GPACK read, WebView image decode, audio decode/load signal where deterministic, fallback event, pause/resume, cancellation, process restart, report recovery, and source immutability.
- [ ] **Step 3: Run local compile/unit verification without device.**

From `android/shell`:

```powershell
.\gradlew.bat testDebugUnitTest
```

- [ ] **Step 4: After explicit user confirmation, run device tests.**

```powershell
.\gradlew.bat connectedDebugAndroidTest
```

Expected: RGSS/Web supported paths pass; Ren'Py reports unavailable rather than simulated success; Tyrano uses only permitted synthetic content.

- [ ] **Step 5: Commit.**

```powershell
git add android/shell/app/src/androidTest android/shell/app/build.gradle.kts
git commit -m "test: validate runtime asset audits on Android"
```

---

### Task 6: End-to-End Regression Matrix

**Files:**
- Modify: `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`
- Modify: `Docs/TEST_PLAN.md`
- Modify: `Docs/ANDROID_RUNTIME.md`
- Modify: `Docs/RUNTIME_CONTRACT.md`

- [ ] **Step 1: Add matrix columns** for static standard, static deep, runtime instrumentation, sampled probe, exhaustive probe, and known unsupported scope.
- [ ] **Step 2: Run all lightweight suites.**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node android/shell/app/src/test/js/diagnostic_suite.test.js
node android/shell/app/src/test/js/web_shims.test.js
node android/shell/app/src/test/js/overlay_interaction_regression.test.js
ruby android/shell/app/src/test/ruby/asset_audit_test.rb
```

From `android/shell`:

```powershell
.\gradlew.bat testDebugUnitTest
```

- [ ] **Step 3: Verify negative acceptance cases:** no progress, zero scan, timeout, parser failure, limit reached, invalid report, decode failure, fallback, and unavailable adapter never display PASS.
- [ ] **Step 4: Commit documentation.**

```powershell
git add Docs/MULTI_ENGINE_VALIDATION_MATRIX.md Docs/TEST_PLAN.md Docs/ANDROID_RUNTIME.md Docs/RUNTIME_CONTRACT.md
git commit -m "docs: record deep asset audit validation matrix"
```
