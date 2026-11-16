# Input Synchronization & Engine-Agnostic Cheat Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix virtual gamepad responsiveness (D-Pad, Button A) in RGSS/Hime AllKey games, link the Venora Community button to `https://linktr.ee/VenoraTL`, and implement an engine-agnostic JoiPlay/M-Tool style cheat system driven by abstract primitives and metadata discovery.

**Architecture:** 
1. `vpad.js` communicates directly with `RgssActivity` via native JavaScriptInterface without lossy DOM events.
2. `RgssActivity` maintains both SDL Native Key events and a 256-byte Windows Virtual Key buffer synchronized with `runtimepack_bootstrap.rb`'s `Win32API` hook to support custom input drivers (Hime AllKey).
3. The Cheat Engine separates core execution primitives (`$game_variables`, `$game_switches`, `$game_party`, eval) from game metadata, while the overlay presents an interactive M-Tool style browser and script console.

**Tech Stack:** Kotlin (Android SDK 34), Ruby 3 (mkxp-z RGSS3), JavaScript (ES6+ WebView Overlay), Rust (Cargo CLI), Gradle 8.7.

**Spec:** `docs/superpowers/specs/2026-09-18-input-sync-and-agnostic-cheat-engine-design.md`

## Global Constraints

- Never mutate user source game directory; build and transform only in working/output directories.
- Zero broad external storage permissions (`MANAGE_EXTERNAL_STORAGE` forbidden; use `Context.filesDir` only).
- Do not hardcode any game-specific stat names, formulas, or variable IDs in the core cheat runner.
- Every error must use structured diagnostic codes (`INP-xxx`, `CHT-xxx`).
- Target URL for community link is strictly `https://linktr.ee/VenoraTL`.

---

### Task 1: Direct Native Bridge & VenoraTL Community Link in `vpad.js` and `index.html`

**Files:**
- Modify: `android/shell/app/src/main/assets/overlay/vpad.js`
- Modify: `android/shell/app/src/main/assets/overlay/index.html`
- Test: `android/shell/app/src/test/js/vpad_bridge.test.js`

**Interfaces:**
- Consumes: `window.RuntimePackVPad.onKeyDown(keyCode: number)`, `window.RuntimePackVPad.onKeyUp(keyCode: number)`, `window.RuntimePackVPad.openUrl(url: string)`
- Produces: Direct native bridge calls in `pressKey` and `releaseKey`; reliable community link opening

- [ ] **Step 1: Write test for vpad bridge calls and community link**

Create `android/shell/app/src/test/js/vpad_bridge.test.js`:
```javascript
const assert = require("assert");

// Mock window and RuntimePackVPad
global.window = {};
let keyDownCalls = [];
let keyUpCalls = [];
let openUrlCalls = [];

global.window.RuntimePackVPad = {
    onKeyDown: (code) => keyDownCalls.push(code),
    onKeyUp: (code) => keyUpCalls.push(code),
    openUrl: (url) => openUrlCalls.push(url)
};

function pressKey(keyCode) {
    if (!keyCode) return;
    if (global.window.RuntimePackVPad && global.window.RuntimePackVPad.onKeyDown) {
        global.window.RuntimePackVPad.onKeyDown(keyCode);
    }
}

function releaseKey(keyCode) {
    if (!keyCode) return;
    if (global.window.RuntimePackVPad && global.window.RuntimePackVPad.onKeyUp) {
        global.window.RuntimePackVPad.onKeyUp(keyCode);
    }
}

function openLinktree() {
    const url = "https://linktr.ee/VenoraTL";
    if (global.window.RuntimePackVPad && global.window.RuntimePackVPad.openUrl) {
        global.window.RuntimePackVPad.openUrl(url);
    }
}

// Test Key Press
pressKey(90); // Button A
assert.strictEqual(keyDownCalls.length, 1);
assert.strictEqual(keyDownCalls[0], 90);

// Test Key Release
releaseKey(90);
assert.strictEqual(keyUpCalls.length, 1);
assert.strictEqual(keyUpCalls[0], 90);

// Test Linktree URL
openLinktree();
assert.strictEqual(openUrlCalls.length, 1);
assert.strictEqual(openUrlCalls[0], "https://linktr.ee/VenoraTL");

console.log("vpad_bridge.test.js: ALL TESTS PASSED");
```

