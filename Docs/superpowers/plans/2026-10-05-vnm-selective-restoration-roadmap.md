# VNM Legacy Reference Selective Restoration: Design and Execution Gates

> **For agentic workers:** Use subagent-driven-development with per-task spec/quality review. Supervisor authors all plans; workers implement and report only. No automatic commits or workspace cleanup.

**Author:** Supervisor main agent.
**Goal:** Reproduce the verified old APK's direct-touch behavior as an optional beta profile, fix the concrete save-load null audio-pool failure, and read old save storage without changing existing records.
**Architecture:** Selective recovery, not wholesale legacy shim replacement. Preserve current bounded GPACK streams, native durable saves, ordered VNM startup, audio pending-reference safety and improved diagnostics. Implement a build-time passive-watermark profile with direct game input and full-overlay mode remaining the default.
**Tech Stack:** Existing Rust build planner/CLI, Kotlin WebActivity and JVM tests, JavaScript VNM shim and genuine-source Node tests. No new dependencies.
**Spec/evidence:** `2026-10-05-vnm-reference-apk-comparison.md` and reviewed reference reports under `.superpowers/sdd/2026-10-05-vnm-reference-apk/`.

## Baselines and Rulings

- Old immutable APK `D:\02_Development\Projects_2\Backup\TFPC-TLFMB.apk`, SHA256 `7D20E47A8DCFA8873EB58FFCF2CFD8CA2FC5013097D4AC95B109566B2548F16B`, historical exp3b passive-watermark build. All129 extracted members match original.
- Old exact shim source recovered at `.superpowers/sdd/2026-10-05-vnm-reference-apk/recovered_assets/assets/web_shims/runtimepack_web_shims.js`, SHA256 `10B76FB0F78F4DB97F6C69678BF516B5AC30B0E5E5270EFBF074E9FFD38FDA9A`.
- Current v3 APK SHA256 `227CBB029D04C3C2F58F99DEEC0CB6F6FB2322FCE5DD30158C6500A86CCD59E3`. Verify current file location before snapshot; root copy vs dist path is not evidence of a different build.
- Old passive DEX routes MotionEvent directly to game, bypassing hitboxes/feedback/focus routing. User reports rapid taps still complete transisions there. This supports a control experiment, not proven exclusive IPC/cancel root cause.
- Keep single DOM dispatch to document with bubbling and explicit keyCode/which/code getters. Do NOT restore old dualwindow/document dispatch.
- Missing touchcancel is a latent release-safety gap; Mouse.buttonDown resets every frame. No false assertion that it proves rapid-tap freeze.
- Neither old nor current shim guards null soundReferences list elements at SCRIPTS3811. This is a concrete uncovered path after load, not a fix we can blindly recover from old APK.
- Old GPACK raw reader allocates whole assets; DO NOT restore it. Current bounded reader must remain byte-identical.
- Core game scripts are identical across checked old/currentpack members; some scene files differ. Cause unknown. No claim translation/version change or whole-game parity.

## Global Constraints

Only edit isolated `D:\02_Development\Projects_2\runtimepack-vnm-reconstruction`. Baseline export, oldAPK/user extraction/recovery/game/saves/previousAPKs are immutable. No deletion/cleanup/Gitinit/reset/commit/merge/push, installs/downloads, MuMu/ADB/UI/gameplay or automatic app data operations. Build beta only after task reviews and independent verification. No added code comments, thirdparty vendoring, wholegame extraction/decryption or fake playback success.

Before edits, snapshot every touched source/test file plus relevant current asset hashes into fresh `.superpowers/sdd/2026-10-05-vnm-selective-restoration/prepatch/` with no overwrites and manifest. This filesystem-isolated workspace has no Git; use content hashes/diff artifacts, not invented commit IDs. Keep all records; no SDDcleanup.

## Task Boundaries

1. **Input profile and cancel safety:** supervisor detailed input plan; executor implements; code-reviewer gate. Restores optional passive routing with no controls/feedback/focus hooks; full mode unchanged except narrowly proven cancel safety and bounded move feedback. No arbitrary click delay/global debounce.
2. **Audio-pool and old-save reads:** supervisor detailed audio/save plan; fresh executor; code-reviewer gate. Fix null/hole entries preserving live/pending references and valid original playSound behavior; current storage wins, old Base64 lookup read-only with no writes/migration/clear.
3. **Whole integration review + nondeviceQA:** review combined snapshot diff, run actual checks/lint/tests with authentic local game/library sources. No skipped source test counted as pass. Existing vendor lint failure disclosed.
4. **Beta delivery:** Build one primary passive-watermark candidate matching old direct-input mode in fresh `dist_vnm_reference_v4_passive`; full mode remains supported through code/config and regression tests. Do not build extra huge full APK unless separately needed. Preserve all prior outputs. Verify signature/hash/runtime-integrity/package/profile/sourcehash and bounded streaming before user handoff.

## Preflight Conflict Matrix

| Tasks | Shared boundary | Ruling |
|---|---|---|
| Input/audio | runtimepack_web_shims.js | Sequential implementers; audio uses reviewed input/cancel version, no wholesale oldshim replacement. |
| Input/build | overlay_mode manifest/host | One documented field and defaultfull; actual runtime must consume same schema string, passive betaVNMonly. |
| Audio/save | GameStorage/AudioManager | Save fallback changes reads only; audio cleanup changes only transient null list entries, never stored payload. |
| All/build | RuntimeIntegrity | Compute from final staged assets incl passive HTML and new shim, verify actualAPKdigest; no disabling check. |
| Input/input tests | passive/full input | Passive bypass has no delay/hitbox/feedback; full retains controls and streamownership/cancellation. |
| Save/save tests | oldnamespace vs current | Prefer currentnamespace record; decode legacy Base64 only when currentabsent; read fallback never writes caches/native data. |
| All/userQA | oldpassive vs newpassive | Source tests not gameplaypass. User tests same saved chapter, slow/rapidclick transition and voicedload. |

## Acceptance

- Optional passivewatermark beta APK input path equivalent to olddirectdispatch with currentcore improvements retained.
- Normal/full overlay configuration default unchanged; no duplicate DOM key events or raw feedback gesture leak.
- Real playSound with null/hole restored sound pool no longer dereferences null; valid same references and pending decode objects remain intact, legitimate engine errors not swallowed.
- Legacy UID-prefixed native Base64 saved data read compatibly and current records never overwritten. Legacy unprefixed ambiguity explicitly gated/diagnosed rather than mixed crossgame.
- Full nondevice tests/review/artefactverification completed. Current gameplay NOT_TESTED until user tests v4. No promise originalrepo/source1:1 or universal VNM stability.
