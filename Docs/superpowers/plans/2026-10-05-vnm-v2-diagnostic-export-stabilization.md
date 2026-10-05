# VNM v2 Diagnostic Classification and Complete Bug Export Implementation Plan

> **For agentic workers:** Use subagent-driven-development with task review gates. Supervisor owns this plan; workers implement and report only.

**Author:** Supervisor main agent.
**Goal:** Make UI/share/static/runtime reports truthful and preserve all diagnostic failures for the current game without evicting errors behind success logs.
**Architecture:** Host produces authoritative categorized probe reports and independently retained game errors; overlay displays/report-exports snapshots rather than treating console text as authoritative counters. Static graph remains a full source inventory; exact build exclusions are recorded separately.
**Tech Stack:** Existing Kotlin/JSON serializers and Android assets/native bridges, JavaScript overlay tests, Rust build metadata and tests.
**Spec:** reviewed Task B diagnosis in stabilization Phase0; user-approved stabilization plan. Audio/focus patch reviewed and must be preserved.

## Global Constraints

Only isolated reconstruction workspace. Original source/game/recovery, native save payloads, v1/v2 APK/report evidence and streaming behavior unchanged. No deletion/cleanup/Git/installs/downloads/devices/MuMu/ADB/UI/gameplay/APK build until final review. No code comments. Supervisor writes plans. No new cryptography/decryption, screenshot/video capture, secret/provider configuration, raw save/localStorage/player-input export. Do not change counts into PASS by hiding unresolved resources.

## Verified Defects

- Seven excluded NW/DirectX DLLs appear in source graph but not game.pack: d3dcompiler_47.dll, ffmpeg.dll, libEGL.dll, libGLESv2.dll, node.dll, nw.dll, nw_elf.dll. Rust packer should_include_in_pack is actual authority, not arbitrary extension filtering.
- 890 resources/Graphics PNG/JPG payloads require original GS.DataPreparer path; native raw image probe cannot certify decode. Plain icon.png is a separate case. Unknown signature under verified VNM preparation profile => INDETERMINATE, not automatic corrupt/missing or fake decrypted PASS.
- Stage counters currently count attempts as success and nonempty size as integrity. These are false claims.
- WebActivity overwrites engine label with rpgmaker_web. Static summary reads loose disk only, ignoring actual packaged report.
- Console250 entries/export last50 loses failures. Game WebView exceptions never cross to overlay export.

## Files / Responsibilities

- `cli/src/main.rs`: emit exact packaging exclusions metadata using existing pack inclusion policy, and copy lightweight report metadata into generated assets along existing audit file staging code. No packing redesign.
- `tests/integration/build_test.rs` and appropriate CLI audit/report tests: metadata parity/exclusions realpolicy, canonical build defaults unchanged.
- Kotlin `AssetProbeRunner.kt`, `AssetAuditModels.kt`, `AssetAuditService.kt`, `WebActivity.kt`, existing bridge implementation where necessary. Add small `DiagnosticLogStore.kt` only if bounded host error storage cannot fit existing service without conflating probe events.
- Android runtime assets `overlay/diagnostic_suite.js`, `web_shims/runtimepack_web_shims.js` error reporting hooks only; mirror diagnostic_suite to preview if intentional shared template fixture. Android asset is authoritative, not preview-only fix.
- Existing Kotlin AssetProbeRunner/Service/Bridge/WebActivity tests, JS diagnostic suite/shim/overlay tests. Schemas updated for any additive report field, with current schema compatibility explicitly tested.
- Evidence/capability registry matching behavior, no historical QA rewrite.

## Data Contracts (Additive, Existing Fields Preserved)

Runtime per-asset event gains optional `category` and `audit_id` (or equivalent current service run identity). Categories: MISSING_ASSET, FILE_UNREADABLE, DECODE_FAILURE, ENCODED_ASSET_UNVERIFIED, EXCLUDED_PLATFORM_BINARY, VERIFICATION_LIMIT, INTEGRITY_FAILURE. Completion/control events are not asset failures.

