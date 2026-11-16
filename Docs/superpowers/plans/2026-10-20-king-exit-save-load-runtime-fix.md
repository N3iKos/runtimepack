# King Exit RGSS Save/Load Runtime Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the Android RGSS runtime so King Exit preserves the configured Langscore language after save/load and displays translated menu text instead of raw numeric script lookup keys. The original playtime-corruption hypothesis is refuted.

**Architecture:** Keep the game source and extracted content immutable. Correct the Android compatibility layer at the Win32 profile-API and RGSS timing boundaries, while preserving game-specific behavior through an overlay/runtime shim. The investigation must first distinguish a bad save value from a bad runtime formatter; no timer change is allowed until evidence identifies the failing boundary.

**Tech Stack:** Ruby RGSS bootstrap/shims, Android/Kotlin host storage, Rust/Cargo tests where relevant, Gradle beta APK packaging, read-only save inspection, MuMu/device validation by the user.

**Spec:** `Docs/superpowers/briefs/2026-10-20-king-exit-save-load-runtime-fix-brief.md`

## Execution status (2026-10-20)

- [x] Task 1: initial investigation delivered; timer attribution subsequently corrected.
- [x] Task 2: bounded INI API implemented, hardened twice, independently reviewed PASS.
- [x] Task 3: configured-language load ordering implemented, independently reviewed PASS.
- [x] Task 4 revised: static lookup keys identified and causal regression added (`dac490e`); no timer patch.
- [x] Task 5: final whole-change review SPEC PASS / QUALITY PASS, zero blockers.
- [x] Task 6: isolated beta build complete; fresh static artifact QA PASS.
- [x] Task 7: final automated QA/documentation/handoff complete; interactive device validation confirmed SUCCESS by user (*"sip dah normal"*). Save/load and menu localization fully resolved on device. Plan complete.

Detailed execution/recovery ledger: `.superpowers/sdd/2026-10-20-king-exit-save-load-runtime-fix/progress.md`.

## Global Constraints

- Treat the input game and extracted `game.pack` content as immutable; never patch source files in place.
- Preserve the RuntimePack build-time/runtime boundary; do not add game-specific logic to `core/`.
- Use app-specific Android storage; do not add broad external-storage permissions.
- Do not overwrite existing APKs, save files, game packs, signing keys, or user artifacts.
- Build test APKs with `--flavor beta` unless the user explicitly changes that requirement.
- Original-font behavior remains the global default; universal `--font-policy` CLI is not implemented and must not be invoked. Retaining the prior explicit diagnostic fontfix baseline does not change global defaults.
- Do not claim save/load is fixed from host tests alone; user performs final APK gameplay validation.
- Use stable diagnostics for compatibility failures; do not swallow errors that affect language or save semantics.
- Preserve original save files and use a new package/output directory for every test artifact.

## Review Focus

1. `GetPrivateProfileString` must use the actual Win32/RGSS argument contract, return the configured `Lang=en`, and respect the caller's default value when the INI key is absent.
2. `WritePrivateProfileString` must persist only to app-private configuration storage and must not report success after a failed write.
3. Language restoration must occur before Langscore reconstructs actor/map/menu state during `extract_save_contents`; unrelated games must not be forced to `en`.
4. Playtime diagnostics must distinguish a corrupt serialized save value from a runtime unit/formatter error before changing conversion logic.
5. Save/config path handling must remain safe for missing, malformed, read-only, and legacy files without overwriting user data.

---

### Task 1: Establish forensic baseline and failing regression cases

**Owner:** `bug-hunter` (read-only)

