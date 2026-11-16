# VNM v3 Chapter 4 Stall: Live Read-Only Diagnosis Plan

**Author:** Supervisor main agent.
**Goal:** Preserve and inspect the user's currently stalled MuMu game state before any replay, patch, build or restart.
**Architecture:** QA captures process/log/view evidence and, only where an already enabled debug interface permits it, reads narrowly scoped engine state. Source review maps observed state to the waiting condition afterward.
**Scope:** Observation of running v3 game only, not automated gameplay QA. User explicitly requested sub-agent analysis of the existing state.

## Global Constraints

- Active project: `D:\02_Development\Projects_2\runtimepack-vnm-reconstruction`.
- Target package: `io.runtimepack.game.timeforpunishmentclasstakinglessonsformyboyfriend`.
- Expected local v3 APK SHA-256: `227CBB029D04C3C2F58F99DEEC0CB6F6FB2322FCE5DD30158C6500A86CCD59E3`; verify installed identity rather than assuming versionCode1 proves v3.
- No tap/swipe/key events, overlay opening, auto-clicker control, Skip, save/load, force-stop, launch/relaunch, install/uninstall, clear-data, logcat clear, emulator boot/reboot or setting changes.
- No source changes, patching, APK builds, cleanup/deletion, Git operations, or recovered game execution. Original export, raw game, saves and all artifacts preserved.
- Supervisor authors all plans. Workers only evidence/report; no worker plan/spec writes.
- Do not export save payloads, player variables, credentials, or unrelated app logs to user-facing summary. Private local evidence may contain app paths; keep private.
- Do not conclude audio/focus/overlay causation from screenshot alone. Screenshot shows classroom/background without visible dialog, not proof the entire engine is frozen or chapter number exact.

## Task 1: Live observation — qa-tester

- [ ] List MuMu instances and identify the running target package. If multiple candidate running instances, return ambiguity rather than arbitrary choice. If none running do not boot.
- [ ] Capture raw logcat all/crash buffers promptly with timestamps, PID and renderer association; never filter only current host PID and lose previous exception/renderer events.
- [ ] Capture screenshot, Android activity/window focus, app exit-info, current host PID and WebView renderer PID, installed package metadata and APK identity if available read-only. Sample process liveness twice without input. Do not kill/pause auto-clicker; record if it appears active.
- [ ] Capture app-owned diagnostic report/error snapshot files via permitted read-only app-private access if debug build allows it. Do not open overlay or invoke a share UI to obtain them. Do not alter saves or report state.
- [ ] If an existing WebView debug interface is available, identify game vs overlay targets by URL and obtain read-only state. A localhost ADB forwarding channel is permitted strictly for inspection; don't expose debug ports publicly or enable debugging/settings on device. No DevTools pause/resume/breakpoints/profiling/game commands.
- [ ] Read-only JavaScript evaluation must not call engine mutators, storage, update, play/stop, callbacks, or getters with context side effects. Prefer Runtime.getProperties/accessor descriptors or plain own data property lookup. Limit output size; do not serialize the scene graph or player variables.
- [ ] Candidate state fields from actual source: document visibility/focus, Graphics paused/stopped/frameCount, SceneManager current scene/changing/fade flags, interpreter instruction/wait/loading state, message component visibility/current message length and wait status, pending resource/audio readiness. Do not guess private property names; use source or inspect descriptors first.
- [ ] Compare narrow scalar state snapshots over a short interval: frame counter advancing vs halted; interpreter instruction static vs progressing; resource wait/error indicators. Record evidence query exactness and whether user auto-clicker continues in background.
- [ ] If live JavaScript state unavailable, report limitation. Logs and screenshot alone cannot prove an exact wait instruction.
- [ ] Save raw evidence and diagnosis only under `.superpowers/sdd/2026-10-05-vnm-chapter4-stall/` or approved temporary root after verifying parent. Return facts, uncertainties and exact source/stack candidates; no fix.

## Task 2: Source-backed interpretation — code-reviewer

- [ ] Read Task1 evidence, current shims and fresh game's matching source. Explain observed blocked condition rather than defaulting to old issue.
- [ ] Distinguish: focus pause; unhandled JS error causing RAF chain termination; hidden dialog; waiting audio/resource/load/animation; script logic wait; renderer/process death.
- [ ] Verify v3 pending reference guards preserve asynchronous decode/play/resume intent and whether wait completion can remain pending while rendering runs. Treat this as hypothesis unless state supports it.
- [ ] Return supported root cause or bounded next evidence needed. Do not patch, replay or restart.

## Exit Criteria

Preserved stalled state with raw evidence and an evidence-backed diagnosis or explicit limitation. Any implementation requires a subsequent supervisor-written plan and review/test gates; no automatic patch during observation.
