# VNM Extreme Input Gate 4: Reviewed Evidence and Optional Transition-Advance Guard

> **For agentic workers:** Use subagent-driven-development with task reviews. Supervisor owns the specification and plans. No experiment commits, merges, or cleanup.

**Author:** Supervisor main agent.
**Goal:** Add a beta-only, default-OFF VNM transition-aware dialogue advance experiment and bounded diagnostic snapshots, preserving the accepted v4 baseline and raw input semantics.
**Architecture:** A separate VNM input adapter asset wraps only mouseUp callbacks registered by the authentic message text-renderer. Raw Android/DOM touch delivery remains unchanged. During a precisely verified active screen transition, dialogue callbacks are suppressed without replay; interactive contexts fail open. Diagnostics sample safe own-data scalars and counters at low rate and are retained by a narrowly validated native bridge. This is an experiment, not a proven white-screen fix.
**Tech Stack:** Existing Rust build metadata, Kotlin/MiniJson bridge and tests, JavaScript genuine-source Node VM tests; no new libraries.
**Spec:** User-authorized baseline-and-extreme-input plan Gate4; Gate3 evidence review and authentic SCRIPTS9901–9952,9994–10007,29521 and ENGINE transition/Mouse source.

## Global Constraints

- Work ONLY in `D:\02_Development\Projects_2\runtimepack-vnm-reconstruction\.worktrees\vnm-extreme-input`, branch `feature/vnm-extreme-input` from `52f9f2f5ef3225980f5a8a90462219341caed3cc`.
- Baseline root/branch/tag `vnm-v4-baseline`, original export, backup APK/extraction, raw game, saves and baseline APK remain immutable.
- No deletion/cleanup/reset/Git config/commit/amend/merge/push, installs/downloads, MuMu/ADB/UI/gameplay, game bootstrap, or APK build before review/QA.
- Main supervisor writes plans; workers report discrepancies rather than replacing requirements. No code comments. Use existing pinned dependencies only.
- No generic click-rate cap, physiological claims, arbitrary transition delay, raw-event dropping, forced interpreter advance/end callback, broad catch that hides game errors, private variable/text export, debug port enabling, whole game decryption or scene script rewriting.
- Preserve single DOM key dispatch and authentic key getters, native save/legacy read compatibility, bounded GPACK streams, audio reference/pool fixes, full/passive overlay modes and existing diagnostics.

## Evidence Corrections and Limits

1. Root cause of extreme-auto white-screen stall remains UNKNOWN. DOM queue saturation, VSync and doubleadvance are hypotheses, not established facts.
2. Graphics.transitionData is actual screen-transition descriptor, cleared on completion; Graphics.frozen is actual plain boolean. SceneManager.isFadeOut concerns whole-scene/exit behavior and is not the chosen in-game predicate.
3. Component_SceneBehavior already bypasses updateContent during Graphics.frozen and flushes Input.update intentionally. A transition gate may be inactive in those frames; diagnostic counters distinguish whether it actually intercepts events. Do not promise elimination of the observed stall.
4. waitingFor.choice is never truthily established; scene.choices is the actual choices array. Don't invent private state/property names.
5. Component_MessageBehavior.actionTrigger mainly cancels Auto mode; normal pointer dialogue advance is registered by gs.Component_MessageTextRenderer.setupEventHandlers on gs.GlobalEventManager `mouseUp`. This callback both calls continue() and, in its alternate branch, changes drawImmediately/waitCounter/waitForKey/isWaiting. Wrapping continue alone misses the latter branch.
6. Built-in Skip directly calls continue() from instantSkip and is preserved unwrapped. Keyboard shortcuts and generic UI event handlers remain untouched.

## Files and Interfaces

Create only explicitly required files:
- `android/shell/app/src/main/assets/web_shims/runtimepack_vnm_input_guard.js`: isolated adapter/IIFE, narrow install hook.
- `android/shell/app/src/test/js/vnm_extreme_input_guard.test.js`: authentic text-renderer method extraction and event-manager harness.
- `android/shell/app/src/main/java/io/runtimepack/host/VnmInputGuardDiagnostics.kt` and JVM test if bounded snapshot storage cannot fit existing DiagnosticLogStore safely.

Modify existing CLI/build-manifest/schema/generator, Kotlin BuildManifestLoader/WebActivity/WebBridge/VfsResourceInterceptor and their tests only where new config/resource/bridge needs wiring. Capability/evidence registry entries use next-free IDs; no marker comments. Existing runtimepack_web_shims.js should remain byte-identical if separate adapter can handle all hooks. Avoid broad refactoring.

