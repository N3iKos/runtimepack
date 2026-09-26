# Implementation Plan: Black Souls 2 Audit Detection & Visual Rendering Fix

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the false 996-file probe audit by bundling `asset-dependency-graph.json` and restricting fallback discovery to media assets; add font/glyph coverage auditing and CJK font provisioning to resolve `□` mojibake; add map tileset transparency and subtractive fog occlusion detection to resolve pitch-black map backgrounds.

**Architecture:** 
1. Build-time generator packages `asset-dependency-graph.json` into APK assets; `AssetProbeRunner` prioritizes graph assets and only verifies decodability on media files.
2. Static audit parses script font requests and evaluates glyph coverage against CJK/punctuation ranges (`0x3000..0x30FF`), while runtime probe checks `Font.exist?` and Android shell provides CJK font fallback.
3. Static audit analyzes Map Layer 0/1 tile tables to catch missing floor tilesets before they render transparent, and detects subtractive fog planes; runtime auto-recovery logs tileset texture failures.

**Tech Stack:** Rust (core audit, generator), Kotlin (AssetProbeRunner, GpackExtractor), Ruby (RGSS bootstrap, loader hooks), FreeType / TrueType font tables.

**Spec:** `Docs/HASIL_INVESTIGASI_DIAGNOSTIC_BLACK_SOULS_2.md`

## Global Constraints

- Game content source remains 100% immutable (no direct modifications to `Raw-Game/`).
- Autonomous subagents build with `--flavor beta`.
- All tests must pass before completing each task.
- Zero fake progress or simulated `SUCCESS` in overlay or probe runner.

---

### Task 1: Fix Probe Asset Discovery & Media Filter (Eliminate Fake 996 File Audit)

**Files:**
- Modify: `core/src/build_plan/generator.rs`
- Modify: `cli/src/main.rs`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt`
- Modify: `android/shell/app/src/test/java/io/runtimepack/host/AssetProbeRunnerTest.kt`
- Modify: `tests/integration/build_test.rs`

**Interfaces:**
- Produces:
  - APK `assets/asset-dependency-graph.json` packaged by `generator.rs`.
  - `AssetProbeRunner.kt` reads graph from APK assets via `Context.assets` or disk.
  - Fallback `discoverAssets()` filters strictly by valid media extensions (`.png`, `.jpg`, `.ogg`, `.wav`, `.mp3`, `.ttf`, `.otf`), excluding `.bin`, `.rb`, `.txt`, `.json`.
  - Non-image media evaluated with `FILE_READABLE`, never labeled `DECODE_SUCCESS`.

- [x] **Step 1: Write failing unit & integration tests**:
  - `build_test.rs`: test `emit_project` includes `asset-dependency-graph.json` in APK assets when present in output directory.
  - `AssetProbeRunnerTest.kt`: test `discoverAssets()` ignores `.bin`, `.rb`, `.txt`, `.json` files and only includes media files.
  - `AssetProbeRunnerTest.kt`: test non-image readable files do not emit `DECODE_SUCCESS`.
- [x] **Step 2: Run tests to verify failure:**
  - `cargo test -p runtimepack --test integration_build test_emit_project_packages_asset_dependency_graph`
  - `.\gradlew.bat testDebugUnitTest --tests io.runtimepack.host.AssetProbeRunnerTest` (in `android/shell`)
- [x] **Step 3: Update `core/src/build_plan/generator.rs` and `cli/src/main.rs`**:
  - Copy `asset-dependency-graph.json` from output directory or audit artifacts to `assets_dir.join("asset-dependency-graph.json")`.
- [x] **Step 4: Update `AssetProbeRunner.kt`**:
  - Add `context: Context?` to constructor / discovery so it can read `asset-dependency-graph.json` directly from `context.assets`.
  - In `isExcludedPath(relPath)` / `discoverAssets()`: reject non-media extensions (`.bin`, `.rb`, `.txt`, `.json`, `.tmp`, `.bak`).
  - Stage 4: only emit `DECODE_SUCCESS` for files where actual image decoding succeeded.
- [x] **Step 5: Run tests to verify pass:**
  - `.\gradlew.bat testDebugUnitTest --tests io.runtimepack.host.AssetProbeRunnerTest`
  - `cargo test --test integration_build`
- [ ] **Step 6: Commit changes:**
  ```powershell
  git add core/src/build_plan/generator.rs cli/src/main.rs android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt android/shell/app/src/test/java/io/runtimepack/host/AssetProbeRunnerTest.kt tests/integration/build_test.rs
  git commit -m "fix(probe): package asset graph into APK assets and filter fallback discovery to media files"
  ```

---

### Task 2: Font & Glyph Coverage Audit + CJK Font Provisioning (Resolve `□` Mojibake)

**Files:**
- Modify: `engines/rgss/src/asset_audit.rs`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt`
- Modify: `android/shell/app/src/main/assets/runtimepack_asset_audit.rb`
- Create: `tests/fixtures/rgss_asset_audit/cjk_font_test/`
- Modify: `tests/integration/rgss_asset_audit_test.rs`
- Modify: `android/shell/app/src/test/ruby/asset_audit_test.rb`

