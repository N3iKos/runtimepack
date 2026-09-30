# Task 3 Code Review: Startup Integrity Gate & RUNTIME_CORRUPTED

**Target Commit:** `54f2c2cba7db0b83d79b7f96f88f3f5e58ae005f`  
**Reviewer:** Code Reviewer Sub-Agent  
**Date:** 2026-09-30  

---

## 1. Verdict

- **Spec Verdict:** ✅
- **Quality Verdict:** Needs fixes (🔴 `REQUEST CHANGES`)

---

## 2. Executive Summary

Task 3 implements the build-time deterministic SHA-256 hash generator in Rust (`core/src/build_plan/generator.rs`) and the runtime verification gate in Kotlin (`RuntimeIntegrity.kt`).
The deterministic hashing algorithm matches byte-for-byte across both environments with verified test vectors and sorting logic.
However, there is an Android lifecycle defect in `RgssActivity.kt` where early return before `super.onCreate()` will trigger `android.app.SuperNotCalledException`, as well as a bypass case in `RuntimeIntegrity.kt` where an empty expected hash string in release mode silently passes verification.

---

## 3. Findings Grouped by Severity

### High Severity

#### F1. `SuperNotCalledException` on early return in `RgssActivity.kt`
- **Location:** `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt:446`
- **Issue:**
  In `RgssActivity.kt`, `if (!RuntimeIntegrity.checkOrShowCorrupted(this)) return` is invoked at the very start of `onCreate()`, but `super.onCreate(savedInstanceState)` is not called until line 469. If the integrity check fails (`checkOrShowCorrupted` returns `false`), the activity terminates its `onCreate()` without calling `super.onCreate(savedInstanceState)`.
  The Android framework enforces that `super.onCreate()` must be called during activity creation; failing to call it causes the runtime to immediately throw `android.app.SuperNotCalledException`. This crashes the process before the user can see or interact with the `APPLICATION CORRUPTED` screen. Note that `WebActivity.kt` and `RenpyActivity.kt` both correctly call `super.onCreate(savedInstanceState)` before invoking `checkOrShowCorrupted`.
- **Suggested Fix:**
  Call `super.onCreate(savedInstanceState)` before invoking `checkOrShowCorrupted`, or ensure `super.onCreate(savedInstanceState)` is called before returning:
  ```kotlin
  override fun onCreate(savedInstanceState: Bundle?) {
      super.onCreate(savedInstanceState)
      if (!RuntimeIntegrity.checkOrShowCorrupted(this)) return

      // Set transparent window background so SurfaceView beneath is visible
      window.setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))
      AssetAuditService.init(filesDir, "rgss", this)
      ...
  ```

---

### Medium Severity

#### F2. Empty expected hash silently passes in release mode
- **Location:** `android/shell/app/src/main/java/io/runtimepack/host/RuntimeIntegrity.kt:97-102`
- **Issue:**
  In `RuntimeIntegrity.verify()`:
  ```kotlin
  if (expected.isNotEmpty() && !actual.equals(expected, ignoreCase = true)) {
      Log.e(TAG, "[RuntimePack] AND-401 Runtime integrity mismatch: expected $expected, got $actual")
      return IntegrityResult.Mismatch(expected, actual)
  }
  return IntegrityResult.Ok(actual)
  ```
  If `expected` is empty (`""`) in release mode (e.g. if hash generation failed or was stripped/tampered with in `RuntimeBuildInfo.kt`), `expected.isNotEmpty()` evaluates to `false`, causing the function to return `IntegrityResult.Ok(actual)`. Per the specification ("Jika hash mismatch atau hilang, tampilkan layar deterministik RUNTIME CORRUPTED"), a missing or blank hash in release mode must be rejected as an integrity violation (`AND-401`).
- **Suggested Fix:**
  In release mode, reject missing or empty expected hash values:
  ```kotlin
  if (expected.isEmpty() || !actual.equals(expected, ignoreCase = true)) {
      Log.e(TAG, "[RuntimePack] AND-401 Runtime integrity mismatch: expected $expected, got $actual")
      return IntegrityResult.Mismatch(expected, actual)
  }
  ```

---

### Low Severity / Polish

#### F3. Plain asset traversal unbounded recursion
- **Location:** `android/shell/app/src/main/java/io/runtimepack/host/RuntimeIntegrity.kt:56-75`
- **Issue:**
  In the fallback path when GPACK is not present, `collectAssets` uses unconstrained recursion across `context.assets.list(path)`. While APK assets are typically shallow, `AssetManager.list()` is an expensive I/O operation. Adding a depth guard or handling errors explicitly is advisable.

---

## 4. Checklist Verification

- [x] **Deterministic hashing algorithm:** Rust (`core/src/build_plan/generator.rs`) and Kotlin (`RuntimeIntegrity.kt`) match byte-for-byte with canonical lowercase sorting and null byte separators (`0x00`).
- [x] **Emission of `RuntimeBuildInfo.kt` and `build-manifest.json`:** Verified in `tests/integration/build_test.rs`.
- [x] **Beta flavor bypass:** Non-release flavor cleanly logs INFO `[RuntimePack] (Beta) Runtime integrity: ...` and returns `IntegrityResult.Ok`.
- [x] **Clean exit:** `activity_corrupted.xml` Exit button cleanly terminates app via `finishAffinity()` and `exitProcess(1)`.
- [x] **Test suite execution:** All 67 `build_test.rs` integration tests, Android Kotlin unit tests (8 tests in `RuntimeIntegrityTest`), and Web Shims tests pass.
- [ ] **Android lifecycle contract in `RgssActivity.kt`:** Failed due to missing `super.onCreate` before early return (F1).

---

## 5. Summary Score

- **Maintainability:** 9/10
- **Readability:** 9/10
- **Security Posture:** 8/10 (Requires fix for F2 empty hash bypass)
- **Robustness:** 7/10 (Requires fix for F1 `SuperNotCalledException` in `RgssActivity`)
