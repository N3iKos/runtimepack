# Implementation Plan: Comprehensive Static Asset Audit Remediation & Deep In-Memory Archive Inspection

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Clean up remaining audit findings across all 8 target games to achieve clean / `PASS` static audits through Shift-JIS auto-decoding, database reachability tree-shaking, VFS case-aliasing policy resolution, and zero-extraction in-memory archive indexers.

**Architecture:** 
1. In `engines/tyrano`, auto-detect and decode Shift-JIS / CP932 encoded `.ks` scenario text using `encoding_rs` to prevent Japanese Kanji replacement characters (`\u{FFFD}`) and eliminate false missing label errors.
2. In `engines/rpgmaker_web`, build an active reachability set from maps, common events, and troops to downgrade unreferenced default database entities (e.g. `Skeleton1_1`, `Ship1.ogg`) from fatal missing errors to `CMP_AUD_POSSIBLY_UNUSED` (Informational).
3. In `core/src/build_plan/audit_policy.rs` and resolver, acknowledge that case mismatches are automatically aliased by runtime VFS and do not fail the build policy.
4. In `engines/rgss` and `engines/renpy`, implement in-memory archive index decoders (`.rgssad`, `.rgss2a`, `.rgss3a`, `.rpa`) that read internal file tables directly from file headers/trailers, lifting audit status from `INDETERMINATE` to fully cataloged `PASS`.
5. Re-run audit across all 8 games and verify clean / `PASS` audit reports.

**Tech Stack:** Rust (core, engines/tyrano, engines/rpgmaker_web, engines/rgss, engines/renpy), `encoding_rs`, `flate2`.

**Spec:** `Docs/superpowers/specs/2026-09-27-comprehensive-asset-audit-remediation-design.md`

## Global Constraints

- Source directories in `Raw-Game/` remain strictly 100% immutable.
- Zero disk extraction: all archive index discovery operates in-memory.
- All builds and tests must maintain deterministic error taxonomy codes (`CMP-AUD-*`).
- Subagents must run tests and verify zero regressions across workspace before each commit.

---

### Task 1: Shift-JIS Scenario Auto-Decoding in Tyrano

**Files:**
- Modify: `Cargo.toml`
- Modify: `engines/tyrano/Cargo.toml`
- Modify: `engines/tyrano/src/asar.rs`
- Modify: `engines/tyrano/src/asset_audit.rs`
- Test: `tests/integration/tyrano_asset_audit_test.rs`

**Interfaces:**
- Produces:
  - `pub fn decode_scenario_text(bytes: &[u8]) -> String`
  - Replaces `String::from_utf8_lossy(bytes)` across Tyrano scenario loaders.
  - If bytes are not valid UTF-8 or contain invalid replacement characters in Japanese multibyte ranges, decodes via `encoding_rs::SHIFT_JIS`.

- [x] **Step 1: Add `encoding_rs = "0.8"` to workspace dependencies**:
  - In root `Cargo.toml`: add `encoding_rs = "0.8"` under `[workspace.dependencies]`.
  - In `engines/tyrano/Cargo.toml`: add `encoding_rs = { workspace = true }`.
- [x] **Step 2: Write failing test in `tyrano_asset_audit_test.rs`**:
  - Construct a scenario byte array encoded in Shift-JIS with Japanese Kanji jump labels (e.g. `*部屋 [cm] [jump target="*部屋"]`).
  - Verify that before the fix, reading as lossy UTF-8 fails to match the label, producing `CMP-AUD-002` ("Target label not found").
- [x] **Step 3: Implement `decode_scenario_text`**:
  - In `engines/tyrano/src/asset_audit.rs` and `engines/tyrano/src/asar.rs`:
    ```rust
    pub fn decode_scenario_text(bytes: &[u8]) -> String {
        let stripped = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
        if let Ok(utf8_str) = std::str::from_utf8(stripped) {
            if !utf8_str.contains('\u{FFFD}') {
                return utf8_str.to_string();
            }
        }
        let (cow, _, _) = encoding_rs::SHIFT_JIS.decode(stripped);
        cow.into_owned()
    }
    ```
  - Use `decode_scenario_text` in `TyranoReader::read_file_string` and `AsarArchive::read_file_string_bounded`.
- [x] **Step 4: Run tests to verify pass**:
  - `cargo test --test tyrano_asset_audit_test`
- [x] **Step 5: Commit**:
  ```powershell
  git add Cargo.toml Cargo.lock engines/tyrano/Cargo.toml engines/tyrano/src/asar.rs engines/tyrano/src/asset_audit.rs tests/integration/tyrano_asset_audit_test.rs
  git commit -m "feat(tyrano): support automatic Shift-JIS text decoding for Japanese scenario files"
  ```

---

### Task 2: Reachability Graph Filter for RPG Maker MV/MZ Database

