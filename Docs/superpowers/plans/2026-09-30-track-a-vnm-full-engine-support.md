# Plan: Track A — Visual Novel Maker (VNM) Full Engine Support

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Menjadikan Visual Novel Maker (VNM) engine first-class citizen penuh di RuntimePack dengan **paritas fitur lengkap** dengan engine lain: deteksi statis EXACT, asset audit reachability + probe GPACK, save/load native adapter, shims NW.js, virtual gamepad VN preset, diagnostic overlay, asset hardening GPACK, dan verifikasi end-to-end dengan game nyata *Time for Punishment Class ~Taking Lessons for My Boyfriend~* (GOS-01).

**Architecture:** VNM = HTML5/WebGL (runtime `gs`) dalam wrapper NW.js → reuse penuh `WebActivity.kt` + VFS streaming GPACK + `runtimepack_web_shims.js` yang sudah terbukti. Crate baru `engines/vnmaker` (detector + auditor). Data game terenkripsi asm.js (`data/lib/boot.js` → `Module.ccall`) dibiarkan utuh — engine `gs` yang men-decrypt sendiri saat runtime di WebView, kita hanya menyajikan file-nya.

**Tech Stack:** Rust (`core`, `engines/vnmaker`, `cli`), Kotlin (`WebActivity.kt` reuse), JavaScript (shim reuse + VN preset).

**Spec:** `Docs/execution-plans/track-a-visual-novel-maker/design.md` + `plan.md` (roadmap approved 2026-09-27) + temuan riset deepresearch 2026-09-30 + inspeksi langsung game GOS-01 + `_modding_notes/technical_findings.md` (enkrripsi asm.js, pola `GS.dataCache`, Live2D).

## Target Uji
- **Utama**: `Raw-Game\Time for Punishment Class ~Taking Lessons for My Boyfriend~` (VNM, ~1.4 GB, NW.js export, Live2D, data terenkripsi asm.js).
- **Negatif/deteksi-silang**: fixture game lain (Forestia MZ, MV, Tyrano) sudah ada di `Raw-Game/` — pastikan tidak salah deteksi.

## Fitur Wajib Paritas Engine Lain (dari audit engine existing)
1. **Deteksi statis** EXACT/HIGH dengan kode `DET-VNM-xxx` + tidak konflik dengan MV/MZ/Tyrano.
2. **Asset audit statis** (reachability graph, case-folding, ruleset version bump) + **probe on-device GPACK** (`AssetProbeRunner` sudah generik via graph).
3. **Save/load native** — riset: VNM `gs.GameStorage` fallback ke `localStorage` saat Node absen → cukup `setDomStorageEnabled(true)` + **web shim save adapter ke native bridge** (pola CAP-WEB-002/003) supaya save persist di `filesDir` bukan localStorage rawan clear.
4. **Web shims** — `nw.gui`/`nw.Window`/`process` stub (sudah ada), `gs.Application` window/exit handling.
5. **Virtual gamepad VN preset** — tombol floating: Quick Save, Quick Load, Backlog, Auto, Skip, Hide UI (wire ke `gs` API/keyboard shortcut).
6. **Diagnostic overlay + engine_family jujur** (`AssetAuditService.currentEngine` = "Visual Novel Maker" via build manifest — otomatis dari engine family).
7. **Asset hardening release** (B+C+D) — otomatis karena generik via flavor.
8. **Probe CLI T1** — otomatis jalan (universal smoke test).
9. **CAP registry** — entri CAP-WEB-0xx baru untuk shim VNM spesifik.
10. **Test port report** — `Docs/test_ports/vnm_time_for_punishment_class/report.md` mengikuti format game lain.

---

## Global Constraints

- `Raw-Game/` 100% immutable (termasuk `_modding_notes/` — jangan disentuh, itu catatan user).
- Semua deteksi/audit **in-memory zero-disk**.
- Build autonomous wajib `--flavor beta`.
- Kode diagnostik: `DET-VNM-xxx` (deteksi), `CMP-AUD-xxx` reuse (audit), `WEB-xxx` untuk shim baru.
- Setiap task: `cargo test --workspace` hijau sebelum commit.
- **Aturan audit cache**: kalau perilaku `asset_audit` berubah (registrar auditor baru termasuk), bump `CURRENT_AUDIT_RULESET_VERSION` — cek apakah menambah auditor mempengaruhi graph; kalau ya bump.
- Ikuti preseden struktur engine lain persis: `engines/tyrano/`, `engines/renpy/` sebagai referensi pola (crate layout, trait implementation, registrasi di `cli/src/main.rs`).

