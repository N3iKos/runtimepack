# Forensic Diagnostic Report — King Exit RGSS Save/Load & Playtime Runtime Fix

> **Correction after independent review (2026-10-20):** Sections attributing the screenshot's numbers to serialized playtime or monotonic nanoseconds are NOT confirmed. The screenshot is the main menu; command/help labels are replaced with colon-separated numbers, including minute-like values 67/84. Standard `%60` playtime formatting cannot explain them, and the report's 32-bit frame-counter evidence conflicts with the 10^13 raw-frame hypothesis. These causal statements below are retained only as superseded investigation history. Do not implement timer conversions or clamps from them. Follow `.superpowers/sdd/2026-10-20-king-exit-save-load-runtime-fix/task-4-reinvestigation.md` when available. The wrong INI API argument mapping remains independently confirmed; full post-load causality requires validation.

**Date:** 2026-10-20  
**Worktree:** `D:\02_Development\Projects_1\runtimepack-vnm-reconstruction\.worktrees\fix-mvmz-logical-root`  
**Task:** Task 1 of King Exit RGSS Save/Load Runtime Fix Plan  
**Investigator:** Bug Hunter Sub-Agent (Read-Only)  
**Status:** COMPLETE (Findings Delivered for Executor)

---

## 1. Executive Summary

A read-only forensic investigation was executed to diagnose two primary defects in the King Exit Android APK:
1. **Language Reversion After Save/Load:** Actor name reverts from `Samidare` to `サミダレ`, map name reverts from `Lokasi Terowongan Bawah Tanah` to `地下深く暗い道`, and menu labels revert to Japanese.
2. **Oversized Playtime Metadata:** Save-list slots display values such as `49037624:37:19`, `72266426:38:21`, and `86258219:33:20`.

### Key Forensic Conclusions:
- **Root Cause 1 (Language Reversion): CONFIRMED.** The Android shims in `win32api_shim.rb:35-44` and `runtimepack_bootstrap.rb:351-361` contain defective implementations of `GetPrivateProfileString` and `WritePrivateProfileString`. The destination buffer pointer is read from `args[4]` (which is `nSize`, an Integer) instead of `args[3]` (`lpReturnedString`), causing the shim to return `0` without reading `./Game.ini`. This causes Langscore's initialization (`LSSetting.load`) to receive an empty string `""`, failing validation and resetting `$langscore_current_language` to `"ja"`. Furthermore, Langscore intentionally serializes original Japanese text during `make_save_contents` and relies on `extract_save_contents` calling `Langscore.changeLanguage($langscore_current_language, true)` post-load. Because `$langscore_current_language` is `"ja"`, post-load translation reconstruction forces all actor names, maps, and menus back into Japanese. Additionally, `WritePrivateProfileString` is a dummy returning `1`, preventing any in-game language option changes from persisting to `./Game.ini`.
- **Root Cause 2 (Playtime Display Location): CONFIRMED.** `Window_SaveFile#draw_playtime` (Script 071:50) directly renders `header[:playtime_s]` loaded from `DataManager.load_header(@file_index)`. The oversized string is **serialized at save time** into the save header dictionary (`header[:playtime_s]`), not computed dynamically at display time.
- **Root Cause 3 (Playtime Oversized Origin): HYPOTHESIS (High Mathematical Confidence).** Deconstruction of `49037624:37:19` reveals total seconds = `176,535,448,639`, which corresponds to `10,592,126,918,340` frames at 60 fps. In nanoseconds, `10,592,126,918,340 ns` = `10,592.126 s` = `2h 56m 32s`. The three reported times (`49037624:37:19`, `72266426:38:21`, `86258219:33:20`) precisely match monotonic system uptime intervals (`CLOCK_MONOTONIC` / `System.nanoTime()`) sampled over consecutive play sessions (uptime ~2.94h, ~4.33h, ~5.17h). Once a save file header or `$game_system.@frames_on_save` contains this corrupted value, loading restores it to `Graphics.frame_count`, propagating it to every subsequent save.
- **Byte-Level User Save Inspection: BLOCKED.** No copy of the user's corrupted save file (`Save*.rvdata2`) exists in the repository or temporary workspace. Exact binary inspection of the user's specific serialized payload cannot proceed without a save copy.

