# Progress Update — 30 September 2026 (Sesi 3, penutupan plan besar)

## [2026-10-05] VNM v4 Baseline Commit (Gate 2) — IN PROGRESS
- **Baseline Git Branch:** `baseline/vnm-v4`
- **Scope & User Acceptance:** User accepted v4 reconstructed candidate behavior: normal gameplay and built-in Skip operate reliably equivalent to old reference APK; rapid auto-clicking exhibits a known minor limitation (holding a white-screen scene transition in both old and v4 APKs).
- **Full Overlay & Ripple Toggle:** Preserved in codebase but not newly device-certified; primary user-test candidate uses passive-watermark profile.
- **Flavor & Release Status:** Beta only; release flavor remains `RELEASE_NOT_VALIDATED`.
- **Integrity & Immutable Inputs:** Raw game source, old reference APK (`Backup/TFPC-TLFMB.apk`), and candidate APKs remain 100% immutable and outside Git tracking.

---

## Status: STABILITY & DIAGNOSTICS OVERHAUL — SELESAI (3 fase komplit, final review APPROVED_FOR_MERGE)

Branch `feature/native-mv-mz-save` (worktree `.worktrees/native-mv-mz-save`) kini berisi 17 commit, semua ter-review:
- **Save/Load fix** (plan 1): input single-tap, codeMapper, RuntimePackInput, HalfMove compat, fail-loud RT-LOAD-FAIL, NativeSaveRepository.
- **Phase 1 Diagnostik Jujur**: report.rs `js/` prefix fix + GPACK probe lookup (juga di-commit ke master: e8857be, 7fc3711), ruleset bump 11 (f3b3e20) → probe on-device **1158→0 failure**, engine_family jujur, instrumentasi debug dihapus.
- **Phase 2 Capability Registry**: 16 CAP (CAP-WEB-001..016) di `Docs/superpowers/compat/COMPAT-REGISTRY.md` + marker kode + 7 test gap ditutup (18 suite JS hijau).
- **Phase 3 Probe CLI T1**: subcommand `runtimepack probe` (trait ProbeTarget, AdbTarget config-driven dari `probe-profiles/devices.toml` — nol hardcode device), 12+ test Rust, verifikasi device MuMu **SELURUHNYA LOLOS** (boot/new_game/touch/gamepad/stability), flag `--probe`/`--probe-device` di build (exit 1 saat probe fail, skip grace saat tanpa device).

### Keputusan desain penting (lihat decisions_log):
- Probe scope T1-only (T2 suntik save & T3 round-trip = future work tercatat).
- Device via config/CLI, bukan hardcode.
- Save-anywhere = capability manual per game-profile (future work, bukan prasyarat probe).
- Rulings urutan task T6 sebelum T3 merge.

### Bug tersembunyi yang ditemukan & diperbaiki di sesi ini:
- Probe race condition: `pidof` dicek ~300ms setelah launch async → false RT-PROCESS-CRASH. Fix: grace period spawn 8s + kode jujur RT-LAUNCH-NO-PROCESS (789aa8e).
- Audit cache tidak memasukkan versi kode → fix benar tapi graph lama terkirim (166 failure). Fix: ruleset bump (pelajaran: SELALU bump `CURRENT_AUDIT_RULESET_VERSION` saat ubah perilaku audit).

### Minor yang DITERIMA final review (parked, tidak blocking merge):
a. RgssActivity belum ada bridge getEngineFamily (track RGSS beda).
b. 1 entry mojibake path image (game encoding, input immutable).
c. Cache key tidak hash kode audit (mitigasi ruleset bump).
d. RuntimePackInput.trigger sync reset (tak dikonsumsi); predicate clearInterval.
e. HalfMove delegation: risiko recursion multi-button (guard try/catch ada).

### Verifikasi user di device: probe EXHAUSTIVE 0 error, load 1x tap PASS, gamepad PASS.

### Next (menunggu keputusan user):
- Merge branch `feature/native-mv-mz-save` ke master (SUDAH di-merge: b4f1d1a).
- Selesai plan `feature/asset-hardening` (7 commit, final review APPROVED_FOR_MERGE 10/10).
- Siap merge `feature/asset-hardening` ke master.

---

