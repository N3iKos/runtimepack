# RPG Maker MV/MZ Web Runtime & NW.js Normalization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable end-to-end packaging and playable execution of RPG Maker MV/MZ games (such as *Fear & Hunger 2: Termina*) by implementing automatic NW.js root normalization in the CLI, dedicated `WebActivity` in the Android host shell with hardware-accelerated WebView, mobile Web compatibility shims (Node path, NW.js window, Greenworks/Steamworks), and storage persistence via IndexedDB.

**Architecture:** Ingest and compatibility analysis detect deployed NW.js packages (`package.json` pointing to `www/index.html` or presence of `www/js/rpg_core.js`) and normalize the content root to `www/`, stripping Windows desktop binary artifacts (`*.dll`, `Game.exe`). The build planner routes Web runtime families to `WebActivity` in the Android shell. At runtime, `WebActivity` hosts the game via local origin `https://game.local/` (served from `game.pack` via `VfsResourceInterceptor`), injects mobile shims, and overlays `vpad.js` virtual controls.

**Tech Stack:** Rust (cargo workspace: `cli`, `core`, `engines/rpgmaker_web`), Kotlin (Android API 26+), Android `WebView` (Chromium WebGL/WebAudio), JavaScript (ES5/ES6 compatibility shims, IndexedDB storage adapter).

**Spec:** `Docs/TECHNICAL_SPEC_V1.md` (Section 7.4 & 7.5), `Docs/ARCHITECTURE.md` (Web Runtime Model), `Docs/CONTENT_VFS_SECURITY.md` (Section 11).

## Global Constraints
- Target minSdk: 26, targetSdk: 34.
- Engine family: `RPG Maker Web` (`RuntimeFamily::WebRpgMaker` for MV and MZ).
- Packaging normalization: Desktop NW.js DLLs and binaries (`nw.dll`, `node.dll`, `ffmpeg.dll`, `d3dcompiler_47.dll`) must NOT be packaged into the Android APK.
- Clean mobile storage: `StorageManager` must save to persistent `IndexedDB` / `localStorage` without crashing on missing Node `fs`.
- Error taxonomy domains: `DET-xxx`, `CMP-xxx`, `WEB-xxx`, `AND-xxx`.
- Source game remains strictly immutable.

---

### Task 1: Deployed NW.js Normalization in `RpgMakerWebAnalyzer` and Ingest

**Files:**
- Modify: `engines/rpgmaker_web/src/scanner.rs:20-60`
- Modify: `cli/src/main.rs:410-440`
- Test: `tests/integration/detection_test.rs`
- Test: `tests/integration/compatibility_test.rs`

**Interfaces:**
- Consumes: `GameSourceIndex` with files in `www/` and desktop DLLs in root.
- Produces: `RpgMakerWebAnalyzer::is_nwjs_wrapper(&source) -> bool`. When true, root-level desktop DLLs are excluded from `CMP-102` fatal errors; CLI packages the inner `www/` content root.

- [ ] **Step 1: Write the failing unit/integration test**

In `tests/integration/compatibility_test.rs`:

```rust
#[test]
fn test_rpgmaker_mv_nwjs_wrapper_normalizes_desktop_dlls() {
    let fixture = setup_mv_nwjs_fixture(); // Fixture containing root nw.dll + www/index.html + www/js/rpg_core.js
    let index = GameSourceIndex::build(&fixture).unwrap();
    let detector = RpgMakerMvDetector;
    let candidate = detector.probe(&index);
    assert_eq!(candidate.confidence, ConfidenceLevel::Exact);

    let analyzer = RpgMakerWebAnalyzer;
    let report = analyzer.analyze(&index, &candidate.engine);
    // Desktop DLLs in root should NOT trigger fatal CMP-102 when www/ is the deployed web root
    assert_ne!(report.status, CompatibilityStatus::UnsupportedDependency);
    assert!(report.diagnostics.iter().all(|d| d.code != "CMP-102"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test integration_compatibility test_rpgmaker_mv_nwjs_wrapper_normalizes_desktop_dlls`
Expected: FAIL with `CMP-102` found for `nw.dll`.

- [ ] **Step 3: Implement NW.js wrapper isolation in `scanner.rs`**

