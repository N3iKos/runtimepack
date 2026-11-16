# Legacy No-Overlay APK Reference: Read-Only Recovery and Behavioral Comparison Plan

> **For agentic workers:** Follow staged subagent-driven execution and review. Supervisor owns this plan. Workers investigate and report, not author implementation plans.

**Author:** Supervisor main agent.
**Goal:** Recover exact readable runtime assets from the user's old APK and identify source-backed behavioral differences from reconstructed v3 before selecting any patch.
**Architecture:** Original APK is the immutable archive authority. Verify the existing extraction against ZIP members, recover selected runtime assets into a new evidence directory, and analyze compiled host methods without installing or running the game. Decompiled source is a behavioral reference, not original Kotlin/Rust recovery.
**Tech Stack:** Existing ZIP/APK tools, pinned local Android SDK tools and available DEX decompilers/disassemblers only; no downloads or new dependencies.
**Spec:** User reports old no-layout APK allows repeated auto-clicker taps while classroom transition still completes. Current v3 transition can stall under repeated taps and chapter5 save-load voice failed at SCRIPTS.js3811 (`r.isPlaying` with null r). This comparison investigates both, without conflating their causes.

## Global Constraints

- Original APK: `D:\02_Development\Projects_2\Backup\TFPC-TLFMB.apk`, READ-ONLY.
- Existing user extraction: `D:\02_Development\Projects_2\Backup\TFPC-TLFMB`, READ-ONLY.
- Current source: `D:\02_Development\Projects_2\runtimepack-vnm-reconstruction`, source unchanged during comparison.
- Current packaged reference: `dist_vnm_stabilized_v3/TimeforPunishmentClass~TakingLessonsforMyBoyfriend~.apk`, SHA256 `227CBB029D04C3C2F58F99DEEC0CB6F6FB2322FCE5DD30158C6500A86CCD59E3`.
- Reports and extracted reference assets allowed only in a fresh `.superpowers/sdd/2026-10-05-vnm-reference-apk/` directory inside current workspace or approved temp root. Verify parent/freshness; never overwrite original backups, extraction, sources or reports.
- No deletion/cleanup, Git initialization/reset/commit/merge/push, APK build/repack/sign/install, MuMu/ADB/UI/app launch, replay, save/load, game bootstrap or recovered code execution.
- No remote uploading of APK, game content, logs, saves or provider configuration. No broad drive scans. No downloads/installation of reverse-engineering tools.
- Preserve game content encryption and container authentication. No DRM bypass/decryption guesses. Container index and plain stored source snippets may be read statically using actual format checks; do not expand the entire game or execute Emscripten preparer.
- Keep save data namespaces/formats intact. Do not assume old no-overlay reader includes later streaming fix.
- User experience is observed evidence: old no-overlay repeated-click transition worked. APK comparison may show other differences too; absence of overlay alone does not prove exclusive causation.
- Supervisor personally writes follow-up patch plan after reviewed findings. No automatic copying into current project.

## Task 1: APK authority and selected source recovery — executor

- [ ] Verify both user paths, current reference APK, archive integrity and original APK size/SHA256. Use existing apksigner/aapt tools for signature, package/version/SDK/ABI, build metadata/flavor/runtime/overlay profile. VersionCode1 is not enough identity.
- [ ] Compare hash against known historical candidates only to identify provenance, not invent commit ancestry: old no-overlay exp3b expected `7D20E47A8DCFA8873EB58FFCF2CFD8CA2FC5013097D4AC95B109566B2548F16B`; old full-overlay streaming expected `bc7cdba43aa26f217ae67e6f178c6bc2e4228b55a6993ed45e6a7422988b78b7`. Mismatch means unknown exact build until embedded evidence.
- [ ] List relevant archive members (web_shims, overlay, build-manifest, diagnostics/profile, dex, libraries), identify whether no-layout means passive watermark or hidden but active full overlay.
- [ ] Verify selected files in existing extraction byte-for-byte against original APK members. Extraction is reference only when matching, not assumed authoritative.
- [ ] Copy selected exact JS/CSS/HTML/manifest runtime assets into fresh evidence-only reference folder with per-file hash/size/original member path. Avoid game.pack copy (large duplicate); inspect header/index by offsets instead.
- [ ] Compare old/current game.pack hashes and index only when practical; inspect plain raw ENGINE/SCRIPTS/boot or generated entry index hashes using bounded reads if supported. Different game pack inventory/content can be a confounder even user confirms same release. Identify original script modifications if any.
- [ ] Return exact provenance, recoverable source assets, differences/confounders, reader streaming known/unknown and local decompiler availability. No application source changes.

