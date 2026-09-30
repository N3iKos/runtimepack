# Task 3 Report: Integrity gate — SHA-256 startup verification + RUNTIME_CORRUPTED

**Plan Reference:** `Docs/superpowers/plans/2026-09-30-asset-hardening-overlay-lockdown.md` (Task 3)  
**Workspace:** `D:\02_Development\Projects_2\RPGM-P\.worktrees\asset-hardening`  
**Branch:** `feature/asset-hardening`  
**Commit Hash:** `54f2c2cba7db0b83d79b7f96f88f3f5e58ae005f`  
**Status:** SUCCESS  

---

## 1. Summary of Deliverables

Implemented the startup integrity verification gate across the build generator and Android host runtime:

1. **Build Generator (`core/src/build_plan/generator.rs`)**:
   - Implemented `compute_runtime_integrity_hash` and `compute_runtime_integrity_hash_from_entries` using standard SHA-256 (`sha2`).
   - Deterministic algorithm sorts canonical lowercase paths (`norm_path = path.replace('\\', "/").trim_start_matches('/').to_lowercase()`), updating SHA-256 with `path (UTF-8)`, byte `0x00`, `file bytes`, and byte `0x00`.
   - Emits generated `RuntimeBuildInfo.kt` into `app/src/main/java/io/runtimepack/host/RuntimeBuildInfo.kt` with computed `RUNTIME_INTEGRITY_SHA256` and `BUILD_FLAVOR`.
   - Emits `runtime_integrity_sha256` into generated `build-manifest.json`.

2. **Kotlin Host (`RuntimeIntegrity.kt` & `RuntimeBuildInfo.kt`)**:
   - Implemented `IntegrityResult` (`Ok`, `Mismatch`, `Unreadable`).
   - Implemented `RuntimeIntegrity.computeHash` with exact byte-for-byte SHA-256 matching the Rust generator.
   - Implemented `RuntimeIntegrity.verify`:
     - Reads runtime assets from `OverlayAssetSource` (GPACK reader or fallback plain assets).
     - In `beta`: logs INFO `[RuntimePack] (Beta) Runtime integrity: <hash>` and returns `Ok`.
     - In `release`: verifies hash matches `RUNTIME_INTEGRITY_SHA256`. If mismatched, returns `IntegrityResult.Mismatch` and logs `[RuntimePack] AND-401`.
     - Returns `IntegrityResult.Unreadable("AND-403")` if GPACK cannot be opened, or `IntegrityResult.Unreadable("AND-402")` if no overlay assets are present in release container.
   - Implemented `RuntimeIntegrity.checkOrShowCorrupted` inflating `activity_corrupted.xml`, displaying error code and details, and wiring the Exit button to cleanly terminate the app (`finishAffinity` + `exitProcess(1)`).

3. **Activity Integration (`WebActivity.kt`, `RgssActivity.kt`, `RenpyActivity.kt`)**:
   - Added startup integrity guard: `if (!RuntimeIntegrity.checkOrShowCorrupted(this)) return` at the entry point of `onCreate` in all three engine activities.

4. **UI Layout (`activity_corrupted.xml`)**:
   - Created dark theme layout (`#121212`) displaying `"APPLICATION CORRUPTED"` in red, `@+id/tv_error_code` with error code, and `@+id/btn_exit`.

---

## 2. Files Touched / Created

