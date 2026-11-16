# Informational build audit default (approved user design)

Goal: asset audit always runs/reports missing assets but doesn't block APK packaging by default. `--strict-audit` restores existing FAIL/INDETERMINATE gates, accept flags retain outcome-specific overrides. Security/native/runtime/detection/signing gates unchanged. No game source edits/dummy assets. Build King Exit release/packed after verified implementation. Main implements, explorer maps contracts, reviewer and QA independently validate. Supersedes previous default-reject audit policy in canonical docs; user explicitly approved policy change.

- [x] Map existing policy/CLI/schema/callers.
- [x] Observe default FAIL regression test red with BLD-104.
- [x] Implement explicit CLI policy + manifest policy; retain strict legacy API/backward deserialization.
- [x] Verify 99 build tests; permanent audit-only safety gate includes CMP-AUD-009 (extra red/green regression); final reviewer approved, remediation no longer suggests override for hard blockers.
- [x] Align source Wizard/help, seven Docs/knowledge files and optional manifest audit.policy schema (legacy absent defaults strict). Portable Windows bundle not refreshed.
- [x] Independent QA workspace: 643 passed / 1 failed / 2 ignored across 39 summaries, stale test_windows_batch_wizard_contract remains unrelated. Final targeted build_test rerun: 99 passed after safety correction.
- [x] Recompile release CLI; King Exit built without accept flags into dist_king_exit_release_informational_20261018_170527. Final host CLI rebuilt after CMP-AUD-009 review correction; generated Android runtime unchanged by that host-only correction.
- [x] APK: [RPGM]KingExit.apk, 1,405,470,863 bytes; SHA-256 938b8ff9e6a8306ad233a03095155af2e6f221732aa30dab8b3660473d25fb5d matches manifest. apksigner exit 0/v2 true/Android Debug signer. audit.policy informational, outcome FAIL, overrides empty; missing character/case warning remain. Cleanup cleaned/no scratch. Device startup/gameplay NOT TESTED.
