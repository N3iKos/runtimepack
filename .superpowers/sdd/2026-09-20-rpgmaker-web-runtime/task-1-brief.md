# Task 1: Deployed NW.js Normalization in RpgMakerWebAnalyzer and Ingest

## Goal
Enable `RpgMakerWebAnalyzer` and `runtimepack build` to recognize deployed NW.js packages (where web assets reside in `www/`), ignore root-level desktop NW.js runner DLLs (`nw.dll`, `node.dll`, `ffmpeg.dll`, `d3dcompiler_47.dll`, etc.) from raising fatal `CMP-102` errors, and normalize the content packaging root to `www/`.

## Requirements
1. In `engines/rpgmaker_web/src/scanner.rs`:
   - Detect whether the game is structured as a deployed NW.js wrapper:
     `let is_nwjs_wrapper = source.has_file("www/index.html") || source.has_file("www/js/rpg_core.js") || source.has_file("www/js/rmmz_core.js");`
   - In loop scanning for native files (`.node`, `.dll`, `.so`):
     If `is_nwjs_wrapper` is true, ignore files that do NOT start with `"www/"` (these are desktop wrapper artifacts outside the web content root).
2. In `cli/src/main.rs`:
   - When preparing `content_source_dir` for packaging:
     If the detected engine is `EngineId::RpgMakerMv` or `EngineId::RpgMakerMz`, and `path.join("www/index.html").exists()` or `path.join("www/js/rpg_core.js").exists()`:
     Normalize content source directory to `path.join("www")`.
3. In `tests/integration/compatibility_test.rs`:
   - Add unit/integration test `test_rpgmaker_mv_nwjs_wrapper_normalizes_desktop_dlls`:
     Create a temporary fixture with `nw.dll`, `node.dll`, `www/index.html`, and `www/js/rpg_core.js`.
     Run `RpgMakerMvDetector.probe` and `RpgMakerWebAnalyzer.analyze`.
     Assert `report.status != CompatibilityStatus::UnsupportedDependency`.
     Assert no `CMP-102` diagnostic issue exists for `nw.dll` or `node.dll`.
4. Run tests:
   `cargo test --test integration_compatibility` and `cargo test`.
5. Write execution report to `.superpowers/sdd/2026-09-20-rpgmaker-web-runtime/task-1-report.md`.
