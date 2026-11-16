# Bitmap Probe Contract & General Low-Level Exception Semantics (Approved Goal & Roles)

Goal: Engine-wide root fix for RGSS low-level Bitmap / Cache exception semantics. Preserve native `Bitmap.new` and `Cache.normal_bitmap` error semantics (do not swallow file-not-found into dummy bitmaps, allowing localization probes like Langscore Script 121 `ls_base_load_bitmap` to catch exceptions cleanly and fall back to base assets). Retain genuinely missing asset recovery only at public Cache typed endpoints (`Cache.<method>` / `RPG::Cache.<method>`), adding missing `:parallax` to `cache_methods`. Base genuinely missing assets continue to receive honest fallback (checkered tileset dummy, smart character marker, transparent dummy for parallax and general bitmaps) with `RuntimePackAssetAudit` logging. Raw `Bitmap.new` and `normal_bitmap` throw original exception on missing files. TDD mandatory using actual extracted production hooks. Package unique candidate beta APK `io.runtimepack.game.rpgmkingexit.diagnostic.bitmapfix` for QA A/B validation without touching original package, game source, or saves.

## Roles
- **Executor:** Implement TDD regression, apply minimal scoped product fix, verify suite, create candidate beta APK and artifacts.
- **Code Reviewer:** Independent review of diff, contract compliance, safety invariants, and test coverage before install.
- **QA Tester:** MuMu Player 12 / ADB validation (A/B side-by-side test with `...diagnostic.bitmaptrace`).

## Tasks & Status Ledger
- [x] **Task 1: Pre-edit baseline protection & TDD regression suite**
  - Saved pre-edit baseline copies and hashes in `Temp/opencode/mumu_bitmap_fix_20261019_005800/baseline_pre_edit`.
  - Wrote generic regression test `android/shell/app/src/test/ruby/bitmap_probe_contract_test.rb` loading actual production bootstrap hooks.
  - Test suites:
    1. Probe failure on localized candidate (`title_`) must propagate exception so caller falls back to base; must not cache dummy or set `$ls_graphic_cache = true`.
    2. When localized file actually exists, localized asset wins.
    3. Genuinely missing base asset at public Cache typed endpoint (`Cache.parallax`, `Cache.character`, `Cache.tileset`, `Cache.picture`) receives documented fallback (tileset 512x384 checkered, character > 32x32 marker, parallax 32x32 transparent) and records `RuntimePackAssetAudit` `FILE_EXISTS` `FAIL`.
    4. Direct raw `Bitmap.new` and `Cache.normal_bitmap` throw original exception (`Errno::ENOENT`) on real missing files; no dummy substitution at primitive level.
    5. Direct decode errors not swallowed at primitive level.
    6. Unlocalized standard game unaffected (loads normal assets, falls back on missing at typed endpoints).
    7. RGSS1 `RPG::Cache` and RGSS2/3 `Cache` idempotence, argument handling (`*args`, hue, multiple arguments).
    8. Case-insensitive path resolver invariance preserved.
  - Ran regression against baseline product code and OBSERVED RED (8 failures). Log: `test_red.log`.
- [x] **Task 2: Implement minimal scoped product fix in `runtimepack_bootstrap.rb` and `runtimepack_asset_audit.rb`**
  - In `runtimepack_bootstrap.rb`:
    - Added `:parallax` to `cache_methods`.
    - Removed swallowing hook from `Bitmap.new`.
    - Removed swallowing hook from `Cache.normal_bitmap`.
    - Kept public Cache typed endpoint wrapper with fallback and audit logging.
  - In `runtimepack_asset_audit.rb`:
    - Added `:parallax` to `cache_methods`.
    - Removed swallowing hook from `install_bitmap_hook!`.
    - Removed swallowing hook from `normal_bitmap`.
  - Updated static tests in `asset_audit_test.rb` and `input_hook_test.rb` that explicitly asserted old dummy semantics on raw `Bitmap.new` / `normal_bitmap` to align with the new documented contract.
  - Ran `ruby -c` on all modified files -> `Syntax OK`.
  - Ran all Ruby behavioral test suites and OBSERVED GREEN (54 passed in probe suite, 83 in audit suite, 96 in input hook suite). Log: `test_green.log`.
