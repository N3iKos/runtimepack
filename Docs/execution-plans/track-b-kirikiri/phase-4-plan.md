# Implementation Plan: Kirikiri Engine Integration — Phase 4 (Packaging Pipeline, VPad Overlay & End-to-End Verification)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the Kirikiri packaging pipeline by adding Kirikiri project generation to `AndroidProjectGenerator`, integrating Visual Novel VPad controls and touch passthrough, enabling GPACK asset packaging for `.xp3` files, and verifying standalone APK generation.

**Architecture:**
1. In `core/src/build_plan/generator.rs`, add support for `RuntimeFamily::KirikiriNative`:
   - Set launcher activity to `io.runtimepack.host.KirikiriActivity`.
   - Copy `libkrkr2.so` and `libc++_shared.so` to output `jniLibs/arm64-v8a/`.
2. In `GpackExtractor.kt`, stage `.xp3` files and loose game files into `filesDir/game/` during bootstrap.
3. In `vpad.js` and `vpad.css`, add a dedicated **Visual Novel Preset** (buttons: Quick Save, Quick Load, Backlog / History, Auto, Skip, Hide Textbox).
4. Verify end-to-end compilation with `runtimepack build <game> --flavor beta`, producing a fully functioning standalone APK.

**Tech Stack:** Rust (generator, packager, CLI), Kotlin (`GpackExtractor.kt`), JavaScript (`vpad.js`).

**Spec:** `Docs/execution-plans/track-b-kirikiri/architecture-spec.md`

## Global Constraints

- Autonomous subagents MUST build with `--flavor beta`.
- All builds must produce APKs that launch directly to the game title screen without interactive file selectors.
- Source directories in `Raw-Game/` remain strictly immutable.
- All workspace Cargo and Android Gradle tests must pass 100%.

---

### Task 1: Android Project Generator Integration for Kirikiri

**Files:**
- Modify: `core/src/build_plan/generator.rs`
- Modify: `cli/src/main.rs`
- Test: `tests/integration/build_test.rs`

**Interfaces:**
- Produces:
  - In `generator.rs`: When `manifest.runtime.family == "Kirikiri Native"`, sets `main_activity = "io.runtimepack.host.KirikiriActivity"`.
  - Copies `runtimes/kirikiri/jni/arm64-v8a/` libraries to the generated Android project.
  - Packages `build-manifest.json` and asset inventory.

- [ ] **Step 1: Write integration test in `build_test.rs`**:
  ```rust
  #[test]
  fn test_emit_project_for_kirikiri_sets_kirikiri_activity_and_jnilibs() {
      let temp = test_temp_dir("kirikiri-emit");
      let manifest = create_test_manifest_for_kirikiri();
      AndroidProjectGenerator::emit_project(&temp, &manifest, None).unwrap();
      
      let manifest_xml = std::fs::read_to_string(temp.join("app/src/main/AndroidManifest.xml")).unwrap();
      assert!(manifest_xml.contains("io.runtimepack.host.KirikiriActivity"));
  }
  ```
- [ ] **Step 2: Run test to verify failure**:
  `cargo test --test build_test test_emit_project_for_kirikiri_sets_kirikiri_activity_and_jnilibs`
- [ ] **Step 3: Update `generator.rs`**:
  - In `emit_project_with_source`:
    Match `manifest.runtime.family.as_str()`:
    `"Kirikiri Native" => "io.runtimepack.host.KirikiriActivity",`
  - In JNI copy logic: copy `runtimes/kirikiri/jni/` into `app/src/main/jniLibs/`.
- [ ] **Step 4: Run test to verify pass**:
  `cargo test --test build_test test_emit_project_for_kirikiri_sets_kirikiri_activity_and_jnilibs`
- [ ] **Step 5: Commit**:
  ```powershell
  git add core/src/build_plan/generator.rs cli/src/main.rs tests/integration/build_test.rs
  git commit -m "feat(generator): support Kirikiri Native Android project generation"
  ```

---

