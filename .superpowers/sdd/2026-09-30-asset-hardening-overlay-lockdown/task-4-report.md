# Task 4 Report: R8/DEX hardening + JS-bridge keep rules

## Overview
Activated R8 minification / DEX hardening for Android release builds (`isMinifyEnabled = true`), provided `android/shell/app/proguard-rules.pro` protecting `@JavascriptInterface` bridge methods, Activities, JNI native methods, `RuntimeIntegrity`, `OverlayAssetSource`, `RuntimeBuildInfo`, and serialization models. Updated build generator to emit the rules file and configure `isMinifyEnabled = true` in generated Android projects.

## Files Touched / Created
- Created: `android/shell/app/proguard-rules.pro`
- Modified: `android/shell/app/build.gradle.kts`
- Modified: `core/src/build_plan/generator.rs`
- Modified: `tests/integration/build_test.rs`

## Test Results

### 1. TDD Integration Test
- `test_release_project_emits_proguard_rules_and_enables_minify` in `tests/integration/build_test.rs`:
  - Initial run: FAILED (asserted `isMinifyEnabled = true` when generator produced `false`).
  - After implementation: PASSED.

### 2. Gradle Compilation & R8 Verification
Executed in `android/shell`:
- `.\gradlew.bat :app:compileReleaseKotlin :app:testDebugUnitTest --console=plain`: BUILD SUCCESSFUL (38 actionable tasks: 2 executed, 36 up-to-date)
- `.\gradlew.bat :app:minifyReleaseWithR8 --console=plain`: BUILD SUCCESSFUL (26 actionable tasks: 8 executed, 18 up-to-date)

### 3. Full Workspace Test Suites
- `cargo test --workspace`:
  - `runtimepack` binary & integration tests (68 tests in `build_test`, 16 in `asset_audit_cli_test`, 6 in `asset_audit_contract_test`, 23 in `asset_audit_report_test`, 9 in `compatibility_test`, 9 in `detection_test`, 13 in `probe_scenario_test`): ALL PASSED.
  - `runtimepack_core`: 67 tests PASSED.
  - `runtimepack_engine_renpy`: 7 unit + 30 integration tests PASSED.
  - `runtimepack_engine_rgss`: 8 unit + 29 integration tests PASSED.
  - `runtimepack_engine_rpgmaker_web`: 34 integration tests PASSED.
  - `runtimepack_engine_tyrano`: 3 unit + 32 integration tests PASSED.
  - `runtimepack_pack`: 4 tests PASSED.
  - `theme_package_test`: 15 tests PASSED.
- `node android/shell/app/src/test/js/web_shims.test.js`: ALL PASSED (10/10 test suites passed).

## Commit Details
- Commit Hash: `ff9875f4a466b9a40ac10fe9f300832f758f2a01`
- Message: `build(android): enable R8 minify for release with comprehensive JS-bridge keep rules`
