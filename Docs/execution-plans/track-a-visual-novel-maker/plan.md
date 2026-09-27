# Implementation Plan: Visual Novel Maker (VNM) Engine Support

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish complete end-to-end support for Visual Novel Maker (VNM) games by registering `EngineId::VisualNovelMaker`, implementing static detection, building the static asset auditor, and wiring compilation directly to the existing hardware-accelerated `WebActivity.kt` runtime.

**Architecture:**
1. In `core/src/domain.rs` and registries, register `EngineId::VisualNovelMaker` with `RuntimeFamily::Web`.
2. Create `engines/vnmaker` crate with `VnMakerDetector` to detect canonical markers (`signature`, `package.json`, `data/Scenes.json`, `data/System.json`).
3. Implement `VnMakerAssetAuditor` in `engines/vnmaker` to audit scene graphic and audio references in `resources/` with reachability filtering.
4. Integrate into `core::build_plan::generator` and CLI, emitting standalone APK projects configured with `WebActivity.kt`, VFS streaming decrypter, and Visual Novel VPad overlay.

**Tech Stack:** Rust (core, engines/vnmaker), HTML5/WebGL, Kotlin (`WebActivity.kt`, `VfsResourceInterceptor.kt`).

**Spec:** `Docs/execution-plans/track-a-visual-novel-maker/design.md`

## Global Constraints

- Source directories in `Raw-Game/` remain strictly 100% immutable.
- Zero disk extraction: all asset discovery operates in-memory.
- Standard error taxonomy codes (`DET-*`, `CMP-AUD-*`, `BLD-*`) must remain consistent.
- Autonomous subagents must build with `--flavor beta`.
- All subagents must run tests and verify zero regressions across the workspace before each commit.

---

### Task 1: Domain & Engine Registration in `core`

**Files:**
- Modify: `core/src/domain.rs`
- Modify: `schemas/build-manifest.schema.json`
- Test: `tests/integration/detection_test.rs`

**Interfaces:**
- Produces:
  - `EngineId::VisualNovelMaker` with `as_str() -> "vnmaker"` and display name `"Visual Novel Maker"`.
  - Maps to `Some(RuntimeFamily::Web)` in `runtime_family()`.

- [ ] **Step 1: Write failing test in `detection_test.rs`**:
  ```rust
  #[test]
  fn test_vnmaker_engine_domain_id_and_runtime_family() {
      use runtimepack_core::domain::{EngineId, RuntimeFamily};
      let engine = EngineId::VisualNovelMaker;
      assert_eq!(engine.as_str(), "vnmaker");
      assert_eq!(engine.runtime_family(), Some(RuntimeFamily::Web));
  }
  ```
- [ ] **Step 2: Run test to verify failure**:
  `cargo test --test detection_test test_vnmaker_engine_domain_id_and_runtime_family`
- [ ] **Step 3: Implement domain registration**:
  - In `core/src/domain.rs`:
    Add `VisualNovelMaker` to `EngineId` enum (`#[serde(rename = "vnmaker")]`).
    Map in `Display` (`"Visual Novel Maker"`), `as_str()` (`"vnmaker"`), and `runtime_family()` (`Some(RuntimeFamily::Web)`).
  - In `schemas/build-manifest.schema.json`:
    Add `"vnmaker"` to engine enum list.
- [ ] **Step 4: Run test to verify pass**:
  `cargo test --test detection_test test_vnmaker_engine_domain_id_and_runtime_family`
- [ ] **Step 5: Commit**:
  ```powershell
  git add core/src/domain.rs schemas/build-manifest.schema.json tests/integration/detection_test.rs
  git commit -m "feat(core): register EngineId::VisualNovelMaker with Web runtime family"
  ```

---

### Task 2: Visual Novel Maker Static Detector (`engines/vnmaker/src/lib.rs`)

**Files:**
- Create: `engines/vnmaker/Cargo.toml`
- Create: `engines/vnmaker/src/lib.rs`
- Modify: `Cargo.toml` (add `engines/vnmaker` to workspace members)
- Modify: `core/Cargo.toml`
- Modify: `core/src/detection/mod.rs`
- Test: `tests/integration/detection_test.rs`

**Interfaces:**
- Produces:
  - `pub struct VnMakerDetector;` implementing `runtimepack_core::detection::Detector`.
  - Registered in `create_default_detector_registry()`.

- [ ] **Step 1: Create `engines/vnmaker/Cargo.toml` and update root `Cargo.toml`**:
  - Add `"engines/vnmaker"` to root `Cargo.toml` `members`.
  - Create `engines/vnmaker/Cargo.toml`:
    ```toml
    [package]
    name = "runtimepack-engine-vnmaker"
    version = "0.1.0"
    edition.workspace = true
    license.workspace = true

    [dependencies]
    runtimepack-core = { path = "../../core" }
    serde = { workspace = true }
    serde_json = { workspace = true }
    thiserror = { workspace = true }
    regex = { workspace = true }
    ```
- [ ] **Step 2: Write integration tests in `detection_test.rs`**:
  - `test_detect_vnmaker_with_signature_and_package_json`:
    Directory with `signature`, `package.json`, and `data/System.json` -> returns `EngineId::VisualNovelMaker`, `DetectionConfidence::Exact`.
  - `test_detect_vnmaker_does_not_conflict_with_rpgmaker_mv`:
    Directory with `js/rpg_core.js` and `data/System.json` -> returns `EngineId::RpgMakerMv`, NOT `VisualNovelMaker`.
