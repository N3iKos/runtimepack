# Task 5 Report: Release Build & Host-Side Inspection

**Execution Date:** 2026-09-30  
**Target Game:** Forestia 1.4.4 IND (`D:\02_Development\Projects_2\RPGM-P\Raw-Game\Forestia1.4.4-IND`)  
**Flavor:** `release`  
**Output Directory:** `dist_release/`  
**Generated Package:** `dist_release/Forestia1.4.4-IND.apk` (818.15 MB, 857,896,878 bytes)

---

## 1. Summary of Actions

1. Fixed dynamic resource resolution in `RuntimeIntegrity.kt` (`showCorruptedScreen`) to resolve resource IDs dynamically (`resources.getIdentifier`) with a programmatic fallback rather than compile-time static `R` imports, ensuring full compatibility when building standalone projects with custom package namespaces (`io.runtimepack.game.*`).
2. Executed end-to-end release build:
   ```powershell
   cargo run -q --bin runtimepack -- build "D:\02_Development\Projects_2\RPGM-P\Raw-Game\Forestia1.4.4-IND" --flavor release --output dist_release
   ```
3. Executed host-side inspection on both `Forestia1.4.4-IND.apk` and `game.pack`.
4. Verified that host RAM remained conserved: no emulator was launched or queried during Task 5.

---

## 2. APK Host-Side Inspection

Inspecting `dist_release/Forestia1.4.4-IND.apk` via `System.IO.Compression.ZipFile`:

| Target Path | Expected | Observed Count / Status | Verdict |
|---|---|---|---|
| `assets/overlay/*` | 0 files (Absent) | 0 files | **PASS** |
| `assets/web_shims/*` | 0 files (Absent) | 0 files | **PASS** |
| `assets/fonts/*` | 0 files (Absent) | 0 files | **PASS** |
| `assets/game.pack` | Present | Present (818,750,956 bytes) | **PASS** |
| `classes.dex` | R8 Minified | 5 DEX files processed by `minifyReleaseWithR8` | **PASS** |
| `build-manifest.json` | `flavor: release` | Recorded `"flavor": "release"` | **PASS** |

### Complete list of assets directly inside APK:
- `assets/asset-audit-report.json` (4,451,779 bytes)
- `assets/asset-dependency-graph.json` (4,095,916 bytes)
- `assets/build-manifest.json` (2,121 bytes)
- `assets/game.pack` (818,750,956 bytes)
- `assets/runtimepack_asset_audit.js` (14,677 bytes)
- `assets/runtimepack_asset_audit.rb` (13,811 bytes)
- `assets/runtimepack_bootstrap.rb` (81,201 bytes)

---

## 3. GPACK Container Inspection

Parsing `dist_release/game.pack` header and index (Magic: `GPACK1\0\0`, Version: 1, File count: 3,315 entries):

| Key Entry | Found in GPACK | Size (Bytes) | Stored Size (Bytes) | Encryption Flag |
|---|---|---|---|---|
| `overlay/index.html` | **YES** | 1,928 | 1,928 | 0 |
| `overlay/vpad.js` | **YES** | 153,587 | 153,587 | 0 |
| `web_shims/runtimepack_web_shims.js` | **YES** | 73,658 | 73,658 | 0 |

### Categorized Bundled Entries in GPACK:
- **`overlay/*` (8 entries):**
  - `overlay/cheat_engine.js` (14,062 bytes)
  - `overlay/cheat_menu.js` (66,112 bytes)
  - `overlay/cheat_profile.js` (130 bytes)
  - `overlay/diagnostic_suite.js` (67,430 bytes)
  - `overlay/index.html` (1,928 bytes)
  - `overlay/theme_loader.js` (8,640 bytes)
  - `overlay/vpad.css` (81,750 bytes)
  - `overlay/vpad.js` (153,587 bytes)
- **`web_shims/*` (2 entries):**
  - `web_shims/runtimepack_asset_audit.js` (14,677 bytes)
  - `web_shims/runtimepack_web_shims.js` (73,658 bytes)
- **`fonts/*` (7 entries):**
  - `fonts/BIZUDPGothic-Bold.ttf` (4,488,628 bytes)
  - `fonts/ReadMe_windows.txt` (1,491 bytes)
  - `fonts/VL-Gothic-Regular.ttf` (3,883,852 bytes)
  - `fonts/YasashisaGothicBold-V2.otf` (2,485,140 bytes)
  - `fonts/ko/NanumGothic-Regular.ttf` (4,091,796 bytes)
  - `fonts/zh_CN/NotoSansSC-VariableFont_wght.ttf` (17,805,476 bytes)
  - `fonts/zh_TW/NotoSansTC-VariableFont_wght.ttf` (11,939,708 bytes)

---

## 4. Manual Verification Guide for User (MuMu Player)

When system RAM and resources permit, the user can verify the release APK on MuMu Player 12 with the following steps:

1. **Install Release APK:**
   ```powershell
   adb install -r "D:\02_Development\Projects_2\RPGM-P\.worktrees\asset-hardening\dist_release\Forestia1.4.4-IND.apk"
   ```
2. **Launch Application:**
   ```powershell
   adb shell monkey -p io.runtimepack.game.forestia144ind -c android.intent.category.LAUNCHER 1
   ```
3. **Observations to Confirm:**
   - **No Watermark:** Confirm that the beta watermark (`Beta Test Veldra Community`) is completely absent on the display.
   - **Overlay Operational:** Tap the overlay toggle button to verify the virtual gamepad (vpad) and menu appear properly (served directly from GPACK via `OverlayAssetSource`).
   - **Game Boot:** Confirm the title screen of Forestia loads cleanly without crash or corrupted screen.
   - **Logcat Verification:**
     ```powershell
     adb logcat -d -s RuntimeIntegrity OverlayAssetSource WebBackend
     ```
     Ensure `RuntimeIntegrity` reports `Integrity verification passed` with zero mismatch.
