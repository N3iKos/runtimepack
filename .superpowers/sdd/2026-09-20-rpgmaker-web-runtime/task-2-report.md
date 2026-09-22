# Task 2 Report: Android WebActivity Implementation (Kotlin)

## Summary
Successfully implemented `io.runtimepack.host.WebActivity` in the Android host shell, wired `generator.rs` to route Web family runtimes (`"RPG Maker Web"`, `"web"`, `"Tyrano Web"`) to `WebActivity`, and added unit and integration test coverage for manifest generation and hardware keycode mapping.

## Modified and Created Files
1. `tests/integration/build_test.rs`:
   - Added integration test `test_generate_manifest_routes_web_family_to_web_activity`, verifying that `manifest.runtime.family == "RPG Maker Web"` routes `android:name` in `AndroidManifest.xml` to `io.runtimepack.host.WebActivity`.
2. `core/src/build_plan/generator.rs`:
   - Updated `emit_project` main activity resolver from binary `if manifest.runtime.family == "RGSS"` to match expression routing `"RPG Maker Web" | "web" | "Tyrano Web"` to `"io.runtimepack.host.WebActivity"`.
3. `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`:
   - Implemented `WebActivity : Activity()` featuring:
     - Hardware-accelerated `gameWebView` with transparent/black background and disabled native scrollbars (`isVerticalScrollBarEnabled = false`, `isHorizontalScrollBarEnabled = false`, `overScrollMode = OVER_SCROLL_NEVER`).
     - Configuration: `javaScriptEnabled = true`, `domStorageEnabled = true`, `databaseEnabled = true`, `mediaPlaybackRequiresUserGesture = false`, `allowFileAccess = true`, `allowContentAccess = true`.
     - Container preparation via `GpackExtractor`: handles both loose extraction to `filesDir/game` and `isEncryptedStreamingMode` preparation.
     - VFS setup: opens `GpackStreamReader` and initializes `VfsResourceInterceptor`, forwarding `shouldInterceptRequest` from `gameWebView` to `vfsInterceptor.intercept(url)`.
     - Layout: `RelativeLayout` layering `gameWebView` underneath and transparent `overlayWebView` on top with `vpad.js` and `cheat_menu.js` bridge (`RuntimePackVPad`).
     - Hardware keyboard event forwarding in `dispatchKeyEvent` dispatching synthetic `KeyboardEvent('keydown'/'keyup')` into `gameWebView` and invoking `Input._onKeyDown` / `_onKeyUp` when available.
     - Full lifecycle hooks: `onPause`, `onResume`, `onDestroy`.
4. `android/shell/app/src/test/java/io/runtimepack/host/KeyMappingTest.kt`:
   - Added unit test `testWebActivityAndroidKeyToJsKeyCode` verifying mapping from Android `KeyEvent` keycodes to JavaScript/DOM keycodes (A-Z, 0-9, Numpad, F1-F12, Arrows, D-Pad, Enter, Escape, Space, Backspace, Modifiers, etc.).

## Verification Results
1. TDD Failing Test:
   - `cargo test --test integration_build test_generate_manifest_routes_web_family_to_web_activity` failed initially with `assertion failed: manifest_xml.contains("android:name=\"io.runtimepack.host.WebActivity\"")`.
2. Post-Implementation Rust Tests:
   - `cargo test --test integration_build test_generate_manifest_routes_web_family_to_web_activity` passed (1/1).
   - `cargo test` passed across workspace (45/45 tests passing).
3. Android Shell Unit Tests:
   - `.\gradlew.bat testDebugUnitTest` in `android/shell` passed (BUILD SUCCESSFUL).

## Status
DONE
