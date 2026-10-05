# VNM v4 Baseline Commit and Isolated Extreme-Input Experiment

> **For agentic workers:** Use subagent-driven-development with spec and quality review at each gate. Supervisor authors plans. Only the baseline commit is authorized; experimental work is not automatically committed or merged.

**Author:** Supervisor main agent.
**Goal:** Preserve the user-accepted v4 reconstructed source as a local Git baseline, then investigate and implement a narrowly scoped optional VNM extreme-input safeguard in a separate worktree.
**Architecture:** Baseline source remains unchanged after commit. A feature branch/worktree holds diagnosis, tests and beta-only experimental behavior. Source/game/saves/reference APKs are immutable inputs. Build-time selection keeps protection disabled by default.
**Tech Stack:** Existing Git, Rust, Kotlin and JavaScript tooling; pinned local Android toolchain. No new dependencies.
**Spec:** User explicitly authorized Git baseline commit, worktree creation and execution of the discussed input approach. Gameplay acceptance: user says v4 behaves well like old APK; normal/manual taps and built-in Skip work, very fast auto-clicking can hold a white-screen transition in both old and v4 APKs. This is not proof of any specific timing mechanism.

## Global Constraints

- Baseline root: `D:\02_Development\Projects_2\runtimepack-vnm-reconstruction`.
- Expected baseline APK v4: `dist_vnm_reference_v4_passive/TimeforPunishmentClass~TakingLessonsforMyBoyfriend~.apk`, SHA256 `C39088E32F8D599B538AEC9D360F02CF02983E2BCC255BE6E3428EC12643ADDF`. Verify actual path and bytes; APK stays outside Git.
- Original export `runtimepack-master`, old backup `Backup/TFPC-TLFMB.apk`, existing user extraction, recovery root, raw game and saves remain unchanged. Do not delete, reset, clean, overwrite, uninstall, clear data, or run game code.
- No MuMu/ADB/device/UI/gameplay, downloads, remote creation, Git configuration changes, hook skipping, amend, force-push or push. User authorized a LOCAL baseline commit only. Git is not an external backup; remote backup remains pending separate authorization.
- Supervisor writes every plan and implementation decision. Workers audit, implement, test and report; no worker-authored plans.
- No added code comments. No global arbitrary 12–15 taps/s restriction, timers guessing scene readiness, forced interpreter advance, pretend ended callbacks, or error swallowing.
- Preserve current native saves, UID/legacy read compatibility, bounded GPACK streams and authentic voice/audio lifecycle contracts.
- Keep full overlay support and passive-watermark profile; primary experiment uses passive profile for comparison. No permanent overlay removal.

## Correction of Earlier Explanation

Prior claims that auto-clicking definitely floods a queue, skips an uncreated dialog, restarts transitions, or that A+B guarantees no stalls were speculative. Only the user's reproduced rapid-input stall is established. Determine the actual engine/input waiting condition before choosing protection. Human-rate numbers cannot be treated as a physiological guarantee or universal safe threshold.

## Gate 1: Safe Baseline Inventory — executor, read-only first

- [ ] Read current project instructions/memory and baseline reports. Verify repository state: no.git expected, but detect changed state instead of overwriting a repository. Check nested Git directories, links/reparse points and active build processes; do not kill processes.
- [ ] Identify current source relative to v4 generated template/source hashes and session snapshots. Any unrelated unreviewed change is excluded or surfaced, not silently certified baseline.
- [ ] Construct explicit source/doc/config/runtime/test staging allowlist with relative paths and sizes. Include necessary runtime/native binaries with existing license/provenance, not whole game or generated binaries.
- [ ] Exclude `target`, every generated root `dist*` output, Android/Gradle builds/caches, `.worktrees`, `.runtimepack`, `.superpowers`, APK/AAB/game.pack and extracted archives/DEX, raw game/save directories, keys/certificates/auth/provider secrets and environment files. Do not rely solely on existing dist/ rule, which does not cover dist_*.
- [ ] Audit candidate source and test fixtures for secrets and full private game/save payloads without printing secret values. Record only path/category if found. Ambiguous sensitive files are NOT staged until resolved.
- [ ] Preserve minimal accepted-baseline status in existing memory/progress: normal gameplay user accepted; rapid-auto transition known minor; full overlay/ripple toggle not newly device-certified; release NOT_VALIDATED. No source behavior change.
- [ ] Return inventory, proposed .gitignore additions and commit contents; no init/stage/commit until reviewer approves allowlist.

## Gate 2: Baseline Review and Commit — code-reviewer then executor

Reviewer verifies staging/exclusions and source correspondence to accepted v4. Supervisor decides any corrections before commit.

