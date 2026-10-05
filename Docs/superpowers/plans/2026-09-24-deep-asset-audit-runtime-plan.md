# Deep Asset Audit Runtime Instrumentation Implementation Plan (Condensed)

> **STATUS: SUPERSEDED / CLOSED (2026-09-30)** — Substansi plan ini telah terealisasi melalui `2026-09-27-comprehensive-asset-audit-remediation-plan.md` dan `2026-09-27-all-games-pass-audit-stabilization-plan.md` (ruleset v3→v11, RGSSAD/RPA in-memory indexer, reachability tree-shaking, VFS aliasing). Tidak akan dieksekusi apa adanya. — Ratifikasi audit 2026-09-30.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement runtime asset load interception (detecting hidden dummy bitmap/audio fallbacks), native VFS evidence capture, sampled/exhaustive runtime probes, and in-game diagnostic bridge cleanup (removing fake progress/success).

**Architecture:** Shared `AssetAuditService` in Android host receives structured events from native loaders and engine hooks (RGSS Ruby, MV/MZ/Tyrano Web). Fallbacks (`PLACEHOLDER_SUBSTITUTED`, `AUDIO_LOAD_SUPPRESSED`) are captured before recovery. The in-game Diagnostic Suite overlay reads authentic structured events through native bridges without synthesizing fake progress.

**Tech Stack:** Kotlin (Android Shell, VFS), Ruby (RGSS bootstrap), JavaScript (web shims, diagnostic overlay), JUnit, Node test harness.

**Spec:** `Docs/superpowers/specs/2026-09-24-multi-engine-deep-asset-audit-design.md`

## Global Constraints

- Never mutate game content or write inside immutable game directories (use app-private `Context.filesDir/runtime-state/`).
- Runtime evidence never erases static audit findings.
- Required asset fallback/suppression yields `FAIL`; optional fallback yields `WARNING`.
- Overlay diagnostic suite must never simulate completion or synthesize progress.
- Runtime probe is optional after build; missing probe yields `runtime_validation: "NOT_RUN"`.

---

### Task 1: Runtime Audit Protocol, Native VFS Evidence & In-Game Overlay Cleanup

