# Task 12 Report: Optional `--probe` Post-Build T1 Verification

## Summary
- **Commit:** `858e9ce13ab3bc39213f32c997d938347fbc0f9d`
- **Branch:** `feature/native-mv-mz-save`
- **Status:** Complete

---

## 1. Flag UX & Ergonomics
The `build` subcommand now supports two optional flags:
- `--probe`: boolean flag (default: `false`). When omitted, build behavior is 100% unchanged. When passed, triggers post-build T1 smoke probe on the emitted APK.
- `--probe-device <device>`: optional string specifying an ADB target address (e.g. `127.0.0.1:16384`) or a configured profile ID from `probe-profiles/devices.toml` (e.g. `mumu`).

### Behavior:
1. **Device Resolution Precedence:**
   - Explicit `--probe-device <id-or-address>` (resolves from profile if defined, or raw address).
   - Config file `probe-profiles/devices.toml`: if 1 profile defined, use it. If >1 defined, yields ambiguous skip.
   - Dynamic ADB scan (`adb devices`): if 1 ready device, use it; if 0 devices or ADB unavailable, yields graceful skip; if >1 devices, yields ambiguous skip.
2. **Graceful Degradation:**
   - If no device is available: prints `[probe] no device available — skipping runtime verification` and exits 0 (SUCCESS).
   - If multiple devices are available without `--probe-device`: prints `[probe] multiple devices available — skipping runtime verification (specify target using --device or --probe-device <address>)` and exits 0 (SUCCESS).
   - If no APK was produced (e.g. `--no-compile`): prints `[probe] skipping runtime verification — no APK was produced by build` and exits 0 (SUCCESS).

---

## 2. Exit Code Decision
- **Probe Failure:** Exits with code `1` (non-zero).
  - *Rationale:* When `--probe` is explicitly provided, verification serves as a gate for CI/CD and automated packaging pipelines. If runtime verification fails (e.g. boot crash, console error, newgame stall), the package cannot be trusted for deployment despite successful packaging.
  - *Failure UX:* Prints `[probe] FAILED — see <path>/probe-report.json` with itemized step failures and codes.
- **Graceful Skip (No Device / Multiple Devices / No APK):** Exits with code `0`.
  - *Rationale:* Packaging itself succeeded; absence of a connected test device in developer environments should not break standard build workflows.

---

## 3. Refactor Shape
- **Extracted `probe::run_probe` and `probe::run_probe_with_config`:**
  - Placed in `cli/src/probe/mod.rs` as reusable entrypoints.
  - Handles ADB target instantiation, scenario execution, step outcome formatting, and report JSON persistence.
  - Called uniformly by both `runtimepack probe` subcommand and `runtimepack build --probe` flag.
- **Pure `probe::resolve_device_target`:**
  - Parameterized with a scanning closure `F: FnOnce() -> Result<Vec<(String, String)>, ProbeError>`.
  - Enables 100% unit-testable device resolution precedence and skip logic without spawning native `adb` subprocesses.
  - Existing `probe::resolve_device` re-uses `resolve_device_target` with `AdbTarget::scan_connected_devices`.
- **Exact APK Path Reuse:**
  - Build pipeline captures the emitted destination APK (`output.join(&sanitized_apk_name)`) and passes it directly to `run_probe`, avoiding path re-derivation or heuristics.

---

## 4. Tests Added & Verification
1. **Unit Tests (`cli/tests/probe_scenario_test.rs`):**
   - `test_resolve_device_target_precedence_and_pure_scan`: Verifies CLI override > profile matching > single profile fallback > zero devices (`NoneAvailable`) > multiple devices (`Ambiguous`) > scan IO error fallback (`NoneAvailable`).
2. **Integration Tests (`tests/integration/build_test.rs`):**
   - `test_cli_build_with_probe_flag_graceful_skip`: Verifies `build --probe --no-compile` executes cleanly and emits skip notice without failure.
   - `test_cli_build_help_mentions_probe_flags`: Verifies `--probe` and `--probe-device` appear in `build --help`.
3. **Documentation:**
   - `AGENTS.md` section 16 updated with pointer to `runtimepack probe` (T1).
4. **Workspace Test Run:**
   - `cargo test --workspace`: all 13 test suites passed (231+ unit and integration tests passing).
   - `cargo build`: compiled cleanly with zero errors.
