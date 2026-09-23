# Task 2: Universal Auto-Recovery Engine (RGSS & Web)

## Goal
Implement a universal Auto-Recovery engine active in all builds that prevents game crashes/freezes when assets (images, audio, or fonts) are missing, misspelled, or corrupt.

## Requirements
1. In `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`:
   - Extend `Bitmap.new`:
     If missing (`Errno::ENOENT` or `RGSSError`), return a safe 32x32 dummy transparent `Bitmap.new(32, 32)` without crashing.
   - Extend `Audio`:
     Hook `bgm_play`, `bgs_play`, `me_play`, `se_play` to catch missing audio errors and fail gracefully without stopping the engine loop.
   - Hook `Cache`:
     Protect `Cache.animation`, `Cache.battleback1`, `Cache.battleback2`, `Cache.battler`, `Cache.character`, `Cache.face`, `Cache.picture`, `Cache.system`, `Cache.tileset`, `Cache.title1`, `Cache.title2`: if missing, return a dummy transparent bitmap.
2. In `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`:
   - When an intercepted image request (`png`, `jpg`, `jpeg`, `webp`) cannot be found in VFS or disk, supply a 1x1 transparent PNG byte stream (`TRANSPARENT_1X1_PNG`) with HTTP 200, preventing WebView `Image.onerror` / `Loading...` hangs.
   - When an intercepted audio request (`ogg`, `m4a`, `mp3`, `wav`) cannot be found, supply a minimal silent audio stream or empty response.
3. Unit Tests:
   - In `android/shell/app/src/test/ruby/input_hook_test.rb`: Add tests verifying `Bitmap.new("non_existent_file.png")` returns a valid dummy bitmap and does not crash.
   - In `android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt`: Add tests verifying 404 image requests return `TRANSPARENT_1X1_PNG`.
4. Run tests:
   `ruby android/shell/app/src/test/ruby/input_hook_test.rb`
   `.\gradlew.bat testDebugUnitTest`
5. Write execution report to `.superpowers/sdd/2026-09-23-beta-diagnostic-and-blacksouls-audit/task-2-report.md`.

## Critical Constraints
- NEVER touch, read, or write any folder named `antigravity` or `.antigravity`.
- Source directories in `Raw-Game/` are strictly READ-ONLY.
