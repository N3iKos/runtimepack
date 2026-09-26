# Implementation Plan: Multi-Engine Stress Test Stability, Frame Yielding & OOM Prevention

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate the game freeze and Linux kernel OOM force close (`lowmemorykiller` SIGKILL) during diagnostic stress tests and runtime probes across all supported engines (RGSS, MV, MZ, Tyrano).

**Architecture:** 
1. **Shared Overlay Layer (`diagnostic_suite.js` & `preview/`):** Fix the dual-execution bug by strictly decoupling engine-level logic audits (`Run Full Stress Test` -> dispatches to engine runner only) from native media decoder probes (`Run Sampled/Exhaustive Probe` -> calls native `AssetProbeRunner` only).
2. **Android Host Layer (`AssetProbeRunner.kt` & `AssetAuditService.kt`):** Implement adaptive GC pacing, memory pressure awareness (`Runtime.getRuntime().freeMemory()`), and batch throttling in native decoding to guarantee resident memory never breaches Android thresholds (~1.6 GB) regardless of project size.
3. **Engine Runner Layer (RGSS Ruby & Web JS Shims):** Implement cooperative frame yielding (calling `Graphics.update` / yielding to event loop every N maps) to keep the SDL / Web viewports responsive and prevent watchdog freezes, coupled with buffered progress writes (replacing 500+ blocking disk writes with batch flushes) and directory entry caching.

**Tech Stack:** JavaScript (Overlay UI), Kotlin (Android Host Shell, Memory Throttle), Ruby (RGSS Bootstrap), Web JS (MV/MZ/Tyrano Shims).

**Spec Reference:** `Docs/HASIL_INVESTIGASI_DIAGNOSTIC_BLACK_SOULS_2.md`

## Global Constraints

- Must solve stability for **all supported engines** (RGSS, MV, MZ, Tyrano), not just Ruby.
- `diagnostic_suite.js` in `android/shell/app/src/main/assets/overlay/` and `preview/` must remain 100% byte-for-byte identical.
- Game source content in `Raw-Game/` remains strictly immutable.
- All unit and integration tests must pass cleanly before completing each task.
- Zero fake progress or simulated `SUCCESS`.

---

### Task 1: Decouple Overlay Triggers & Add Memory Pacing to Native Probe (Fix OOM Crash)

**Files:**
- Modify: `android/shell/app/src/main/assets/overlay/diagnostic_suite.js`
- Modify: `preview/diagnostic_suite.js`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/AssetAuditService.kt`
- Modify: `android/shell/app/src/test/js/diagnostic_suite.test.js`
- Modify: `android/shell/app/src/test/java/io/runtimepack/host/AssetProbeRunnerTest.kt`

**Interfaces:**
- Produces:
  - `runStressTest()` strictly dispatches `run_stress_test` to the engine without invoking `startAudit("exhaustive")`.
  - `runProbe(mode)` strictly invokes native `startAudit(mode)` without dispatching engine stress tests.
  - `AssetProbeRunner.kt` incorporates memory throttling: monitors memory headroom, pauses `10ms` every 20 bitmaps, and explicitly requests `System.gc()` if available memory drops below 25%, preventing `lowmemorykiller` threshold breach.

- [ ] **Step 1: Write failing unit tests**:
  - `diagnostic_suite.test.js`: test that calling `DiagnosticSuite.runStressTest()` does NOT invoke `RuntimePackVPad.startAudit` or `RuntimePackWeb.startAudit`.
  - `diagnostic_suite.test.js`: test that calling `DiagnosticSuite.runProbe("sampled")` does NOT dispatch `{ action: "run_stress_test" }`.
  - `AssetProbeRunnerTest.kt`: test that batch decoding honors memory throttle thresholds and does not accumulate unbounded un-recycled native references.
- [ ] **Step 2: Update `diagnostic_suite.js` & `preview/diagnostic_suite.js`**:
  - Separate `runStressTest` from `runProbe`:
    - `runStressTest`: dispatches command to engine (`postCheatCmd({ action: "run_stress_test" })`) and polls progress without starting native probe runner.
    - `runProbe(mode)`: starts native probe (`startAudit(JSON.stringify({ mode: mode }))`) and polls native event queue.
  - Sync `assets/overlay/` and `preview/` byte-for-byte.
- [ ] **Step 3: Update `AssetProbeRunner.kt`**:
  - Add memory pacing in `run()`:
    - Check available memory before decoding each batch of 20 images.
    - Call `Thread.sleep(5)` to allow GC and system looper to catch up on native allocations.
    - Guarantee `bitmap.recycle()` is called immediately in a `finally` block for every single decoded bitmap.
- [ ] **Step 4: Run tests to verify pass**:
  - `node android/shell/app/src/test/js/diagnostic_suite.test.js`
  - `.\gradlew.bat testDebugUnitTest` in `android/shell`

---

### Task 2: Cooperative Frame Yielding in Engine Audits (Prevent Freeze / SDL Lockup)

**Files:**
- Modify: `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
- Modify: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
- Modify: `android/shell/app/src/test/ruby/asset_audit_test.rb`
- Modify: `android/shell/app/src/test/js/web_shims.test.js`