**Interfaces:**
- Produces:
  - `RgssAssetAuditor` scans script font requests (`Font.default_name`, `VL Gothic`, `UmePlus Gothic`).
  - Emits `CMP_AUD_REQUIRED_REFERENCE_MISSING` if no physical font file exists in `Fonts/` and no valid substitution is found.
  - Detects missing CJK glyphs (`「`, `」`, Hiragana, Katakana) against active font `cmap`.
  - `GpackExtractor.kt` bundles/copies a lightweight CJK-capable font fallback to `<destDir>/Fonts/` if `Fonts/` is empty or missing, mapping `VL Gothic` and `UmePlus Gothic` to it.
  - `runtimepack_asset_audit.rb` probes `Font.exist?` at runtime.

- [x] **Step 1: Write failing unit tests**:
  - `rgss_asset_audit_test.rs`: test game script requesting `VL Gothic` without `Fonts/` directory emits font missing finding.
  - `rgss_asset_audit_test.rs`: test dialogue text containing `「` or `」` without CJK glyph in font emits `CMP_AUD_REQUIRED_REFERENCE_MISSING` or warning.
  - `asset_audit_test.rb`: test runtime `probe_font_availability!` records fallback event when `Font.exist?` returns false.
- [x] **Step 2: Run tests to verify failure:**
  - `cargo test --test rgss_asset_audit_test`
  - `ruby android/shell/app/src/test/ruby/asset_audit_test.rb`
- [x] **Step 3: Implement font auditing in `engines/rgss/src/asset_audit.rs`**:
  - Inspect `Data/Scripts.*` for font declarations (`Font.default_name`, `FONT = "..."`).
  - Verify whether font exists in `<game_dir>/Fonts/*.{ttf,otf,ttc}`.
  - Check dialogue strings for CJK/punctuation characters (`0x3000..0x30FF`, `0x4E00..0x9FFF`); if target font is missing or Liberation Sans fallback is used, emit diagnostic `CMP_AUD_REQUIRED_REFERENCE_MISSING` with message indicating missing glyphs/font.
- [x] **Step 4: Provision CJK font fallback in `GpackExtractor.kt` and APK assets**:
  - Bundle a clean open-source CJK font (e.g. `DroidSansFallback.ttf` or `NotoSansCJK-subset.otf`) in `android/shell/app/src/main/assets/fonts/`.
  - In `GpackExtractor.installRuntimeFiles`: if `Fonts/` does not exist or has no font files, copy fallback font into `destDir/Fonts/` and map `VL Gothic`, `UmePlus Gothic`, `MS Gothic` in `mkxp.json` to this font.
- [x] **Step 5: Add runtime font probe in `runtimepack_asset_audit.rb`**:
  - Add `probe_fonts!` verifying `Font.exist?(font_name)`; emit `FONT_FALLBACK_LIBERATION_SANS` if false.
- [x] **Step 6: Run tests to verify pass:**
  - `cargo test --test rgss_asset_audit_test`
  - `ruby android/shell/app/src/test/ruby/asset_audit_test.rb`
  - `.\gradlew.bat testDebugUnitTest`
