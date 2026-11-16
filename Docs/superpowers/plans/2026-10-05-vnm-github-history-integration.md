# GitHub History-Preserving Integration Addendum

**Author:** Supervisor main agent.
**Applies to:** `2026-10-05-vnm-backup-merge-master-push.md` Tasks 2–4.

## Explicit User Decisions

- Destination: `https://github.com/N3iKos/runtimepack`, confirmed PRIVATE, authenticated N3iKos ADMIN.
- User selected **preserve existing history** through a controlled integration branch, then normal push to master. This explicitly authorizes joining independent histories; NO force-push, reset, history replacement or branch deletion.
- Remote master observed at `fd70c960fab1033fd35c29977cd7a209270359d4`. Recheck before fetch/integration/push; a new remote commit must not be ignored.
- Local stable baseline `52f9f2f5ef3225980f5a8a90462219341caed3cc` and annotated tag remain fixed.
- Stable source ZIP verified at `D:\02_Development\Projects_2\Backup\runtimepack-vnm-v4-source-before-input-merge-20261005-1.zip`, 45,138,125 bytes, SHA256 `941C0E00D4A9DDD22F437C16D4E64A78A33C9E95AD2A76E8E31A5DD185F45C84`; all2,794 source files match stable Git blobs.

## Revised Integration Order

1. Fresh feature tests/source review/security staging; commit only approved worktree changes with user process identity N3ikos <epcg69@gmail.com>. Preserve worktree. Commit feature before integration.
2. Add `origin` ONLY if absent, exact user URL, then fetch remote master normally (no forced local branch update). Never change user/credential configuration. Link remote is approved.
3. Create fresh ignored `.worktrees/vnm-master-integration` branch `integration/vnm-master` from exact fetched origin/master after ancestry check; no overwrite/collision cleanup.
4. In this separate worktree run `git merge --no-ff --no-commit --allow-unrelated-histories feature/vnm-extreme-input` with user identity. Capture status and conflict file list BEFORE resolution. A conflict exit is expected evidence, not license for force/reset/abort/delete.
5. Resolution policy is selective: preserve both parent histories. Accepted reconstructed feature source is authoritative for VNM host/runtime/build/tests and current instructions/memory. Identical shared files merge normally. Preserve remote-only files unless a specific review finds generated/private data; never globally choose ours/theirs, use merge strategy ours, or delete remote files to force identical trees. Conflicting docs/config/native assets get per-path review against feature and remote rather than blanket replacement.
6. Worker reports conflicts and proposed per-file outcomes to supervisor; code-reviewer validates resolution matrix. Supervisor writes exact resolution brief if needed. No automatic conflict commit before review.
7. After approved resolutions, verify no conflict markers/unmerged index, source feature behavior retained, remote-only docs preserved, baseline branch/tag untouched, full tests and staged-secret scan. Inspect gitstatus/diff/log--oneline-10 and cached diff then commit integration `merge: preserve upstream history with reconstructed VNM runtime` using process identity. No amend/hook bypass.
8. Verify integration commit has BOTH fetched remote head and feature commit as parents/ancestors. Review actual delta against origin/master for accidental omissions/credentials/game payloads.
9. Create local master from fetched origin/master if absent; otherwise require existing master to be an ancestor of integration. Root checkout may switch to master only if clean, leaving baseline/vnm-v4 branch/tag unchanged. `git merge --ff-only integration/vnm-master` advances local master. No fallback reset.
10. Recheck GitHub master. If it still equals fetched parent and is an ancestor, normal non-force push master. Also explicitly push `baseline/vnm-v4`, `feature/vnm-extreme-input`, and `vnm-v4-baseline` for source preservation; do not push other refs or publish APKs/releases. If a destination ref already differs, stop for review rather than force/overwrite.
11. Verify GitHub master SHA and remote branch/tag pointers equal intended commits via gh/Git. All worktrees and branches remain on disk; no worktree removal/prune.

## Safety and Visibility

No source game/save/APK/archive/caches/private SDD/key/provider configuration is published. Current repository remains PRIVATE; screenshots/native runtime assets have existing provenance but public redistribution is not authorized. Source ZIP is pre-feature baseline, not a Git bundle or entire-drive backup. User-tested guard remains default OFF in source unless explicitly enabled at build time; no promise all extreme-input stalls are fixed.