## Task 2: Exact JavaScript and startup comparison — executor

- [ ] Read recovered old shim in full and current source relevant contracts, using original archive-authorized files.
- [ ] Compare VNM GameStorage namespace/serialization/native bridge contracts; Buffer/JSZip; native WebAudio state/ownership/WebIDL hooks; voiceDuration and AudioManager.soundReferences/playSound/fromDataBundle behavior.
- [ ] Determine old audio guards and deferredplay/deserialize fixes absent or changed in v3, including whether old APK patches game SCRIPTS itself via VFS or hooks AudioManager. No assumption null list entry always caused by engine alone.
- [ ] Compare synchronous pre-engine/post-SCRIPTS hook interface/ordering and any generated HTML transform in old assets. Distinguish game-owned script bytes from host-injected runtime patches.
- [ ] Inspect passive watermark HTML: no controls/editor/input/focus hooks expected, but verify rather than infer from label.
- [ ] Produce minimal behavioral difference matrix with exact old asset line vs current line, risk to saves/audio/native contracts, and candidates for selective restoration. Do not port any code yet.

## Task 3: DEX host comparison — executor or code-reviewer using existing tools

- [ ] Statically disassemble/decompile only relevant classes/methods: WebActivity dispatchTouchEvent/handlePointerEvent, focus/modal/hide/text callbacks, feedback dispatch, key injection and native save bridge, VfsResourceInterceptor transform/interception, GpackStreamReader raw stream path, RuntimeBuildInfo.
- [ ] Keep original DEX untouched; write derived artifacts only in evidence directory. Record tool/version and decompiler uncertainties; do not equate generated Java with original Kotlin source.
- [ ] Verify old passive mode bypasses overlay hitboxes/focus/feedback and compare current event routing. Track actual DOWN/MOVE/UP/CANCEL delivery, keyboard duplicates, UI thread/focus generation and whether VNM input clears every tap/focus event.
- [ ] Identify concrete double event, lost release or unnecessary focus dispatch differences that could affect rapid-tap transitions. Source candidates require a realistic regression reproduction, not an arbitrary global click delay.
- [ ] Verify old GPACK raw reader: whole-entry ByteArray vs bounded source slice. Preserve current streaming if old predates it, even if old transition worked.
- [ ] Establish contracts for keeping full overlay optional/default as user preference later; don't remove overlay as an automatic final solution.

## Task 4: Review and recommendation — code-reviewer

- [ ] Review archive/extraction hash evidence and exact JS/DEX behavioral differences against current source and fresh game scripts.
- [ ] Separate recovered exact source, inferred decompilation behavior, confirmed code defect, and unconfirmed causal link to observed chapter4/chapter5.
- [ ] Rank restoration targets: usable old shim source, proven input/focus mismatch, audio restore/list handling, startup, reader memory safety.
- [ ] Recommend a narrow follow-up implementation plan scope and acceptance: same saved chapter/scene slow+rapid input, normalvoice/load, full-overlay modal/hide, no save or streaming regression. No automatic patch/build.

## Exit Criteria

Original APK/hash preserved, usable exact runtime assets recovered privately, no-overlay implementation and DEX contracts identified, difference matrix reviewed, and selective recovery recommendation ready for supervisor-written implementation plan. Current v3 remains unchanged; no gameplay or stability claim from static comparison alone.
