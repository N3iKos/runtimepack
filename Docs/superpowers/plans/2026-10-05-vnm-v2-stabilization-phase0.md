# VNM v2 Stabilization Phase 0: Audio/Focus and Diagnostic Evidence

**Author:** Supervisor main agent.
**Goal:** Establish source-backed diagnoses for the user's two audio exceptions and inconsistent audit/share report before proposing code changes.
**Architecture:** Two independent read-only investigation domains, followed by evidence review and supervisor-authored bounded implementation plans. No game bootstrap or device execution.
**Evidence:** User tested reconstructed v2, SHA-256 `18F8E1A5FE064CE15C888C8140434FD510EA05DCAF3ACBAFA4EEFDD0F6FB781F`. Save currently works according to user. Gameplay failed before chapter 2, editor/hide interactions freeze; distinct pause and stop stacks read a null `error` property.

## Global Constraints

- Active isolated workspace: `D:\02_Development\Projects_2\runtimepack-vnm-reconstruction` (non-Git source snapshot).
- Original source export: `D:\02_Development\Projects_2\runtimepack-master`, immutable.
- Fresh game: `D:\Old-Recvery\Project_2\RPGM-P\Raw-Games\Time for Punishment Class ~Taking Lessons for My Boyfriend~`, immutable.
- User report: `dist_vnm_reconstructed_v2/Audit-app.txt`, read-only evidence.
- Preserve prior APKs, reports, streaming, native saves and recovery. No deletion, cleanup, reset, Git initialization/commit/merge/push, installs/downloads, MuMu, ADB, emulator/UI interaction, or APK build in diagnosis.
- Supervisor personally writes plans/specifications. Workers return diagnoses and source references; no worker-authored plans.
- No recovered game/project code execution. Isolated authentic method reproduction is allowed only after identifying the snippet and using a constrained local test harness; no game bootstrap.
- No code changes in Phase 0. Reports may be written only under `.superpowers/sdd/2026-10-05-vnm-v2-stabilization/` after verifying parent paths.
- No assumptions that old synthetic test passes establish current gameplay correctness. No guessed exception suppression, forced singleton AudioContext, feature removal, or zeroing audit failures.

## Observations to Preserve

1. Popup/layout interaction: `ENGINE.js:1:49411` getter, `pause` at column147228, callback at81294. User distinguishes this from normal-dialog progression.
2. Dialog progression: `ENGINE.js:1:52688` stop, `AudioManager.stopAllSounds` around Main_112.js3673/3676, SceneManager.update2429 and Main.updateFrame44855.
3. Current export: `engine_family: rpgmaker_web`, static outcome NOT_RUN/static_summary null, runtime total1976, passed1079, failed897. Screenshot UI Missing7. Static HTML missing0 is a different scope, not proof of runtime decoding.
4. Source inventory1976 versus packed entries1969: seven-file difference is a hypothesis until exact paths are identified.
5. 897=890+7 suggests historical encrypted-image probe issue but is not a verified classification. Identify actual requested formats and stage outcomes.
6. Screenshot memory counter is JS heap telemetry, not direct evidence of Java heap OOM. No new OOM diagnosis without logs.

## Task A: Audio and focus diagnosis — executor

- [ ] Read project instructions, required canonical docs and relevant reconstruction reports/source.
- [ ] Map 1-based ENGINE.js columns49411/52688/147228/81294/81646 to exact class/method/properties with source offsets and snippets. Avoid off-by-one/source-version claims without hash check.
- [ ] Trace reference creation, assignment, pause/resume/stop/dispose and global active-audio collection removal. Identify null `audioBuffer` versus null `source` versus other holder; inspect class inheritance and native/WebIDL callbacks.
- [ ] Compare current wrappers in runtimepack_web_shims.js to actual game prototypes and registration timing. Determine whether wrappers cover the exact class, disposal order and active-list lifecycle.
- [ ] Trace overlay modal open/Done, eye visibility toggle, text focus and activity focus/background pathways in WebActivity/vpad and engine blur/focus handlers. Distinguish normal intentional pause while modal open from unrecoverable exception/focus loss after close.
- [ ] Identify a minimal faithful isolated regression for each user's stack using actual method/closure snippets. Return reproduction feasibility and specific uncovered state transitions, not broad guessed patch code.
- [ ] Return supported root cause(s), remaining timing uncertainties, exact minimal changed-file candidates, contracts to preserve (valid audio, shared buffers, native save, streaming, full overlay) and diagnostic needs.

## Task B: Probe counters and share/export diagnosis — executor

- [ ] Read Audit-app.txt fully and build static/runtime reports; inspect diagnostic_suite.js, runtimepack_asset_audit.js, AssetProbeRunner, AssetAuditService/Bridge, WebActivity engine/static report plumbing, runtime integrity and packet source filtering.
- [ ] Trace wrong engine label and missing static report separately from numerical classification.
- [ ] Produce exact set difference of source inventory and game.pack index via bounded read of header/index only. Identify all seven paths and actual reason each was excluded; don't classify excluded source files as gameplay missing assets.
- [ ] Inspect a bounded representative set of reported graphics/font failures and legitimate loader behavior to establish whether encryption/ciphertext probing explains890. Inventory evidence is separate from dependency or decode success.
- [ ] Trace UI counts7 vs runtime failed897 to actual counter predicates/events/report aggregation. Identify mismatched count scopes or discarded statuses. Record existence/readability/integrity/decode claims that currently overstate verification.
- [ ] Trace share export input: 250-entry live console ring, failure-report preservation, exception capture from game versus overlay WebViews, missing detail/run identity and truncation metadata.
- [ ] Return concrete report schema/UI requirements using existing interfaces: full per-asset failure details and stack traces; missing/decode/indeterminate/excluded separate; static/runtime linked without overwriting either; no save payloads or secrets.
- [ ] No implementation, new dependency, full media decode, giant asset allocation, or device access.

## Task C: Evidence review — code-reviewer

Review the two source diagnoses, exact pack exclusions, exception lifecycle and counter/export traces. Reject speculative format/cause claims and test-only helper logic. Supervisor then writes separate audio/focus and audit/export plans with red-green regression acceptance. Implementation proceeds sequentially to avoid overlapping shared WebActivity/diagnostic source edits.

## Exit Criteria

Source-backed explanations of both exceptions, exact seven-file set difference, truthful scope of decode failures, identified engine/report/export bugs, and specific regression targets. Unknowns remain explicitly unknown. No source code, saves, original artifacts or game data changed.
