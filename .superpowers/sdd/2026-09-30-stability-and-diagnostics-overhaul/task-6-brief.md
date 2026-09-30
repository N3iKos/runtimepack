# Task 6 Brief: Commit file save-adapter yang tersisa di worktree

**Plan:** `Docs/superpowers/plans/2026-09-30-stability-and-diagnostics-overhaul.md` (Task 6, digeser ke depan merge karena file tracked yang dimodifikasi memblokir merge — ruling controller)

**Workspace:** worktree `D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save` (branch `feature/native-mv-mz-save`). Windows, PowerShell 5.1.

## Context
File berikut adalah deliverable plan save-adapter sebelumnya yang belum pernah di-commit (save-nya sudah terbukti bekerja di device: file1 valid, load PASS):
- `android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt` (modified — tambahan 5 method bridge native save: `saveSlot`, `loadSlot`, `slotExists`, `deleteSlot`, `listSlots`)
- `android/shell/app/src/main/java/io/runtimepack/host/NativeSaveRepository.kt` (untracked — modul Kotlin penyimpanan atomik NIO `ATOMIC_MOVE` + fsync)
- `android/shell/app/src/test/java/io/runtimepack/host/NativeSaveRepositoryTest.kt` (untracked — JVM test)

## Steps

- [ ] **Step 1: Verifikasi diff WebBackend.kt hanya berisi 5 bridge method save**

```powershell
git diff HEAD -- android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt
```
Expected: tambahan method `saveSlot`/`loadSlot`/`slotExists`/`deleteSlot`/`listSlots` (dengan anotasi `@JavascriptInterface`), tidak ada perubahan lain yang tidak terkait save. Bila ada perubahan lain, evaluasi: kalau bagian kesatuan adapter save → boleh ikut; kalau milik pekerjaan lain → STOP report BLOCKED.

- [ ] **Step 2: Baca cepat NativeSaveRepository.kt + test-nya** — pastikan tidak ada debug code/kode mati yang jelas (komentar TODO besar, print debug). Kalau bersih, lanjut.

- [ ] **Step 3: Jalankan test JVM**

```powershell
cd android\shell
gradlew.bat :app:compileDebugKotlin :app:testDebugUnitTest --console=plain
```
Expected: BUILD SUCCESSFUL, `NativeSaveRepositoryTest` PASS.

- [ ] **Step 4: Commit selective (3 file saja)**

```powershell
git add android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt android/shell/app/src/main/java/io/runtimepack/host/NativeSaveRepository.kt android/shell/app/src/test/java/io/runtimepack/host/NativeSaveRepositoryTest.kt
git commit -m "feat(android-host): atomic native save repository with JVM tests (save adapter finalization)"
```

- [ ] **Step 5: Verifikasi akhir** — `git status --porcelain` tidak lagi menampilkan file tracked modified (hanya untracked sdd/docs yang boleh tersisa).

## Report contract
Tulis report lengkap ke:
`D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save\.superpowers\sdd\2026-09-30-stability-and-diagnostics-overhaul\task-6-report.md`

Reply ke saya HANYA: status, commit hash, ringkasan test satu baris, concerns.