## [2026-10-05] VNM Reference-Mode Passive Beta Build (Task 5) — SELESAI
- **Workspace:** `D:\02_Development\Projects_2\runtimepack-vnm-reconstruction`
- **Output:** `dist_vnm_reference_v4_passive` (semua artifak sebelumnya tetap dipreservasi utuh).
- **Deliverable APK (v4 passive candidate):**
  - Path: `dist_vnm_reference_v4_passive\TimeforPunishmentClass~TakingLessonsforMyBoyfriend~.apk`
  - Size: 1,427,602,367 bytes (1361.47 MB)
  - SHA-256: `C39088E32F8D599B538AEC9D360F02CF02983E2BCC255BE6E3428EC12643ADDF`
  - Package: `io.runtimepack.game.timeforpunishmentclasstakinglessonsformyboyfriend`
  - MinSDK: 26, TargetSDK: 34, ABI: `arm64-v8a`
  - Signer: Android Debug (v2 scheme verified, certificate SHA-256 `531643144f89...` identik, update OS kompatibel)
- **Cryptographic Asset Integrity Verified:**
  - `RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256`: `2f83e641576ea6b45ef128711fbb4b48b19754981812f0d8c38c0aac6d5bf943`
  - Computed actual APK staged assets (4 file: `overlay/index.html`, `web_shims/runtimepack_asset_audit.js`, `web_shims/runtimepack_web_shims.js`, `fonts/vl-gothic-regular.ttf` via exact Kotlin algorithm): `2f83e641576ea6b45ef128711fbb4b48b19754981812f0d8c38c0aac6d5bf943`
  - Status: **EXACT MATCH (`EQUAL: True`)**; tidak memerlukan atau bergantung pada toleransi exception beta.
- **Parity Container & Shims:**
  - GPACK `game.pack`: SHA-256 `E55CE664A990173A7FA5E1F74D61FC16333C6D53291CDB3BD5B3C7AF98D3DDFC` (1,385,960,367 bytes, 1969 entries, 100% `encryption=0`).
  - Container diff: 1,968 dari 1,969 entri identik dengan v3. Tepat 1 file (`globalData.vndata`) bertambah dari 17,661 ke 31,513 bytes (+13,852 bytes) pada live session sebelumnya. Diungkapkan jujur tanpa modifikasi input.
  - Web shims: SHA-256 `A311855227BF52C56AC2EADF499A4BAA4C65CF17720C32FDD0D1FBCC0CB36A52` byte-for-byte identik dengan template source. Memuat touchcancel safety (`CAP-WEB-024`), AudioManager pool pruning (`CAP-WEB-025`), legacy Base64 save fallback (`CAP-WEB-026`), dan bridge `postDiagnosticError`.
  - Passive overlay: Static HTML watermark (`Beta Test Veldra Community`) 100% identik dengan old reference APK; JS disabled, zero controls, zero ripple/feedback scripts.
  - DEX classes: Memuat `BuildManifestLoader`, `DiagnosticLogStore`, `NativeSaveRepository` (read-only persistent paths), `BoundedEntryStream`, dan passive touch dispatch branch di `WebActivity`.
- **Context & Scope Disclosures:**
  - Status gameplay: **`NOT_TESTED`** (pengujian gameplay sepenuhnya dimiliki oleh user; zero ADB/MuMu/device boot; tidak ada klaim kelulusan lama atau jaminan build 'stable').
  - Siap untuk audit independen Task 6 oleh `qa-tester`.

---

## [2026-10-05] VNM Stabilized v3 Candidate APK Build (Task Build) — SELESAI
- **Workspace:** `D:\02_Development\Projects_2\runtimepack-vnm-reconstruction`
- **Output:** `dist_vnm_stabilized_v3` (output `dist_vnm_reconstructed` dan `dist_vnm_reconstructed_v2` tetap dipreservasi utuh).
- **Deliverable APK (v3):**
  - Path: `dist_vnm_stabilized_v3\TimeforPunishmentClass~TakingLessonsforMyBoyfriend~.apk`
  - Size: 1,427,674,927 bytes (1361.54 MB)
  - SHA-256: `227CBB029D04C3C2F58F99DEEC0CB6F6FB2322FCE5DD30158C6500A86CCD59E3`
  - Package: `io.runtimepack.game.timeforpunishmentclasstakinglessonsformyboyfriend`
  - MinSDK: 26, TargetSDK: 34, ABI: `arm64-v8a`
  - Signer: Android Debug (v2 scheme verified, certificate SHA-256 `531643144f89...` identik dengan v1 dan v2, kompatibilitas save/data terjaga)