**Files:**
- Read: `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
- Read: `android/shell/app/src/main/assets/win32api_shim.rb`
- Read: `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`
- Read: `android/shell/app/src/main/java/io/runtimepack/host/SaveService.kt`
- Read: extracted King Exit `Game.ini`, `Data/Translate/*.csv`, and user-provided save copies only if available
- Report: `Docs/superpowers/reports/2026-10-20-king-exit-save-load-forensics.md`

**Interfaces:**
- Consumes: current runtime source and existing extracted package.
- Produces: exact call signatures, path/storage evidence, reproduction assumptions, and a confirmed or unresolved source for the oversized playtime value. No code changes.

- [ ] **Step 1: Trace the INI API call contract**

Record every `Win32API.new(..., 'GetPrivateProfileString', ...)` and `WritePrivateProfileString` call shape used by existing assets/shims. Identify buffer, buffer size, section, key, default, and file-path positions.

- [ ] **Step 2: Trace language state restoration**

Locate the Langscore initialization and save/load aliases in extracted script artifacts or available reports. Record when `Lang=en` is read, when `$langscore_current_language` can become `ja`, and whether restoration precedes actor/map/menu reconstruction.

- [ ] **Step 3: Trace playtime data flow**

Compare `Graphics.frame_count`, RGSS `playtime_s`, save header construction, and save-list rendering. Determine whether values such as `49037624:37:19` are serialized or generated at display time.

- [ ] **Step 4: Produce a read-only report**

Report observations, confirmed root cause, remaining uncertainty, exact files/lines, and the smallest discriminating test. Do not recommend a timer conversion without evidence.

**Verification:** Report must state `CONFIRMED`, `HYPOTHESIS`, or `BLOCKED` for each of language, INI persistence, and playtime.

---

### Task 2: Fix and regression-test Android INI compatibility

**Owner:** `executor`

**Files:**
- Modify: `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
- Modify: `android/shell/app/src/main/assets/win32api_shim.rb`
- Test: `android/shell/app/src/test/ruby/win32api_profile_test.rb` or the existing Ruby shim test harness selected after Task 1
- Test: `android/shell/app/src/test/java/io/runtimepack/host/...` only if Kotlin storage integration is required by the confirmed design

**Interfaces:**
- Consumes: Task 1's exact API contract and storage boundary.
- Produces: tested profile read/write behavior used by Langscore without game-specific language hardcoding.

- [ ] **Step 1: Write failing profile API tests**

Pin these cases:

- existing `Game.ini` with `[Langscore] Lang=en` returns `en`;
- missing key returns the caller-provided default;
- missing file returns the caller-provided default;
- buffer writes are NUL-safe and bounded;
- writes persist to the approved app-private configuration path;
- failed writes return failure rather than synthetic success;
- path traversal or non-game configuration paths are rejected or safely isolated according to the existing storage contract.

- [ ] **Step 2: Run the focused test and verify baseline failure**

Run the repository's Ruby test command for the new harness. Expected: the current stub either returns the wrong argument value, ignores the INI file, or reports successful writes without persistence.

- [ ] **Step 3: Implement the smallest profile compatibility layer**

Use the runtime's existing game-root/configuration path resolution. Parse only the INI operations required by the RGSS compatibility contract. Preserve section/key/default semantics and avoid forcing `Lang=en` globally.

- [ ] **Step 4: Run focused tests and existing Ruby regressions**

Run the new profile tests plus `bitmap_probe_contract_test.rb`, `asset_audit_test.rb`, `input_hook_test.rb`, `cheat_runner_test.rb`, and `stress_test_and_diag_test.rb`.

- [ ] **Step 5: Commit the task**

Commit only the profile shim and tests after the executor reports exact files, tests, and remaining limitations.

**Verification:** Focused profile tests pass; existing Ruby suites remain green; no source-game files change.

---

### Task 3: Restore Langscore language before save/load reconstruction

**Owner:** `executor`

**Files:**
- Modify: `android/shell/app/src/main/assets/runtimepack_bootstrap.rb` or the generated compatibility overlay selected by Task 1
- Test: Ruby regression fixture for language state ordering
- Test: extracted-script contract test if the project harness supports it

**Interfaces:**
- Consumes: Task 2's profile read/write behavior.
- Produces: a generic RGSS-compatible initialization hook that preserves each game's configured language before `extract_save_contents` performs translation restoration.

- [ ] **Step 1: Write a failing ordering test**

Model a save/load sequence where the configured language is `en`, the save temporarily contains original Japanese strings, and the post-load hook must restore `en` before actor/map/menu text is queried. Assert that no global `ja` fallback is used when the configuration is valid.

- [ ] **Step 2: Run the ordering test and verify baseline failure**

Expected: current shim/state ordering leaves the language at the default or restores it after translated objects are already reconstructed.

- [ ] **Step 3: Implement the ordering fix**

Restore language from the validated configuration at the earliest safe RGSS bootstrap point and reapply it before Langscore's load reconstruction. Do not alter translation CSVs or save payloads. Do not force the language for games without the Langscore configuration.

- [ ] **Step 4: Run focused language and existing runtime tests**

Verify valid `en`, missing configuration, malformed configuration, and non-Langscore RGSS behavior.

- [ ] **Step 5: Commit the task**

Commit the ordering fix and regression tests separately from the profile API fix.

**Verification:** Language-state tests pass; behavior is opt-in/config-driven; no translation asset is modified.

---

### Task 4: Resolve numeric menu-label corruption at the confirmed boundary

**2026-10-20 disposition:** Completed as forensic reinvestigation plus regression test (`dac490e`), not timer implementation. Exact screenshot strings byte-match `Data/Translate/Scripts.csv` lookup keys in `SCRIPT_ID:LINE:COLUMN` format. Historical INI shim produces empty language `""`; missing-language translation returns the input (Japanese database text or numeric script key). Tasks 2/3 correct that boundary. New regression covers boot/load, Indonesian and zh-cn, unchanged fixture save, and historical failure. No timer manipulation is warranted; real user's save bytes remain uninspected.

**2026-10-20 correction:** The screenshots show MAIN MENU command/help labels, not merely save slots. Strings with minute-like fields `67`/`84` cannot come from the displayed `%60` playtime formatter, and the inspected 32-bit native frame counter contradicts the proposed 10^13 raw-nanosecond explanation. Treat the old forensic report's timer/serialized-save causal attribution as UNVERIFIED. Reinvestigate translation/script-location keys and restore the actual text lookup boundary first. The historical timer steps below apply ONLY if new evidence independently confirms a timer defect; do not clamp or convert counters as a guessed fix.

**Owners:** `bug-hunter` (non-executing forensic evidence), `executor` (authored regression only).

**Files:**
- Report: `.superpowers/sdd/2026-10-20-king-exit-save-load-runtime-fix/task-4-reinvestigation.md`
- Test: `android/shell/app/src/test/ruby/langscore_menu_key_regression_test.rb`
- Brief/report: `task-4-regression-brief.md`, `task-4-regression-report.md` in the canonical SDD directory.

**Interfaces:**
- Consumes: real Langscore profile-call and missing-language lookup contracts; Tasks 2/3 production shims.
- Produces: evidence-backed regression reproducing numeric labels under historical shim and translated labels after the fix.

- [x] Identify exact numeric keys in `Scripts.csv` and static script callers without executing whole game.
- [x] Safely load copied historical shim from `10e2a92` and demonstrate zero-length INI result / empty-language / raw numeric keys.
- [x] Exercise current production shims at boot and pre-load with configured `en` and alternate `zh-cn`.
- [x] Verify authored neutral Japanese fixture save remains unchanged; do not claim actual user's unavailable save verified.
- [x] Commit owned regression test as `dac490e`; no timer conversion or save clamp.
- [ ] Independent final review of the causal regression and current product implementation (Task 5).

**Verification:** 100 assertions reported across bootstrap/standalone modes; final host rerun and independent final review are tracked separately.

---

### Task 5: Independent code review

**Owner:** `code-reviewer`

**Files:**
- Review all changed files and the Task 1 forensic report.
- Review tests, diagnostics, storage boundaries, and generated overlay behavior.

- [ ] **Step 1: Review for correctness and scope**

Check API argument semantics, configuration path safety, language ordering, timer conversion, source immutability, and cross-game behavior.

- [ ] **Step 2: Review test evidence**

Reject claims that depend only on compilation or host tests. Confirm that APK gameplay remains user-owned validation.

- [ ] **Step 3: Produce review report**

Classify findings as blocking, required before APK packaging, or acceptable disclosure.

**Verification:** No blocking findings remain before packaging.

---

### Task 6: Build a beta APK for user validation

**Owner:** `executor`

**Files:**
- Modify only generated build workspace/output; do not modify source game or existing APKs.
- Create: unique beta output directory with APK, manifest, checksums, and compatibility report.

- [ ] **Step 1: Validate source and output disjointness**

Confirm the extracted/game source path is read-only and the output directory is new and empty.

- [ ] **Step 2: Build with explicit beta flavor**

Use beta configuration and a new package ID/output name. Universal `--font-policy` is not implemented; do not invoke that flag. For this diagnostic candidate, preserve the previously tested fontfix substitution and outline as an explicit baseline to isolate the save/load repair without reintroducing known freeze; global original-font defaults remain unchanged. Follow `.superpowers/sdd/2026-10-20-king-exit-save-load-runtime-fix/task-6-packaging-brief.md`, retain scratch, and verify actual beta BuildInfo/marker if using an existing generated Gradle project.

- [ ] **Step 3: Verify artifact integrity**

Run build-manifest checks, SHA-256 verification, APK signature verification, and static permission checks. Preserve intermediate workspace on failure.

- [ ] **Step 4: Prepare the user test handoff**

Provide APK path and exact manual test sequence:

1. Start a new game or use a duplicated save.
2. Save to a new slot.
3. Load that new slot.
4. Confirm Indonesian actor/map/menu text.
5. Confirm playtime is normal.
6. Restart the APK.
7. Load again.
8. Confirm the same result.

The user performs this gameplay test; the agent must not claim runtime success before receiving the result.

**Verification:** APK artifacts pass static integrity/signature checks; gameplay status remains `PENDING_USER_DEVICE_VALIDATION`.

---

### Task 7: Final verification and documentation update

**Owner:** `qa-tester`, then supervisor integration

**Files:**
- Update: `Docs/reports/2026-10-19-king-exit-issues-and-fixes.md` or a new dated report
- Update: `memory/progress.md`, `memory/decisions_log.md`, `memory/technical_findings.md` as applicable
- Preserve: user test report and APK checksum

- [ ] **Step 1: Review user-provided gameplay result**

Record exact APK package, checksum, save slot procedure, language result, playtime result, and any remaining failure.

- [ ] **Step 2: Run final host verification**

Run focused Ruby tests, affected Android unit tests, and relevant Cargo/Gradle checks. Document the known stale Wizard test separately if it remains the only workspace failure.

- [ ] **Step 3: Update documentation**

State separately: code fixed, host tests passed, APK statically verified, and gameplay user validation status. Do not claim completion if the user's test is not passed.

- [ ] **Step 4: Final review gate**

No commit/push to `master`. Any branch commit/push decision remains a separate approved step after verification.

**Verification:** Final report distinguishes verified facts from user-pending validation and lists exact remaining limitations.
