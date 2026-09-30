# Task 2 Report: Kotlin Patch — `dispatchKeyToGame` Anti-Sanitasi Chromium (`WebActivity.kt`)

## 1. Task Summary
- **Target File:** `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`
- **Method Modified:** `dispatchKeyToGame(jsKeyCode: Int, type: String)`
- **Objective:** Prevent Chromium modern event sanitization from stripping `keyCode` (resetting to 0) during synthetic `KeyboardEvent` creation.
- **Implemented Mechanism:**
  1. Synthetic `KeyboardEvent` created with `Object.defineProperty` overriding `keyCode` and `which`.
  2. Mapped standard key codes to modern `code` strings (e.g. `13 -> "Enter"`, `90 -> "KeyZ"`) and defined on the event.
  3. Direct injection into `window.Input._onKeyDown` / `window.Input._onKeyUp` and `window.TouchInput`.
  4. Direct action fallback to `window.RuntimePackInput.press(action)` / `release(action)` using `Input.keyMapper` and `Input.codeMapper`.

## 2. Commit
- **Commit Hash:** `e054bcb3b8e5eed2a5087816d77799fe3d7dce7a` (short: `e054bcb`)
- **Commit Message:** `fix(android-host): inject keyCode via defineProperty and direct Input state to survive Chromium event sanitization`
- **Files Committed:** Only `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`

## 3. Verification Evidence

### 3.1 Kotlin Compilation (`:app:compileDebugKotlin`)
Command: `.\gradlew.bat :app:compileDebugKotlin --rerun-tasks --console=plain`
Working directory: `android/shell`

Output:
```
> Task :app:checkKotlinGradlePluginConfigurationErrors
> Task :app:preBuild UP-TO-DATE
> Task :app:preDebugBuild UP-TO-DATE
> Task :app:generateDebugResValues
> Task :app:mapDebugSourceSetPaths
> Task :app:checkDebugAarMetadata
> Task :app:generateDebugResources
> Task :app:packageDebugResources
> Task :app:createDebugCompatibleScreenManifests
> Task :app:extractDeepLinksDebug
> Task :app:processDebugMainManifest
> Task :app:parseDebugLocalResources
> Task :app:processDebugManifest
> Task :app:processDebugManifestForPackage
> Task :app:mergeDebugResources
> Task :app:processDebugResources
> Task :app:compileDebugKotlin

BUILD SUCCESSFUL in 37s
15 actionable tasks: 15 executed
```

### 3.2 JavaScript Test Suites
All 5 test suites executed from root worktree (`D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save`):

1. `node android/shell/app/src/test/js/web_shims.test.js`
```
Running Mobile Web Shims Unit Tests...
Engine Input Patch tests PASSED
All Mobile Web Shims tests passed successfully!
```
Exit code: 0

2. `node android/shell/app/src/test/js/vpad_bridge.test.js`
```
Running Virtual Gamepad Native Bridge & Community Link Tests...
✔ pressKey(90) calls window.RuntimePackVPad.onKeyDown(90)
✔ window.pressKey(90) calls window.RuntimePackVPad.onKeyDown(90)
✔ releaseKey(90) calls window.RuntimePackVPad.onKeyUp(90)
✔ window.releaseKey(90) calls window.RuntimePackVPad.onKeyUp(90)
✔ openCommunityLink() calls window.RuntimePackVPad.openCommunityLink()
✔ window.openCommunityLink() calls window.RuntimePackVPad.openCommunityLink()
✔ window.openLinktree() calls window.RuntimePackVPad.openCommunityLink()
✔ openLinktree() export calls window.RuntimePackVPad.openCommunityLink()
✔ Graceful fallback to window.RuntimePackVPad.openUrl("https://linktr.ee/Veldra.Works") when openCommunityLink is missing
✔ index.html openCommunityLink() and openLinktree() call native openCommunityLink() with openUrl fallback
✔ Graceful degradation when window.RuntimePackVPad is absent
✔ Multi-touch button release does not globally release other buttons
✔ D-Pad touch release is isolated from button touches (drives setupDpadTouch)
✔ Overlay input/textarea focus and blur call RuntimePackVPad.setOverlayTextFocused(true/false)
✔ Window blur and hidden document reset RuntimePackVPad.setOverlayTextFocused(false)
✔ Layout preset D-Pad raised (cy: 0.75), STORAGE_KEY v11, classic wrench icon, Liquid Glass, and Matte default verified
✔ Universal thin-ring ripple, canvas motion trail logic, and strict control isolation verified
✔ Continuous Bézier spline ribbon rendering and zero idle CPU lifecycle verified
✔ Tap-only non-tail behavior, movement threshold, and stroke session isolation verified
Running 3-Mode Feedback Toggle & Pill Toast Tests...
✔ 3-mode cycling, icon updates, toast presentation, and mode behavior enforcement verified

All test assertions passed successfully.
```
Exit code: 0