---

## Task Breakdown

### Task 1: Domain & Engine Registration
- Modify: `core/src/domain.rs` (`EngineId::VisualNovelMaker`, `as_str() = "vnmaker"`, display "Visual Novel Maker", `runtime_family() = Some(RuntimeFamily::Web)`), `schemas/build-manifest.schema.json` (+`"vnmaker"`).
- TDD: `detection_test.rs` — domain id + runtime family. (Konten lengkap ada di plan lama Task 1, baris 39-63 — pakai verbatim.)

### Task 2: Static Detector `engines/vnmaker`
- Create: `engines/vnmaker/` crate (`VnMakerDetector`).
- Scoring dari design.md §3.2: `signature` +90 (`DET-VNM-001`), VNM `data/System.json` schema +80 (`DET-VNM-002`), `package.json` main=index.html +40 (`DET-VNM-003`), `data/Scenes.json`/`Layouts.json` +50 (`DET-VNM-004`). Guard anti-konflik: `js/rpg_core.js`/`js/rmmz_core.js`/`tyrano/` → None.
- **Catatan dari game nyata GOS-01**: `data/System.json` klasik mungkin TIDAK ada (game ini pakai `data/*.json.js` ter-hash + `ENGINE.js` + `SCRIPTS.js` + `globalData.vndata`) → tambah marker `DET-VNM-005`: keberadaan `data/ENGINE.js` ATAU `data/SCRIPTS.js` ATAU file `*.vndata` di root (+70). Pastikan GOS-01 terdeteksi minimal `HIGH` ideal `EXACT` — tulis test dengan fixture mini yang meniru struktur GOS-01 (signature + package.json main=index.html + data/ENGINE.js).
- TDD: 3 test (fixture VNM klasik → EXACT; fixture GOS-01 style → ≥HIGH; Forestia MZ → bukan VNM).

### Task 3: Asset Reachability Auditor `engines/vnmaker/src/asset_audit.rs`
- Create: `VnMakerAssetAuditor` (trait `AssetAuditor` seperti Tyrano/RenPy).
- Parse referensi dari `data/Scenes.json`/`data/System.json` klasik **DAN** dari `data/SUMMARIES.json.js`+`RESOURCES.json.js` (GOS-01 style; `RESOURCES.json.js` = manifest aset → sumber reachability utama). Auditor tidak perlu men-decrypt file terenkripsi — hanya `RESOURCES.json.js`/`SUMMARIES.json.js` yang plaintext.
- Direktori aset: `resources/Graphics|Audio|Live2D|Movies|other|v1..v5`, `fonts/`. Case-folding alias (pola engine lain).
- Reachability: file di `resources/` yang tidak direferensikan → `CMP_AUD_POSSIBLY_UNUSED` (Info, bukan fail).
- Registrasi di `core/src/asset_audit/mod.rs` + `cli/src/main.rs`. **Bump `CURRENT_AUDIT_RULESET_VERSION`** (graf audit berubah).
- TDD: `tests/integration/vnmaker_asset_audit_test.rs` — fixture sintetis (Scenes.json klasik + RESOURCES.json.js style), aset ada → Pass; case-mismatch → Warning; missing → Error/finding.

### Task 4: Shims & Save Adapter untuk VNM (`runtimepack_web_shims.js` + Kotlin)
- Modify: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` — `patchVnmCompat()` late-binding (guard `__runtimepack_vnm_patched`, poll saat `root.gs` atau `GS` terdeteksi):
  1. `gs.GameStorage`/localStorage save → redirect ke native bridge `RuntimePackNative.saveSlot("vnm", key, b64)` / `loadSlot` (pola CAP-WEB-002) dengan fallback localStorage murni bila bridge absen (unit test Node).
  2. `nw.gui`/`nw.Window`/`process.exit` stub sudah ada — verifikasi cukup untuk `gs.Application` (exit → `RuntimePackNative.exitGame()` bila ada, else no-op).
  3. CAP registry marker `// [CAP-WEB-0xx]`.
- Verify `WebActivity` sudah `setDomStorageEnabled(true)` + `setMediaPlaybackRequiresUserGesture(false)` (cek; tambah bila belum).
- TDD: suite JS baru "VNM Save Adapter" di `web_shims.test.js` (mock `gs`/`GS` + bridge, assert save→slot ter-redirect, load→kembali, fallback localStorage).

