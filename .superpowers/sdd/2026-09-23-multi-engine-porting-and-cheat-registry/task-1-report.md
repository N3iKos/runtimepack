# Task 1 Execution Report: Dynamic Icon Extraction from PE Executable & Shell Template Clean-up

## Summary
- **Status:** DONE
- **Objective:** Dynamic icon extraction from PE executables during Android packaging, clean-up of hardcoded Lona icons from template to neutral Android launcher gamepad icons, backward compatibility in generator API, and full verification via unit and integration tests.

## Changes Completed

1. **Cleaned Static Lona Icons from Template:**
   - Replaced hardcoded Lona character icons in `android/shell/app/src/main/res/mipmap-*` (`mdpi`, `hdpi`, `xhdpi`, `xxhdpi`, `xxxhdpi`) with a clean, neutral dark slate/cyan gamepad launcher icon template for both standard (`ic_launcher.png`) and round (`ic_launcher_round.png`).
   - Added `ensure_neutral_icons` in `core/src/build_plan/icon.rs` using embedded neutral icon templates to ensure launcher icons always exist even if template directories are missing.

2. **Implemented Dynamic PE Icon Extractor (`core/src/build_plan/icon.rs`):**
   - Implemented PE `.rsrc` section parsing for PE32 and PE32+ executables.
   - Traversed 3-level resource directory hierarchy to locate `RT_GROUP_ICON` (14) and `RT_ICON` (3).
   - Scored icon entries to select the highest-resolution/bpp asset (handling 256x256 dimensions where width/height byte is 0).
   - Handled both raw PNG icon streams (Vista+ modern icons) and synthesized standard ICO streams from DIB data (`BITMAPINFOHEADER` + raw masks).
   - Added Python PIL fallback decoder for exotic DIB formats.
   - Resized extracted icons to standard Android mipmap densities:
     - `mipmap-mdpi`: 48x48
     - `mipmap-hdpi`: 72x72
     - `mipmap-xhdpi`: 96x96
     - `mipmap-xxhdpi`: 144x144
     - `mipmap-xxxhdpi`: 192x192
   - Generated anti-aliased round launcher icons (`ic_launcher_round.png`) with smooth circular alpha masks.
   - Implemented prioritized executable candidate discovery (`Game.exe` / `game.exe`, `<app_name>.exe`, alphabetical fallback, 1-level deep search).

3. **Updated Generator API & CLI:**
   - Added `AndroidProjectGenerator::emit_project_with_source(project_dir, manifest, packed_content, game_source_dir)` in `core/src/build_plan/generator.rs`.
   - Maintained backward-compatible `emit_project(project_dir, manifest, packed_content)` delegating to `emit_project_with_source(..., None)`.
   - Updated `cli/src/main.rs` in `cmd_build` to invoke `emit_project_with_source` passing the game source directory `path`.

4. **Testing & Verification:**
   - Added unit tests in `core/src/build_plan/icon.rs`:
     - `test_extract_icon_from_pe` (verifies 256x256 extraction from real PE).
     - `test_extract_and_generate_icons_flow` (verifies 5 densities, square & round generation).
     - `test_fallback_to_neutral_icons_when_no_exe` (verifies clean neutral fallback).
   - Added integration tests in `tests/integration/build_test.rs`:
     - `test_emit_project_generates_neutral_mipmap_icons_by_default`.
     - `test_emit_project_with_source_extracts_dynamic_pe_icons`.
   - Executed full workspace test suite: all 50 tests passed cleanly.

## Modified Files
- `android/shell/app/src/main/res/mipmap-mdpi/ic_launcher.png`
- `android/shell/app/src/main/res/mipmap-mdpi/ic_launcher_round.png`
- `android/shell/app/src/main/res/mipmap-hdpi/ic_launcher.png`
- `android/shell/app/src/main/res/mipmap-hdpi/ic_launcher_round.png`
- `android/shell/app/src/main/res/mipmap-xhdpi/ic_launcher.png`
- `android/shell/app/src/main/res/mipmap-xhdpi/ic_launcher_round.png`
- `android/shell/app/src/main/res/mipmap-xxhdpi/ic_launcher.png`
- `android/shell/app/src/main/res/mipmap-xxhdpi/ic_launcher_round.png`
- `android/shell/app/src/main/res/mipmap-xxxhdpi/ic_launcher.png`
- `android/shell/app/src/main/res/mipmap-xxxhdpi/ic_launcher_round.png`
- `core/Cargo.toml`
- `Cargo.toml`
- `Cargo.lock`
- `cli/Cargo.toml`
- `core/src/build_plan/mod.rs`
- `core/src/build_plan/icon.rs` (new)
- `core/src/build_plan/generator.rs`
- `cli/src/main.rs`
- `tests/integration/build_test.rs`

## Test Results
Command: `cargo test`
Results:
- `cli` integration tests: 24 passed (`integration_build`), 7 passed (`integration_compatibility`), 9 passed (`integration_detection`)
- `runtimepack-core`: 6 passed (including 3 icon unit tests)
- `runtimepack-pack`: 4 passed
- Total: 50 passed, 0 failed, 0 ignored.