RuntimeValidationReport/RuntimeSummaryInfo gains counters `missing_count`, `decode_failed_count`, `indeterminate_count`, `excluded_count`, `attempted_count`, `stage_attempt_counts`; existing `stage_counts` now explicitly counts SUCCESSFUL checks only. Existing `failed_count` totals actual failure outcomes, `passed_count` means declared tested scope only, not gameplay. Counts must reconcile over visited assets, not report whole planned inventory as tested on timeout. Overall precedence FAIL > INDETERMINATE > WARNING > PASS; zero assets/no static report remains explicit NOT_RUN/INDETERMINATE.

Bug report schema2 includes:
- engine identity and separate runtime id/family/version from manifest, no unknown engine-version invented.
- package/version/flavor/build_timestamp, source/pack/runtime-integrity hashes actually available. Do not label content hash APK SHA256 (selfhash insideAPK not generally available).
- combined_validation_summary plus static_report and runtime_report/full issue snapshot including paths/stage/category/outcome/message/sequence/run.
- game_errors snapshot independent of recent console; structured message/stack/file/line/column/kind/timestamp/source; total/stored/omitted/truncated explicit.
- recent_logs existing250 cap explicitly total/stored/omitted; do not silently slice last50.
- no save payloads. User manually shares; no background telemetry/upload.

## Task D1: Exact packaging exclusions and report loading

- [ ] Prepatch snapshot touched source/test/report files no overwrite under session SDD; hashes of game criticalfiles recorded.
- [ ] RED tests source graph contains known desktop DLL excluded by SAME pack predicate and game resources named .dat/.pak remain eligible if predicate includes them. No blanket DLL/exe/media extension skip.
- [ ] Generate `packaging-exclusions.json` (`schema_version=1`, `source_hash`, `entries:[{logical_path,reason}]`) from actual built source index and pack predicate for packed mode. Record only proven omitted paths; reason uses existing recognized platform-exclusion rules. Exact source-minus-packed fixture parity tested. Put lightweight metadata alongside existing audit files in APK assets. Plain content mode cannot assume packed omissions.
- [ ] Probe unions applicable packaged inventory with required bootstrap/dependency paths from graph. Inventory completeness maintained even incomplete graph; excluded metadata gives explicit EXCLUDED outcome/category and does not become Missing. Missing required game file absent index stays failure, not excluded by filename guess.
- [ ] Load static report from actual assets/GPACK/loose fallback via bounded JSON read and verify engine/source identity when available. Malformed report yields explicit unavailable diagnostic rather than fake staticPASS. Reuse existing JSON library; no ad-hoc regex extraction of nested summary that can accidentally match finding outcome.
- [ ] Preserve static INDETERMINATE completeness fields separately from runtime result. No automatic graph pruning claiming scene reference coverage.

## Task D2: Truthful probe checks and classification

- [ ] RED cases: excluded7 does not create missing; encrypted-like VNM graphics =>indeterminate, valid actual-decoded plaintext image =>success; invalid plaintext signature elsewhere remains real decode failure; unreadable/truncated/missing actual resources preserved.
- [ ] Classify known VNM preparation-needed payload using engine/profile and bounded magic inspection (PNG8/JPEG3/GIF6/BMP2/WebP RIFF+WEBP), not if any decode fails assume encrypted. Mismatched signature on resources/Graphics under verified VNM profile is unresolved encoded candidate; INDETERMINATE preserves reason no decryption attempted. Reuse same opened bounded stream with prefix replay so decoder sees original bytes.
- [ ] Remove fake INTEGRITY_VERIFIED from length checks. If no authenticated/hash verification executed, integrity count remains0; record nonempty as readability only. Do not add full-game rehash to interactive probe merely to populate stage.
- [ ] Stage success counts increment only successful checks. Stage attempts separately. For bitmap decoder, remove header-only success fallback: genuine decoder result or explicit inability. JVM tests inject BitmapProbeDecoder mock through existing interface, production must not pretend image magic alone proves decode. Avoid `stream.readBytes()` for large image; use bounded stream/budget and decoder memory bound consistent with current API. Exceeding verifier budget yields INDETERMINATE VERIFICATION_LIMIT, not OOM or PASS. No broad catch Throwable swallowing OOM.
- [ ] Readability check reads bounded prefix and reports only FILE_READABLE. Audio playback/fonts applied/data parse remain untested, not certified by readable.
- [ ] Checkpoint restore must include ruleset/profile/source inventory identity and new category counters; stale old checkpoint invalidates, not reuse based solelycount. Run identity must prevent cancelled prior runner events contaminating a new audit after reset. Do not create unlimited history to fix full error capture.
- [ ] Tests completion/cancel/timeout visitedcounts reconciling counters and no artificial RUNTIME_USE_SUCCESS success stage from DONE event. Preserve legacy report fields while versioning clarified semantics.