**Files:**
- Create: `android/shell/app/src/main/java/io/runtimepack/host/AssetAuditModels.kt`
- Create: `android/shell/app/src/main/java/io/runtimepack/host/AssetAuditService.kt`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/GpackStreamReader.kt`
- Modify: `android/shell/app/src/main/assets/overlay/diagnostic_suite.js`
- Modify: `preview/diagnostic_suite.js`
- Create: `android/shell/app/src/test/java/io/runtimepack/host/AssetAuditServiceTest.kt`
- Modify: `android/shell/app/src/test/js/diagnostic_suite.test.js`

**Interfaces:**
- Produces:
  - `RuntimeAuditEvent`, `RuntimeAuditStage`, `RuntimeAuditOutcome`, `AssetAuditService`.
  - VFS event logging on file open, decryption, MIME check, and 404 fallback.
  - Overlay `diagnostic_suite.js` reads structured JSON progress and removes synthetic success loop.

- [ ] **Step 1: Write failing Kotlin & JS unit tests**:
  - `AssetAuditServiceTest.kt`: verify event ordering, atomic report persistence in `filesDir/runtime-state/audit/`, sequence monotonic IDs, and fallback outcome reduction.
  - `diagnostic_suite.test.js`: verify polling parses structured JSON events (`START`, `PROGRESS`, `STAGE`, `DONE`), sets `FAILED` on error, and never simulates completion on empty progress.
- [ ] **Step 2: Run tests to verify failure:**
  - `.\gradlew.bat testDebugUnitTest --tests io.runtimepack.host.AssetAuditServiceTest` (in `android/shell`)
  - `node android/shell/app/src/test/js/diagnostic_suite.test.js`
- [ ] **Step 3: Implement `AssetAuditModels.kt` & `AssetAuditService.kt`**:
  - Bounded memory event queue, atomic JSON report write on completion.
  - Expose bridge methods: `startAudit(requestJson)`, `getAuditEvents(afterSeq)`, `getAuditReport()`, `cancelAudit()`.
- [ ] **Step 4: Instrument `VfsResourceInterceptor.kt` & `GpackStreamReader.kt`**:
  - Emit structured events when assets are resolved, decrypted, or when 404 transparent fallback is triggered.
  - Ensure fallback emits `PLACEHOLDER_SUBSTITUTED` with original requested path.
- [ ] **Step 5: Clean up `diagnostic_suite.js`**:
  - Remove lines 1019-1041 (simulated progress and unconditional "Audit Complete: All map events and database references verified").
  - Connect to `getAuditEvents` / `getAuditReport` native bridge.
  - Sync `android/shell/.../diagnostic_suite.js` and `preview/diagnostic_suite.js` byte-for-byte.
- [ ] **Step 6: Run tests to verify pass:** Kotlin unit tests and Node overlay tests pass cleanly.
- [ ] **Step 7: Commit changes.**

```powershell
git add android/shell/app/src/main/java/io/runtimepack/host/AssetAudit* android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt android/shell/app/src/main/java/io/runtimepack/host/GpackStreamReader.kt android/shell/app/src/main/assets/overlay/diagnostic_suite.js preview/diagnostic_suite.js android/shell/app/src/test
git commit -m "feat: add runtime audit service, VFS instrumentation, and remove fake overlay progress"
```

---

### Task 2: Multi-Engine Loader Instrumentation (RGSS, Web, Tyrano) & Android Activity Bridges

**Files:**
- Create: `android/shell/app/src/main/assets/runtimepack_asset_audit.rb`
- Modify: `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
- Create: `android/shell/app/src/main/assets/web_shims/runtimepack_asset_audit.js`
- Modify: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`
- Create: `android/shell/app/src/test/ruby/asset_audit_test.rb`
- Modify: `android/shell/app/src/test/js/web_shims.test.js`
- Modify: `core/src/build_plan/generator.rs`

**Interfaces:**
- Produces:
  - RGSS loader hooks (`Bitmap.new`, `Cache.*`, `Audio.*`) emitting fallback diagnostics before returning dummy assets.
  - Web loader hooks (`Image.onload`, `Audio`, `syncFetch`) emitting load/decode events.
  - `RgssActivity` and `WebActivity` expose uniform `@JavascriptInterface` for audit & probe control.

- [ ] **Step 1: Write failing Ruby and JS unit tests**:
  - `asset_audit_test.rb`: test RGSS `Bitmap.new` failure emits `PLACEHOLDER_SUBSTITUTED` with exception class and requested path; `Audio.bgm_play` failure emits `LOAD_SUPPRESSED`.
  - `web_shims.test.js`: test missing web image emits fallback event; test broken transfer emits error event.
- [ ] **Step 2: Run tests to verify failure:**
  - `ruby android/shell/app/src/test/ruby/asset_audit_test.rb`
  - `node android/shell/app/src/test/js/web_shims.test.js`
- [ ] **Step 3: Implement `runtimepack_asset_audit.rb` & update bootstrap**:
  - Intercept `Bitmap.new`, `Cache.normal_bitmap`, `Audio.bgm_play`, `se_play`, `bgs_play`, `me_play`.
  - Capture file path and exception *before* returning 1x1 dummy bitmap or nil.
  - Write atomic event records to `files/runtime-state/audit/events.json` or forward via IPC.
- [ ] **Step 4: Implement `runtimepack_asset_audit.js` & update web shims**:
  - Intercept `Graphics.playVideo`, `ImageManager.loadBitmap`, `AudioManager.playBgm`, `playSe`.
  - Notify `AssetAuditService` via `RuntimePackWeb.postAuditEvent(eventJson)`.
- [ ] **Step 5: Connect `RgssActivity.kt` and `WebActivity.kt`**:
  - Wire JavaScript interfaces: `startAssetAudit`, `getAuditEvents`, `getAuditReport`, `cancelAssetAudit`.
  - Update `core/src/build_plan/generator.rs` to package the new audit scripts into APK assets.
- [ ] **Step 6: Run tests to verify pass:** Ruby tests, Node tests, Kotlin tests, and `cargo test --test integration_build`.
- [ ] **Step 7: Commit changes.**

```powershell
git add android/shell/app/src/main/assets/runtimepack_asset_audit.rb android/shell/app/src/main/assets/runtimepack_bootstrap.rb android/shell/app/src/main/assets/web_shims android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt core/src/build_plan/generator.rs android/shell/app/src/test
git commit -m "feat: instrument RGSS and Web asset loaders and connect activity bridges"
```

---

### Task 3: Sampled & Exhaustive Runtime Probe Engine with Checkpoints

**Files:**
- Create: `android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt`
- Create: `schemas/runtime-validation-report.schema.json`
- Create: `schemas/combined-validation-summary.schema.json`
- Create: `android/shell/app/src/test/java/io/runtimepack/host/AssetProbeRunnerTest.kt`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/AssetAuditService.kt`
- Modify: `android/shell/app/src/main/assets/overlay/diagnostic_suite.js`
- Modify: `preview/diagnostic_suite.js`
- Modify: `Docs/TEST_PLAN.md`
- Modify: `Docs/RUNTIME_CONTRACT.md`

