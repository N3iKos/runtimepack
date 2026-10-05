# Implementation Plan: Dynamic Icon Extraction, Fingerprint Cheat Registry & Multi-Engine Porting Pipeline

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 
1. Fix the APK icon pipeline so launcher icons are dynamically extracted from each game's Windows `.exe` at build time (avoiding static Lona icon bleeding).
2. Implement a fingerprint-based cheat profile registry (`profiles/games/<fingerprint>/cheats.js`) so each game gets its own dedicated cheat menu without hardcoding, and immediately implement Termina's profile.
3. Establish an autonomous subagent porting & testing workflow across all engines in `Raw-Game/` (*To the Moon*, *The Witch's House MV*, *Forestia*, *BLACK SOULS*, *NTRdemic / show_off*), testing on emulator, verifying in-game interaction up to Title/Start, capturing screenshots, and recording test reports.

**Architecture:** 
- **Dynamic Icon Extraction:** `core/src/build_plan/generator.rs` inspects the game's executable (`Game.exe` or root `.exe`), parses PE `.rsrc` icon headers, generates Android mipmaps (`mdpi`..`xxxhdpi`), and cleans static icons out of the shell template.
- **Fingerprint-Based Cheat Registry:** The build planner computes a deterministic SHA-256 fingerprint from engine markers (`System.json` / `Game.ini` / `package.json`). If a matching profile exists in `profiles/games/<id>/cheats.js`, it is injected into the build. The default template remains neutral/clean.
- **Autonomous Multi-Engine Validation Loop:** Subagents run sequentially for each game: inspect -> build -> install to emulator -> launch -> test in-game -> screenshot -> close -> write report to `Docs/test_ports/<engine>_<game>/report.md` -> update `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.

**Global Constraints:**
- Source directories in `Raw-Game/` are STRICTLY READ-ONLY and immutable.
- **NEVER touch, scan, or access any directory named `antigravity` or `.antigravity`.**
- Target emulator: MuMu Player at `127.0.0.1:16384`.
- All errors must use standard domains (`DET-xxx`, `CMP-xxx`, `BLD-xxx`, `AND-xxx`).

---

### Task 1: Dynamic Icon Extraction from PE Executable & Shell Template Clean-up

**Files:**
- Modify: `core/src/build_plan/generator.rs`
- Modify: `cli/src/main.rs`
- Delete: `android/shell/app/src/main/res/mipmap-*/ic_launcher*.png` (replaced by clean default launcher icon)
- Test: `tests/integration/build_test.rs`

**Interfaces:**
- Consumes: `input_path` (path to game directory).
- Produces: Dynamic mipmaps in `app/src/main/res/mipmap-{mdpi,hdpi,xhdpi,xxhdpi,xxxhdpi}/` derived from `input_path/*.exe` icon resource; fallback to neutral Android robot icon if no icon resource.

- [ ] **Step 1: Write integration test for dynamic icon extraction**
Assert that when building a project with an `.exe` containing an icon, the generated Android project has mipmaps matching the game's icon rather than a hardcoded template icon.

- [ ] **Step 2: Clean static Lona icons from `android/shell/app/src/main/res`**
Remove hardcoded Lona icons from template so builds without an icon do not accidentally inherit Lona's face.

- [ ] **Step 3: Implement PE Icon extractor in Rust (`core/src/build_plan/generator.rs` or `pack/src/icon.rs`)**
Extract `RT_GROUP_ICON` / `RT_ICON` from PE executable, decode via `image` crate, and write resized mipmap PNGs directly to `app/src/main/res/mipmap-*` during `emit_project`.

- [ ] **Step 4: Verify test passes**
Run `cargo test --test integration_build`.

---

### Task 2: Fingerprint-Based Cheat Registry & Fear & Hunger 2: Termina Profile

**Files:**
- Create: `profiles/games/lona/cheats.js` (move Lona cheats here)
- Create: `profiles/games/termina/cheats.js` (Termina cheats: Save Anywhere, Coin Flip Always Heads, Restore Body & Mind, Clear Hunger, Revive Limbs, Add Shillings)
- Modify: `android/shell/app/src/main/assets/overlay/cheat_profile.js` (clean/neutral fallback template)
- Modify: `core/src/build_plan/generator.rs` (fingerprint matcher & profile injector)
- Test: `tests/integration/build_test.rs`

**Interfaces:**
- Consumes: Game source files (`System.json` / `Game.ini`).
- Produces: Injects matching `cheats.js` from `profiles/games/<id>/` into `app/src/main/assets/overlay/cheat_profile.js` if matched; leaves neutral template if no profile exists.

- [ ] **Step 1: Create modular profile structure in `profiles/games/`**
Move Lona RPG cheats to `profiles/games/lona/cheats.js`.
Reset `android/shell/app/src/main/assets/overlay/cheat_profile.js` to an empty neutral structure (`window.GAME_CHEATS = { title: "Game Cheats", sections: [] };`).

- [ ] **Step 2: Create Termina cheat profile in `profiles/games/termina/cheats.js`**
Define categories:
- *System & Save:* Open Save Menu Anywhere, Quick Save, Always Win Coin Flip (`$gameVariables.setValue(1, 1)`).
- *Vitals & Stats:* Full Restore (Body 100, Mind 100), Clear Hunger (`$gameVariables.setValue(10, 0)`), Cure All Negative Statuses.
- *Economy:* Add 100 Shillings, Add Soul Stone (`$dataItems[98]`).

- [ ] **Step 3: Implement fingerprint / game-ID resolver in `generator.rs`**
Match game by identifier (e.g. `System.json` title, package name, or marker hash). If matched, copy the profile over `cheat_profile.js`.

- [ ] **Step 4: Verify Termina build**
Rebuild Termina:
`cargo run --bin runtimepack -- build "Raw-Game\Fear & Hunger 2 Termina" -o dist/termina --package com.example.termina --name "Termina"`
Verify: Termina APK has its own icon, and Cheat Menu shows Termina's cheat profile.

---

### Task 3: Porting & Validation — *To the Moon* (RPG Maker XP / RGSS1)

**Files:**
- Target: `Raw-Game/To the Moon`
- Report: `Docs/test_ports/rpgxp_to_the_moon/report.md`
- Matrix: `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`

- [ ] **Step 1: Inspect and resolve RGSS1 dependencies**
Run `runtimepack inspect "Raw-Game\To the Moon"`. Check for desktop DLLs (`fluidsynth.dll`, `steam_api.dll`) and configure shims.
- [ ] **Step 2: Build APK**
`cargo run --bin runtimepack -- build "Raw-Game\To the Moon" -o dist/to_the_moon --package com.example.tothemoon --name "To the Moon"`
- [ ] **Step 3: Test on MuMu Emulator via subagent**
Install, launch, verify Title Screen, press New Game, advance intro, verify audio/controls.
- [ ] **Step 4: Capture screenshot & write report**
Save screenshot to `Docs/test_ports/rpgxp_to_the_moon/screenshot.png`.
Write `Docs/test_ports/rpgxp_to_the_moon/report.md` and check item in `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.

---

### Task 4: Porting & Validation — *The Witch's House MV* (RPG Maker MV)

**Files:**
- Target: `Raw-Game/The_Witchs_House_MV`
- Report: `Docs/test_ports/rpgmv_the_witchs_house_mv/report.md`

- [ ] **Step 1: Inspect & Build APK**
`cargo run --bin runtimepack -- build "Raw-Game\The_Witchs_House_MV" -o dist/witchs_house --package com.example.witchshouse --name "The Witch's House"`
- [ ] **Step 2: Test on MuMu Emulator via subagent**
Verify Title Screen, New Game, character control in forest, audio BGM.
- [ ] **Step 3: Write report & update matrix**
Record results in `Docs/test_ports/rpgmv_the_witchs_house_mv/report.md` and update `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.

---

### Task 5: Porting & Validation — *Forestia* (RPG Maker MZ)

**Files:**
- Target: `Raw-Game/Forestia1.4.4-IND`
- Report: `Docs/test_ports/rpgmz_forestia/report.md`

- [ ] **Step 1: Inspect & Build APK**
`cargo run --bin runtimepack -- build "Raw-Game\Forestia1.4.4-IND" -o dist/forestia --package com.example.forestia --name "Forestia"`
- [ ] **Step 2: Test on MuMu Emulator via subagent**
Verify Title Screen, New Game, MZ WebAudio/WebGL renderer, virtual pad touch.
- [ ] **Step 3: Write report & update matrix**
Record results in `Docs/test_ports/rpgmz_forestia/report.md` and update `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.

---

### Task 6: Porting & Validation — *BLACK SOULS* (RPG Maker VX Ace / RGSS3)

**Files:**
- Target: `Raw-Game/BLACK SOULS`
- Report: `Docs/test_ports/rpgvxa_black_souls/report.md`

- [ ] **Step 1: Inspect & Build APK**
`cargo run --bin runtimepack -- build "Raw-Game\BLACK SOULS" -o dist/black_souls --package com.example.blacksouls --name "Black Souls"`
- [ ] **Step 2: Test on MuMu Emulator via subagent**
Verify Title Screen, New Game, intro scene, map movement.
- [ ] **Step 3: Write report & update matrix**
Record results in `Docs/test_ports/rpgvxa_black_souls/report.md` and update `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.

---

### Task 7: Porting & Validation — *TyranoScript* (*[Kimochi] NTRdemic* or *show_off*)

**Files:**
- Target: `Raw-Game/[Kimochi] NTRdemic...` or `Raw-Game/[Kimochi] show_off`
- Report: `Docs/test_ports/tyrano_kimochi/report.md`

- [ ] **Step 1: Asar unpacker integration for Electron Tyrano packages**
Add support in `runtimepack` to inspect/extract `resources/app.asar` when encountering Electron-wrapped Tyrano games.
- [ ] **Step 2: Build & Test APK on MuMu Emulator**
Verify TyranoScript HTML5 canvas boots to Title Screen, dialogue advances on tap.
- [ ] **Step 3: Write report & update matrix**
Record results in `Docs/test_ports/tyrano_kimochi/report.md` and update `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.
