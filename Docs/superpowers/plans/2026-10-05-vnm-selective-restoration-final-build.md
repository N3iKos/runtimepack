# VNM Selective Restoration: Final Verification and Passive Beta Build

> **For agentic workers:** Use separate final review, independent non-device QA and artifact verification gates. Supervisor authors this plan. No commits, source input mutation, or cleanup.

**Author:** Supervisor main agent.
**Goal:** Deliver a reference-mode candidate with legacy direct input, null-audio-pool safety and backwards read compatibility, preserving current memory and diagnostic improvements.
**Architecture:** Full overlay remains default configuration. Primary user-test candidate explicitly uses passive-watermark beta for isolated comparison with verified legacy APK. Current gamepack bytes are unchanged; host/shim changes are tested independently.
**Tech Stack:** Existing Rust, Kotlin, Node, pinned JDK17/SDK34/Gradle8.7 local toolchain.
**Spec:** selective-restoration roadmap/input/audio-save/r1 plans, approved Task1/Task2 reviews.

## Global Constraints

Only reconstructionworkspace source edits already approved. Originalmaster/BackupTFPC-TLFMB APK/extraction/freshgame/saves/v3artifact unchanged. No deletion/cleanup/Git initialization/reset/commit/merge/push, installs/downloads, MuMu/ADB/UI/gameplay. Beta only; no --probe. Worker reports, not plans. No code comments, arbitrary click throttling, wholesaleoldshim transplant or reversion to oldwholefileByteArray. Current source has noGit; compare with content snapshots, not inventedcommitids.

## Task 3: Whole-change source review — code-reviewer

- [ ] Compare complete session source diff/prepatchmanifest with current files and reviewed Task1/Task2. Verify passive path matches old DEX contract but full defaultpreserved and betaVNMonly gate consistent allgeneratorentries.
- [ ] Verify actualconfiguredKotlinloader beforeWebViewcreation, staticpassiveHTML JavaScriptdisabled/no bridgecontrols, streamoriginalevents andreturnBoolean once withoutfocusfeedback/hitboxes. Activitylifecycle/nativeerrorcollector stillnormal.
- [ ] Verify fullmovefeedbacklatestcoalescing/terminalcancels/state generation/singleDOMkey dispatch. VNMtouchcancel scoped, no doublecancel/resetlost next input. Rapid-tapfix stillhypothesis untiluserplays.
- [ ] Verify poolcleaning null/hole only, realnewvoicedelegation and pendingreferenceidentity intact; storedsavepayload neverrewritten. Oldsave fallback exactUIDprefixstrictBase64UTF8/currentdominance/readerrorfailclosed and zero write/cache/register onread.
- [ ] Verify fresh NativeSaveRepository default process canread priorUIDrecord validatedcanonicalappprivate roots while unauthorized mutations remainrejected.
- [ ] Verify finalstagedassetIntegrityscope passiveHTML+shims/fonts matchesKotlinalgorithm, actualmetadata `overlay_mode` recorded returnedexport schema.
- [ ] Report errors/deferrals honestly: pendingplay/resume and asyncfromDataBundleplaying state remain explicit unresolvedsource risks; oldAPK did notfix rawplaySoundnull; no allgamecompatclaim.

## Task 4: Independent non-device QA — qa-tester

