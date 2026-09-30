# Task 11 Fix Report: Probe Process Spawn Grace Period (RT-LAUNCH-NO-PROCESS)

## Task Executed
Fixed race condition in probe scenario boot loop where `adb shell monkey` returns before Android finishes spawning the application process. Instead of immediately reporting false `RT-PROCESS-CRASH` when `pid_alive` is initially false, introduced a process-appear grace period wait loop polling up to `process_appear_timeout_ms` (default 8000ms, 500ms poll interval). If the process never spawns within the grace period, reports `RT-LAUNCH-NO-PROCESS` ("Process did not appear within Ns after launch"). Once seen alive, subsequent missing PID during boot or later steps correctly reports `RT-PROCESS-CRASH`.

## Files Modified
- `cli/src/probe/target.rs`:
  - Added error code constant `pub const RT_LAUNCH_NO_PROCESS: &str = "RT-LAUNCH-NO-PROCESS";`.
- `cli/src/probe/scenario.rs`:
  - Added `pub process_appear_timeout_ms: u64` to `ScenarioConfig` (default: 8000ms, test_fast: 30ms).
  - Added process appear grace period wait loop after `target.launch(&package)`.
  - Updated boot loop failure classification to distinguish `RT-LAUNCH-NO-PROCESS` from `RT-PROCESS-CRASH`.
- `cli/tests/probe_scenario_test.rs`:
  - Added regression test `probe_reports_no_process_when_app_never_spawns` validating honest `RT-LAUNCH-NO-PROCESS` code when PID never appears.
  - Added regression test `probe_waits_for_slow_process_spawn` simulating delayed process spawn (~2s delay) and confirming boot phase succeeds once process appears.

## Results & Data Summary
- TDD RED phase verified: both new tests failed as expected against previous implementation (`RT-PROCESS-CRASH` reported instead of `RT-LAUNCH-NO-PROCESS` or successful boot).
- TDD GREEN phase verified: all 12 probe scenario tests passed after fix.
- Workspace test run: 355 tests passed across all crates, 0 failed.
- Commit: `789aa8e` (`fix(probe): wait for process spawn before treating missing pid as crash (RT-LAUNCH-NO-PROCESS)`).

## Ready for QA
Deliverable is verified and ready for audit/testing by `qa-tester`.