### Task 2: GPACK Staging & Bootstrapping for XP3 Archives

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/GpackExtractorTest.kt`

**Interfaces:**
- Produces:
  - In `GpackExtractor.kt`:
    When extracting assets from `game.pack`:
    Extracts `.xp3` archives directly into `destDir/game/` (e.g. `data.xp3`, `system.xp3`, `patch.xp3`).
    Ensures file permissions and atomic writes so TVP can seek and open archives immediately upon startup.

- [ ] **Step 1: Write unit test in `GpackExtractorTest.kt`**:
  - Test that an archive containing `data.xp3` is extracted cleanly into `destDir/game/data.xp3`.
- [ ] **Step 2: Run test to verify failure**:
  `.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests io.runtimepack.host.GpackExtractorTest`
- [ ] **Step 3: Implement XP3 staging in `GpackExtractor.kt`**:
  - Ensure `.xp3` and `.tjs` files preserve relative paths under `game/`.
- [ ] **Step 4: Run test to verify pass**:
  `.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests io.runtimepack.host.GpackExtractorTest`
- [ ] **Step 5: Commit**:
  ```powershell
  git add android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt android/shell/app/src/test/java/io/runtimepack/host/GpackExtractorTest.kt
  git commit -m "feat(android): support XP3 archive staging in GpackExtractor"
  ```

---

### Task 3: Visual Novel Virtual Pad Preset (`vpad.js`)

**Files:**
- Modify: `android/shell/app/src/main/assets/overlay/vpad.js`
- Modify: `android/shell/app/src/main/assets/overlay/vpad.css`
- Modify: `preview/vpad.js`
- Modify: `preview/vpad.css`
- Test: `android/shell/app/src/test/js/vpad.test.js`

**Interfaces:**
- Produces:
  - Adds `"vn"` preset to `window.RuntimePackVPad`:
    - Floating action buttons: `Quick Save`, `Quick Load`, `Backlog`, `Auto`, `Skip`, `Hide UI`.
    - Compact, non-intrusive layout suitable for visual novels.
    - Transparent touch pass-through to game canvas.

- [ ] **Step 1: Write JS unit test in `vpad.test.js`**:
  - Test that selecting `"vn"` preset renders the visual novel toolbar buttons.
- [ ] **Step 2: Implement VN preset in `vpad.js` & `vpad.css`**:
  - Add preset definition and styling.
  - Synchronize byte-for-byte to `preview/`.
- [ ] **Step 3: Run test to verify pass**:
  `node android/shell/app/src/test/js/vpad.test.js`
- [ ] **Step 4: Commit**:
  ```powershell
  git add android/shell/app/src/main/assets/overlay/vpad.js android/shell/app/src/main/assets/overlay/vpad.css preview/vpad.js preview/vpad.css
  git commit -m "feat(vpad): add Visual Novel control preset for Kirikiri and narrative games"
  ```

---

### Task 4: End-to-End Build Verification on Synthetic Kirikiri Game

**Files:**
- Test target: Synthetic Kirikiri project with `data.xp3`
- Output: `dist/synthetic_kirikiri/SyntheticKirikiri.apk`

**Interfaces:**
- Produces:
  - Verified compilation of standalone Android APK for Kirikiri.
  - Exit code 0 from Gradle build.
  - Verification of `--flavor beta` watermark and direct-boot configuration.

- [ ] **Step 1: Write integration build test in `tests/integration/build_test.rs`**:
  - `test_build_synthetic_kirikiri_game_produces_valid_apk_project`:
    Executes build plan for a synthetic Kirikiri game directory, verifies output directory, manifest, and settings.
- [ ] **Step 2: Run test to verify pass**:
  `cargo test --test build_test test_build_synthetic_kirikiri_game_produces_valid_apk_project`
- [ ] **Step 3: Run full workspace verification**:
  `cargo test --workspace`
  `.\android\shell\gradlew.bat -p android/shell testDebugUnitTest`
- [ ] **Step 4: Update documentation & commit**:
  ```powershell
  git add tests/integration/build_test.rs memory/progress.md memory/decisions_log.md
  git commit -m "chore: verify end-to-end Kirikiri build pipeline and APK packaging"
  ```
