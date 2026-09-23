# Task 3: Build Flavor Pipeline (--flavor beta vs --flavor release) & Permanent Beta Watermark

## Goal
Implement build flavor pipeline (`--flavor beta` vs `--flavor release`) across the Rust CLI, build planner, and Android shell overlay, enforcing the permanent Beta Watermark (`Beta Test Veldra Community`) on beta builds and clean UI on release builds.

## Requirements
1. In `cli/src/main.rs`:
   - Add CLI argument `#[arg(long, default_value = "beta")] flavor: String` to `BuildArgs`.
   - Validate that `flavor` is either `"beta"` or `"release"` (case-insensitive); return diagnostic `BLD-101` if invalid.
   - Record `flavor` in `BuildManifest` (e.g. `provenance` or dedicated field).
   - Pass `flavor` into `AndroidProjectGenerator::emit_project_with_source`.
2. In `core/src/build_plan/generator.rs`:
   - When emitting project:
     - If `flavor.to_lowercase() == "beta"`:
       Configure `overlay/vpad.js` / `overlay/index.html` to display the permanent top-right watermark:
       ```html
       <div id="fnh-watermark">
           <div>Beta Test</div>
           <div>Veldra Community</div>
       </div>
       ```
       with CSS `#fnh-watermark { display: block !important; opacity: 0.8 !important; pointer-events: none; }`.
     - If `flavor.to_lowercase() == "release"`:
       Ensure `#fnh-watermark` is cleanly disabled (`display: none !important;`).
3. In `AGENTS.md`:
   - Document the mandatory agent policy: Autonomous subagents MUST ALWAYS build using `--flavor beta` unless explicitly instructed otherwise by the user.
4. Integration Tests in `tests/integration/build_test.rs`:
   - Add `test_build_flavor_beta_injects_watermark`: verify watermark text appears in emitted overlay when flavor is beta.
   - Add `test_build_flavor_release_omits_watermark`: verify watermark text is absent/disabled when flavor is release.
5. Run tests:
   `cargo test`
6. Write execution report to `.superpowers/sdd/2026-09-23-beta-diagnostic-and-blacksouls-audit/task-3-report.md`.

## Critical Constraints
- NEVER touch, read, or write any folder named `antigravity` or `.antigravity`.
- Source directories in `Raw-Game/` are strictly READ-ONLY.
