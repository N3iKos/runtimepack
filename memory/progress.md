# Progress Update — 30 September 2026 (Sesi 3, penutupan plan besar)

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

## [2026-09-30] Asset Hardening & Overlay Lockdown (Plan B+C+D) — SELESAI & APPROVED_FOR_MERGE

Branch: `feature/asset-hardening` (worktree `.worktrees/asset-hardening`, BASE `404749c`).

### Yang diselesaikan:
1. **Containerization (B)**: Pada build `--flavor release`, file `overlay/*`, `web_shims/*`, dan `fonts/*` dikemas ke dalam biner terenkripsi `game.pack` (GPACK) dengan algoritma AEAD ChaCha20-Poly1305. File-file tersebut **100% dihilangkan** dari direktori `assets/` APK. Host Android (`OverlayAssetSource`) memuat aset runtime langsung dari stream GPACK terdekripsi dalam memori.
2. **Startup Integrity Verification (C)**: Generator build Rust menghitung hash SHA-256 deterministik (`sorted canonical paths + null byte + content bytes + null byte`) dan menyematkannya ke dalam kode sumber Kotlin terkompilasi (`RuntimeBuildInfo.kt`). Pada `onCreate`, `RuntimeIntegrity` memvalidasi payload sebelum WebView dimuat. Jika terdapat modifikasi atau penghapusan aset, aplikasi menampilkan layar `APPLICATION CORRUPTED` (`AND-401`/`AND-402`/`AND-403`) dan keluar secara rapi.
3. **DEX Hardening / R8 (D)**: Mengaktifkan R8 minification (`isMinifyEnabled = true`) dengan aturan `-keep` komprehensif pada `@android.webkit.JavascriptInterface`, Activity, dan metode native JNI di `proguard-rules.pro`.
4. **Verifikasi Host-Side**: APK release `dist_release/Forestia1.4.4-IND.apk` terverifikasi 0 file plain di `assets/overlay/`, `assets/web_shims/`, `assets/fonts/`; seluruh 8 file overlay, 2 file web shims, dan 7 file fonts ada di dalam `game.pack`.
5. **Test Suite**: Semua 357 Cargo workspace tests, 12 Gradle Android unit test suites, dan 10/10 Node web shims tests lulus 100%. Final review: APPROVED_FOR_MERGE (Score 10/10, 0 Critical, 0 Important, 2 Minor).