Executor may then:
- [ ] Update existing .gitignore only with approved output/security exclusions; do not broadly hide required source tests/native binaries. `.worktrees/` must be ignored before creating it.
- [ ] Run available baseline checks offline: cargo check/clippy/workspace tests, all JS suites with authentic game/library path, Kotlin compile/JVM tests/lint. Capture known vendor lint failure explicitly; no artificial permission fix. Failures outside accepted known baseline are blockers.
- [ ] Initialize Git ONLY if still absent: `git init -b baseline/vnm-v4` from verified baseline root. Do not change user.name/email or other Git configuration. Existing identity absent => report blocker and ask user, no guessed identity.
- [ ] Inspect `git status`, `git diff`, `git log --oneline -10` (initial no-history result recorded), .gitignore status and candidate list. Stage only reviewed paths explicitly (batch or verified list); NEVER git add . or git add -A.
- [ ] Inspect cached diff/stat/name list and secret scan of EXACT staged content, ensure no generated artifact/private save/credential entered index. Initial commit diff is the entire approved source, not a fabricated upstream history.
- [ ] Commit once with `feat: establish reconstructed VNM v4 baseline` (or existing repo-style equivalent if actual history exists). No amend/hook bypass. Commit failure => fix approved issue and create successful commit normally; no empty commit.
- [ ] Verify resulting SHA, clean tracked source, staged files empty, expected APK/reference/source hashes unchanged. Create annotated tag `vnm-v4-baseline` only if tag not already present, labeling user acceptance scope and remaining rapid-input limitation. Tag is a baseline marker, not universal release support.
- [ ] Write baseline manifest/report locally under ignored SDD. No push or remote setup.

## Gate 3: Worktree and Read-Only Input Diagnosis — executor

- [ ] Check current Git/worktree isolation and .worktrees ignored. Create fresh `.worktrees/vnm-extreme-input` on `feature/vnm-extreme-input` from exact baseline SHA; if path/branch exists, inspect and stop rather than overwrite. Verify parent before creation; no deletion.
- [ ] Run targeted baseline tests in worktree. All production changes subsequently occur there only; baseline branch/source/APK remain frozen.
- [ ] Trace authentic VNM touch/mouse update, command waits, show-message progression, movie/fade/timer transitions and built-in Skip. Use existing source matching hashes, never execute whole game during detection. Identify state fields and getter side effects before any instrumentation.
- [ ] Reproduce minimal rapid-touch state-machine issue with isolated authentic methods where possible. Determine event sequence frequency, DOWN/UP pairing, cancellation, focus, whether wait is elapsed-time/input-release/animation/video/resource and how built-in Skip differs. Do not assume previous hypotheses proven.
- [ ] Return source-backed diagnostic, concrete test seams and minimal safe hook boundaries. No production guard implementation yet; supervisor writes detailed plan from this evidence.

## Gate 4: Optional Guard Design and TDD — supervisor plan, executor, reviewer

Two possible mechanisms are conditional on diagnosis:
1. **State-aware advance gating:** suppress only dialogue-advance intent during a specifically verified noninteractive transition. Never gate on generic isWaiting, because messages/choices/video skip/input prompts may legitimately wait for user input. No queuing/replaying suppressed clicks after a new scene/dialog appears.
2. **Emergency burst guard:** if source/evidence shows high-rate input is the cause and state gating is insufficient, optional bounded rate limit scoped to VNM advance action (not all Android gestures). Threshold derived from tests, not asserted human limit. Preserve paired DOWN/UP/CANCEL and first valid action.

Do not automatically combine both. Preserve raw menu/choice/overlay/system input, held keys, Skip/Auto and release events. No filter intercepting DOWN while forwarding its UP to an unrelated gesture; no stuck buttons. Default disabled and beta-specific build metadata records selected mode. Baseline behavior unchanged when off.

If exact stall cannot be reproduced with available non-device evidence, implement a bounded diagnostic capture plus explicit experimental guard only if its semantics can still be proven. Do not label speculative guard as root-cause fix or guarantee. Stop affected behavior change if every hook is guesswork; report concrete missing evidence.

## Gate 5: Review, Verification and Experimental APK

- [ ] Per-task and whole-worktree code-reviewer gates before build. Fix blockers through implementer, never silently bypass protection.
- [ ] Independent non-device QA validates normal taps, burst frequencies and durations, transition readiness, release/cancel, menu/choices/Skip/Auto, save-restart reads and full/passive modes. Verify authentic sources are used; no duplicate test implementation or skip-as-pass.
- [ ] Run actual offline lint/typecheck/test commands. Known vendor lint failures disclosed, not hidden. Baseline SHA/tag and source/APK hash remain unchanged.
- [ ] Build experimental beta from worktree to fresh `dist_vnm_extreme_input_beta`, with actual verified CLI mode flags, passive-watermark and explicit audit unknown acceptance. No --probe/device install or generated-source workaround. Preserve all outputs.
- [ ] Independent artifact verification: size/hash/signature, runtime integrity expected=actual, guard configuration, current shim/DEX generated source parity, pack/profile metadata and source invariants. Gameplay user QA pending.
- [ ] Do not merge or commit experimental branch changes without further user authorization. Return baseline SHA/tag, worktree path, APK path/hash and exact guard behavior/limitations.

## Acceptance Boundary

Local Git baseline and isolated worktree verified. Optional experiment is separately reviewed/tested and delivered without altering accepted baseline. Extreme-auto white-screen behavior remains user-tested acceptance criterion; normal gameplay and Skip must not regress. No remote backup, 1:1 upstream repo restoration or universal compatibility claimed.