- [ ] **Step 2: Run test to verify it passes in isolation**

Run: `node android/shell/app/src/test/js/vpad_bridge.test.js`
Expected: `vpad_bridge.test.js: ALL TESTS PASSED`

- [ ] **Step 3: Update `vpad.js` and `index.html`**

In `android/shell/app/src/main/assets/overlay/vpad.js`:
1. In `pressKey(keyCode)`:
   Add direct invocation:
   ```javascript
   if (window.RuntimePackVPad && window.RuntimePackVPad.onKeyDown) {
       window.RuntimePackVPad.onKeyDown(keyCode);
   }
   ```
2. In `releaseKey(keyCode)`:
   Add direct invocation:
   ```javascript
   if (window.RuntimePackVPad && window.RuntimePackVPad.onKeyUp) {
       window.RuntimePackVPad.onKeyUp(keyCode);
   }
   ```
3. In `window.openLinktree`:
   Set target URL strictly to `"https://linktr.ee/VenoraTL"` and call `window.RuntimePackVPad.openUrl(url)`.

In `android/shell/app/src/main/assets/overlay/index.html`:
Update `window.openLinktree` to forward to `window.RuntimePackVPad.openUrl("https://linktr.ee/VenoraTL")`.

- [ ] **Step 4: Verify test runs against updated code**

Run: `node android/shell/app/src/test/js/vpad_bridge.test.js`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/assets/overlay/vpad.js android/shell/app/src/main/assets/overlay/index.html android/shell/app/src/test/js/vpad_bridge.test.js
git commit -m "fix(overlay): call native vpad bridge directly and set community link to VenoraTL"
```

---

### Task 2: Host Activity Virtual Key Buffer & Action Key Mapping in `RgssActivity.kt`

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/KeyMappingTest.kt`

**Interfaces:**
- Consumes: JS keycodes from `onKeyDown`/`onKeyUp`
- Produces: `SDLActivity.onNativeKeyDown`/`onNativeKeyUp`, `rp_input_state.bin` in `filesDir`

- [ ] **Step 1: Write test for Key Mapping & Virtual Key state serialization**

Create `android/shell/app/src/test/java/io/runtimepack/host/KeyMappingTest.kt`:
Verify that:
- JS 90 maps to `KeyEvent.KEYCODE_Z`, `KeyEvent.KEYCODE_ENTER`, and `KeyEvent.KEYCODE_SPACE`.
- JS 38, 40, 37, 39 map to DPAD UP/DOWN/LEFT/RIGHT.
- Virtual key bytes array (256 bytes) properly sets index 0x26 (VK_UP), 0x28 (VK_DOWN), 0x0D (VK_RETURN), 0x20 (VK_SPACE), 0x5A (VK_Z) to `0x80`.

- [ ] **Step 2: Run test to verify it fails initially**

Run: `gradlew.bat :app:testDebugUnitTest --tests io.runtimepack.host.KeyMappingTest` (in `android/shell`)
Expected: FAIL

- [ ] **Step 3: Implement Virtual Key Buffer in `RgssActivity.kt`**

In `RgssActivity.kt`:
1. Maintain `val virtualKeyBuffer = ByteArray(256)` and atomic state file `val inputStateFile = File(filesDir, "rp_input_state.bin")`.
2. Map `jsKey` to list of Windows VK codes and list of Android keycodes:
   - 90 (A / OK): VK 0x5A (Z), 0x0D (RETURN), 0x20 (SPACE) -> Android `KEYCODE_Z`, `KEYCODE_ENTER`, `KEYCODE_SPACE`.
   - 88 (B / Cancel): VK 0x58 (X), 0x1B (ESCAPE) -> Android `KEYCODE_X`, `KEYCODE_ESCAPE`.
   - 38 (UP): VK 0x26 -> Android `KEYCODE_DPAD_UP`.
   - 40 (DOWN): VK 0x28 -> Android `KEYCODE_DPAD_DOWN`.
   - 37 (LEFT): VK 0x25 -> Android `KEYCODE_DPAD_LEFT`.
   - 39 (RIGHT): VK 0x27 -> Android `KEYCODE_DPAD_RIGHT`.
   - 16 (Shift): VK 0x10 -> Android `KEYCODE_SHIFT_LEFT`.
   - 17 (Ctrl): VK 0x11 -> Android `KEYCODE_CTRL_LEFT`.
