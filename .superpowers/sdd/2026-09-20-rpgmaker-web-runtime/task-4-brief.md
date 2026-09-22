# Task 4: In-Game Virtual Gamepad & Cheat Integration for MV/MZ

## Goal
Integrate virtual gamepad input dispatching and cheat command execution between `overlayWebView` and `gameWebView` in `WebActivity.kt` and `runtimepack_web_shims.js`.

## Requirements
1. In `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`:
   - Add `window.executeCheatCmd(cmd)`:
     - `set_variable`: calls `$gameVariables.setValue(cmd.id, cmd.value)` if `$gameVariables` exists.
     - `set_switch`: calls `$gameSwitches.setValue(cmd.id, Boolean(cmd.value))` if `$gameSwitches` exists.
     - `gain_gold`: calls `$gameParty.gainGold(cmd.amount)` if `$gameParty` exists.
     - `recover_all`: calls `$gameParty.members().forEach(m => m.recoverAll())` if `$gameParty` exists.
     - `gain_item`: calls `$gameParty.gainItem($dataItems[cmd.id] || $dataWeapons[cmd.id] || $dataArmors[cmd.id], cmd.amount)` if available.
     - `eval`: evaluates `cmd.code` in global context; reports result to `window.RuntimePackWeb.setEvalResult(res)`.
     - `dump_state`: serializes `$gameParty.gold()`, `$gameVariables._data`, `$gameSwitches._data` and reports to `window.RuntimePackWeb.setCheatState(JSON.stringify(payload))`.
2. In `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`:
   - Register `RuntimePackWeb` JavascriptInterface on `gameWebView`:
     - `setCheatState(stateJson: String)`: saves to volatile `cachedCheatState`.
     - `setEvalResult(resStr: String)`: saves to volatile `cachedEvalResult`.
   - Implement `getCheatState()`: returns `cachedCheatState`.
   - Implement `getEvalResult()`: returns and clears `cachedEvalResult`.
   - Ensure `dispatchKeyToGame` triggers both DOM `KeyboardEvent` and direct `window.TouchInput` / `window.Input` callbacks if available.
3. In `android/shell/app/src/test/js/web_shims.test.js`:
   - Add unit test validating `window.executeCheatCmd` with mock `$gameVariables`, `$gameSwitches`, `$gameParty`.
4. Run tests:
   `node android/shell/app/src/test/js/web_shims.test.js`
   `.\gradlew.bat testDebugUnitTest`
5. Write execution report to `.superpowers/sdd/2026-09-20-rpgmaker-web-runtime/task-4-report.md`.
