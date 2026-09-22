# Task 2: Fingerprint-Based Cheat Registry & Fear & Hunger 2: Termina Profile

## Goal
Implement a modular cheat profile registry under `profiles/games/<game_id>/cheats.js`. The build generator deterministically identifies the game by inspecting source markers (e.g. `Game.ini` title or `System.json` gameTitle) and copies the matching profile to `assets/overlay/cheat_profile.js`. If no profile matches, a clean neutral template is used.

## Requirements
1. **Move Lona Cheats to Registry:**
   - Create `profiles/games/lona/cheats.js` containing Lona RPG's specific cheats.
   - Reset `android/shell/app/src/main/assets/overlay/cheat_profile.js` to a clean neutral template with empty sections (`window.GAME_CHEATS = { title: "Game Cheats", sections: [] };`).
2. **Create Fear & Hunger 2: Termina Profile:**
   - Create `profiles/games/termina/cheats.js` with SVG icons and no text slop:
     - Categories:
       - *System & Save:* Open Save Menu Anywhere (`SceneManager.push(Scene_Save)`), Always Win Coin Flip (`$gameVariables.setValue(1, 1)`).
       - *Vitals & Body:* Full Restore Body & Mind (`$gameParty.members().forEach(m => { m.setHp(m.mhp); m.setMp(m.mmp); })`), Clear Hunger (`$gameVariables.setValue(10, 0)`), Restore Limbs & Statuses.
       - *Economy:* Add 100 Shillings (`gain_gold`), Add Soul Stone.
3. **Implement Profile Matcher in `core/src/build_plan/generator.rs`:**
   - When emitting project:
     - Inspect `game_source_dir`:
       - If `Game.ini` has `Title=LonaRPG` or `lona` in path -> match `"lona"`.
       - If `System.json` (at root or in `www/data/System.json`) has `"gameTitle":"Termina"` or `Termina` in path -> match `"termina"`.
     - If matched and `profiles/games/<id>/cheats.js` exists, copy it to `project_dir/app/src/main/assets/overlay/cheat_profile.js`.
     - Otherwise, leave the neutral template.
4. **Testing:**
   - Add integration test in `tests/integration/build_test.rs` asserting profile matching and neutral fallback.
   - Run `cargo test`.
5. **Rebuild Termina & Device Verification:**
   - Build Termina: `cargo run --bin runtimepack -- build "Raw-Game\Fear & Hunger 2 Termina" -o dist/termina --package com.example.termina --name "Termina"`.
   - Verify Termina launcher icon is extracted from `Termina` (not Lona).
   - Install to MuMu emulator (`127.0.0.1:16384`), launch, open cheat menu, and capture screenshot to `C:\Users\Nekooz\AppData\Local\Temp\opencode\termina_cheat_menu_verified.png`.
6. **Report:**
   Write full report to `.superpowers/sdd/2026-09-23-multi-engine-porting-and-cheat-registry/task-2-report.md`.