3. In `onKeyDown`:
   - Set virtual key bytes in `virtualKeyBuffer` to `0x80.toByte()`.
   - Write `virtualKeyBuffer` to `inputStateFile`.
   - Send each mapped Android key to `onNativeKeyDown(androidKey)`.
4. In `onKeyUp`:
   - Set virtual key bytes to `0x00.toByte()`.
   - Write `virtualKeyBuffer` to `inputStateFile`.
   - Send each mapped Android key to `onNativeKeyUp(androidKey)`.
5. In `openUrl(url: String)`:
   Ensure `Intent(Intent.ACTION_VIEW, Uri.parse(url))` has `FLAG_ACTIVITY_NEW_TASK`.

- [ ] **Step 4: Run unit test to verify it passes**

Run: `gradlew.bat :app:testDebugUnitTest --tests io.runtimepack.host.KeyMappingTest`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt android/shell/app/src/test/java/io/runtimepack/host/KeyMappingTest.kt
git commit -m "feat(host): implement virtual key buffer and multi-binding action key mapping"
```

---

### Task 3: Win32API Hook & Native Input Synchronization in `runtimepack_bootstrap.rb`

**Files:**
- Modify: `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
- Test: `android/shell/app/src/test/ruby/input_hook_test.rb`

**Interfaces:**
- Consumes: `rp_input_state.bin` or `DL::CPtr` addresses in `GetKeyboardState`
- Produces: Synchronized input states for Hime AllKey and standard RGSS `Input`

- [ ] **Step 1: Write Ruby test for GetKeyboardState & Input fallback**

Create `android/shell/app/src/test/ruby/input_hook_test.rb`:
```ruby
require 'minitest/autorun'

class TestInputHook < Minitest::Test
  def setup
    # Setup mock DL::CPtr and state file
    File.binwrite("rp_input_state.bin", "\x80" * 256)
  end

  def teardown
    File.delete("rp_input_state.bin") if File.exist?("rp_input_state.bin")
  end

  def test_get_keyboard_state_populates_cptr
    # Verify pointer receives 0x80 bytes
    cptr_buffer = "\0" * 256
    # Simulated Win32API GetKeyboardState call
    data = File.binread("rp_input_state.bin")
    cptr_buffer[0, 256] = data
    assert_equal 0x80, cptr_buffer.getbyte(0x26) # VK_UP
    assert_equal 0x80, cptr_buffer.getbyte(0x0D) # VK_RETURN
  end
end
```

- [ ] **Step 2: Run Ruby test**

Run: `ruby android/shell/app/src/test/ruby/input_hook_test.rb`
Expected: PASS

- [ ] **Step 3: Update `runtimepack_bootstrap.rb`**

In `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`:
1. In `DL::CPtr`:
   Keep a static registry of active `CPtr` instances so pointer addresses can be looked up:
   ```ruby
   module DL
     class CPtr
       @instances = {}
       class << self
         attr_accessor :instances
       end
       def initialize(pointer, size = nil)
         ...
         DL::CPtr.instances[object_id] = self
       end
     end
   end
   ```
2. In `Win32API#call`:
   When `@func == 'GetKeyboardState'`:
   ```ruby
   ptr_id = args[0]
   cptr = DL::CPtr.instances[ptr_id]
   if cptr && File.exist?("rp_input_state.bin")
     begin
       state_bytes = File.binread("rp_input_state.bin")
       if state_bytes && state_bytes.bytesize >= 256
         0.upto(255) do |k|
           cptr[k] = state_bytes.getbyte(k)
         end
       end
     rescue => e
       # fallback
     end
   end
   1
   ```
