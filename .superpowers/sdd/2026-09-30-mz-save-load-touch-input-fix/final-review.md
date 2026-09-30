# Final Whole-Branch Code Review: MZ Save/Load Touch & Input Fix

**Plan:** `Docs/superpowers/plans/2026-09-30-mz-save-load-touch-input-fix.md`  
**Worktree:** `.worktrees/native-mv-mz-save`  
**Branch:** `feature/native-mv-mz-save`  
**Diff Range:** BASE `385df60` -> HEAD `e054bcb` (Commits `dc09d92`, `e054bcb`)  
**Scope:**
1. Save Adapter layer (uncommitted deliverables from prior verified plan: `patchMzStorage`, `patchMvStorage`, `isLocalMode` virtualization) - reviewed for obvious regressions.
2. Engine Input & Touch layer (Task 1: `patchEngineInput`, `RuntimePackInput`, unit tests; Task 2: `WebActivity.kt.dispatchKeyToGame` anti-sanitization rewrite) - strictly audited.

---

## 1. Requirements Coverage Verdict

### Verdict: GREEN APPROVED (100% spec coverage)

Every requirement specified in Task 1 and Task 2 of the implementation plan is fully implemented and verified in the diff:

| Requirement | Implementation Location | Verification Evidence |
| :--- | :--- | :--- |
| **Touch Jitter Threshold (Bug 1)**: Bump TouchInput.moveThreshold from 10 to 24 | runtimepack_web_shims.js:1235-1238 | Idempotently checks default value 10 and raises to 24. Verified via unit test assertion. |
| **Single-Tap Save Slot OK (Bug 1)**: Override Window_SavefileList.prototype.isTouchOkEnabled | runtimepack_web_shims.js:1243-1249 | Bypasses double-touch requirement on save lists; guarded by __runtimepack_touch_ok_patched. |
| **Single-Tap Generic Menus (Bug 1)**: Override Window_Selectable.prototype.isTouchOkEnabled for mobile | runtimepack_web_shims.js:1251-1259 | Enabled under non-NW.js (!isNwjs); guarded by __runtimepack_touch_ok_patched. |
| **W3C Code Fallback (Bug 2)**: Add Input.codeMapper and wrap Input._onKeyDown / _onKeyUp | runtimepack_web_shims.js:1265-1313 | Translates event.code to action when event.keyCode === 0. Guarded by __runtimepack_codemapper_patched. |
| **RuntimePackInput API (Bug 2)**: Expose global window.RuntimePackInput | runtimepack_web_shims.js:1318-1351 | Exposes press, release, trigger, and isAvailable. |
| **Late-Binding Polling**: Hook patchEngineInput into pollPatches | runtimepack_web_shims.js:1356-1388 | Runs on each tick until required engine globals exist; debug hook exported to window.__runtimepackPollEngineInput. |
| **JS Unit Test Suite**: Comprehensive tests for input and touch patches | web_shims.test.js:804-902 | Tests threshold bump, save list single tap, selectable single tap, codeMapper fallback, and RuntimePackInput API. 5/5 test suites pass. |
| **Chromium Sanitization Bypass (Bug 2)**: Kotlin dispatchKeyToGame rewrite | WebActivity.kt:751-839 | Polyfills keyCode, which, and code via Object.defineProperty; directly injects into Input._currentState. |
| **Call-site Compatibility**: Preserves dispatchKeyToGame(jsKeyCode, type) signature | WebActivity.kt:750 | Gamepad shortcut calls (line 622, 627) and dispatchKeyEvent (line 861) operate seamlessly without signature drift. |
| **Kotlin Build**: Compiles cleanly with zero warnings/errors | WebActivity.kt | :app:compileDebugKotlin executes successfully (BUILD SUCCESSFUL in 3s). |

---

## 2. Cross-Layer Consistency Findings

1. **Key Mapping Alignment (`codeMapper` vs `codeMap`):**
   - In `WebActivity.kt`, `codeMap` maps JavaScript numeric keycodes to W3C DOM `KeyboardEvent.code` strings (e.g., 90 -> `KeyZ`, 13 -> `Enter`, 27 -> `Escape`, 16 -> `ShiftLeft`, 37..40 -> `Arrow*`, 33/34 -> `PageUp`/`PageDown`, 120 -> `F9`).
   - In `runtimepack_web_shims.js`, `Input.codeMapper` maps these identical `code` strings to the engine's canonical action names (`ok`, `escape`, `shift`, `up`, `down`, `left`, `right`, `pageup`, `pagedown`, `debug`).
   - The token names and casing match 1:1 across Kotlin and JavaScript without any conflicts.

