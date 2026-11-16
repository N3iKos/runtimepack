# VNM v4 Source Backup and Reviewed Feature Integration to Master

> **For agentic workers:** Use staged executor/reviewer/verification gates. Supervisor writes this plan. Preserve all worktrees and artifacts; no automatic cleanup or branch deletion.

**Author:** Supervisor main agent.
**Goal:** Create and verify an immutable ZIP of the stable pre-feature v4 source, then commit the user-tested input experiment, integrate it into local master, and push only to the explicitly selected GitHub repository without force.
**Architecture:** Baseline tag/branch remain at their original commit. The feature branch is committed separately. Master fast-forwards to the reviewed feature commit when local history permits. Remote history is inspected before publication; unrelated history is not overwritten.
**Tech Stack:** Existing Git, ZIP validation, gh, offline Rust/Node/Kotlin tooling.
**Spec:** User explicitly authorized: backup stable source first, commit worktree, integrate into master, push GitHub. User reports the worktree build feels better. Default guard remains off; the previously tested candidate used transition-aware explicitly. No claim all rapid-click stalls eliminated.

## Global Constraints

- Repository: `D:\02_Development\Projects_2\runtimepack-vnm-reconstruction`.
- Feature worktree: `.worktrees/vnm-extreme-input`, branch `feature/vnm-extreme-input`.
- Stable source commit: `52f9f2f5ef3225980f5a8a90462219341caed3cc`, branch `baseline/vnm-v4`, annotated tag `vnm-v4-baseline`. Never move/delete this branch/tag.
- Expected stable APK SHA256: `C39088E32F8D599B538AEC9D360F02CF02983E2BCC255BE6E3428EC12643ADDF`. APK is retained but not placed in source ZIP/Git.
- Expected feature APK SHA256: `55C777050BDF039CA2665C8F2BCBE9EE00780C09893CC8DE9084FBD0CFEBADD2`. Preserve it separately.
- User-provided author/committer: `N3ikos <epcg69@gmail.com>` via command/process environment only; no Git config mutation.
- Backup parent: `D:\02_Development\Projects_2\Backup`. Verify exists before creation. Fresh ZIP filename `runtimepack-vnm-v4-source-before-input-merge-20261005.zip`; if exists choose a verified unique suffix, never overwrite/delete.
- No deletion/cleanup/reset/clean, worktree or branch removal, amend, hook bypass, force-push, remote branch overwrite, installs/downloads, MuMu/ADB/UI/device/gameplay, app install, source game/save/backup APK mutation, or new APK build.
- No worker-authored plans or added code comments. Do not stage generated outputs, SDD private evidence, APK/game.pack/DEX/caches/keys/provider secrets.
- Only publish to explicit user-provided GitHub URL. No guessed repository or automatic repository creation. GitHub private is recommended because baseline contains native/runtime binary redistribution obligations and game screenshots in documentation.
- Use gh for GitHub repository/auth/status verification without printing credentials. No broad account/repository scan.

## Task 1: ZIP Stable Source — executor

- [ ] Inspect status, diff, recent log, worktree list and tag dereference. Confirm root clean and tag target exactly stable commit.
- [ ] Verify Backup parent and fresh destination. `git archive --format=zip --prefix=runtimepack-vnm-v4-baseline/ --output=<verified-path> 52f9f2f5ef3225980f5a8a90462219341caed3cc` captures the exact tracked pre-feature source, including required runtime assets/config/tests/docs. No APK, game, saves, build caches or linked .git pointer.
- [ ] Validate archive CRC/integrity and compare all ZIP regular-file paths/sizes/hashes against Git blobs in stable commit. Expected source tree 2,794 tracked files; if actual count differs investigate before success.
- [ ] Measure ZIP size/SHA256; record source commit/tag and validation results in feature worktree ignored SDD. Source ZIP is not a standalone Git repository; original Git history/tag remain preserved locally.
- [ ] Independently verify ZIP and unchanged baseline hash before any feature commit/merge.

