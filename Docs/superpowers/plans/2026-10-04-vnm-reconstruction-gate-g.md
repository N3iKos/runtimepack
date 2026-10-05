# VNM Reconstruction Gate G: Final Review, Non-Device Verification and Beta APK

**Author:** Supervisor main agent.
**Goal:** Package reconstructed full-overlay VNM runtime for user gameplay QA after verified contracts, without claiming old build byte equivalence or new playthrough success.
**Spec:** reconstruction-roadmap Gate G. Gates B1/B2/CDE/F reviewed.

## Constraints

Only runtimepack-vnm-reconstruction modifications. Original export, recovery, fresh game immutable. No delete/cleanup/Git/init/commit/install/download/device/MuMu/ADB/UI. Supervisor plans only. No new code comments. Full beta overlay/watermark/diagnostics required. No native dependency or audit bypass blanket. Encrypted path still whole-chunk authenticated.

## Task 1: Remove only reconstruction-pending blocker and test policy

Files: engines/vnmaker/src/lib.rs analyzer and tests/integration/vnmaker_detection_test.rs/asset audit tests as necessary. Replace always-present pending RT-101 Error with explicit experimental compatibility diagnostic and status COMPATIBLE_WITH_SHIMS only for the verified legacy export profile requiring implemented contracts. Unknown native dependencies must retain Error/hard block. Detection unresolved profile/runtime missing retain original hard blockers. Do not globally remove RT-101 from CLI hard gate.

Tests before change: verified profile previously pendingblocked now supports experimental adapter; unsupported .node/unknownDLL remains blocked, auditINDETERMINATE still requires accept-audit-unknown; accept-audit-fail does not bypass unknown. Proven capability implementation must be referenced in registry/evidence. Status reflects implemented shim requirements, not gameplay acceptance.

Run targeted tests, cargo check/clippy/workspace offline, JS suites with VNM_GAME_ROOT or VNM_JSZIP_PATH pointing to freshgame so genuine library test runs (no skip counted pass), Kotlincompile/JVM/lint offline. Preserve actual rawlogs/XML and report counts without arithmetic guesses.

## Task 2: Whole-reconstruction final code review

Code-reviewer examines aggregate changes against original filesystem manifest including domain/schemas/detectoraudit/nativeGS/binaryBuffer/window/WebAudio/WebIDL/voice/startup/overlay/streaming. Check actual profile and shim URL runtime delivery, source scoped compatibility, no no-op save, standard pointerlifecycles and sharedstream handling. No device tests. Fix blocking review findings through executor then scoped re-review before build. Worker cannot authorize own build.

## Task 3: Build after supervisor review gate

Executor first verifies CLI flags via actual --help, toolchain/parent/freshoutput. Proposed invocation (use verified actualoptions):
`cargo run --offline --bin runtimepack -- build "D:\Old-Recvery\Project_2\RPGM-P\Raw-Games\Time for Punishment Class ~Taking Lessons for My Boyfriend~" --flavor beta --accept-audit-unknown --output dist_vnm_reconstructed`

Do not use --overlay-mode unless actually implemented; current scope full default overlay only. Pinned process JAVA_HOME/SDK settings, no global mutation. Use offline Gradle where supported existingCLI; if build driver unconditionally network and cache missing report blocker no install. Output must not overwrite an earlier artifact.

Build metadata indicates experimental reconstructed legacy VNM adapter, unknown manifests override recorded. If runtime integrity hashes generated require exact packaged files provenance, no disabling protection workaround. No source changes to generated project that evade reviewed template unless explicitly reviewed fix.

## Task 4: Independent artifact handoff

qa-tester read-only no device: verify archive integrity, APK size/SHA256, apksigner signer, aapt package/ABI/minSDK, actual embedded shim matches reviewed source, generated streaming Kotlinmatches reviewed hash and relevant DEX class exists, fulloverlay watermark/diagnostics present, gamepack readable index with flags rawexpected. Source gamecritical hashes unchanged; original snapshot intact. No claim matching former APK signature without actual surviving artifact. Never uninstall/clear save for update mismatch.

Reports and progress in `.superpowers/sdd/2026-10-04-vnm-reconstruction/`, memory updated only isolatedworkspace per instructions. Preserve APK, all source/evidence and outputs; no cleanup.

## Acceptance and User Handoff

Contract tests+review+build+artifact verification complete. Deliver exact APK path/hash and checklist dialog/voice/choice/save-restart/overlaycycles/background/video-longplaythrough. Current gameplay status NOT_TESTED until user reports results. Historical oldbuild endingpass is target, not currentacceptance. Record remaining lint failures and encrypted memorylimits clearly.