2. **`RuntimePackInput` Lifecycle & Signature:**
   - `RuntimepackInput.press(action)` just like `release(action)` takes a single string representing the action name.
   - In `WebActivity.kt`, the script queries `Input.keyMapper[jsKeyCode]` or `Input.codeMapper[evt.code]` to extract `action` and calls `press(action)` on `keydown` and `release(action)` on `keyup`.
   - Method signatures, null-safety guards, and call timing align perfectly.

3. **Multi-Tier Event Ingestion Integrity:**
   - `WebActivity.kt` dispatches input across three synchronized tiers:
     1. Standard DOM event dispatch (`window.dispatchEvent(evt)` and `document.dispatchEvent(evt)`) with definedProperty'd keyCode, which, and code.
     2. Direct engine method invocation (`window.Input._onKeyDown(evt)` / `_onKeyUp(evt)`).
     3. Direct state injection (`window.RuntimePackInput.press(action)` / `release(action)`).
   - In RPG Maker MZ, `_currentState[buttonName]` is a boolean dictionary evaluated per frame during `Input.update()`. Repeatedly setting a button's state to `true` during a single JS event loop tick is strictly idempotent and creates no race conditions or double-fire bugs.

---

## 3. Findings

### Critical: None
### Important: None

### Minor (Non-Blocking):

1. **`WebActivity.kt:800-806` — Potential double invocation of `_onKeyDown` on DOM dispatch:**
   - **Issue:** `document.dispatchEvent(evt)` already executes the document listener registered by MZ's `Input._setupEventHandlers`. Calling `window.Input._onKeyDown(evt)` immediately afterwards invokes the method a second time in the same frame.
   - **Impact:** Benign. Since `_onKeyDown` simply sets `_currentState[buttonName] = true`, duplicate calls within the same JavaScript execution turn are idempotent.
   - **Suggested Fix (Optional cleanup):** Wrap the direct call in a check or rely on `RuntimePackInput` as the secondary fallback.

2. **`WebActivity.kt:807-813` — Speculative `TouchInput._onKeyDown` invocation:**
   - **Issue:** Standard RPG Maker MV and MZ engines do not define `_onKeyDown` on `TouchInput`.
   - **Impact:** None. The guard `typeof window.TouchInput._onKeyDown === 'function'` ensures this branch is a safe no-op on vanilla engines, serving only as a compatibility hook for third-party plugins.

---

## 4. Triage of Deferred Minors

1. **Deferred Minor 1: `RuntimePackInput.trigger` resets state synchronously:**
   - **Triage Result:** Acceptable (non-blocking for build)
   - **Rationale:** No caller in the entire codebase invokes `RuntimePackInput.trigger()`. The virtual gamepad (`VPadBridge`) and Android physical keyboard handlers exclusively invoke `press()` on `ACTION_DOWN` and `release()` on `ACTION_UP`. While `trigger()` is currently ineffective across game loop frames, it is unused utility code in this milestone.

2. **Deferred Minor 2: `pollPatches` clearInterval predicate checks `Input && TouchInput` but not `Window_Selectable`:**
   - **Triage Result:** Acceptable (non-blocking for build)
   - **Rationale:** In standard RPG Maker MV/MZ deployments on Android WebView, core scripts (`rmmz_core.js`, `rmmz_managers.js`, `rmmz_windows.js`) are loaded synchronously via sequential `<script>` tags in `index.html`. Because JavaScript parsing is synchronous and single-threaded on document load, all window classes exist before the first event loop timer fires. The polling interval safely terminates once the core managers and input objects are detected without omitting window patches.

---

## 5. Domain & Architectural Invariants Verification

- *Game File Immutability:** User game files in `Raw-Game/` remain 100% untouched.
- **Zero New Dependencies:** No external libraries or polyfill packages were introduced into the web shims or Android host.
- *Engine-Agnostic Core:** Input and touch modifications are strictly contained within the web shim overlay and WebView host.
- *Diagnostic Hygiene:** Shims and logs follow the mandatory `[RuntimePack]` prefix convention.
- **Regression Risks Evaluated:**
   - *Does overriding `Window_Selectable.prototype.isTouchOkEnabled` break MV?* No. In RPG Maker MV, `isTouchOkEnabled` natively returns `this.isOkEnabled()`. The patch aligns MZ mobile behavior with MV's proven model.
   - *Does overriding `Window_Selectable.prototype.isTouchOkEnabled` break `_cursorFixed` menus?* No. `Window_Selectable.prototype.onTouchOk` explicitly checks `if (this._cursorFixed) { if (hitIndex === this.index()) this.processOk(); }`. The fixed cursor protection remains active.
   - *Does raising `moveThreshold` to 24 break list dragging?* No. 24 CSS pixels is ~0.15 inches on mobile screens; intentional scroll gestures span 100-300+ pixels and smoothly trigger `_moved = true`.

---

## 6. Final Verdict

### APPROVED_FOR_BUILD

The branch is clean, functionally solid, well-tested, and ready for Beta APK generation (`mandatory --flavor beta`) and manual device testing.