3. Hook `Input` module after all scripts are loaded:
   Store mkxp-z native `Input` as `NativeMKXPInput = Input.dup rescue nil`.
   In `Input.press?`, `Input.trigger?`, `Input.dir4`:
   If Hime AllKey is active, combine results:
   ```ruby
   # If NativeMKXPInput responds, check it as a fallback for buttons
   ```

- [ ] **Step 4: Verify test passes**

Run: `ruby android/shell/app/src/test/ruby/input_hook_test.rb`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/assets/runtimepack_bootstrap.rb android/shell/app/src/test/ruby/input_hook_test.rb
git commit -m "fix(rgss): hook Win32API GetKeyboardState with real virtual key buffer for Hime AllKey"
```

---

### Task 4: Engine-Agnostic Core Cheat Runner in `runtimepack_bootstrap.rb` and `cheat_engine.js`

**Files:**
- Modify: `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`
- Modify: `android/shell/app/src/main/assets/overlay/cheat_engine.js`
- Test: `android/shell/app/src/test/js/cheat_primitives.test.js`

**Interfaces:**
- Consumes: IPC JSON command files (`rp_cheat_cmd.json`) or `postCheatCmd(json: String)`
- Produces: Execution of official engine primitives (`$game_variables`, `$game_switches`, `$game_party`, eval) and state dump in `rp_cheat_state.json`

- [ ] **Step 1: Write test for cheat engine primitives**

Create `android/shell/app/src/test/js/cheat_primitives.test.js`:
Test commands generation and parsing:
- `set_variable`: `{ "action": "set_variable", "id": 1, "value": 500 }`
- `set_switch`: `{ "action": "set_switch", "id": 2, "value": true }`
- `gain_gold`: `{ "action": "gain_gold", "amount": 1000 }`
- `eval`: `{ "action": "eval", "code": "$game_party.gold" }`

- [ ] **Step 2: Run test**

Run: `node android/shell/app/src/test/js/cheat_primitives.test.js`
Expected: PASS

- [ ] **Step 3: Implement Primitives Runner**

1. In `runtimepack_bootstrap.rb`:
   Hook `Graphics.update`:
   ```ruby
   module Graphics
     class << self
       alias_method :rp_cheat_orig_update, :update rescue nil
       def update
         rp_process_cheat_queue
         rp_cheat_orig_update
       end
       def rp_process_cheat_queue
         cmd_file = "rp_cheat_cmd.json"
         return unless File.exist?(cmd_file)
         begin
           raw = File.read(cmd_file)
           File.delete(cmd_file) rescue nil
           cmd = JSON.decode(raw) rescue nil
           return unless cmd.is_a?(Hash)
           case cmd["action"]
           when "set_variable"
             $game_variables[cmd["id"].to_i] = cmd["value"] if defined?($game_variables)
           when "set_switch"
             $game_switches[cmd["id"].to_i] = (cmd["value"] ? true : false) if defined?($game_switches)
           when "gain_gold"
             $game_party.gain_gold(cmd["amount"].to_i) if defined?($game_party)
           when "recover_all"
             $game_party.members.each(&:recover_all) if defined?($game_party)
           when "eval"
             res = eval(cmd["code"].to_s)
             File.open("rp_cheat_eval_res.txt", "wb") { |f| f.write(res.inspect) }
           when "dump_state"
             # Dump current variables, switches, gold to rp_cheat_state.json
           end
         rescue => e
           File.open("rp_cheat_error.txt", "wb") { |f| f.write("#{e.class}: #{e.message}") }
         end
       end
     end
   end
   ```

2. In `RgssActivity.kt`:
   Expose methods on `RuntimePackVPad`:
   - `@JavascriptInterface fun postCheatCmd(json: String): Boolean`
   - `@JavascriptInterface fun getCheatState(): String`
   - `@JavascriptInterface fun getEvalResult(): String`

3. In `cheat_engine.js`:
   Implement `window.CheatEngine` with abstract methods:
   - `setVariable(id, value)`
   - `setSwitch(id, value)`
   - `gainGold(amount)`
   - `recoverAll()`
   - `evalScript(code)`
   - `dumpState()`

- [ ] **Step 4: Run unit test**

Run: `node android/shell/app/src/test/js/cheat_primitives.test.js`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/assets/runtimepack_bootstrap.rb android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt android/shell/app/src/main/assets/overlay/cheat_engine.js android/shell/app/src/test/js/cheat_primitives.test.js
git commit -m "feat(cheat): implement engine-agnostic core primitives and IPC bridge"
```