### Task 5: VN Preset Gamepad + Generator & CLI Wiring
- Modify: `android/shell/app/src/main/assets/overlay/vpad.js`/`cheat_profile.js` — **VN preset**: tombol Quick Save/Quick Load/Backlog/Auto/Skip/Hide UI (mapping ke keyboard shortcut `gs` bila ada, atau `RuntimePackInput`); preset aktif otomatis untuk engine `vnmaker` (profil per-engine sudah ada polanya di cheat_profile).
- Modify: `core/src/build_plan/generator.rs` — `"vnmaker" => WebActivity` (launcher, packaging `index.html`, `package.json`, `signature`, `data/`, `resources/`, `fonts/`, `js/` → GPACK; **exclude** file NW.js runtime: `nw.dll`, `node.dll`, `*.pak` runtime NW, `icudtl.dat`, `snapshot_blob.bin`, `natives_blob.bin`, `GOS01.exe`, `ffmpeg.dll`, `d3dcompiler_47.dll`, `libEGL/GLES*.dll` — ikuti pola exclusion yang sudah ada untuk MV/MZ NW.js builds; juga exclude `_modding_notes/`).
- Modify: `cli/src/main.rs` — registrasi detector/auditor/analyzer VNM ke registry default.
- TDD: `build_test.rs` — manifest vnmaker → emit WebActivity + vpad overlay + exclusion NW files dari pack list.

### Task 6: Build Game Nyata + Verifikasi On-Device
- Build GOS-01 `--flavor beta` → APK.
- Verifikasi host-side: audit statis (outcome + findings), GPACK berisi `index.html`/`data/ENGINE.js`/`resources/*`, TIDAK berisi `nw.dll`/`GOS01.exe`/`_modding_notes`.
- Install MuMu + QA (bila RAM host lega; else user manual): boot → title → new game → dialog jalan (data terenkripsi ter-load → membuktikan asm.js decrypt bekerja di WebView) → save → load → vpad VN tombol respons. Screenshot + report.
- Probe CLI T1: `runtimepack probe <apk> --device mumu` (universal smoke).
- Tulis `Docs/test_ports/vnm_time_for_punishment_class/report.md` (format mengikuti report game lain).

### Task 7: Registry, Memory & Penutupan
- `Docs/superpowers/compat/COMPAT-REGISTRY.md`: entri CAP baru (VNM save adapter, VN preset, deteksi) + update `Docs/ENGINE_SUPPORT_MATRIX.md` status VNM → Supported.
- Update `memory/progress.md`, `decisions_log.md`, `technical_findings.md` (termasuk temuan asm.js decrypt di WebView).
- Final review `code-reviewer` atas seluruh branch → merge via finishing-a-development-branch.

---

## Urutan Eksekusi & Dispatch Map

| Task | Sub-agent | Butuh device? |
|:---|:---|:---|
| 1-3 (Rust: domain, detector, auditor) | executor ×3 (TDD) | ❌ |
| 4 (shim save adapter) | executor (TDD JS) | ❌ |
| 5 (vpad preset + generator + CLI) | executor (TDD Rust+JS) | ❌ |
| 6 (build + verifikasi) | executor build + qa-tester ATAU user manual | ✅ (fleksibel) |
| 7 (review + docs + merge) | code-reviewer + supervisor | ❌ |

Review task standar SDD per task; final review di Task 7. Checkpoint user: setelah Task 6 (hasil uji game nyata).

## Self-Review
1. **Coverage paritas**: deteksi ✅(T2), audit+probe ✅(T3, graph→probe generik), save native ✅(T4), shims ✅(T4), vpad ✅(T5), diagnostic engine_family ✅(otomatis via manifest), hardening ✅(otomatis via flavor), probe CLI ✅(T6), CAP registry ✅(T7), test port report ✅(T6). Gap dari plan lama: tidak ada — semua task lama tercakup + tambahan GOS-01 marker & asm.js.
2. **Placeholder**: langkah konkret; Task 1 merujuk plan lama verbatim (file ada di repo).
3. **Konsistensi**: `DET-VNM-001..005`, slot ns `"vnm"`, preset id `vnmaker`, crate `runtimepack-engine-vnmaker` konsisten antar task.
4. **Risiko terbesar**: (a) game nyata terenkripsi asm.js — mitigasi: engine `gs` decrypt sendiri di WebView (kita tak perlu port decryptor; kalau ternyata butuh `boot.js` env khusus, fallback: eksekusi `boot.js` di shim — catatan user sudah membuktikan pola decrypt-nya di Node); (b) Live2D di WebView — `live2dcubismcore.min.js` sudah bundled game, WebGL WebView mendukung; (c) ukuran 1.4 GB → GPACK streaming sudah terbukti dengan Forestia 818 MB.
