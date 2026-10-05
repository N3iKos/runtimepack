# Master Integration: Reviewed Conflict Resolution and Evidence Preservation

**Author:** Supervisor main agent.
**Applies to:** 2026-10-05-vnm-github-history-integration.md.
**User approval:** Preserve both Git histories, keep stable source ZIP, commit reviewed feature and update GitHub master normally without force.

## Exact State

- Integration worktree: `D:\02_Development\Projects_2\runtimepack-vnm-reconstruction\.worktrees\vnm-master-integration`.
- Integration branch HEAD/upstream parent: `fd70c960fab1033fd35c29977cd7a209270359d4`.
- MERGE_HEAD/reviewed feature: `07a056b26f6bf2a2689f404632cad62cf176e8ea`.
- Stable baseline branch/tag: `52f9f2f5ef3225980f5a8a90462219341caed3cc`, unchanged.
- Stable source ZIP already verified: `Backup/runtimepack-vnm-v4-source-before-input-merge-20261005-1.zip`, SHA256 `941C0E00D4A9DDD22F437C16D4E64A78A33C9E95AD2A76E8E31A5DD185F45C84`.
- Actual Git unmerged index contains49 paths. Native formatted command summaries incorrectly describe these as staged; use raw porcelain/ls-files output to verify.

## Global Constraints

Only resolve named integration paths. No deletion/cleanup/reset/clean/abort, blanket merge strategy, force-push, amend/hooks bypass, remote visibility change, installs/downloads, MuMu/ADB/UI/gameplay/APK build, game/save/backup APK mutation or worker plans. No added code comments. Preserve all worktrees/branches and the original baseline branch/tag. Git identity `N3ikos <epcg69@gmail.com>` through process environment only.

## Task A: Root Evidence Archive Before Any Checkout

The integration will retain170 remote-only tracked files:169 historical SDD files and1 fixture pycache. Some root ignored SDD files have CRLF bytes different from the corresponding remote blobs. Do not overwrite them during a root checkout.

- Verify parent `D:\02_Development\Projects_2\Backup` exists and select fresh unique `superpowers-root-backup-20261006.zip` (or a collision-safe numbered suffix).
- Archive ALL regular files of root `.superpowers/` byte-exactly without following links, modifying source or deleting anything. Preserve full relative paths. Validate CRC and every entry's size/SHA256 against disk source; record archive size/hash and count. Local report evidence remains private, not published.
- Root tracked source and `.superpowers/` bytes must remain unchanged by archive operation.

## Task B: Resolve Reviewed Paths Only

Reviewer examined each parent difference and approved feature content for48 paths. Capture unmerged status before resolution and verify HEAD/MERGE_HEAD. Explicit per-path restoration from the feature commit is authorized here as conflict resolution in the INTEGRATION worktree only. Never use a global ours/theirs merge strategy, git restore ., reset or overwrite original root.

Accept exact feature blob for these48 paths:

```text
.gitignore
Cargo.lock
Cargo.toml
cli/Cargo.toml
Docs/ENGINE_DETECTION.md
Docs/REFERENCES.md
Docs/superpowers/compat/COMPAT-REGISTRY.md
schemas/asset-audit-report.schema.json
schemas/build-manifest.schema.json
schemas/combined-validation-summary.schema.json
schemas/runtime-manifest.schema.json
schemas/runtime-validation-report.schema.json
schemas/runtimepack-config.schema.json
cli/src/audit_command.rs
cli/src/main.rs
core/src/asset_audit/cache.rs
core/src/build_plan/generator.rs
core/src/build_plan/mod.rs
core/src/domain.rs
core/src/runtime_registry/mod.rs
pack/src/gpack.rs
pack/src/lib.rs
tests/integration/asset_audit_report_test.rs
tests/integration/build_test.rs
android/shell/app/src/main/java/io/runtimepack/host/AssetAuditModels.kt
android/shell/app/src/main/java/io/runtimepack/host/AssetAuditService.kt
android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt
android/shell/app/src/main/java/io/runtimepack/host/BuildManifestLoader.kt
android/shell/app/src/main/java/io/runtimepack/host/GpackStreamReader.kt
android/shell/app/src/main/java/io/runtimepack/host/NativeSaveRepository.kt
android/shell/app/src/main/java/io/runtimepack/host/RuntimeApplication.kt
android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt
android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt
android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt
android/shell/app/src/test/java/io/runtimepack/host/AssetAuditServiceTest.kt
android/shell/app/src/test/java/io/runtimepack/host/AssetProbeRunnerTest.kt
android/shell/app/src/test/java/io/runtimepack/host/GpackStreamReaderTest.kt
android/shell/app/src/test/java/io/runtimepack/host/NativeSaveRepositoryTest.kt
android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt
android/shell/app/src/test/js/diagnostic_suite.test.js
android/shell/app/src/test/js/web_shims.test.js
android/shell/app/src/main/assets/overlay/diagnostic_suite.js
android/shell/app/src/main/assets/overlay/vpad.js
android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js
preview/diagnostic_suite.js
memory/decisions_log.md
memory/progress.md
memory/technical_findings.md
```

