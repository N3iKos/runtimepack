# Task 4 Execution Report: Porting & Validation — The Witch's House MV (RPG Maker MV)

## Summary
- **Status:** DONE
- **Objective:** Port and validate *The Witch's House MV* on Android using the Web runtime (`WebActivity.kt`). Inspect the game directory, verify NW.js deployed package normalization to `www/`, build standalone APK with `runtimepack build`, install to MuMu emulator (`127.0.0.1:16384`), verify interactive title screen and starting the game, capture screenshots, write report in `Docs/test_ports/rpgmv_the_witchs_house_mv/report.md`, and update `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.

## Changes Completed

1. **Static Engine Detection & Inspection:**
   - Ran `cargo run --bin runtimepack -- inspect "Raw-Game\The_Witchs_House_MV"`.
   - Engine detected as **RPG Maker MV** with `Confidence: EXACT` (Score: 210, Status: `COMPATIBLE_WITH_SHIMS`).
   - Static evidence confirmed: `www/index.html` (+40), `www/js/rpg_core.js` (+100), `www/js/rpg_managers.js` (+40), `www/data/System.json` (+30).
   - Compatibility issues correctly identified NW.js desktop wrapper and mapped to web shims: `web.node.path` and `web.nwjs_subset`.

2. **Standalone Android Packaging:**
   - Ran `cargo run --bin runtimepack -- build "Raw-Game\The_Witchs_House_MV" -o dist/witchs_house --package com.example.witchshouse --name "The Witch's House"`.
   - Content packaging root automatically normalized to `www/` (excluding desktop PE binaries: `Game.exe`, `nw.dll`, `node.dll`).
   - GPACK container `game.pack` created (129.81 MB, hash: `547bd15c`).
   - Generated APK: `dist/witchs_house/TheWitchsHouse.apk` (166.84 MB / 174,942,668 bytes, SHA-256: `FBB50C9878F8BF99DEBC3CC18211B3330DB4607B20A9190E631D0C7BADF9AE26`).

3. **Emulator Installation & Live In-Game Verification:**
   - Connected to MuMu Player 12 emulator (`127.0.0.1:16384`).
   - Installed APK: `adb -s 127.0.0.1:16384 install -r dist/witchs_house/TheWitchsHouse.apk`.
   - Cleared app state: `adb -s 127.0.0.1:16384 shell pm clear com.example.witchshouse`.
   - Launched WebActivity: `adb -s 127.0.0.1:16384 shell am start -n com.example.witchshouse/io.runtimepack.host.WebActivity`.
   - Logcat confirmed:
     - `WebActivity: Starting game loading: https://game.local/index.html`
     - `PixiJS 4.5.4 - ✰ WebGL ✰` initialized
     - `RuntimePackHealth: MARKER: GAME_STARTED` and `MARKER: FIRST_FRAME` reached in ~360ms.
     - WebAudio initialized (`audioHardwareBufferSize = 512`).
   - Title screen loaded flawlessly: "The Witch's House MV" with menu options ("Begin", "Continue", "Options") and virtual gamepad overlay.
   - Interacted with menu: selected "Begin", confirmed difficulty ("Easy"), transitioned smoothly into opening forest cutscene.
   - Explored opening forest map: Viola moved responsive to D-Pad inputs.

4. **Visual Evidence & Documentation:**
   - Captured and resized screenshots (960x540):
     - `Docs/test_ports/rpgmv_the_witchs_house_mv/screenshot.png` (Title Screen)
     - `Docs/test_ports/rpgmv_the_witchs_house_mv/screenshot_gameplay.png` (Forest Opening Map)
   - Created `Docs/test_ports/rpgmv_the_witchs_house_mv/report.md` following the canonical template.
   - Updated `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md` marking `[x] The Witch’s House MV`.
   - Cleanly stopped the application via `adb shell am force-stop com.example.witchshouse`.

## Modified & Created Files
- `Docs/test_ports/rpgmv_the_witchs_house_mv/report.md`
- `Docs/test_ports/rpgmv_the_witchs_house_mv/screenshot.png`
- `Docs/test_ports/rpgmv_the_witchs_house_mv/screenshot_gameplay.png`
- `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`
- `.superpowers/sdd/2026-09-23-multi-engine-porting-and-cheat-registry/task-4-report.md`

## Verification
- CLI inspect command: `cargo run --bin runtimepack -- inspect "Raw-Game\The_Witchs_House_MV"` -> Exit 0, EXACT match.
- CLI build command: `cargo run --bin runtimepack -- build "Raw-Game\The_Witchs_House_MV" -o dist/witchs_house --package com.example.witchshouse --name "The Witch's House"` -> Exit 0.
- Live MuMu emulator test: Title Screen, Menu navigation, Difficulty select, and Opening map movement verified.