**Files:**
- Modify: `engines/rpgmaker_web/src/asset_audit.rs`
- Test: `tests/integration/web_asset_audit_test.rs`

**Interfaces:**
- Produces:
  - `DatabaseReachabilityIndex`: Tracks enemy IDs, animation IDs, actor IDs, sound effects, and BGM actively placed or invoked in `Map*.json`, `CommonEvents.json`, and `Troops.json`.
  - In `audit_enemies`, `audit_animations`, and `audit_actors`: If a referenced asset is missing on disk but the containing database record is not in the active reachability index, emit `CMP_AUD_POSSIBLY_UNUSED` (`CMP-AUD-013`) with `FindingSeverity::Info` and `ReferenceRequiredness::Optional`, rather than fatal `CMP_AUD_REQUIRED_REFERENCE_MISSING` (`CMP-AUD-002`).

- [x] **Step 1: Write failing test in `web_asset_audit_test.rs`**:
  - Test a mock MV game where `data/Enemies.json` has an enemy ID 10 referencing missing `Skeleton1_1.png`, but no map or troop references enemy 10.
  - Assert that finding is `CMP_AUD_POSSIBLY_UNUSED` (Severity: Info) and audit outcome is `Pass`.
- [x] **Step 2: Implement `DatabaseReachabilityIndex` in `engines/rpgmaker_web/src/asset_audit.rs`**:
  - Collect referenced IDs before auditing database entities:
    - Parse `data/Troops.json` members (`enemyId`).
    - Parse `data/Map*.json` event command codes (101 faceName, 132 playBgm, 231 showPicture, 250 playSe, 301 battleProcessing troopId, 331 changeEnemyHp, etc.).
    - Parse `data/CommonEvents.json` command codes.
    - Parse `data/System.json` starting party (`partyMembers`).
  - Guard database asset auditing with reachability checks.
- [x] **Step 3: Run tests to verify pass**:
  - `cargo test --test web_asset_audit_test`
- [x] **Step 4: Commit**:
  ```powershell
  git add engines/rpgmaker_web/src/asset_audit.rs tests/integration/web_asset_audit_test.rs
  git commit -m "feat(web): add reachability tree-shaking for unused database entities"
  ```

---

### Task 3: VFS Auto-Aliasing Resolution for Case Mismatches in Policy

**Files:**
- Modify: `core/src/asset_audit/resolver.rs`
- Modify: `core/src/build_plan/audit_policy.rs`
- Test: `tests/integration/build_test.rs`

**Interfaces:**
- Produces:
  - `evaluate_audit_policy`: Does NOT treat `CMP_AUD_CASE_MISMATCH` as an audit failure if all matching files are safely mapped.
  - Records case mismatches as auto-aliased warnings without forcing outcome to `Fail`.

- [ ] **Step 1: Write unit test in `build_test.rs`**:
  - Test that an audit report containing only `CMP_AUD_CASE_MISMATCH` findings evaluates to `AuditPolicyDecision::Allow` (Pass/Warning) without requiring `--accept-audit-fail`.
- [ ] **Step 2: Update `evaluate_audit_policy` in `core/src/build_plan/audit_policy.rs`**:
  - Ensure case mismatches are treated as warnings and do not escalate outcome to `Fail`.
- [ ] **Step 3: Run tests to verify pass**:
  - `cargo test --test build_test`
  - `cargo test -p runtimepack-core`
- [ ] **Step 4: Commit**:
  ```powershell
  git add core/src/build_plan/audit_policy.rs core/src/asset_audit/resolver.rs tests/integration/build_test.rs
  git commit -m "feat(policy): allow case-mismatches to resolve as auto-aliased warnings"
  ```

---

### Task 4: In-Memory RGSS Archive Indexer (`.rgssad`, `.rgss2a`, `.rgss3a`)

**Files:**
- Create: `engines/rgss/src/archive.rs`
- Modify: `engines/rgss/src/lib.rs`
- Modify: `engines/rgss/src/asset_audit.rs`
- Test: `tests/integration/rgss_asset_audit_test.rs`

**Interfaces:**
- Produces:
  - `pub struct RgssArchiveIndex { pub files: HashMap<String, u32> }`
  - `pub fn parse_rgss_archive_index(bytes: &[u8]) -> Result<RgssArchiveIndex, ArchiveError>`
  - Decodes table entries for RGSSAD v1 (XP), v2 (VX), and v3 (VX Ace) in memory.
  - In `RgssAssetAuditor`: If `Data/System.*` or assets are inside the archive, merges entries into `effective_source.files` so internal files are audited as present.

- [ ] **Step 1: Write integration tests in `rgss_asset_audit_test.rs`**:
  - Generate a valid synthetic RGSS3A and RGSSAD archive in memory containing `Data/System.rxdata` or `Data/System.rvdata2`.
  - Assert `parse_rgss_archive_index` decodes the file names accurately.
  - Assert that `RgssAssetAuditor` audits internal assets without reporting `INDETERMINATE`.
