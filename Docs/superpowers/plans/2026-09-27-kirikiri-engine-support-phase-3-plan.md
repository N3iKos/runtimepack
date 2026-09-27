# Implementation Plan: Kirikiri Engine Integration — Phase 3 (Headless Native C++ Runtime & Android Shell)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the headless C++ native engine runtime (`libkrkr2.so`) compiled via NDK r28 with 16 KB page-size alignment, patch out the interactive file picker for 100% direct-boot execution, and implement `KirikiriActivity.kt` in the Android host shell.

**Architecture:**
1. Pin upstream repository `2468785842/krkr2` at commit `dca7264572a63e753c1b07ca7053513d1751ce70`.
2. Apply C++ direct-boot patches to `AndroidUtils.cpp` and `AppDelegate.cpp` to bypass `TVPMainFileSelectorForm` and boot directly from `filesDir/game`.
3. Build `libkrkr2.so` and `libc++_shared.so` for `arm64-v8a` using NDK r28, enforcing 16 KB ELF page alignment.
4. Implement `KirikiriActivity.kt` in `android/shell/app/src/main/java/io/runtimepack/host/` to manage TVP/Cocos lifecycle, touch forwarding, and save data isolation.

**Tech Stack:** C++17, Android NDK r28, CMake 3.31.1, Kotlin (Android SDK API 34).

**Spec:** `Docs/superpowers/specs/2026-09-27-kirikiri-complete-architecture-spec.md`

## Global Constraints

- Standalone APKs must never show a file browser or launcher UI; direct boot to title screen is mandatory.
- All native shared libraries must have 16 KB ELF alignment (`max-page-size=16384`).
- Save data must be isolated strictly to `Context.filesDir + "/savedata"`.
- Zero broad external storage permissions (`MANAGE_EXTERNAL_STORAGE` forbidden).

---

### Task 1: Direct-Boot Native C++ Patches & Headless TVP Ingestion

**Files:**
- Create: `runtimes/kirikiri/patches/0001-direct-boot-bypass-file-selector.patch`
- Create: `runtimes/kirikiri/runtime-manifest.json`
- Test: `tests/integration/runtime_registry_test.rs`

**Interfaces:**
- Produces:
  - C++ patch for `AndroidUtils.cpp` overriding `TVPCheckStartupArg()` to call `TVPMainScene::GetInstance()->startupFrom(entry)`.
  - C++ patch for `AppDelegate.cpp` eliminating `TVPMainFileSelectorForm::create()`.
  - Manifest registering `kirikiri-native` with runtime package `runtimes/kirikiri`.

- [ ] **Step 1: Write integration test in `runtime_registry_test.rs`**:
  ```rust
  #[test]
  fn test_kirikiri_runtime_registered_in_runtime_registry() {
      let registry = RuntimeRegistry::new();
      let runtimes = registry.find_for_engine(EngineId::Kirikiri);
      assert_eq!(runtimes.len(), 1);
      assert_eq!(runtimes[0].id, "kirikiri-native");
      assert_eq!(runtimes[0].family, RuntimeFamily::KirikiriNative);
  }
  ```
- [ ] **Step 2: Run test to verify failure**:
  `cargo test --test runtime_registry_test test_kirikiri_runtime_registered_in_runtime_registry`
- [ ] **Step 3: Create patch and runtime manifest**:
  - In `runtimes/kirikiri/runtime-manifest.json`:
    Register `kirikiri-native` runtime backend.
  - In `runtimes/kirikiri/patches/0001-direct-boot-bypass-file-selector.patch`:
    Formalize the exact C++ patch replacing `TVPCheckStartupArg()` and `AppDelegate.cpp` file selector fallback.
  - Register in `core/src/runtime_registry/mod.rs`.
- [ ] **Step 4: Run test to verify pass**:
  `cargo test --test runtime_registry_test test_kirikiri_runtime_registered_in_runtime_registry`
- [ ] **Step 5: Commit**:
  ```powershell
  git add runtimes/kirikiri/ tests/integration/runtime_registry_test.rs core/src/runtime_registry/mod.rs
  git commit -m "feat(runtime): register kirikiri-native runtime and direct-boot native C++ patches"
  ```

---

### Task 2: Native Library Provisioning & 16 KB Page Alignment Verification

**Files:**
- Create: `android/shell/app/src/main/jniLibs/arm64-v8a/libkrkr2.so` (or staging script)
- Create: `scripts/verify-16kb-alignment.ps1`
- Test: Native ELF header verification

**Interfaces:**
- Produces:
  - Valid `libkrkr2.so` for `arm64-v8a`.
  - `llvm-readelf -Wl libkrkr2.so` verifies `p_align >= 16384` on all `LOAD` segments.

- [ ] **Step 1: Write verification script `scripts/verify-16kb-alignment.ps1`**:
  - Executes `llvm-readelf -Wl` (or inspects ELF program header bytes in Rust/PowerShell).
  - Asserts that all PT_LOAD segments have `Align >= 0x4000` (16384).
- [ ] **Step 2: Provision compiled `libkrkr2.so` in `runtimes/kirikiri/jni/arm64-v8a/`**:
  - Compiled with NDK r28, CMake 3.31.1, C++17, and direct-boot patches.
- [ ] **Step 3: Run alignment verification**:
  - Run `.\scripts\verify-16kb-alignment.ps1 runtimes/kirikiri/jni/arm64-v8a/libkrkr2.so`
  - Expected: PASS (16 KB aligned).
- [ ] **Step 4: Commit**:
  ```powershell
  git add runtimes/kirikiri/jni/ scripts/verify-16kb-alignment.ps1
  git commit -m "feat(kirikiri): provision 16KB-aligned libkrkr2.so native runtime for arm64-v8a"
  ```

---

### Task 3: Android Shell Activity & Save Data Isolation (`KirikiriActivity.kt`)

**Files:**
- Create: `android/shell/app/src/main/java/io/runtimepack/host/KirikiriActivity.kt`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/KirikiriActivityTest.kt`

**Interfaces:**
- Produces:
  - `class KirikiriActivity : AppCompatActivity()` (or bridging `Cocos2dxActivity`).
  - Manages TVP native lifecycle (`onPause`, `onResume`, `onDestroy`).
  - Forces `-datapath` and `savedata` path to `File(filesDir, "savedata")`.
  - Routes multi-touch events:
    - 1 finger: Left click / advance.
    - 2 fingers: Right click / menu.
    - 3 fingers: Middle click.

- [ ] **Step 1: Write unit test in `KirikiriActivityTest.kt`**:
  - Test that `getEffectiveSaveDir(context)` returns `File(context.filesDir, "savedata")`.
  - Test that entry path is resolved to `File(context.filesDir, "game/data.xp3")` or `File(context.filesDir, "game")`.
- [ ] **Step 2: Run test to verify failure**:
  `.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests io.runtimepack.host.KirikiriActivityTest`
- [ ] **Step 3: Implement `KirikiriActivity.kt`**:
  - Set up JNI environment and ensure `libkrkr2.so` is loaded.
  - Configure internal paths and touch handlers.
- [ ] **Step 4: Run test to verify pass**:
  `.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests io.runtimepack.host.KirikiriActivityTest`
- [ ] **Step 5: Commit**:
  ```powershell
  git add android/shell/app/src/main/java/io/runtimepack/host/KirikiriActivity.kt android/shell/app/src/test/java/io/runtimepack/host/KirikiriActivityTest.kt
  git commit -m "feat(android): implement KirikiriActivity with direct boot and save isolation"
  ```
