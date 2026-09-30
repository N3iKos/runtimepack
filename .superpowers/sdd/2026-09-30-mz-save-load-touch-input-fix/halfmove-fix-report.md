# HalfMove Compatibility Fix Report

## 1. Plugin Analysis (Phase 2)

### Call Sites in Plugins
Two plugins in Forestia 1.4.4 contain the problematic pattern:
1. `D:\02_Development\Projects_2\RPGM-P\Raw-Game\Forestia1.4.4-IND\js\plugins\triacontane\HalfMove.js` (lines 613-619):
```javascript
var convertEscapeCharacters = function(text) {
    if (text === null || text === undefined) {
        text = '';
    }
    var windowLayer = SceneManager._scene._windowLayer;
    return windowLayer ? windowLayer.children[0].convertEscapeCharacters(text) : text;
};
```
   - Used in `getArgString` (line 605) and `getArgNumber` (line 587).
   - Called during `Game_Event.prototype.initialize` on event metadata (`Width`, `Height`, `TriggerExpansion`, `ExpansionArea`) and in `Game_Event.prototype.getExpansionArea` / `setupPage`.

2. `D:\02_Development\Projects_2\RPGM-P\Raw-Game\Forestia1.4.4-IND\js\plugins\inazuma\thirdpartyex\HalfMoveEx.js` (lines 87-96):
```javascript
function convertEscapeCharacters(text_) {
    let text = text_;
    if (text === null || text === undefined) {
        text = "";
    }
    const windowLayer = SceneManager._scene?._windowLayer;
    return windowLayer
        ? windowLayer.children[0].convertEscapeCharacters(text)
        : text;
}
```
   - Used in `getArgString` (line 98), `getArgArrayFloat` (line 114), and `getExpansionArea` (line 123).

### Exact Code Path Hit During `Scene_Load` Save Extraction
1. Player selects File 1 on `Scene_Load`.
2. `StorageManager.loadObject("file1")` resolves with save contents.
3. `DataManager.loadGame` invokes `DataManager.extractSaveContents(contents)`.
4. Plugin `inazuma/npc/GlobalMap.js` overrides `DataManager.extractSaveContents`:
   - Calls `$globalMap.extractSaveContents(contents)`
   - Calls `$globalMap.setup()` -> `setupNonPlayers()`
   - Instantiates `new GlobalMap.MB_NonPlayer(npc, ...)`
5. `MB_NonPlayer` extends `Game_Event`.
   - `Game_Event.prototype.initialize` calls `this.getExpansionArea()`.
   - `getExpansionArea` reads meta tags and passes them to `convertEscapeCharacters(text)`.
6. Inside `convertEscapeCharacters`, the active scene is `SceneManager._scene` (`Scene_Load`).
7. It accesses `SceneManager._scene._windowLayer.children[0]`.

### Why PC/NW.js and JoiPlay Do Not Hit This Error
1. **Pre-shipped config with `touchUI: false`:**
   In Forestia 1.4.4's PC distribution, `save/config.rmmzsave` is bundled with `touchUI: false`.
2. **Game-level default:**
   `inazuma/System.js` sets `ConfigManager.touchUI = false;`.
3. **Menu window layer hierarchy:**
   In RPG Maker MZ `rmmz_scenes.js`:
   ```javascript
   Scene_MenuBase.prototype.create = function() {
       Scene_Base.prototype.create.call(this);
       this.createBackground();
       this.updateActor();
       this.createWindowLayer();
       this.createButtons();
   };
   Scene_MenuBase.prototype.createButtons = function() {
       if (ConfigManager.touchUI) {
           if (this.needsCancelButton()) {
               this.createCancelButton(); // adds _cancelButton (Sprite_Button) to _windowLayer
           }
           ...
       }
   };
   ```
   - When `ConfigManager.touchUI === false` (PC / JoiPlay with bundled config), `createButtons()` does not instantiate or add `_cancelButton`.
   - Next, `Scene_File.prototype.create` adds `_helpWindow` (`Window_Help` -> `Window_Base`).
   - Consequently, on PC, `_windowLayer.children[0]` is `_helpWindow` (an instance of `Window_Base`), where `convertEscapeCharacters` exists and functions normally.