---

## 2. Evidence Inspected

### 2.1 Repository & Android Host Source
1. `android/shell/app/src/main/assets/win32api_shim.rb` (lines 35-44)
2. `android/shell/app/src/main/assets/runtimepack_bootstrap.rb` (lines 51-61, 283-379, 978-1078, 1846-1910, 1950-2035)
3. `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt` (lines 445-465)
4. `android/shell/app/src/main/java/io/runtimepack/host/SaveService.kt` (lines 10-52)
5. `android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt` (lines 272-284, 310-325)
6. `pack/src/gpack.rs` (lines 9-31, 201-285)

### 2.2 Packaged Game Content (`game.pack`)
Inspected via non-destructive read-only unpacking of `/home/mhsymr/mumu_font_workaround_20261019_190949/app/src/main/assets/game.pack`:
1. `Game.ini`:
   ```ini
   [Game]
   Library=System\RGSS301.dll
   Scripts=Data\Scripts.rvdata2
   Title=KING EXIT
   [Langscore]
   Lang=en
   ```
2. `Data/Translate/Actors.csv`:
   - Header: `original,en,ja,zh-cn`
   - Row 1: `original` = Japanese text; `en` = Indonesian translation ("Ratu dari negeri yang jauh. Dia jadi korban jebakan orang lain..."); `ja` = Japanese text.
3. `Data/Scripts.rvdata2`:
   - Decompressed all 222 Ruby scripts from the binary Marshal dump:
     - Script 004 `DataManager` (267 lines)
     - Script 010 `Game_System` (122 lines)
     - Script 071 `Window_SaveFile` (69 lines)
     - Script 109 `Scene_File` (203 lines)
     - Script 110 `Scene_Save` (38 lines)
     - Script 111 `Scene_Load` (40 lines)
     - Script 121 `langscore` (1209 lines)
     - Script 131 `共通セーブファイル` (102 lines)
     - Script 174 `テストプレー高速化` (265 lines)
     - Script 181 `Custom Menu Base` (1670 lines)
     - Script 217 `langscore_custom` (14906 lines)

### 2.3 Native Shared Libraries
- `android/shell/app/src/main/jniLibs/arm64-v8a/libmkxp-z.so`:
  - `_ZNK8Graphics13getFrameCountEv` at `0x57eb70`: loads 32-bit `w0` from `[x8, #0x214]`.
  - `_ZN8Graphics13setFrameCountEi` at `0x57eba4`: stores 32-bit `w8` into `[x9, #0x214]`.
  - `_ZN8Graphics6updateEb` at `0x57d7a0`: increments 32-bit `[x10, #0x214]` via `add w9, w9, #1` at `0x57d864`.
  - ELF relocations: confirmed dynamic linking to `clock_gettime@LIBC` and `gettimeofday@LIBC`.

---

## 3. Language & INI Data Flow Analysis

### 3.1 Win32 Profile API Contract vs. Actual Langscore Calls
In Script 121 (`langscore`), lines 1178-1197:
```ruby
class LSSetting
  GetPrivateProfileString = Win32API.new('kernel32', 'GetPrivateProfileString', %w(p p p p l p), 'L')
  WritePrivateProfileString = Win32API.new('kernel32', 'WritePrivateProfileString', %w(p p p p), 'i')

  BufferSize = 16+1

  def self.load
    $langscore_current_language = Langscore::DEFAULT_LANGUAGE
    result = ' ' * BufferSize
    r = GetPrivateProfileString.call("Langscore", "Lang", "ja", result, BufferSize, "./Game.ini")
    if r != BufferSize-2
      $langscore_current_language = result.slice(0...r)
      p "Langscore Load ini : #{$langscore_current_language}"
    end
  end

  def self.save
    WritePrivateProfileString.call("Langscore", "Lang", $langscore_current_language, "./Game.ini")
  end
end

LSSetting.load()
```

