# VNM Reconstruction Gate C–E: Runtime Contracts and Ordered Startup

**Author:** Supervisor main agent.
**Goal:** Restore VNM runtime environment, native storage, audio/voice lifecycle, deterministic startup and full-overlay interaction without changing original game content.
**Architecture:** Engine-profile-scoped JavaScript adapter integrated with existing WebActivity/VFS and native repository. Use observed fresh game contracts and existing host bridges; no global generic emulator redesign.
**Spec:** reconstruction-roadmap Gates C/D/E. B2 completed; RT-101 pending runtime blocker remains until Gate F and final verification.

## Constraints

Only isolated runtimepack-vnm-reconstruction. No original/recovery/game writes, Git, deletes, cleanup, installs/downloads, MuMu/ADB/UI, APK build. User owns gameplay QA. No new code comments. Supervisor authors plans; workers may report discrepancies but not rewrite requirements. Existing pinned libraries only. Preserve canonical MV/MZ/Tyrano behavior through regressions.

## Files / Responsibilities

- `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`: binary Buffer/Window contract, scoped VNM storage/audio/voice hooks and bounded polling fallback.
- `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`: actual native save repository bridge, UI-affine JS, exclusive pointer ownership/focus lifecycle and configured profile startup.
- `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`: scoped synchronous served-HTML startup transform.
- `android/shell/app/src/main/assets/overlay/vpad.js`: only source-proven release/cancel/visibility bridge changes; preserve visuals/control layout.
- Native repository/bridge Kotlin only if actual missing contract requires it; current `WebBridge(saveSlot/loadSlot/slotExists/deleteSlot/listSlots)` already exists.
- Tests create six necessary JS suites: vnm_save_storage, jszip_buffer_regression, vnm_window_adapter, vnm_audio_lifecycle, vnm_overlay_focus_lifecycle, vnm_startup_lifecycle. Add Kotlin WebActivityTest if absent and extend VfsResourceInterceptorTest/NativeSaveRepositoryTest.
- `Docs/REFERENCES.md`, `Docs/superpowers/compat/COMPAT-REGISTRY.md`, compatibility-db entry follow actual conventions and next-free IDs. Preserve historical CAP-WEB-020/021 where no collisions.
- Optional diagnostic passive mode implementation deferred: full overlay restoration is current scope, no extra CLI mode needed to achieve acceptance. Do not invent unsupported flags.

## Source Evidence Required Before Each Task

Worker must inspect full neighboring imports and actual fresh game relevant source, not execute its bootstrap. Return unknown fields rather than using arbitrary `this.disposed`, fake callbacks or guessed durations. Fresh SCRIPTS line11501 voice method accesses source.buffer; line11764 exports vn.Component_MessageBehavior; final gs.Application.onReady starts game. ENGINE gs.GameStorage selects filesystem via isOffline/nonSteamOS/window.nw. Reference disposal nulls source but not shared decoded buffer. Source-node onended may be native WebIDL setter. All external/API evidence recorded with current Sxx in REFERENCES.

## Task C1: Binary Buffer and window adapter

RED tests first:
- Evaluate bundled actual JSZip in isolated VM, with current shim. ZIP roundtrip including requested nodebuffer yields Buffer.isBuffer true and byte equality, with callable/new Buffer, Uint8Array inputs, alloc/from/toString/slice required by actual JSZip.
- Test UTF8/base64/hex and multibyte payload relevant operations, rejecting unsupported encodings explicitly rather than dummy output.
- Existing generic web shims tests continue passing.
- Window get contract observed caller setResizable/on/fullscreen methods uses state-consistent semantic host mapping; tests actual extracted caller where practical.

Implementation reuse existing polyfill if present/license-pinned; otherwise narrowly implement Uint8Array-backed callable Buffer using existing platform APIs. Do not claim full Node Buffer implementation. Do not expose fs write success silently for VNM.

## Task C2: Durable gs.GameStorage adapter

Test actual isolated ENGINE storage implementation against supplied window.nw to demonstrate old fs no-op routing. Hook singleton GameStorage object (actual gs/window references), not fictional constructor instance. Native bridge namespace uses stable game UID `$PARAMS.uid`, scoped key for every save payload/Header/settings/globalData, string payload semantics identical to original.

Bridge contract from WebBackend.kt:
`saveSlot(namespace, slot, payload): Boolean`, `loadSlot(namespace, slot): String?`, `slotExists`, `deleteSlot`: Boolean, `listSlots(namespace): JSON string`.

Tests simulate repository persisting across separate VM sessions: slots1/2/3+Headers+global/settings UTF8 roundtrip and removal/exists; native false/throw does not update successful cache/local data; absent payload does not become empty successful JSON. Fail loud with stable code. No automatic read migration deleting existing data. Bridge/repository creation must use app filesDir and preserve existing MV/MZ namespaces.

