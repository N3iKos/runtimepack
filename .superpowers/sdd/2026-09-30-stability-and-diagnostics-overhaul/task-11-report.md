# Task 11 QA Probe Execution Report

## Overview
- **Command:** `cargo run -q --bin runtimepack -- probe "D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save\dist\Forestia1.4.4-IND.apk" --device mumu --report "D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save\dist\probe-report.json"`
- **Working Directory:** `D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save`
- **Target Device:** `127.0.0.1:16384` (MuMu Player 12, Android 15)
- **Target Package:** `io.runtimepack.game.forestia144ind`
- **Overall Verdict:** FAIL

## Runs Summary

### Run 1
- **CLI Output:**
  ```text
  RuntimePack Probe — Universal T1 Smoke Test
  Target Device: 127.0.0.1:16384
  APK:           D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save\dist\Forestia1.4.4-IND.apk

  Steps:
    [FAIL] boot (312ms) [RT-PROCESS-CRASH: Process died during boot phase]

  Overall: FAIL
  Report saved to: D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save\dist\probe-report.json
  ```
- **Report JSON Content:**
  ```json
  {
    "device": "127.0.0.1:16384",
    "apk": "D:\\02_Development\\Projects_2\\RPGM-P\\.worktrees\\native-mv-mz-save\\dist\\Forestia1.4.4-IND.apk",
    "package": "io.runtimepack.game.forestia144ind",
    "steps": [
      {
        "step": "boot",
        "status": "FAIL",
        "code": "RT-PROCESS-CRASH",
        "detail_ms": 312,
        "note": "Process died during boot phase"
      }
    ],
    "overall": "FAIL"
  }
  ```

### Run 2 (Retry)
- **CLI Output:**
  ```text
  RuntimePack Probe — Universal T1 Smoke Test
  Target Device: 127.0.0.1:16384
  APK:           D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save\dist\Forestia1.4.4-IND.apk

  Steps:
    [FAIL] boot (299ms) [RT-PROCESS-CRASH: Process died during boot phase]

  Overall: FAIL
  Report saved to: D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save\dist\probe-report.json
  ```
- **Report JSON Content:** Identical failure step `boot`, `code: RT-PROCESS-CRASH`, `detail_ms: 299`.

## Per-Step Verdicts
- `boot`: FAIL (`RT-PROCESS-CRASH: Process died during boot phase`)
- Subsequent steps (`title`, `new_game`, `scene_change`, `tap_center`, `gamepad_a`, `error_log_scan`): Not executed due to immediate boot exit.

## Duration
- Run 1 step duration: 312 ms (command execution ~3.5 s including cargo launch)
- Run 2 step duration: 299 ms (command execution ~3.3 s including cargo launch)

## Anomalies & Diagnostic Findings
- The application process terminates immediately within ~300ms after launch (`pid` check detected dead process before boot/title milestone was reached).
- The probe correctly halted and generated diagnostic JSON `dist\probe-report.json` with code `RT-PROCESS-CRASH`.