#### Standard Win32 API Argument Mapping:
`GetPrivateProfileStringA(lpAppName, lpKeyName, lpDefault, lpReturnedString, nSize, lpFileName)`:
- `args[0]` (`p`): Section name (`"Langscore"`)
- `args[1]` (`p`): Key name (`"Lang"`)
- `args[2]` (`p`): Default value if missing (`"ja"`)
- `args[3]` (`p`): Destination buffer string (`result` = `' ' * 17`)
- `args[4]` (`l`): Buffer capacity in characters (`BufferSize` = `17`)
- `args[5]` (`p`): INI file path (`"./Game.ini"`)
- Return value (`L`): Number of characters copied into `args[3]` (excluding terminating NUL). If key not found, copies `lpDefault` into `args[3]` and returns `strlen(lpDefault)`.

`WritePrivateProfileStringA(lpAppName, lpKeyName, lpString, lpFileName)`:
- `args[0]` (`p`): Section name (`"Langscore"`)
- `args[1]` (`p`): Key name (`"Lang"`)
- `args[2]` (`p`): Value string to write (`$langscore_current_language`)
- `args[3]` (`p`): INI file path (`"./Game.ini"`)
- Return value (`i`): `1` (TRUE) on success, `0` (FALSE) on failure.

### 3.2 Current Android Shim Defects
In `win32api_shim.rb:35-44` and `runtimepack_bootstrap.rb:351-361`:
```ruby
    when 'GetPrivateProfileString'
      buffer = args[4]
      if buffer.is_a?(String)
        value = args[1].to_s
        buffer[0, value.bytesize] = value
        value.bytesize
      else
        0
      end
    when 'WritePrivateProfileString'
      1
```

#### Defect Breakdown:
1. **Wrong Buffer Argument Index:** The shim inspects `args[4]`, which in the standard contract is the integer `nSize` (`17`). Since `17.is_a?(String)` is `false`, the shim branches to `else: 0`.
2. **Ignored INI File:** The shim never inspects `args[5]` (`"./Game.ini"`). It completely fails to read `Game.ini` from disk.
3. **Flawed Value Assignment:** Even if `args[4]` had been a string, the code assigned `args[1]` (the key name `"Lang"`!), not the value from the file or the default (`args[2]`).
4. **Synthetic Non-Persistent Write:** `WritePrivateProfileString` returns `1` immediately without writing or persisting anything to disk.

### 3.3 Cascade into Langscore Language State & Save/Load
1. **At Boot:**
   - `LSSetting.load` runs.
   - `GetPrivateProfileString.call` returns `0`.
   - `r = 0`. Since `0 != 15` (`BufferSize - 2`), line 1189 runs:
     `$langscore_current_language = result.slice(0...0)` -> `""` (empty string).
   - In `DataManager.init -> Langscore.get_available_languages` (lines 324-327):
     `""` is not found in `@@available_languages` (`["ja", "en", "zh-cn"]`).
     Langscore resets: `$langscore_current_language = Langscore::DEFAULT_LANGUAGE` (`"ja"`).
   - `DataManager.load_normal_database` executes:
     `Langscore.changeLanguage("ja", true)`.
2. **Saving (`DataManager.make_save_contents`, lines 899-917):**
   ```ruby
   def self.make_save_contents
     data_temp = Marshal.dump($game_actors)
     actors = $game_actors.instance_variable_get(:@data)
     actors.each.with_index do |actor,i|
       next unless actor
       actor_id = actor.instance_variable_get(:@actor_id)
       $game_actors[actor_id].name     = Langscore.fetch_original_text(actor.name, $ls_actor_tr)
       $game_actors[actor_id].nickname = Langscore.fetch_original_text(actor.nickname, $ls_actor_tr)
     end
     result = ls_make_save_contents
     $game_actors = Marshal.load(data_temp)
     result
   end
   ```
   Langscore **intentionally sanitizes actor names back to original Japanese (`fetch_original_text`)** before writing to the save file. This design ensures save files are language-agnostic.