## Task 2: Fresh Review/Verification and Feature Commit — qa-tester/code-reviewer/executor

- [ ] Read current diff and untracked files; latest plan is intentional source documentation, not generated/private data. Compare reviewed guard/config/bridge tests with final actual source.
- [ ] Run offline cargo check/clippy/workspace tests, all JS suites with genuine local VNM/JSZip paths, Kotlin compilation/JVM tests/lint. Record raw results/XML counts. Disclose the known vendor lint failure; do not call lint clean or add broad permissions.
- [ ] Check source input and baseline APK hashes unchanged. Verify no secret/private payload or output files in intended commit.
- [ ] Update worktree memory acceptance truthfully: user-tested candidate feels better, normal/Skip baseline retained, extreme white-screen root cause and efficacy remain not conclusively established. Default OFF and beta-only mode stay unchanged.
- [ ] Before commit inspect git status, unstaged diff, log --oneline -10, and explicit intended file list. Stage only those files including required new adapter/classes/tests/plans; no git add . / -A / --force.
- [ ] Inspect cached diff/stat/full new-file content and staged secret scan. Commit with concise repo-style message `feat: add optional VNM transition-aware input guard`. Use user identity via environment, no config changes. Commit failure handled normally; no amend/hook bypass.
- [ ] Verify exact commit/tree and staged empty/clean tracked worktree. Keep ignored evidence/build outputs and worktree in place.

## Task 3: Local Master Integration — executor after backup and feature gates

- [ ] Inspect actual local branches/master existence. Current known repo has baseline/vnm-v4 and feature/vnm-extreme-input only; do not assume master exists.
- [ ] If master absent, create it from the stable source commit in root checkout without renaming/moving baseline branch. If master exists, verify ancestry and clean state before integration; unrelated/divergent local work requires supervisor discussion, no reset.
- [ ] `git switch master`, then `git merge --ff-only feature/vnm-extreme-input` if master is an ancestor. If fast-forward is not possible, stop and report; don't silently choose an overwrite or unrelated-history merge.
- [ ] Verify master target feature commit, stable branch/tag still original, worktree preserved, no APK/save/source-input change. Run targeted checks on merged result; full suite if integration changes tree beyond already tested feature commit.
- [ ] No automatic worktree removal, prune, branch deletion or experimental flag default change.

## Task 4: Explicit GitHub Destination and Safe Push — executor/verification

- [ ] Obtain user repository URL if still missing. Validate exact owner/repo and visibility/auth via gh. No token values in logs.
- [ ] Inspect repository default branch, remote tracking/heads/tags and recent GitHub master commit. Initial local history is independent from the old upstream repository; publishing is not guaranteed fast-forward.
- [ ] If remote master is absent or is an ancestor of local master, use a normal non-force push of master, feature branch and stable baseline branch/tag as agreed in supervisor ruling. Do not mass-push other refs/tags.
- [ ] If remote master has independent history, stop before push to master. Explain normal push would reject; propose preserving remote history through a separately reviewed integration commit/PR or using a fresh private repository. Never force-push or merge unrelated histories without explicit approval.
- [ ] Adding an exact named remote after URL confirmation is authorized repository linkage; no git config user/credential changes. Do not replace an unexpected existing remote silently.
- [ ] Verify pushed remote commit SHA and stable tag/branch pointers with gh/Git remote query; report repository URL and local ZIP path/hash. No release/APK publication unless separately requested.

## Acceptance

Verified stable source ZIP completed first. Feature changes committed and integrated into local master without moving stable baseline. GitHub publication happens only to verified explicit target and non-destructive compatible history; any destination/history blocker remains clearly reported. Worktree, source/game/saves/APKs retained. Local ZIP on drive D is not protection against entire-drive failure; external/off-device backup still advisable.
