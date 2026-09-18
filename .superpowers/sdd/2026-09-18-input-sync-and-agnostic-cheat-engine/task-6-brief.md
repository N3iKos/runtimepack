# Task 6 Brief: End-to-End Build & Validation on Emulator

## Context
Tasks 1 through 5 have implemented and unit-tested:
- Direct native bridge and VenoraTL community link in `vpad.js` and `index.html`.
- Virtual Key buffer and multi-binding action key mapping in `RgssActivity.kt`.
- Win32API keyboard hook (`GetKeyboardState`, `GetAsyncKeyState`, `GetKeyState`) and native `Input` fallback in `runtimepack_bootstrap.rb`.
- Engine-agnostic core cheat runner with frame hook and atomic IPC command queue.
- JoiPlay and M-Tool style dynamic cheat UI (Variables, Switches, Spawner, Script Console with snippet bookmarks).

Now, compile the standalone Android APK for Lona RPG, deploy it to MuMu Player (ADB `127.0.0.1:16385`), and verify functional in-game execution.

## Requirements & Scope
1. **Asset & Source Synchronization**:
   - Copy updated files from `android/shell/app/src/main/`:
     - `assets/overlay/*` (`vpad.js`, `cheat_engine.js`, `cheat_menu.js`, `index.html`) -> `dist/lona/android-project/app/src/main/assets/overlay/`
     - `assets/runtimepack_bootstrap.rb` -> `dist/lona/android-project/app/src/main/assets/runtimepack_bootstrap.rb`
     - `java/io/runtimepack/host/RgssActivity.kt` -> `dist/lona/android-project/app/src/main/java/io/runtimepack/host/RgssActivity.kt`
   - Also ensure `GpackExtractor.kt` in `android/shell` and `dist/lona/android-project` copies `runtimepack_bootstrap.rb` to `destDir` if `game.pack` is extracted.

2. **Compile APK**:
   - In `dist/lona/android-project`:
     Run `.\gradlew.bat assembleDebug`
   - Copy generated APK (`app/build/outputs/apk/debug/app-debug.apk`) to `dist/lona/LonaRPG.apk`.

3. **Install & Launch via ADB**:
   - Use ADB at `C:\Users\Nekooz\AppData\Local\Android\Sdk\platform-tools\adb.exe` targeting `127.0.0.1:16385`:
     - Install: `& "$adb" -s 127.0.0.1:16385 install -r dist/lona/LonaRPG.apk`
     - Launch: `& "$adb" -s 127.0.0.1:16385 shell am start -n com.example.lonarpg/io.runtimepack.host.RgssActivity`

4. **Verify Live Game & Diagnostics**:
   - Inspect logcat: `& "$adb" -s 127.0.0.1:16385 logcat -d -s RuntimePackRgss SDL/APP ActivityTaskManager`
   - Verify `rp_input_state.bin` is updated on key taps.
   - Confirm game boots and Title screen renders without crashing.

5. **Report Contract**:
   - Full report in `.superpowers/sdd/2026-09-18-input-sync-and-agnostic-cheat-engine/task-6-report.md`.
   - Return status `DONE`, commits, test output summary, concerns.