In `engines/rpgmaker_web/src/scanner.rs`:

```rust
        let is_nwjs_wrapper = source.has_file("www/index.html")
            || source.has_file("www/js/rpg_core.js")
            || source.has_file("www/js/rmmz_core.js");

        // 1. Scan for forbidden native files (.node, .dll, .so)
        for (path, file) in &source.files {
            // Ignore desktop NW.js binary artifacts outside the web content root
            if is_nwjs_wrapper && !path.starts_with("www/") {
                continue;
            }

            if path.ends_with(".node") { ... }
            if path.ends_with(".dll") || path.ends_with(".so") { ... }
        }
```

- [ ] **Step 4: Update `cli/src/main.rs` to normalize content packaging path**

In `cli/src/main.rs`:
If the detected engine is `RpgMakerMv` or `RpgMakerMz` and `path.join("www").is_dir()`, set the content packaging directory to `path.join("www")`:

```rust
    let content_source_dir = if (engine_id == EngineId::RpgMakerMv || engine_id == EngineId::RpgMakerMz)
        && path.join("www/index.html").exists()
    {
        println!("Detected deployed NW.js wrapper; normalizing content root to 'www/'");
        path.join("www")
    } else {
        path.to_path_buf()
    };
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test integration_compatibility` and `cargo test --test integration_detection`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add engines/rpgmaker_web/src/scanner.rs cli/src/main.rs tests/integration/compatibility_test.rs
git commit -m "feat(web): normalize NW.js desktop wrapper and exclude root DLLs from compatibility check"
```

---

### Task 2: Android `WebActivity` Implementation (Kotlin)

**Files:**
- Create: `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`
- Modify: `core/src/build_plan/generator.rs:185-200`
- Test: `tests/integration/build_test.rs`

**Interfaces:**
- Consumes: `game.pack` (or extracted web assets), `VfsResourceInterceptor`, `WebResourceResponse`.
- Produces: `WebActivity` hosting full-screen hardware-accelerated `WebView`, intercepting `https://game.local/*`, and attaching `vpad.js` virtual controller.

- [ ] **Step 1: Write integration test for WebActivity manifest generation**

In `tests/integration/build_test.rs`:

```rust
#[test]
fn test_generate_manifest_routes_web_family_to_web_activity() {
    let manifest = build_test_manifest_for_engine(EngineId::RpgMakerMv);
    let xml = AndroidProjectGenerator::generate_manifest_for_build(&manifest).unwrap();
    assert!(xml.contains("android:name=\"io.runtimepack.host.WebActivity\""));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test integration_build test_generate_manifest_routes_web_family_to_web_activity`
Expected: FAIL.

- [ ] **Step 3: Update `core/src/build_plan/generator.rs`**

In `core/src/build_plan/generator.rs`:

```rust
        let main_activity = match manifest.runtime.family.as_str() {
            "RGSS" => "io.runtimepack.host.RgssActivity",
            "RPG Maker Web" | "web" => "io.runtimepack.host.WebActivity",
            _ => "io.runtimepack.host.BootstrapActivity",
        };
```

- [ ] **Step 4: Implement `WebActivity.kt`**

Create `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`:
- Full-screen translucent / black window background.
- Instantiates `gameWebView` (`MATCH_PARENT`, WebGL enabled, WebAudio enabled, hardware accelerated).
- Initializes `GpackStreamReader` and `VfsResourceInterceptor`.
- Loads `https://game.local/index.html`.
- Attaches `overlayWebView` on top with `vpad.js` and `cheat_menu.js`.
- Dispatches hardware keyboard keys to `gameWebView` via JavaScript `KeyEvent` injection.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test integration_build` and `.\gradlew.bat testDebugUnitTest`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add core/src/build_plan/generator.rs android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt tests/integration/build_test.rs
git commit -m "feat(web): implement WebActivity and route RPG Maker Web manifest to WebActivity"
```

---

### Task 3: Mobile Web Shims (Node.js, NW.js, Steamworks, Storage)

