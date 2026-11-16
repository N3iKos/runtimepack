# Portable release finalization implementation plan

> Execute inline, no subagents. User approved local destination, Windows CLI/wizard, Linux fixes/checks/CI, and GitHub branch release/20261116. No force push or changes to old runtimepack-master.

**Goal:** Preserve reviewed source, create a standalone local Windows bundle, and push a clean source branch with native Linux CI.
**Architecture:** Existing Rust workspace + Python wizard. New directory D:/02_Development/Projects_1/Runtimepack/RuntimePack-20261116 with source/, windows/, release-record/. Public source separated from local-only runtimes and history artifacts.
**Spec:** Approved in-chat design and user-reported manual tests PASS; exact game/device scenarios not supplied. Do not invent coverage or claim bug-free.

## Constraints
- Windows-native work only, no WSL, no subagents. No installs/global configuration edits, source/game deletion, old destination overwrite, game/device execution, or APK rebuild.
- Existing source modifications are intended current version; preserve them. Existing Git history includes local diagnostic-only artifacts: publish only reviewed source tree if history cannot be certified clean.
- Never publish games, saves, RTP, credentials, temporary artifacts, or RGSS SOURCE_UNVERIFIED native binaries (Docs/LICENSE_COMPLIANCE.md).
- Existing Windows CLI binary verified SHA d08cdd07f2205b709aa5b33cdb4a44b9b8474d53ebd784e32d0975b210e96b09. Native-only CLI unchanged unless necessary; wizard/platform changes separately tested.
- Linux execution deferred to GitHub native Ubuntu runners; do not claim Linux verified until run succeeds. Windows Android suite has six known symlink privilege failures; retain honest results.
- Source remote https://github.com/N3iKos/runtimepack, target branch release/20261116; no force push, no overwrite existing remote branch. Push permission given.

## Tasks
- [ ] Inspect Linux path/launcher and publication/licensing constraints; implement minimal fixes with failing behavioral tests first.
- [ ] Add native Linux CI for Rust/Python/JS checks using existing dependencies; run local applicable suites and document blocked/skipped tests.
- [ ] Snapshot source into new destination, separate public tracked inputs from ignored local RTP/RGSS libraries; verify exact copy hashes. No linked .git file copied.
- [ ] Prepare standalone Windows CLI + wizard/runtime resources using existing packaging helpers; hashes, help/doctor and wizard startup checks from destination, no old project/env root dependence.
- [ ] Record user manual PASS and artifact identities, known issues and verification limits; verify no secrets/game/temp data staged.
- [ ] Commit clean source snapshot on release/20261116 and push without force; inspect CI status or report authentication/runner blockers.

## Review focus
1. Linux must select ELF runtimepack, not Windows EXE; source cargo target path should work without override.
2. Portable bundle must resolve its own resource root independent of source and environment overrides.
3. RGSS native licensing gate and RTP exclusion apply to public source/history as well as bundles.
4. Broad git add must not accidentally include untracked staged game data inside _temp.
5. GitHub push and CI report must distinguish uploaded configuration, actual Linux run, and runtime gameplay tests.

## Progress
Plan approved in conversation. Existing linked worktree runtime-v2. Target RuntimePack-20261116 not yet created. Old runtimepack-master Git access refused due ownership; no safe.directory bypass or configuration change attempted. It will remain untouched.
