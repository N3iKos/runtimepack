# Design Spec: Input Synchronization & Engine-Agnostic Cheat Engine

## 1. Problem Statement & Goals

### 1.1 Input Synchronization & Community Link Issues
- **D-Pad & Button A unresponsive in-game**: In `vpad.js`, key presses create synthetic `new KeyboardEvent("keydown", { keyCode })` on the WebView document. Chromium/Android WebView drops or reports `0` for `keyCode` in synthetic `KeyboardEvent` constructors. Furthermore, in `RgssActivity.kt`, although `onKeyDown` maps certain keys to Android `KeyEvent.KEYCODE_*` for SDL, games running custom input engines like **Hime AllKey** (such as Lona RPG) completely bypass SDL and RGSS `Input` to read raw Windows keyboard buffers via Win32API `GetKeyboardState`. In `runtimepack_bootstrap.rb`, `GetKeyboardState` previously zeroed the memory buffer with `\0`, rendering all keys permanently released.
- **Broken Community Link**: The Venora Community button did not reliably launch the external browser to `https://linktr.ee/VenoraTL` due to fallback `window.open` restrictions inside Android WebView.

### 1.2 Engine-Agnostic Cheat Architecture
- The existing cheat implementation hardcoded specific stat values or lacked an engine-agnostic separation between core execution primitives and game-specific metadata.
- **Requirement**: Build an engine-agnostic cheat module where:
  1. The **Core Runner** exposes only official engine primitives (`$gameVariables`, `$gameSwitches`, `$gameParty`, `$dataStates`, inventory, eval) without hardcoded game stats or formulas.
  2. The **Game Profile Manifest** (`cheat_profile.json`) is dynamically populated from game discovery metadata (`System.json`, `Data/System.rvdata2`, `CommonEvents.json`), mapping engine IDs to human-readable names.
  3. The **UI** follows a flexible JoiPlay / M-Tool style: Variable & Switch Browser with search/inline editing, Item Spawner, and an interactive Custom Script Console for user-defined cheats and snippets.

---

## 2. Architecture & Components

```
+-------------------------------------------------------------------------------+
|                             Android Host Shell                                |
|                                                                               |
|  +---------------------------+             +-------------------------------+  |
|  |   WebView Overlay Layer   |             |     SDLActivity Native UI     |  |
|  |  (vpad.js, cheat_menu.js) |             |  (libmkxp-z.so, libruby.so)   |  |
|  +-------------+-------------+             +---------------+---------------+  |
|                |                                           ^                  |
|                | JavascriptInterface (RuntimePackBridge)    |                  |
|                v                                           |                  |
|  +---------------------------------------------------------+---------------+  |
|  |                 RgssActivity (Host Controller)                          |  |
|  |  - onKeyDown(keyCode) -> SDL key event + Virtual Key IPC State          |  |
|  |  - onKeyUp(keyCode)   -> SDL key event + Virtual Key IPC State          |  |
|  |  - openUrl(url)       -> Android Intent.ACTION_VIEW (VenoraTL)          |  |
|  |  - postCheatCmd(json) -> IPC Command Queue                              |  |
|  |  - getCheatState()    -> Read IPC State Dump                            |  |
|  +-------------------------------------+-----------------------------------+  |
|                                        |                                      |
|                                        v                                      |
|                           Local Filesystem IPC Queue                          |
|                     (rp_input_state.bin / rp_cheat_cmd.json)                  |
+----------------------------------------+--------------------------------------+
                                         |
                                         v
+-------------------------------------------------------------------------------+
|                       RGSS3 / mkxp-z Execution Environment                     |
|                                                                               |
|  +-------------------------------------------------------------------------+  |
|  |                      runtimepack_bootstrap.rb                           |  |
|  |  1. Win32API Hook:                                                      |  |
|  |     - GetKeyboardState: reads active key buffer (Hime AllKey support)    |  |
|  |     - Preserves SDL NativeInput fallback alias                          |  |
|  |  2. Cheat Runner Hook:                                                  |  |
|  |     - Graphics.update frame tick: drains rp_cheat_cmd.json              |  |
|  |     - Mutates $game_variables, $game_switches, $game_party              |  |
|  |     - Writes back current variable/switch dumps to rp_cheat_state.json  |  |
|  +-------------------------------------------------------------------------+  |
+-------------------------------------------------------------------------------+
```

