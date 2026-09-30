# Load Chain Diagnostic Report

- **Date:** 2026-09-30
- **Game:** Forestia 1.4.4-IND (RPG Maker MZ)
- **Target Package:** `io.runtimepack.game.forestia144ind`
- **Instrumentation Commit:** `be978f9` (`diag(web-runtime): instrument MZ load chain with RP_LOAD_DEBUG stage loggers`)
- **Diagnostic Log Path:** `C:\Users\Nekooz\AppData\Local\Temp\opencode\forestia_saves\logcat_diag.txt`

---

## 1. Build and Installation Verification

1. **Unit Test Suite:**
   - Ran `node android/shell/app/src/test/js/web_shims.test.js`
   - Result: All tests passed (100% green).
2. **Commit:**
   - Hash: `be978f9`
   - File: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
3. **APK Compilation:**
   - Command: `cargo run -q --bin runtimepack -- build "D:\02_Development\Projects_2\RPGM-P\Raw-Game\Forestia1.4.4-IND" --flavor beta --output dist`
   - Manifest package: `io.runtimepack.game.forestia144ind`
   - Shim confirmation: Verified 23 `RP_LOAD_DEBUG` log hooks inside `dist/android-project/app/src/main/assets/web_shims/runtimepack_web_shims.js`.
4. **Emulator Installation:**
   - Target: MuMu Player 12 (vmindex 0)
   - APK: `dist/Forestia1.4.4-IND.apk`
   - Save migration directory `/data/data/io.runtimepack.game.forestia144ind/files/saves/mz/` survived reinstall with all files intact (`config`, `file0`, `file1`, `global`, `system`).

---

## 2. Tap Test (Single-Tap on Slot "File 1")

### Verbatim Log Sequence

```text
[RP_LOAD_DEBUG] 1790713035678 onTouchSelect trigger=true hitIndex=1 index=1
[RP_LOAD_DEBUG] 1790713035697 onTouchOk touchOkEnabled=true
[RP_LOAD_DEBUG] 1790713035697 processOk idx=1
[RP_LOAD_DEBUG] 1790713035698 onSavefileOk
[RP_LOAD_DEBUG] 1790713035700 executeLoad id=1
[RP_LOAD_DEBUG] 1790713035701 DataManager.loadGame savefileId=1
[RP_LOAD_DEBUG] 1790713035701 StorageManager.loadObject saveName=file1
[RP_LOAD_DEBUG] 1790713035855 StorageManager.loadObject resolved saveName=file1 type=object length=30
[RP_LOAD_DEBUG] 1790713036311 loadGame rejected windowLayer.children[0].convertEscapeCharacters is not a function
[RP_LOAD_DEBUG] 1790713036311 onLoadFailure
```

### Stage Where Chain Stops

The chain stops inside **`DataManager.loadGame()` promise resolution** during save content extraction (`extractSaveContents`). It rejects with:
`TypeError: windowLayer.children[0].convertEscapeCharacters is not a function`
and immediately drops into `Scene_Load.prototype.onLoadFailure()`.

---

## 3. Enter / Virtual Gamepad A Test (Slot "File 1")

### Verbatim Log Sequence

```text
[RP_LOAD_DEBUG] 1790713107926 input ok pressed (keyCode=13, code=Enter)
[RP_LOAD_DEBUG] 1790713107926 input ok pressed (keyCode=13, code=Enter)
[RP_LOAD_DEBUG] 1790713107926 input ok pressed (RuntimePackInput)
[RP_LOAD_DEBUG] 1790713107927 processOk idx=1
[RP_LOAD_DEBUG] 1790713107928 onSavefileOk
[RP_LOAD_DEBUG] 1790713107929 executeLoad id=1
[RP_LOAD_DEBUG] 1790713107930 DataManager.loadGame savefileId=1
[RP_LOAD_DEBUG] 1790713107930 StorageManager.loadObject saveName=file1
[RP_LOAD_DEBUG] 1790713108011 StorageManager.loadObject resolved saveName=file1 type=object length=30
[RP_LOAD_DEBUG] 1790713108394 loadGame rejected windowLayer.children[0].convertEscapeCharacters is not a function
[RP_LOAD_DEBUG] 1790713108394 onLoadFailure
```

### Stage Where Chain Stops

Identical failure point: `DataManager.loadGame()` promise rejection with `windowLayer.children[0].convertEscapeCharacters is not a function` -> `onLoadFailure()`.

---

## 4. Root Cause Analysis

1. **Storage Bridge Works:**
   `StorageManager.loadObject("file1")` successfully returned the parsed 30-property save object in ~154ms through the native Android storage bridge.
2. **Input and Selection Work:**
   Both touch input (`onTouchSelect` + `onTouchOk` -> single-tap OK enabled) and key input (`Input._onKeyDown` / `RuntimePackInput`) successfully trigger `Window_SavefileList.prototype.processOk` and `Scene_Load.prototype.executeLoad`.
3. **The Crash Trigger:**
   During `DataManager.extractSaveContents(contents)`, game objects and event meta are initialized.
   The game includes `HalfMove.js` and `HalfMoveEx.js` (Triacontane / Inazuma). In `HalfMove.js` line 618 and `HalfMoveEx.js` line 94:
   ```javascript
   function convertEscapeCharacters(text_) {
       ...
       const windowLayer = SceneManager._scene?._windowLayer;
       return windowLayer
           ? windowLayer.children[0].convertEscapeCharacters(text)
           : text;
   }
   ```
   In RPG Maker MZ, when `ConfigManager.touchUI === true`, `Scene_MenuBase.prototype.createButtons` creates `_cancelButton` (`Sprite_Button`) and adds it to `_windowLayer` *before* other windows (`addWindow(this._cancelButton)`).
   Because the active scene during `DataManager.loadGame()` is still `Scene_Load`, `SceneManager._scene._windowLayer.children[0]` is a `Sprite_Button` instance, NOT a `Window_Base` instance.
   `Sprite_Button` does not have `convertEscapeCharacters`, throwing `TypeError` and causing `DataManager.loadGame()` promise rejection and silent fallback to `onLoadFailure()`.

---

## 5. Anomalies & Notes

- **`isLoadableSlot` & `onLoadInvalid`:**
  These loggers were never called because standard RPG Maker MZ core and Forestia's plugins do not define `isLoadableSlot` or `onLoadInvalid`. The engine uses `isSavefileEnabled()` which calls `Window_SavefileList.prototype.isEnabled()`, and failures route directly to `onLoadFailure()`.
- **Instrumentation Status:**
  Retained in `runtimepack_web_shims.js` pending controller review.
