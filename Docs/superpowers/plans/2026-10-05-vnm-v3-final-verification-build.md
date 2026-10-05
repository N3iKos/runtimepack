# VNM Stabilized v3: Final Non-Device QA and Beta Delivery Plan

**Author:** Supervisor main agent.
**Goal:** Deliver a reviewed beta candidate addressing user-reported null.error audio/focus failures and uninformative/misclassified audit export, without claiming device playthrough stability.
**Spec:** Phase0 diagnosis, audio-focus plan/r1/r2, diagnostic plan/r1/r2/r3. Both source review gates approved after focused remediation.

## Constraints

Active workspace only runtimepack-vnm-reconstruction; originalmaster/game/recovery/saves/v1/v2APK/Audit-app.txt immutable. No deletion/cleanup/reset/Git/init/commits, installs/downloads, MuMu/ADB/device/UI/gameplay. No workerplans or newcodecomments. Full beta overlay and diagnostics retained; save namespace/package unchanged, no broad storage permissions or largeHeap workaround. Release NOT_VALIDATED remains outside current task. Build only after independent QA passes source contracts.

## Task QA: Independent verification — qa-tester

1. Read current source and previous review reports, not archived stale diff. Run offline cargo check --workspace, clippy --workspace --all-targets, test --workspace. Capture testresult count from executed output, not --list/manualoldtotal. Identify tests changed/removed compared prepatchsnapshot; no unreviewed regression disguised by smaller totals.
2. Run every current JS suite (all12) with explicit VNM_GAME_ROOT and VNM_JSZIP_PATH to actual fresh same-version game, node --check modified JS. No fake skip counts. Ensure authentic nonconfig audio descriptors, pendingfromDataBundle lifecycle (pause/stop/resume/sharedbuf), full overlay cancellationgeneration/textfocus eligibility, source-backed genuineJSzip.
3. Offline Kotlincompile/JVMalltests and lint. Count XML exacttests/failures. Disclose baseline36 vendorerrors if actual; do not fix via permissions/suppressions.
4. Independently verify diagnostic focused production tests: 7exactexcluded vsmissingresource, enabled/disabled/unknownVNM preparationprofile on magicheaders; stage counters nonempty!=integrity; retained890 after6000success; authoritative currentrun snapshot onrestart +strictsourceengine+limit/omittedmetadata; stalecontrol/filecommit atomicrunlock; MiniJsonbudget; structuredcrossWebView stack and multilingualUTF8exportcap/bridgepath. If test coverage claim missing, report gap not fakePASS.
5. Verify save/GpackStreamReader source hashes match audio-diagnostic prepatch baselines, candidateAPKv2 hash unchanged, originalgame ENGINE/SCRIPTS/index/package critical hashes unchanged. Check original source manifest includes all baseline unaffected except supervisorauthoredplans outside snapshothistory properly recorded.
6. Report `.superpowers/sdd/2026-10-05-vnm-v2-stabilization/non-device-final-verification.md`. No edits source/buildAPK; blockers return supervisor for bounded plan.

## Task Build: executor after QA gate

Verify outputparent and freshness `dist_vnm_stabilized_v3`, actualCLIflags/help. Use pinned existing processJDK17/SDK34; no globalconfiguration. Command:
`cargo run --offline --bin runtimepack -- build "D:\Old-Recvery\Project_2\RPGM-P\Raw-Games\Time for Punishment Class ~Taking Lessons for My Boyfriend~" --flavor beta --accept-audit-unknown --output dist_vnm_stabilized_v3`

Defaultfulloverlay; no nonexistentreuse-pack or overlay-mode flags. Don't run--probe. Preserve allpreviousartifacts; no cleanup. Sourcefixduringbuild requiresreview, generatedprojectedits cannotbypassreviewedtemplate.

Verify actualpackaging-exclusions.json exact7 entries sourcehash matches manifest; vnm-preparation-profile.json positivepreparationenabled verifiedfreshsource and samehash; engineidentity/runtimeroutesVNMWeb; staticauditreportpacked and accessible with realengine/source metadata. Source inventory1976+pack1969 notfake missingdifference; full bitmap/audio gameplaystillNOT_TESTED.

Verify emittedAPK hashsize signatureSdkABI; embedded shim/audiofix ownedaccessors/diagnosticerrorbridge matchreviewedsource; packageddiagnostics exactUTF8cap/errorretention methods identitylive info; generatedhost source/dexcontainsmethods andBoundedEntryStream; runtimeintegrityexpected==recomputeactualAPKscope normalizedsortedpath-null-bytes-null. Do not relyon betaallowmismatch. Ensure maintainednative saveUID unaffected. Write buildreport onlySDDandmemoryisolated no plans.

## Task Artifact QA: qa-tester read-only

FreshindependentAPKsizehash/signatureandactualintegritydigest match expectedbytecode; sourcehashpackagingexclusions/profile and packparityverify; newfullassets correct no fakeenginefamilyrpgmaker; v2preserved; actualhostkeyfocusaudiohooks packaged. No install/launch/ADB/MuMu. Returnapprovedhandoff orblocker.

## User Handoff

Deliver exactv3APKpathhash and affectedchanges plus conciseQAchecklist: editoropenDone +hide/show repeated, normaldialog pastchapter2, voiceend/skip transitions, save-restartload, auditcategorysummaryshareincludesissuesandgameerrors. CurrentgameplayNOT_TESTED untiluserreports. Neveruninstall/clearsaves oninstallsignatureconflict. RecordseparatestaticINDETERMINATE and runtimeunknownnot 'missing0 meansallvalid'.
