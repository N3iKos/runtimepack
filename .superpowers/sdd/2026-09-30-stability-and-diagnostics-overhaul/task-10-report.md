# Task 10 Report: Probe CLI T1 — ProbeTarget Trait + AdbTarget Config-Driven + Scenario

## 1. Summary
Implemented the universal smoke test subcommand `runtimepack probe` (T1) in the CLI package. The architecture enforces zero hardcoded devices via a generic `AdbTarget` taking arbitrary ADB host:port/serial addresses, resolved dynamically from CLI flags or `probe-profiles/devices.toml`. Strict TDD discipline was followed with 10 unit and scenario tests covering all verdict codes and execution paths using an in-memory `MockTarget`.

## 2. Module Structure
- `cli/src/probe/mod.rs`: Module declaration and exports.
- `cli/src/probe/target.rs`: Definition of `ProbeTarget` trait, `ProbeError` error enum, and stable diagnostic verdict codes (`RT-BOOT-TIMEOUT`, `RT-NEWGAME-STALL`, `RT-CONSOLE-ERROR`, `RT-PROCESS-CRASH`, `RT-DEVICE-UNAVAILABLE`).
- `cli/src/probe/report.rs`: Serializer for `ProbeReport` and `StepResult` to structured JSON matching project diagnostic specs.
- `cli/src/probe/scenario.rs`: T1 universal smoke test scenario implementation (`run_scenario`), image comparison heuristics (`is_black_screen`, `screen_differs`, `screen_differs_substantially`), and logcat diagnostic error filtering (`scan_logcat_for_errors`).
- `cli/src/probe/adb.rs`: Generic `AdbTarget` implementing `ProbeTarget` via `adb -s <address>`.
- `cli/src/probe/config.rs`: Parser and resolver for `probe-profiles/devices.toml` and CLI device flags.
- `probe-profiles/devices.toml`: Default example configuration file defining `[devices.mumu]` with address `127.0.0.1:16384`. (String "mumu" exists exclusively in this config file, never in Rust source).
- `cli/tests/probe_scenario_test.rs`: Suite of 10 scenario and unit tests verifying full PASS path, all FAIL verdict codes, configuration parsing, heuristics, and serialization.
- `cli/src/main.rs`: CLI wiring for `runtimepack probe` subcommand and `--list-devices` inventory flag.

## 3. New Tests (10 tests)
Location: `cli/tests/probe_scenario_test.rs`
1. `test_scenario_pass_full_path`: Complete PASS execution covering boot transitions, title stabilization, new game screen diff, touch input, gamepad input, clean logcat, and process stability.
2. `test_scenario_fail_boot_timeout`: Verifies `RT-BOOT-TIMEOUT` verdict when screen does not render non-black content within timeout.
3. `test_scenario_fail_newgame_stall`: Verifies `RT-NEWGAME-STALL` verdict when screen remains static on title screen after DPAD + ENTER.
4. `test_scenario_fail_console_error`: Verifies `RT-CONSOLE-ERROR` verdict when fatal error is encountered in logcat (`[GameConsole]` ERROR level).
5. `test_scenario_fail_process_crash`: Verifies `RT-PROCESS-CRASH` verdict when target PID terminates during input steps.
6. `test_scenario_fail_device_unavailable`: Verifies `RT-DEVICE-UNAVAILABLE` verdict when target device cannot be reached.
7. `test_devices_config_and_resolution`: Verifies TOML config parsing, lookup by ID, raw address passthrough, single device fallback, and ambiguity error on multiple unselected devices.
8. `test_report_json_serialization`: Verifies JSON serialization formatting of `ProbeReport` and `StepResult`.
9. `test_adb_target_properties`: Verifies generic `AdbTarget` naming and initialization.
10. `test_screen_diff_algorithms`: Verifies black screen detection and frame diff thresholds.

## 4. Test Results
- **Step 1 (RED Phase):** All 6 initial scenario tests failed as expected (`not yet implemented: run_scenario stub for TDD Step 1`).
- **Step 3 (GREEN Phase):** Workspace-wide test suite passed:
  - `probe_scenario_test`: 10 passed; 0 failed (0.09s)
  - `asset_audit_cli_test`: 16 passed; 0 failed
  - `asset_audit_contract_test`: 6 passed; 0 failed
  - `asset_audit_report_test`: 23 passed; 0 failed
  - `build_test`: 59 passed; 0 failed
  - `integration_compatibility`: 9 passed; 0 failed
  - `integration_detection`: 9 passed; 0 failed
  - `runtimepack_core`: 67 passed; 0 failed
  - `theme_package_test`: 15 passed; 0 failed
  - `runtimepack_engine_renpy`: 7 passed; 0 failed
  - `renpy_asset_audit_test`: 30 passed; 0 failed
  - `runtimepack_engine_rgss`: 8 passed; 0 failed
  - `rgss_asset_audit_test`: 29 passed; 0 failed
  - `web_asset_audit_test`: 34 passed; 0 failed
  - `runtimepack_engine_tyrano`: 3 passed; 0 failed
  - `tyrano_asset_audit_test`: 32 passed; 0 failed
  - `runtimepack_pack`: 4 passed; 0 failed
  - Total: 353 passed; 0 failed across entire workspace.

## 5. Smoke Output (`--list-devices`)
Command: `cargo run -q --bin runtimepack -- probe --list-devices`
```
RuntimePack Probe — Device Inventory

Configured Profiles (probe-profiles/devices.toml):
  - [mumu] -> 127.0.0.1:16384

Connected ADB Devices:
  (No connected ADB devices detected)
```

## 6. Commit Hash
`b7705ce` (`feat(cli): runtime probe subcommand (T1 universal) with config-driven AdbTarget`)
