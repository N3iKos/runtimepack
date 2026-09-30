# Task 1 Report: Add Hitbox and Touch Filtering Models to RenpyActivity

**Status:** DONE

## Overview
Added pure hit-testing and control touch filtering logic to `RenpyActivity.Companion` along with constructor compatibility on `WebActivity.Companion.ControlHitbox`.

## Files Touched
1. `android/shell/app/src/main/java/io/runtimepack/host/RenpyActivity.kt`
   - Added imports for `ControlHitbox`.
   - Implemented `shouldForwardTouchToGame` and `resolveGestureCapturedByGame` pure static companion functions.
2. `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`
   - Added `constructor(id: String, left: Float, top: Float, right: Float, bottom: Float)` to `ControlHitbox` with optional `id` default parameter for seamless test and caller compatibility.
3. `android/shell/app/src/test/java/io/runtimepack/host/RenpyActivityTest.kt`
   - Added unit test `testShouldForwardTouchToRenpyGameOutsideControls` testing touch forwarding outside controls, touch consumption inside active hitboxes, and modal blocking.

## Test Evidence
### 1. Failure Confirmation (Red Stage)
Command:
```powershell
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests "io.runtimepack.host.RenpyActivityTest.testShouldForwardTouchToRenpyGameOutsideControls"
```
Output:
```
> Task :app:compileDebugUnitTestKotlin
e: file:///D:/02_Development/Projects_2/RPGM-P/android/shell/app/src/test/java/io/runtimepack/host/RenpyActivityTest.kt:321:44 Unresolved reference: shouldForwardTouchToGame
e: file:///D:/02_Development/Projects_2/RPGM-P/android/shell/app/src/test/java/io/runtimepack/host/RenpyActivityTest.kt:334:43 Unresolved reference: shouldForwardTouchToGame
e: file:///D:/02_Development/Projects_2/RPGM-P/android/shell/app/src/test/java/io/runtimepack/host/RenpyActivityTest.kt:347:46 Unresolved reference: shouldForwardTouchToGame

> Task :app:compileDebugUnitTestKotlin FAILED
BUILD FAILED in 32s
```

### 2. Pass Confirmation (Green Stage)
Command:
```powershell
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests "io.runtimepack.host.RenpyActivityTest.*"
```
Output:
```
> Task :app:compileDebugUnitTestKotlin
> Task :app:testDebugUnitTest
BUILD SUCCESSFUL in 18s
24 actionable tasks: 6 executed, 18 up-to-date
```

Regression test `io.runtimepack.host.KeyMappingTest.*` also passed with `BUILD SUCCESSFUL in 7s`.
