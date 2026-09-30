# Task 1 Code Review: Shim Patch — Single-Tap Load & Touch Threshold (`runtimepack_web_shims.js`)

## 1. Spec Compliance Verdict: ✅ FULLY COMPLIANT

All requirements from `task-1-brief.md` have been implemented faithfully and accurately:

- [x] **File placement**: Added `patchEngineInput()` after `patchSceneManager()` (line 1229) and before `pollPatches()` (line 1361).
- [x] **Touch jitter threshold**: `TouchInput.moveThreshold` raised from 10 to 24 when default is detected (idempotent).
- [x] **Single-tap OK on save list**: `Window_SavefileList.prototype.isTouchOkEnabled` overridden to return `this.isOkEnabled()` without `_doubleTouch`, guarded by `__runtimepack_touch_ok_patched`.
- [x] **Single-tap OK on generic selectables**: `Window_Selectable.prototype.isTouchOkEnabled` overridden to return `this.isOkEnabled()` when `!isNwjs`, guarded by `__runtimepack_touch_ok_patched`.
- [x] **Input.codeMapper W3C fallback**: Comprehensive `codeMapper` table added for `Enter`, `KeyZ`, `Escape`, arrows, etc. Wrapped `_onKeyDown` and `_onKeyUp` to inspect `event.code` when `!keyCode` (W3C standard in modern Chromium). Guarded by `__runtimepack_codemapper_patched`.
- [x] **RuntimePackInput API**: Global `window.RuntimePackInput` and `root.RuntimePackInput` exposed with `press`, `release`, `trigger`, and `isAvailable`.
- [x] **Late-binding & polling integration**: `patchEngineInput()` invoked inside `pollPatches()` if `Input`, `TouchInput`, or `Window_Selectable` exist. Polling termination condition updated to require `root.Input && root.TouchInput`.
- [x] **Manual hook exposed**: `root.__runtimepackPollEngineInput` and `window.__runtimepackPollEngineInput` exported for sandbox testing.
- [x] **Test suite**: Verified exact test suite transcribed to `android/shell/app/src/test/js/web_shims.test.js` (lines 804-902), testing threshold bump, single-tap OK on save list & generic selectables, codeMapper key down/up fallback, and `RuntimePackInput` press/release/availability.
- [x] **Zero external dependencies**: Pure vanilla JavaScript.
- [x] **Scope and Git discipline**: Commit `dc09d92` contains only `runtimepack_web_shims.js` and `web_shims.test.js`. No Kotlin files or user game files modified.

---

## 2. Issues Found

### Minor 1: `RuntimePackInput.trigger()` resets synchronously with empty `setTimeout`
- **Location**: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js:1337-1344`
- **Issue**: In `RuntimePackInput.trigger(buttonName)`, `this.press(buttonName)` sets `I._currentState[buttonName] = true`, but is immediately followed by synchronous reset `I._currentState[buttonName] = false;` within the same call stack, accompanied by an empty `setTimeout(function() {}, 0);`. Consequently, the engine's game loop tick (`Input.update()`) will never observe the `true` state if `trigger()` is called.
  *Note*: This was transcribed verbatim from the brief's Step 3 snippet, and Task 2 (`WebActivity.kt`) only uses `press()` on `ACTION_DOWN` and `release()` on `ACTION_UP` (not `trigger()`). Non-blocking for current plan.
- **Suggested Fix**:
  ```javascript
  trigger: function(buttonName) {
      this.press(buttonName);
      var I = root.Input;
      if (typeof requestAnimationFrame === "function") {
          requestAnimationFrame(function() {
              if (I && I._currentState) I._currentState[buttonName] = false;
          });
      } else {
          setTimeout(function() {
              if (I && I._currentState) I._currentState[buttonName] = false;
          }, 16);
      }
  },
  ```

### Minor 2: Polling clearInterval predicate omits `Window_Selectable`
- **Location**: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js:1386-1389`
- **Issue**: `clearInterval(patchTimer)` checks `root.StorageManager && root.AudioManager && root.Graphics && root.SceneManager && root.Input && root.TouchInput`. In standard RPG Maker MZ/MV all core scripts run synchronously, so `Window_Selectable` is present on tick 0. In rare cases where an asynchronous plugin loader delays window classes, the polling timer could stop before `Window_Selectable` is patched.
- **Suggested Fix**: Add `root.Window_Selectable` to the predicate if async script loading support is desired.

---

## 3. ⚠️ Cannot Verify From Diff

- **Real WebView touch event timing**: The unit tests verify mock objects in a Node VM sandbox. Verification of real touch latency, gesture cancellation, and DPI scaling under Android WebView will occur in subsequent on-device / emulator QA phases.

---

## 4. Task Quality Verdict: Approved

The implementation is clean, robust, adheres strictly to the plan specification and constraints, introduces zero external dependencies, maintains full backward compatibility with the existing save adapter code, and passes all unit tests cleanly.