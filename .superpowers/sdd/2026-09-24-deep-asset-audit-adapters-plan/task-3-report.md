# Task 3 Report: MV and MZ Static Adapter

## Implementation

- Created `engines/rpgmaker_web/src/asset_audit.rs` implementing `RpgMakerWebAssetAuditor` which implements `runtimepack_core::asset_audit::AssetAuditor` for `EngineId::RpgMakerMv` and `EngineId::RpgMakerMz`.
- Exported `RpgMakerWebAssetAuditor` in `engines/rpgmaker_web/src/lib.rs`.
- Implemented content root normalization:
  - Supports both direct root layout (`data/System.json`, `index.html`, `js/plugins.js`, `img/`, `audio/`) and NW.js deployed nested layout (`package.json`, `www/data/System.json`, `www/index.html`, etc.).
  - Resolves target paths accurately relative to the normalized content root.
- Implemented static traversal of RPG Maker MV and MZ databases and maps without executing JavaScript:
  - `System.json`: Title1/2 images, Windowskin, standard IconSet, sound effects, BGM/ME audio, vehicle graphics and audio, encryption flags (`hasEncryptedImages`, `hasEncryptedAudio`, `encryptionKey`).
  - `Actors.json`: Character sprite, face graphic, and side-view battler graphic.
  - `Enemies.json`: Enemy battler graphics across both front-view (`img/enemies/`) and side-view (`img/sv_enemies/`) directories.
  - `Tilesets.json`: Multi-layer tileset textures (`img/tilesets/`).
  - `Animations.json`: Classic animation sheets (`animation1Name`, `animation2Name`), timing sound effects, and MZ Effekseer particles (`effects/{name}.efkefc` / `.efk`).
  - `CommonEvents.json` and `Troops.json`: Event command lists and troop battle pages.
  - `Map*.json`: BGM/BGS, parallaxes, dual battlebacks, tileset references, event page character graphics, move routes (Code 41: Change Graphic, Code 44: Play SE), and event commands.
- Implemented event command traversal:
  - Code 201 (Player Transfer): Separates direct transfer (verifying target `Map{:03}.json` on disk, emitting `CMP_AUD_REQUIRED_REFERENCE_MISSING` on broken targets) from variable transfer (`mode = 1`, recorded as dynamic/optional reference without failing audit).
  - Code 212 (Show Animation): References database animation entity.
  - Code 231 (Show Picture): Resolves `img/pictures/{name}`.
  - Code 241, 245, 249, 250 (BGM, BGS, ME, SE): Resolves respective audio directories with audio extensions.
  - Code 261 (Play Movie): Resolves `movies/{name}` with video extensions (`.webm`, `.mp4`).
  - Code 322 (Change Actor Graphic): Resolves updated character, face, and side-view battler graphics.
- Encrypted asset support:
  - MV: Resolves `.rpgmvp` for images, `.rpgmvo` / `.rpgmvm` for audio when encryption is active.
  - MZ: Resolves `.png_` / `.rmmzimg` for images, `.ogg_` / `.rmmzaud` for audio when encryption is active.
- External URL handling:
  - If a required reference points to an HTTP(S) URL and `!options.allow_network`, emits `CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED` with outcome `INDETERMINATE`.
  - When `options.allow_network == true`, records reference without blocking.
- Path safety and error handling:
  - Emits `CMP_AUD_UNSAFE_PATH` on directory traversal (`..`), absolute paths, or NUL bytes.
  - Emits `CMP_AUD_MALFORMED_RESOURCE` on syntax or parse errors in JSON.
  - Emits `CMP_NODE_NATIVE_MODULE` on precompiled `.node` addons in source tree or plugins.
  - Emits `CMP_AUD_CASE_MISMATCH` with `WARNING` outcome on casing differences.
  - Emits `CMP_AUD_CASE_COLLISION` with `FAIL` outcome on case-insensitive collisions.
