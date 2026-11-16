# Baseline Commit Gate 2 Addendum: Approved Identity and Staging

**Author:** Supervisor main agent.
**Spec:** 2026-10-05-vnm-v4-baseline-and-extreme-input.md, Gates 1–2.

## User-Provided Identity

User explicitly supplied commit identity `N3ikos <epcg69@gmail.com>`. Use process-only `GIT_AUTHOR_NAME`, `GIT_AUTHOR_EMAIL`, `GIT_COMMITTER_NAME`, and `GIT_COMMITTER_EMAIL` for the authorized baseline commit and annotated tag. Do NOT change local/global Git configuration. No remote or push authorized.

## Reviewed Staging Decisions

The reviewer approved the candidate source allowlist (2,793 regular files) with output exclusions. Apply bare .gitignore patterns only, without comments:

```gitignore
/dist*/
dist*/
*.gpack
game.pack
__pycache__/
*.pyc
local.properties
SDD/
*.log
*.dex
Raw-Games/
saves/
```

Verify essential source/test paths are not hidden by these additions. Native .so/font/wrapper assets have existing notices and are included for LOCAL source preservation only; no public redistribution approval implied. Retain duplicated root SDD reports but exclude them from index, never delete them.

## Execution Contract

1. Source/test/config paths in the reviewed allowlist only, plus this supervisor-authored addendum and intentional existing memory/progress/decisions acceptance updates. Re-audit any file/hash drift before staging. No arbitrary newly discovered paths enter index.
2. Read the existing memory files before editing and record v4 user acceptance scope and rapid-auto limitation; no blanket all-engine or full-overlay device stability claim.
3. Run actual configured baseline checks offline and capture failed lint status. No production fixes or new behavior during baseline preservation.
4. Initialize Git only after verifying no repository exists at/above the exact root; branch `baseline/vnm-v4`. No Git history is recovered by this operation.
5. Inspect git status, diff and initial log result before commit; stage reviewed relative paths explicitly, preferably a dedicated literal pathspec allowlist file. No `git add .`, `git add -A`, forced staging, reset, clean, hooks bypass or configuration mutation.
6. Inspect exact cached file list/diff/stat; scan staged content without exposing secrets. Confirm no game.pack/APK/DEX/dist/cache/SDD/save/key/provider credential included.
7. Commit `feat: establish reconstructed VNM v4 baseline`. Annotated tag `vnm-v4-baseline` may be created if absent, using user identity and acceptance text. Do not amend or push.
8. Independently verify commit/tree/tag/index state, accepted source and APK hashes. Commit report in ignored SDD. Any unexpected source drift or author/signing/hook blocker stops affected gate; do not invent identity/config workaround.

Worktree creation follows only after this baseline is independently verified. Experimental changes are not automatically committed or merged.