## Task D1: Audio lifecycle and WebIDL ownership

Inspect extracted real ENGINE classes and audio callbacks; tests must exercise actual snippets/library methods with realistic native descriptor behavior.
- stop before start guarded only for established invalid state; normal started stop unchanged and native unrelated exceptions surface.
- Callback assigned through native onended setter must be registered via original setter. Wrap with owner/generation check; stale disposed callback cannot reconnect null destination or resurrect old loop. Valid natural loop callback still runs.
- Node/context ownership handled from actual sources, no enforced singleton context or fake connect success.
- Reference lifetime independent of shared buffer; callbacks from disposed reference release must not dispose every reference.

Record RED for known start/stop and onended disposed reproduction, then GREEN with realistic setter spy; test scope does not patch non-VNM games unexpectedly. No generic catch-all swallowing.

## Task D2: CAP-WEB-020 guarded delegation

Expose narrow hook `window.__runtimepackPollVnmMessageCompat` (historical public adapter hook) invoking installer. Validate `vn.Component_MessageBehavior.prototype.voiceDuration` is function before marker; idempotent after successful wrap.

Behavior:
- `tempSettings.skip` =>1 before null access (matches original final return semantics).
- If actual original settings voice gate active and voice exists but source/buffer absent =>0 with one-shot capability diagnostic; explicitly DEGRADED timing fallback not proven equivalent playback.
- Otherwise call captured original with same this/arguments; unrelated settings/errors preserved.
- Non-finite numeric result may map0 with separate bounded diagnostic; finite valid math unmodified; no playbackRate=1 fallback, no unproven decodedBuffer duration shortcut.

Tests actual extracted authentic voiceDuration body with settings gate on/off, no voice, valid rate/duration exact frames, source/buffer null, skip, unrelated errors, late prototype method assignment, idempotence, non-VNM scope and real reference disposal integration. Mocks cannot replace actual method behavior.

## Task E1: Synchronous two-boundary served HTML

Inspect actual VFS URL mapping and backend family configuration before changes. Scope identified VNM profile via build-manifest engine/runtime, never apply to generic game by bare file extension.

Boundary1: synchronous external shim tag served at correct local origin BEFORE ENGINE and library consumers (before first library script appropriate). Boundary2: narrow inline or dedicated script hook immediately AFTER blocking SCRIPTS tag, BEFORE DOM ready begins Main.start. Existing onPageFinished fallback may remain but not sole installation. No onReady property trap. Bounded polling fallback with explicit timeout diagnostically reports missing install rather than unbounded timer.

VFS transforms in generated overlay/servedresponse only, input file immutable. Handle origin root and index path consistently, optional./ quoted tags, duplicate injection marker; unsupported async/defer/missing expected anchors explicit failure diagnostic not silently boot unprotected.

Kotlin tests execute real transform method for exact served bytes/order/MIME/root/duplicate/missing cases. JS tests lifecycle BEFOREENGINE globals and AFTERSCRIPTS patch before simulated ready using actual extracted voice body; no duplicate JavaScript implementation of Kotlin transform.

## Task E2: Full overlay ownership and focus

Preserve toolbar, themes, controls, editor, diagnostics, touch feedback, watermark. No visual redesign or permanent feature removal.
- Pointer beginning on overlay control remains owned overlay through MOVE/UP/CANCEL; feedback forwarded separately without whole extra game gesture.
- Game-owned pointer returns to game; control bounds changes midgesture cannot steal stream.
- Release latched keys/pointers on genuine cancel/editor/focus/background transitions.
- JS evaluateJavascript dispatched on UI thread; monotonic focus generation prevents stale deferred focus after later modal open.
- Modal focus enters overlay when needed and restores game on Done only when foreground/no modal/text input. Do not assume direct private Graphics callback or forcefocus every tick.
- Hide/show controls is not same as in-game Space hide; do not inject key32 for eye toggle unless actual intended control contract says so.

Tests production Kotlin dispatchTouchEvent with hitbox coordinates, multitouch/cancel and mock transparent production focus path; Node release/visibility/editor tests. Static tests prove contract only, not real render/Live2D stability.

## Verification / Review

Worker completes sequential C/D/E tasks in one bounded integration package, keeps task results separated in report; no parallel competing workers. Run all existing/new JS suites, nodecheck changed files, offline Kotlin compile/JVM tests and lint; record actual failures incl baseline SDL2. Rust check/clippy if generator changed, all Rust tests after any configuration change. Save report and complete changed-file diff vs source snapshot in `.superpowers/sdd/2026-10-04-vnm-reconstruction/gate-cde-report.md`.

Code-reviewer validates spec and quality, real-library/native setter/startup tests, non-VNM regression and no success hiding. RT-101 pending build gate still present. Supervisor next authors Gate F streaming plan; no APK until F/final review.
