# VNM Selective Restoration Task 2: Audio Pool Null Safety and Legacy Save Reads

> **For agentic workers:** Use test-driven subagent implementation and spec/quality review. Supervisor owns this plan. No worker plan edits, commits, or cleanup.

**Author:** Supervisor main agent.
**Goal:** Fix chapter5 voiced-dialog failure after load and provide non-destructive legacy save reads, while retaining current valid audio playback, pending decode lifecycle and storage write format.
**Architecture:** Small delegated wrappers sanitize transient null/hole audio pool entries before authentic manager methods; current GameStorage reads gain a narrowly validated legacy namespace fallback only when current record is absent. Do not transplant old whole shim or rewrite AudioManager.
**Tech Stack:** Existing JS shims/Node VM authentic source tests; Kotlin manifest loader/JVM tests for Task1 hardening; no dependencies.
**Spec:** selective-restoration-roadmap Task2 and reviewed APK comparison. Task1 input profile reviewed.

## Constraints

Active isolatedworkspace only; originalsource/backupAPK/extraction/recovery/freshgame/saves/previousAPKs immutable. No deletes/cleanup/Git/install/download/MuMuADB/UI/gameplay/APKbuild/newcomments/subagents/workerplans. Add prepatch snapshots for any newly touched file to current session ledger with no overwrite. Existing pending null audioBuffer references and sharedbuffers must NOT be removed or replaced with dummy objects. SingleDOMdispatch/currentGpackstreaming/diagnosticerrorstore/inputTask1 preserved.

## Files

Modify `runtimepack_web_shims.js`; authentic tests in `vnm_audio_lifecycle.test.js` and `vnm_save_storage.test.js` plus startup tests if required. Minimal Kotlin NativeSaveRepository readpath change only if existing get/contains unexpectedly creates directories, with corresponding unit tests; never alter write format. Manifest loader and its existing JVM tests for explicit Task1 hardening below. Evidence/compatregistry minimal entries. No original game files modified.

## Task2A: Authentic manager pool RED/GREEN

Fresh SCRIPTS3811 reads `r.isPlaying` inside playSound; restoreAudioPlayback20903 assigns restored soundReferences. Extract exact manager methods and restore assignment in isolated VM; no fullgameboot. Constructor exported `gs.AudioManager` class, `window.AudioManager` singleton; patch real prototype/singleton idempotently after SCRIPTS before Main.start.

RED tests:
- Restored soundReferences[name]=[null,validInactiveRef] and playVoice/playSound -> exact TypeError beforefix.
- Holes/undefined elements and all-null array followed by ResourceManager actual mockreadybuffer -> creates new reference through originalmethod.
- Valid playing references skipped, existing inactive references reused with same identity/order/rate/volume and exactly one playcall.
- Pending reference object with audioBuffer=null remains in pool; previouslysafe pending accessor/method hooks remain active. Do not prune it as nullentry.
- Genuine errors on malformed non-null object/native playback propagate; no blanket{} skip/false success.
- stopSound and stopAllVoices analogous null-entry paths; stopAllSounds existing nullcheck semantics preserved. Serialization/save payload untouched; transientmanagerarray identity preserved.

Implementation: helper removes ONLY null/undefined/hole entries from relevant arrays in-place, backwards splice or stable compaction. Wrapper determines relevant name consistently with original string/object namehandling (folderPath etc), then delegates captured original with unchangedthis/args/returns. For stopAll methods iterate actual pools but never arrays/objects containing legitimatepending refs; no wholesale resetting manager. Originalenginecreatesbufferreference asusual when no candidate. Bounded diagnostic count for skippednulls with capability ID, no dump save content.

Install hook via existing synchronous postSCRIPTS __runtimepackPollVnmAudioCompat invocation/coordinator, plus existingregisteredmanager handling. No newscript-transformregexpatch to originalplaySoundbody required.

## Task2B: Legacy READ-only save fallback