**Files:**
- Create: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`
- Test: `android/shell/app/src/test/js/web_shims.test.js`

**Interfaces:**
- Injected into `gameWebView` on `onPageStarted` / prepended to `index.html`.
- Implements:
  - `window.require('path')`: `join`, `dirname`, `basename`, `extname`, `resolve`, `normalize`, `delimiter`, `sep: '/'`.
  - `window.require('nw.gui')` / `window.nw`: `Window.get()`, `App.manifest`.
  - `window.Greenworks`: Steamworks stub (`init: () => true`, `isSteamRunning: () => false`, `getScreenLanguage: () => "en"`).
  - `StorageManager.isLocalMode = () => false`: Forces MV/MZ to use `IndexedDB` / `localStorage` rather than attempting Node `fs.writeFile`.

- [ ] **Step 1: Write failing Node.js unit test for shims**

Create `android/shell/app/src/test/js/web_shims.test.js` validating `path.join`, `nw.Window.get()`, `Greenworks.init()`, and `StorageManager.isLocalMode()`.

- [ ] **Step 2: Run test to verify it fails**

Run: `node android/shell/app/src/test/js/web_shims.test.js`
Expected: FAIL with "file not found".

- [ ] **Step 3: Implement `runtimepack_web_shims.js`**

Implement the lightweight, zero-dependency shims file in `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`.

- [ ] **Step 4: Run test to verify it passes**

Run: `node android/shell/app/src/test/js/web_shims.test.js`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js android/shell/app/src/test/js/web_shims.test.js
git commit -m "feat(web): add runtimepack_web_shims for Node, NW.js, Steamworks, and mobile storage"
```

---

### Task 4: In-Game Virtual Gamepad Integration for MV/MZ

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`
- Modify: `android/shell/app/src/main/assets/overlay/vpad.js`

**Interfaces:**
- Consumes: Virtual gamepad tap/touch events from `vpad.js`.
- Produces: Injects synthetic `KeyboardEvent` (`keydown`/`keyup`) into the underlying game DOM window.

- [ ] **Step 1: Verify JS key event dispatch bridge in `WebActivity.kt`**

Add JavaScript bridge method:
```kotlin
@JavascriptInterface
fun onKeyDown(jsKeyCode: Int) {
    runOnUiThread {
        gameWebView.evaluateJavascript("window.dispatchEvent(new KeyboardEvent('keydown', {keyCode: $jsKeyCode, which: $jsKeyCode, bubbles: true}));", null)
    }
}
```

- [ ] **Step 2: Verify `KeyMappingTest` and Android build**

Run: `.\gradlew.bat testDebugUnitTest`
Expected: BUILD SUCCESSFUL.

- [ ] **Step 3: Commit**

```bash
git add android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt android/shell/app/src/main/assets/overlay/vpad.js
git commit -m "feat(web): bridge virtual gamepad events to game WebView DOM window"
```

---

### Task 5: End-to-End Build & Playtest Verification with Fear & Hunger 2: Termina

**Files:**
- Execute: `cargo run --bin runtimepack -- build "Raw-Game\Fear & Hunger 2 Termina" -o dist/termina --package com.example.termina --name "Termina"`
- Verify: `dist/termina/Termina.apk`
- Test: Install and launch on MuMu Player (`127.0.0.1:16384`).
- Document: Create `Docs/test_ports/rpgmv_fear_and_hunger_2_termina/report.md`.

- [ ] **Step 1: Run `runtimepack inspect` on Fear & Hunger 2 Termina**

Verify it normalizes to `www/`, detects `RPG Maker MV`, status `COMPATIBLE` or `COMPATIBLE_WITH_SHIMS`, with 0 fatal errors.

- [ ] **Step 2: Run `runtimepack build`**

Generate `dist/termina/Termina.apk`. Confirm exit status 0 and verify APK size (omitting 1.4 GB Windows DLLs).

- [ ] **Step 3: Install on MuMu Player emulator and verify in-game**

Install APK, boot to Title Screen, start New Game, test train/map movement, virtual gamepad, and save.

- [ ] **Step 4: Create playtest report**

Save findings to `Docs/test_ports/rpgmv_fear_and_hunger_2_termina/report.md` and check item in `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.

- [ ] **Step 5: Commit**

```bash
git add Docs/test_ports/rpgmv_fear_and_hunger_2_termina/report.md Docs/MULTI_ENGINE_VALIDATION_MATRIX.md
git commit -m "docs: record Fear & Hunger 2 Termina port and verification report"
```
