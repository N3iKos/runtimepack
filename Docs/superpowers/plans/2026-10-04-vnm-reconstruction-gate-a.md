# VNM Reconstruction Gate A Implementation Plan

**Author:** Supervisor main agent.
**Goal:** Create a verified isolated source snapshot and collect baseline non-device test evidence without altering the source export or recovery.
**Architecture:** Filesystem isolation because baseline has no Git metadata. No source implementation in this gate.
**Spec:** `Docs/superpowers/plans/2026-10-04-vnm-reconstruction-roadmap.md`, Gate A.

## Global Constraints

- Source: `D:\02_Development\Projects_2\runtimepack-master`.
- Destination: `D:\02_Development\Projects_2\runtimepack-vnm-reconstruction`.
- Parent `D:\02_Development\Projects_2` must exist; destination must not exist before creation. Collision means stop, no overwrite/delete.
- No Git initialization, destructive commands, installs/downloads, game execution, MuMu/ADB/UI, commit or build APK.
- Original source, fresh game and recovery are read-only.
- Supervisor owns planning; executor must not author/revise plans.
- Output records are permitted only in new workspace `.superpowers/sdd/2026-10-04-vnm-reconstruction/`.

## Task 1: Exact filesystem copy — executor

1. Verify named parent/source/destination paths and absence of active writes affecting included source.
2. Enumerate baseline regular files without traversing links/reparse points. Report any links explicitly rather than silently dropping necessary runtime code.
3. Include all regular source/config/runtime/documentation files and root hidden configs. Exclude only root `target`, proven root `dist*` generated output directories, and exact generated Android shell `app/build` / `.gradle` directories. Do not exclude source directories by generic substring `build` or `cache`. Copy `pack`, `runtimes`, `.cargo`, `profiles`, `probe-profiles`, `compatibility-db`, native libraries, tests, docs and memory.
4. Preserve user evidence/documents; do not execute their contents. Avoid following stale `.git` pointers if discovered; baseline expected no.git.
5. Copy bytes into fresh destination with no recursive destructive helper. Temporary helper script, if required, must use dedicated write tool under approved temp path and contain no deletion capability.
6. Generate manifest of relative path, size and SHA-256. Verify every copied regular file against baseline hash; differing source during copy is a blocker, not automatic retry-overwrite.
7. Write snapshot-manifest.json and baseline-copy-report.md in the permitted ledger directory. Record exclusions, errors, links, bytes and counts. Record absence of Git; do not call this Git-clean.

## Task 2: Baseline non-device checks — executor

1. Read mandatory docs/nested instructions in copied workspace. Check actual Cargo manifests, test scripts and Gradle task/toolchain configuration.
2. Resolve existing pinned JDK/SDK locally with process-only environment settings. Do not use global config changes. If unavailable report TOOLCHAIN_MISSING.
3. Run from copied workspace using tool `workdir`, not shell directory changes:
   - `cargo check --workspace`
   - `cargo clippy --workspace --all-targets`
   - `cargo test --workspace`
   - Existing JS standalone suites `web_shims.test.js`, `vpad_bridge.test.js`, `overlay_interaction_regression.test.js`, `diagnostic_suite.test.js`, `cheat_primitives.test.js`, `cheat_menu.test.js` with node and exact actual paths.
   - `node --check android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
4. Android from destination `android/shell`, use `.\gradlew.bat --offline :app:compileDebugKotlin :app:testDebugUnitTest :app:lintDebug` only if required pinned toolchain/distribution available. Existing caches allowed; no implicit downloading. Separate task runs if lint failure would obscure unit result.
5. No CLI game inspect until experimental detector exists; no boot/game libraries executed here.
6. Record commands, exit status, raw result summaries, measured test totals and baseline failures. Do not suppress or fix failures in this task.
7. Recheck original included files remain hash-identical to manifest; new source changes in destination are not permitted in this gate except generated test outputs and ledger records.

## Review Gate — code-reviewer

Verify copy manifest/hash coverage, essential runtime/source roots, non-Git handling, safe exclusions, tests actually executed and failures accurately disclosed. No fixes or code execution recovered from evidence. Supervisor then authors Gate B detailed implementation plan from actual isolated source interfaces.

## Acceptance

Verified isolated snapshot with no original content alteration, clear prerequisite/baseline test evidence and explicit blockers. No reconstructed source changes or APK generated.