3. `node android/shell/app/src/test/js/cheat_menu.test.js`
```
Running Cheat Menu Suite Tests (cheat_menu.test.js)...
✔ CheatMenu DOM initialized successfully

--- Test 1: Tab Switching ---
✔ Tab switching correctly toggles active panes

--- Test 2: Variable Search & Set Dispatch ---
✔ Variable search, set dispatch, and refresh working properly

--- Test 3: Switch Toggle Dispatch ---
✔ Switch toggle dispatch, UI update, and search working properly

--- Test 4: Item Spawner & Quick Cheats ---
✔ Spawner quick actions (Max Gold, Recover All) & item spawn dispatch working properly

--- Test 5: Script Console Execution & Output ---
✔ Console script evaluation and output terminal rendering working properly

--- Test 6: Snippet Persistence in LocalStorage ---
✔ Snippet save, dropdown listing, loading, and deletion in localStorage working properly

--- Test 7: Theme Toggle ---
✔ Theme toggle correctly toggles .vpad-theme-retro class

--- Test 8: Neutralized CSS & Zero Injected Hardcoded Styles ---
✔ Neutralized styles and zero hardcoded hex verified in Cheat Menu

==================================================
All 8 Cheat Menu test suites passed successfully! (100%)
==================================================
```
Exit code: 0

4. `node android/shell/app/src/test/js/cheat_primitives.test.js`
```
Running Cheat Engine Primitives & Bridge Tests...
[RuntimePack] cheat_engine.js initialized.

--- Part 1: Native RGSS Mode Tests ---
✔ Native setVariable(15, 42) posts valid JSON command
✔ Native setSwitch(7, true/false) posts valid JSON command
✔ Native gainGold(5000) posts valid JSON command
✔ Native recoverAll() posts valid JSON command
✔ Native gainItem() posts valid type, id, amount JSON commands
✔ Native requestStateDump() & getLoadedState() work correctly
✔ Native evalScript() posts eval command and receives result

--- Part 2: Web MV/MZ Mode Tests ---
[CheatEngine] Action 'forbidden_action' is forbidden by game profile!
✔ Web setVariable(3, 777) calls $gameVariables.setValue
✔ Web setSwitch(9, true/false) calls $gameSwitches.setValue
✔ Web gainGold(300) calls $gameParty.gainGold
✔ Web recoverAll() calls recoverAll on all members
✔ Web gainItem() gains item, weapon, and armor
✔ Web requestStateDump() & getLoadedState() construct accurate state snapshot
✔ Web evalScript('10 + 25') executes and passes result to callback

--- Part 3: Backward Compatibility & Profile ---
✔ Forbidden features in profile are blocked
✔ Fallback executeAction('heal_all') routes to recoverAll

==================================================
All CheatEngine primitive tests passed successfully!
==================================================
```
Exit code: 0

5. `node android/shell/app/src/test/js/diagnostic_suite.test.js`
```
Running Diagnostic Suite JS Unit Tests...
Test 1: DOM Elements Injection
Test 2: Modal Toggle & VPad Overlay Hook
Test 3: Live Error Logging
Test 4: Action Dispatches (stress test, teleport, bug report)
Test 5: Empty Progress Never Triggers Fake Success
Test 6: Structured JSON Events Parsing & Error Handling (Continues Scanning on Failure)
Test 7: Structured Line Events (START, STAGE, DONE)
Test 8: Stream of Passing Events Does NOT Prematurely Stop Audit
Test 9: getAuditEvents Bridge Queries with lastEventSeq
Test 10: Tab Asset Audit & Probe and Probe Buttons Exist
Test 11: Real Authentic Live Stage Counters
Test 12: exportBugReport includes combined_validation_summary
Test 13: Stress Test Real-Time Progress, Counter Sync, and Failure Enforcement
Test 14: Neutralized CSS & Zero Injected Hardcoded Styles
All Diagnostic Suite JS unit tests passed successfully!
```
Exit code: 0

## 4. Self-Review Findings
- **Isolation:** Staged and committed strictly `WebActivity.kt`. Untracked files and uncommitted changes in `WebBackend.kt` were untouched.
- **Contract Compatibility:** `dispatchKeyToGame(jsKeyCode: Int, type: String)` signature and call-sites (`dispatchKeyEvent`, gamepad shortcuts) remain 100% compatible.
- **Safety:** Both standard DOM event dispatch and direct engine internal dispatch are performed with try-catch guards to prevent exceptions on unexpected engines or DOM configurations.
- **No Game File Changes:** No files in user game source modified.
