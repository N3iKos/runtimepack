# VNM v2 Audio/Focus Stabilization Implementation Plan

> **For agentic workers:** Use subagent-driven-development task gates; supervisor owns this plan. Steps are tracked in the session ledger. Do not create or revise implementation plans.

**Author:** Supervisor main agent.
**Goal:** Fix authentic pending-audio reference exceptions in blur pause, focus resume, and scene stop while preserving future decode completion, valid playback, shared buffers, and full overlay.
**Architecture:** Wrap writable engine methods and install per-instance delegated accessors at the engine's audio registration seam. Leave native non-configurable prototype descriptors and actual audioBuffer identity unchanged. Overlay controls remain non-focus-stealing except genuine text input; normal Android lifecycle remains intact.
**Tech Stack:** Existing JavaScript shims/Node VM, Kotlin WebActivity/JVM tests, no new dependency.
**Spec:** User-approved stabilization roadmap and `2026-10-05-vnm-v2-stabilization-phase0.md`, Task A plus reviewed source diagnosis.

## Global Constraints

- Only `D:\02_Development\Projects_2\runtimepack-vnm-reconstruction` edits. Original export/recovery/fresh game/saves immutable. Preserve v1/v2 outputs and user Audit-app.txt.
- No deletes/cleanup/reset/Git/init/commit/merge/push, installs/downloads, MuMu/ADB/UI/gameplay, or APK build before both patch reviews.
- No added code comments. No blanket catch-and-continue, fake healthy audioBuffer sentinel, library-wide false success, disposal of pending references, removing arrays wholesale, voice disable, or overlay removal.
- Verify current source copied byte-for-byte into plan ledger prepatch snapshot before edit (only files touched plus critical game hashes); do not overwrite existing snapshot if collision.
- Record original native descriptors and preserve `this`, arguments and return semantics for valid paths.
- Tests execute isolated authentic ENGINE class expressions and blur/focus handlers only, not whole ENGINE/SCRIPTS boot. Root discovery/environment input required; absence must be explicit NOT_TESTED, never silently replaced by synthetic engine code.

## Verified Root Cause and Seam

ENGINE hash `928372395b6e82020d693653468aa1a02e1326a924dcfac7b7c6d0739b107885`.
- Reference isPlaying reads `this.audioBuffer.error` at col49411.
- Reference.stop reads same at52688.
- Audio.pause sets customData.wasPlaying by reading isPlaying before its try/catch at47214/47228 (not147228).
- Blur sets Graphics.paused=true then calls Audio.pause; focus sets paused=false then Audio.resume. Failures skip later input/focus callbacks.
- fromDataBundle creates `new AudioBufferReference(null, ...)` and registers it while waiting for finishDecode. Pending null is legitimate; actual failure cause/timing of decode on device still unknown.
- Prototype isPlaying/volume/playbackRate/context/duration accessors are NON-CONFIGURABLE. Do not attempt redefining them. Instance own non-enumerable delegated accessor can legally shadow inherited descriptor. Reference.audioBuffer must remain actual null until real decode assignment, not transformed to fake buffer.
- Audio singleton `addAudioBuffer(ref)` is called during construction before initialize. Wrap it idempotently to install instance-safe descriptors for only recognized VNM audio instances; retrofit already registered objects. GS and gs aliases can reference same objects; avoid double wrapping.

## Files

Modify existing `runtimepack_web_shims.js`, `WebActivity.kt`, optional minimal `vpad.js` focus/cancel calls only if source-proven, existing `vnm_audio_lifecycle.test.js`, `vnm_overlay_focus_lifecycle.test.js`, `vnm_startup_lifecycle.test.js`, `WebActivityTest.kt`, and relevant capability/evidence registry entries. Don't touch native save repository or GpackStreamReader behavior. Add a reusable test extractor/helper only if existing test conventions need one; no copied proprietary full library fixture.

## Task 1: Authentic RED tests and descriptor constraints

- [ ] Extract exact Audio manager and Reference class IIFEs using structural anchors and bounded source, preserve property descriptor flags and constructor registration. Stub only external dependencies (AudioContext/native source/event emitter) with faithful descriptor semantics.
- [ ] Reproduce both supplied errors with a registered null-buffer reference: authentic Audio.pause triggers49411; authentic reference.stop via isolated authentic stopAllSounds triggers52688.
- [ ] Reproduce focus Audio.resume when wasPlaying flag is stale with null reference. Verify state before/after real extracted blur/focus closures, input clear counts and frame paused gate.
- [ ] Assert prototype accessor descriptors remain non-configurable; prototype redefining must not be implementation strategy.
- [ ] Add pending -> finishDecode/real buffer assignment -> initialize -> valid playback test. Verify reference identity stays in manager and audioBuffer remains null while pending, then identical actual buffer after completion.
- [ ] Add two references sharing buffer, disposal of one not dispose other/shared buffer, queued stale callback/disposed pending completion no resurrection.