- Standard vs Deep split:
  - Standard mode audits databases, maps, and plugin manifests (`js/plugins.js`).
  - Deep mode inspects active JavaScript plugin source files bounded by file size limits for string literals pointing to assets (`img/...`, `audio/...`, `movies/...`, `effects/...`), checking their presence on disk. Dynamic path concatenations emit `CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED` and yield `INDETERMINATE`.

## TDD Evidence

### RED
```text
cargo test --test web_asset_audit_test
```
Result: Compilation failed with `unresolved import runtimepack_engine_rpgmaker_web::RpgMakerWebAssetAuditor` because `RpgMakerWebAssetAuditor` did not yet exist.

### GREEN
```text
cargo test --test web_asset_audit_test
```
Result: 18 passed; 0 failed; 0 ignored; finished in 0.08s.

### Full Workspace Validation
```text
cargo test
cargo fmt --all -- --check
cargo clippy -p runtimepack-engine-rpgmaker-web --all-targets
```
Result:
- 146 tests passed across the entire workspace (51 core, 34 build, 18 rgss adapter, 18 web adapter, 9 cli, 9 detection, 8 compatibility, 4 pack, 1 tyrano asar).
- Zero test failures.
- Zero formatting diffs (`cargo fmt`).
- Zero clippy warnings in `runtimepack-engine-rpgmaker-web`.

## Tests Added

Integration tests in `tests/integration/web_asset_audit_test.rs`:
- `test_web_auditor_supports_mv_and_mz_only`: Validates engine support filters and metadata.
- `test_mv_valid_root_layout`: Validates complete pass and graph population for RPG Maker MV root layout.
- `test_mv_valid_deployed_www_layout`: Validates pass and graph normalization for deployed `www/` layout.
- `test_mz_valid_root_layout`: Validates pass and Effekseer animation support for RPG Maker MZ.
- `test_mv_missing_asset_fails`: Validates missing picture reference emits `CMP_AUD_REQUIRED_REFERENCE_MISSING`.
- `test_mv_case_mismatch_warning`: Validates case mismatch emits `CMP_AUD_CASE_MISMATCH` with `WARNING` outcome.
- `test_mv_case_collision_fails`: Validates case collision emits `CMP_AUD_CASE_COLLISION` with `FAIL` outcome.
- `test_mv_broken_transfer_fails`: Validates direct transfer to missing map emits `CMP_AUD_REQUIRED_REFERENCE_MISSING`.
- `test_mv_variable_transfer_passes`: Validates variable transfer is separated from direct transfer and does not fail audit.
- `test_mv_malformed_json_fails`: Validates malformed JSON emits `CMP_AUD_MALFORMED_RESOURCE` with `FAIL` outcome.
- `test_mv_unsafe_path_fails`: Validates path traversal in reference emits `CMP_AUD_UNSAFE_PATH` with `FAIL` outcome.
- `test_mv_encryption_enabled_resolves_rpgmvp_and_rpgmvo`: Validates MV encrypted asset extensions resolve cleanly.
- `test_mz_encryption_enabled_resolves_png_underscore_and_ogg_underscore`: Validates MZ encrypted asset extensions resolve cleanly.
- `test_mv_unsupported_native_node_module_fails`: Validates `.node` addon emits `CMP_NODE_NATIVE_MODULE` with `FAIL` outcome.
- `test_external_resource_unverified_yields_indeterminate`: Validates unverified external URL emits `CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED` and yields `INDETERMINATE`.
- `test_external_resource_allowed_network_does_not_block`: Validates `--allow-network-audit` does not block on external URLs.
- `test_deep_mode_scans_plugin_source_literals`: Validates deep mode scans plugin source code for asset literals and verifies on disk.
- `test_deep_mode_unresolved_dynamic_yields_indeterminate`: Validates dynamic path expressions in plugins emit `CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED` and yield `INDETERMINATE`.

## Synthetic Fixtures Created

Under `tests/fixtures/web_asset_audit/`:
- `mv_valid_root`
- `mv_valid_www`
- `mz_valid_root`
- `mv_missing_asset`
- `mv_case_mismatch`
- `mv_broken_transfer`
- `mv_variable_transfer`
- `mv_malformed_json`
- `mv_unsafe_path`
- `mv_encrypted`
- `mz_encrypted`
- `mv_unsupported_node`
- `mv_external_url`
- `mv_deep_plugin_literals`
- `mv_deep_dynamic`
- `generate.py` (deterministic generator)