Public contracts:
- CLI `--vnm-input-guard` values `off` (default) and `transition-aware`.
- BuildManifest additive `vnm_input_guard` string with serde/Kotlin default `off`; exposed through existing getRuntimeBuildInfo.
- Only VNM beta may enable transition-aware. Invalid mode/non-VNM/release rejected before output creation/packing/compile. Full and passive overlays supported, no default profile changes.
- VFS post-SCRIPTS injects the separate blocking adapter script ONLY when VNM + transition-aware. The adapter installs immediately after its evaluation, when actual renderer class exists. Missing target/hook yields bounded explicit diagnostic, not fake protection. Do not alter engine source.
- Allowlisted resource `web_shims/runtimepack_vnm_input_guard.js` served by existing controlled local-origin asset route; unrelated/mode-off request cannot silently enable it.
- Narrow native `postVnmInputSnapshot(json): Boolean` on game bridge(s). Host validates exact allowed keys/types and size before storing; no arbitrary JSON/message/stack/player payload accepted.

## Task 1: Configuration, served asset and diagnostic transport (executor)

### RED tests

- Mode defaults off across CLI/serde/Kotlin missing field. Enabled rejected for release/non-VNM/unknown value before side effects.
- VNM beta enabled generated manifest matches host config; full/passive still valid.
- Actual VFS transform yields ordered ENGINE/SCRIPTS -> normal audio/storage/voice hooks -> new guard asset before DOM-ready startup. Off/non-VNM served HTML has NO guard script. Missing/async bootstrap anchors preserve existing rejection.
- Allowlisted guard resource gets actual bytes/MIME from OverlayAssetSource when enabled; unknown path cannot escape namespace. Off route remains safe and no script executes accidentally.
- Production manifest parse retains sourceHash/runtime/app/overlay metadata and optional new field.
- Native snapshot validation rejects malformed/deep/oversize JSON, unknown keys, non-finite numeric state, arbitrary messages/text/save variables, wrong profile. OFF bridge returns disabled without starting observer/storage.

### Implementation

Reuse exact existing CLI and manifest plumbing, no new general policy subsystem. Generator includes guard asset when enabled; if template assets are copied in OFF builds, they are inert and no script tag/hook is activated. RuntimeIntegrity computed AFTER final staged asset bytes; generated project tests compare independent current asset oracle, not hardcoded production hash.

Diagnostic scalar allowlist (all optional/null if unavailable):
`schema_version=1`, `sequence`, `elapsed_ms`, `mode`, `frame_count`, `frame_delta`, `graphics_paused`, `graphics_stopped`, `graphics_frozen`, `transition_present`, `transition_alpha`, `transition_duration`, `left_button_state`, `button_down`, `button_up`, `mouse_down_count`, `mouse_up_count`, `touch_cancel_count`, `dialogue_callback_count`, `blocked_dialogue_count`, `delegated_dialogue_count`, `choice_count`, `video_present`, `skip_active`, `auto_active`, `unknown_state_count`.

Adapter sample interval 1,000ms ONLY enabled profile. No full scene/command arrays, text, variables, save data, source names or resource URLs. Read fields through property descriptors/plain own data only; getters with side effects are never invoked. Missing/unsupported fields represented null and reason through bounded numeric unknown counter, not guessed healthy state. Capture transition alpha/frame trends without classifying static frameCount as proof of RAF termination.

Host keeps at most64 snapshot records, max2KiB serialized UTF8 each, max128KiB retained, with total/stored/omitted counters. Store latest validated snapshots in app-private diagnostic state; persist at most once every5,000ms via a latest-only coalesced job/appropriate existing service boundary, not per touch or UI-thread full-tree writes. Atomic metadata replacement only; no source/game/save writes. Existing bug export may add input_guard_snapshot when full mode bridge provides it; passive user can provide captured app-owned report later with explicit permission, no automated ADB now.

## Task 2: Authentic callback guard and observer (same integration deliverable as Task1)

Task1 and Task2 share the new adapter asset: implement them sequentially within one fresh executor task and review the complete integration package. Do not leave a no-op placeholder or pretend installed protection while the adapter is absent. Build remains blocked until both are verified.

### RED tests

Extract exact setupEventHandlers/disposeEventHandlers and continue methods from fresh SCRIPTS without full game execution. Stub external event/native renderer dependencies, NOT method logic. Genuine event-manager semantics retain owner, e.breakChain, and original return behavior.

Test cases:
- Off: identical callback mutations/returns/throws and no listeners/timer added by adapter.
- Active valid transition: renderer mouseUp callback does not call continue OR clear waitCounter/waitForKey/isWaiting/drawImmediately. No input flags reset, no queue/replay; menu/global event chain continues normally.
- Transition cleared: next valid tap delegates exactly once with original this/args/return and current renderer; previously suppressed clicks do not replay into new dialogue.
- Partial-text tap outside transition sets drawImmediately exactly as original.
- Positive scene.choices/non-null scene.video, actual explicit input prompt, built-in skip/Auto: fail open or leave original logic untouched. Unknown state/descriptor/accessor: fail open and counter increments; NEVER use generic interpreter.isWaiting or waitingFor.choice.
- KeyUp handler unchanged; direct continue()/instantSkip unchanged. No delay/rate cap on raw DOWN/UP/CANCEL and held-state semantics.
- Renderer setup called repeatedly: original offByOwner disposes old listener; no duplicate wrappers/listeners. Restore/save serialization excludes adapter internals (WeakMap sidecar/nonenumerable state).
- Separate renderer instances do not share blocked/delegated ownership. Non-VNM globals not patched. Original game exception still propagates.
- Snapshot observer records actual own-data scalars/nulls with bounded retained host history and no player text; caps maintained after long synthetic burst, one latest coalesced metadata persistence job.