Originalrecovered getNativeSlotName oldshim2129: `encodeURIComponent(uid ? uid + '_' + candidate : candidate)`, fixednamespace `vnm`, nativepayloadBase64UTF8. Current writes native namespaceuid/rawslot plainJSON remainunchanged.

Required read order:
1. CurrentUID namespace, rawslot from currentbridge. If exists/currentdata empty or corrupt, do NOT silently fall back to differentoldprogress; respectcurrentrecord diagnostic semantics.
2. Only currentrecordABSENT, old `vnm` namespace, `encodeURIComponent(uid+'_'+candidate)` and decode Base64 to UTF8.
3. Unprefixed old `encodeURIComponent(candidate)` is ambiguous crossgame. Do not auto-read it when UID known; permit only if UID actuallyabsent and explicitlegacyprofileproof supports fallback, otherwise returnmissing and diagnosticmanualmigrationneeded. Existing UIDrequiredgame known no unprefixedneeded.
4. Local storage fallbackifnativeunavailable followscurrentexisting behavior; legacy read must not call setItem/cache writes/registerNamespace/saveSlot/deleteSlot/migration helpers. Native reads may use alreadyallowedvnm namespace. No automatic write migration.

Methods getData/getObject/exists agree for legacy-only records; newsetData/setObject continuecurrentUIDwrites. list keys/native delete clear remaincurrentnamespace only, no destructivelegacy operations. StrictBase64 validation/UTF8 decode: malformedrecord logs structured loadfailure and never returns dummy decodedstring or currentpayloadunderwrongencoding. Don't assume any ASCIIstring isBase64. Currentrecord JSONpayload UTF8exact untouched. Readonlycollisioncurrentwins tests Header/settings/globalData andmultilingual.

Tests: prefixoldBase64record loads exactUTF8; currentvalidrecord shadowsold; currentempty/corruptrecord notreplaced; absentboth->nullfalse; getObject parsedfields; UIDmismatchlegacyignored; malformedBase64expliciterror; oldrecordbyte-for-byteunchanged acrossreads; spy0writes0deletes0setItem/registernamespace duringfallback; oldremove/clear notcalled; currentwritesremaincurrentformat. Current NativeSaveRepository.get/contains resolves readpaths without mkdir sideeffects if minimal splitneeded; put maycreateexistingdircorrectly. Test no directories created onmissing legacylookup.

## Task2C: Manifest loader hardening from Task1 review

CurrentMiniJson loader defaults required app/runtime/content toempty objects; restore failclosed requiredstructuralobjects plus essentialid/package/versionfields per actualschema. Old optionaloverlay_mode defaultsfull; optionalmetadata nullable intentionally preserved. Add authenticproductionmanifest test with audit/provenance/nullversion/capabilityarrays from actualexistingv3artifact copied small to testfixture ONLY with no private data; no test depends on hugeAPK/hardcodedDpath. Verify sourcehash/contenthash/flavor/buildtimestamp/runtimefamily/overlayMode exactsemanticparity. No runtime API changes beyondrequiredparsefailures.

## Verification and Review

Use TDDredgreen each task. Run allJSsuites genuineVNM_GAME_ROOT/JSZipenv, nodechecks, offlineKotlincompile/JVM/lint and cargo check/clippy/workspacetests. Record actualrawcounts notold380guess; no claims0lint if vendor36fails.

Write `.superpowers/sdd/2026-10-05-vnm-selective-restoration/task2-audio-save-report.md` plus completepatchdiff against sessionprepatch/currentTask1baseline. Code-reviewer checks narrowlyscopedpoolchanges, oldsavefallback0writes, profilehookinstallationtiming and manifestfailclosed. NoAPKuntilcombinedreview.

## Deferred Scope

Queued pendingplay/resume intent preservation and asyncfromDataBundle savedplaying restoration are sourceidentified risks but need separate lifecycle testdesign; don't silentlybundle speculativeplaycalls into this patch. Record existing limitations in report. No inventedendedevents or interpreterforcedunblock. Full overlayrippletoggle remainsdeferredminor.
