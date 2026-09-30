# Task 5 Report: Hapus Instrumentasi `[RP_LOAD_DEBUG]` + Fail-Loud RT-LOAD-FAIL

## 1. Summary of Execution
- **Task:** Task 5 - Remove `[RP_LOAD_DEBUG]` instrumentation and add fail-loud `RT-LOAD-FAIL` diagnostics.
- **Commit:** `a11fd24eb71e177d30fbc7c01317d405798c45c8`
- **Branch:** `feature/native-mv-mz-save`
- **Status:** COMPLETED

## 2. Changes Implemented
1. **Tests (TDD Cycle):**
   - Added unit test suite `Load Failure Diagnostics (fail-loud RT-LOAD-FAIL)` in `android/shell/app/src/test/js/web_shims.test.js`.
   - Verified initial failure (RED) on missing `[RuntimePack] RT-LOAD-FAIL` diagnostics logging.
   - Tested:
     - `Scene_Load.prototype.onLoadFailure` hook invokes `__runtimepackShowLoadError` with `RT-LOAD-FAIL` while preserving original callback behavior.
     - `DataManager.loadGame` promise rejection interception logs `RT-LOAD-FAIL` and re-throws original error.
     - DOM overlay generation via `__runtimepackShowLoadError` with fixed styling and 8s auto-removal when `document` is available.
     - Non-throwing fallback when DOM `document` is undefined.

2. **Shim Cleanup & Implementation (`runtimepack_web_shims.js`):**
   - Completely removed all temporary `[RP_LOAD_DEBUG]` logging statements (input ok handlers in `Input_._onKeyDown`, `RuntimePackInput.press`, `patchHalfMoveCompat`).
   - Removed temporary `patchLoadDebug()` function and debug state variables.
   - Replaced with permanent fail-loud diagnostics helper `__runtimepackShowLoadError(msg)`.
   - Implemented `patchLoadDiagnostics()` wrapping `Scene_Load.prototype.onLoadFailure` and `DataManager.loadGame`.
   - Exposed `root.__runtimepackPollLoadDiagnostics` / `window.__runtimepackPollLoadDiagnostics` for late-binding test and lifecycle polling.
   - Integrated `patchLoadDiagnostics()` into `pollPatches()`.
   - Preserved HalfMove compatibility (`convertEscapeCharacters` delegation) and input patches (`patchEngineInput`, `RuntimePackInput`).

## 3. Verification & Grep Metrics
- **Remaining `RP_LOAD_DEBUG` matches:** 0
  - Command: `(Select-String -Path android\shell\app\src\main\assets\web_shims\runtimepack_web_shims.js -Pattern "RP_LOAD_DEBUG").Count`
  - Result: `0`
- **HalfMove `convertEscapeCharacters` matches:** 10 (> 0)
  - Command: `(Select-String -Path android\shell\app\src\main\assets\web_shims\runtimepack_web_shims.js -Pattern "convertEscapeCharacters").Count`
  - Result: `10`
- **Unit Test Execution:**
  - Command: `node android/shell/app/src/test/js/web_shims.test.js`
  - Result: All tests PASSED (including Engine Input Patch, HalfMove Compat, and Load Failure Diagnostics).

## 4. Deliverables
- `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
- `android/shell/app/src/test/js/web_shims.test.js`
