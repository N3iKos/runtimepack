# Task 2 Brief: Host Activity Virtual Key Buffer & Action Key Mapping in `RgssActivity.kt`

## Context & Project Fit
Custom input engines in RPG Maker VX Ace (notably Hime AllKey in Lona RPG) bypass standard RGSS `Input` and poll Windows virtual key buffers directly via Win32API `GetKeyboardState`. Standard SDL key events are not sufficient for Hime AllKey, and single-key bindings (e.g. only `KEYCODE_Z`) miss games configured to read `RETURN` or `SPACE` for Action/OK.

## Requirements & Scope
- Files to modify:
  - `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`
  - `android/shell/app/src/main/assets/overlay/index.html` (remove duplicate synthetic forwarding listener to keep bridge calls clean)
- File to create:
  - `android/shell/app/src/test/java/io/runtimepack/host/KeyMappingTest.kt`

### Detailed Changes:
1. In `RgssActivity.kt`:
   - Create a companion/helper or member function `mapJsKey(jsKey: Int): Pair<List<Int>, List<Int>>` (Android KeyCodes, Windows Virtual Key Codes):
     - `90` (A / OK): Android `[KEYCODE_Z, KEYCODE_ENTER, KEYCODE_SPACE]`, VK `[0x5A, 0x0D, 0x20]`
     - `88` (B / Cancel): Android `[KEYCODE_X, KEYCODE_ESCAPE]`, VK `[0x58, 0x1B]`
     - `38` (UP): Android `[KEYCODE_DPAD_UP]`, VK `[0x26]`
     - `40` (DOWN): Android `[KEYCODE_DPAD_DOWN]`, VK `[0x28]`
     - `37` (LEFT): Android `[KEYCODE_DPAD_LEFT]`, VK `[0x25]`
     - `39` (RIGHT): Android `[KEYCODE_DPAD_RIGHT]`, VK `[0x27]`
     - `16` (Shift / Dash): Android `[KEYCODE_SHIFT_LEFT]`, VK `[0x10]`
     - `17` (Ctrl / Skip): Android `[KEYCODE_CTRL_LEFT]`, VK `[0x11]`
     - `27` (Escape / Menu): Android `[KEYCODE_ESCAPE]`, VK `[0x1B]`
     - `13` (Enter): Android `[KEYCODE_ENTER]`, VK `[0x0D]`
     - `32` (Space): Android `[KEYCODE_SPACE]`, VK `[0x20]`
     - `81` (Q / L): Android `[KEYCODE_Q]`, VK `[0x51]`
     - `69` (E / R): Android `[KEYCODE_E]`, VK `[0x45]`
     - Numbers 48..57 -> `KEYCODE_0..9`, VK `0x30..0x39`
     - Letters 65..90 (A..Z fallback) -> matching `KEYCODE_A..Z`, VK `0x41..0x5A`
   - Maintain a thread-safe `virtualKeyBuffer = ByteArray(256)` and a `pressedJsKeys = ConcurrentHashMap.newKeySet<Int>()`.
   - In `onKeyDown(jsKeyCode: Int)`:
     - Check `pressedJsKeys.add(jsKeyCode)`: if already added, return (idempotent / avoid double firing).
     - Set all mapped VK bytes in `virtualKeyBuffer` to `0x80.toByte()`.
     - Write `virtualKeyBuffer` atomically to `File(filesDir, "rp_input_state.bin")`.
     - For each mapped Android key, call `onNativeKeyDown(androidKey)`.
   - In `onKeyUp(jsKeyCode: Int)`:
     - `pressedJsKeys.remove(jsKeyCode)`
     - Set all mapped VK bytes in `virtualKeyBuffer` to `0x00.toByte()`.
     - Write `virtualKeyBuffer` atomically to `File(filesDir, "rp_input_state.bin")`.
     - For each mapped Android key, call `onNativeKeyUp(androidKey)`.
   - In `openUrl(url: String)`:
     - Ensure intent has `Intent.FLAG_ACTIVITY_NEW_TASK` and is wrapped safely.

2. In `android/shell/app/src/test/java/io/runtimepack/host/KeyMappingTest.kt`:
   - Unit test the key mapping logic, virtual key buffer state updates, and serialization to `rp_input_state.bin`.
   - Run tests via Gradle: `gradlew.bat testDebugUnitTest --tests io.runtimepack.host.KeyMappingTest` from `android/shell`.

3. Report Contract:
   - Full report in `.superpowers/sdd/2026-09-18-input-sync-and-agnostic-cheat-engine/task-2-report.md`.
   - Return status `DONE`, commits, test command & output summary, concerns.