### Narrow hook algorithm

Wrap only `gs.Component_MessageTextRenderer.prototype.setupEventHandlers` idempotently. During synchronous original setup, intercept that same gs.GlobalEventManager.on registration locally and only for event `mouseUp` with owner === current renderer.object; replace its callback with a closure checking the strict predicate then delegating original. Forward authentic `on(eventName, handler, data, owner, priority)` arguments verbatim with the original receiver; keep owner/priority/data so offByOwner cleanup works. Restore event-manager on function in finally even if original setup throws. No permanent generic global event suppression. Nested setup must safely bypass further temporary interception (fail open) or use a tested reentrant owner stack; do not wrap another renderer's or commandWaitForInput's registration. Owner association and original handler this/arguments/return verified in tests.

Strict predicate:
- Mode exactly transition-aware, VNM class/source profile recognized.
- Graphics.transitionData is a confirmed own-data non-null object with finite duration>0 and finite currentAlpha<255. No generic frozen/isWaiting/fadeOut predicate alone.
- SceneManager.scene and required context available through safe data descriptors; if unavailable => fail open.
- scene.choices length>0 or scene.video non-null => fail open (not suppressing choice/video interaction).
- GameManager.tempSettings.skip true or GameManager.settings.autoMessage.enabled true => fail open, preserve built-in paths.
- Number/text prompts use actual interpreter.waitingFor.inputNumber and inputText; inspect these only via safe data descriptors when available, and fail open if truthy. Generic commandWaitForInput handlers have owner=this.interpreter.object, not renderer.object, and must not be intercepted. Use no invented waitingFor.choice, command IDs or dialog text to infer state. If required context cannot exclude interaction confidently => fail open.
- Return false for all unknown formats/version states. Guard suppresses renderer callback only; doesn't modify the event object, breakChain, raw input events, Graphics state, timers or interpreter.

Passive capture listeners on window may increment numerical input counts only, never preventDefault/stopPropagation or query side-effect getters. One interval observer per enabled install; no observer while document hidden if current descriptor permits visibility check; stop/pause samples on page lifecycle without mutating game. Diagnostics errors must not throw into game callbacks; bounded one-shot host bridge failure warning.

## Task 3: Review and independent non-device QA

Per-task reviewers check config defaults/enable scope, actual VFS asset delivery, descriptor-safe state, callback owner/restoration/no global rate cap, valid original paths and menu/choices/Skip/release invariants. No self-testing mock duplicate logic. Independent qa-tester runs offline cargo check/clippy/workspace tests, ALL existing/new JS suites with genuine VNM_GAME_ROOT/JSZip, nodecheck, Kotlincompile/JVM/lint. Report actualrawcounts; known vendor lint failures disclosed. Baseline branch/tag/source/APK unchanged. Experiment code remains uncommitted in feature worktree.

## Task 4: Experimental beta build and artifact gate

Only after source reviews and QA approved, build to fresh worktree `dist_vnm_extreme_input_beta` using actual verified flags:
`cargo run --offline --bin runtimepack -- build "D:\Old-Recvery\Project_2\RPGM-P\Raw-Games\Time for Punishment Class ~Taking Lessons for My Boyfriend~" --flavor beta --overlay-mode passive-watermark --vnm-input-guard transition-aware --accept-audit-unknown --output dist_vnm_extreme_input_beta`

No --probe/install/replay. Verify source/shim/guard hashes, new mode and actual guard script load path, passive static watermark, snapshot bridge/classes compiled, bounded GPACK preserved, native save compatibility unchanged, staged runtime-integrity expected=actualAPK, signature/package/SDK/ABI. OFF default tests are baseline comparison; no second huge APK unless needed.

Report paths in worktree ignored `.superpowers/sdd/2026-10-05-vnm-extreme-input/`: task1-config-report.md, task2-guard-report.md, review packages, verification/build/artifact reports, progress.md. Supervisor tracks rulings. No worker plans or source changes in baseline root.

## Completion and Honest Handoff

Baseline commit/tag preserved and clean; experiment isolated and default-OFF, candidate beta enabled for user test. State-aware protection may suppress zero callbacks because original engine already ignores frozen updates; diagnostics reveal that. Do NOT claim white-screen fixed until user repeats same slow/rapid input scenario. If no suppression occurs and stall remains, next step uses captured transition/input state rather than adding arbitrary global timing restriction. No forced progression or fake callback completion.
