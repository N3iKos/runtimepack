# Task 5 Brief: JoiPlay / M-Tool Style Dynamic Cheat UI in `cheat_menu.js`

## Context & Vision
Provide a flexible, user-driven cheat interface inspired by JoiPlay and M-Tool:
1. Users can browse and edit all engine variables and switches by name or ID.
2. Quick party utilities & item spawner.
3. Interactive Script Console (M-Tool style) where users can run custom code snippets, view returned output, and save favorite scripts into local storage bookmarks.

## Requirements & Scope
- Files to modify:
  - `android/shell/app/src/main/assets/overlay/cheat_menu.js`
- Files to create:
  - `android/shell/app/src/test/js/cheat_menu.test.js`

### Detailed UI Architecture in `cheat_menu.js`:
1. **Four Core Tabs**:
   - `#fnh-tab-variables` (Variables Browser):
     - Search input (`#fnh-var-search`) filtering by name or ID.
     - Dynamic cards displaying `[#ID] Name = Value`.
     - Inline value edit field and "Set" button triggering `window.CheatEngine.setVariable(id, val)`.
     - "Refresh" button querying `window.CheatEngine.dumpState()` / `window.CheatEngine.getState()`.
   - `#fnh-tab-switches` (Switches Browser):
     - Search input (`#fnh-switch-search`) filtering by name or ID.
     - Cards showing `[#ID] Name` with toggle button (ON/OFF).
     - Clicking button calls `window.CheatEngine.setSwitch(id, newValue)`.
   - `#fnh-tab-spawner` (Item Spawner & Quick Cheats):
     - Quick Action Buttons:
       - "Max Gold" -> `window.CheatEngine.gainGold(999999)`
       - "Recover All" -> `window.CheatEngine.recoverAll()`
     - Spawner panel:
       - Dropdown for Category: "item", "weapon", "armor".
       - Input for ID and Count (+1, +5, +50, or custom amount).
       - Button "Spawn Item" -> `window.CheatEngine.gainItem(type, id, count)`.
   - `#fnh-tab-console` (M-Tool Script Console & Snippet Manager):
     - Multi-line `<textarea id="fnh-console-input">` placeholder: `e.g. $game_player.move_speed = 6`.
     - Button "▶ Run Script" calling `window.CheatEngine.evalScript(code, callback)`.
     - Output terminal `<pre id="fnh-console-output">` showing return value or diagnostic error.
     - Bookmark / Snippet Manager:
       - Input for Snippet Name.
       - Button "💾 Save Snippet" storing `{ name, code }` in `localStorage` (`vpad_user_cheat_snippets`).
       - List or select dropdown of saved snippets to load or delete.

2. **Theming & Touch Compatibility**:
   - Respect `.vpad-theme-retro` CSS rules as defined in existing stylesheet.
   - Maintain clean styles for inputs, buttons, and scrollable panels.
   - Guard against missing browser objects (`document`, `window`) to allow clean CommonJS export for testing.

3. **Testing**:
   - In `android/shell/app/src/test/js/cheat_menu.test.js`:
     - Test tab switching.
     - Test variable search and set dispatch.
     - Test switch toggle dispatch.
     - Test console script execution and output rendering.
     - Test snippet persistence in mock `localStorage`.
     - Run: `node android/shell/app/src/test/js/cheat_menu.test.js`.

4. **Report Contract**:
   - Full report in `.superpowers/sdd/2026-09-18-input-sync-and-agnostic-cheat-engine/task-5-report.md`.
   - Return status `DONE`, commits, test output summary, concerns.
