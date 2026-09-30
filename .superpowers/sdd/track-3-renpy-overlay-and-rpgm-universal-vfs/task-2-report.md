# Task 2 Report: Implement Overlay Attachment and Touch Routing in RenpyActivity

**Status:** DONE

## Overview
Implemented transparent overlay WebView attachment, `RuntimePackVPad` JavascriptInterface, VPad button-to-SDL keycode mapping, selective touch event routing via `dispatchTouchEvent`, and lifecycle bindings (`onPause`, `onResume`, `onDestroy`) in `RenpyActivity`.

## Files Touched
1. `android/shell/app/src/main/java/io/runtimepack/host/RenpyActivity.kt`
   - Added instance state properties: `overlayWebView`, `isGamepadHidden`, `isOverlayModalOpen`, `isTouchCapturedByGame`, and `activeControlHitboxes`.
   - Added `mapVPadButtonToKeyCode` static mapping function to `companion object`.
   - Added `setupOverlayWebView()` attaching transparent WebView with `RuntimePackVPad` JavascriptInterface (`openCommunityLink`, `updateControlHitboxes`, `setGamepadHidden`, `setOverlayModalOpen`, `sendKeyEvent`).
   - Added `setupOverlayWebView()` call inside `onStart()` when `overlayWebView == null` and content is mounted.
   - Implemented `dispatchTouchEvent(event: MotionEvent)` routing non-control touches directly to `mSurface?.dispatchTouchEvent(event)`.
   - Implemented lifecycle forwarders: `onPause()`, `onResume()`, and `onDestroy()`.
2. `android/shell/app/src/test/java/io/runtimepack/host/RenpyActivityTest.kt`
   - Added unit test `testRenpyKeyMappingForGamepadButtons` covering D-pad, action buttons (A, B), Menu, and Dash/Shift mappings.

## Test Evidence
### 1. Failure Confirmation (Red Stage)
Command:
```powershell
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests "io.runtimepack.host.RenpyActivityTest.testRenpyKeyMappingForGamepadButtons"
```
Output:
```
e: file:///D:/02_Development/Projects_2/RPGM-P/android/shell/app/src/test/java/io/runtimepack/host/RenpyActivityTest.kt:362:75 Unresolved reference: mapVPadButtonToKeyCode
...
> Task :app:compileDebugUnitTestKotlin FAILED
BUILD FAILED in 6s
```

### 2. Pass Confirmation (Green Stage)
Command:
```powershell
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests "io.runtimepack.host.RenpyActivityTest.*"
```
Output:
```
> Task :app:compileDebugKotlin
> Task :app:compileDebugUnitTestKotlin
> Task :app:testDebugUnitTest
BUILD SUCCESSFUL in 14s
24 actionable tasks: 6 executed, 18 up-to-date
```

Full unit test suite (`.\android\shell\gradlew.bat -p android/shell testDebugUnitTest`) also passed cleanly:
```
BUILD SUCCESSFUL in 6s
24 actionable tasks: 2 executed, 22 up-to-date
```