- [ ] **Step 3: Implement `VnMakerDetector` in `engines/vnmaker/src/lib.rs`**:
  - Check for RPG Maker markers (`js/rpg_core.js`, `js/rmmz_core.js`) and Tyrano markers (`tyrano/tyrano.js`). If found, return `DetectionConfidence::None` to prevent conflicts.
  - Score detection:
    - `signature` file present -> +90 pts, `DET-VNM-001`.
    - `data/System.json` or `data/Scenes.json` present without RPG Maker scripts -> +80 pts, `DET-VNM-002`.
    - `package.json` with `"main": "index.html"` + root `index.html` -> +40 pts, `DET-VNM-003`.
    - `resources/Graphics/` or `resources/Audio/` present -> +30 pts, `DET-VNM-004`.
  - Confidence: `Exact` if score >= 90, `High` if score >= 60, `Medium` if score >= 30, else `None`.
  - Register `VnMakerDetector` in `core/src/detection/mod.rs`.
- [ ] **Step 4: Run tests to verify pass**:
  `cargo test --test detection_test`
- [ ] **Step 5: Commit**:
  ```powershell
  git add Cargo.toml Cargo.lock engines/vnmaker/ core/Cargo.toml core/src/detection/mod.rs tests/integration/detection_test.rs
  git commit -m "feat(vnmaker): implement static engine detector for Visual Novel Maker"
  ```

---

### Task 3: Static Asset Reachability Auditor (`engines/vnmaker/src/asset_audit.rs`)

**Files:**
- Create: `engines/vnmaker/src/asset_audit.rs`
- Modify: `engines/vnmaker/src/lib.rs`
- Modify: `core/src/asset_audit/mod.rs`
- Test: `tests/integration/vnmaker_asset_audit_test.rs`

**Interfaces:**
- Produces:
  - `pub struct VnMakerAssetAuditor;` implementing `runtimepack_core::asset_audit::AssetAuditor`.
  - Registered in `create_default_asset_auditor_registry()`.

- [ ] **Step 1: Write failing test in `vnmaker_asset_audit_test.rs`**:
  - Create synthetic VNM project with `data/Scenes.json` referencing `resources/Graphics/Backgrounds/classroom.png` and `resources/Audio/BGM/theme.ogg`.
  - Assert that auditor verifies referenced assets exist and returns `AuditOutcome::Pass`.
  - Assert that case-mismatches resolve as `AuditOutcome::Warning` without failing.
- [ ] **Step 2: Run test to verify failure**:
  `cargo test --test vnmaker_asset_audit_test`
- [ ] **Step 3: Implement `VnMakerAssetAuditor`**:
  - Inspect `data/Scenes.json` and `data/System.json`.
  - Parse resource references in scenes (backgrounds, characters, pictures, audio BGM/SE).
  - Use `AssetResolver` with case-folding support.
  - Register `VnMakerAssetAuditor` in `core/src/asset_audit/mod.rs`.
- [ ] **Step 4: Run tests to verify pass**:
  `cargo test --test vnmaker_asset_audit_test`
- [ ] **Step 5: Commit**:
  ```powershell
  git add engines/vnmaker/src/asset_audit.rs engines/vnmaker/src/lib.rs core/src/asset_audit/mod.rs tests/integration/vnmaker_asset_audit_test.rs
  git commit -m "feat(vnmaker): implement static asset reachability auditor for Visual Novel Maker"
  ```

---

### Task 4: Generator & CLI Wiring for Visual Novel Maker

**Files:**
- Modify: `core/src/build_plan/generator.rs`
- Modify: `cli/src/main.rs`
- Test: `tests/integration/build_test.rs`

**Interfaces:**
- Produces:
  - `AndroidProjectGenerator`: Emits project with `WebActivity.kt` for `vnmaker` engine projects.
  - Automatically packages `signature`, `package.json`, `index.html`, `data/`, `resources/`, `fonts/`, `js/` into `game.pack`.
  - Injects `vpad.js` with Visual Novel control preset.

- [ ] **Step 1: Write integration build test in `build_test.rs`**:
  ```rust
  #[test]
  fn test_build_vnmaker_project_emits_web_activity_and_vpad_overlay() {
      let temp = test_temp_dir("vnmaker-build");
      let manifest = create_test_manifest_for_vnmaker();
      AndroidProjectGenerator::emit_project(&temp, &manifest, None).unwrap();
      
      let manifest_xml = std::fs::read_to_string(temp.join("app/src/main/AndroidManifest.xml")).unwrap();
      assert!(manifest_xml.contains("io.runtimepack.host.WebActivity"));
  }
  ```
- [ ] **Step 2: Run test to verify failure**:
  `cargo test --test build_test test_build_vnmaker_project_emits_web_activity_and_vpad_overlay`
- [ ] **Step 3: Update `generator.rs` and `cli/src/main.rs`**:
  - In `generator.rs`: Match `"vnmaker" => "io.runtimepack.host.WebActivity",`
  - In `cli/src/main.rs`: Include `runtimepack-engine-vnmaker` in default detector, analyzer, and auditor registries.
- [ ] **Step 4: Run full test suite**:
  `cargo test --workspace`
- [ ] **Step 5: Commit**:
  ```powershell
  git add core/src/build_plan/generator.rs cli/src/main.rs tests/integration/build_test.rs
  git commit -m "feat(generator): support Visual Novel Maker project emission and packaging via WebActivity"
  ```
