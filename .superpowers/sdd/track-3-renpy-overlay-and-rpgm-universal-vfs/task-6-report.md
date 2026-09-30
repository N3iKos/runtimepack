# Task 6 Report: Implement Pilar 4 — VFS-Aware Diagnostics & Path Sanitizer Fix

**Status:** DONE

## Overview
Implemented Pilar 4 of the Universal VFS architecture and diagnostics hardening:
1. **Rust Path Sanitizer Fix (`core/src/asset_audit/report.rs`):**
   - Fixed `sanitize_path_str` in `core/src/asset_audit/report.rs` by switching from `rposition` to `position` when searching for well-known directory prefixes.
   - Prevents chopping outer directory segments like `js/` or `www/` when nested well-known directories (such as `plugins/` or `audio/`) appear deeper in the path (e.g. `js/plugins/inazuma/Activity.js` now cleanly sanitizes to `js/plugins/inazuma/Activity.js`).
   - Added unit test `test_sanitize_path_str_preserves_js_plugins_prefix` verifying preservation of `js/plugins/...` and `www/js/plugins/...` paths.
2. **VFS-Aware Asset Probing (`AssetProbeRunner.kt` & `AssetAuditService.kt`):**
   - Added `gpackReader: GpackStreamReader? = null` parameter to `AssetProbeRunner` with fallback to `WebActivity.currentInstance?.gpackReader`.
   - Updated Stage 1 (`FILE_EXISTS`), Stage 2 (`FILE_READABLE`), and Stage 3 (`INTEGRITY_VERIFIED`) to probe against `gpackReader` if present, falling back to disk `File`.
   - Updated `discoverAssets` to discover assets from `gpackReader` JSON metadata (`asset-dependency-graph.json` / `asset-audit-report.json`) or directly from `reader.entries` when disk scan returns empty.
   - Wired `gpackReader = WebActivity.currentInstance?.gpackReader` in `AssetAuditService.kt` in `startAudit` and `runProbe`.
   - Added unit test `testGpackAwareProbingValidatesPackedAssets` in `AssetProbeRunnerTest.kt` verifying end-to-end probing of GPACK-contained assets with zero disk footprint.

## Files Touched
1. `core/src/asset_audit/report.rs`
   - Replaced `segments.iter().rposition` with `segments.iter().position`.
   - Added unit test `test_sanitize_path_str_preserves_js_plugins_prefix`.
2. `android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt`
   - Added `gpackReader: GpackStreamReader? = null` to constructor.
   - Probing loop checks `reader.hasFile(relPath)` and streams via `reader.openStream(relPath)`.
   - Integrity check tests `reader.getEntry(relPath)?.size`.
   - `discoverAssets` supports streaming graph JSON or indexing `reader.entries`.
3. `android/shell/app/src/main/java/io/runtimepack/host/AssetAuditService.kt`
   - Passed `WebActivity.currentInstance?.gpackReader` to `AssetProbeRunner` in `startAudit` and `runProbe`.
4. `android/shell/app/src/test/java/io/runtimepack/host/AssetProbeRunnerTest.kt`
   - Added `@Test fun testGpackAwareProbingValidatesPackedAssets()` and `createTestGpack(...)`.

## Test Evidence
### 1. Failing Rust Unit Test Verification (Step 2)
Command:
```powershell
cargo test -p runtimepack-core test_sanitize_path_str_preserves_js_plugins_prefix
```
Output:
```
test asset_audit::report::tests::test_sanitize_path_str_preserves_js_plugins_prefix ... FAILED
thread 'asset_audit::report::tests::test_sanitize_path_str_preserves_js_plugins_prefix' panicked at core\src\asset_audit\report.rs:374:9:
assertion `left == right` failed
  left: "plugins/inazuma/Activity.js"
 right: "js/plugins/inazuma/Activity.js"
```

### 2. Passing Rust Unit Test Verification (Step 3)
Command:
```powershell
cargo test -p runtimepack-core test_sanitize_path_str_preserves_js_plugins_prefix
```
Output:
```
running 1 test
test asset_audit::report::tests::test_sanitize_path_str_preserves_js_plugins_prefix ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 66 filtered out
```

### 3. Full Rust Workspace Test Suite Pass
Command:
```powershell
cargo test --workspace
```
Output:
```
test result: ok. 67 passed in runtimepack_core
test result: ok. 59 passed in build_test
test result: ok. 34 passed in web_asset_audit_test
test result: ok. 32 passed in tyrano_asset_audit_test
test result: ok. 30 passed in renpy_asset_audit_test
test result: ok. 29 passed in rgss_asset_audit_test
test result: ok. 23 passed in asset_audit_report_test
test result: ok. 16 passed in asset_audit_cli_test
test result: ok. 15 passed in theme_package_test
test result: ok. 9 passed in detection_test
test result: ok. 9 passed in compatibility_test
test result: ok. 6 passed in asset_audit_contract_test
test result: ok. 4 passed in runtimepack_pack
All tests passed!
```

### 4. Android Unit Test Suite Pass (Step 5)
Command:
```powershell
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest
```
Output:
```
BUILD SUCCESSFUL in 14s
24 actionable tasks: 6 executed, 18 up-to-date
```
All Android unit tests including `AssetProbeRunnerTest` pass 100%.
