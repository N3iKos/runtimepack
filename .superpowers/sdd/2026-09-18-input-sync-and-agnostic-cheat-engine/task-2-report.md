# Task 2 Report: Host Activity Virtual Key Buffer & Action Key Mapping in `RgssActivity.kt`

## Execution Summary
- **Status:** `DONE`
- **Task ID:** Task 2 (Subagent)
- **Files Modified:**
  - `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`
  - `android/shell/app/src/main/assets/overlay/index.html`
  - `android/shell/app/src/main/AndroidManifest.xml` (cleaned label definition)
  - `android/shell/app/build.gradle.kts` (added testOptions and junit dependency)
  - `.gitignore` (ignored `.gradle/` and `build/`)
- **Files Created:**
  - `android/shell/app/src/test/java/io/runtimepack/host/KeyMappingTest.kt`
  - `android/shell/gradlew`, `android/shell/gradlew.bat`, `android/shell/gradle/wrapper/*` (generated Gradle wrapper)

## Implementation Details
1. **Virtual Key Code & Android Key Code Mapping (`mapJsKey`):**
   - Implemented `RgssActivity.mapJsKey(jsKey: Int): Pair<List<Int>, List<Int>>` mapping JavaScript keycodes to both Android `KeyEvent` codes and Windows Virtual Key (VK) codes:
     - `90` (A / OK): Android `[KEYCODE_Z, KEYCODE_ENTER, KEYCODE_SPACE]`, VK `[0x5A, 0x0D, 0x20]`.
     - `88` (B / Cancel): Android `[KEYCODE_X, KEYCODE_ESCAPE]`, VK `[0x58, 0x1B]`.
     - `38` (UP): Android `[KEYCODE_DPAD_UP]`, VK `[0x26]`.
     - `40` (DOWN): Android `[KEYCODE_DPAD_DOWN]`, VK `[0x28]`.
     - `37` (LEFT): Android `[KEYCODE_DPAD_LEFT]`, VK `[0x25]`.
     - `39` (RIGHT): Android `[KEYCODE_DPAD_RIGHT]`, VK `[0x27]`.
     - `16` (Shift / Dash): Android `[KEYCODE_SHIFT_LEFT]`, VK `[0x10]`.
     - `17` (Ctrl / Skip): Android `[KEYCODE_CTRL_LEFT]`, VK `[0x11]`.
     - `27` (Escape / Menu): Android `[KEYCODE_ESCAPE]`, VK `[0x1B]`.
     - `13` (Enter): Android `[KEYCODE_ENTER]`, VK `[0x0D]`.
     - `32` (Space): Android `[KEYCODE_SPACE]`, VK `[0x20]`.
     - `81` (Q / L): Android `[KEYCODE_Q]`, VK `[0x51]`.
     - `69` (E / R): Android `[KEYCODE_E]`, VK `[0x45]`.
     - Numbers `48..57`: `KEYCODE_0..9`, VK `0x30..0x39`.
     - Letters fallback `65..90`: `KEYCODE_A..Z`, VK `0x41..0x5A`.
     - Function & editing keys: Tab (`9`), Backspace (`8`), F5 (`116`), F9 (`120`), F12 (`123`).
     - Unmapped keys safely return empty lists.

2. **Thread-Safe Virtual Key Buffer & Idempotency:**
   - Initialized `virtualKeyBuffer = ByteArray(256)` and `pressedJsKeys = ConcurrentHashMap.newKeySet<Int>()`.
   - `onKeyDown(jsKeyCode)` validates `pressedJsKeys.add(jsKeyCode)`: duplicate events immediately return early to avoid double firing.
   - Mapped VK codes are set to `0x80.toByte()`.
   - Thread-safe atomic write to `File(filesDir, "rp_input_state.bin")` uses temp-file copy/atomic move semantics.
   - Forwarding to `onNativeKeyDown(androidKey)` dispatches native key events for each mapped Android key code.
   - `onKeyUp(jsKeyCode)` removes the key from `pressedJsKeys`, resets mapped VK bytes to `0x00.toByte()`, reaffirms any remaining pressed keys in the buffer, atomically updates the binary state file, and dispatches `onNativeKeyUp(androidKey)`.

3. **Safe URL Launching:**
   - `openUrl(url)` adds `Intent.FLAG_ACTIVITY_NEW_TASK` and handles exceptions safely without crashing the activity.

4. **Overlay Cleanup:**
   - Removed duplicate `keydown` / `keyup` event listeners from `index.html`. Bridge calls from `vpad.js` are forwarded directly through `RuntimePackVPad` without synthetic event double bubbling.

## Verification & Test Results
- **Command:** `.\gradlew.bat testDebugUnitTest --tests io.runtimepack.host.KeyMappingTest` (working directory: `android/shell`)
- **Outcome:** Passed 11 of 11 tests:
  - `testMapJsKeyActionA`
  - `testMapJsKeyCancelB`
  - `testMapJsKeyDPad`
  - `testMapJsKeyModifiersAndControl`
  - `testMapJsKeyNumbers`
  - `testMapJsKeyLettersFallback`
  - `testMapJsKeyUnknownReturnsEmpty`
  - `testVirtualKeyBufferStateUpdates`
  - `testVirtualKeyBufferAtomicSerialization`
  - `testPressedJsKeysIdempotency`
  - `testMultiKeyBufferStateOverlap`
- **Full Workspace Check:** `cargo test` passed 37 tests (0 failures).

## Concerns & Observations
- None. The binary state file `rp_input_state.bin` is updated atomically with minimal overhead and is ready for Task 3's `GetKeyboardState` shim reader in Ruby.