---

## 3. Detailed Specifications

### 3.1 Input Synchronization & Key State Bridging

#### 3.1.1 Direct Native Bridge in `vpad.js`
Eliminate reliance on synthetic `KeyboardEvent` bubbling in `vpad.js`:
- In `pressKey(keyCode)`:
  - Check `window.RuntimePackVPad.onKeyDown(keyCode)`.
  - Also maintain internal fallback for HTML5 games (`Input._currentState`).
- In `releaseKey(keyCode, forceAll)`:
  - Check `window.RuntimePackVPad.onKeyUp(keyCode)`.
- Key Code Standard Mapping:
  - `38`: UP (VK 0x26)
  - `40`: DOWN (VK 0x28)
  - `37`: LEFT (VK 0x25)
  - `39`: RIGHT (VK 0x27)
  - `90`: A / OK / Decision (VK 0x5A 'Z', VK 0x0D 'Return', VK 0x20 'Space')
  - `88`: B / Cancel / Menu (VK 0x58 'X', VK 0x1B 'Escape')
  - `16`: DASH / Shift (VK 0x10 'Shift')
  - `17`: SKIP / Ctrl (VK 0x11 'Control')
  - `27`: ESCAPE (VK 0x1B)
  - `81`: L / Prev / Q (VK 0x51 'Q')
  - `69`: R / Next / E (VK 0x45 'E')

#### 3.1.2 Virtual Key Buffer in `RgssActivity.kt`
- Maintain a 256-byte thread-safe array `mVirtualKeyBuffer`.
- When `onKeyDown(jsKeyCode)` arrives:
  - Look up Windows Virtual Key (VK).
  - Set `mVirtualKeyBuffer[vk] = 0x80.toByte()`.
  - For OK / A (`90`), set VK_Z (0x5A), VK_RETURN (0x0D), and VK_SPACE (0x20).
  - Write updated buffer to `Context.filesDir/rp_input_state.bin` or native JNI memory shared pointer.
  - Forward Android `KeyEvent` to SDL via `SDLActivity.onNativeKeyDown(androidKey)`.
- When `onKeyUp(jsKeyCode)` arrives:
  - Clear `mVirtualKeyBuffer[vk] = 0x00.toByte()`.
  - Forward to SDL via `SDLActivity.onNativeKeyUp(androidKey)`.

#### 3.1.3 Win32API Hook in `runtimepack_bootstrap.rb`
- In `Win32API#call`:
  - When `func == 'GetKeyboardState'`:
    - Read `rp_input_state.bin` (256 bytes) if modified, or maintain internal Ruby key state table.
    - Resolve the destination pointer `ptr_addr = args[0]`.
    - If `DL::CPtr` is used (Hime AllKey), update `DL::CPtr` instance buffer so `@state[key] & 0x80 == 0x80` returns true for pressed keys.
  - In `Input` module:
    - Alias mkxp-z's native `Input` as `NativeInput`.
    - Enhance `Input.press?`, `Input.trigger?`, `Input.dir4`, `Input.dir8` to check both Hime AllKey `@state` and `NativeInput` states.

### 3.2 Community Linktree
- In `vpad.js` and `index.html`:
  - `window.openLinktree` explicitly invokes `window.RuntimePackVPad.openUrl("https://linktr.ee/VenoraTL")`.
- In `RgssActivity.kt`:
  - `openUrl(url)` constructs `Intent(Intent.ACTION_VIEW, Uri.parse(url))` with `FLAG_ACTIVITY_NEW_TASK` and launches via `startActivity`.

---

### 3.3 Engine-Agnostic Cheat Engine

#### 3.3.1 Core Primitives Protocol
The core runner does not have any game-specific variable IDs or stat calculations hardcoded.
It accepts standardized commands:

| Command Action | Parameters | RGSS Implementation | Web (MV/MZ) Implementation |
|---|---|---|---|
| `get_state` | none | Dumps `$game_variables`, `$game_switches`, `$game_party.gold` to JSON | Reads directly from `$gameVariables`, `$gameSwitches`, `$gameParty` |
| `set_variable` | `id: number, value: any` | `$game_variables[id] = value` | `$gameVariables.setValue(id, value)` |
| `set_switch` | `id: number, value: boolean` | `$game_switches[id] = value` | `$gameSwitches.setValue(id, value)` |
| `gain_gold` | `amount: number` | `$game_party.gain_gold(amount)` | `$gameParty.gainGold(amount)` |
| `gain_item` | `type: string, id: number, amount: number` | `$game_party.gain_item($data_items[id], amount)` | `$gameParty.gainItem($dataItems[id], amount)` |
| `recover_all` | none | `$game_party.members.each(&:recover_all)` | `$gameParty.members().forEach(m => m.recoverAll())` |
| `eval` | `code: string` | `eval(code)` (returns inspected string) | `eval(code)` (returns string) |

#### 3.3.2 Game Profile Manifest (`cheat_profile.json`)
Generated during static analysis / asset preparation or extracted from game metadata:
```json
{
  "engine": "rgss3",
  "variables": [
    { "id": 1, "name": "Gold / Currency" },
    { "id": 2, "name": "Current Day" },
    { "id": 15, "name": "Player Stamina" }
  ],
  "switches": [
    { "id": 1, "name": "Intro Sequence Done" },
    { "id": 5, "name": "Debug / NoClip" }
  ],
  "items": [
    { "id": 1, "name": "Potion", "type": "item" },
    { "id": 2, "name": "Hi-Potion", "type": "item" }
  ]
}
```
If `cheat_profile.json` is absent or incomplete for a specific game, the UI dynamically defaults to numerical index ranges (1..999) with live reading.

#### 3.3.3 UI Implementation (JoiPlay / M-Tool Style in `cheat_menu.js`)
- **Tab 1: Variables Browser**:
  - Live search by Variable Name or ID.
  - Card layout showing `[ID] Name: Value`.
  - Inline input to modify value immediately via `set_variable`.
- **Tab 2: Switches Browser**:
  - Live search by Switch Name or ID.
  - Interactive ON/OFF toggle switch with colored visual indicator.
- **Tab 3: Spawner & Quick Cheats**:
  - Searchable Item & Equipment database.
  - Spawner count slider / buttons (`+1`, `+5`, `+50`).
  - Quick Utilities: "Max Gold", "Full Recovery", "No-Clip (if detected)".
- **Tab 4: Custom Script Console (M-Tool Style)**:
  - Multi-line code input area.
  - "Run Script" button that executes arbitrary engine code safely.
  - Output display showing stdout / return value / error stack.
  - Snippet Manager: Ability to bookmark and name custom cheat scripts saved to `localStorage`.

---

## 4. Error Handling & Diagnostics

| Domain / Code | Condition | Behavior |
|---|---|---|
| `INP-001` | Unknown key code received by bridge | Logs warning, defaults to `KEYCODE_UNKNOWN`, avoids crash |
| `INP-002` | `rp_input_state.bin` read lock contention | Retries non-blocking read; falls back to last cached frame |
| `CHT-001` | Invalid variable ID or out of bounds | Core runner catches exception, writes error response to IPC queue |
| `CHT-002` | Script evaluation error in custom console | Captures exception message and backtrace; displays in UI output without crashing the game |
| `CHT-003` | Missing `cheat_profile.json` | Emits info log; activates dynamic numerical fallback mode |

---

## 5. Testing & Verification Strategy

1. **Unit Tests (Rust / Web / Ruby)**:
   - Verify `vpad.js` calls bridge methods directly.
   - Verify `RgssActivity` virtual key buffer maps properly for arrows and action keys.
   - Verify Ruby `Win32API` `GetKeyboardState` returns expected bytes when buffer has active keys.
   - Verify `eval` and primitive operations in `runtimepack_bootstrap.rb` with mock commands.
2. **Integration Test on Android (MuMu Player / ADB)**:
   - Deploy build to MuMu Player via `adb install -r`.
   - Tap D-Pad arrows on Virtual Controller -> Verify Title screen cursor moves UP/DOWN.
   - Tap Button A -> Verify game confirms menu selection (enters New Game / Load Game).
   - Tap Community Link -> Verify device opens browser to `https://linktr.ee/VenoraTL`.
   - Open Cheat Menu -> Verify Variable & Switch browser displays values and can edit variables.
   - Run console script in M-Tool tab -> Verify script executes and returns expected output.
