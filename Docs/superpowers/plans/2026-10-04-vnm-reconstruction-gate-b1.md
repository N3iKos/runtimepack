# VNM Reconstruction Gate B1: Experimental Detection and Domain Integration

**Author:** Supervisor main agent.
**Goal:** Restore static VNM identification and consistent engine/runtime identity without claiming runtime compatibility or generating an APK.
**Architecture:** Add a separate experimental engine crate implementing existing Detector interface, with engine-neutral registration in CLI and serialization updates. Auditor/runtime bootstrap implementation follows in separately authored Gate B2 plan after review.
**Spec:** `2026-10-04-vnm-reconstruction-roadmap.md`, Gate B. Requirements PRD-001, PRD-002, PRD-017, TECH-CORE-001/003/004.

## Global Constraints

Work only in `D:\02_Development\Projects_2\runtimepack-vnm-reconstruction`. Original source export and game/recovery are immutable. No Git operations, deletions, installs/downloads, APK build, game execution, MuMu/ADB/UI. No code comments. Supervisor authors plans; executor implements only this stage. New dependencies must reuse pinned existing workspace versions. VNM experimental support must be described explicitly, without changing canonical seven-engine release promise.

## Files and Interfaces

- Create `engines/vnmaker/Cargo.toml` and `engines/vnmaker/src/lib.rs` (crate `runtimepack-engine-vnmaker`, following neighboring crate package/dependency conventions).
- Modify root `Cargo.toml`, `Cargo.lock`, `cli/Cargo.toml`, `core/src/domain.rs` and exhaustive enum uses required to compile.
- Modify detector registration in `cli/src/main.rs` and `cli/src/audit_command.rs`.
- Modify existing engine/runtime identity schema enums where their actual serialization contract requires it. Do not rename existing identities.
- Create `tests/integration/vnmaker_detection_test.rs`; register integration target via existing Cargo patterns if necessary.
- Modify `Docs/REFERENCES.md` and experimental detection documentation with source evidence only. No unrelated documentation rewrite.

New interfaces:
```rust
pub struct VnMakerDetector;
impl runtimepack_core::detection::Detector for VnMakerDetector {
    fn id(&self) -> &'static str;
    fn probe(&self, source: &runtimepack_core::ingest::GameSourceIndex)
        -> runtimepack_core::detection::DetectionCandidate;
}
```
Engine variant `EngineId::VisualNovelMaker` serialized `visual-novel-maker`; runtime variant `RuntimeFamily::VnMakerWeb` serialized `vn-maker-web` by existing serde conventions unless an explicit serde rename is required to preserve historical `vnmaker-web`. **Decision:** use explicit serde rename `vnmaker-web` for runtime family, and retain human display `Visual Novel Maker` / `VNM Web`. This is adapter identity, not an upstream engine version. Default mapping `VisualNovelMaker -> VnMakerWeb`.

## Task 1: Failing static detection tests

Build fixture files inside existing temporary-directory harness, not inside game input. Fixtures must supply real temporary source text so detector reads bytes through actual `IndexedFile.real_path`, not hardcoded memory-only mocks.

Test cases:
- `vnm_matching_source_markers_detect_exact`: index includes blocking ENGINE and SCRIPTS script references, ENGINE text `window.gs` and `GameStorage`, SCRIPTS text `vn.Component_MessageBehavior` and `gs.Application`; candidate EXACT with deterministic evidence.
- `vnm_generic_html_is_not_positive`: index only -> NONE.
- `vnm_dummy_signature_and_save_files_are_not_positive`: signature 27 ASCII zero bytes plus settings/globalData vndata -> NONE.
- `vnm_generic_engine_filenames_without_namespaces_are_not_exact`: filenames and arbitrary scripts -> NONE or LOW, never HIGH/EXACT.
- `vnm_missing_game_storage_is_not_exact`: missing supported engine namespace marker fails strict verified profile; do not invent cross-version support.
- `vnm_source_budget_bounded`: oversized script prefix with markers outside declared read budget -> not automatic HIGH/EXACT; document unsupported evidence range.
- `vnm_registry_conflict_remains_ambiguous`: VNM and MV/MZ fixture markers together resolved by existing registry conflict policy, not first-match wins.
- Engine/runtime serde roundtrip and Display/default mapping match chosen identities.