- **Cryptographic Asset Integrity Verified:**
  - `RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256`: `f321cb006d7cf9f1fc93aff32d50bbce08195f53af9fcd785e9cc7ea64d03b1e`
  - Computed actual APK staged assets (11 file di `overlay/`, `web_shims/`, `fonts/` via exact Kotlin algorithm): `f321cb006d7cf9f1fc93aff32d50bbce08195f53af9fcd785e9cc7ea64d03b1e`
  - Status: **EXACT MATCH (`EQUAL: True`)**; tidak memerlukan atau bergantung pada toleransi exception beta.
- **Parity Container & Shims:**
  - GPACK `game.pack`: SHA-256 `21079D20C44E2F38DF7AFC0EECA978EFE148593E0539140B944FF40B5D60DF60` (identik persis dengan v1 dan v2). 1969 entries, 100% `encryption=0`.
  - Web shims: SHA-256 `1d9f63cd18be97705cc9259784bbe17d97c02570e98a114095076963d1ae8b28` byte-for-byte identik dengan template source. Memuat own-instance audio accessors (`isPlaying`, `volume`, `playbackRate`), sidecar `vnmPendingSidecar`, dan bridge `postDiagnosticError`.
  - Diagnostic suite: SHA-256 `a649af0e...` mirror exact dengan preview. Memuat `getAuditFailures`, `getRuntimeBuildInfo`, dan 8 MiB UTF-8 byte boundary trimming.
  - Metadata outside gamepack: `packaging-exclusions.json` (7 desktop DLLs), `vnm-preparation-profile.json` (status Enabled).
  - DEX classes3.dex: Memuat `MiniJson`, `DiagnosticLogStore`, `AssetAuditService` (dengan `runLock`), `BoundedEntryStream`, `RuntimeIntegrity`.
- **Context & Scope Disclosures:**
  - Riwayat pengujian user v2: gagal sebelum chapter 2 (save normal, pause/stop terdiagnosa).
  - Status gameplay v3: **`NOT_TESTED`** (pengujian gameplay sepenuhnya dimiliki oleh user; zero ADB/MuMu/device boot; tidak ada klaim kelulusan lama atau jaminan build 'stable').
  - Android Lint baseline 36 vendor errors diungkapkan jujur, 0 host errors.
  - Raw source 1976 files vs historical 2279 files (historical 303 diff unknown, cannot claim solved).
  - Seluruh artifak lama (`dist_vnm_reconstructed`, `dist_vnm_reconstructed_v2`, `Audit-app.txt`) dipreservasi 100% tanpa cleanup.

---

## [2026-10-05] Gate G1 / Gate G: Task 3 V2 Visual Novel Maker Candidate APK Rebuild — SELESAI
- **Workspace:** `D:\02_Development\Projects_2\runtimepack-vnm-reconstruction`
- **Output:** `dist_vnm_reconstructed_v2` (output `dist_vnm_reconstructed` yang terblokir tetap dipreservasi utuh).
- **Deliverable APK (v2):**
  - Path: `dist_vnm_reconstructed_v2\TimeforPunishmentClass~TakingLessonsforMyBoyfriend~.apk`
  - Size: 1,427,619,428 bytes (1361.48 MB)
  - SHA-256: `18F8E1A5FE064CE15C888C8140434FD510EA05DCAF3ACBAFA4EEFDD0F6FB781F`
  - Package: `io.runtimepack.game.timeforpunishmentclasstakinglessonsformyboyfriend`
  - MinSDK: 26, TargetSDK: 34, ABI: `arm64-v8a`
  - Signer: Android Debug (v2 scheme verified)
- **Cryptographic Asset Integrity Verified:**
  - `RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256`: `5812be9a3e656d633333667bf05593fb8085361e8a7bbdfcbc8693327431035d`
  - Computed actual APK staged assets (11 file di `overlay/`, `web_shims/`, `fonts/` via exact Kotlin algorithm): `5812be9a3e656d633333667bf05593fb8085361e8a7bbdfcbc8693327431035d`
  - Status: **EXACT MATCH (`EQUAL: True`)**; tidak memerlukan atau bergantung pada toleransi exception beta.
- **Parity Container & Shims:**
  - GPACK `game.pack`: SHA-256 `21079D20C44E2F38DF7AFC0EECA978EFE148593E0539140B944FF40B5D60DF60` (identik persis dengan build v1 fresh). 1969 entries, 100% `encryption=0`.
  - Web shims: SHA-256 `247aed00...` byte-for-byte identik dengan template source.
  - DEX classes3.dex: Memuat `BoundedEntryStream`, `GpackStreamReader`, `GpackExtractor`, `RuntimeIntegrity`.
