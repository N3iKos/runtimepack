# Task 1: Native Touchscreen & TouchInput Pass-Through in WebActivity.kt

## Requirements
1. In `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`:
   - Override `dispatchTouchEvent(event: MotionEvent): Boolean`.
   - Implement touch pass-through logic:
     - Check if touch is on topbar or interactive overlay modal.
     - When gamepad is hidden (`isGamepadHidden`) or touches occur outside toolbar/active modal:
       Forward touch event directly to `gameWebView?.dispatchTouchEvent(event)`.
     - When virtual gamepad is visible, ensure taps on the map / game area pass through to `gameWebView` while taps on virtual controls are handled by `overlayWebView`.
     - To achieve this cleanly on Android:
       In `WebActivity`, compute if touch coordinates hit `#vpad-topbar` or `#fnh-cheat-overlay`. If not inside active controls, deliver to `gameWebView.dispatchTouchEvent(event)`.
2. In `android/shell/app/src/test/java/io/runtimepack/host/KeyMappingTest.kt`:
   - Add unit test verifying touch bounding box logic.
3. Run tests:
   `.\gradlew.bat testDebugUnitTest` in `android/shell`.
4. Write execution report to `.superpowers/sdd/2026-09-23-beta-diagnostic-and-blacksouls-audit/task-1-report.md`.

## Critical Constraints
- NEVER touch, read, or write any folder named `antigravity` or `.antigravity`.
- Keep source directories in `Raw-Game/` strictly READ-ONLY.
