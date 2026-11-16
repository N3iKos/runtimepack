# VNM Reconstruction Gate B2: Honest Audit and Pinned Experimental Runtime

**Author:** Supervisor main agent.
**Goal:** Restore truthful VNM audit and build-time adapter identity, while keeping APK builds blocked until runtime capability gates C–F are reconstructed.
**Architecture:** VnMakerAssetAuditor and VnMakerAnalyzer live in engines/vnmaker; CLI uses existing registries; generated WebActivity routing is explicit. No game code execution or runtime shim implementation in this stage.
**Spec:** reconstruction-roadmap Gate B; B1 completed with reviewer approval.

## Global Constraints

Only modify isolated runtimepack-vnm-reconstruction. Original export/recovery/game immutable. No Git init/commit/merge, deletes/cleanup, installs/downloads, APK build, device/emulator/ADB. No code comments. Supervisor owns plans. Reuse existing pinned dependencies. Explicit experimental profile is not canonical v1 compatibility claim.

## Files / Interfaces

- Modify `engines/vnmaker/src/lib.rs` (detector tightening and analyzer integration).
- Create `engines/vnmaker/src/asset_audit.rs`, export `VnMakerAssetAuditor` implementing existing AssetAuditor interface.
- Create `tests/integration/vnmaker_asset_audit_test.rs`, register via cli/Cargo.toml.
- Modify CLI audit registries in main.rs/audit_command.rs using current public registration API.
- Add `runtimes/manifests/vnmaker-web.toml` and adapter notice location using runtime manifest/notice conventions.
- Modify exact runtime-family routing and mappings in `core/src/build_plan/generator.rs`, domain/schema inconsistencies only as needed.
- Modify evidence registry with concrete local source references, audit cache ruleset/version declaration where necessary.
- Extend current VNM detector tests; add targeted build-generator fixture tests using existing manifest construction patterns, not full APK.

Produces:
```rust
pub struct VnMakerAssetAuditor;
impl runtimepack_core::asset_audit::AssetAuditor for VnMakerAssetAuditor {
    fn id(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn supports(&self, engine: EngineId) -> bool;
    fn audit(&self, source: &GameSourceIndex, engine: &EngineDescriptor,
             options: &AssetAuditOptions) -> AssetAuditReport;
}
```
Exact imports follow core interfaces. Audit ID `AUD-VNM`, version `0.1.0` for this reconstruction profile. Runtime ID `vnmaker-web`, adapter version `0.1.0`, family `vnmaker-web`, engines `["visual-novel-maker"]`, runtime_api=1, SDK26/34. Native ABIs reflect actual template arm64 availability, not unsupported x86 promise. This is RuntimePack adapter version, not upstream VNM version.

## Task 1: Detection precision and negative regression

Previous marker pair budgets remain. Entry confidence requires actual script src attributes outside HTML comments, not arbitrary text containing paths. Implement bounded parser using existing libraries or small explicit supported-tag scanner without new dependency. Supported forms: quoted src with whitespace around equals, optional `./` prefix, case-insensitive tag/attribute, source path normalized `/`; strip HTML comments first. Do not parse arbitrary inline JavaScript as script src.

Tests: commented fake tags, text-only references, valid single/double quotes and `./` prefixes, missing index marker pair MEDIUM. Package main optional `./index.html` corroboration. Do not accept async/defer profile as guaranteed deterministic startup; detector can identify engine but capture unsupported bootstrap layout for later startup gate.

## Task 2: Audit red tests and conservative profile

Read existing auditor node/finding/completeness constructors and fresh index/manifest prefixes. Tests must cover:
- Manifest opaque/encoded JS (`RESOURCES.json.js` and `SUMMARIES.json.js`) => INDETERMINATE with CMP-AUD encrypted/unknown-format diagnostic from actual catalog; incomplete reference scope; no orphan inference.
- File inventory does not grant DECODE_SUCCESS/ENGINE_LOAD_SUCCESS/RUNTIME_USE_SUCCESS.
- Missing explicit script referenced by index => required FAIL; missing dynamic opaque manifest resource cannot be guessed.
- External URL requests => explicit network policy handling using current audit helpers.
- Root/path traversal requests fail safely; bounded reads and audit limits enforced.
- Plain manifests that have no implemented/verified schema => still INDETERMINATE, not pretend complete parser. Restore only parse shapes proven by fresh source or authoritative docs. This gate can deliver honest bootstrap+inventory scope without comprehensive plaintext manifest support.
- Deterministic sorted graph/findings and same options repeated JSON identical.
- Empty/generic source not misleading PASS.
- Wrong auditor engine supports false, all canonical auditors unaffected.
- Audit override: unknown requires accept-audit-unknown; accept-audit-fail does not bypass unknown; actual corrupt/missing bootstrap FAIL remains distinct.

Implement inventory nodes with real logical paths and byte sizes through existing helper, bounded bootstrap resource dependency edges from exact HTML src/href and known ENGINE/SCRIPTS files. Record covered bootstrap references separately from uncovered scene/manifest/dynamic references. Always set required_scope_complete=false until verified dependency coverage is actually implemented; no confidence percentages. Preserve actual evidence stage hierarchy.

Asset inventory may include encrypted game assets whose byte format is unknown; no decryption or decode executed. Cache ruleset/version must invalidate prior profile results; test engine/option/version isolation using existing cache tests where needed.

## Task 3: Analyzer dependency evidence without premature compatible state

Inspect fresh package and neighboring scanner allowlists; dependency code should live VNM adapter, not core. Report .node as unsupported native; known desktop wrapper DLLs only excluded from Android payload if actual packing filtering handles them, no claim arbitrary DLL support. Unknown native DLL/so remains issue, rather than silently ignoring. Do not classify save files/executable wrapper presence as engine evidence.

Keep compatibility UNKNOWN with a pending runtime reconstruction diagnostic through C–F. Required capabilities describe intended profile (web local origin, JS/WebGL/WebAudio, VFS reads, native saves); do not say implemented shim when absent. Gates C–F supervisor explicitly remove pending blocker only after acceptance.

## Task 4: Pinned manifest and explicit generator routing

Copy neighboring runtime manifest schema structure but verify actual parser field nesting: capabilities must belong to intended schema section rather than accidentally artifacts. Distinct experimental adapter license notice states runtime host implementation licensing provenance only, not rights to user game or entire VNM runtime. No additional thirdparty code.

Explicit generator family `VNM Web`/`vnmaker-web` maps to WebActivity; no wildcard BootstrapActivity fallback for selected VNM adapter. Test generated AndroidManifest and build-manifest identity from synthetic fixture. Runtime registry selection finds this manifest but real CLI build remains blocked by analyzer pending-runtime issue and/or audit unknown; test no output generation side effect before policy rejection.

## Task 5: Verification and handoff

Write report/diff metadata in `.superpowers/sdd/2026-10-04-vnm-reconstruction/gate-b2-report.md`. Do not edit this plan. Run targeted detection/audit/build policy tests and cargo check/clippy/all workspace tests offline. Verify CLI inspect/explain/audit fresh named game statically, output reports only under workspace ledger. Verify game critical hashes before/after.

Reviewer checks precise detection, incomplete audit honestly represented, no generated-playability claims, runtime registry and schema identity, hard pending capability gate/no APK side effects. Supervisor next authors C/D detailed plans after actual native/WebAudio interfaces inspected.