3. **Loading (`DataManager.extract_save_contents`, lines 921-927):**
   ```ruby
   def self.extract_save_contents(contents)
     ls_extract_save_contents(contents)
     Langscore.update_language_state_variables()
     Langscore.changeLanguage($langscore_current_language, true)
   end
   ```
   - When loading, `ls_extract_save_contents(contents)` restores `$game_actors` from the save file (which contains the original Japanese: `サミダレ`).
   - Line 926 immediately calls `Langscore.changeLanguage($langscore_current_language, true)`.
   - On PC, `$langscore_current_language` is `"en"` (loaded from `Game.ini`), so `changeLanguage("en", true)` translates `サミダレ` -> `Samidare`, and `地下深く暗い道` -> `Lokasi Terowongan Bawah Tanah`.
   - On Android, because `GetPrivateProfileString` failed, `$langscore_current_language` is `"ja"`.
   - `changeLanguage("ja", true)` re-evaluates database and actor strings against column `ja`, leaving actor names and maps permanently in Japanese.
   - If the player manually selected Indonesian via `Scene_Language`, `LSSetting.save` failed to persist `Lang=en` via the shim. As soon as the APK process restarts, `$langscore_current_language` reverts to `"ja"`.

---

## 4. Playtime Data Flow Analysis

### 4.1 Save Header vs. Save-List Rendering
1. **Rendering Boundary:** Script 071 (`Window_SaveFile`), lines 47-51:
   ```ruby
   def draw_playtime(x, y, width, align)
     header = DataManager.load_header(@file_index)
     return unless header
     draw_text(x, y, width, line_height, header[:playtime_s], 2)
   end
   ```
   `Window_SaveFile` performs **zero calculation or formatting**. It directly renders the string stored in `header[:playtime_s]`.
2. **Save Time Generation Boundary:** Script 004 (`DataManager`), lines 199-204:
   ```ruby
   def self.make_save_header
     header = {}
     header[:characters] = $game_party.characters_for_savefile
     header[:playtime_s] = $game_system.playtime_s
     header
   end
   ```
   The playtime string is generated **at the moment the user saves** by calling `$game_system.playtime_s`.
3. **Playtime Calculation Boundary:** Script 010 (`Game_System`) & Script 174 (`テストプレー高速化`):
   ```ruby
   def playtime_count(count)
     count / Graphics.frame_rate
   end

   def playtime_s_count(count)
     hour = playtime_count(count) / 60 / 60
     min = playtime_count(count) / 60 % 60
     sec = playtime_count(count) % 60
     sprintf("%02d:%02d:%02d", hour, min, sec)
   end

   def playtime_s
     count = SpeedUp::TIME_NO_SPD ? Graphics.spd_frame_count : Graphics.frame_count
     playtime_s_count(count)
   end
   ```
   Both implementations calculate:
   $$\text{playtime} = \left\lfloor \frac{\text{count}}{\text{Graphics.frame\_rate}} \right\rfloor$$
   $$\text{hour} = \left\lfloor \frac{\text{playtime}}{3600} \right\rfloor, \quad \text{min} = \left\lfloor \frac{\text{playtime}}{60} \right\rfloor \pmod{60}, \quad \text{sec} = \text{playtime} \pmod{60}$$

### 4.2 Mathematical Deconstruction of Observed Values
| Save Slot String | `hour` | `min` | `sec` | Total Seconds ($S$) | Frames at 60 fps ($F = S \times 60$) | Nanoseconds ($F / 10^9$) | Uptime Interpretation |
|---|---|---|---|---|---|---|---|
| `49037624:37:19` | 49,037,624 | 37 | 19 | 176,535,448,639 | 10,592,126,918,340 | 10,592.127 s | **2h 56m 32.1s** |
| `72266426:38:21` | 72,266,426 | 38 | 21 | 260,159,135,901 | 15,609,548,154,060 | 15,609.548 s | **4h 20m 09.5s** |
| `86258219:33:20` | 86,258,219 | 33 | 20 | 310,529,590,400 | 18,631,775,424,000 | 18,631.775 s | **5h 10m 31.7s** |