## Fix Round 1 (Review Findings Addressed)

1. **Required Database & Entrypoint Checks (Critical 1 & Important 4):**
   - Added validation for entrypoint presence: if `index.html` (or `www/index.html`) is missing, emits `CMP_AUD_REQUIRED_REFERENCE_MISSING` for `Entrypoint:index.html`.
   - Added presence checks for all core database files: `System.json`, `Actors.json`, `Enemies.json`, `Tilesets.json`, `Animations.json`, `CommonEvents.json`, `Troops.json`. Missing required files emit `CMP_AUD_REQUIRED_REFERENCE_MISSING`.
   - Added integrity validation for `Classes.json` and `MapInfos.json` when present, emitting `CMP_AUD_MALFORMED_RESOURCE` if corrupt.

2. **Command 201 Player Transfer CaseMismatch (Critical 2):**
   - Updated Command 201 direct player transfer to resolve target map path via `resolver.resolve_path(&target_map_path, AssetKind::Map)`.
   - Correctly handles `ResolutionResult::CaseMismatch`, emitting `CMP_AUD_CASE_MISMATCH` with Warning disposition and recording the resolved edge.
   - Handled `ResolutionResult::CaseCollision`, `Missing`, and `Unsafe` with appropriate diagnostic codes.

3. **Audit Limits Validation (Important 3):**
   - Enforced `options.limits.max_single_file_bytes` check in `read_json_file` before reading and parsing database files and maps.
   - Enforced `max_single_file_bytes` check before reading `plugins.js` manifest and plugin source files.
   - Emits `CMP_AUD_LIMIT_REACHED` if any single file exceeds the configured limit.

4. **Plugin Parsing & Transitive Dependencies (Important 5 & 7):**
   - Validates that `plugins.js` contains a valid JSON array. If the array cannot be located or parsed, emits `CMP_AUD_MALFORMED_RESOURCE`.
   - Added scanner for `@base <DependencyName>` annotations in active plugin headers, resolving each transitive plugin dependency `js/plugins/{dep_name}.js` with `ReferenceRequiredness::Required`. Missing dependencies emit `CMP_AUD_REQUIRED_REFERENCE_MISSING` pointing to the parent plugin node.

5. **Deep Mode Dynamic String Handling (Important 6):**
   - Hardened deep mode regex matching: dynamic path concatenations are identified, emit `CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED`, and their partial prefix strings are excluded from static disk inspection to prevent false missing asset errors.

6. **Refactoring & Minor Cleanups (Minor 8, 10, 11):**
   - Encapsulated traversal state, options, and collections into `AuditContext<'a>`, eliminating all `#[allow(clippy::too_many_arguments)]` annotations.
   - Cached compiled regular expressions using `std::sync::OnceLock<regex::Regex>`.
   - Cleaned up redundant dereferences `&mut *`.

7. **Tests Added in Fix Round 1:**
   - `test_mv_missing_entrypoint_fails_audit`: Validates missing `index.html` fails audit.
   - `test_mv_missing_required_database_fails_audit`: Validates missing `Actors.json` fails audit.
   - `test_mv_malformed_classes_fails_audit`: Validates corrupt `Classes.json` fails audit.
   - `test_mv_malformed_mapinfos_fails_audit`: Validates corrupt `MapInfos.json` fails audit.
   - `test_mv_transfer_case_mismatch_warning`: Validates player transfer case mismatch yields Warning outcome.
   - `test_mv_file_exceeds_max_single_file_bytes_fails_audit`: Validates single file size limit produces `CMP_AUD_LIMIT_REACHED`.
   - `test_mv_malformed_plugins_manifest_fails_audit`: Validates corrupt `plugins.js` produces `CMP_AUD_MALFORMED_RESOURCE`.
   - `test_mv_transitive_plugin_dependency_resolved`: Validates transitive `@base` plugin dependency resolution and missing detection.

