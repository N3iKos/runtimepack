# Task 3 Fix 1 Report: Runtime Integrity Lifecycle and Release Hash Validation

**Target Commit:** `64ccac0` (fixes `54f2c2c`)  
**Implementer:** Implementer Sub-Agent  
**Date:** 2026-09-30  

---

## 1. Summary of Changes

### F1: SuperNotCalledException on early return in `RgssActivity.kt`
- **Location:** `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`
- **Fix:** Moved `if (!RuntimeIntegrity.checkOrShowCorrupted(this)) return` to line 468, directly after `super.onCreate(savedInstanceState)` and preceding overlay attachment.
- **Cross-check:** Verified that `WebActivity.kt` (line 272) and `RenpyActivity.kt` (line 203) both call `super.onCreate(savedInstanceState)` before invoking `RuntimeIntegrity.checkOrShowCorrupted(this)`.

### F2: Empty expected hash bypass in release mode
- **Location:** `android/shell/app/src/main/java/io/runtimepack/host/RuntimeIntegrity.kt`
- **Fix:** In `verify()`, if `flavor == "release"` and `expected.isEmpty()`, immediately fail closed by returning:
  `IntegrityResult.Unreadable("AND-403", "Expected integrity hash is missing in release build")`
  with error log.
- **Unit Test:** Added `testVerifyReleaseFlavorEmptyExpectedHashReturnsAnd403` to `RuntimeIntegrityTest.kt` asserting that `verify(null)` with `BUILD_FLAVOR = "release"` and empty expected hash returns `IntegrityResult.Unreadable` with code `"AND-403"` and detail `"Expected integrity hash is missing in release build"`.

---

## 2. Files Modified

1. `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`
2. `android/shell/app/src/main/java/io/runtimepack/host/RuntimeIntegrity.kt`
3. `android/shell/app/src/test/java/io/runtimepack/host/RuntimeIntegrityTest.kt`

---

## 3. Test Verification

1. **Rust Workspace Tests:**
   ```powershell
   cargo test --workspace
   ```
   - Result: 276 tests passed; 0 failed.

2. **Android Host Shell Compilation and Unit Tests:**
   ```powershell
   cd android\shell; .\gradlew.bat :app:compileDebugKotlin :app:testDebugUnitTest --console=plain
   ```
   - Result: `BUILD SUCCESSFUL in 8s`, 24 actionable tasks (all unit tests passed including new `testVerifyReleaseFlavorEmptyExpectedHashReturnsAnd403`).

3. **Web Shims Unit Tests:**
   ```powershell
   node app/src/test/js/web_shims.test.js
   ```
   - Result: All Mobile Web Shims tests passed successfully.

---

## 4. Git Commit Details

- **Commit:** `64ccac0`
- **Message:** `fix(runtime): call super.onCreate before integrity check and fail-closed on empty expected hash in release`
- **Files Staged & Committed:**
  - `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`
  - `android/shell/app/src/main/java/io/runtimepack/host/RuntimeIntegrity.kt`
  - `android/shell/app/src/test/java/io/runtimepack/host/RuntimeIntegrityTest.kt`