RED command: `cargo test --offline --test vnmaker_detection_test`. If target absent before implementation first introduce tests/target and verify missing type/variant compilation failure, then implement.

## Task 2: Minimal detector implementation

Inspect indexed real file access and existing detector confidence helper before edit. Bounded text reads using `File::take`/Read, not whole asset loading. Set budgets: index/package <=64KiB, ENGINE <=512KiB, SCRIPTS <=16MiB (fresh game method/export occurs deep in script; whole-file bounded 16MiB acceptable static text budget). Missing/unreadable text cannot support confidence. No regex dependency needed for simple exact verified symbols.

Evidence weights for this deliberately narrow legacy profile:
- Verified ENGINE file contains `window.gs` and `GameStorage`: +60.
- Verified SCRIPTS file contains `vn.Component_MessageBehavior` and `gs.Application`: +60.
- Entry index references both `data/ENGINE.js` and `data/SCRIPTS.js`: +80 only if both verified source markers exist. Missing matching index limits the pair to MEDIUM (120), not automatic HIGH/EXACT.
- Package main=index.html optional corroboration only, no additional confidence needed; parse bounded JSON using existing serde_json.

Only full verified marker pair plus matching index qualifies automatic build confidence. Other versions/export layouts stay unknown rather than being silently assumed supported. Do not use settings files or dummy signature as required markers. Sort evidence deterministically before return. Preserve existing candidate conflicts registry semantics.

Add evidence registry entries at next unused Sxx IDs, citing exact fresh game paths/source patterns and existing research/API source URLs already provided in local research. Record that this identifies a supported experimental export profile, not all VNM generations. Do not hardcode game title/UID in detector.

## Task 3: Domain and registration integration

Follow actual serde/type match conventions for EngineId/RuntimeFamily. Find all exhaustive uses; update only identity handling needed for compilation. Runtime selection remains unresolved until Gate B2 runtime manifest/analyzer/auditor are implemented; do not map VNM to RPGMaker profile just to make build succeed. If current resolver requires manifest absent, return existing unresolved runtime diagnostic. Do not introduce permissive stub analyzer or PASS auditor.

Wire `Box::new(VnMakerDetector)` into BOTH `create_default_detector_registry` in main.rs and `create_detector_registry` in audit_command.rs. The existing AnalyzerRegistry fallback produces COMPATIBLE for detected engines without an analyzer; therefore add a narrowly scoped experimental `VnMakerAnalyzer` implementing existing Analyzer trait, explicitly returning UNKNOWN with a stable existing diagnostic explaining pending compatibility implementation. This is not a permissive compatible stub. Test inspect/explain UNKNOWN; no changes to other engines' defaults. Gate B2 replaces this diagnostic analyzer with evidence-based dependency analysis.

Audit for VNM must remain fail-closed INDETERMINATE if no auditor yet; verify actual fallback and add test. Add `vnmaker-web` to `schemas/runtime-manifest.schema.json` family enum and consistent `visual-novel-maker` identities to actual engine schema enums. Do not claim release support.

## Task 4: Verification and review

Run:
- `cargo test --offline --test vnmaker_detection_test`
- `cargo check --workspace --offline`
- `cargo clippy --workspace --all-targets --offline`
- `cargo test --workspace --offline`
- Actual inspect/explain of named fresh game is permitted now because detector is static. Use actual CLI --help first; no build or execution.

Record raw commands/result totals and source-diff package relative to snapshot manifest (non-Git workspace). Write worker report only in `.superpowers/sdd/2026-10-04-vnm-reconstruction/gate-b1-report.md` and changed-file list. Missing tests/failing enum integrations are blockers not success.

Code-reviewer checks detection false positives, budgets, deterministic evidence, identity/schema consistency, conflict policy and no accidental compatible/audit PASS claims. Supervisor authors Gate B2 before additional implementation.