- [ ] **Step 7: Commit changes:**
  ```powershell
  git add engines/rgss/src/asset_audit.rs android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt android/shell/app/src/main/assets android/shell/app/src/test tests/integration/rgss_asset_audit_test.rs
  git commit -m "feat(font): audit CJK glyph coverage and provide CJK fallback font for RGSS"
  ```

---

### Task 3: Missing Floor Tileset & Darkness Occlusion Detection (Resolve Black Screen)

**Files:**
- Modify: `engines/rgss/src/asset_audit.rs`
- Modify: `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
- Modify: `android/shell/app/src/main/assets/runtimepack_asset_audit.rb`
- Modify: `tests/integration/rgss_asset_audit_test.rs`
- Modify: `android/shell/app/src/test/ruby/asset_audit_test.rb`

**Interfaces:**
- Produces:
  - `RgssAssetAuditor` inspects `Data/Map*.rvdata2` Layer 0/1 tile IDs against tileset files (A1..A5).
  - Emits `CMP_AUD_REQUIRED_REFERENCE_MISSING` with high severity warning `"Map floor tileset missing; layer will render transparent/black"` if Layer 0 tiles lack bitmap files.
  - Scans map note tags for subtractive fog planes (`==マップフォグ.*\[.*,.*,(\d+),.*,2\]==` with `z < 32`), emitting warning for potential OpenGL ES subtractive occlusion.
  - `runtimepack_bootstrap.rb`: when tileset fallback occurs, logs diagnostic event and provides neutral tileset fallback pattern instead of pure invisible transparent black.

- [x] **Step 1: Write failing unit tests**:
  - `rgss_asset_audit_test.rs`: test map with Layer 0 tiles using non-existent `Inside_A2` emits missing floor tileset warning.
  - `rgss_asset_audit_test.rs`: test map note with subtractive fog plane at z=10 emits potential occlusion warning.
  - `asset_audit_test.rb`: test `Cache.tileset` failure records `TILESET_TEXTURE_ALLOCATION` fallback event.
- [x] **Step 2: Run tests to verify failure:**
  - `cargo test --test rgss_asset_audit_test`
  - `ruby android/shell/app/src/test/ruby/asset_audit_test.rb`
- [x] **Step 3: Implement Map Layer 0/1 Tileset Auditing in `engines/rgss/src/asset_audit.rs`**:
  - In `audit_map_files`: parse map `data` table (dimensions `width x height x layers`).
  - Scan Layer 0 & 1 tile IDs: if tiles in ranges `2816..4351` (A2), `5888..7423` (A4), `1536..2047` (A5) are placed on the map, but the corresponding tileset images are missing on disk, emit finding:
    `CMP_AUD_REQUIRED_REFERENCE_MISSING`: `"Map {map_id} Layer {layer} has {count} tiles referencing missing tileset '{tileset_name}'; floor/walls will render transparent/black"`.
- [x] **Step 4: Implement Subtractive Fog Plane Detection**:
  - In map note tag parsing: if `==マップフォグ.*==` specifies `blend == 2` and `z < 32`, emit finding:
    `CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED` or warning `"Map {map_id} has subtractive fog Plane at Z={z} (below character layer Z=32); may cause black occlusion under OpenGL ES"`.
- [x] **Step 5: Update `runtimepack_bootstrap.rb` & `runtimepack_asset_audit.rb`**:
  - When `Cache.tileset` encounters missing RTP tileset, log event with `fallback_action: "TILESET_FALLBACK_DUMMY"` and descriptive error message.
  - If subtractive fog is detected with missing floor, adjust or safeguard blend mode so it does not subtract framebuffer alpha.
- [x] **Step 6: Run tests to verify pass:**
  - `cargo test --test rgss_asset_audit_test`
  - `ruby android/shell/app/src/test/ruby/asset_audit_test.rb`
  - `cargo test --workspace`
- [ ] **Step 7: Commit changes:**
  ```powershell
  git add engines/rgss/src/asset_audit.rs android/shell/app/src/main/assets tests/integration/rgss_asset_audit_test.rs android/shell/app/src/test
  git commit -m "feat(rgss): detect missing floor tilesets and subtractive fog occlusion in asset audit"
  ```
