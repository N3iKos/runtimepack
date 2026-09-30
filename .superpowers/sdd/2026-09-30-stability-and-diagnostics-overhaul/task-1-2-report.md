# Task 1+2 Report: Selective Commit Probe False-Positive Fixes di Master

## 1. Commit Hashes
- **Task 1 (Compiler Path Truncation Fix):** `e8857be` (`fix(asset-audit): preserve js/ prefix for js/plugins paths in logical path sanitization (1158 false positives)`)
- **Task 2 (Android Host GPACK-Aware Probe):** `7fc3711` (`fix(android-host): probe resolves assets inside GPACK instead of disk-only lookup`)

## 2. Diff Verification
### Task 1: `core/src/asset_audit/report.rs`
- Sanitization change verified:
  ```rust
  -    if let Some(pos) = segments.iter().rposition(|segment| {
  +    if let Some(pos) = segments.iter().position(|segment| {
           let lower = segment.to_ascii_lowercase();
           WELL_KNOWN_DIRS.contains(&lower.as_str())
       }) {
  ```
- Regression test added and verified:
  ```rust
  #[test]
  fn test_sanitize_path_str_preserves_js_plugins_prefix() {
      let input = "js/plugins/inazuma/Activity.js";
      let sanitized = sanitize_path_str(input);
      assert_eq!(sanitized, "js/plugins/inazuma/Activity.js");
      ...
  }
  ```

### Task 2: `android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt` & `AssetAuditService.kt`
- `AssetAuditService.kt`: Pass `gpackReader = WebActivity.currentInstance?.gpackReader` to runner.
- `AssetProbeRunner.kt`:
  - Added constructor param `val gpackReader: GpackStreamReader? = null`.
  - Probing resolution:
    ```kotlin
    val reader = gpackReader ?: WebActivity.currentInstance?.gpackReader
    val hasPackFile = reader?.hasFile(relPath) == true
    val file = File(gameDir, relPath)
    val exists = hasPackFile || file.exists()
    ```
  - Open stream & length verification falls back cleanly to GPACK stream when file is packed.
  - Asset graph/report discovery searches inside GPACK reader streams.
  - Fallback scan iterates `reader.entries` when disk scan produces zero media assets.

## 3. Test Results
- **Rust Unit Tests (`cargo test -p runtimepack-core asset_audit -- --nocapture`):**
  - Result: 45 passed, 0 failed, 0 ignored.
  - Included `test_sanitize_path_str_preserves_js_plugins_prefix` (PASS).
- **Kotlin Unit Tests (`gradlew.bat :app:compileDebugKotlin :app:testDebugUnitTest --tests "io.runtimepack.host.AssetProbeRunnerTest" --rerun-tasks`):**
  - Result: BUILD SUCCESSFUL, 11 tests passed, 0 failed.
  - Included new test `testGpackAwareProbingValidatesPackedAssets` (PASS).

## 4. Extra Files Decision
- **`AssetProbeRunnerTest.kt`:** **IKUT di-commit di Task 2.**
  - *Alasan:* Berisi unit test `testGpackAwareProbingValidatesPackedAssets` yang menguji langsung integrasi `AssetProbeRunner` dengan `GpackStreamReader`.
- **`VfsResourceInterceptor.kt` & `VfsResourceInterceptorTest.kt`:** **TIDAK IKUT di Task 2.**
  - *Alasan:* Sesuai dokumen rencana `Docs/execution-plans/track-d-game-runtime-fixes/track-3-renpy-overlay-and-rpgm-universal-vfs-plan.md`, perubahan ini merupakan bagian dari **Track 3 Task 5: Implement Pilar 3 — O(1) Case-Insensitive Hash Map Resolver** (`perf(vfs): replace O(N) linear scans with O(1) hash map resolver and pre-indexed plugin table`), bukan probe false-positive fix.

## 5. Uncommitted Files Intentionally Left Untouched
Tracked files:
- `AGENTS.md`
- `Docs/execution-plans/track-d-game-runtime-fixes/design.md`
- `Docs/execution-plans/track-d-game-runtime-fixes/master-roadmap.md`
- `android/shell/app/src/main/assets/overlay/diagnostic_suite.js`
- `android/shell/app/src/main/assets/overlay/vpad.css`
- `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
- `android/shell/app/src/main/java/io/runtimepack/host/RenpyActivity.kt`
- `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`
- `android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt`
- `android/shell/app/src/test/java/io/runtimepack/host/RenpyActivityTest.kt`
- `android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt`
- `android/shell/app/src/test/js/web_shims.test.js`
- `memory/decisions_log.md`
- `memory/progress.md`
- `memory/technical_findings.md`

Untracked files:
- `.superpowers/sdd/native-mv-mz-save/`
- `.superpowers/sdd/track-3-renpy-overlay-and-rpgm-universal-vfs/`
- `Docs/AGENTS-old.md`
- `Docs/FORESTIA_SAVE_LOAD_HANDOVER.md`
- `Docs/execution-plans/track-d-game-runtime-fixes/native-mv-mz-save-design.md`
- `Docs/execution-plans/track-d-game-runtime-fixes/native-mv-mz-save-plan.md`
- `Docs/execution-plans/track-d-game-runtime-fixes/track-3-renpy-overlay-and-rpgm-universal-vfs-plan.md`
- `Docs/superpowers/plans/2026-09-28-build-time-theme-package-engine.md`
- `Docs/superpowers/plans/2026-09-28-liquid-glass-touch-ripple-layout-refinement.md`
- `Docs/superpowers/plans/2026-09-28-touch-tail-mobile-fit-overlay-hardening.md`
- `Docs/superpowers/specs/2026-09-28-build-time-theme-package-engine-design.md`
- `Docs/test_ports/debug_mizuki_blackscreen.png`
- `Raw-Game/`
- `asset-audit-report.html`
- `asset-audit-report.json`
- `asset-dependency-graph.json`
- `graphify-out/`
