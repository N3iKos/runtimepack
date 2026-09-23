# Task 3 Execution Report: Build Flavor Pipeline (--flavor beta vs --flavor release) & Permanent Beta Watermark

## Summary
Successfully implemented the build flavor pipeline (`--flavor beta` vs `--flavor release`) across the Rust CLI, build planner, manifest schemas, and Android overlay generation, enforcing the permanent Beta Watermark (`Beta Test Veldra Community`) on beta builds and clean UI on release builds.

## Changes Made
1. **`cli/src/main.rs`**:
   - Added `BuildArgs` struct with `#[arg(long, default_value = "beta")] flavor: String`.
   - Updated `Commands::Build(BuildArgs)`.
   - Added validation in `cmd_build`: ensures flavor is either `"beta"` or `"release"` (case-insensitive); reports `[BLD-101] Invalid build flavor '<flavor>'. Must be 'beta' or 'release'.` and exits with code 1 if invalid.
   - Recorded `flavor` in `BuildManifest` (both `manifest.flavor` and `manifest.provenance.flavor`).
   - Passed `Some(&normalized_flavor)` into `AndroidProjectGenerator::emit_project_with_source`.

2. **`core/src/build_plan/mod.rs`**:
   - Added `#[serde(default, skip_serializing_if = "Option::is_none")] pub flavor: Option<String>` to `BuildManifest`.
   - Added `#[serde(default, skip_serializing_if = "Option::is_none")] pub flavor: Option<String>` to `ProvenanceManifestEntry`.

3. **`core/src/build_plan/generator.rs`**:
   - Extended `emit_project_with_source` to accept `flavor: Option<&str>`.
   - Updated `emit_project` to forward `manifest.flavor` / `manifest.provenance.flavor`.
   - Implemented `find_overlay_src` to reliably locate `android/shell/app/src/main/assets/overlay` from working directory or manifest root.
   - Implemented `configure_overlay_watermark(overlay_dir: &Path, flavor: &str)`:
     - For `"beta"`:
       - Configures `#fnh-watermark` in `vpad.js` with:
         ```html
         <div id="fnh-watermark">
             <div>Beta Test</div>
             <div>Veldra Community</div>
         </div>
         ```
       - Configures CSS rule `#fnh-watermark { display: block !important; opacity: 0.8 !important; pointer-events: none; }` in `vpad.js` and `index.html`.
     - For `"release"`:
       - Configures `#fnh-watermark` in `vpad.js` with `display: none !important;` and omits/removes beta watermark text.
       - Enforces `#fnh-watermark { display: none !important; }` in both `vpad.js` and `index.html`.

4. **`schemas/build-manifest.schema.json`**:
   - Added optional `"flavor": { "type": ["string", "null"] }` to schema root properties and `provenance.properties`.

5. **`AGENTS.md`**:
   - Documented the mandatory agent policy in Section 23: Autonomous subagents MUST ALWAYS build using `--flavor beta` unless explicitly instructed otherwise by the user.

6. **`tests/integration/build_test.rs`**:
   - Added `test_build_flavor_beta_injects_watermark`: verifies that emitting a project with `--flavor beta` injects `"Beta Test"`, `"Veldra Community"`, `display: block !important`, and `opacity: 0.8 !important` into the overlay.
   - Added `test_build_flavor_release_omits_watermark`: verifies that emitting a project with `--flavor release` cleanly omits `"Beta Test"`, `"Veldra Community"`, and enforces `display: none !important`.
   - Updated existing test cases constructing `BuildManifest` with `flavor: None`.

## Verification Results
- Ran `cargo test --test integration_build`: 27/27 tests passed.
- Ran `cargo test`: all tests passed across all crates in workspace (55+ tests passed, 0 failures, 0 errors).
- Ran `cargo run -- build --help`: verifies `--flavor <FLAVOR> [default: beta]`.
- Ran `cargo run -- build dummy_path --flavor invalid_flavor`: verified diagnostic `[BLD-101] Invalid build flavor 'invalid_flavor'. Must be 'beta' or 'release'.` output.
