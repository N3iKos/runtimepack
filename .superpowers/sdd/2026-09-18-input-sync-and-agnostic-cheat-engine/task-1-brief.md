# Task 1 Brief: Direct Native Bridge & VenoraTL Community Link

## Requirements & Scope
- Files to modify:
  - `android/shell/app/src/main/assets/overlay/vpad.js`
  - `android/shell/app/src/main/assets/overlay/index.html`
- File to create & test:
  - `android/shell/app/src/test/js/vpad_bridge.test.js`

### Changes Required:
1. In `android/shell/app/src/main/assets/overlay/vpad.js`:
   - In `pressKey(keyCode)`:
     Directly invoke `window.RuntimePackVPad.onKeyDown(keyCode)` if available:
     ```javascript
     if (typeof window !== "undefined" && window.RuntimePackVPad && window.RuntimePackVPad.onKeyDown) {
         window.RuntimePackVPad.onKeyDown(keyCode);
     }
     ```
   - In `releaseKey(keyCode, forceAll)`:
     Directly invoke `window.RuntimePackVPad.onKeyUp(keyCode)` if available:
     ```javascript
     if (typeof window !== "undefined" && window.RuntimePackVPad && window.RuntimePackVPad.onKeyUp) {
         window.RuntimePackVPad.onKeyUp(keyCode);
     }
     ```
   - In `window.openLinktree`:
     Target URL must be strictly `"https://linktr.ee/VenoraTL"`.
     If `window.RuntimePackVPad && window.RuntimePackVPad.openUrl` exists, call `window.RuntimePackVPad.openUrl(url)`.

2. In `android/shell/app/src/main/assets/overlay/index.html`:
   - In `window.openLinktree`:
     Set URL strictly to `"https://linktr.ee/VenoraTL"` and invoke `window.RuntimePackVPad.openUrl(url)`.

3. In `android/shell/app/src/test/js/vpad_bridge.test.js`:
   - Write a Node.js unit test that asserts:
     - `pressKey(90)` calls `window.RuntimePackVPad.onKeyDown(90)`
     - `releaseKey(90)` calls `window.RuntimePackVPad.onKeyUp(90)`
     - `openLinktree()` calls `window.RuntimePackVPad.openUrl("https://linktr.ee/VenoraTL")`
   - Run test with `node android/shell/app/src/test/js/vpad_bridge.test.js` and verify it exits 0 with all assertions passing.

4. Report:
   - Write full report to `.superpowers/sdd/2026-09-18-input-sync-and-agnostic-cheat-engine/task-1-report.md`.
   - Return status `DONE`, commits, one-line test summary, and concerns (if any).
