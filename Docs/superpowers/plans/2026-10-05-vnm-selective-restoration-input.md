# VNM Selective Restoration Task 1: Passive Direct Input and Cancel Safety

> **For agentic workers:** Follow subagent-driven task review. Supervisor authors requirements; no worker plan edits. No commits or cleanup.

**Author:** Supervisor main agent.
**Goal:** Reintroduce the legacy passive-watermark input profile without regressing full controls, diagnostics backend, bounded streaming or saves.
**Architecture:** Build-time overlay_mode carried consistently through CLI/manifest/host. Passive mode directly dispatches original Android MotionEvent to game with no overlay routing/feedback/focus operations. Full mode remains default and receives narrowly scoped cancel safety.
**Tech Stack:** Existing Rust/Kotlin/JavaScript and regression harnesses.
**Spec:** selective-restoration-roadmap Task1; oldAPK passive dispatch verified via DEX and passive HTML recovered exactly.

## Constraints

Active isolatedworkspace only. Originalbackup/extraction/game/source/saves/previousAPKs unchanged. No deletion/cleanup/Git/install/download/device/MuMuADB/UI/gameplay/APKbuild. No codecomments/workerplans/subagents. Snapshot touchedfiles first nooverwrite in new session prepatch directory. Existing native save/Gpack source hash unchanged. Current valid single DOMdispatch kept; no arbitrary global inputdelay/debounce.

## Files

- CLI `cli/src/main.rs` BuildArgs and buildvalidation/manifestconstruction.
- Core build manifest/generator and only necessary exhaustive fixture initializations: `core/src/build_plan/mod.rs`, generator.rs, tests/integration/build_test.rs, schemas/build-manifest.schema.json if actual filename differs inspect convention first.
- Kotlin BuildManifestLoader.kt and WebActivity.kt, JVMWebActivityTest/manifest tests as required.
- JS runtimepack_web_shims.js narrowcancel listener plus existing vnm_overlay_focus_lifecycle.test.js and genuine input source extraction test helper if needed.
- Source evidence/compatregistry minimal newentry nextfreeID.

## Task1A: Configuration RED/GREEN

Add `--overlay-mode` Clap argument via existing BuildArgs style WITHOUT doc comments (use attribute help). Values `full` and `passive-watermark`; defaultfull. Add same field to BuildManifest with serde defaultfull, Runtime build metadata returned by getRuntimeBuildInfo, Kotlin overlayMode defaultfull for historicalmanifestmissingfield.

Tests:
- CLIdefaultfull preserved for canonicalengine.
- passivewatermark rejected for release or nonVNM BEFORE output creation/packing/compile. Unknownstring failswith stable buildinvalidconfig code fromactualcatalog.
- VNMbeta passive generatedmanifest agrees exactstring +fullydeterministic assets.
- Missing oldmanifestfield choosesfull, notpassive byfilename guess.
- Schemas allowbothprofilevalues but canonicaldefaultremainsfull; runtimeengine/generatorreason documentedexperimental.

## Task1B: Passive assets and host entrypoint

Generate minimal static passiveHTML based on recovered oldindex, with BetaTest/VeldraCommunity and pointer-events:none; JavaScriptdisabled for passiveoverlayWebView, no vpad/editor/feedback/cheat/diagnostic scripts loaded, no overlayfocus listeners/bridge controls. Do not recursively remove copiedoutputdirectories; decide generation branch BEFORE copyingfull assets. Required gamewebshims/nativebridge remainidentical forbothmodes.

RuntimeIntegrityhash must becomputed AFTERfinalpassiveHTML placement/fonts/shims from exactstagedassets; independentoracle projecttest validates actualbytes scope. Defaultfull copies existingfullassets unchanged. Passive requires no theme/cheatprofile runtime assets; no extra broadthemecachework. Host diagnostic/errorcollector remains enabled on GAME WebView regardlessoverlaymode, app-stateerrorreports availablewithoutgameinteraction; passivehasnoon-screen diagnostic controls. Full mode retains existingdiagnosticUI/export.

Host `isPassiveOverlay` set from actualmanifest overlayMode. `dispatchTouchEvent` firstbranch directly returns `dispatchTouchEventToGame(event)` onpassive; no hitboxtest/resolveGesture/handleTouchEvent/feedbackDOM/inputreset/repeatedfocus. Preserve exactevent and returnBoolean. Nativeactivitypause/resume/storage/VFS remain normal; passive-specific no-overlay focusrecovery only if neededactualgameactivityfocusnot everytap.

ProductionJVMtests: hostilehitbox/modals/textflag/oldfocuscallbacks in passive still directDOWN/MOVE/UP/CANCEL/multitouchpointer streams deliveredonce originalevent; no feedback/key/event injection. Full ownershiptestsstillpass. Mockhooks transparent and test entrypoint not localcopyoflogic.

## Task1C: Cancel safety and full feedback limits

Extract genuine Mouse class/update/clear methods from freshENGINE in isolatedtest; nofullgameexecute. Confirm buttonDown reset perframe but pressedbuttons requirecancelclear.

Install scoped idempotent VNM touchcancel handler once onwindowcapture (document/canvas duplication only if justified rather than threecallspergesture). Resolve actual Input.Mouse API at eventtime; clear pressedbuttons/buttonDown/buttonUp and useexistingmouse.clear safely ifmethoddoesnotreplaceframe state incorrectly. No permanent blur/focus clearing everytap, no fabricatedkeyup command or Spacehideinjection. When dispatchingnativeACTION_CANCEL fullmode gameDOM receivescancel; test actual eventbridge mousepressed false aftercancelandnextnormaltap works. Preserve native inputlistenersandotherengines.

Full mode movement feedback bounded <=60 dispatches/s throughlatest-samplecoalescing. DOWN and UP/CANCEL visual feedback terminalevents preserved; no modifications togameMotionEvent cadence orblockingtransitiontimers. QueueholdsatmostonependingMOVEsample; lifecycle/hidecancelclears pendingfeedback toavoidlateoldtrail. Rendereranimationhooks no broadframe/touch timing rewrite. This is overheadreduction notprovenrapidtransitionfix; tests counting productionfeedback scheduling and no eventdrops togame.

Ripple/stroke toggle unresponsive remains separateuserdeferredminor; don't visually redesignorclaimfixedunless codechange explicittestcontract demonstrates samecontrolpath.

## Verification and Review

Run offlinecargo check/clippyalltargets/workspacetests; nodecheckchanged/allJSwithfreshgameenv; offlineKotlincompile/JVM/lint. Capture actualrawcounts and vendorlintdisclosure nofakealllintclean. NoAPKbuildyet.

Write Task1report/reviewdiff in `.superpowers/sdd/2026-10-05-vnm-selective-restoration/task1-input-report.md`; actualprepatchsnapshotmanifestallchanges recordnogit. Code-reviewer spec+quality approval required before Task2 sharedJSchanges.
