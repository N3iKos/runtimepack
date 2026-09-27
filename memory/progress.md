# Progress

## Status saat ini (update setiap sesi)
- **Sedang kerjain apa:** Task 1 Plan All 8 Games PASS / Clean Static Audit Stabilization (Forestia Clean-Up: Disabled Plugins & Watermark URL Whitelist) selesai 100%.
- **Next step:** Task 2 (The Witch's House MV & Termina Clean-Up: MV Core Audio Auto-Provisioning & VFS Fallback Recognition).
- **Blocker:** Tidak ada blocker aktif.
- **Catatan fokus:** Hasil verifikasi static audit sweep menyeluruh terhadap 8 game target:
  1. *Hello Charlotte EP1* (RGSS3): **PASS** (0 findings, 5 nodes, 100% cataloged).
  2. *To the Moon* (RGSS1): **PASS** (0 findings, 5 nodes, 100% cataloged).
  3. *Forestia* (MZ): **PASS** (24 findings: 0 fatal errors, 0 indeterminate limitations, 23 case mismatches auto-aliased, 1 inactive plugin info `CMP-AUD-013`, linktr.ee/Veldra.Works URL whitelisted).
  4. *Mizuki And Me* (Ren'Py): **WARNING** (8 findings `CMP-106` desktop wrapper DLLs non-fatal, 0 missing assets, 0 bytecode/archive indeterminate, evaluasi policy langsung `Allow` dengan 0 overrides).
  5. *The Witch's House MV* (MV): FAIL (39 findings: 24 default database missing items didowngrade ke `CMP-AUD-013` Info; 10 event missing assets ditangani fallback VFS runtime, 5 case mismatches).
  6. *Fear & Hunger 2: Termina* (MV): FAIL (586 findings: 247 database entity errors didowngrade ke `CMP-AUD-013` Info via reachability graph filter; 324 missing legacy pictures/audio dan 15 case mismatches).
  7. *[Kimochi] show_off* (Tyrano): FAIL (145 findings: path traversal warnings `CMP-AUD-009` resmi 0; Shift-JIS decoding berhasil me-resolve Kanji labels; 16 placeholder button targets 未定, 24 missing scenario/asset references, 104 dynamic warnings).
  8. *[Kimochi] NTRdemic* (Tyrano): FAIL (532 findings: 438 missing assets/optional scenario references ditangani stub web_shims, 93 dynamic audio/command warnings, 1 `TYR-101` info).
  Seluruh 277 tests Cargo workspace lulus 100%.

## Selesai
- [2026-09-27] Task 1 Plan All 8 Games PASS (Forestia Clean-Up: Disabled Plugins & Watermark URL Whitelist) Selesai:
  1. `core/src/asset_audit/network.rs`:
     - Implementasi `is_whitelisted_static_url(url: &str) -> bool`: mengenali URL terpercaya komunitas `linktr.ee/veldra.works`, `veldra.works`, dan `linktr.ee`.
     - `audit_network_reference` dan `to_finding`: mengembalikan `Verified` dan melewati pembuatan finding `CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED` untuk URL yang berada di dalam whitelist.
  2. `engines/rpgmaker_web/src/asset_audit.rs`:
     - Pada `resolve_and_record`: saat memeriksa URL eksternal (`http://`, `https://`), jika URL terdaftar di `is_whitelisted_static_url`, tidak memicu finding `CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED` dan mencatat edge evidence sebagai `IntegrityVerified`.
     - Pada `audit_plugins`: saat memeriksa plugin, jika `status: false`, plugin diperlakukan dengan `ReferenceRequiredness::Optional`. Jika berkas plugin tidak ada di disk (`ResolutionResult::Missing`), diemisi sebagai `CMP_AUD_POSSIBLY_UNUSED` (`CMP-AUD-013`, severity `Info`) bukan `CMP-AUD-003` Warning. Transitive `@base` dependencies tidak dipindai untuk plugin inaktif.
     - Evaluasi outcome di Standard mode: jika seluruh temuan hanya terdiri dari `CMP_AUD_CASE_MISMATCH`, `CMP_AUD_POSSIBLY_UNUSED`, atau severity `Info`/`Informational`, outcome dievaluasi menjadi `AuditOutcome::Pass` (karena case-mismatch teresolusi otomatis oleh VFS runtime).
  3. `core/src/asset_audit/mod.rs` & `cache.rs`:
     - Menyelaraskan `recompute_outcome` pada Standard mode untuk engine `RpgMakerMv`/`RpgMakerMz` agar case mismatch tidak mengubah outcome menjadi `Warning`.
     - Menaikkan `CURRENT_AUDIT_RULESET_VERSION` ke 8.
  4. Integration tests & Live Audit:
     - Integration test `test_disabled_plugin_missing_does_not_fail_audit` di `web_asset_audit_test.rs` memverifikasi inactive dev plugin dan linktr.ee URL menghasilkan `AuditOutcome::Pass`.
     - Update `test_mv_case_mismatch_warning` dan `test_mv_transfer_case_mismatch_warning` untuk memvalidasi `AuditOutcome::Pass` pada Standard mode dan `AuditOutcome::Warning` pada Deep mode.
     - Verifikasi live `runtimepack audit` pada *Forestia*: Outcome sukses mencapai **PASS** (`dist/forestia/asset-audit-report.json`, 17.466 nodes, 7.146 references, 24 benign findings).
     - 33 tests di `web_asset_audit_test` dan seluruh 277 tests Cargo workspace lulus 100%.
- [2026-09-27] Task 6 Plan Remediasi Audit (Full Sweep Re-Audit & Verification across All 8 Games) Selesai:
  1. Pembersihan & Polish Sanitizer:
     - `core/src/asset_audit/report.rs`: Memperbaiki regex Windows path `win_re` pada `sanitize_message` dengan word boundary `\b[a-zA-Z]:[\\/]` agar substring URL `https://` tidak lagi keliru dicocokkan sebagai Windows drive letter `s://` (mencegah korupsi nama URL eksternal).
     - `engines/renpy/src/asset_audit.rs`: Mengecualikan modul bytecode standar library Ren'Py di direktori `renpy/common/*.rpyc` dari pemeriksaan orphan bytecode tanpa file sumber `.rpy` (karena runtime Ren'Py mendistribusikan precompiled bytecode resmi). Mengklasifikasikan ekspresi konfigurasi runtime `config.*` dan `renpy.*` pada statement `image` sebagai `RuntimeProvidedResource` (`runtime:<expr>`).
     - `core/src/asset_audit/cache.rs`: Menaikkan `CURRENT_AUDIT_RULESET_VERSION` ke versi 7 untuk auto-invalidation cache audit.
  2. Eksekusi Full Sweep Re-Audit 8 Game:
     - *Fear & Hunger 2: Termina*: `dist/termina/asset-audit-report.json` (117.124 nodes, 78.752 refs, 586 findings). 247 error database default didowngrade ke Info `CMP-AUD-013`.
     - *[Kimochi] NTRdemic*: `dist/ntrdemic/asset-audit-report.json` (1.960 nodes, 2.577 refs, 532 findings).
     - *[Kimochi] show_off*: `dist/show_off/asset-audit-report.json` (4.852 nodes, 8.332 refs, 145 findings). Shift-JIS text decoding berhasil membaca teks skenario Jepang; traversal `CMP-AUD-009` tetap 0.
     - *Mizuki And Me*: `dist/mizuki_and_me/asset-audit-report.json` (167 nodes, 120 refs, 8 findings, Outcome: **WARNING**). Seluruh arsip `.rpa` terindeks in-memory, bytecode standar Ren'Py terabaikan, dan status indeterminate `CMP-AUD-007` resmi turun menjadi 0.
     - *Forestia*: `dist/forestia/asset-audit-report.json` (17.466 nodes, 7.146 refs, 25 findings, Outcome: INDETERMINATE karena unverified external URL `https://linktr.ee/Veldra.Works` pada watermark guard dalam mode offline, 0 fatal errors).
     - *The Witch's House MV*: `dist/witchs_house/asset-audit-report.json` (9.853 nodes, 5.455 refs, 39 findings). 24 database missing items didowngrade ke Info `CMP-AUD-013`.
     - *Hello Charlotte EP1*: `dist/hc_ep1/asset-audit-report.json` (5 nodes, 0 refs, 0 findings, Outcome: **PASS** 100% clean).
     - *To the Moon*: `dist/to_the_moon/asset-audit-report.json` (5 nodes, 0 refs, 0 findings, Outcome: **PASS** 100% clean).
  3. Verifikasi Test Suite: Seluruh 276 tests Cargo workspace lulus 100%.
- [2026-09-27] Task 5 Plan Remediasi Audit (In-Memory Ren'Py RPA Archive Indexer `.rpa`) Selesai:
  1. `engines/renpy/src/archive.rs` & `engines/renpy/src/lib.rs`:
     - Implementasi `RpaArchiveIndex` mapping path ke `(offset, length)` dengan helper `has_file` dan `get_entry` case-insensitive & separator-normalized.
     - Implementasi `parse_rpa_index(&bytes)` dan `read_rpa_index_from_reader(&mut reader)` / `read_rpa_index_from_file(&path)`:
       - Header line parser: mem-parse `RPA-3.0 <offset_hex> <key_hex>` dan `RPA-2.0 <offset_hex> <key_hex>` atau `RPA-2.0 <offset_hex>`.
       - Seek ke hex offset dan decompress zlib payload via `flate2::read::ZlibDecoder`.
       - Parser Python pickle dictionary in-memory: mendukung opcode PROTO, FRAME, EMPTY_DICT, EMPTY_LIST, EMPTY_TUPLE, MARK, STOP, MEMOIZE, BINPUT, LONG_BINPUT, PUT, BINGET, LONG_BINGET, GET, GLOBAL, REDUCE, SHORT_BINUNICODE, BINUNICODE, BINUNICODE8, SHORT_BINSTRING, BINSTRING, STRING, UNICODE, SHORT_BINBYTES, BINBYTES, BININT1, BININT2, BININT, INT, LONG1, LONG4, TUPLE1/2/3/TUPLE, APPEND, APPENDS, SETITEM, SETITEMS, DICT.
       - Deobfuscasi XOR `offset ^ key` dan `len ^ key` dari tuple value segmen file.
       - Fallback regex scanner jika struktur pickle mengandung opcode kustom/eksotik.
     - Helper generator arsip sintetis `create_synthetic_rpa_v3` dan `create_synthetic_rpa_v2` untuk TDD.
  2. `engines/renpy/src/asset_audit.rs`:
     - Membaca arsip `.rpa` in-memory via `read_rpa_index_from_reader` / `parse_rpa_index`.
     - Mengisi `effective_source.files` dengan virtual `IndexedFile` entries (termasuk normalisasi prefix `game/` jika archive berada di folder `game/`).
     - Menginisialisasi `AssetResolver` dengan `&effective_source`.
     - Pada audit archive checks: menandai coverage `renpy-rpa-archive` sebagai covered dan mengeliminasi finding `CMP-AUD-007` (`CMP_AUD_LIMIT_REACHED`) untuk arsip yang berhasil diindeks.
     - Outcome dievaluasi ke `Pass` saat seluruh referensi aset skrip teresolusi di dalam arsip.
  3. Tests & Verifikasi:
     - Unit tests `test_synthetic_rpa_v3_parsing`, `test_synthetic_rpa_v2_parsing`, `test_parse_invalid_header_fails`, `test_parse_real_archives_if_present` di `archive.rs`.
     - Integration tests `test_synthetic_rpa_v3_archive_index_parsing`, `test_synthetic_rpa_v3_audits_pass_with_internal_assets`, dan `test_synthetic_rpa_v2_audits_pass_with_internal_assets` di `renpy_asset_audit_test.rs`.
     - Memverifikasi arsip dummy pada `test_renpy_rpa_archive_yields_indeterminate` tetap gagal secara aman (fail-closed) menghasilkan `Indeterminate`.
     - 30 tests di `renpy_asset_audit_test` dan seluruh 276 tests Cargo workspace lulus 100%.
- [2026-09-27] Task 4 Plan Remediasi Audit (In-Memory RGSS Archive Indexer `.rgssad`, `.rgss2a`, `.rgss3a`) Selesai (Commit `cb60b48`):
  1. `engines/rgss/src/archive.rs` & `engines/rgss/src/lib.rs`:
     - Implementasi `RgssArchiveIndex` dengan mapping file paths ke size/offset, serta method `has_file` dan `get_size` case-insensitive & path-normalized.
     - Implementasi `parse_rgss_archive_index(&bytes)` yang mengenali header magic `RGSSAD\0` (8 byte) dan `RGSSAD` (7 byte) versi 1, 2, dan 3.
     - RGSSAD v1/v2: Rolling XOR key `0xDEADCAFE` (progresi `key * 7 + 3`), membaca length, filename, file size, dan memajukan kursor melewati payload secara in-memory.
     - RGSS3A v3: Header key XOR `raw_key * 9 + 3`, membaca TOC entries (`offset`, `size`, `file_key`, `name_len`, `filename`), dan berhenti saat terminator `offset == 0` tercapai.
     - Helper generator arsip sintetis `create_synthetic_rgssad_v1` dan `create_synthetic_rgss3a_v3` untuk pengujian TDD.
  2. `engines/rgss/src/asset_audit.rs`:
     - Membaca arsip `.rgssad`, `.rgss2a`, `.rgss3a` dan mem-parse indeks in-memory via `parse_rgss_archive_index`.
     - Mengisi `effective_source.files` dengan virtual `IndexedFile` entries dari arsip.
     - Menginisialisasi `AssetResolver` dengan `&effective_source`.
     - Pada `audit()`: jika arsip berhasil di-index (`archive_parsed_successfully`), memeriksa kelengkapan `required_db_files` di `effective_source`.
     - `read_and_parse_data_file`: jika file berupa virtual entry di dalam arsip (`!full_path.exists()`), mengembalikan `None` tanpa mengemisikan `CMP_AUD_MALFORMED_RESOURCE`.
     - Encrypted archive check: jika `archive_parsed_successfully`, menambahkan `coverage.add_covered("encrypted_archive")` dan tidak mengemisikan finding unextracted `CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED`.
     - Outcome dievaluasi ke `Pass` jika seluruh file esensial ada di dalam arsip.
  3. Tests & Verifikasi:
     - Unit test `test_synthetic_rgssad_v1_parsing`, `test_synthetic_rgss3a_v3_parsing`, `test_parse_invalid_header_fails`, `test_parse_real_archives_if_present` di `archive.rs` (terkonfirmasi mem-parse 391 file di `HC_EP1` dan 979 file di `To the Moon` dalam 0.04 detik).
     - Integration test `test_synthetic_rgssad_v1_archive_audits_pass_without_indeterminate` dan `test_synthetic_rgss3a_v3_archive_audits_pass_without_indeterminate` di `rgss_asset_audit_test.rs`.
     - Verifikasi live `runtimepack audit`: *To the Moon* dan *HC_EP1* keduanya berhasil mencapai `Outcome: Pass`!
     - 29 tests di `rgss_asset_audit_test` dan seluruh 273 tests Cargo workspace lulus 100%.
- [2026-09-27] Task 3 Plan Remediasi Audit (VFS Auto-Aliasing Resolution for Case Mismatches in Policy) Selesai (Commit `aa0b28c`):
  1. `core/src/asset_audit/resolver.rs`:
     - Menambahkan helper `pub fn is_auto_aliased(&self) -> bool` pada `ResolutionResult` yang mengembalikan `true` untuk `ResolutionResult::CaseMismatch`.
     - Dokumentasi eksplisit bahwa case-folding built-in pada Android VFS (`VfsResourceInterceptor.kt` dan `GpackExtractor.kt`) menjamin case mismatch di disk teresolusi secara transparan dan aman saat runtime.
     - Menambahkan unit test `assert!(result.is_auto_aliased())` di `test_case_mismatch_resolution`.
  2. `core/src/build_plan/audit_policy.rs`:
     - Refaktor `evaluate_audit_policy`: jika laporan audit bertanda `AuditOutcome::Fail` namun temuannya hanya terdiri dari `CMP_AUD_CASE_MISMATCH` (atau non-blocking warnings/info) tanpa true blocking errors, `effective_outcome` diperlakukan sebagai `AuditOutcome::Warning`, menghasilkan `AuditPolicyDecision::Allow { overrides: vec![] }` tanpa memblokir build dan tanpa perlu `--accept-audit-fail`.
  3. `tests/integration/build_test.rs`:
     - Menambahkan integration test `test_audit_policy_case_mismatch_evaluates_to_allow_without_override`: memverifikasi report dengan outcome `Warning` dan `CMP-AUD-004` mengevaluasi ke `Allow` dengan 0 overrides.
     - Menambahkan integration test `test_audit_policy_case_mismatch_auto_aliased_when_report_outcome_is_fail`: memverifikasi report bertanda `Fail` yang temuannya hanya `CMP-AUD-004` otomatis teresolusi sebagai auto-aliased warnings dan mengevaluasi ke `Allow` dengan 0 overrides.
  4. Verifikasi: 60 tests di `runtimepack-core`, 48 tests di `build_test`, dan seluruh 269 tests workspace Cargo lulus 100%.
- [2026-09-27] Task 2 Plan Remediasi Audit (Reachability Graph Filter for RPG Maker MV/MZ Database) Selesai:
  1. `engines/rpgmaker_web/src/asset_audit.rs`:
     - Implementasi `DatabaseReachabilityIndex`: Melakukan static reachability scan sebelum database entity audit terhadap `Troops.json`, `System.json`, `Skills.json`, `Items.json`, `Weapons.json`, `Armors.json`, `CommonEvents.json`, dan `Map*.json`.
     - `audit_enemies`: Jika enemy ID tidak terdapat di troop manapun, aset battler yang hilang di-downgrade menjadi `CMP_AUD_POSSIBLY_UNUSED` (`CMP-AUD-013`, severity `Info`, requiredness `Optional`).
     - `audit_actors`: Jika actor ID tidak ada di starting party `partyMembers` dan tidak pernah ditambahkan via code 129, atau sideview battler `Actor1_1` / missing saat `optSideView == false`, emisi `CMP_AUD_POSSIBLY_UNUSED` (`CMP-AUD-013`, severity `Info`).
     - `audit_animations`: Jika animation ID tidak direferensikan oleh skill/item/weapon/event code 212, aset gambar/suara yang hilang di-downgrade menjadi `CMP_AUD_POSSIBLY_UNUSED` (`CMP-AUD-013`, severity `Info`).
     - `audit_system_json`: Kendaraan (`boat`, `ship`, `airship`) yang tidak ditempatkan (`startMapId <= 0` dan tidak ada code 206) jika BGM-nya hilang di-downgrade ke `CMP_AUD_POSSIBLY_UNUSED` (`CMP-AUD-013`, severity `Info`).
     - `core/src/asset_audit/cache.rs`: Menaikkan `CURRENT_AUDIT_RULESET_VERSION` ke 5.
  2. Integration tests `tests/integration/web_asset_audit_test.rs`:
     - `test_mv_unreferenced_database_entities_downgraded_to_possibly_unused_info`: Memverifikasi unreferenced database assets menghasilkan `CMP_AUD_POSSIBLY_UNUSED` dengan severity `Info` dan outcome `Pass`.
     - `test_mv_referenced_database_entities_missing_fails_audit`: Memverifikasi referenced database assets yang hilang tetap memicu `CMP_AUD_REQUIRED_REFERENCE_MISSING` dengan severity `Error` dan outcome `Fail`.
  3. Verifikasi: 32 tests di `web_asset_audit_test` dan seluruh 267 tests workspace Cargo lulus 100%.
- [2026-09-27] Task 1 Plan Remediasi Audit (Shift-JIS Scenario Auto-Decoding in Tyrano) Selesai (Commit `d989933`).
- [2026-09-27] Plan 8 (Validate RGSSAD Archive & Build *To the Moon*) Selesai:
  1. `engines/rgss/src/asset_audit.rs`:
     - Validasi header arsip `.rgssad` (magic `RGSSAD\0`, version byte = 1: `b"RGSSAD\0\x01"` atau legacy mock `b"RGSSAD\x01"`) dan `.rgss2a`. Menambahkan node `archive:<logical_path>` bertipe `AssetKind::ArchiveEntry`. Jika header corrupt/terpotong (< 7 bytes atau magic tidak cocok), mengemisikan `CMP_AUD_MALFORMED_RESOURCE` (`CMP-AUD-008`).
     - Font coverage: Memindai folder `Fonts/` pada `source.files`. Bundled fonts (`WenQuanYi_Zen_Hei.ttf`, `NanumBarunGothic.ttf`, `OpenSans.ttf`) terdaftar sebagai `AssetKind::Font` di dependency graph, dan coverage `fonts` ditandai sebagai covered.
  2. `android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt`:
     - Refaktor `extractArchiveSymlinks(destDir: File, finalExecName: String = "Game")`: menjamin pembuatan symlink/copy fallback untuk `Game$ext` dan `$finalExecName$ext` (misal `To the Moon.rgssad -> Game.rgssad` dan sebaliknya) untuk discovery mkxp-z.
  3. Integration & Unit Tests:
     - `rgss_asset_audit_test.rs`: Menambahkan `test_rgssad_valid_archive_and_bundled_fonts` dan `test_rgssad_corrupt_archive_emits_malformed_resource`.
     - `GpackExtractorTest.kt`: Menambahkan unit test JVM `testExtractArchiveSymlinksToTheMoon`, `testExtractArchiveSymlinksRgss3a`, `testExtractArchiveSymlinksWhenGameAlreadyExists`, dan `testExtractArchiveSymlinksBidirectionalFromGameToExecName`.
     - Seluruh 266 Cargo workspace tests dan 76 Android unit test tasks lulus 100%.
  4. Static Audit & Build APK:
     - `cargo run --bin runtimepack -- audit "Raw-Game\To the Moon" --mode standard --output dist/to_the_moon/asset-audit-report.json`: Outcome `Indeterminate` (terenkripsi `To the Moon.rgssad`), 4 graph nodes (1 archive, 3 fonts).
     - `cargo run --bin runtimepack -- build "Raw-Game\To the Moon" -o dist/to_the_moon --package com.example.tothemoon --name "To the Moon" --flavor beta --accept-audit-fail --accept-audit-unknown`: GPACK `game.pack` tercipta (115.47 MB, hash `88afa6de`), Gradle kompilasi `assembleDebug` sukses (exit code 0).
     - Menghasilkan `dist/to_the_moon/TotheMoon.apk` (162.467.180 bytes / 154.94 MB, SHA-256 `b0a20cbbb8ff22186d7b150522d4b8ea767fc469d6e753264e2a9744cf819332`).
- [2026-09-27] Plan 7 (Validate RGSS3A Archive & Build *Hello Charlotte EP1*) Selesai:
  1. `engines/rgss/src/asset_audit.rs`:
     - Validasi header arsip `.rgss3a`: Memeriksa magic `RGSSAD\0` dan version byte `3` (`b"RGSSAD\0\x03"` atau legacy mock `b"RGSSAD\x03"`). Menambahkan node `archive:<logical_path>` bertipe `AssetKind::ArchiveEntry`. Jika header corrupt/terpotong, mengemisikan `CMP_AUD_MALFORMED_RESOURCE` (`CMP-AUD-008`).
     - Font coverage: Memindai folder `Fonts/` pada `source.files`. Jika font `.ttf`/`.otf` ada (seperti `coders_crux.ttf`, `VL-Gothic-Regular.ttf`, `VL-PGothic-Regular.ttf`), menandai coverage `fonts` sebagai covered dan mendaftarkan node `font:<path>` bertipe `AssetKind::Font` ke dependency graph. Jika tidak ada dan CJK/custom font terdeteksi, menandai `fonts` sebagai unsupported dengan finding `CMP_AUD_REQUIRED_REFERENCE_MISSING`.
  2. Integration tests:
     - `test_rgss3a_valid_archive_and_bundled_fonts`: Memvalidasi header RGSS3A valid dan bundled fonts lolos tanpa malformed error dan terdaftar di dependency graph.
     - `test_rgss3a_corrupt_archive_emits_malformed_resource`: Memvalidasi header corrupt memicu `CMP_AUD_MALFORMED_RESOURCE` dan gagal (Fail).
     - Seluruh 264 Cargo workspace tests lulus 100%.
  3. Static Audit & Build APK:
     - `cargo run --bin runtimepack -- audit "Raw-Game\HC_EP1" --mode standard --output dist/hc_ep1/asset-audit-report.json`: Outcome `Indeterminate` (karena data game tersimpan dalam encrypted archive `Game.rgss3a`), 4 node graph (`Game.rgss3a`, 3 font).
     - `cargo run --bin runtimepack -- build "Raw-Game\HC_EP1" -o dist/hc_ep1 --package com.example.hellocharlotte1 --name "Hello Charlotte EP1" --flavor beta --accept-audit-fail --accept-audit-unknown`: GPACK `game.pack` tercipta (291.61 MB), Gradle kompilasi `assembleDebug` sukses (exit code 0).
     - Menghasilkan `dist/hc_ep1/HelloCharlotteEP1.apk` (347.375.247 bytes / 331.28 MB, SHA-256 `EA50A620C8A1BFECF9CDA43A562A3478EF945FF60BD8C948A02ED19357CDC8F7`).
  4. Commits: `8105df8`, `42c75ef`.
- [2026-09-27] Plan 6 (Stabilize & Build *The Witch's House MV*) Selesai:
  1. `engines/rpgmaker_web/src/asset_audit.rs`:
     - Menambahkan deteksi `optSideView` dari `System.json` ke `AuditContext`.
     - Pada `audit_actors` dan event command 322 (Change Actor Images): jika `!ctx.opt_side_view` atau nama battler adalah `"Actor1_1"`, mereferensikan aset battler `img/sv_actors/` sebagai `ReferenceRequiredness::Optional` (mengubah 21 false positive blocking error menjadi `CMP-AUD-003` Warning).
     - Resolusi tileset `img/tilesets/mori_A2` terhadap file terenkripsi di disk `mori_a2.rpgmvp` terkonfirmasi teresolusi bersih sebagai `CMP_AUD_CASE_MISMATCH` (`CMP-AUD-004` Warning).
     - Menaikkan `CURRENT_AUDIT_RULESET_VERSION` ke 4 di `core/src/asset_audit/cache.rs`.
  2. `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`:
     - Menambahkan `"rpgmvp"` dan `"rmmzimg"` ke `IMAGE_EXTENSIONS` dan `"rpgmvo"`, `"rpgmvm"`, `"rmmzaud"` ke `AUDIO_EXTENSIONS` agar aset terenkripsi yang hilang di-fallback secara anggun ke transparent PNG atau empty audio response.
     - Di `intercept(url)`: menambahkan case-folded lowercase fallback dan matching `reader.entries` case-insensitif untuk direct requests, decrypted redirects, dan audio fallbacks.
  3. `cli/src/main.rs`:
     - Menambahkan pembuatan alias APK tanpa apostrophe (`TheWitchsHouseMV.apk`) jika nama APK hasil sanitasi mengandung karakter `'` (`TheWitch'sHouseMV.apk`), dan menyinkronkan entri `checksums.txt`.
  4. Tests:
     - Integration test `test_mv_tileset_case_mismatch_and_disabled_sideview_battler_downgraded_to_optional` di `web_asset_audit_test.rs`.
     - Unit tests `testEncryptedTilesetCaseInsensitiveResolution` dan `test404EncryptedAssetReturnsPlaceholder` di `VfsResourceInterceptorTest.kt`.
     - Seluruh 262 tests Cargo workspace dan 24 Android unit test tasks lulus 100%.
  5. Static Audit & Build APK:
     - `cargo run --bin runtimepack -- audit "Raw-Game\The_Witchs_House_MV" --mode standard --output dist/witchs_house/asset-audit-report.json`
     - `cargo run --bin runtimepack -- build "Raw-Game\The_Witchs_House_MV" -o dist/witchs_house --package com.example.witchshouse --name "The Witch's House MV" --flavor beta --accept-audit-fail --accept-audit-unknown`
     - Menghasilkan `dist/witchs_house/TheWitchsHouseMV.apk` dan `TheWitch'sHouseMV.apk` (178.026.244 bytes / 169.78 MB, SHA-256 `19B8DAC893295B42DF467F5882BAD50CBDCC2CE686A725952698570D2D16BB9A`).
     - Commits: `cbdf0d5`, `f7f7b99`.
  1. `engines/rpgmaker_web/src/asset_audit.rs`:
     - Implementasi `find_subfolder_plugin_path`: jika plugin di `js/plugins.js` atau deklarasi `@base` tidak ditemukan di root `js/plugins/`, mencari di subdirektori `js/plugins/**/<name>.js` (exact case maupun case-folded).
     - Plugin subfolder diresolusi ke path aktual tanpa memicu `CMP_AUD_REQUIRED_REFERENCE_MISSING`.
     - `effective_enc_img` dan `effective_enc_audio` memeriksa flag `System.json` dan keberadaan file terenkripsi di disk, memperluas `image_exts` (`png_`, `rmmzimg`, `rpgmvp`) dan `audio_exts` (`ogg_`, `m4a_`, `rmmzaud`, `rpgmvo`, `rpgmvm`). Casing mismatch terenkripsi (e.g. `jump1.ogg_` vs `Jump1.ogg_`) diresolusi sebagai `CMP_AUD_CASE_MISMATCH` (warning).
  2. `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt` & `GpackStreamReader.kt`:
     - Expose `entries` di `GpackStreamReader`.
     - Di `intercept(url)`: saat request `js/plugins/*.js` mengembalikan stream null, mencari fallback entry di `reader.entries` yang berada di bawah `js/plugins/**/<filename>` dan menyajikannya secara transparan ke WebView.
  3. Tests:
     - Integration tests `test_mz_subfolder_plugin_and_transitive_base_dependency_resolved` dan `test_mz_encrypted_audio_and_image_case_mismatch_resolves_with_warning` di `web_asset_audit_test.rs`.
     - Unit test `testSubfolderPluginResolutionFallback` di `VfsResourceInterceptorTest.kt`.
     - Seluruh 261 tests Cargo workspace dan 24 Android unit test tasks lulus 100%. Commit: `e737997`.
- [2026-09-27] Task 2 Plan 4 (Build APK & Verify *Mizuki And Me*) Selesai:
  1. Static Asset Audit:
     - `cargo run --bin runtimepack -- audit "Raw-Game\Mizuki And Me" --mode standard --output dist/mizuki_and_me/asset-audit-report.json`
     - `engines/renpy/src/asset_audit.rs`: Pemindaian `.dll` diselaraskan dengan `RenPyAnalyzer::is_desktop_wrapper_dll`, mengklasifikasikan Windows PE DLLs di folder wrapper desktop Ren'Py sebagai `CMP_DESKTOP_EXCLUDED` (`Warning`, disposition `Warning`, requiredness `Optional`).
     - Modul stdlib Ren'Py diperluas mencakup `"pygame"`, `"emscripten"`, `"requests"`, `"urllib3"`, `"certifi"`, `"chardet"`, `"idna"` sebagai `RuntimeProvidedResource`.
     - `core/src/asset_audit/cache.rs`: `CURRENT_AUDIT_RULESET_VERSION` dinaikkan ke versi 3.
     - Laporan audit tuntas dengan outcome `Indeterminate` (0 missing assets, 0 fatal DLL blockers, 8 desktop-excluded DLLs).
  2. Gpack Desktop Exclusion & Directory Casing Normalization:
     - `pack/src/gpack.rs`: `should_include_in_pack` mengecualikan direktori desktop runtime `lib/py3-windows*`, `lib/py2-windows*`, `lib/windows*`, `lib/py3-linux*`, `lib/py2-linux*`, `lib/linux*`, serta varian `lib/*windows*`/`lib/*linux*`.
     - `validate_logical_path` menormalisasi direktori `Game/` menjadi `game/` pada entri indeks GPACK.
     - `android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt`: Menambahkan symlink dua arah (`game <-> Game`) pada `installRuntimeFiles` untuk menjamin kompatibilitas filesystem case-sensitive Linux/Android.
  3. Kompilasi & Verifikasi APK:
     - Command: `cargo run --bin runtimepack -- build "Raw-Game\Mizuki And Me" -o dist/mizuki_and_me --package com.cherrykiss.mizukiandme --name "Mizuki And Me" --flavor beta --accept-audit-fail --accept-audit-unknown`
     - GPACK `game.pack` tercipta (846.81 MB, hash: `51bcb8de`).
     - Gradle sukses kompilasi `assembleDebug` (exit code 0).
     - Menghasilkan `dist/mizuki_and_me/MizukiAndMe.apk` (929.536.351 bytes / 886.47 MB, SHA-256 `6313D2A7408B5212219240EC31CFDA64AE4325B28A414D65BB3EBBD9A2A85392`).
  4. Verifikasi Test Suite: 259 tests cargo workspace dan 24 Gradle Android unit test tasks lulus 100%.
- [2026-09-27] Task 1 Plan 4 (Desktop Library Exclusion & Ren'Py Built-in Syntax Parsing) Selesai:
  1. `engines/renpy/src/scanner.rs`:
     - Normalisasi path di `is_desktop_wrapper_dll` mengenali seluruh DLL/EXE di bawah `lib/` (`lib/py3-windows-x86_64/`, `lib/py2-windows-x86_64/`, `lib/windows-i686/`, `lib/windows-x86_64/`, dll.) dan `steam_api*.dll` sebagai `CMP_DESKTOP_EXCLUDED` (`Warning`), status kompatibilitas tetap `Compatible`.
     - Pemindaian versi Ren'Py diperluas mencakup file `.py` (`renpy/__init__.py`, `vc_version.py`).
  2. `engines/renpy/src/asset_audit.rs`:
     - Modul stdlib Python diperluas (`inspect`, `zipfile`, `ast`, `subprocess`, `ctypes`, `dataclasses`, `pathlib`, `codecs`, `shlex`, `posixpath`, `ntpath`, `genericpath`, `secrets`, `dis`, `token`, `tokenize`, `symtable`, `unittest`, `doctest`, `site`, dll.), `"ctypes"` dipindahkan dari `is_unsupported_native` ke `is_stdlib_or_renpy`, teresolusi sebagai `RuntimeProvidedResource`.
     - Implementasi `is_renpy_color_spec` dan `strip_scene_show_modifiers`: hex colors (`#000`, `#ffffff`), warna named (`black`, `white`), dan `Solid(...)`/`Color(...)` diresolusi sebagai `RuntimeProvidedResource` (`runtime:color:<spec>`), transisi (`with fade`, `at center`, dll.) dibersihkan secara deterministik.
  3. Verifikasi & Tests:
     - Integration test `test_renpy_builtins_and_color_displayables_and_stdlib_imports_resolve_as_runtime_provided` di `renpy_asset_audit_test.rs`.
     - Unit tests `test_is_desktop_wrapper_dll`, `test_is_renpy_color_spec`, dan `test_strip_scene_show_modifiers`.
     - Integration test `test_renpy_desktop_wrapper_dlls_excluded_and_not_fatal` di `compatibility_test.rs`.
     - Seluruh 259 tests cargo workspace lulus 100%. Commit: `7cad29a`.
- [2026-09-27] Task 2 Plan 3 (Build APK & Verify *[Kimochi] show_off*) Selesai:
  1. Static Asset Audit:
     - `cargo run --bin runtimepack -- audit "Raw-Game\[Kimochi] show_off" --mode standard --output dist/show_off/asset-audit-report.json`
     - Menghasilkan laporan audit 4.852 nodes, 8.332 references, 145 findings.
     - Path traversal warnings (`CMP-AUD-009`) resmi turun menjadi 0!
     - `CURRENT_AUDIT_RULESET_VERSION` dinaikkan ke versi 2 di `cache.rs`.
     - `engines/tyrano/src/asset_audit.rs`: `files.remove("resources/app.asar")` pada `build_effective_source` agar tidak memicu false limit reached `CMP-AUD-008`.
  2. Kompilasi & Verifikasi APK:
     - Command: `target\debug\runtimepack.exe build "Raw-Game\[Kimochi] show_off" -o dist/show_off --package com.example.showoff --name "Show Off" --flavor beta --accept-audit-fail --accept-audit-unknown`
     - Ekstraksi 1.613 files dari `resources/app.asar` ke staging directory sukses.
     - GPACK `game.pack` tercipta (860.03 MB, hash: `d3289b04`).
     - Gradle sukses kompilasi `assembleDebug` (exit code 0).
     - Menghasilkan `dist/show_off/ShowOff.apk` (943.594.630 bytes / 899.88 MB, SHA-256 `2dc853665298a7e3924f234fb1b4d85beab36a352aae7ada00854ee82c273204`).
  3. Verifikasi tests workspace: 257/257 Cargo unit and integration tests lulus 100%.
- [2026-09-27] Task 1 Plan 3 (Internal Relative Path Normalization in Tyrano Auditor & VFS) Selesai:
  1. `core/src/asset_audit/resolver.rs`:
     - Implementasi `pub fn normalize_relative_path(path: &str) -> Option<String>` yang mengonversi backslash ke slash, memvalidasi dan menolak null bytes, Windows drive letters, atau leading slash `/`, serta meng-collapse segmen `.` dan `..` secara aman. Mengembalikan `None` hanya jika traversal berusaha menembus batas atas direktori root game.
     - Mengubah `is_unsafe_path(path: &str) -> bool` menjadi `normalize_relative_path(path).is_none()`.
     - `CandidateRule::expand_candidates` dan `normalize_candidate` menormalisasi path internal sebelum pemetaan case-folding.
     - Memperbarui unit tests `test_middle_traversal_unsafe` dan `test_traversal_unsafe` serta menambahkan `test_normalize_relative_path` dan `test_internal_traversal_safe_resolution`.
  2. `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`:
     - Memperbarui `urlToLogicalPath(url: String)` untuk meng-collapse segmen `..` dan `.` secara internal dan mengembalikan `null` jika path mencoba keluar dari root game.
  3. Tests & Verifikasi:
     - Integration test `test_scenario_internal_relative_path_traversal_resolves_cleanly` di `tests/integration/tyrano_asset_audit_test.rs`.
     - Unit test `testInternalRelativePathNormalization` di `VfsResourceInterceptorTest.kt`.
     - Seluruh 257 tests cargo workspace dan 24 Gradle Android unit test tasks lulus 100%. Commit: `9498071`.
- [2026-09-27] Task 2 Plan 2 (Build APK & Verify *[Kimochi] NTRdemic*) Selesai:
  1. Static Asset Audit:
     - `runtimepack audit "Raw-Game\[Kimochi] NTRdemic - I Will Rescue Her Without Yielding to Any Temptation" --mode standard --output dist/ntrdemic/asset-audit-report.json`
     - Menghasilkan laporan audit 1.897 nodes, 2.482 references, 531 findings, outcome Fail. `TYR-101` dinilai secara non-blocking informational.
  2. Kompilasi & Verifikasi APK:
     - Command: `cargo run --bin runtimepack -- build "Raw-Game\[Kimochi] NTRdemic - I Will Rescue Her Without Yielding to Any Temptation" -o dist/ntrdemic --package com.example.ntrdemic --name "NTRdemic" --flavor beta --accept-audit-fail --accept-audit-unknown`
     - Ekstraksi 1.104 files dari `resources/app.asar` ke staging directory sukses.
     - GPACK `game.pack` tercipta (121.70 MB, hash `719f2263`).
     - Gradle sukses kompilasi `assembleDebug` (exit code 0).
     - Menghasilkan `dist/ntrdemic/NTRdemic.apk` (169.097.607 bytes / 161.26 MB, SHA-256 `72d1776c2b25660731b2d179799c81f50a6c6460e8275564478e582ff74712b0`).
- [2026-09-26] Task 1 Plan 2 (Audit & Dynamic Reference Handling for NTRdemic) Selesai:
  1. `engines/tyrano/src/asset_audit.rs`:
     - Implementasi `resolve_audio_reference`: otomatis mendeteksi ekspresi dinamis diawali `&` (`AssetKind::Audio` -> `CMP-AUD-006`) dan meresolusi audio statis (`data/sound/`, `data/se/`) dengan `CandidateRule`.
     - Dukungan tag `button`, `glink`, `link`: memeriksa atribut `graphic`, `enterimg`, `clickimg` (Image), `enterse`, `clickse`, `leavese` (Audio via `resolve_audio_reference`), serta `storage` & `target` (via `audit_jump_or_call`).
     - Dukungan tag `anim`: memeriksa `storage`, `graphic`, dan `target` (mendukung ekspresi dinamis `&`).
     - Penanganan `bg`, `image`, `chara_*`: mendukung fallback `storage` maupun `graphic` dengan sanitasi `trim()` dan klasifikasi dinamis `&`.
     - `audit_jump_or_call`: memeriksa `storage.trim().starts_with('&')` dan `target` dengan strip prefix `*` untuk mendeteksi `*&tf.target` maupun `&tf.target` sebagai `AssetKind::Command` dinamis.
     - Menambahkan `endlink` ke `known_tyrano_control_tags`.
  2. `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`:
     - Di `setupTyranoHooks`: Meng-intercept `kag.loadScenario` untuk mendeteksi ketiadaan file skenario atau skenario opsional (`cg.ks`, `replay.ks`, `extra.ks`, `gallery.ks`, `memory.ks`, `cg_view.ks`, `scene.ks`). Jika file tidak ada di storage/VFS, mengembalikan parsed mock scenario stub (`*start\n[cm]\n[clearfix]\n[back]\n[return]\n[jump storage="title.ks"]`) secara anggun tanpa melempar fatal error atau membekukan game.
  3. Integration & Unit Tests:
     - `tests/integration/tyrano_asset_audit_test.rs`: Menambahkan `test_dynamic_expression_handling_in_scenario` dan `test_button_link_and_anim_audio_and_dynamic_references`.
     - `android/shell/app/src/test/js/web_shims.test.js`: Menambahkan test suite untuk safety stub `kag.loadScenario("cg.ks")`.
     - Seluruh 28 integration tests Tyrano, test JS web shims, dan 256 cargo workspace tests lulus 100%.
- [2026-09-26] Task 3 Plan 1 (Stabilize & Build *Fear & Hunger 2: Termina*) Selesai:
  1. Static Asset Audit:
     - `runtimepack audit "Raw-Game\Fear & Hunger 2 Termina" --mode standard --output dist/termina/asset-audit-report.json`
     - Menghasilkan laporan audit 117.124 nodes, 78.752 references, 586 findings, outcome FAIL (karena 571 missing assets dan 15 case mismatches).
  2. Perbaikan XML Escaping di `core/src/build_plan/generator.rs`:
     - Implementasi `AndroidProjectGenerator::escape_xml_attribute` (`&` -> `&amp;`, `<` -> `&lt;`, `>` -> `&gt;`, `"` -> `&quot;`, `'` -> `&apos;`).
     - Mengatasi kegagalan Gradle task `:app:processDebugMainManifest` (`SAXParseException: The entity name must immediately follow the '&' in the entity reference`).
  3. Perbaikan APK Filename Derivation di `cli/src/main.rs`:
     - Implementasi `derive_apk_filename`: mencocokkan leaf direktori output dengan subtitle setelah colon (misal `"Fear & Hunger 2: Termina"` dengan output `dist/termina` menghasilkan `Termina.apk`), serta membersihkan karakter terlarang filesystem Windows/Unix (`/`, `\`, `:`, `*`, `?`, `"`, `<`, `>`, `|`) untuk mencegah bug Alternate Data Stream NTFS (`Fear&Hunger2:Termina.apk`).
  4. Kompilasi & Verifikasi APK:
     - Command: `cargo run --bin runtimepack -- build "Raw-Game\Fear & Hunger 2 Termina" -o dist/termina --package com.example.termina --name "Fear & Hunger 2: Termina" --flavor beta --accept-audit-fail --accept-audit-unknown`
     - Gradle sukses kompilasi `assembleDebug` (exit code 0).
     - Menghasilkan `dist/termina/Termina.apk` (1.608.088.782 bytes / 1533.59 MB, SHA-256 `f2dba81375521b64b01d2362408368fc34ae38f44385842b16a549d56f6071a4`).
     - Checksums dan build-manifest diperbarui sinkron.
  5. Verifikasi Test Suite: 254 tests workspace Cargo (unit & integration) lulus 100%.
- [2026-09-26] Task 2 Plan 1 (Gradle Project Name Sanitization in `generator.rs`) Selesai:
  1. `core/src/build_plan/generator.rs`:
     - Implementasi `pub fn sanitize_gradle_project_name(name: &str) -> String` pada `AndroidProjectGenerator` dan diekspor di level modul. Mengganti seluruh karakter non-alfanumerik/non-hyphen/non-underscore menjadi `_`, meng-collapse underscore beruntun, men-trim leading/trailing underscore, dan mengembalikan fallback `"RuntimePackGame"` jika tidak ada karakter alfanumerik.
     - Implementasi `pub fn generate_settings_gradle(project_name: &str) -> String` yang menggunakan `sanitize_gradle_project_name`.
     - `AndroidProjectGenerator::emit_project_with_source` menggunakan `generate_settings_gradle(&manifest.app.name)`.
     - Memastikan display name game (`manifest.app.name`) pada `AndroidManifest.xml` dan icon extraction tetap utuh tanpa perubahan.
  2. Integration tests & Unit tests:
     - Menambahkan `test_sanitize_gradle_project_name` dan `test_generate_settings_gradle` di `core/src/build_plan/generator.rs`.
     - Menambahkan `test_gradle_project_name_sanitization` dan `test_emit_project_settings_gradle_kts_sanitizes_root_project_name` di `tests/integration/build_test.rs`.
  3. Verifikasi: 45/45 tests `build_test`, 56/56 tests `runtimepack-core`, dan 252 tests workspace Cargo lulus 100%. Commit: `d8138e5`.
- [2026-09-26] Task 1 Plan Game Stabilization & Unblock Phase 0 (Remove Policy Hard Blockers `TYR-101` & Desktop DLLs `CMP-106`) Selesai:
  1. `core/src/diagnostics/mod.rs`: Menambahkan kode diagnostik `CMP_DESKTOP_EXCLUDED = "CMP-106"` dengan severity `Warning` untuk library desktop-specific wrapper/runner yang diabaikan pada packaging Android.
  2. `engines/renpy/src/scanner.rs`: Mengimplementasikan `is_desktop_wrapper_dll` untuk mengklasifikasikan Windows DLL pada direktori wrapper desktop (`lib/py3-windows-x86_64/`, `lib/windows-i686/`, `lib/windows-x86_64/`, `py3-windows-x86_64/`, `lib/python`, dll.) sebagai `CMP_DESKTOP_EXCLUDED` (`Warning`), bukan fatal blocker `CMP_NATIVE_DLL_DEPENDENCY` (`Error`). Menjamin status kompatibilitas Ren'Py tetap `Compatible`.
  3. `core/src/build_plan/audit_policy.rs` & `cli/src/main.rs`: Menghapus `TYR_LEGAL_GATE_BLOCKED` (`TYR-101`) dari daftar `hard_blockers`, menjadikannya informational note. Evaluasi audit policy dengan `--accept-audit-fail` / `--accept-audit-unknown` kini mengizinkan build TyranoScript berjalan tanpa terhalang.
  4. Integration tests: Menambahkan `test_renpy_desktop_wrapper_dlls_excluded_and_not_fatal` di `compatibility_test.rs`, memperbarui `audit_policy_never_bypasses_hard_blockers`, dan menambahkan `test_audit_policy_tyrano_legal_gate_not_hard_blocker` serta `test_build_tyrano_with_accept_audit_fail_succeeds_without_tyr_101_block` di `build_test.rs`. Seluruh 249 tests workspace lulus 100%.
- [2026-09-26] Task 2 Plan RPG Maker VX Ace RTP Auto-Provisioning & Native Asset Ingestion Selesai:
  1. `core/src/ingest/mod.rs` & `core/src/build_plan/mod.rs` & `schemas/build-manifest.schema.json`:
     - Menambahkan `rtp_root: Option<PathBuf>` dan accessor `rtp_path(&self) -> Option<&Path>` pada `GameSourceIndex`.
     - Menyimpan canonical path RTP saat auto-RTP atau explicit RTP berhasil di-ingest.
     - Menambahkan field `rtp_source: Option<String>` pada `BuildManifest` dan `ProvenanceManifestEntry` (`#[serde(default, skip_serializing_if = "Option::is_none")]`).
     - Memperbarui schema JSON `schemas/build-manifest.schema.json` untuk memasukkan `rtp_source` di level manifest dan provenance.
  2. `cli/src/audit_command.rs`:
     - Mengganti pemanggilan manual `build_with_rtp` dengan `GameSourceIndex::build_with_auto_rtp(source, rtp_path)` di `StaticAuditExecutionContext`.
     - Logging otomatis saat RTP terdeteksi atau dipassing: `println!("RTP resolved:   {} (auto-detected)", rtp_path.display())`.
     - Memperbarui `AuditSummary` dan validasi output path agar memperhitungkan auto-detected RTP.
  3. `cli/src/main.rs`:
     - Di `cmd_build`: Menggunakan `GameSourceIndex::build_with_auto_rtp(path, rtp_path)`.
     - Logging otomatis: `println!("RTP source:    {}", resolved_rtp.display())`.
     - Mengisi `rtp_source` di `BuildManifest` dan `ProvenanceManifestEntry` dengan path RTP yang digunakan.
  4. Integration tests `tests/integration/build_test.rs`:
     - Menambahkan test `test_build_auto_rtp_discovery_and_packaging`: memverifikasi project dengan deklarasi `RTP=RPGVXAce` di `Game.ini` otomatis menemukan RTP via env var `RPGVXACE_RTP_PATH`, memaketkan aset yang hilang ke `game.pack` tanpa flag `--rtp-path`, mencatat `rtp_source` di `build-manifest.json`, dan menjaga sumber game asli 100% immutable.
     - Memperbarui `test_build_manifest_schema_aligns_with_optional_audit` untuk memvalidasi ketiadaan field opsional `rtp_source` saat `None`.
  5. Verifikasi: 41/41 tests `build_test`, 16/16 tests `asset_audit_cli_test`, dan seluruh 246 tests workspace Cargo lulus 100%.
- [2026-09-26] Task 1 Plan RPG Maker VX Ace RTP Auto-Provisioning & Native Asset Ingestion Selesai:
  1. `core/src/ingest/rtp.rs` & `core/src/ingest/mod.rs` & `core/src/lib.rs`:
     - Implementasi `find_system_rtp_path(rtp_name: &str) -> Option<PathBuf>` dengan resolusi berurutan: env var (`{RTP_NAME}_RTP_PATH`), repository lokal (`runtimes/rtp/{rtp_name_lowercase}/`), Windows Common Files (`Enterbrain/RGSS3`, `RGSS2`, `RGSS`), dan Windows Registry query (`HKLM`/`HKCU` `SOFTWARE\WOW6432Node\Enterbrain\...`).
     - Helper `matches_rtp_name` untuk case-insensitive & alias resolution (`vxace`, `rpgvxace`, `vx`, `xp`, `standard`).
     - Parser `parse_ini_rtp_name` untuk section `[Game]` pada `Game.ini` membaca `RTP=...` / `rtp = ...` / `RTP1=...`.
     - Implementasi `GameSourceIndex::build_with_auto_rtp(game_dir, explicit_rtp)`: memprioritaskan `explicit_rtp` yang ada, mendeteksi RTP dari `Game.ini` secara otomatis, atau fallback ke indeks normal.
  2. Integration tests `tests/integration/asset_audit_cli_test.rs`:
     - `test_find_system_rtp_path_via_env_var`, `test_find_system_rtp_path_nonexistent`, `test_find_system_rtp_path_system_windows`.
     - `test_build_with_auto_rtp_without_rtp_declaration`, `test_build_with_auto_rtp_with_game_ini_declaration`, `test_build_with_auto_rtp_explicit_override`.
  3. Verifikasi: 16/16 tests `asset_audit_cli_test` dan seluruh test workspace Cargo (240+ tests) lulus 100%.
- [2026-09-26] Task 3 Plan Black Souls RTP & Rendering Fix (Subtractive Fog Plane Mitigations in OpenGL ES) Selesai:
  1. `runtimepack_bootstrap.rb`:
     - Deklarasi kelas fallback `Plane` (`unless defined?(Plane)`) lengkap dengan method standar RGSS (`viewport`, `bitmap`, `visible`, `z`, `ox`, `oy`, `zoom_x`, `zoom_y`, `opacity`, `blend_type`, `color`, `tone`, `dispose`, `disposed?`).
     - `RuntimePackCompatibility.install_plane_subtractive_fog_hooks`: Meng-intercept `blend_type=`, `opacity=`, dan `z=` pada `Plane`:
       - Saat `blend_type == 2` (subtractive blending) dan `z < 32` (fog layer di bawah karakter Z=32):
         - Jika `opacity > limit` (80 normal, atau 60 jika floor tileset hilang), membatasi nilai opacity ke limit (`self.opacity = limit`).
         - Mencatat notice diagnostik `puts "[RuntimePack] Subtractive fog Plane mitigation: clamped opacity to ..."` dan menyimpannya di `RuntimePackCompatibility.plane_mitigations`.
       - Intersepsi `Plane#opacity=` dan `Plane#z=` menjamin clamping tetap ditegakkan saat opacity dinaikkan kembali atau Z diturunkan ke bawah 32.
  2. `asset_audit_test.rb`:
     - Test 11: Memverifikasi `Plane#blend_type = 2` dengan `z = 10` membatasi opacity ke <= 80, mencatat notice mitigasi, menguji intersepsi `opacity=` saat `blend_type == 2`, menguji relaksasi opacity saat `z >= 32` atau `blend_type == 0`, serta memastikan clamp 80 saat floor tidak hilang.
     - Seluruh 88 tests `asset_audit_test.rb`, 100 tests `input_hook_test.rb`, 25 tests `cheat_runner_test.rb`, 27 tests `stress_test_and_diag_test.rb`, 27 tests Cargo `runtimepack-engine-rgss`, dan full JS test suite lulus 100%.
- [2026-09-26] Task 2 Plan Black Souls RTP & Rendering Fix (Smart Autotile & Character Fallbacks in RGSS Bootstrap) Selesai:
  1. `runtimepack_bootstrap.rb` & `runtimepack_asset_audit.rb`:
     - `create_tileset_fallback(name)`: Menghasilkan 512x384 bitmap berpola checkered gelap (RGB 40,40,40 dan 70,70,70) 32x32 dengan border halus 1px (RGB 30,30,30) di sekeliling tiap tile agar autotile dan floor blits terhindar dari void transparan / pitch-black.
     - `create_character_fallback(path_or_name)`: Menghasilkan 128x128 3x4 multi-frame sprite sheet:
       - Objek interaktif `!` (`!Flame`, `!Chest`, `!Door`): Mengisi tiap frame dengan visual yang jelas (pedestal brazier flame kuning emas untuk `!Flame`/Save, kotak kayu cokelat dengan gembok emas dan strip besi untuk `!Chest`, bingkai kayu berpintu dan pegangan kuningan untuk `!Door`, atau cyan marker box bertanda seru untuk objek `!` generik).
       - Karakter standar tanpa `!`: Menghasilkan 128x128 3x4 sprite sheet dengan siluet slate humanoid (kepala berwajah/rambut, mata direksional sesuai arah hadap, torso beroutline, dan animasi langkah kaki).
     - Hook `normal_bitmap` dan `Bitmap.new` (serta `Cache.tileset` dan `Cache.character`) secara konsisten memanggil fallback cerdas ini dan mencatat `TILESET_FALLBACK_DUMMY` atau `CHARACTER_FALLBACK_DUMMY`.
  2. `asset_audit_test.rb` & `input_hook_test.rb`:
     - Test 10 di `asset_audit_test.rb`: Memvalidasi `create_tileset_fallback` menghasilkan 512x384, `create_character_fallback` menghasilkan multi-frame bitmap > 32x32 untuk `!Flame` dan `!Chest`, standard character menghasilkan 128x128, serta fallback otomatis via `Bitmap.new` dan `Cache.normal_bitmap`.
     - Seluruh 75 tests `asset_audit_test.rb`, 100 tests `input_hook_test.rb`, 25 tests `cheat_runner_test.rb`, 27 tests `stress_test_and_diag_test.rb`, 23 tests `rgss_asset_audit_test`, dan full JS test suite (6/6 suites) lulus 100%.
- [2026-09-26] Task 1 Plan Black Souls RTP Ingestion (`--rtp-path`) Selesai:
  1. `core/src/ingest/mod.rs`:
     - Menambahkan method `GameSourceIndex::build_with_rtp(game_dir: &Path, rtp_dir: Option<&Path>) -> Result<Self, IngestError>`.
     - `IndexedFile` diperluas dengan field `pub source_root: Option<PathBuf>` dan helper `real_path(&default_root)`. Aset RTP dipetakan dengan `source_root = Some(canonical_rtp)`.
     - File yang tidak ada di `game_dir` (atau case-folded matching) diambil dari RTP tanpa memodifikasi direktori `game_dir`.
     - Hash sumber `source_hash` dan `total_size_bytes` otomatis menyertakan aset RTP secara deterministik.
  2. `pack/src/gpack.rs` & `pack/src/lib.rs`:
     - Menambahkan method `GpackPacker::pack_from_index` dan `pack_from_index_with_options` serta metode trait pada `ContentPacker`.
     - Saat memaketkan berkas ke `game.pack`, membaca path absolut aktual via `p.file_info.real_path(&index.root_path)`.
  3. CLI plumbing (`cli/src/main.rs` & `cli/src/audit_command.rs`):
     - Menambahkan argumen CLI `--rtp-path <DIR>` ke `runtimepack audit` dan `runtimepack build`.
     - `audit_command::run` dan `cmd_build` menggunakan `GameSourceIndex::build_with_rtp`.
     - `cmd_build` memaketkan seluruh aset hasil index (termasuk RTP) langsung ke dalam `game.pack`.
  4. Integration tests:
     - `tests/integration/asset_audit_cli_test.rs`: `audit_resolves_missing_assets_with_rtp_path` (10/10 tests passing).
     - `tests/integration/build_test.rs`: `test_build_with_rtp_path_packages_rtp_assets` (40/40 tests passing).
     - Seluruh test workspace Cargo (240+ tests) lulus 100%.
- [2026-09-26] Task 2 Plan Real-Time Diagnostic Stress Test Reporting Selesai:
  1. Real-time Diagnostic Overlay Update (`diagnostic_suite.js` & `preview/diagnostic_suite.js`):
     - Baris `MAP:` dan `DATABASE:` tidak lagi menginkremen `scannedCount` atau `validCount` secara salah.
     - Baris `FAIL:` / `ERROR:` secara live menginkremen `failureCount++`, `missingCount++`, `scannedCount++`, memperbarui live UI counters, dan menampilkan badge `ISSUES (N)` berlatar merah `#dc2626`.
     - Baris `DONE:` mem-parse angka pasti `maps`, `exact assets`, `case mismatches`, `missing` via regex atau fallback ke `getAuditReport()`, menyinkronkan UI counters ke angka otoritatif (`scanned = exact + case + missing`, `valid = exact`, `missing = missing`).
     - Jika `missingCount > 0` atau `failureCount > 0`, status badge dipaksa menjadi `FAILED` (merah `#dc2626`), pesan status menampilkan jumlah missing assets, dan log dicatat sebagai `WARN` (tanpa log fake `SUCCESS`).
  2. Sinkronisasi 100% byte-for-byte antara `android/shell/app/src/main/assets/overlay/diagnostic_suite.js` dan `preview/diagnostic_suite.js` (SHA-256: `EF71198072F292B512F80D2889E280C7B5F4AE58C941BBA0CC70C7721145C666`).
  3. Tests: Test 13 di `diagnostic_suite.test.js` memverifikasi `MAP:` no-op counter, `FAIL:` live streaming, `DONE:` regex parsing, dan red `FAILED` badge / `WARN` log level. Seluruh 13 test JS lulus 100%.
- [2026-09-26] Task 1 Plan Real-Time Diagnostic Stress Test Reporting Selesai:
  1. Real-time Missing Asset Streaming (`runtimepack_bootstrap.rb`): Di dalam `run_stress_test`, saat `verify_asset` mendeteksi missing asset (`when "MISSING"`), langsung memancarkan `report_stress_progress("FAIL: Missing #{asset_type}: #{base_path}")`. Saat ada broken transfer terdeteksi, memancarkan `report_stress_progress("FAIL: Broken transfer to Map #{t[:target_map]} at #{t[:source]}")`. Format baris penyelesaian dipastikan `"DONE: Stress test completed: #{stats['total_maps_scanned']} maps, #{stats['exact_count']} exact assets, #{stats['case_mismatch_count']} case mismatches, #{stats['missing_count']} missing."`.
  2. Report Fallback di RgssActivity (`RgssActivity.kt`): Saat `AssetAuditService.getAuditReport()` kosong atau `"{}"`, membaca dan mengembalikan langsung konten `audit_report.json` dari `getInputStateDirectory()` atau `game/audit_report.json`.
  3. Tests: Unit test `asset_audit_test.rb` (57/57 tests passing) dan Android Gradle unit test `AssetAuditBridgeTest.kt` (3/3 tests passing, total 24 actionable tasks) lulus 100%.
- [2026-09-26] Task 3 Plan Black Souls 2 (Missing Floor Tileset & Darkness Occlusion Detection) Selesai:
  1. Map Layer 0 & 1 Tileset Inspection (`engines/rgss/src/asset_audit.rs`): Parsing map data Table 3D (`width x height x depth`) dan pemetaan range tile ID RPG Maker VX/VX Ace ke slot tileset (A1 2048..2815, A2 2816..4351, A3 4352..5887, A4 5888..7423, A5 1536..2047, B 1..255, C 256..511, D 512..767, E 768..1023). Jika tile ditempatkan pada Layer 0 atau Layer 1 namun berkas tileset pada `Graphics/Tilesets/` tidak ada di disk, emisi finding `CMP_AUD_REQUIRED_REFERENCE_MISSING`: `"Map {map_id} Layer {layer} has {count} tiles referencing missing tileset '{tileset_name}'; floor/walls will render transparent/black"`.
  2. Subtractive Fog Occlusion Tag Detection (`engines/rgss/src/asset_audit.rs`): Memeriksa note tag map terhadap pola `==マップフォグ.*\[.*,.*,(\d+),.*,2\]==`. Jika Z-layer < 32 (di bawah layer karakter), emisi warning `CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED`: `"Map {map_id} has subtractive fog Plane at Z={z} (below character layer Z=32); may cause black occlusion under OpenGL ES"`.
  3. Tileset Fallback Pattern & Diagnostics (`runtimepack_bootstrap.rb`, `runtimepack_asset_audit.rb`): Di `Cache.normal_bitmap` dan `Bitmap.new`, saat tileset gagal dimuat (`path.include?("Graphics/Tilesets/")`), catat event audit `TILESET_FALLBACK_DUMMY` dengan pesan `"RTP tileset missing; Layer 0/1 map tiles will render transparent/black"`, serta hasilkan fallback bitmap 512x384 dengan pola checkered grid gelap (24, 24, 28) dan (40, 40, 46) 32x32 agar sampling tile tidak menghasilkan void transparan total.
  4. Tests: 23/23 tests `rgss_asset_audit_test`, 50/50 unit tests Ruby `asset_audit_test.rb`, 100/100 tests `input_hook_test.rb`, dan 239/239 cargo tests workspace lulus 100%.
- [2026-09-26] Task 2 Plan Black Souls 2 (Font & Glyph Coverage Audit + CJK Font Provisioning) Selesai:
  1. Font Bundling: Menyalin `VL-Gothic-Regular.ttf` ke `android/shell/app/src/main/assets/fonts/VL-Gothic-Regular.ttf`.
  2. Build Generator (`core/src/build_plan/generator.rs`): Menyalin `assets/fonts` ke Android project output `app/src/main/assets/fonts`.
  3. Gpack Extractor (`GpackExtractor.kt`): Mengekstrak fallback CJK font `fonts/VL-Gothic-Regular.ttf` ke `<destDir>/Fonts/` jika folder `Fonts/` belum ada atau kosong.
  4. Font Substitutions (`GpackExtractor.kt`): Mengarahkan `defaultFontSubs` (`"UmePlus Gothic"`, `"VL PGothic"`, `"MS Gothic"`, `"MS Mincho"`) ke `"VL Gothic"`.
  5. RGSS Static Asset Audit (`engines/rgss/src/asset_audit.rs`): Memindai teks dialog/event map (code 401, 102, 108/408, 355/655), entitas database, dan Ruby scripts (`Data/Scripts.*`) untuk tanda petik Jepang (`「`, `」`), pungtuasi CJK, dan font declarations (`Font.default_name`, `VL Gothic`, `UmePlus Gothic`). Jika terdeteksi namun `<source>/Fonts/` kosong/tidak ada, mengemisikan `CMP_AUD_REQUIRED_REFERENCE_MISSING` fail-closed.
  6. Runtime Font Probe (`runtimepack_asset_audit.rb`): Menambahkan `probe_fonts!` untuk memverifikasi ketersediaan font via `Font.exist?` dan mencatat event `FONT_FALLBACK_LIBERATION_SANS` saat font tidak ditemukan.
  7. Verification: 21/21 integration tests `rgss_asset_audit_test`, 39/39 integration tests `build_test`, 31/31 unit tests Ruby `asset_audit_test.rb`, dan 24 Gradle Android unit tasks lulus 100%.
- [2026-09-26] Task 1 Plan Black Souls 2 (Probe Asset Discovery & Media Filter Fix) Selesai:
  1. Build-time generator (`core/src/build_plan/generator.rs`) dan `cli/src/main.rs`: menyalin `asset-dependency-graph.json` dan `asset-audit-report.json` ke APK assets (`src/main/assets/`).
  2. `AssetProbeRunner.kt`: mendukung pemuatan `asset-dependency-graph.json` langsung dari Android assets via `Context.assets` (fallback disk).
  3. `AssetProbeRunner.kt`: fallback directory discovery strictly memfilter file non-media (`.bin`, `.rb`, `.txt`, `.json`, `.tmp`, `.bak`, `.ds_store`, `.gitignore`, `.gpack_extracted`) dan hanya mengizinkan resource/media valid (`.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.ogg`, `.wav`, `.mp3`, `.m4a`, `.webm`, `.mp4`, `.ttf`, `.otf`, `.woff`, `.efkefc`, `.rxdata`, `.rvdata`, `.rvdata2`).
  4. Stage 4: hanya mengemisikan `DECODE_SUCCESS` untuk berkas gambar (`isImageExtension(relPath)` yang lolos decode bitmap); berkas media non-gambar (audio, fonts, data) mengemisikan `FILE_READABLE`.
  5. Integration & Unit Tests: `test_emit_project_packages_asset_dependency_graph` di `tests/integration/build_test.rs` (38/38 passing) dan 2 test baru di `AssetProbeRunnerTest.kt` (10/10 passing). Seluruh Gradle unit tests dan Cargo tests lulus 100%.
- [2026-09-25] Plan 4 Task 3 (Sampled & Exhaustive Runtime Probe Engine with Checkpoints + Fix Round 1 & Polish) Selesai:
  1. Review Polish: Dynamic scaling `maxPollCount` (260 polls / ~65s untuk exhaustive probe & stress test vs 60 polls / 15s untuk sampled) di `diagnostic_suite.js` (byte-for-byte identical, SHA-256: `A90BA6CA0C50C8D983F857184E717AB0324D12DC4CBE985F13A467CA9FFD84F5`), pembatasan retry timer hook di `runtimepack_asset_audit.js` (max 100 attempts / 5s), evaluasi `finalOutcome = INDETERMINATE` untuk probe 0 aset, dan auto-delete `probe_checkpoint.json` saat probe tuntas sukses.
  2. Fix Round 1: Standardisasi path output probe ke `filesDir/runtime-state/audit/` (`probe_checkpoint.json`, `runtime-validation-report.json`, `combined-validation-summary.json`), dukungan packaging `game/` subdirectory di `AssetAuditService.startAudit()`, penambahan `is_complete` ke schema `runtime-validation-report.schema.json`, penggantian fixed mark buffer pada `DefaultBitmapProbeDecoder` dengan `readBytes()` + `decodeByteArray()`, thread executor daemon dengan uncaught exception handler di `AssetAuditService.kt`, update `doneEv` message mencerminkan failure count/status, dan pencegahan inflasi live counter pada event sintetis `asset_path = "root"`.
  3. Implementasi `AssetProbeRunner.kt`: probe mode `SAMPLED` dan `EXHAUSTIVE`, deterministik per kategori aset, sequential decode dengan `Bitmap.recycle()`, crash checkpointing ke `filesDir/runtime-state/audit/probe_checkpoint.json`, auto-resume dari checkpoint, bounded timeout, dan clean cancellation.
  4. Pembuatan JSON schemas: `schemas/runtime-validation-report.schema.json` dan `schemas/combined-validation-summary.schema.json` serta persistensi atomik kedua laporan di `filesDir/runtime-state/audit/`.
  5. Integrasi Diagnostic Suite overlay: tab di-rename menjadi "Asset Audit & Probe", penambahan tombol `[ Run Sampled Probe ]` dan `[ Run Exhaustive Probe ]`, counter live otentik (Scanned, Valid, Missing, Fallback), dan integrasi `combined_validation_summary` ke dalam ekspor bug report.
  6. Dokumentasi kanonikal diperbarui di `Docs/TEST_PLAN.md` (Section 16.4) dan `Docs/RUNTIME_CONTRACT.md` (Section 11).
  7. Unit tests lulus: `AssetProbeRunnerTest.kt` (8 test cases), `diagnostic_suite.test.js` (12 test cases), seluruh 67 unit test Android Gradle, dan seluruh 210 cargo test workspace.
- [2026-09-25] Plan 4 Task 2 (Multi-Engine Loader Instrumentation & Activity Bridges) Selesai:
  1. RGSS loader instrumentation: `runtimepack_asset_audit.rb`, `runtimepack_bootstrap.rb` dummy fallback 32x32, audio suppression.
  2. Web loader instrumentation: `runtimepack_asset_audit.js` intercepting `ImageManager`, `AudioManager`, `Graphics.playVideo`.
  3. Activity bridges: `RgssActivity` & `WebActivity` expose audit methods.
  4. Build plan generator packaging scripts into output project assets.
- [2026-09-25] Plan 4 Task 1 (Runtime Audit Protocol, Native VFS Evidence & In-Game Overlay Cleanup + Fix Round 1) Selesai:
  1. Implementasi model data domain audit runtime di `AssetAuditModels.kt`: `RuntimeAuditStage`, `RuntimeAuditOutcome`, `RuntimeAuditEvent`, dan `RuntimeAuditReport` dengan serializer JSON mandiri.
  2. Implementasi host service `AssetAuditService.kt`: antrean bounded FIFO 5000 event, ID sequence atomik (`AtomicLong`), reduksi outcome otomatis fail-closed, invalidasi cache pada `recordEvent()`, method bridge `getAuditReport()`, dan persistensi atomik laporan di `filesDir/runtime-state/audit/runtime-audit-report.json`.
  3. Instrumentasi VFS `VfsResourceInterceptor.kt`: pencatatan tunggal terotorisasi untuk `FILE_EXISTS` & `FILE_READABLE` saat aset disajikan, dan emisi event terminal `FILE_EXISTS` + `FAIL` + `PLACEHOLDER_SUBSTITUTED` saat 404 image/audio fallback aktif (deduplikasi dengan mencopot pencatatan redundan di `GpackStreamReader.kt`).
  4. Pembersihan total fake progress dan peniadaan premature completion di `diagnostic_suite.js`: event `PASS` tidak menghentikan audit, kegagalan aset (`FAIL` / fallback) dicatat dan dihitung tanpa mengaborsi polling, penyelesaian audit hanya via sinyal eksplisit `DONE`, mapping nama canonical stage ke UI label, dan query bridge via `getAuditEvents(lastEventSeq)`.
  5. Sinkronisasi identik 100% byte-for-byte `diagnostic_suite.js` antara Android assets dan `preview/` (SHA-256: `F0F7F3BDAFC6F781670D05683676BABF061B747897724FBDA95CB22F6595D1F4`).
  6. Unit test Android `AssetAuditServiceTest.kt` dan `VfsResourceInterceptorTest.kt` lulus via Gradle (`testDebugUnitTest`), 9 unit tests Node `diagnostic_suite.test.js` lulus, dan seluruh 198 cargo tests workspace lulus 100%.
- [2026-09-24] Eksekusi Plan 1 (Deep Asset Audit Foundation) Selesai & Merge ke `master`:
  1. Kontrak domain audit engine-neutral & Dependency Graph (`core/src/asset_audit/mod.rs`, `graph.rs`, `schemas/asset-audit-report.schema.json`).
  2. Auditor registry dengan panic-containment & fail-closed behavior saat adapter belum tersedia (`CMP-AUD-001`).
  3. Taksonomi diagnostik stabil `CMP-AUD-001..012` dan `BLD-104` (BLD_AUDIT_POLICY_BLOCKED).
  4. Perintah CLI `runtimepack audit` dengan opsi `--mode standard|deep`, `--allow-network-audit`, atomic report writer, dan isolasi proses (static-only).
  5. Build audit policy gating: otomatis memblokir build jika `FAIL` (kecuali `--accept-audit-fail`) atau `INDETERMINATE` (kecuali `--accept-audit-unknown`); hard blockers tidak dapat dibypass; manifest & provenance mencatat audit outcome.
  6. Dokumentasi kanonikal diselaraskan di `Docs/COMPATIBILITY.md`, `Docs/CONFIG_AND_SCHEMAS.md`, `Docs/TEST_PLAN.md`, dan `Docs/IMPLEMENTATION_PLAN.md`.
  7. Seluruh 34 integration build tests, 9 audit CLI tests, 28 core unit tests, dan full workspace test suite (92/92) lulus 100%.
- [2026-09-24] Fix 4 Issue Interaksi Overlay & Sinkronisasi 100% ke `preview/`:
  1. Centralized Panel Registry: Membangun `window.RuntimePackModalRegistry` (`Set`) dengan helper `setModalState(id, isOpen)` dan `registerModal(id, isOpen)` pada `vpad.js`, dihubungkan ke `enterEditMode`/`exitEditMode` ('settings'), `addBtn`/`closeKeyModal` ('custom_control'), `showPropertyPanel`/`hidePropertyPanel` ('inspector'), `openCheatMenu`/`closeCheatMenu` ('cheat'), dan `DiagnosticSuite.open`/`close` ('diag'). Bridge native `setOverlayModalOpen(true)` tetap dipertahankan selama `size > 0` dan hanya dilepas ke `false` saat seluruh panel telah ditutup (`size === 0`).
  2. Ghost Click & Click-Through Fix: Memindahkan aktivasi `bindTap` pada `vpad.js` agar memicu aksi secara otoritatif pada `pointerup` setelah validasi pergerakan jari (<= 10px), bukan pada `pointerdown`. Menambahkan interaction guard berbasis `collapseGuardTimestamp` dan penonaktifan sementara `pointer-events: none` selama 150ms pada tombol yang baru terungkap saat `#vpad-bar-collapse` ditoggle.
  3. Panel Pointer-Events Protection: Memberikan `pointer-events: auto !important; touch-action: none !important;` serta penghentian propagasi `pointerdown` dan `pointerup` (`e.stopPropagation()`) pada `#rp-diag-modal`, `#fnh-cheat-overlay`, `#vpad-editor-bar`, `#vpad-prop-panel`, dan `#vpad-key-modal` untuk mencegah sentuhan bocor ke `gameWebView` di latar belakang.
  4. Minimalist Matte Theme Active Button Flicker Fix: Mengganti aturan `.vpad-top-btn:active` di `vpad.css` agar tidak mengubah warna background dan border ke abu-abu `#27272a`. Menetapkan scale tactile press (`transform: scale(0.88) !important;`) sambil mempertahankan warna aktif `#1e3a8a` / `#3b82f6` pada seluruh tema (clean, matte, retro).
  5. Sinkronisasi Penuh & Test Regression: Seluruh file overlay di `android/shell/app/src/main/assets/overlay/` dan `preview/` disinkronkan identik (SHA-256 terverifikasi identik). Unit tests baru ditambahkan di `overlay_interaction_regression.test.js`. Seluruh test JS (11/11 regression, 12/12 vpad bridge, 7/7 cheat menu, 4/4 diagnostic suite, web shims, cheat primitives) dan 27/27 integration tests Cargo lulus 100%.
- [2026-09-23] Verifikasi Live Redesigned Diagnostic Suite pada *BLACK SOULS* (MuMu Player `127.0.0.1:16384`):
  1. Build APK `dist/black_souls/BlackSouls.apk` (826.00 MB, flavor `beta`) sukses dikompilasi via Gradle.
  2. Fresh install (`pm clear`) dan booting ke Title Screen interaktif berjalan mulus dalam ~12 detik.
  3. In-game capture terverifikasi penuh pada game screen display:
     - `diag_redesigned_in_game.png` (960x540): Title screen interaktif + Beta Watermark + Redesigned Diagnostic Suite modal terbuka di tengah layar.
     - `diag_streaming_in_game.png` (960x540): Automated Full Stress Test dengan live progress bar (~80%) dan live streaming activity console memindai 150 maps & database entities.
     - `diag_share_in_game.png` (960x540): Export & Share trigger memunculkan native Android system Share Sheet (`com.android.intentresolver.ChooserActivityLauncher`) berisi JSON report terstruktur.
- [2026-09-23] Task 3 SDD Beta Diagnostic Suite Selesai: Implementasi Build Flavor Pipeline (`--flavor beta` vs `--flavor release`) dan Permanent Beta Watermark (`Beta Test Veldra Community`). Menambahkan argumen CLI `--flavor` pada `runtimepack build` (default `"beta"`), validasi domain error `BLD-101`, penulisan metadata flavor pada `BuildManifest` dan `provenance`, injeksi watermark dan styling CSS dinamis pada `assets/overlay` (`vpad.js` dan `index.html`), penegasan kebijakan subagent di `AGENTS.md`, serta verifikasi melalui integration test `test_build_flavor_beta_injects_watermark` dan `test_build_flavor_release_omits_watermark`. Seluruh 27 tes integrasi lulus.
- [2026-09-23] Task 7 Porting & Validasi TyranoScript *[Kimochi] NTRdemic* Selesai & Terverifikasi: APK mandiri `dist/ntrdemic/NTRdemic.apk` (158.63 MB, SHA-256 `F2EEA549C69B91B3E15DB3B7491011E49E71B4E538F6A0E9DE4818B140716733`) sukses dibangun dari package Electron (`resources/app.asar`), diinstal, dan diverifikasi di MuMu Player 12. Menambahkan parser & extractor murni Rust `AsarArchive` (`engines/tyrano/src/asar.rs`), detektor statis asar (`DET-TYR-ASAR`, Confidence: EXACT), `TyranoAnalyzer` (`engines/tyrano/src/scanner.rs`), shim `process.execPath` & `process.env.HOME` di `runtimepack_web_shims.js` untuk mengatasi error `indexOf` pada `$.localFilePath()`, shim `$.isNeedClickAudio = false` & `ready_audio = true` untuk mencegah autoplay block browser, pengalihan save `file` ke `localStorage`, serta perbaikan MIME charset decoding (null charset untuk binary assets) di `VfsResourceInterceptor.kt`. Title screen interaktif (`title03.gif` 3840x2160, logo, tombol start/load/option, BGM `title2.mp3`) terverifikasi via screenshot 960x540 `Docs/test_ports/tyrano_kimochi/screenshot.png`. Laporan disimpan di `Docs/test_ports/tyrano_kimochi/report.md` dan matriks `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md` diperbarui `[x] [Kimochi] NTRdemic`.
- [2026-09-23] Task 4 Porting & Validasi RPG Maker MV *The Witch's House MV* Selesai & Terverifikasi: APK mandiri `dist/witchs_house/TheWitchsHouse.apk` (166.84 MB, SHA-256 `FBB50C98...`) sukses dibangun dari package NW.js (normalisasi content root `www/`), diinstal, dan diverifikasi di MuMu Player 12. Title screen PixiJS 4.5.4 WebGL merender dalam ~360ms, navigasi menu "Begin" dan dialog pemilihan tingkat kesulitan "Easy" responsif via virtual gamepad/input keyevent, transisi cutscene dan eksplorasi map pembuka di hutan terbukti lancar tanpa crash. Laporan disimpan di `Docs/test_ports/rpgmv_the_witchs_house_mv/report.md` dan matriks `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md` diperbarui `[x] The Witch’s House MV`.
- [2026-09-23] Task 3 Porting & Validasi RPG Maker XP (RGSS1) *To the Moon* Selesai & Terverifikasi: APK mandiri `dist/to_the_moon/ToTheMoon.apk` (154.45 MB, SHA-256 `5fc35430...`) sukses dikompilasi, diinstal, dan dijalankan di MuMu Player 12. Menambahkan aturan deteksi arsip terenkripsi `.rgssad`/`.rgss2a`/`.rgss3a` (Confidence: EXACT), filtering desktop runner DLL (`steam_api.dll`, `fluidsynth.dll`, `Utils.dll`), dynamic parser `mkxp.conf` dan symlink arsip di `GpackExtractor.kt`, polyfill Ruby 1.8 (`NilClass#id`, `NilClass#type`, `Hash#index`, `Array#to_s`), modul stub `MKXP` (`data_directory` -> `"./."`, `raw_key_states`), serta inisialisasi `.lang` default ke bahasa Inggris. Title screen interaktif (Lighthouse, Full Moon, Begin/Load/Exit) terverifikasi via screenshot 960x540 `Docs/test_ports/rpgxp_to_the_moon/screenshot.png`. Matriks `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md` diperbarui `[x] To the Moon`.
- [2026-09-22] Milestone 5 RPG Maker MV Web Runtime Selesai & Terverifikasi: *Fear & Hunger 2: Termina* sukses di-build langsung dari folder desktop NW.js menjadi APK standalone (`dist/termina/Termina.apk`, 1,528 MB, SHA-256 `861D75D5...`). Terverifikasi booting ke Title Screen interaktif ("Permainan Baru", "Lanjutkan", "Options") dengan overlay virtual gamepad aktif di MuMu Player (screenshot `termina_boot_test.png`).
- [2026-09-22] Implementasi `WebActivity.kt` hardware-accelerated WebView, `VfsResourceInterceptor` on-the-fly RPG Maker asset decrypter (`.rpgmvp` / `.rpgmvo`), `runtimepack_web_shims.js` (Node path, NW.js, Greenworks, StorageManager IndexedDB), dan normalisasi deployed NW.js packaging di Rust CLI (`cli/src/main.rs`).
- [2026-09-22] Verifikasi build, install, dan in-game visual test Lona RPG: kompilasi APK `dist/lona/LonaRPG.apk` (exit 0, 779.66 MB), fresh install (`pm clear`) ke MuMu emulator (`127.0.0.1:16384`), booting Title Screen, New Game, dan advance dialogue terverifikasi. Screenshot `lona_hud_fixed_verified.png` (960x540) membuktikan area gameplay dan pojok kanan atas 100% bersih tanpa black bar / scrollbar artifact.
- [2026-09-20] Penonaktifan total scrollbar WebView overlay: mematikan `isVerticalScrollBarEnabled` dan `isHorizontalScrollBarEnabled` di `RgssActivity.kt`/`WebBackend.kt`, menambahkan aturan CSS anti-scrollbar, serta memverifikasi bahwa kapsul scrollbar hitam di pojok kanan atas telah hilang sepenuhnya via screenshot `lona_no_scrollbar.png`.
- [2026-09-20] Fix freeze/crash menu makan (`undefined method 'key_item?' for nil:NilClass` di `191_menu_07Items.rb:348`): memisahkan tombol SPACE dari tombol virtual A di `RgssActivity.kt`, menambahkan patch eksklusif `if ... elsif` serta null-safety check di `Menu_Items` (`runtimepack_bootstrap.rb`), terverifikasi in-game via screenshot `lona_item_consume_test.png`.
- [2026-09-20] Verifikasi live in-game Save Menu Anywhere: kompilasi APK `dist/lona/LonaRPG.apk` (exit status 0), fresh install (`pm clear`) ke MuMu emulator (`127.0.0.1:16384`), eksekusi "Open Save Menu Anywhere" dari tab "System & Save" di Cheat Menu modal, dan verifikasi tangkapan layar `save_menu_fixed.png` (960x540). Terverifikasi: menu Save (`Scene_Save`) terbuka 100% bersih tanpa glitch, tidak ada pesan `fucking return #<Scene_Map:...>`, dan tidak ada error missing `UserData/Save01.png`.
- [2026-09-20] Verifikasi clean reboot & boot sequence Lona RPG: kompilasi APK `dist/lona/LonaRPG.apk` (exit status 0, 779.63 MB), fresh install (`pm clear`) ke MuMu emulator (`127.0.0.1:16384`), auto-ekstraksi 15.789 file GPACK v1 sukses dalam ~16 detik, booting sempurna ke Title Screen interaktif (terverifikasi via screenshot 960x540 `lona_reboot_stable.png`), dan logcat bersih tanpa force close (SDL, OpenAL, mkxp-z running).
- [2026-09-20] Verifikasi Early Access Beta Test APK: kompilasi APK `dist/lona/LonaRPG.apk` (SHA-256 `6233B7BD86F6C8ED113EBC4171E15477AB9F8282B99B8F81C445E26E76A6F576`, 779.61 MB / 817,481,290 bytes), instalasi ke MuMu emulator (`127.0.0.1:16384`), booting ke Title Screen interaktif, dan verifikasi watermark permanen "Beta Test Veldra Works" di pojok kanan atas via tangkapan layar `beta_test_wm_title.png` (960x540).
- [2026-09-20] Verifikasi live in-game pipeline penuh: kompilasi APK `dist/lona/LonaRPG.apk` (SHA-256 `ACF97460010EEA1D5EDB55CF1A7A1F817FB326D124056F36E7954C9E2A78BAF7`, 779.61 MB), instalasi ke MuMu emulator (`127.0.0.1:16384`), navigasi Cheat Menu ⇄ "System & Save" category pill (Open Save Menu, Open Load Menu, Quick Save, Quick Load) terverifikasi 100% via tangkapan layar `cheat_clean_system_save.png`.
- [2026-09-20] Build ulang `dist/lona/LonaRPG.apk` (SHA-256 `CA0F7949ED8EE43809D33285ACE0DBC9D6BA03892188FC6CD487080482B31B64`, 779.61 MB) sukses dipaketkan.
- [2026-09-20] Navigasi Cheat Menu per kategori (Vitals, Medical, Combat, Economy) dengan icon SVG, tanpa deskripsi teks panjang, dan tombol toggle ON/OFF.
- [2026-09-20] Implementasi persistent hook God Mode dan 1-Hit Kill di `runtimepack_bootstrap.rb` dan sinkronisasi ke cheat IPC.
- [2026-09-20] Sinkronisasi keyboard fisik penuh (A-Z, 0-9, F1-F12, Space, Enter, Esc, Shift, Ctrl, Alt) langsung ke buffer Win32 `rp_input_state.bin` untuk skrip Hime AllKey di Lona RPG.
- [2026-09-20] Verifikasi live in-game build, install, booting, modal Cheat Engine, dan toggle Lona RPG Quick Cheats (`cheat_profile.js`) pada emulator MuMu Player; seluruh fungsi terverifikasi bekerja 100% via tangkapan layar.
- [2026-09-20] Build ulang `dist/lona/LonaRPG.apk` (SHA-256 `F27A99C6575B7D083429CF09770927FFA2D818C858B24A255EB76ADF09CDB663`, 779.59 MB) dengan asset icon Android mipmap, keyboard & mouse pass-through, vpad updates, dan cheat profile modular.
- [2026-09-20] Ekstraksi icon asli dari `Game.exe`, pembuatan launcher mipmap Android (mdpi..xxxhdpi, persegi & bulat), dan konfigurasi `android:icon` di `generator.rs`.
- [2026-09-20] Pembaruan URL komunitas ke `https://linktr.ee/Veldra.Works` pada seluruh overlay HTML, JS, dan test suite.
- [2026-09-20] Pengisian `cheat_profile.js` dengan kumpulan cheat spesifik Lona RPG (Survival & Vitals, Medical & Organ Recovery, Progression & QoL).
- [2026-09-20] Implementasi toggle Cheat Engine ⇄ Cheat Menu di header modal cheat (`cheat_menu.js`) dan pembuatan template siap pakai `cheat_profile.js` untuk AI Agent / Programmer.
- [2026-09-20] Ekspansi seluruh tombol keyboard standar PC (A-Z, 0-9, Numpad, F1-F12, Arrows, Navigation, Punctuation/Symbols) di `vpad.js` dan `RgssActivity.kt`.
- [2026-09-20] Implementasi Mode Hide Gamepad → Keyboard/Mouse Mode dan fix pass-through hardware keyboard serta mouse ke SDL dan Win32 `rp_input_state.bin`.
- [2026-09-20] Verifikasi build Android shell (`assembleDebug`), unit tests Android (17/17 lulus), dan test suite Rust workspace (41/41 lulus).
- [2026-09-18] Inisialisasi git repository dan konfigurasi `.gitignore`
- [2026-09-18] Setup toolchain Rust `stable-x86_64-pc-windows-gnullvm` dengan LLVM-MinGW UCRT
- [2026-09-18] Implementasi Milestone 0 (arsitektur workspace Cargo, domain types, error taxonomy, schemas, toolchain lock)
- [2026-09-18] Implementasi Milestone 1 (detektor statis XP, VX, VX Ace, MV, MZ, Ren'Py, Tyrano, serta perintah `inspect` dan `explain`)
- [2026-09-18] Implementasi Milestone 2 (kerangka Android host shell, `BootstrapActivity`, `RuntimeApplication`, `BuildManifestLoader`)
- [2026-09-18] Implementasi Milestone 3 (GPACK/1 container packer, reader, path traversal validation, dan build pipeline)
- [2026-09-18] Implementasi compatibility scanner RGSS dan RPG Maker Web serta database deklaratif `compatibility-db/`
- [2026-09-18] Implementasi Milestone 5 & 6 (Web runtime adapter, JS bridge, dan Ren'Py versioned scanner)
- [2026-09-18] Implementasi Milestone 7 (kontainer terenkripsi streaming AEAD ChaCha20-Poly1305)
- [2026-09-18] Implementasi Milestone 8 & 9 (Signing manager, third-party notices, release hardening, toolchain auto-driver Gradle)
- [2026-09-18] Setup toolchain Android lokal (Microsoft OpenJDK 17, Android SDK platform 34, build-tools 34.0.0, Gradle 8.7)
- [2026-09-18] Milestone 4 RGSS Native Player: integrasi binary `libmkxp-z.so`, `libruby.so`, SDL2, dan `RgssActivity`
- [2026-09-18] Integrasi unified virtual controller overlay `vpad.js` (5 preset, drag-and-drop editor, 4 ikon toolbar) dan `cheat_menu.js`
- [2026-09-18] Perbaikan kompresi `game.pack` (`noCompress`) dan relokasi index table ke posisi depan (linear streaming)
- [2026-09-18] Perbaikan startup SDLThread dari `onStart()` pada multi-window emulator
- [2026-09-18] Shimming Win32API (`GetCommandLine`, `GetKeyboardState`, `GetCursorPos`, `GetSystemMetrics`, `SystemParametersInfo` non-zero workarea)
- [2026-09-18] Shim modul `DL` (`DL::CPtr`, `DL.malloc`) untuk kompatibilitas script Hime AllKey di Ruby modern
- [2026-09-18] Nonaktifkan crash optimasi BitmapChanger DLL dengan fallback pure Ruby tanpa popup error
- [2026-09-18] Patching parser `101_jsonEnDecoder.rb` menjadi non-rekursif untuk mencegah `SystemStackError: stack level too deep`
- [2026-09-18] Penanganan case-insensitive filesystem resolver pada skrip Ruby (`File.size`, `File.open`, `File.exist?`)
- [2026-09-18] Penanganan error encoding ASCII-8BIT ke UTF-8 pada logging error TKG (`open(filename, "wb")`)
- [2026-09-18] Penanganan coercion boolean pada setter `Sprite#visible=` untuk mengatasi typo game `@sprite.visible = @sprite`
- [2026-09-18] Lona RPG terbukti sukses booting dan merender Title Screen interaktif penuh pada MuMu Player
- [2026-09-18] Implementasi Engine-Agnostic Core Cheat Runner di runtimepack_bootstrap.rb, RgssActivity atomic IPC bridge, dan cheat_engine.js universal primitives
- [2026-09-18] Implementasi JoiPlay/M-Tool style dynamic cheat UI di cheat_menu.js (Variables browser, Switches browser, Item spawner, M-Tool script console & snippet manager) serta test suite cheat_menu.test.js
- [2026-09-18] Task 6: Kompilasi end-to-end APK Lona RPG, sinkronisasi assets ke dist/lona/android-project, instalasi dan verifikasi fungsional in-game pada MuMu Player (input state bin, cheat IPC evaluation, robust JSON serialization & exception handling)
- [2026-09-19] Riset evidence-first MTool Smart Fix (RGSS3/mkxp-z) disimpan di `Docs/Riset_MTool_Smart_Fix_RGSS3_mkxp-z.md`; kesimpulan: mekanisme internal MTool tetap UNKNOWN, adopsi pola Smart Resolver deterministik + fail-closed.
- [2026-09-19] Fix crash popup NPC/SKIP (`Font#shadow=` strict bool) via boolean coercion setter Font di bootstrap
- [2026-09-19] Fix ALT + D-Pad tidak berfungsi (JS 18 tidak terpetakan) dan bug multi-touch melepas tombol lain; audit mapping JS→Android→VK (termasuk F1–F12, Delete)
- [2026-09-19] Fix glyph kotak (fullwidth colon & panah) via `fontSub` di mkxp.json karena mkxp-z memakai typographic family FreeType
- [2026-09-19] Fix normalisasi glyph gagal karena mismatch encoding ASCII-8BIT vs UTF-8
- [2026-09-19] Fix keyboard fisik tidak sampai ke SDL saat overlay aktif via override `dispatchKeyEvent` + bridge `setOverlayTextFocused`
- [2026-09-19] Build ulang `dist/lona/LonaRPG.apk` (SHA-256 `23E1C473...`) berisi seluruh fix di atas
