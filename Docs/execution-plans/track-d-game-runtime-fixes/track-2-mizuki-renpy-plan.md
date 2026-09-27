# Implementation Plan: Track 2 — Mizuki And Me (Ren'Py) Android Runtime

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Provide a working Ren'Py Android runtime by bundling the official RAPT native libraries and implementing a RuntimePack `RenpyActivity`, then rebuild `MizukiAndMe.apk` and verify it boots and plays on the MuMu emulator.

**Architecture:** Acquire + audit the official RAPT arm64 runtime (`libpython3.x.so`, `librenpython.so`, SDL2 libs), create `RenpyActivity.kt` that initializes the Python VM, mounts game content from `filesDir`, and drives GL/audio/input; wire the generator to route Ren'Py games to `RenpyActivity` and copy the native libs into `jniLibs`.

**Tech Stack:** Rust (`core` generator/registry, `cli`), Kotlin/Android (`RenpyActivity.kt`, shell), Ren'Py RAPT native runtime, Gradle, MuMu emulator.

**Spec:** `Docs/execution-plans/track-d-game-runtime-fixes/design.md`

## Global Constraints

- Source game in `Raw-Game/Mizuki And Me` remains strictly 100% immutable.
- Builds use `--flavor beta`.
- Third-party native binaries require pinned version + provenance + license notice before bundling (`AGENTS.md` §12).
- Save data isolated to `Context.filesDir`; no broad external storage permissions.
- Mizuki is Ren'Py 8 (Python 3) — runtime version must match.

---

### Task 1: Acquire, Pin & Audit the RAPT arm64 Runtime

**Files:**
- Create: `runtimes/renpy/manifest.json`
- Create: `runtimes/renpy/jni/arm64-v8a/` (native libs)
- Modify: `Docs/LICENSE_COMPLIANCE.md` / third-party notices

**Interfaces:**
- Produces:
  - Pinned Ren'Py SDK version + exact list of native libs (name, size, SHA-256, license).
  - `runtimes/renpy/manifest.json` describing the runtime backend.

- [ ] **Step 1:** Determine the exact Ren'Py SDK version matching Mizuki (Ren'Py 8 / Python 3) from `Raw-Game/Mizuki And Me` (`renpy/` markers, `log.txt`, `.rpy` version strings). Record evidence.
- [ ] **Step 2:** Fetch the official RAPT runtime for `arm64-v8a` (`libpython3.x.so`, `librenpython.so`, `libSDL2*.so`, `libc++_shared.so`, etc.). Pin the source + version.
- [ ] **Step 3:** Record provenance + license notices; verify redistribution permission for each binary.
- [ ] **Step 4:** Write `runtimes/renpy/manifest.json` with runtime id `renpy-android-8` (or matching generation).
- [ ] **Step 5: Commit**
  ```powershell
  git add runtimes/renpy/ Docs/LICENSE_COMPLIANCE.md
  git commit -m "feat(renpy): pin and audit official RAPT arm64 runtime with provenance"
  ```

---

### Task 2: Implement `RenpyActivity.kt` Shell

**Files:**
- Create: `android/shell/app/src/main/java/io/runtimepack/host/RenpyActivity.kt`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/RenpyActivityTest.kt`

**Interfaces:**
- Produces:
  - `RenpyActivity` that loads the Ren'Py native runtime, initializes the Python VM, mounts game content from `filesDir/game`, sets save dir to `filesDir/saves`, and presents a GL surface.
  - Touch input forwarding (tap = advance, swipe = menu/backlog per Ren'Py conventions).

- [ ] **Step 1:** Write JVM unit tests for path resolution (game root, save dir, `.rpa` presence).
- [ ] **Step 2:** Implement `RenpyActivity.kt` adopting the RAPT `PythonActivity`/`SDLActivity` launch pattern (JNI init, native lib load order, asset extraction on first run).
- [ ] **Step 3:** Run `.\android\shell\gradlew.bat -p android/shell testDebugUnitTest`.
- [ ] **Step 4: Commit**
  ```powershell
  git add android/shell/app/src/main/java/io/runtimepack/host/RenpyActivity.kt android/shell/app/src/test/java/io/runtimepack/host/RenpyActivityTest.kt
  git commit -m "feat(android): implement RenpyActivity with Python VM init and save isolation"
  ```

---

### Task 3: Generator & Registry Wiring for Ren'Py

**Files:**
- Modify: `core/src/build_plan/generator.rs` (activity mapping + jniLibs copy)
- Modify: `core/src/runtime_registry/mod.rs` (point Ren'Py to the real runtime)
- Test: `tests/integration/build_test.rs`

**Interfaces:**
- Produces:
  - Ren'Py family → `io.runtimepack.host.RenpyActivity`.
  - `runtimes/renpy/jni/arm64-v8a/*` copied to the generated project `jniLibs`.

- [ ] **Step 1:** Write integration test asserting the emitted `AndroidManifest.xml` for a Ren'Py game references `RenpyActivity` and that jniLibs are copied.
- [ ] **Step 2:** Update `generator.rs` mapping and copy logic.
- [ ] **Step 3:** Update runtime registry to resolve `renpy-android-8` to the real bundled runtime.
- [ ] **Step 4:** Run `cargo test --test build_test` and `cargo test --workspace`.
- [ ] **Step 5: Commit**
  ```powershell
  git add core/src/build_plan/generator.rs core/src/runtime_registry/mod.rs tests/integration/build_test.rs
  git commit -m "feat(generator): route Ren'Py games to RenpyActivity and bundle RAPT runtime"
  ```

---

### Task 4: Rebuild & Verify Mizuki on MuMu

**Files:**
- Output: `dist/mizuki_and_me/MizukiAndMe.apk`

- [ ] **Step 1: Rebuild**
  Run: `cargo run --bin runtimepack -- build "Raw-Game\Mizuki And Me" -o dist/mizuki_and_me --package com.cherrykiss.mizukiandme --name "Mizuki And Me" --flavor beta --accept-audit-fail --accept-audit-unknown`
  Verify Gradle exit 0.
- [ ] **Step 2: Install on MuMu**
  Run: `adb -s 127.0.0.1:16384 install -r dist/mizuki_and_me/MizukiAndMe.apk`
- [ ] **Step 3: Launch & capture**
  Launch `com.cherrykiss.mizukiandme`, capture logcat + screenshot. Verify: Ren'Py initializes, title screen renders (not black), no Python traceback.
- [ ] **Step 4: Verify stability**
  Confirm navigation, text advance, and save/load work.
- [ ] **Step 5: Commit verification record**
  ```powershell
  git commit --allow-empty -m "chore: verify Mizuki And Me Ren'Py runtime on MuMu emulator"
  ```
