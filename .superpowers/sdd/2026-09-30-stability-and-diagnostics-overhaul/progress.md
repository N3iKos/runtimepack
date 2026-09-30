# SDD ledger — plan: Docs/superpowers/plans/2026-09-30-stability-and-diagnostics-overhaul.md

## Pre-flight

- Worktree: `.worktrees/native-mv-mz-save`, branch `feature/native-mv-mz-save`, HEAD awal `a14860b3e655e9ce97601a326c550977e0187fd5`.
- Worktree bersih (5 untracked: sdd workspace, plan baru, halfmove report/review, load-diag report).
- MASTER repo utama: HEAD `385df60`, 19 file uncommitted milik Track 3/Track D — Task 1-2 melakukan selective stage HANYA file milik fix probe.
- Spec: plan self-contained + `Docs/FORESTIA_SAVE_LOAD_HANDOVER.md`.

## Conflict scan (pre-flight rulings)

| Pair / Task | Produces vs Consumes | Finding | Ruling |
| :--- | :--- | :--- | :--- |
| Task 1-2 (master) vs Task 3 (merge) | Master commit fix probe → worktree merge | Fix `report.rs` belum ada di worktree (verifikasi explorer: `git diff master feature/... -- core engines cli android` = no diff pada file itu) | Merge normal; konflik shim diselesaikan memihak worktree (HEAD) |
| Task 4 vs Task 5 | Keduanya menyentuh file berbeda (`diagnostic_suite.js`+`WebActivity.kt` vs `runtimepack_web_shims.js`+test) | Tidak tumpang tindih file | Tidak ada konflik |
| Task 5 vs commit `be978f9`/`a14860b` | Hapus `patchLoadDebug` (be978f9) tanpa merusak fix HalfMove (a14860b, fungsi terpisah `patchHalfMoveCompat`-ish) | Fungsi terpisah delimited komentar — hapus hanya blok `RP_LOAD_DEBUG` | Verifikasi post-hapus: grep `RP_LOAD_DEBUG` = 0, grep HalfMove compat marker masih ada |
| Task 6 vs master uncommitted | `WebBackend.kt` di worktree modified — bukan milik master working tree (worktree punya versi sendiri) | Aman — worktree file | Tidak ada konflik |
| Task 10 vs AGENTS.md §6.3 (core engine-agnostic) | Probe trait di `cli/src/probe/`, bukan `core/` | Sesuai: engine-specific scenario di cli layer, core tetap bersih | Tidak ada konflik |
| Task 11 negative test | Build APK kedua (~800MB) di branch throwaway | Biaya diterima (dinyatakan di plan) | Ruling: throwaway worktree dihapus setelahnya |

Scan bersih; dispatch Task 1.

## Progress

Task 1+2: complete (commits master e8857be + 7fc3711, executor ses_f10535d04ffep73M9sScfTTgHd)
- Rust 45 test PASS (termasuk regression `js/` prefix), Kotlin 11 PASS (GPACK probe test ikut di-commit).
- `VfsResourceInterceptor.kt` + test DIKUCILKAN (milik Track 3 Task 5) — benar sesuai decision rule.
- Review paket: fix sederhana (transcription commit yang sudah diverifikasi explorer sebelumnya); spec-compliance dicek dari report executor + verifikasi diff langsung di bawah.

Task 3: complete (merge commit 0d968b2, clean tanpa konflik)
- Ruling urutan: Task 6 dikerjakan SEBELUM Task 3 karena WebBackend.kt modified memblokir pre-merge check. Task 6 complete dulu (commit 84529bd, Gradle hijau, worktree bersih).
- Post-merge verifikasi: JS suite hijau, Rust asset_audit 45/45, Gradle compile+unit hijau.

Task 4: complete (commit ee91ac5, executor ses_f10499885ffeV06s1D5rhqqd8a)
- Field aktual: `AssetAuditService.currentEngine` (dibuat public getter). Bridge `getEngineFamily()` di WebActivity; diagnostic_suite.js bridge-first + fallback globals game; hardcoded RGSS blind fallback dihapus.
- Minor (deferred): `RgssActivity.kt` juga bind VPadBridge tapi belum punya `getEngineFamily()` → diagnostic RGSS jatuh ke "unknown". Bukan scope plan ini (RGSS runtime beda track); catat untuk final review.

