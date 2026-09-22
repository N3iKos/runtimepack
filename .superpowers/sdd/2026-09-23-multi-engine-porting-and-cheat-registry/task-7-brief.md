# Task 7: Porting & Validation — TyranoScript ([Kimochi] NTRdemic / show_off)

## Goal
Implement support for Electron-packaged TyranoBuilder / TyranoScript games (`resources/app.asar` containing `index.html`, `tyrano/`, `data/`). The subagent must:
1. Enable `engines/tyrano` to detect Electron-wrapped TyranoScript games by reading the inner `app.asar` archive or providing an ingest normalizer.
2. In `engines/tyrano`, recognize `tyrano/tyrano.js`, `data/system/Config.tjs`, `index.html`.
3. Support packaging the extracted web tree into `game.pack`.
4. Build the standalone APK for `Raw-Game/[Kimochi] NTRdemic - I Will Rescue Her Without Yielding to Any Temptation` (or `[Kimochi] show_off`).
5. Install and verify on MuMu emulator (`127.0.0.1:16384`) using `io.runtimepack.host.WebActivity`.
6. Advance until Title Screen / Prologue is active, capture screenshot (960x540) to `Docs/test_ports/tyrano_kimochi/screenshot.png`.
7. Create `Docs/test_ports/tyrano_kimochi/report.md` following `Docs/test_ports/TEMPLATE.md` and update `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`.
8. Write execution report to `.superpowers/sdd/2026-09-23-multi-engine-porting-and-cheat-registry/task-7-report.md`.

## Critical Constraints
- NEVER touch, read, or write any folder named `antigravity` or `.antigravity`.
- Source directories in `Raw-Game/` are strictly READ-ONLY.