- **Scope Note:**
  - Release flavor: `RELEASE_NOT_VALIDATED` (bukan bagian dari deliverable gate ini).
  - Playability / gameplay: `NOT_TESTED` (pengujian gameplay sepenuhnya dimiliki oleh user; zero ADB/device boot).

---

## [2026-10-05] Gate G: Task 3 Visual Novel Maker Candidate APK Build (v1 - BLOCKED by Gate G1) — PRESERVED
- **Workspace:** `D:\02_Development\Projects_2\runtimepack-vnm-reconstruction`
- **Source Input:** `D:\Old-Recvery\Project_2\RPGM-P\Raw-Games\Time for Punishment Class ~Taking Lessons for My Boyfriend~` (1976 files, 1433.87 MB, source_hash `dcfd5c07...`, 100% read-only).
- **Baseline Context:** Raw source contains 1976 files vs historical reference 2279 files (different package name; confirmed user version distinction; no byte parity claim made).
- **Candidate APK Deliverable:**
  - Path: `dist_vnm_reconstructed\TimeforPunishmentClass~TakingLessonsforMyBoyfriend~.apk`
  - Size: 1,427,619,428 bytes (1361.48 MB)
  - SHA-256: `ACEDFA73281C008C9C5551DF406B9DB9ADD152F3A71DDAD37097FE2F406DEC1F`
  - Package: `io.runtimepack.game.timeforpunishmentclasstakinglessonsformyboyfriend`
  - MinSDK: 26, TargetSDK: 34, ABI: `arm64-v8a`
  - Signer: Android Debug (v2 scheme verified)
- **Artifact Verification:**
  - Embedded web shims SHA-256 matches source byte-for-byte (`247aed00...`).
  - GPACK container: 1,385,946,515 bytes, 1969 entries, 100% `encryption=0` (flavor beta unencrypted).
  - DEX classes contain `BoundedEntryStream`, `GpackStreamReader`, `GpackExtractor`, `RuntimeIntegrity`.
  - Beta overlay diagnostics & watermark (`Beta Test Veldra Community`) verified in APK assets.
- **Playability Status:** `NOT_TESTED` (Gameplay owned by user; independent of previous pass; no emulator/device run performed).

---

## [2026-09-30] Asset Hardening & Overlay Lockdown (Plan B+C+D) — SELESAI & APPROVED_FOR_MERGE

Branch: `feature/asset-hardening` (worktree `.worktrees/asset-hardening`, BASE `404749c`).

### Yang diselesaikan:
1. **Containerization (B)**: Pada build `--flavor release`, file `overlay/*`, `web_shims/*`, dan `fonts/*` dikemas ke dalam biner terenkripsi `game.pack` (GPACK) dengan algoritma AEAD ChaCha20-Poly1305. File-file tersebut **100% dihilangkan** dari direktori `assets/` APK. Host Android (`OverlayAssetSource`) memuat aset runtime langsung dari stream GPACK terdekripsi dalam memori.
2. **Startup Integrity Verification (C)**: Generator build Rust menghitung hash SHA-256 deterministik (`sorted canonical paths + null byte + content bytes + null byte`) dan menyematkannya ke dalam kode sumber Kotlin terkompilasi (`RuntimeBuildInfo.kt`). Pada `onCreate`, `RuntimeIntegrity` memvalidasi payload sebelum WebView dimuat. Jika terdapat modifikasi atau penghapusan aset, aplikasi menampilkan layar `APPLICATION CORRUPTED` (`AND-401`/`AND-402`/`AND-403`) dan keluar secara rapi.
3. **DEX Hardening / R8 (D)**: Mengaktifkan R8 minification (`isMinifyEnabled = true`) dengan aturan `-keep` komprehensif pada `@android.webkit.JavascriptInterface`, Activity, dan metode native JNI di `proguard-rules.pro`.
4. **Verifikasi Host-Side**: APK release `dist_release/Forestia1.4.4-IND.apk` terverifikasi 0 file plain di `assets/overlay/`, `assets/web_shims/`, `assets/fonts/`; seluruh 8 file overlay, 2 file web shims, dan 7 file fonts ada di dalam `game.pack`.
5. **Test Suite**: Semua 357 Cargo workspace tests, 12 Gradle Android unit test suites, dan 10/10 Node web shims tests lulus 100%. Final review: APPROVED_FOR_MERGE (Score 10/10, 0 Critical, 0 Important, 2 Minor).

