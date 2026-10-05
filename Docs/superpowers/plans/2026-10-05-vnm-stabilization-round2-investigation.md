# VNM Stabilization Round 2: Evidence Investigation Plan

**Author:** Supervisor main agent.
**Goal:** Identify actual invalid audio reference states and explain runtime audit/export discrepancies before patching.
**Scope:** Read-only diagnosis in runtimepack-vnm-reconstruction and freshly downloaded game; no builds or implementation in this phase.

## Constraints

- Work root `D:\02_Development\Projects_2\runtimepack-vnm-reconstruction`.
- Game root `D:\Old-Recvery\Project_2\RPGM-P\Raw-Games\Time for Punishment Class ~Taking Lessons for My Boyfriend~`, immutable.
- Preserve original export, recovery, saves, APKs and user Audit-app.txt.
- No deletion, cleanup, Git, installs, downloads, MuMu, ADB, UI, full game execution or APK build.
- Supervisor writes every plan. Workers return evidence only; no planning documents written by workers.
- User owns gameplay QA. v2 NOT_STABLE based on user reports before chapter2; save currently reported normal, retain regressions.

## Task 1: Audio and focus investigation — executor

- [ ] Read actual fresh ENGINE snippets around 1-based columns49411/147228/81294/52688 and map class/method/property names. Record offset convention precisely.
- [ ] Trace constructor, initialize, play, pause, resume, stop, dispose and native onended callbacks for affected reference/buffer. Identify which field is null before getter error access and which disposal path nulls it.
- [ ] Trace SCRIPTS AudioManager.stopAllSounds/SceneManager.update and focus blur handler. Do not claim same cause for popup and dialogue without source proof.
- [ ] Compare current adapter wrappers and whether existing tests exercise authentic reference .pause/.stop/getters versus synthetic AudioBuffer mocks.
- [ ] Trace production WebActivity modal/editor/hide paths and vpad bridge; identify genuine blur/focus versus synthetic events, release state and foreground. No UI manipulation.
- [ ] Return minimal failing-state reproduction strategy and exact affected methods/source evidence, not a code fix.

## Task 2: Runtime probe and export discrepancy — executor

- [ ] Read `dist_vnm_reconstructed_v2/Audit-app.txt`, HTML/static audit and embedded reports. Preserve unchanged.
- [ ] Identify source of exported engine_family=rpgmaker_web; actual manifest engine/runtime mapping in native and JS bridges.
- [ ] Trace UI Missing7 versus runtime failed897, inventory1976 versus pack1969. Read-only pack index/reference list to enumerate exact excluded7, not infer from counts alone.
- [ ] Check candidate890 decode failures by representative payload signatures and existing VNM loading/encryption flags. No decryption bypass or direct game execution.
- [ ] Verify probe evidence stages: FileReadable/IntegrityVerified/DecodeSuccess and classifications; never label bytes decoded based on extension alone. Probe stresses themselves must not unbounded-buffer huge videos.
- [ ] Trace static report visibility NOT_RUN despite build report, event retention console250 versus export log subset; identify missing raw failure/error export source.
- [ ] Return fields needed for comprehensive export: structured failures, stage/path/reason/event sequence, buildidentity, engineidentity, static/runtime scope, truncation count, JS stack, device versions. No secrets/saves exported.

## Review and Implementation Gate

Code-reviewer checks evidence and distinct root causes. Supervisor authors bounded lifecycle/overlay patch plan first, separate diagnostics/export plan second. Each implementation uses RED/GREEN authentic production path regression and review before build. Source/core saves/streaming remain protected. No vague blanket catch/no-op pause/stop to mask bugs.