- [ ] Run cargo check --workspace --offline, cargo clippy --workspace --all-targets --offline, cargo test --workspace --offline. Record rawtest summaries and countfromactualexecutedtests, no stale380/412/413 manualestimates. Check newtests notremovedothers using snapshots.
- [ ] Run all currentJSsuites withVNM_GAME_ROOT namedfreshsameversion andVNM_JSZIP_PATH; syntaxcheck shims/diagnostic/vpad/preview. No source-independentfixture fallback pretending authenticgamepassed.
- [ ] OfflineKotlincompile/JVMalltests andlint; exactXMLcount, disclose36baselineerrorsifactual. Do notaddpermissionstomakelintgreen.
- [ ] Exercise productionpassiveMotionEventbypass andloaderprofile fromsyntheticauthenticmanifest; fullprofilegeneratedasset/shimhash integrity independentoracle; currentrestartUID/readONLY legacyfallback +pendingaudio poolauthenticcases +sourceerrorlogging/probereportexistingregressions.
- [ ] Hash oldAPK unchanged7D20..., v3 unchanged227C..., GpackStreamReader unchangedF706..., freshENGINE928372.../SCRIPTS E8DB.../index/package. Sourceexport snapshotunchanged; no actual appsaveaccess.
- [ ] Save verification report/sessionledger only in `.superpowers/sdd/2026-10-05-vnm-selective-restoration/non-device-final-verification.md`.

## Task 5: Build — executor after both review/QA approved

Freshoutput `dist_vnm_reference_v4_passive`; verifyparent+absence beforecreation. Preserveearlieroutputs. Verify actualCLI --help now exposesmode. Command:
`cargo run --offline --bin runtimepack -- build "D:\Old-Recvery\Project_2\RPGM-P\Raw-Games\Time for Punishment Class ~Taking Lessons for My Boyfriend~" --flavor beta --overlay-mode passive-watermark --accept-audit-unknown --output dist_vnm_reference_v4_passive`

PinnedprocessJDK17/SDK34, localGradlecache; no globalconfiguration/install/download. Don't silently generatedsourcepatch toforcebuild; report failuresforreview. Do not buildsecondfullhugeAPKunlessrequested; defaultfullisregressiontested andavailable via CLI.

Verificationinsidebuild:
- Actualoverlay_mode passive-watermark manifest/host branch consistent, sourceengineVNMruntimevnmakerweb, beta mandatory staticwatermark/no controls/editor/ripple/feedbackscripts loaded.
- Gamewebshims/audio-pool/legacyread/nativeerrorstore retained independentoverlay UI. Backgrounddiagnosticbackendpresent but passivehasnoonscreensharebutton; don'tfake availability.
- GPACK byteparity againstcurrentv3 expectedSHA21079D20... unless documentedactualpack changes; entry1969 rawcompression/encryption flagsunchanged. No mediadecode/gameboot.
- ExpectedRuntimeBuildInfo runtimeintegrity==actualAPKsortednormalizedruntimeassetsdigest, no beta tolerance reliance. Signature/apkzip/hash/SDKABI/packageversion measured; debugsamekeyknownkey ifmatches no guaranteeinstallerbehavior.
- Generated host sourcehash equals reviewedcurrenttemplate andDEXcontainsBoundedEntryStream/passivebranch/loader/nativeReadResolver. VerifyShimarchivebytesexactreviewedsource, no old149KiBblindrollout.

Reportbuild only under SDD/task5-build-report.md. Memoryupdateisolated records gameplayNOT_TESTED +configuredpassive mode, no oldgamepass inherited. No artifactcleanup.

## Task 6: Independent artifact QA — qa-tester

FreshmeasureAPKsize/SHA/signature/profile+runtimeintegrity expectedconstant==actualbytes, gamepack/sharedreader/oldreference/sourcehash invariants. VerifypackagedpassiveHTMLnointeractiveJS fullsourcecodecapabilitypreserved notpretendfullUIinpassive. Verifyappsaveprotocolread0migration acrossunit evidence notactualsave mutation. Report SDD/artifact-verification.md.

## User Acceptance

Deliver exactpassivev4APKpath/hash, explicitly nooverlaycontrols, and remaininglimits. User tests chapter4classroom transition with normal and rapid tap fromsamechapterstate; chapter5loadthenvoiceddialog; save/restartload bothold/current recordwhereavailable; longplay pastpreviousOOMpoint. Compare APKold vscandidate samecore gameversion evidence scene differencesunknown. Don'tclearappdata/uninstall ifupdateconflict. Notpromise1:1source orstableuntiluserconfirms.
