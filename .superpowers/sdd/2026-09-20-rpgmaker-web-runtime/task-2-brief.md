# Task 2: Android WebActivity Implementation (Kotlin)

## Goal
Implement `io.runtimepack.host.WebActivity` in the Android host shell, wire `generator.rs` to route `RPG Maker Web` projects to `WebActivity`, and provide full-screen hardware-accelerated WebView execution with local VFS streaming and input dispatching.

## Requirements
1. In `core/src/build_plan/generator.rs`:
   - In `emit_project` (around lines 187–191):
     ```rust
     let main_activity = match manifest.runtime.family.as_str() {
         "RGSS" => "io.runtimepack.host.RgssActivity",
         "RPG Maker Web" | "web" | "Tyrano Web" => "io.runtimepack.host.WebActivity",
         _ => "io.runtimepack.host.BootstrapActivity",
     };
     ```
2. In `tests/integration/build_test.rs`:
   - Add integration test `test_generate_manifest_routes_web_family_to_web_activity`:
     Verify that `manifest.runtime.family == "RPG Maker Web"` routes `android:name` in `AndroidManifest.xml` to `io.runtimepack.host.WebActivity`.
3. In `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`:
   - Create `WebActivity : Activity()`:
     - `gameWebView`: Hardware accelerated, transparent/black background.
     - Settings:
       - `javaScriptEnabled = true`
       - `domStorageEnabled = true`
       - `databaseEnabled = true`
       - `mediaPlaybackRequiresUserGesture = false`
       - `allowFileAccess = true`
       - `allowContentAccess = true`
       - `isVerticalScrollBarEnabled = false`
       - `isHorizontalScrollBarEnabled = false`
       - `overScrollMode = View.OVER_SCROLL_NEVER`
     - Extract / prepare `game.pack` using `GpackExtractor`:
       If loose extraction is used, extract to `filesDir/game`. If `isEncryptedStreamingMode`, prepare streaming container.
     - Setup `GpackStreamReader` and `VfsResourceInterceptor`.
     - `WebViewClient`:
       `shouldInterceptRequest` forwards to `vfsInterceptor.intercept(url)`.
     - Layout: `RelativeLayout` adding `gameWebView` underneath and `overlayWebView` on top with `vpad.js` and `cheat_menu.js`.
     - Hardware keyboard dispatching in `dispatchKeyEvent`:
       Forward keydown/keyup events to `gameWebView` by evaluating `window.dispatchEvent(new KeyboardEvent('keydown'/'keyup', {keyCode: ...}))`.
     - Lifecycle methods: `onPause`, `onResume`, `onDestroy`.
4. Run tests:
   `cargo test` and `.\gradlew.bat testDebugUnitTest` in `android/shell`.
5. Write execution report to `.superpowers/sdd/2026-09-20-rpgmaker-web-runtime/task-2-report.md`.
