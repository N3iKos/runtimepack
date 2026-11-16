# VNM Reconstruction: Long-Horizon Design and Stage Gates

**Author:** Supervisor main agent. Workers must not author or substitute implementation plans.
**Status:** Phase 0 reviewed; staged reconstruction design. Each coding stage requires a supervisor-authored detailed task plan based on its actual source interfaces before dispatch.
**Goal:** Reconstruct the behavior of the prior full-overlay VNM beta that the user played to completion after raw GPACK streaming.

## Evidence and Baseline

- Working baseline: `D:\02_Development\Projects_2\runtimepack-master`. Verified no `.git`; this is a source export, not a verified Git checkout. Do not initialize Git or invent a commit ID.
- Game: `D:\Old-Recvery\Project_2\RPGM-P\Raw-Games\Time for Punishment Class ~Taking Lessons for My Boyfriend~`. User states newly downloaded same version. Critical inspected script layout matches historical lines; unsampled assets are not certified by this observation.
- Recovery: `D:\Old-Recvery\Project_2\RPGM-P\.worktrees\vnm-exp2`. Sampled source and patches are unreadable binary. Do not execute or import them. Some historical notices/checksums/metadata survived, but these do not restore source.
- Readable research: `Docs/vnm_android_webview_research.md`.
- Readable postmortem: `Docs/research/VNM_PORTING_POSTMORTEM_AND_RECOMMENDATIONS.md`. This describes the earlier paused revision, not the later successful streaming playthrough. Preserve its historical caveats rather than silently rewriting it as final acceptance.
- Historical conversation: CAP-WEB-020 guarded/delegating voice duration; CAP-WEB-021 ordered pre-engine/post-SCRIPTS hooks; bounded raw-entry streams; user completion with full overlay. These are reconstruction requirements, not byte-identical source proof.
- Reviewer corrected audit overclaims: sampled validity is not 100% tree integrity; settings/globalData/signature placeholders are not reliable mandatory detector markers; tool version presence is not a complete successful build proof.

## Safety and Scope

1. Preserve source export baseline, recovery root and game. No delete, reset, cleanup, overwrite-in-place of recovery, commits, merges, pushes, emulator actions, ADB or installations.
2. Isolate implementation in a fresh named sibling workspace `D:\02_Development\Projects_2\runtimepack-vnm-reconstruction`. If it exists, stop and inspect; never overwrite it.
3. Isolation worker copies the baseline tree without following symlinks/reparse points and excludes only proven generated outputs (`target`, local Gradle/app build, dist directories). Include essential source roots omitted by simplistic lists: `pack`, `runtimes`, `profiles`, `probe-profiles`, `compatibility-db`, `.cargo`, `memory`, all root manifests/locks and native libraries.
4. Record relative paths, size and SHA-256 of each included regular file. Verify copied content against manifest before editing. Source input stays external; never copy game into cleanup scope.
5. Full overlay is the final target. Passive-watermark mode is diagnostic only. Preserve mandatory beta watermark and diagnostics in full mode.
6. VNM is explicitly user-approved experimental reconstruction outside canonical seven-engine v1 promise. Core remains trait/registry driven; no scattered game-specific branch hacks.
7. No new dependencies without exact version/license/provenance review. No code comments added.
8. No broad storage permissions, largeHeap workaround, swallowed OOM/empty successful response, DRM bypass, whole-game persistent plaintext extraction or encryption-format redesign.
9. Gameplay QA belongs to user. Build success and synthetic tests do not establish playability.

## Reconstruction Strategy

Prefer verified surviving source where available; otherwise implement behavior from current source contracts, fresh game source and documented evidence. Do not treat reviewer sample snippets as authoritative complete implementations. Source-specific proof takes precedence over generic API docs.

### Gate A: Baseline preservation and isolated workspace

**Worker:** executor. **Review:** code-reviewer.
Deliver content manifest and byte-verified isolation. Run available baseline tests in isolation only, with actual toolchain resolution. Separate preexisting failures from newly introduced failures. No assumption that Gradle, SDK, native libraries or signing identity are complete until checked. No Git operations needed.

### Gate B: Experimental engine vertical slice

**Worker:** executor. **Review:** code-reviewer.
Supervisor detailed plan must inspect detector/analyzer/auditor/resolver traits, domain variants, schemas, Cargo membership, CLI registrations and runtime manifests first.

Required outcomes:
- Deterministic static VNM marker evidence using validated `data/ENGINE.js`, `data/SCRIPTS.js`, namespace/source markers and entrypoint/package structure; generic HTML, dummy `signature`, `.vndata` save files and title alone cannot imply HIGH/EXACT.
- Explicit conflict and unsupported native dependency reporting. No game execution in detection.
- Honest encrypted manifest audit: INDETERMINATE/incomplete, not zero-reference PASS. No unused/orphan inference from unavailable dependency graph. Audit cache ruleset/version updated when semantics change.
- One pinned VNM Web adapter selected during build; schema/report/CLI identities agree.
- Runtime inventory and decode/consumer acceptance kept separate; avoid historical stale-five-file probe claim.

### Gate C: Runtime environment, storage and window contracts

