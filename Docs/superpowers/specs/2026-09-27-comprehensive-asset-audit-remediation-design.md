# Specification: Comprehensive Static Asset Audit Remediation & Deep In-Memory Archive Inspection

**Date:** 2026-09-27  
**Status:** Approved  
**Scope:** Host CLI Static Audit Pipeline, Engine Auditing Modules (`tyrano`, `rpgmaker_web`, `rgss`, `renpy`), and Resolver Policy across all 8 supported games.

---

## 1. Context & Motivation

In the initial stabilization sweep, all 8 games successfully built standalone Android APKs (`dist/<game>/*.apk`). However, the static asset audit reports (`asset-audit-report.json`) for several games still exhibit non-critical errors or indeterminate status due to limitations in the static analyzer:
1. **Shift-JIS Encoding Mojibake in Tyrano:** Games like *[Kimochi] show_off* and *[Kimochi] NTRdemic* use Japanese Shift-JIS / CP932 encoded scenario files (`.ks`). The auditor read them with standard UTF-8 lossy decoding, turning Japanese Kanji jump labels into replacement characters (`\u{FFFD}` / ``), which resulted in 40 false `Target label not found` errors.
2. **Unused Database Entity Over-Reporting in RPG Maker MV/MZ:** RPG Maker MV/MZ projects (*Fear & Hunger 2: Termina*, *The Witch's House MV*) ship default engine database templates (e.g. `Enemies.json`, `Animations.json`, `Actors.json`) containing references to stock assets (`Skeleton1_1.png`, `Ship1.ogg`) that developers intentionally do not include in production because they are never placed on any map. Blind auditing marked all 571 unreferenced assets as fatal missing errors.
3. **Case Mismatch Fatalism:** Windows filesystem is case-insensitive, while Linux/Android is case-sensitive. The runtime Android VFS (`VfsResourceInterceptor.kt` and `GpackExtractor.kt`) already handles case-folding automatically. The static auditor should recognize that these assets are safely served by the VFS case-aliasing layer rather than reporting build-failing errors.
4. **Indeterminate Status on Encrypted Binary Archives:** Games packed inside binary archives (*Hello Charlotte EP1* with `Game.rgss3a`, *To the Moon* with `To the Moon.rgssad`, and *Mizuki And Me* with Ren'Py `.rpa`) were classified as `INDETERMINATE` because the static auditor did not inspect the contents of the archive tables in memory.

---

## 2. Core Principles & Architecture

### Principle 1: Zero Mutation on Input Games
The source directories in `Raw-Game/` remain strictly immutable. All parsing, decoding, reachability filtering, and archive index decryption occur in-memory.

### Principle 2: Zero Extraction Overhead (In-Memory Table Inspection)
Do not extract multi-gigabyte archives to temporary disk files. Parse archive index tables directly from the file headers in memory to discover internal files.

### Principle 3: Reachability-Driven Database Auditing
Assets referenced only by dormant database records that are not reachable from map events, common events, or troops are downgraded from fatal errors to `CMP_AUD_POSSIBLY_UNUSED` (Informational).

---

## 3. Detailed Component Design

### 3.1 Component 1: Shift-JIS / CP932 Scenario Text Decoding (Tyrano)
- **Target:** `engines/tyrano/src/asset_audit.rs` and `engines/tyrano/src/asar.rs`.
- **Implementation:**
  - Introduce `decode_scenario_text(bytes: &[u8]) -> String`:
    - First attempts standard UTF-8 validation. If valid UTF-8 and does not contain raw Shift-JIS multibyte sequences that produce invalid characters, use UTF-8.
    - If UTF-8 validation fails or produces `\u{FFFD}`, decode using `encoding_rs::SHIFT_JIS.decode(bytes)`.
  - Apply this decoder to all `.ks` scenarios, `.tjs` scripts, and scenario files inside ASAR containers.
- **Expected Outcome:** Eliminates all 40 `Target label '...' not found` findings in *show_off* and improves label resolution in *NTRdemic*.

### 3.2 Component 2: Reachability Graph Filter for RPG Maker MV/MZ Database
- **Target:** `engines/rpgmaker_web/src/asset_audit.rs`.
- **Implementation:**
  - Build a set of actively used database IDs:
    - Scan all `data/Map*.json` event pages for commands referencing enemies, animations, pictures, BGM, and sound effects.
    - Scan `data/CommonEvents.json` for active game mechanics and triggers.
    - Scan `data/Troops.json` for enemies that actually appear in encounters.
    - Check initial party in `data/System.json` for active actors.
  - When scanning `data/Enemies.json`, `data/Animations.json`, `data/Actors.json`:
    - If a referenced asset is missing but the referencing database entry is NOT in the active reachability set, emit `DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED` (Severity: Info) with requiredness `Optional`.
- **Expected Outcome:** Reduces 571 missing asset errors in *Termina* down to 0 fatal errors, turning them into informative warnings.

### 3.3 Component 3: VFS Auto-Aliasing Resolution for Case Mismatches
- **Target:** `core/src/asset_audit/resolver.rs` and `core/src/build_plan/audit_policy.rs`.
- **Implementation:**
  - In `AssetResolver`, when a candidate matches a file via case-folding (e.g. `occultist1_1.rpgmvp` -> `Occultist1_1.rpgmvp` or `jump1.ogg_` -> `Jump1.ogg_`), classify the resolution as `ResolutionResult::CaseMismatch`.
  - In `evaluate_audit_policy`, case mismatches that are automatically aliased and handled by the Android VFS decrypter layer do not cause an audit `FAIL`. They are recorded as auto-resolved warnings.
- **Expected Outcome:** Case mismatches in *Termina*, *Forestia*, and *The Witch's House MV* no longer fail the audit policy.

### 3.4 Component 4: In-Memory Encrypted Archive Table Readers
- **Target:** `engines/rgss/src/archive.rs` and `engines/renpy/src/archive.rs`.
- **RGSS Archive Parser (`.rgssad`, `.rgss2a`, `.rgss3a`):**
  - Read header magic `RGSSAD\0`.
  - Version 1/2 (XP/VX): Extract key from initial 4 bytes, decode file table entries (name length, filename, file size, offset) by XOR with rolling key.
  - Version 3 (VX Ace): Read key from offset, decode table entries.
  - Register internal file entries into an in-memory virtual file table for `GameSourceIndex`.
  - The static auditor can verify existence of `Data/System.rxdata`, `Data/Map*.rxdata`, audio, and graphics directly from the table.
- **Ren'Py Archive Parser (`.rpa`):**
  - Read header (e.g. `RPA-3.0 <offset> <key>`).
  - Read index bytes at `<offset>`, decompress with `flate2::read::ZlibDecoder`.
  - Parse file paths from the index dictionary.
  - Register all internal archive file paths into the source index.
- **Expected Outcome:** The audit outcome for *Hello Charlotte EP1*, *To the Moon*, and *Mizuki And Me* shifts from `INDETERMINATE` to fully inventoried `PASS`.

---

## 4. Testing & Verification Plan

1. **Unit & Integration Tests:**
   - `tyrano_asset_audit_test.rs`: Test Shift-JIS encoded `.ks` scenario with Japanese Kanji labels resolves target labels without errors.
   - `web_asset_audit_test.rs`: Test reachability filter on mock MV project with unused enemy `Skeleton1_1` in `Enemies.json` (not in any map) -> classified as `POSSIBLY_UNUSED` rather than `REQUIRED_REFERENCE_MISSING`.
   - `rgss_asset_audit_test.rs`: Test synthetic RGSSAD and RGSS3A archives parsed in-memory -> verifies internal files without disk extraction.
   - `renpy_asset_audit_test.rs`: Test synthetic RPA archive index decoded -> verifies internal paths without disk extraction.
2. **End-to-End Game Audits:**
   - Re-run `runtimepack audit` on all target games in `Raw-Game/`.
   - Assert that outcomes transition to clean/PASS or minor warnings.

---

## 5. Definition of Done

- All 4 components implemented cleanly in Rust.
- 100% of workspace tests pass (`cargo test --workspace` and Gradle unit tests).
- All 8 games re-audited and updated reports saved in `dist/<game>/asset-audit-report.json`.
- `Raw-Game/` input directories remain strictly untouched.