**Interfaces:**
- Produces:
  - `AssetProbeRunner` supporting `ProbeMode::Sampled` (fast deterministic sample) and `ProbeMode::Exhaustive` (all unique assets with memory limits & checkpoints).
  - Emits `runtime-validation-report.json` and `combined-validation-summary.json`.
  - Overlay provides UI buttons to trigger Sampled Probe or Exhaustive Probe with real live counters.

- [ ] **Step 1: Write failing Kotlin & JS unit tests**:
  - `AssetProbeRunnerTest.kt`: test sampled mode selects representative images/audio deterministically; test exhaustive mode checkpoints every 100 items; test memory limit releases decoded bitmaps immediately; test timeout / cancellation halts cleanly.
  - `diagnostic_suite.test.js`: test triggering probe updates UI with authentic stage counters and exports combined report.
- [ ] **Step 2: Run tests to verify failure:** `.\gradlew.bat testDebugUnitTest --tests io.runtimepack.host.AssetProbeRunnerTest`.
- [ ] **Step 3: Implement `AssetProbeRunner.kt`**:
  - Reads embedded `asset-dependency-graph.json` or asset list.
  - Mode `Sampled`: selects N items per asset category (tileset, character, audio, picture).
  - Mode `Exhaustive`: queues all unique assets, decodes in small sequential batches, immediately releases memory (`Bitmap.recycle()`).
  - Checkpoints progress to `filesDir/runtime-state/audit/probe_checkpoint.json` for crash resilience.
- [ ] **Step 4: Implement report generators and schemas**:
  - Save `runtime-validation-report.json` and `combined-validation-summary.json`.
  - Validate against JSON schemas.
- [ ] **Step 5: Connect probe controls to in-game overlay**:
  - Diagnostic Suite tab "Stress Test" renamed to "Asset Audit & Probe".
  - Add `[ Run Sampled Probe ]` and `[ Run Exhaustive Probe ]` buttons.
  - Real live counters: `Scanned: X | Valid: Y | Missing: Z | Fallback: W`.
  - Export Bug Report includes `combined-validation-summary.json`.
- [ ] **Step 6: Run full test suite**:
  - `.\gradlew.bat testDebugUnitTest`
  - `node android/shell/app/src/test/js/diagnostic_suite.test.js`
  - `cargo test --workspace`
- [ ] **Step 7: Commit changes.**

```powershell
git add android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt schemas/runtime-validation-report.schema.json schemas/combined-validation-summary.schema.json android/shell/app/src/test android/shell/app/src/main/assets/overlay preview Docs/TEST_PLAN.md Docs/RUNTIME_CONTRACT.md
git commit -m "feat: implement sampled and exhaustive runtime probe engine with checkpoints"
```
