# Task 1: Dynamic Icon Extraction from PE Executable & Shell Template Clean-up

## Requirements
1. **Clean static Lona icons from template:**
   - Remove or replace hardcoded Lona icons from `android/shell/app/src/main/res/mipmap-*`.
   - Provide a clean, neutral fallback launcher icon in `android/shell/app/src/main/res/` (or generate on the fly).
2. **Dynamic PE Icon Extractor:**
   - Implement icon extraction for any game being packaged:
     - Check game source directory for executable files (`*.exe`, prioritizing `Game.exe`, `<app_name>.exe`, or first `.exe` with `RT_GROUP_ICON`).
     - Parse the PE `.rsrc` section to find `RT_GROUP_ICON` (14) and `RT_ICON` (3).
     - Extract the highest-resolution icon (e.g. 32x32, 48x48, 128x128, 256x256).
     - Resize to Android launcher mipmap densities:
       - `mipmap-mdpi`: 48x48
       - `mipmap-hdpi`: 72x72
       - `mipmap-xhdpi`: 96x96
       - `mipmap-xxhdpi`: 144x144
       - `mipmap-xxxhdpi`: 192x192
     - Write both `ic_launcher.png` and `ic_launcher_round.png` into the generated project's `app/src/main/res/mipmap-<density>/`.
   - If no `.exe` or icon is found (e.g. pure web game without exe), fallback to neutral launcher icon.
3. **Update `AndroidProjectGenerator::emit_project`:**
   - Allow passing `game_source_dir: Option<&Path>` to `emit_project` in `core/src/build_plan/generator.rs` and update caller in `cli/src/main.rs`.
   - During `emit_project`, generate the dynamic icons for the specific game into `app/src/main/res/`.
4. **Testing:**
   - In `tests/integration/build_test.rs`, assert that `emit_project` generates mipmap folders and launcher icons.
   - Run `cargo test` to verify all 43 tests pass.
5. **Report:**
   Write full report to `.superpowers/sdd/2026-09-23-multi-engine-porting-and-cheat-registry/task-1-report.md`.