4. **WebView environment difference:**
   - In the WebView APK environment, the app starts with a fresh storage directory without a pre-existing `config.rmmzsave`.
   - When `ConfigManager.load()` runs, `ConfigManager.applyData({})` defaults `touchUI` to `true` (`this.touchUI = this.readFlag(config, "touchUI", true)`).
   - With `touchUI === true`, `createButtons()` runs and adds `_cancelButton` (`Sprite_Button`) as `children[0]` of `_windowLayer`.
   - `Sprite_Button` does not inherit from `Window_Base` and has no `convertEscapeCharacters` method, throwing `TypeError`.

---

## 2. Chosen Fix Shape

### Chosen Shape: Option B (Delegate from `Sprite_Button.prototype.convertEscapeCharacters`)
- **Implementation:**
  Implement `Sprite_Button.prototype.convertEscapeCharacters(text)` in `runtimepack_web_shims.js`:
  1. Inspect `SceneManager._scene`. If `_helpWindow` exists and implements `convertEscapeCharacters`, delegate to it.
  2. Otherwise, iterate `_windowLayer.children` and delegate to the first child that implements `convertEscapeCharacters` and is not `this`.
  3. If no window exists in the scene, fall back to a cached dummy `Window_Base` instance (`new Window_Base(new Rectangle(0, 0, 0, 0))`) to ensure standard escape sequences (including `\N[n]`, `\P[n]`, and plugin hooks) evaluate without exception.
  4. If `Window_Base` is not available, return `text` untouched.
  5. Log `[RP_LOAD_DEBUG] halfmove compat applied` on patch initialization.
- **Why this shape:**
  - Strictly preserves PC parity: when `children[0]` is already a `Window_Base` (PC / `touchUI === false`), `Sprite_Button.prototype.convertEscapeCharacters` is never invoked.
  - When `touchUI === true` (mobile / WebView), `_cancelButton` delegates cleanly to the actual `Window_Base` instances (`_helpWindow` or `_listWindow`), running through all plugin hooks.
  - Completely non-destructive and independent of game file modifications.

---

## 3. Test Results
- Unit test suite: `node android/shell/app/src/test/js/web_shims.test.js`
- Result: **PASS** (100% green).
- Output:
```
Running Mobile Web Shims Unit Tests...
[RP_LOAD_DEBUG] 1790713871656 input ok pressed (keyCode=0, code=KeyZ)
Engine Input Patch tests PASSED
[RP_LOAD_DEBUG] halfmove compat applied
HalfMove Compat tests PASSED
All Mobile Web Shims tests passed successfully!
```
- Added test coverage:
  - Verified `Sprite_Button.prototype.convertEscapeCharacters` patched.
  - Verified delegation to `SceneManager._scene._helpWindow`.
  - Verified delegation to first `Window_Base` in `SceneManager._scene._windowLayer.children` when `_helpWindow` is null.
  - Verified fallback to `Window_Base.prototype` when scene has no windows.
  - Verified PC parity when `windowLayer.children[0]` is a `Window_Base`.

---

## 4. Build and Install Verification
- **Commit:** `a14860b` (`fix(web-runtime): compat for HalfMove plugin expecting Window_Base at windowLayer.children[0] during Scene_Load extraction`).
- **Build Command:** `cargo run -q --bin runtimepack -- build "D:\02_Development\Projects_2\RPGM-P\Raw-Game\Forestia1.4.4-IND" --flavor beta --output dist`
- **Build Status:** SUCCESS (APK: `dist\Forestia1.4.4-IND.apk`, 816.58 MB).
- **Shim Verification:** Confirmed `patchHalfMoveCompat` present in `dist\android-project\app\src\main\assets\web_shims\runtimepack_web_shims.js`.
- **Install:** Reinstalled over existing package `io.runtimepack.game.forestia144ind` on MuMu emulator (vmindex 0).
- **Save Preservation:** Verified `/data/data/io.runtimepack.game.forestia144ind/files/saves/mz/` (config, file0, file1, global, system) remained intact across reinstall.

