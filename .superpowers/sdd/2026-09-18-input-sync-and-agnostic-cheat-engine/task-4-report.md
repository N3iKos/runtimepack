# Task 4 Report: Engine-Agnostic Core Cheat Runner in `runtimepack_bootstrap.rb` and `cheat_engine.js`

## Overview
Implemented the engine-agnostic core cheat execution runner in `runtimepack_bootstrap.rb`, native-to-web IPC bridge methods in `RgssActivity.kt`, and unified `CheatEngine` JS primitives in `cheat_engine.js`. This architecture enables atomic, cross-engine cheat operations supporting both Native RGSS (via atomic IPC files in internal storage) and Web RPG Maker MV/MZ (via direct game object hooks) without hardcoding game-specific formulas or stats.

## Implemented Components

### 1. `runtimepack_bootstrap.rb`
- Added `RuntimePackCheatRunner` module:
  - `CMD_FILE = "rp_cheat_cmd.json"`
  - `STATE_FILE = "rp_cheat_state.json"`
  - `EVAL_FILE = "rp_cheat_eval_res.txt"`
  - `ERROR_FILE = "rp_cheat_error.txt"`
- `RuntimePackCheatRunner.update`:
  - Atomic read and cleanup of `CMD_FILE`
  - Dual JSON decoder compatibility (`JSON.decode` / `JSON.parse`) supporting both custom RPG Maker JSON decoders (e.g. Lona's `101_jsonEnDecoder.rb`) and standard Ruby JSON library
  - Error capture logged with stable error taxonomy code `[CHT-001]`
- `RuntimePackCheatRunner.process_cmd`:
  - `set_variable`: Sets `$game_variables[id] = value`
  - `set_switch`: Sets `$game_switches[id] = bool`
  - `gain_gold`: Calls `$game_party.gain_gold(amount)`
  - `recover_all`: Recovers all members in `$game_party.members` / `$game_party.all_members`
  - `gain_item`: Selects item from `$data_items`, `$data_weapons`, or `$data_armors` and adds to party
  - `dump_state`: Extracts gold, active variables (1..300), and switches (1..300) into `rp_cheat_state.json`
  - `eval`: Runs Ruby code in `TOPLEVEL_BINDING`, writes result to `rp_cheat_eval_res.txt`, captures errors with `[CHT-002]`
- `Graphics.update` hook:
  - Aliased `Graphics.update` to automatically execute `RuntimePackCheatRunner.update` each frame

### 2. `RgssActivity.kt`
- Added atomic file writer `RgssActivity.writeAtomic(targetFile, content)` using temporary swap file with atomic replace/move
- Implemented core bridge methods in `RgssActivity`:
  - `postCheatCmd(jsonCmd: String): Boolean`: writes command JSON atomically to `rp_cheat_cmd.json` in internal working directory (and game subdirectory if present)
  - `getCheatState(): String`: reads `rp_cheat_state.json` content or returns `"{}"`
  - `getEvalResult(): String`: reads `rp_cheat_eval_res.txt`, deletes the result file, and returns the output
- Exposed `@JavascriptInterface` annotations inside `RuntimePackVPad` WebView bridge:
  - `postCheatCmd(jsonCmd)`
  - `getCheatState()`
  - `getEvalResult()`

### 3. `cheat_engine.js`
- Unified `window.CheatEngine` with abstract primitives:
  - `setVariable(id, value)`
  - `setSwitch(id, value)`
  - `gainGold(amount)`
  - `recoverAll()`
  - `gainItem(type, id, amount)`
  - `evalScript(code, callback)`
  - `requestStateDump()`
  - `getLoadedState()`
- Dual-mode routing:
  - Native mode (RGSS): formats commands as JSON and sends via `RuntimePackVPad.postCheatCmd`
  - Web mode (MV/MZ): interacts directly with `$gameVariables`, `$gameSwitches`, `$gameParty`, and `$data*`
- Backward-compatibility preserved for `profile`, `forbidden_features`, and `executeAction`
- Universal CommonJS export `module.exports = CheatEngine` for automated Node testing

## Test Verification

1. **Ruby Core Runner Tests** (`android/shell/app/src/test/ruby/cheat_runner_test.rb`):
   - 25 assertions covering:
     - `set_variable` command and state mutation
     - `set_switch` (true/false)
     - `gain_gold`
     - `recover_all`
     - `gain_item` for items, weapons, armors
     - `dump_state` serialization and non-zero filters
     - `eval` execution and `[CHT-002]` error handling
     - Batch JSON array execution
     - `Graphics.update` frame hook
   - Status: **PASSED (25/25, 100%)**

2. **JavaScript Primitives Tests** (`android/shell/app/src/test/js/cheat_primitives.test.js`):
   - Verified Native RGSS mode command generation and bridge calls
   - Verified Web MV/MZ direct object execution
   - Verified `evalScript` with immediate and asynchronous callbacks
   - Verified profile-driven forbidden action enforcement
   - Status: **PASSED (100%)**

3. **Kotlin Android Shell Unit Tests** (`CheatBridgeTest.kt` & `KeyMappingTest.kt`):
   - Command: `gradlew.bat testDebugUnitTest`
   - Verified atomic file creation, state read, and eval result read-and-delete
   - Status: **BUILD SUCCESSFUL (24 actionable tasks, all unit tests passed)**

4. **Rust Cargo Tests** (`cargo test`):
   - Status: **BUILD & TESTS SUCCESSFUL (38 tests passed, 0 failed)**