Task 5: complete (commit a11fd24, executor ses_f104432e7ffe02RzOv3bIr1ovH, TDD red→green)
- `RP_LOAD_DEBUG` sisa: 0 (verifikasi supervisor). Fix HalfMove + RuntimePackInput tetap utuh (grep 16 hit). Suite baru "Load Failure Diagnostics" PASS.
- Task review: verifikasi supervisor langsung (diff stat 286 baris: -241 instrumentasi/+126 fail-loud+test; grep dua arah) — dianggap cukup karena perubahan biner (hapus blok delimited) + test TDD. Final review menyapu ulang.

Task 7 (checkpoint Phase 1): complete — SEMUA VERDIKT PASS
- Build+bundle PASS (flavor beta, shim bersih dari RP_LOAD_DEBUG, GPACK fix terbundle).
- Probe: 1158 → 0 failure; engine_family = `rpgmaker_web` (bukan RGSS).
- Load test PASS (tap 1x → map); gamepad A PASS.
- Anomali: tidak ada. APK final Phase 1 terinstall di MuMu.

Task 7b (tindak lanjut cache): complete (commit f3b3e20)
- Root cause 166 failure tersisa: audit cache hit pra-fix (cache key tanpa code version; e8857be lupa bump ruleset). Fix: `CURRENT_AUDIT_RULESET_VERSION` 10→11 → audit REGENERATE (bukan cache hit), graph final: 0 bare path, 166 `js/plugins/` utuh.
- Verifikasi on-device oleh USER (bukan QA agent — RAM host sibuk, QA dibatalkan atas permintaan user): probe EXHAUSTIVE hasil **0 error**, engine_family `rpgmaker_web`, load game aman.
- Pelajaran tercatat: klaim QA agent "0 failure" pertama ternyata keliru (graph stale) — data user adalah verifikasi final yang mengikat.
- Minor (deferred, dari explorer): mojibake path `img/pictures/still/majo/<mojibake>/H001_00.png_` vs disk `majo\h\` — encoding issue terpisah (1 entry, bukan bagian 166). Final review triage.
- Hardening opsional (belum dikerjakan): masukkan hash kode audit/git rev ke `AuditCacheKey::to_sha256` supaya perubahan logika audit selalu mem-invalidasi cache otomatis.

## PHASE 1: COMPLETE (T1-T7b semua selesai, diverifikasi user di device)

Task 8+9 (Phase 2): complete (commits b3cfc3c + 97e9982, executor ses_f0e3f11c6ffetOkC3cK6R2XuyK)
- Registry: 16 CAP (CAP-WEB-001..016) di `Docs/superpowers/compat/COMPAT-REGISTRY.md` + marker komentar di shim (16↔16 grep dua arah diverifikasi).
- Test gaps ditutup: 7 (CAP-001 fallback, 010, 011, 012, 013, 014, 016). Suite total 18, semua hijau (supervisor re-run: PASS).
- Plan direvisi (revisi user): Phase 3 jadi T1-only (universal smoke), T2/T3 future work, device via config CLI (AdbTarget generik, bukan MumuTarget hardcode).

## Sisa: T10 (probe T1 Rust), T11 (verifikasi positif), T12 (--probe flag), T13 (final review + merge)

Task 10: complete (commit b7705ce, executor ses_f0e3556fcffeF9gzXeWI9itEJo, TDD 10 test baru)
- 11 file, 1.392 baris: modul probe terpecah (target/adb/config/scenario/report), AdbTarget generik config-driven (nol hardcode device; "mumu" hanya di devices.toml). 353 test workspace hijau. Smoke `--list-devices` jalan.

Task 11: complete — FIX RACE CONDITION + verifikasi device PASS
- Bug ditemukan supervisor dengan bukti device (pid 3277 hidup padahal probe vonis mati): `pidof` dicek ~300ms setelah launch async monkey → proses belum spawn → false RT-PROCESS-CRASH.
- Fix commit 789aa8e (executor ses_f0e24018bffe2sYF46NIBO6WU3, TDD 2 test regression baru): grace period muncul-proses 8s + kode jujur RT-LAUNCH-NO-PROCESS. 355 test hijau.
- Verifikasi device OLEH SUPERVISOR langsung (QA agent tidak dipakai ulang — hemat): probe di MuMu = **SELURUHNYA LOLOS** (boot 36977ms, new_game 58093ms, touch_input, gamepad, stability — semua PASS; report dist\probe-report.json).
- Pelajaran: launch async but grace period — dicatat ke technical_findings saat penutupan.