#### Key Deductions:
1. The numbers are **not random memory garbage**.
2. Notice the deltas between the three saves:
   - Slot 1 to Slot 2: $+1\text{h } 23\text{m } 37\text{s}$
   - Slot 2 to Slot 3: $+0\text{h } 50\text{m } 22\text{s}$
   This precisely matches realistic gameplay intervals between consecutive save points on an active Android device.
3. The underlying raw value fed into `count` is on the order of $10^{13}$, which matches **system monotonic uptime in nanoseconds** (`CLOCK_MONOTONIC` or `System.nanoTime()`).
4. In `Game_System` (Script 010):
   ```ruby
   def on_before_save
     @frames_on_save = Graphics.frame_count
     ...
   end
   def on_after_load
     Graphics.frame_count = @frames_on_save
     ...
   end
   ```
   Once a save file serializes `@frames_on_save` with this nanosecond-magnitude value, loading that save immediately assigns `Graphics.frame_count = @frames_on_save`, thereby permanently corrupting in-memory frame counts for all subsequent saves created in that session.

---

## 5. Status Classification per Issue

| Issue Area | Status | Evidence / Blocking Factor |
|---|---|---|
| **Win32 Profile API Compatibility** | **CONFIRMED** | `win32api_shim.rb:35-44` and `runtimepack_bootstrap.rb:351-361` check `args[4]` instead of `args[3]`, ignore `./Game.ini`, and stub `WritePrivateProfileString`. |
| **Language Restoration Ordering** | **CONFIRMED** | Script 121 (`make_save_contents`:899-917 and `extract_save_contents`:921-927) proves save payloads intentionally contain original Japanese, relying on `$langscore_current_language` to restore Indonesian post-load. |
| **Playtime Display vs Serialization** | **CONFIRMED** | Script 071 (`Window_SaveFile`:50) proves display code only draws `header[:playtime_s]`; the string is serialized at save time by Script 004 (`DataManager`:202). |
| **Playtime Nanosecond Uptime Origin** | **HYPOTHESIS** | Proven mathematically ($F/10^9$ yields 2h56m, 4h20m, 5h10m), but the exact injection path into `Graphics.frame_count` or `@frames_on_save` requires dynamic runtime tracing or binary save inspection. |
| **User Save Byte-Level Payload** | **BLOCKED** | No copy of the user's corrupted save file exists in the repository or temporary files. Byte-level verification of existing user save files is blocked until a save copy is provided. |

---

## 6. Exact File and Line References

| Component | File Path | Line Numbers | Description of Role |
|---|---|---|---|
| Win32API Shim | `android/shell/app/src/main/assets/win32api_shim.rb` | Lines 35-44 | Defective `GetPrivateProfileString` & `WritePrivateProfileString` shims |
| Runtime Bootstrap | `android/shell/app/src/main/assets/runtimepack_bootstrap.rb` | Lines 351-361 | Identical defective shims in primary runtime bootstrap |
| Bootstrap Save Dirs | `android/shell/app/src/main/assets/runtimepack_bootstrap.rb` | Lines 55-61 | Sets `MKXP.save_directory` to `"./."` (working dir) |
| Android Activity | `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt` | Lines 450-465 | Manages `gameDir` extraction and `mkxp-z` native launch |
| Android SaveService | `android/shell/app/src/main/java/io/runtimepack/host/SaveService.kt` | Lines 18-52 | Web-only save service (not used by `RgssActivity`) |
| Gpack Extractor | `android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt` | Lines 273-283, 310-325 | Copies `Game.ini` and configures `mkxp.json` |
| Langscore Core | `Data/Scripts.rvdata2` -> Script 121 (`langscore`) | Lines 1178-1197 | `LSSetting.load` and `LSSetting.save` profile calls |
| Langscore Init | `Data/Scripts.rvdata2` -> Script 121 (`langscore`) | Lines 30, 324-327 | Default language reset fallback on empty string |
| Langscore Save/Load | `Data/Scripts.rvdata2` -> Script 121 (`langscore`) | Lines 899-927 | Japanese sanitization on save, restoration on load |
| DataManager Save Header| `Data/Scripts.rvdata2` -> Script 004 (`DataManager`) | Lines 160-166, 199-204 | Serialization of `header[:playtime_s]` |
| Game_System Playtime | `Data/Scripts.rvdata2` -> Script 010 (`Game_System`) | Lines 80-109 | Calculation of `playtime_s` from `Graphics.frame_count` |
| Save Window Display | `Data/Scripts.rvdata2` -> Script 071 (`Window_SaveFile`) | Lines 47-51 | Verbatim rendering of `header[:playtime_s]` |
| SpeedUp Playtime Hook | `Data/Scripts.rvdata2` -> Script 174 (`テストプレー高速化`)| Lines 117-135, 178-184 | SpeedUp hook on `playtime_s` and `spd_frame_count` |
| mkxp-z Native Binary | `android/shell/app/src/main/jniLibs/arm64-v8a/libmkxp-z.so` | `0x57eb70`, `0x57eba4`, `0x57d864` | `getFrameCount`, `setFrameCount`, `update` (`frameCount++`) |