**Worker:** executor. **Review:** code-reviewer.
Supervisor plan reads actual bundled JSZip and fresh ENGINE storage methods before tests/code.
- Callable binary-compatible Uint8Array-backed Buffer with JSZip real-library regression; no plain-object mock or dummy isBuffer.
- Hook actual gs.GameStorage methods, preserving serialization and separating payload/Header/settings/globalData; native durable store errors must surface and not poison cache.
- Use actual native bridge interface/key naming, not fabricated API names. Test multi-sandbox restart through repository plus read/write failure paths. Actual UI slot roundtrip remains user QA.
- Window contract handles observed setResizable/fullscreen methods through safe host mapping; no claim full NW.js emulation.

### Gate D: Audio and voice contracts

**Worker:** executor. **Review:** code-reviewer.
Supervisor plan traces actual fresh ENGINE source/disposal/onended and SCRIPTS voice method.
- Guard start/stop lifecycle without turning unrelated native API errors into success.
- Track source owner/generation against stale callbacks; native onended WebIDL setter delegation must remain real, not a plain property mock.
- Reference.dispose does not imply shared buffer disposal. No arbitrary `disposed` property assumption.
- CAP-WEB-020 wrapper validates target method before installation, delegates valid original paths, preserves settings gate/errors/frame math, early skip=1, and bounded degradation warning for missing source/buffer. No speculative duration fallback from shared decoded buffer or playbackRate=1.
- Tests extract isolated actual method/library behavior; never bootstrap the whole game in static inspection.

### Gate E: Deterministic startup and full overlay

**Worker:** executor. **Review:** code-reviewer.
Supervisor plan verifies fresh HTML script sequence and actual ready/start boundary.
- Synchronous environment shim before ENGINE/library consumers.
- Synchronous narrow voice hook immediately after blocking SCRIPTS before DOM-ready startup. No invasive onReady accessor trap; polling is backup with diagnostic bounds, not sole readiness mechanism.
- Scope VFS HTML transformation to identified VNM, handle index/root paths and reject unsupported startup shapes explicitly. Ensure correct resource URL/MIME and duplicate handling.
- Preserve full controls/editor/themes/touch feedback/diagnostics. Exclusive pointer stream ownership through UP/CANCEL; release virtual states at genuine transitions; UI-affine JS and generation-safe deferred focus.
- Optional passive watermark profile restricted to VNM beta, no hidden controls/listeners/focus recovery; same storage/VFS/activity lifecycle.
- Timing race, Space/eye equivalence, WebGL loss and overlay-specific root causes remain hypotheses absent device evidence.

### Gate F: Shared GPACK bounded raw streams

**Worker:** executor. **Review:** code-reviewer.
Supervisor detailed plan reads full source/producer/crypto and consumer lifecycle before implementation.
- Raw compression=0 encryption=0 entries return bounded positional InputStream; no asset-size ByteArray. Validate nonnegative sizes, offset overflow, raw storedSize=size, unsupported entry flags.
- Serialize source positional reads as required for RAF/channel and base offset. Independent stream.close must not close reader; parent close makes later reads fail.
- Test byte/hash equality, zero-length reads/EOF/skip/available/bounds, truncated source, concurrent distinct streams, APK base offset, virtual huge entry via production openStream without huge allocation.
- Authenticated encrypted path remains whole-chunk and can still be heap-limited. No invented 50MB threshold or unimplemented subchunk encryption.
- Preserve existing HTTP request semantics; do not infer Range support unnecessary from interception API alone.

### Gate G: Non-device verification and beta delivery

**Worker:** qa-tester for non-device commands; executor for build. **Review:** code-reviewer.
- Run actual configured Rust check/clippy/tests; JS syntax/regressions; Kotlin compilation/JVM tests/lint. Record measured counts from raw outputs/XML, not old tallies.
- Existing 36 SDL2 lint errors are historical context only; re-evaluate actual errors and disclose failures without suppressing or adding broad permissions.
- Build one full-overlay beta candidate with exact absolute game path and separately named output. `--accept-audit-unknown` only if current encrypted-manifest INDETERMINATE warrants explicit diagnostic override. No nonexistent reuse-pack feature.
- Verify APK archive, signature, ABI/minSDK, generated source and embedded shim hashes, manifest identity, watermark/diagnostics and packed content. Previous signer/package equality not assumed; never recommend uninstall/clear data to fix update conflict.
- Deliver actual APK path/hash plus user checklist. No assertion gameplay restored before user plays it.

## User Acceptance

Normal voiced/unvoiced dialogue without Skip; choices; natural audio completion/loops and scene skip; settings; slot1/2/3 different-state save/load after restart; overlay editor Done and hide/show >=10 cycles; background/resume; long play past prior OOM point through ending. Outcomes recorded per action/build, with NOT_TESTED where omitted. One-title completion means sufficiently stable for tested GOS-01, not universal VNM compatibility.

## Execution Ledger and Plan Ownership

Supervisor authors each bounded detailed plan BEFORE dispatch. Worker returns implementation report/diff/test evidence, never a replacement plan. No multiple concurrent implementers touching shared foundations. Review precedes subsequent dependent work; decisions and unresolved risks are recorded. This roadmap is not permission for workers to improvise interfaces or unspecified patches.