---

## 5. Emulator Verification Result

### Single-Tap Load Attempt (Touch Navigation)
- **Action:** Launched game -> selected "Lanjutkan" -> single tap on File 1 (`input -d 13 tap 500 340`).
- **Result:** **PASS** — Game transitioned from load menu into the map with character rendered in room beside bed.
- **Logcat Evidence:**
```text
09-30 03:40:02.703  8104  8104 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714402702 onTouchSelect trigger=true hitIndex=1 index=1 (:1418)
09-30 03:40:02.703  8104  8104 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714402703 onTouchOk touchOkEnabled=true (:1431)
09-30 03:40:02.703  8104  8104 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714402703 processOk idx=1 (:1397)
09-30 03:40:02.704  8104  8104 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714402703 onSavefileOk (:1449)
09-30 03:40:02.704  8104  8104 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714402704 executeLoad id=1 (:1477)
09-30 03:40:02.705  8104  8104 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714402705 DataManager.loadGame savefileId=1 (:1526)
09-30 03:40:02.706  8104  8104 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714402706 StorageManager.loadObject saveName=file1 (:1552)
09-30 03:40:02.846  8104  8104 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714402845 StorageManager.loadObject resolved saveName=file1 type=object length=30 (:1560)
09-30 03:40:06.366  8104  8104 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714406366 loadGame resolved (:1532)
09-30 03:40:06.366  8104  8104 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714406366 onLoadSuccess (:1489)
09-30 03:40:06.370  8104  8104 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714406369 SceneManager.goto Scene_Map (:1581)
```

### On-Screen Virtual Gamepad A Button Attempt
- **Action:** Restarted app -> opened "Lanjutkan" -> tapped on-screen green `A` button (`input -d 14 tap 1710 840`).
- **Result:** **PASS** — Game transitioned from load menu into the map with character rendered in room.
- **Logcat Evidence:**
```text
09-30 03:43:29.639  8477  8477 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714609638 input ok pressed (keyCode=90, code=KeyZ) (:1303)
09-30 03:43:29.639  8477  8477 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714609639 input ok pressed (RuntimePackInput) (:1342)
09-30 03:43:29.652  8477  8477 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714609647 processOk idx=1 (:1397)
09-30 03:43:29.653  8477  8477 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714609649 onSavefileOk (:1449)
09-30 03:43:29.653  8477  8477 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714609651 executeLoad id=1 (:1477)
09-30 03:43:29.653  8477  8477 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714609651 DataManager.loadGame savefileId=1 (:1526)
09-30 03:43:29.653  8477  8477 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714609652 StorageManager.loadObject saveName=file1 (:1552)
09-30 03:43:29.788  8477  8477 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714609787 StorageManager.loadObject resolved saveName=file1 type=object length=30 (:1560)
09-30 03:43:33.221  8477  8477 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714613221 loadGame resolved (:1532)
09-30 03:43:33.221  8477  8477 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714613221 onLoadSuccess (:1489)
09-30 03:43:33.225  8477  8477 I WebActivity: [GameConsole] [RP_LOAD_DEBUG] 1790714613224 SceneManager.goto Scene_Map (:1581)
```

- **Saved Verification Artifacts:**
  - `C:\Users\Nekooz\AppData\Local\Temp\opencode\forestia_saves\07_after_tap_file1.png` (Single-tap load entering map)
  - `C:\Users\Nekooz\AppData\Local\Temp\opencode\forestia_saves\08_after_a_button.png` (Virtual Gamepad A-button load entering map)
  - `C:\Users\Nekooz\AppData\Local\Temp\opencode\forestia_saves\logcat_fix_verify.txt` (Complete filtered logcat dump)