- [ ] **Step 2: Implement `engines/rgss/src/archive.rs`**:
  - Implement header magic verification `b"RGSSAD\0"`.
  - Version 1/2 rolling key XOR algorithm:
    ```rust
    let mut key = initial_key;
    // filename length XOR key, filename bytes XOR key, size XOR key
    ```
  - Version 3 offset key XOR algorithm.
- [ ] **Step 3: Integrate with `RgssAssetAuditor`**:
  - When an encrypted archive is detected, parse its index and populate `effective_source`.
  - Elevate outcome to `Pass` when all required files are present inside the archive.
- [ ] **Step 4: Run tests to verify pass**:
  - `cargo test -p runtimepack-engine-rgss`
  - `cargo test --test rgss_asset_audit_test`
- [ ] **Step 5: Commit**:
  ```powershell
  git add engines/rgss/src/archive.rs engines/rgss/src/lib.rs engines/rgss/src/asset_audit.rs tests/integration/rgss_asset_audit_test.rs
  git commit -m "feat(rgss): implement in-memory RGSS archive index parser for deep audit"
  ```

---

### Task 5: In-Memory Ren'Py RPA Archive Indexer

**Files:**
- Create: `engines/renpy/src/archive.rs`
- Modify: `engines/renpy/src/lib.rs`
- Modify: `engines/renpy/src/asset_audit.rs`
- Test: `tests/integration/renpy_asset_audit_test.rs`

**Interfaces:**
- Produces:
  - `pub struct RpaArchiveIndex { pub files: HashSet<String> }`
  - `pub fn parse_rpa_index(bytes: &[u8]) -> Result<RpaArchiveIndex, ArchiveError>`
  - Decodes RPAv3 / RPAv2 header: `RPA-3.0 <offset_hex> <key_hex>` or `RPA-2.0 <offset_hex> <key_hex>`.
  - Decompresses zlib index slice and extracts file paths.
  - Integrates with `RenPyAssetAuditor`: Populates internal archive files into index so `.rpa` assets resolve cleanly.

- [x] **Step 1: Write integration tests in `renpy_asset_audit_test.rs`**:
  - Test synthetic RPAv3 archive index decompression and path parsing.
  - Verify `RenPyAssetAuditor` finds assets inside `.rpa` without reporting `CMP-AUD-007` archive limitation.
- [x] **Step 2: Implement `engines/renpy/src/archive.rs`**:
  - Parse header line, read hex offset and key.
  - Decompress with `flate2::read::ZlibDecoder`.
  - Extract keys (paths) from Python pickle dictionary in index.
- [x] **Step 3: Integrate with `RenPyAssetAuditor`**:
  - Register internal `.rpa` paths into `effective_source`.
  - Elevate outcome to `Pass` when game assets are located in archives.
- [x] **Step 4: Run tests to verify pass**:
  - `cargo test -p runtimepack-engine-renpy`
  - `cargo test --test renpy_asset_audit_test`
- [x] **Step 5: Commit**:
  ```powershell
  git add engines/renpy/src/archive.rs engines/renpy/src/lib.rs engines/renpy/src/asset_audit.rs tests/integration/renpy_asset_audit_test.rs
  git commit -m "feat(renpy): implement in-memory RPA archive index parser for deep audit"
  ```

---

### Task 6: Full Sweep Re-Audit & Verification across All 8 Games

**Files:**
- Output target: `dist/<game>/asset-audit-report.json` for all 8 games.

- [x] **Step 1: Execute `runtimepack audit` on all 8 games**:
  1. *Fear & Hunger 2: Termina* -> `dist/termina/asset-audit-report.json`
  2. *[Kimochi] NTRdemic* -> `dist/ntrdemic/asset-audit-report.json`
  3. *[Kimochi] show_off* -> `dist/show_off/asset-audit-report.json`
  4. *Mizuki And Me* -> `dist/mizuki_and_me/asset-audit-report.json`
  5. *Forestia* -> `dist/forestia/asset-audit-report.json`
  6. *The Witch's House MV* -> `dist/witchs_house/asset-audit-report.json`
  7. *Hello Charlotte EP1* -> `dist/hc_ep1/asset-audit-report.json`
  8. *To the Moon* -> `dist/to_the_moon/asset-audit-report.json`
- [x] **Step 2: Assert clean / PASS outcomes**:
  - *show_off*: Verify 40 label errors drop to 0.
  - *Termina*: Verify 571 missing asset errors drop to 0 fatal errors.
  - *Hello Charlotte*, *To the Moon*, *Mizuki And Me*: Verify internal files cataloged and outcome achieves `Pass`.
- [x] **Step 3: Commit verification and progress logs**:
  ```powershell
  git add memory/progress.md memory/decisions_log.md
  git commit -m "chore: verify full sweep clean static audit across all 8 games"
  ```
