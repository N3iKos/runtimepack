# Task 4: Floating Diagnostic & Automated Stress-Test Suite

## Goal
Implement the agent-friendly floating diagnostic suite and automated 1-click stress test engine in `diagnostic_suite.js` (overlay) and `runtimepack_bootstrap.rb` (RGSS engine), producing machine-readable `audit_report.json` and `bug_report.json`.

## Requirements
1. In `android/shell/app/src/main/assets/overlay/diagnostic_suite.js`:
   - Inject floating bug icon `[ 🐞 ]` in `#vpad-topbar`.
   - Implement Diagnostic Modal:
     - **Live Error Log:** Catches and displays exceptions/warnings transparently.
     - **Automated Stress Test Button `[ Run Full Stress Test ]`:** Dispatches `run_stress_test` action to engine.
     - **Map Warper / Teleport:** Input Map ID, X, Y -> dispatches `teleport_map`.
     - **Export Bug Report `[ Export Bug Report ]`:** Dispatches `export_bug_report` and writes `bug_report.json`.
     - **FPS & Memory Meter:** Updates real-time stats in the modal.
2. In `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`:
   - Handle `"run_stress_test"`:
     - Scans `Data/Map*.rvdata2` (or `Data/Map*.rxdata`).
     - Inspects all event commands:
       - Code 201 (Transfer player): records target map ID.
       - Code 231 (Show Picture): verifies `Graphics/Pictures/<name>`.
       - Code 241 (Play BGM): verifies `Audio/BGM/<name>`.
       - Code 245 (Play BGS): verifies `Audio/BGS/<name>`.
       - Code 250 (Play SE): verifies `Audio/SE/<name>`.
       - Code 322 (Change Actor Graphic): verifies character & face names.
     - Inspects Database files: `Data/Enemies.rvdata2`, `Data/Actors.rvdata2`, `Data/Tilesets.rvdata2`, etc.
     - Performs case-insensitive vs exact disk checks.
     - Compiles findings into `audit_report.json` in `filesDir` (or `filesDir/game/`).
   - Handle `"teleport_map"`:
     - Calls `$game_player.reserve_transfer(map_id, x, y)` if `$game_player` is present.
   - Handle `"export_bug_report"`:
     - Captures current scene name, map ID, player X and Y, and writes `bug_report.json`.
3. In `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`:
   - Support `"run_stress_test"`, `"teleport_map"`, and `"export_bug_report"` for Web MV/MZ engines.
4. Flavor Awareness in `core/src/build_plan/generator.rs`:
   - Load `diagnostic_suite.js` in `index.html` ONLY when `flavor == "beta"`.
5. Unit Tests:
   - In `android/shell/app/src/test/js/`: Add test verifying `diagnostic_suite.js` initialization.
   - In `android/shell/app/src/test/ruby/`: Add test verifying `run_stress_test` serialization.
   - Run tests: `cargo test` and `.\gradlew.bat testDebugUnitTest`.
6. Write execution report to `.superpowers/sdd/2026-09-23-beta-diagnostic-and-blacksouls-audit/task-4-report.md`.

## Critical Constraints
- NEVER touch, read, or write any folder named `antigravity` or `.antigravity`.
- Source directories in `Raw-Game/` are strictly READ-ONLY.