## Task D3: Engine identity and retained diagnostic errors

- [ ] WebActivity initializes AssetAuditService from actual manifest engine identity, not rpgmaker_web. Expose structured runtime/build info separately. Canonical engine selectors in existing cheat/diag code must retain their expected behavior; map known aliases deliberately.
- [ ] Add thread-safe host game-error buffer independent of success logs. Max2000 records and4MiB UTF8, per-message/stack cap32KiB; earliest dropped/oversize text explicitly counted. Current game's~897 problem entries fit; export all retained problem records, never last50-only. Run categorized probe issues use persisted report snapshot separately; don't duplicate every success console into errorbuffer.
- [ ] Game error/unhandledrejection hooks call narrow existing/native bridge method (add `postDiagnosticError(json)` and overlay `getDiagnosticErrors()` if needed). Kotlin validates/bounds structured JSON and stores stack/file/line/column/kind without swallowing engine exception. ConsoleMessage errors also retained with source; de-duplicate obvious same error if practical but don't drop distincttimestamps/stacks. No preventDefault global error or fake recovered update loop.
- [ ] Host recentlog ring500 with explicit droppedcount, sanitized/capped metadata. Redact sensitive credential patterns and do not collect save/localStorage payloads or input text; reports contain runtime stack/paths only. Acknowledge arbitrary thirdparty console may be sensitive, avoid exporting all console.info messages from game.
- [ ] Overlay export obtains complete persisted runtime problem events and hosterrors despite success-log flood. Bounded JSON export total cap8MiB with truncation metadata; split or explicit limitation if beyondcap, no hidden truncation. UI/share snapshot sameauditid/outcome/counters. Include all diagnostics/error categories not just outcomeFAIL so890indeterminate shown.
- [ ] Starting new probe does not clear already captured game exceptions; explicituser Clear may clearrecentconsole only unless labelaccurate. Stale audit file from previousbuild must not presented as current.

## Task D4: Overlay summary and export tests

- [ ] UI shows Scanned/Readable/Missing/DecodeFail/Unverified/Excluded distinctly; Valid label if retained explains tested scope. Completion text INDETERMINATE with unverified counts, not misleading Audit Complete missing7.
- [ ] Export retains legacy fields as appropriate plus schema2structured engine/runtime/errors/problem snapshot/logtruncation. StaticHTML0missing remains separate/staticINDETERMINATE not runtimepass.
- [ ] Tests >250+50 successlogs after earlier failures retains all issue paths; >2000 problemcaps accurately omitted flag; matchingrunID counters UI=export; structured actual game error stackcrossWebView bridge roundtrip; reportsourced from assets; canonicalMV/MZdiagaliases; no secret/save exported. Test productionJSexport not helperduplicate.
- [ ] Full nondevice commands: cargo check/clippy/workspacetests offline, nodecheck/all JS with genuine library env, Kotlincompile/JVM/lintactualbaseline. Report exact outputs/XMLcounts; avoid old manualtestcounts.
- [ ] Report/diff `.superpowers/sdd/2026-10-05-vnm-v2-stabilization/diagnostic-report.md` and review. Audio/focus prepatchsource changes preserved; savestorage/Gpackimplementation untouched.

## Build Acceptance After Review

Supervisor authorizes new fresh output dist_vnm_stabilized_v3 only after audio+diagnostics review. Full beta overlay/watermark/diagnostics, explicitunknownauditoverride, exact runtimeintegrity stageddigest=APK digest, gamepack/source no mutating inputs. APK gameplay remains NOT_TESTED pending user chapter2/longplay/save/overlay QA.
