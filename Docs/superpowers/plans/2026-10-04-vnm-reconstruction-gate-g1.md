# VNM Reconstruction Gate G1: Final Asset Integrity Digest

**Author:** Supervisor main agent.
**Goal:** Fix verified template-vs-staged-asset digest mismatch without disabling integrity verification.
**Evidence:** artifact-verification.md records template fe38ef57... versus packaged5812be9a...; watermark transformation occurs after current template hash. Beta tolerates warning but candidate must have consistent provenance.

## Constraints
Only isolated reconstruction source. No original/game/recovery edits, deletions, Git, devices/ADB/MuMu, installs, cleanup. No comments. Worker no plans. No disabling integrity check or beta warning bypass. Do not rebuild APK until review.

## Task 1: Regression and minimal generator ordering

Read full generator staging/overlay/fonts/theme/protection hash sequence and RuntimeIntegrity algorithm. Add regression in existing build integration tests generating project without compilation: recompute exact normalized sorted runtime asset path-null-bytes-null digest from FINAL staged assets and compare generated RuntimeBuildInfo constant. Assert watermark transformations actually applied, not hashing untouched fixtures. RED prior mismatch, GREEN after moving compute/write of constant to post-final transformation stage. Account all theme/overlay changes and runtime asset selection according actual Kotlin scopes, release packed runtime assets maintain authenticated packaging provenance. No release APK required; synthetic release-generation regression validates no omitted staged runtime assets. Preserve source template immutability.

Modify core/src/build_plan/generator.rs and tests/integration/build_test.rs only as necessary. Avoid broadrefactor; finaldigest changes only after all byte mutations. Do not claim allbytes runtimeifactualscope excludes theme files document exact scope parity. RuntimeBuildInfo path generated after digest but beforecompile.

## Task 2: Verify/review

Offline targeted build tests then cargo check/clippy/workspacetests. Record actual counts and redgreen logs in gate-g1-report.md under existing SDD. Reviewer checks actual packagedasset algorithm parity, release integration, no protectionbypass. No rebuild until supervisor approval; new output dist_vnm_reconstructed_v2 preserves initial blockedartifact.
