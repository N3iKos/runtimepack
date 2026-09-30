# Task 3 Re-Review 1: Startup Integrity Gate & RUNTIME_CORRUPTED

**Target Commit:** `64ccac0` (fixes `54f2c2c`)  
**Reviewer:** Code Reviewer Sub-Agent  
**Date:** 2026-09-30  

---

## 1. Verdict

- **Final Verdict:** 🟢 APPROVE (All findings addressed)

---

## 2. Findings Verification

### Finding 1: Early return before `super.onCreate` in `RgssActivity.kt`
- **Location:** `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt:467-468`
- **Status:** **ADDRESSED**
- **Analysis:**
  `super.onCreate(savedInstanceState)` is now called before the integrity gate.
  Immediately following `super.onCreate`, line 468 evaluates `if (!RuntimeIntegrity.checkOrShowCorrupted(this)) return`.
  If the integrity check fails, `showCorruptedScreen` displays the corrupted screen and `return` exits cleanly without throwing `android.app.SuperNotCalledException`.
  This aligns with the lifecycle implementations in `WebActivity.kt` and `RenpyActivity.kt`.

### Finding 2: Empty expected hash bypass in release mode in `RuntimeIntegrity.kt`
- **Location:** `android/shell/app/src/main/java/io/runtimepack/host/RuntimeIntegrity.kt:92-95`
- **Status:** **ADDRESSED**
- **Analysis:**
  In `RuntimeIntegrity.verify()`, when `flavor.equals("release", ignoreCase = true)` and `expected.isEmpty()`, the code now logs an error and fails closed by returning:
  `IntegrityResult.Unreadable("AND-403", "Expected integrity hash is missing in release build")`.
  Unit test `testVerifyReleaseFlavorEmptyExpectedHashReturnsAnd403` in `RuntimeIntegrityTest.kt` verifies this exact behavior.

---

## 3. New Breakage Assessment

- **Diff Inspection:** `git diff 54f2c2c..64ccac0`
- **New Breakages:** **None**
- **Verification:**
  - Full Android shell test suite passed (`:app:testDebugUnitTest`, `:app:testReleaseUnitTest`).
  - No syntax or lifecycle regressions introduced.

---

## 4. Summary Score

- **Maintainability:** 10/10
- **Readability:** 10/10
- **Security Posture:** 10/10 (Fail-closed on empty expected hash in release mode)
- **Robustness:** 10/10 (Android lifecycle contract properly preserved)