| File | Action | Description |
|---|---|---|
| `android/shell/app/src/main/java/io/runtimepack/host/RuntimeBuildInfo.kt` | Created | Default template in shell tree with `@Volatile var RUNTIME_INTEGRITY_SHA256` and `BUILD_FLAVOR`. |
| `android/shell/app/src/main/java/io/runtimepack/host/RuntimeIntegrity.kt` | Created | Startup integrity verification logic, deterministic SHA-256, error handling, and corrupted screen handler. |
| `android/shell/app/src/main/java/io/runtimepack/host/OverlayAssetSource.kt` | Modified | Added `ensureInitialized(context: Context)` to auto-open `game.pack` from app storage or APK assets when needed. |
| `android/shell/app/src/main/res/layout/activity_corrupted.xml` | Created | Error screen layout with `tv_error_code` and `btn_exit`. |
| `android/shell/app/src/test/java/io/runtimepack/host/RuntimeIntegrityTest.kt` | Created | Unit test suite covering deterministic vectors, sorting, path normalization, beta bypass, release match/mismatch, AND-402, and AND-403. |
| `core/src/build_plan/generator.rs` | Modified | Added SHA-256 computation and emission of `RuntimeBuildInfo.kt` and `build-manifest.json`. |
| `core/src/build_plan/mod.rs` | Modified | Re-exported `compute_runtime_integrity_hash` and `compute_runtime_integrity_hash_from_entries`. |
| `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt` | Modified | Added `if (!RuntimeIntegrity.checkOrShowCorrupted(this)) return` in `onCreate`. |
| `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt` | Modified | Added `if (!RuntimeIntegrity.checkOrShowCorrupted(this)) return` in `onCreate`. |
| `android/shell/app/src/main/java/io/runtimepack/host/RenpyActivity.kt` | Modified | Added `if (!RuntimeIntegrity.checkOrShowCorrupted(this)) return` in `onCreate`. |
| `tests/integration/build_test.rs` | Modified | Added tests verifying matching hash test vectors, release emission with 64-char hash in `RuntimeBuildInfo.kt` & `build-manifest.json`, and beta emission. |

---

## 3. Test & Verification Results

1. **Gradle Compilation & Android Host Unit Tests**:
   - Command: `.\gradlew.bat :app:compileDebugKotlin :app:testDebugUnitTest --rerun-tasks --console=plain`
   - Result: **BUILD SUCCESSFUL** (24 actionable tasks executed)
   - `RuntimeIntegrityTest`: 8 test cases passed (single vector, sorted vector, path normalization, content change detection, beta bypass, release match, release mismatch, AND-402 missing files, AND-403 unreadable reader).

2. **Cargo Workspace Tests**:
   - Command: `cargo test --workspace`
   - Result: **ALL PASSED**
     - `build_test.rs`: 67 passed, 0 failed (including `test_compute_runtime_integrity_hash_from_entries_vectors`, `test_emit_project_emits_runtime_build_info_and_manifest_hash`, `test_emit_project_beta_flavor_runtime_build_info`).
     - `theme_package_test.rs`: 15 passed, 0 failed.
     - `asset_audit_report_test.rs`: 23 passed, 0 failed.
     - `asset_audit_cli_test.rs`: 16 passed, 0 failed.
     - `asset_audit_contract_test.rs`: 6 passed, 0 failed.
     - `probe_scenario_test.rs`: 13 passed, 0 failed.
     - Engine tests (`renpy`, `rgss`, `rpgmaker_web`, `tyrano`, `pack`): all passed.

3. **Node Web Shims Tests**:
   - Command: `node app/src/test/js/web_shims.test.js`
   - Result: **ALL PASSED** (Engine input patch, HalfMove compat, load failure diagnostics, global logger, AudioManager, SceneManager, Graphics, readdirSync, TyranoBuilder, late-binding polling coordinator).

---

## 4. Contract Conformance

- Hash algorithm matches byte-for-byte between Rust and Kotlin:
  - Vector 1 (`overlay/test.txt` -> `hello`): `b3d9db26cfb28fa5d189b914eb404c61b995a743b9c2e583d155bc1dfcbfaa88`
  - Vector 2 (`fonts/a.ttf` -> `world`, `overlay/b.js` -> `test`): `f5f5a7000e20110ccac51637f556b04558263117b9eb36b5db6cb50504bac4a0`
- Exit codes / error codes conform to specification: `AND-401` (mismatch), `AND-402` (missing container files), `AND-403` (unopened container).
- Clean exit via `finishAffinity()` and `exitProcess(1)`.
