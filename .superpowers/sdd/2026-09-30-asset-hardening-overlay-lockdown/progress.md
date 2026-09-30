# SDD ledger — plan: Docs/superpowers/plans/2026-09-30-asset-hardening-overlay-lockdown.md

## Pre-flight

- Worktree: `.worktrees/asset-hardening`, branch `feature/asset-hardening`, BASE `404749c86ce3e2f5b5c92c8a3297a7e91d9165d2` (forked from clean master after merge of Track 3 + Save/Load + Stability Overhaul).
- Spec: `Docs/superpowers/plans/2026-09-30-asset-hardening-overlay-lockdown.md`, AGENTS.md §17 (asset hardening), memory decisions 2026-09-30.

## Conflict scan (pre-flight rulings)

| Pair / Task | Produces vs Consumes | Finding | Ruling |
| :--- | :--- | :--- | :--- |
| Task 1 vs Task 2 | Task 1 puts overlay/shim/fonts into GPACK (release); Task 2 `OverlayAssetSource` reads them from GPACK | Konsisten: entry names `overlay/*`, `web_shims/*`, `fonts/*` identik | Tidak ada konflik |
| Task 2 vs Task 3 | Task 2 reads entries; Task 3 verifies combined SHA-256 hash | Hash dihitung dari bytes yang sama persis (`OverlayAssetSource.read` bytes) | Konsisten |
| Task 3 vs Task 4 | Task 3 adds `RuntimeIntegrity` gate in `onCreate`; Task 4 enables R8 | R8 keep-rules wajib melindungi `RuntimeIntegrity` dan generated `RuntimeBuildInfo` | Wajib ditambahkan ke keep-rules di Task 4 |
| Task 5 | E2E device verification on release & beta | Membutuhkan MuMu running | Sesuai plan |

## Progress

Task 0: complete (master clean, save/load branch merged into master b4f1d1a, worktree feature/asset-hardening created at .worktrees/asset-hardening, BASE 404749c).

Task 1: complete (commit 777b5a1, executor ses_f0db21f1dffee6rDta3db9iwR0, review approved by ses_f0da122d8ffeTBwVWRjDVPJo9j)
- Release flavor bundles `overlay/*`, `web_shims/*`, and `fonts/*` into `game.pack` as encrypted chunks and omits them from `assets/`.
- Beta flavor retains plain assets without regression.
- 3 new integration tests in `build_test.rs` pass; all 357 cargo tests and 10/10 web_shims tests pass.

Task 2: complete (commit f6185e9, executor ses_f0d9e266cffeVa2WLEYDG7JM7w, review approved by ses_f0d95bbcfffesk3WcKYNEeiy3Y)
- Implemented `OverlayAssetSource` with GPACK streaming and plain asset fallback.
- Integrated into `WebActivity`, `RgssActivity`, and `RenpyActivity` with overlay webViewClient interception and shim fallback loading.
- 12/12 Kotlin unit test suites pass (including 4 new tests in `OverlayAssetSourceTest`); 10/10 web_shims test suites pass.

Task 3: complete (commits 54f2c2c..64ccac0, executor ses_f0d8176e3ffeQ3L5hfNpgIkgZd, re-review approved by ses_f0d7e4be3ffeIe75kqRXH4CJK9)
- Implemented deterministic SHA-256 calculation in Rust generator and Kotlin `RuntimeIntegrity`.
- Added `RuntimeBuildInfo.kt` template and emission, `activity_corrupted.xml` layout, and integrity check in `onCreate` across `WebActivity`, `RgssActivity`, and `RenpyActivity`.
- Fix round 1 addressed: `RgssActivity` check moved after `super.onCreate` and release mode with empty expected hash fails closed with `AND-403`.
- All 276 Cargo workspace tests, all Gradle Kotlin unit tests (8 RuntimeIntegrityTest cases), and 10/10 web_shims tests pass.

Task 4: complete (commit ff9875f, executor ses_f0d7ba9ddffemZL7nPnG0JVj22, review approved by ses_f0d75acb9ffehqiDqQGkdZVqDF)
- Enabled R8 minification (`isMinifyEnabled = true`) for release builds in `build.gradle.kts` and generator.
- Added comprehensive `proguard-rules.pro` keeping all `@JavascriptInterface` methods, Activities, JNI native methods, and RuntimeIntegrity classes.
- Verified Gradle `minifyReleaseWithR8` compiles successfully. All Cargo workspace tests (including 2 new build integration tests) pass.

Task 5: complete (commits ff9875f..049909f, executor ses_f0d7452baffeETeLJ2gPHs54xo)
- Generated release APK `dist_release/Forestia1.4.4-IND.apk` (818.15 MB, flavor: release).
- Verified release APK: `assets/overlay/`, `assets/web_shims/`, `assets/fonts/` are 100% ABSENT (0 plain files in APK).
- Verified GPACK container: all 8 overlay files, 2 web_shims files, and 7 fonts files are bundled and encrypted inside `game.pack`.
- Dynamic package resource resolution with programmatic fallback implemented in `RuntimeIntegrity.kt` to avoid R namespace mismatches in standalone packages.
- All 277 Rust workspace tests and Android unit tests pass.





