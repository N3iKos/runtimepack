# Task 5 Report: Full Automated Regression & APK Rebuild

**Status:** DONE

## Overview
Executed Task 5 of `Docs/execution-plans/track-d-game-runtime-fixes/native-mv-mz-save-plan.md` in the isolated worktree `D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save`. All regression test suites passed with 100% success, both RPG Maker MZ (`Forestia`) and RPG Maker MV (`The Witch's House MV`) were rebuilt into standalone Android APK packages with exit code 0, and raw game directories were verified to be 100% immutable.

---

## 1. Automated Regression Test Outcomes

### A. JavaScript Web Shims Suite
- **Command:** `node android/shell/app/src/test/js/web_shims.test.js`
- **Result:** PASS (100%)
- **Details:** Verified `StorageManager` shim behavior for both RPG Maker MV (synchronous web storage adapter, slot mapping, backup/restore) and RPG Maker MZ (asynchronous localforage bridge, Base64 round-trip, key listing).
- **Log Output:**
  ```text
  Running Mobile Web Shims Unit Tests...
  All Mobile Web Shims tests passed successfully!
  ```

### B. Kotlin Android Host Unit Test Suite
- **Command:** `.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --rerun-tasks`
- **Result:** BUILD SUCCESSFUL (24 actionable tasks executed, 0 failures)
- **Details:** Verified `NativeSaveRepositoryTest` (atomic move semantics, directory isolation between `mz` and `mv`, slot path traversal sanitization, overwrite persistence) and `WebBridge` JavascriptInterface bindings.

### C. Rust Workspace Test Suite
- **Command:** `cargo test --workspace`
- **Result:** PASS (100% across all crates, 0 failed)
- **Details:**
  - `runtimepack` binary & integration tests: 114 passed
  - `runtimepack_core`: 66 passed
  - `runtimepack_engine_renpy`: 37 passed
  - `runtimepack_engine_rgss`: 37 passed
  - `runtimepack_engine_rpgmaker_web`: 34 passed
  - `runtimepack_engine_tyrano`: 35 passed
  - `runtimepack_pack`: 4 passed
  - All Doc-tests: 0 failed

---

## 2. APK Rebuild Outcomes & Artifact Metrics

Both games were built using `runtimepack` CLI targeting the standalone Android project structure with Gradle compilation:

### A. Forestia (RPG Maker MZ)
- **Engine:** RPG Maker MZ (Confidence: EXACT)
- **Build Command:**
  ```powershell
  cargo run --bin runtimepack -- build "Raw-Game\Forestia1.4.4-IND" -o dist/forestia --package com.example.forestia --name "Forestia" --flavor beta --accept-audit-fail --accept-audit-unknown
  ```
- **Gradle Compilation:** Exit Code 0 (Success)
- **Generated File:** `dist/forestia/Forestia.apk`
- **File Size:** `856,239,037` bytes (~816.57 MB)
- **SHA256 Hash:** `A281EC16314450EFAF05A86782C333B206DACAC194A07D56D6D579B9263C2127`

### B. The Witch's House MV (RPG Maker MV)
- **Engine:** RPG Maker MV (Confidence: EXACT)
- **Build Command:**
  ```powershell
  cargo run --bin runtimepack -- build "Raw-Game\The_Witchs_House_MV" -o dist/witchs_house --package com.example.witchshouse --name "The Witch's House MV" --flavor beta --accept-audit-fail --accept-audit-unknown
  ```
- **Gradle Compilation:** Exit Code 0 (Success)
- **Generated Files:**
  - `dist/witchs_house/TheWitchsHouse.apk`
  - `dist/witchs_house/TheWitch'sHouseMV.apk`
  - `dist/witchs_house/TheWitchsHouseMV.apk`
- **File Size:** `177,946,437` bytes (~169.70 MB)
- **SHA256 Hash:** `18F2DF119D28E24560A331B4527733481B93D8938AC0C74004C06F298B2E8471`

---

## 3. Immutability Verification of `Raw-Game/` Source Trees

Before and after the build operations, full recursive file measurements and write-timestamp audits were performed on `D:\02_Development\Projects_2\RPGM-P\Raw-Game`:

| Directory | Pre-Build Count | Post-Build Count | Pre-Build Total Bytes | Post-Build Total Bytes | Delta |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `Forestia1.4.4-IND` | 3,313 | 3,313 | 988,503,179 | 988,503,179 | **0 (Identical)** |
| `The_Witchs_House_MV` | 1,325 | 1,325 | 310,603,144 | 310,603,144 | **0 (Identical)** |

Files modified during run: **0**. Source game assets remain strictly 100% immutable.

---

## 4. Deliverable Status & Next Steps

All Task 5 objectives are fulfilled:
- Regression test suites: 100% PASS
- Target APKs: Generated and verified in `dist/`
- Ready for QA auditing and on-device save/load testing via `qa-tester`.
