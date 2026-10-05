# Deep Asset Audit Engine Adapters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement bounded static dependency-graph adapters for every RuntimePack v1 engine.

**Architecture:** Each engine crate implements the common `AssetAuditor` contract. Adapters parse only evidenced formats, report unsupported/dynamic input as indeterminate, and publish explicit coverage metadata.

**Tech Stack:** Rust, serde_json, existing ASAR parser, synthetic test fixtures.

**Spec:** `Docs/superpowers/specs/2026-09-24-multi-engine-deep-asset-audit-design.md`

## Global Constraints

- Requires Foundation Plan completion.
- Never execute source code.
- Every reference is required unless an explicit adapter rule marks it optional.
- No external engine-format claim may be implemented without an Sxx entry in `Docs/REFERENCES.md`.
- Tyrano technical results never clear `TYRANO-LEGAL`.

---

### Task 1: Shared Resolver and Coverage Declaration

**Files:**
- Create: `core/src/asset_audit/resolver.rs`
- Modify: `core/src/asset_audit/mod.rs`
- Test: `core/src/asset_audit/resolver.rs`

**Interfaces:**
- Produces: `AssetResolver`, `ResolutionResult::{Exact,CaseMismatch,CaseCollision,Missing,Unsafe}`, `CandidateRule`, `AuditCoverage`.

- [ ] **Step 1: Write resolver tests** for exact path, case mismatch, case collision, traversal, absolute path, Unicode, extension candidates, and deterministic candidate order.
- [ ] **Step 2: Run `cargo test -p runtimepack-core asset_audit::resolver` and verify failure.**
- [ ] **Step 3: Implement resolver against `GameSourceIndex`**, preserving original spelling and refusing global lowercasing.
- [ ] **Step 4: Implement `AuditCoverage`** with covered, partial, unsupported, and dynamic reference classes; unsupported required classes force completeness to indeterminate.
- [ ] **Step 5: Run focused tests and commit.**

```powershell
git add core/src/asset_audit
git commit -m "feat: add deterministic asset resolver"
```

---

### Task 2: RGSS Static Adapter

**Files:**
- Create: `engines/rgss/src/asset_audit.rs`
- Modify: `engines/rgss/src/lib.rs`
- Modify: `Docs/REFERENCES.md` only if required evidence is missing
- Create: `tests/integration/rgss_asset_audit_test.rs`
- Create synthetic fixtures under: `tests/fixtures/rgss_asset_audit/`

**Interfaces:**
- Produces: `RgssAssetAuditor` implementing `AssetAuditor`.

- [ ] **Step 1: Write fixture tests** for XP/VX/VX Ace exact, missing, case mismatch, corrupt Marshal, direct/variable transfer, event-page graphic, move-route SE/graphic, BGM/BGS/ME/SE, animations/timing SE, tileset/autotile/battleback/panorama/fog, System resources, Troop battle events, Common Events, and RTP/archive references.
- [ ] **Step 2: Run `cargo test --test rgss_asset_audit_test` and verify failure.**
- [ ] **Step 3: Implement explicit generation profiles** for `.rxdata`, `.rvdata`, `.rvdata2`; no universal parameter table.
- [ ] **Step 4: Implement bounded map/database traversal** for Actors, Classes, Skills, Items, Weapons, Armors, Enemies, Troops, States, Animations, Tilesets, CommonEvents, System, and MapInfos.
- [ ] **Step 5: Preserve provenance** as map/event/page/command index or database/entity/field.
- [ ] **Step 6: Implement standard/deep split:** standard covers structured data; deep additionally inventories script/archive references only through verified parsers. Unsupported encrypted/archive/script parsing yields `INDETERMINATE`, not PASS.
- [ ] **Step 7: Run focused tests and commit.**

```powershell
git add engines/rgss tests/integration/rgss_asset_audit_test.rs tests/fixtures/rgss_asset_audit Docs/REFERENCES.md
git commit -m "feat: add RGSS asset audit adapter"
```

---

### Task 3: MV/MZ Static Adapter

**Files:**
- Create: `engines/rpgmaker_web/src/asset_audit.rs`
- Modify: `engines/rpgmaker_web/src/lib.rs`
- Create: `tests/integration/web_asset_audit_test.rs`
- Create synthetic fixtures under: `tests/fixtures/web_asset_audit/`

**Interfaces:**
- Produces: `RpgMakerWebAssetAuditor` for distinct MV and MZ profiles.