Run JS suite before production fix and record specific RED failures, not only missing exportedhelper assertion.

## Task 2: Narrow lifecycle implementation

- [ ] Install non-enumerable per-instance `isPlaying` accessor for recognized Reference/Stream instances through `gs.Audio.addAudioBuffer`; capture original prototype descriptor and call its getter on valid state. Pending reference (`audioBuffer == null`) returns false; disposed source/stream returns false. Do not shadow actual audioBuffer or substitute a fake error object. For unsupported own non-configurable descriptor, explicit diagnostic and no silent overwrite.
- [ ] Wrap Reference.stop/play/resume/initialize only for demonstrated invalid state: pending/disposed or required source unavailable for playback calls yields safe inactive operation with bounded diagnostic; valid state delegates unchanged. Initialize must permit genuine decoded buffer with source initially null, not mistakenly block source creation. Do not catch unrelated original errors.
- [ ] Inspect original volume/playbackRate null dereferences and serialization. If pending access requires guarding, use own non-enumerable delegated get/set with sidecar pending values based on actual units; preserve queued setter values for apply AFTER real initialize, do not pretend playbackRate=1 or alter active values. Pending getters have explicit documented inactive values matching actual engine error branch (volume100, playbackRate undefined) unless queued value exists. Other getters such as duration already null-safe remain unmodified unless source-proven defect.
- [ ] Sidecar state WeakMap where supported (existing JS level) or non-enumerable field; no enumerable shim state entering serialization. Only registered objects of known engine class; not arbitrary third-party buffers.
- [ ] Preserve existing native stop-before-start and native onended WebIDL delegation. Source ownership attached for initialize/play/resume-created nodes, not only play. Dispose invalidates before original cleanup; late decode initialize does not restart a disposed reference.
- [ ] Because safe isPlaying eliminates the pause getter exception, prefer delegating Audio.pause/resume unchanged rather than recreating manager loops. Any required extra manager guard must be narrow and have authentic test evidence; no catch-all.
- [ ] Ensure synchronous post-ENGINE/post-SCRIPTS hook and fallback installer work idempotently. Patch pre-existing reference collection too, avoiding new array replacement.

## Task 3: Focus policy and valid modal lifecycle

- [ ] Overlay WebView initialized `isFocusable=false`, `isFocusableInTouchMode=false` to prevent eye/toolbar/button touches stealing game focus. Touch handling remains interactive; no controls removal.
- [ ] Modal state still consumes overlay touch and releases keys/captured gesture as required, but opening a non-text panel does not call overlay.requestFocus merely to show it.
- [ ] Genuine overlay text-input focus enables overlay focusability on UI thread; exit disables it and schedules game focus restoration respecting foreground/modal/text state and monotonic generation.
- [ ] Done closes modal and restores actual game focus safely. Hide/show both maintain/restore focus without injecting Space, release stuck keys/pointers without double event streams.
- [ ] Real activity/system blur remains honored; no faking private Graphics callback, permanent paused=false, hidden document override, or running game while app background.
- [ ] UI thread wrappers for every WebView focus/evaluate operation touched. Foreground resume focus restoration needs explicit eligibility checks, not force focus into an active text editor/modal.

## Task 4: GREEN, coverage and review

- [ ] Authentic blur with pending and active references completes Audio.pause, Input.clear and onFocusLost; later actual focus completes resume, clear and onFocusReceive. Pending ref survives and can later decode/play.
- [ ] StopAllSounds completes with pending ref while active ref is correctly stopped; unrelated injected native errors propagate per original manager behavior.
- [ ] Kotlin tests exercise production focus methods and queued runnables, actual default focus configuration test seam; no test-defined duplicate state-machine closures. Cover editor open/Done, eye hide/show, text enter/exit, background/foreground, repeated stale tokens, control held during hide.
- [ ] Run existing/new JS suites with explicit VNM_GAME_ROOT (genuine library/source no skip), node syntax, offline Kotlincompile/JVM/lint. Rustcheck/clippy/workspace to protect combined baseline. Lint failures disclosed.
- [ ] Write worker report/diff/redgreen evidence under `.superpowers/sdd/2026-10-05-vnm-v2-stabilization/audio-focus-report.md` and update ledger. Supervisor reviews plan alignment; code-reviewer approves before diagnostic task starts shared edits.

## Completion Boundary

Non-device reference/focus contract fixes verified; save/streaming/full-overlay features preserved. Real audio cadence, device focus/rendering and chapter2 progression remain user QA; no stability claim from tests alone.