For `memory/memory.md`, read both parent blobs, retain the FEATURE file except replace its current status line with this exact truthful synthesis:

```text
7 engine first-class (gate TYRANO-LEGAL dihapus 2026-09-30), save/load Forestia terverifikasi user, asset hardening B+C+D merged, probe CLI T1 tersedia; VNM legacy profile tetap experimental, baseline v4 dipreservasi, guard input beta opsional default OFF tersedia dengan feedback user membaik, sementara root cause rapid-auto white-screen belum terkonfirmasi.
```

Stage these49 resolved paths explicitly plus this supervisor-authored resolution document. Feature-only additions already staged by Git merge remain intact; do not stage unrelated untracked files. Retain ALL remote-only170 files and all identical shared files without edits. No conflict marker/unmerged index entries may remain. Verify staged48 blob IDs exactly equal feature commit; memory file is the sole manually synthesized existing file. Only new plan document may differ in addition.

## Task C: Review and Fresh Test Gate Before Commit

- Compare complete stage0 index tree with feature tree and upstream tree. Expected delta versus feature:170 upstream-only retained files, manually synthesized memory line, and this new plan; no source/runtime/test behavior differences.
- Verify source inputs, stable ZIP/tag/branch, accepted APKs and private root SDD archive unchanged. Scan exact index contents for secrets/private game/save payloads; don't print secret values.
- Independent qa-tester runs offline cargo check/clippy/workspace tests, all13 JS suites with authentic game/JSZip environment, Kotlin compilation/debug+release JVM tests and lint. Capture actual commands/results and XML counts, not prior manual totals. Existing36 vendor lint errors disclosed, not suppressed.
- code-reviewer approves staged resolution and tests. No integration commit or master update until this gate passes. No APK/gameplay tests.

## Task D: Commit, Local Master and Normal Push

After review/QA approval, inspect status, cached diff/stat and log --oneline -10; commit pending merge with `merge: preserve upstream history with reconstructed VNM runtime`. Verify two parents: upstreamfd70... and feature07a...; both must be ancestors. Keep baseline/tag at52f9...

**Root-preservation ruling:** Do not switch root checkout to master or risk overwriting ignored local reports. If local master is absent, create and check it out at the reviewed integration commit IN THE INTEGRATION WORKTREE using `git switch -c master <integrationSHA>`; tree is identical, so no baseline root files change. Branch integration/vnm-master remains at the same commit for evidence. If master already exists, verify safe ancestry and use a separately reviewed fast-forward workflow, never force-update it.

Recheck GitHub master before push. Require it still equals upstream parent or is proven ancestor; new divergent commits stop publication. Push normal explicit refs for master, baseline/vnm-v4, feature/vnm-extreme-input and vnm-v4-baseline (prefer --atomic), no --force or mass-tags. Verify repository PRIVATE, exact URL and final remote SHA/tag/branch pointers via gh/Git. No APK/release publication. Root checkout intentionally remains on preserved baseline/vnm-v4; local/GitHub master holds merged implementation. All worktrees remain on disk.