---

## 7. Smallest Safe Fix Boundary for Implementation

### Boundary 1: INI Profile Compatibility (Task 2)
Modify `android/shell/app/src/main/assets/win32api_shim.rb` and `runtimepack_bootstrap.rb`:
1. **`GetPrivateProfileString` Contract Enforcement:**
   - Parameter signature: `args[0]` (section), `args[1]` (key), `args[2]` (default), `args[3]` (buffer), `args[4]` (buffer size), `args[5]` (filename).
   - Resolve `filename` safely within the app-private game directory (e.g. `./Game.ini` -> `File.expand_path(filename)`).
   - Read and parse INI sections and keys using a lightweight parser.
   - If found, copy the string into `args[3]` (truncated to `args[4] - 1`) and return copied byte length.
   - If missing, copy `args[2]` (default) into `args[3]` and return `args[2].bytesize`.
2. **`WritePrivateProfileString` Persistence:**
   - Parameter signature: `args[0]` (section), `args[1]` (key), `args[2]` (value), `args[3]` (filename).
   - Persist changes to the INI file in the app-private working directory.
   - Return `1` on successful write, `0` on write failure. Reject path traversal attempts.

### Boundary 2: Language Restoration Ordering (Task 3)
1. In `runtimepack_bootstrap.rb`:
   - With Boundary 1 in place, `LSSetting.load` reads `Lang=en` from `Game.ini` during script evaluation, correctly initializing `$langscore_current_language = "en"`.
   - Add a generic RGSS post-load hook for games featuring Langscore: ensure that before `DataManager.extract_save_contents` calls `changeLanguage`, `$langscore_current_language` is verified against the game's configured INI language.
   - **Constraint:** Do NOT hardcode `"en"` globally for unrelated games. The language restored must come from the game's actual configuration (`Game.ini`).

### Boundary 3: Playtime Serialization & Legacy Sanitization (Task 4)
1. In `runtimepack_bootstrap.rb`:
   - Intercept `Game_System#playtime_s` or sanitize at the `DataManager.make_save_header` boundary.
   - **Validation Rule:**
     If `count` exceeds normal gameplay limits (e.g. `count > 100_000_000`, ~460 hours of play), detect whether it represents nanosecond uptime (`count > 10^10`):
     - If nanoseconds detected: compute seconds as `count / 1_000_000_000 / 60` or normalize `Graphics.frame_count`.
     - Bound the hour field to a sane ceiling (e.g. `999:59:59` or normalized value) and log a diagnostic warning (`[RuntimePack] Sanitized oversized playtime counter`).
   - Add defensive legacy handling in `DataManager.load_header`: if a previously saved header contains an invalid/oversized string like `49037624:37:19`, normalize or clamp it so the save-load window displays a sensible representation without crashing or misrepresenting playtime.

---

## 8. Unresolved Limitations

1. **No User Save Copy in Workspace:** Direct binary inspection of the user's specific `.rvdata2` save file could not be performed.
2. **Device-Side Dynamic Validation:** Final runtime validation remains strictly owned by the user on the beta APK (`PENDING_USER_DEVICE_VALIDATION`). Host unit tests will verify the contracts, but cannot replace physical touch/SDL/Ruby lifecycle validation on Android.
