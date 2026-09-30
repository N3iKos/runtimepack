# Task 1+2 Brief: Selective commit probe false-positive fixes di MASTER

**Plan:** `Docs/superpowers/plans/2026-09-30-stability-and-diagnostics-overhaul.md` (Task 1 + Task 2, digabung karena satu bentuk kerja yang sama)

**Workspace:** repo utama `D:\02_Development\Projects_2\RPGM-P` (branch `master`). PENTING: ini BUKAN worktree — semua command `git` dijalankan dengan workdir repo utama itu.

## Context
Master punya **19 file uncommitted** milik beberapa track pekerjaan berbeda (Track 3, Track D). Anda hanya boleh melakukan **selective stage** untuk file milik fix probe. JANGAN menyentuh/men-commit file lain (terutama `AGENTS.md`, `RenpyActivity.kt`, `memory/*`, `Docs/execution-plans/*`).

Fix yang mau di-commit sudah ADA di working tree (ditulis sesi sebelumnya, terverifikasi oleh explorasi sub-agent):
1. `core/src/asset_audit/report.rs` — perubahan `rposition` → `position` pada sanitasi path (sekitar line 148-154) + regression test `test_sanitize_path_str_preserves_js_plugins_prefix`. Ini fix "1158 false positive" — sanitasi sebelumnya memotong prefix `js/` dari path `js/plugins/...` sehingga logical path di asset-dependency-graph.json jadi salah (`plugins/...` tanpa `js/`) dan probe FILE_EXISTS melaporkan semua 1158 plugin hilang.
2. `android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt` + `android/shell/app/src/main/java/io/runtimepack/host/AssetAuditService.kt` — tambahan param `gpackReader`, lookup `reader?.hasFile(relPath)` di probe loop, discovery graph/report dari stream GPACK, fallback scan `reader.entries`.

## Task 1 Steps

- [ ] **Step 1: Verifikasi diff Task 1**

```powershell
git diff HEAD -- core/src/asset_audit/report.rs
```
Expected: `rposition(|segment|` → `position(|segment|` + test baru `test_sanitize_path_str_preserves_js_plugins_prefix` yang assert `"js/plugins/inazuma/Activity.js"` tetap `"js/plugins/inazuma/Activity.js"`.

Jika diff TIDAK sesuai expected (misal fix belum ada atau bentuknya berbeda), STOP dan report BLOCKED dengan isi diff aktual.

- [ ] **Step 2: Test Rust**

```powershell
cargo test -p runtimepack-core asset_audit -- --nocapture
```
(Nama package mungkin `runtimepack-core` atau lain — cek `Cargo.toml` workspace members bila command gagal; gunakan nama package yang benar.)
Expected: semua test asset_audit PASS termasuk test baru.

- [ ] **Step 3: Commit selective**

```powershell
git add core/src/asset_audit/report.rs
git commit -m "fix(asset-audit): preserve js/ prefix for js/plugins paths in logical path sanitization (1158 false positives)"
```

## Task 2 Steps

- [ ] **Step 4: Verifikasi diff Task 2**

```powershell
git diff HEAD -- android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt android/shell/app/src/main/java/io/runtimepack/host/AssetAuditService.kt
```
Expected: `gpackReader` param + `reader?.hasFile(relPath)` + GPACK stream discovery + fallback `reader.entries`.

Periksa juga apakah `android/shell/app/src/test/java/io/runtimepack/host/AssetProbeRunnerTest.kt` dan `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt` termasuk kesatuan fix ini (lihat diff-nya):
- `AssetProbeRunnerTest.kt` (untracked/modified): jika isinya test untuk GPACK lookup → ikut di-commit Task 2.
- `VfsResourceInterceptor.kt`: jika diff-nya bagian dari perbaikan probe/VFS terkait → ikut; jika milik pekerjaan lain (misal Ren'Py overlay Track D) → JANGAN ikut. Putuskan dari isi diff; ragu = jangan ikut + catat di report.

- [ ] **Step 5: Kompilasi + test Kotlin**

```powershell
cd android\shell
gradlew.bat :app:compileDebugKotlin :app:testDebugUnitTest --console=plain
```
Expected: BUILD SUCCESSFUL, `AssetProbeRunnerTest` PASS (jika ada).

- [ ] **Step 6: Commit selective**

```powershell
git add android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt android/shell/app/src/main/java/io/runtimepack/host/AssetAuditService.kt
# plus AssetProbeRunnerTest.kt bila diputuskan ikut (dan VfsResourceInterceptor.kt bila ikut)
git commit -m "fix(android-host): probe resolves assets inside GPACK instead of disk-only lookup"
```

## Verifikasi akhir
```powershell
git log --oneline -3
git status --porcelain   # sisa file uncommitted lain harus tetap ada, TIDAK ikut ter-commit
```

## Report contract
Tulis report lengkap ke:
`D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save\.superpowers\sdd\2026-09-30-stability-and-diagnostics-overhaul\task-1-2-report.md`
isi: hasil verifikasi diff (kutipan penting), hasil test, 2 commit hash, keputusan file tambahan (test/interceptor ikut atau tidak + alasan), sisa file uncommitted yang sengaja ditinggalkan (list nama saja).

Reply ke saya HANYA: status, 2 commit hash, ringkasan test satu baris, keputusan file-tambahan satu baris, concerns.
