# Task 6 Execution Report: Porting & Validation — BLACK SOULS (RPG Maker VX Ace / RGSS3)

## Summary
Ported and validated *BLACK SOULS* (`Raw-Game/BLACK SOULS`) on Android using the RGSS backend (`RgssActivity.kt` / `libmkxp-z.so`). Handled missing Windows RTP background layer dependencies, Ruby string/regex encoding compatibility in custom Ruby scripts, and BMSP map fog nil safety. Built standalone APK, verified execution and interactive in-game gameplay on MuMu Player (`127.0.0.1:16384`), captured gameplay screenshot, and documented findings in `Docs/test_ports/rpgvxa_black_souls/report.md` and `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.

---

## 1. Engine Inspection
- **Command:** `cargo run --bin runtimepack -- inspect "Raw-Game\BLACK SOULS"`
- **Result:**
  - Indexed 980 files (788.89 MB, sha256: `3962e62b`)
  - Engine: RPG Maker VX Ace
  - Confidence: `EXACT` (Score: 240)
  - Status: `COMPATIBLE`
  - Runtime Family: RGSS
  - Evidence: `Data/Scripts.rvdata2` (+120), `Game.ini` (+80), `Game.ini` (+30), `Game.exe` (+10)

---

## 2. Build Artifacts
- **Command:** `cargo run --bin runtimepack -- build "Raw-Game\BLACK SOULS" -o dist/black_souls --package com.example.blacksouls --name "Black Souls"`
- **Package ID:** `com.example.blacksouls`
- **Output APK:** `dist/black_souls/BlackSouls.apk`
- **APK Size:** 866,127,071 bytes (825.95 MB)
- **APK SHA-256:** `0FCBAA8DE465EE0AF42CFD1C5C0D8F25AD169B1BF4E4096A32E76DA5514717DF`
- **Container (`game.pack`):** 980 files (788.97 MB, hash: `a71f9c34`)

---

## 3. Shims & Compatibility Fixes
1. **Missing RTP Title Background Layer (`Graphics/Titles2/Mist`):**
   - In standard Windows RPG Maker VX Ace, `Game.ini` declares `rtp = RPGVXAce` and `Data/System.rvdata2` sets `@title2_name = "Mist"`. This asset is part of the Windows Enterbrain RTP installer and is not distributed in standalone game files, triggering `Errno::ENOENT` in `Cache.title2` / `Bitmap.new`.
   - Updated `android/shell/app/src/main/assets/runtimepack_bootstrap.rb` to ensure `Bitmap.new` and `Cache.normal_bitmap` hooks are active from bootstrap startup, safely falling back to a transparent 32x32 empty bitmap instead of failing with an uncaught `Errno::ENOENT`.
2. **Ruby Regexp Encoding Compatibility (`Encoding::CompatibilityError`):**
   - Custom script `ステータス限界突破` (Status Limit Break) line 153 executes regex scanning with fullwidth colon `[\xef\xbc\x9a:]` against actor `note` strings. Decompressing scripts via `Zlib::Inflate.inflate` in Ruby defaults their string encoding to `ASCII-8BIT`, raising `Encoding::CompatibilityError: incompatible encoding regexp match (ASCII-8BIT regexp with UTF-8 string)` when matched against UTF-8 game data.
   - Fixed by forcing `# encoding: utf-8\n` and `force_encoding("UTF-8")` on each script before `eval`, and adding failsafe monkey-patches on `Regexp#=~` and `String#scan`.
3. **BMSP Map Fog NilClass Error (`NoMethodError: undefined method 'empty?' for nil:NilClass`):**
   - The map fog script `hulogu` line 270 called `@bmsp_fog[:file].empty?` during `post_transfer` / `fadein` on the prologue map where no fog is defined (`@bmsp_fog` is nil or empty).
   - Added polyfill `NilClass#empty?` (returning `true`) in `runtimepack_bootstrap.rb` and guarded `Game_Map#bmsp_mapfog_update_fog`.
4. **Font Glyph Substitution:**
   - Added `VL Gothic>Noto Sans CJK TC` and `UmePlus Gothic>Noto Sans CJK TC` to `defaultFontSubs` in `GpackExtractor.kt` to ensure complete Japanese/Chinese punctuation coverage (e.g. Japanese quotes `「` / `」`).

---

## 4. Emulator Validation & Gameplay Verification
- **Target Device:** MuMu Player (`127.0.0.1:16384`, Android 12 x86_64, Native ARM64 translation)
- **Installation:** `adb -s 127.0.0.1:16384 install -r dist/black_souls/BlackSouls.apk` (Success)
- **Startup:** Clean launch via `adb shell am start -n com.example.blacksouls/io.runtimepack.host.RgssActivity`.
- **Title Screen:** Rendered cleanly with "Game Baru", "Lanjutkan", "Keluar Game". Title BGM `タイトル、アリス.mp3` played through OpenAL Soft.
- **In-Game Gameplay:** Started "Game Baru", transitioned from `Scene_Title` to `Scene_Map`, transferred to the prologue room, triggered Alice event and dialogue box ("Alice: Ya, aku bicara padamu. Selamat. Kamu terpilih menjadi protagonis dari cerita ini.").
- **Screenshot:** Captured active gameplay screen and saved 960x540 image to `Docs/test_ports/rpgvxa_black_souls/screenshot.png`.
- **Clean Exit:** Force-stopped cleanly via `adb shell am force-stop com.example.blacksouls`.

---

## 5. Artifacts & Documentation
- `Docs/test_ports/rpgvxa_black_souls/report.md` created.
- `Docs/test_ports/rpgvxa_black_souls/screenshot.png` captured (960x540).
- `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md` updated with `[x] Black Souls`.

---

## Status
DONE