- [x] **Task 3: Workspace & Rust integration verification**
  - Ran `cargo test --test build_test`: 99 passed, 0 failed.
  - Ran `cargo test --workspace --no-fail-fast`: 643 passed, 1 failed (`test_windows_batch_wizard_contract` [known stale]), 2 ignored. Log: `cargo_workspace_test.log`.
- [x] **Task 4: Build candidate beta Android package (`...diagnostic.bitmapfix`)**
  - Created fresh candidate project reusing existing `game.pack`, native libs, and build config at `/home/mhsymr/mumu_bitmap_fix_20261019_005800`.
  - Target package ID: `io.runtimepack.game.rpgmkingexit.diagnostic.bitmapfix`.
  - Flavor: `--flavor beta`, `--content-mode packed`.
  - Updated canonical runtime integrity hash (`058d59263d048d41b0a9e1df20262c6d5c63662d9df9e8c2eaa06107e87c3dfb`) and 12 authoritative paths.
  - Compiled candidate APK via Gradle wrapper (`assembleDebug`): BUILD SUCCESSFUL in 3m 2s.
  - Verified APK structure: 138 total entries, 20 assets, 8 arm64 native libs, scheme v2 verified.
  - APK: `C:\Users\MhsymR\AppData\Local\Temp\opencode\mumu_bitmap_fix_20261019_005800\kingexit_diagnostic_bitmapfix.apk` (1,407,991,311 bytes, SHA-256: `a7b1a6d1070733edb4d84f37becd45847e3142885722cdc94a01db59f43fc109`).
- [x] **Task 5: Review package & Handover**
  - Generated scoped git patch diff against dirty baseline (1,031 lines unified diff): `review-package.diff`.
  - Prepared `executor-report.md` and `manifest.json` in `Temp/opencode/mumu_bitmap_fix_20261019_005800`.
  - Staged all artifacts for independent Code Reviewer before any device install.

## Review & QA Final Outcome
- **Independent Code Review:** APPROVED (`review-report.md`). Scoped diff verified: low-level primitive exception preservation, `:parallax` added to `cache_methods`, public typed endpoints maintain fallback and audit logging, zero arbitrary DLL/unpinned dependency introduction, immutable source respected.
- **Independent QA Testing:** PASS (`C:\Users\MhsymR\AppData\Local\Temp\opencode\mumu_bitmapfix_qa_20261019_012617\QA-report.md`).
  - Device: MuMu Player 12 (`127.0.0.1:16384`), Display `4619827144452474627` (Display #4, LayerStack 4).
  - Clean boot: 3,058 files extracted, SDL/mkxp-z booted without crash or monkeypatching.
  - Title Screen Visuals: Full artwork rendered from clean launch (1.589.441 non-black pixels, 76.65% screen coverage, color palette matched to `title_parallax.png` at `(134, 144, 178)`, `(134, 67, 87)`, `(246, 136, 149)`).
  - Cursor: Original diamond red cursor (`Graphics/Characters/casor.png`, 384x256) rendered at New Game; dummy marker sheet eliminated.
  - Langscore Contract Verified: Read-only inspection confirms `$ls_graphic_cache["title"] = false`, `@cache["Graphics/Parallaxes/title"]` = `[640, 480]`.
  - Input & Gameplay Progression: D-Pad RIGHT input shifted cursor from New Game (`Var 90 = 0`, 18.626 px change) to Continue (`Var 90 = 1`, 31.974 px change); confirm button A launched New Game, transferred from Map 56 to Map 24 (prologue), displayed Indonesian dialogue (`["看守", "「クソッ！振り切られた！」"]`), and button A advanced dialogue cleanly without crash in the observed window.
  - Foreground status: Beta `bitmapfix` left in foreground; original production package and saves remain untouched.
- **Forensic & Hypothesis Disclosures:**
  - Active vs Passive Evidence: The 6 `BITMAP_ERROR` events in previous `bitmaptrace` spike were active manual probe evaluations (`eval_helper`), whereas in candidate `bitmapfix`, clean boot, title screen, kursor, and dialog progression succeeded 100% passively end-to-end.
  - Web overlay / touch controller / GPU driver fault hypotheses completely disproved. Zero GPU fix, driver adjustment, or shader hack was required; the root cause was entirely Ruby loader error swallowing defeating Langscore probe fallback.
  - Release APK not rebuilt (candidate beta package only). No guarantee of 100% full-game completion, save/load, or battle system beyond observed prologue window.