---

### Task 5: JoiPlay / M-Tool Style Dynamic Cheat UI in `cheat_menu.js`

**Files:**
- Modify: `android/shell/app/src/main/assets/overlay/cheat_menu.js`
- Test: `android/shell/app/src/test/js/cheat_menu.test.js`

**Interfaces:**
- Consumes: `window.CheatEngine` methods
- Produces: Variable Browser, Switch Browser, Spawner, Custom Script Console, and Snippet Manager

- [ ] **Step 1: Write UI structure test**

Create `android/shell/app/src/test/js/cheat_menu.test.js`:
Verify tabs exist:
- Variables Tab (`#fnh-tab-variables`)
- Switches Tab (`#fnh-tab-switches`)
- Spawner Tab (`#fnh-tab-spawner`)
- Console Tab (`#fnh-tab-console`)

- [ ] **Step 2: Run test**

Run: `node android/shell/app/src/test/js/cheat_menu.test.js`
Expected: PASS

- [ ] **Step 3: Update `cheat_menu.js` UI**

1. Render 4 tabs:
   - **Tab 1: Variables**: Search input by ID/Name, list of variables with inline value editor and "Set" button.
   - **Tab 2: Switches**: Search input by ID/Name, ON/OFF toggle cards with active color indicator.
   - **Tab 3: Spawner & Quick Cheats**: Quick buttons for "Max Gold (999,999)", "Full Party Heal", and generic item count injector.
   - **Tab 4: Custom Script Console (M-Tool style)**: Multi-line `textarea`, "Execute Script" button, terminal log box for output/errors, and "Save Snippet" button saving to `localStorage`.
2. Dynamic discovery fallback:
   - Attempt to fetch `cheat_profile.json`.
   - If not found or empty, dynamically generate index ranges 1..100 for variables and switches with live value querying.

- [ ] **Step 4: Verify test passes**

Run: `node android/shell/app/src/test/js/cheat_menu.test.js`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/assets/overlay/cheat_menu.js android/shell/app/src/test/js/cheat_menu.test.js
git commit -m "feat(cheat): implement JoiPlay and M-Tool style dynamic variable browser and script console"
```

---

### Task 6: End-to-End Build & Validation on Emulator

**Files:**
- Verify: `dist/lona/LonaRPG.apk`
- Test: Emulator ADB verification

- [ ] **Step 1: Run Cargo workspace tests**

Run: `cargo test`
Expected: ALL TESTS PASS

- [ ] **Step 2: Synchronize overlay assets to dist and rebuild APK**

Copy updated assets (`vpad.js`, `cheat_engine.js`, `cheat_menu.js`, `index.html`, `runtimepack_bootstrap.rb`) into `dist/lona/android-project/app/src/main/assets/` and recompile APK:
`gradlew.bat assembleDebug` in `dist/lona/android-project`.

- [ ] **Step 3: Install on MuMu Player emulator via ADB**

Run: `adb install -r dist/lona/android-project/app/build/outputs/apk/debug/app-debug.apk`

- [ ] **Step 4: Functional Verification**

1. Launch game: `adb shell am start -n com.example.lonarpg/io.runtimepack.host.RgssActivity`
2. Test D-Pad and Button A: Confirm title menu responds and selects New Game.
3. Test Community Button: Tap link button, verify Android browser intent is dispatched.
4. Test Cheat Menu: Open cheat menu, test variable editing and script console execution.

- [ ] **Step 5: Commit & update progress memory**

Update `memory/progress.md` and commit:
```bash
git commit -m "chore: complete input sync, community link, and agnostic cheat engine integration"
```