**Interfaces:**
- Produces:
  - RGSS `run_stress_test` yields every 5 maps: calls `Graphics.update` and `Input.update` so the SDL render loop, window message pump, and overlay remain responsive.
  - Web shims `run_stress_test` processes maps asynchronously or in chunked frames (`requestAnimationFrame` / `setTimeout`) rather than blocking the WebView main thread.

- [ ] **Step 1: Write failing unit tests**:
  - `asset_audit_test.rb`: test that `run_stress_test` invokes frame yield hook during map iteration.
  - `web_shims.test.js`: test that web stress test processes maps in chunked batches without locking event loop.
- [ ] **Step 2: Implement frame yield in `runtimepack_bootstrap.rb`**:
  - In `run_stress_test`, inside map loop:
    ```ruby
    if (idx % 5).zero?
      Graphics.update rescue nil
      Input.update rescue nil
    end
    ```
  - Allows SDL thread to process input events, keep on-screen FPS counter ticking, and avoid Android system thread starvation.
- [ ] **Step 3: Implement frame yield in `runtimepack_web_shims.js`**:
  - Chunk map and database auditing across animation frames so long-running audits do not freeze HTML5 audio or DOM interactions.
- [ ] **Step 4: Run tests to verify pass**:
  - `ruby android/shell/app/src/test/ruby/asset_audit_test.rb`
  - `node android/shell/app/src/test/js/web_shims.test.js`

---

### Task 3: Buffered Progress I/O & Directory Cache Optimization

**Files:**
- Modify: `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`
- Modify: `android/shell/app/src/test/ruby/asset_audit_test.rb`

**Interfaces:**
- Produces:
  - `runtimepack_bootstrap.rb` buffers `FAIL:` progress lines in memory and flushes them to disk once per map (or in chunks of 20), reducing 600+ synchronous open/write/close operations to under 20.
  - `RuntimePackCasePath.resolve` caches `Dir.entries` lookups in an in-memory hash table for the duration of the audit, eliminating thousands of redundant directory disk scans on `Graphics/Pictures/`.
  - `RgssActivity.kt:getAuditProgress()` reads progress safely without locking or colliding with active writes.

- [ ] **Step 1: Write failing unit test** in `asset_audit_test.rb`:
  - Test that buffered progress writer reduces disk open operations and emits complete progress output.
  - Test that `RuntimePackCasePath` directory caching avoids redundant `Dir.entries` calls.
- [ ] **Step 2: Implement progress buffer & directory cache in `runtimepack_bootstrap.rb`**:
  - Add `@progress_buffer = []` and `flush_progress_buffer!`. Flush every 10 missing items, at the end of each map, and at `DONE:`.
  - In `RuntimePackCasePath`: cache directory contents in `@dir_cache ||= {}` during audit runs, clearing it on completion.
- [ ] **Step 3: Run full verification suite**:
  - `ruby android/shell/app/src/test/ruby/asset_audit_test.rb`
  - `node android/shell/app/src/test/js/diagnostic_suite.test.js`
  - `node android/shell/app/src/test/js/web_shims.test.js`
  - `.\gradlew.bat testDebugUnitTest` in `android/shell`
  - `cargo test --workspace`
- [ ] **Step 4: Commit changes and prepare build for user testing.**