- [ ] **Step 1: Write tests** for root and `www/` layouts; System, maps, event pages, common/troop events, actor/enemy/animation/tileset resources, BGM/BGS/ME/SE, movies, plugin declarations, static JavaScript literals/imports, encrypted alternatives, case collision, dynamic expression, native `.node`, and external URL.
- [ ] **Step 2: Verify failure with `cargo test --test web_asset_audit_test`.**
- [ ] **Step 3: Implement bounded JSON parsing and normalized content root.**
- [ ] **Step 4: Implement separate MV/MZ command and encryption profiles** using documented signatures; do not merge them into generic HTML.
- [ ] **Step 5: Implement standard/deep split:** standard parses engine databases/manifests; deep parses bounded JS/plugin references and transitive plugin dependencies.
- [ ] **Step 6: Mark required HTTP(S) references `INDETERMINATE` unless network evidence is supplied; never download by default.**
- [ ] **Step 7: Run tests and commit.**

```powershell
git add engines/rpgmaker_web tests/integration/web_asset_audit_test.rs tests/fixtures/web_asset_audit
git commit -m "feat: add MV and MZ asset audit adapters"
```

---

### Task 4: Ren'Py Static Adapter

**Files:**
- Create: `engines/renpy/src/asset_audit.rs`
- Modify: `engines/renpy/src/lib.rs`
- Create: `tests/integration/renpy_asset_audit_test.rs`
- Create synthetic fixtures under: `tests/fixtures/renpy_asset_audit/`
- Modify: `Docs/REFERENCES.md` before implementing any new archive syntax claim

**Interfaces:**
- Produces: `RenPyAssetAuditor`.

- [ ] **Step 1: Write tests** for `.rpy` image/audio/video declarations, script includes, Python imports, dynamic expressions, missing resources, native `.pyd/.dll/.so`, `.rpyc/.rpa` inventory, and unresolved runtime version.
- [ ] **Step 2: Verify failure with `cargo test --test renpy_asset_audit_test`.**
- [ ] **Step 3: Implement bounded `.rpy` tokenizer/parser** for only evidenced declarative forms; retain source line/column.
- [ ] **Step 4: Treat unsupported `.rpyc/.rpa` generations as explicit unsupported coverage** until an evidenced parser exists.
- [ ] **Step 5: Keep asset outcome separate from runtime-version resolution; unknown runtime cannot become PASS for build eligibility.**
- [ ] **Step 6: Run tests and commit.**

```powershell
git add engines/renpy tests/integration/renpy_asset_audit_test.rs tests/fixtures/renpy_asset_audit Docs/REFERENCES.md
git commit -m "feat: add RenPy asset audit adapter"
```

---

### Task 5: Tyrano Static Adapter

**Files:**
- Create: `engines/tyrano/src/asset_audit.rs`
- Modify: `engines/tyrano/src/lib.rs`
- Modify: `engines/tyrano/src/asar.rs`
- Create: `tests/integration/tyrano_asset_audit_test.rs`
- Create synthetic fixtures under: `tests/fixtures/tyrano_asset_audit/`

**Interfaces:**
- Produces: `TyranoAssetAuditor` for unpacked and ASAR layouts.

- [ ] **Step 1: Write tests** for `.ks` scenario jumps, image/background/character/audio/video/script/plugin tags, missing jump target, macro/dynamic storage, unpacked files, ASAR members, native `.node`, traversal, and archive limits.
- [ ] **Step 2: Verify failure with `cargo test --test tyrano_asset_audit_test`.**
- [ ] **Step 3: Extend ASAR with bounded list/read metadata APIs** without extracting into the source directory.
- [ ] **Step 4: Implement scenario tokenizer using only S10/S12-supported semantics**; unknown tags remain explicit unknowns.
- [ ] **Step 5: Preserve separate fields:** `technical_outcome` and immutable `policy_outcome = BLOCKED` while legal gate remains active.
- [ ] **Step 6: Run tests and commit.**

```powershell
git add engines/tyrano tests/integration/tyrano_asset_audit_test.rs tests/fixtures/tyrano_asset_audit
git commit -m "feat: add Tyrano asset audit adapter"
```

---

### Task 6: Registry Integration and Cross-Engine Contracts

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `tests/integration/asset_audit_cli_test.rs`
- Create: `tests/integration/asset_audit_contract_test.rs`

- [ ] **Step 1: Write cross-engine tests** asserting all v1 engines register exactly one adapter, generic HTML remains out of scope, reports are deterministic, deep coverage is not weaker than standard, and source hashes are unchanged.
- [ ] **Step 2: Register all adapters in `create_default_asset_auditor_registry`.**
- [ ] **Step 3: Run all adapter tests.**

```powershell
cargo test --test rgss_asset_audit_test
cargo test --test web_asset_audit_test
cargo test --test renpy_asset_audit_test
cargo test --test tyrano_asset_audit_test
cargo test --test asset_audit_contract_test
```

- [ ] **Step 4: Run lint/type verification.**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 5: Commit.**

```powershell
git add cli/src/main.rs tests/integration
git commit -m "feat: register multi-engine asset auditors"
```
