# Task 5 Brief: Release Build & Host-Side Inspection

**Plan:** `Docs/superpowers/plans/2026-09-30-asset-hardening-overlay-lockdown.md` (Task 5)

**Workspace:** worktree `D:\02_Development\Projects_2\RPGM-P\.worktrees\asset-hardening` (branch `feature/asset-hardening`). Windows, PowerShell 5.1.

## Goal
1. Build APK Forestia dengan `--flavor release` ke output `dist_release/`.
2. Verifikasi host-side (tanpa emulator untuk menghemat RAM):
   - Unzip / inspect isi file `Forestia1.4.4-IND.apk` hasil release:
     - `assets/overlay/` **TIDAK ADA** (0 file).
     - `assets/web_shims/` **TIDAK ADA** (0 file).
     - `assets/fonts/` **TIDAK ADA** (0 file).
     - `assets/game.pack` **ADA** (mengandung chunk terenkripsi).
     - `classes.dex` minified oleh R8.
     - `build-manifest.json` mencatat `flavor: release`.
   - Buka `assets/game.pack` dengan `GpackReader` atau script verifikasi: pastikan entry `overlay/index.html`, `overlay/vpad.js`, dan `web_shims/runtimepack_web_shims.js` ada di dalam container biner!
3. Siapkan panduan manual testing singkat untuk user menguji di MuMu saat RAM santai.

## Steps

- [ ] **Step 1: Build release APK**
```powershell
cargo run -q --bin runtimepack -- build "D:\02_Development\Projects_2\RPGM-P\Raw-Game\Forestia1.4.4-IND" --flavor release --output dist_release
```

- [ ] **Step 2: Inspect APK contents via zip reader**
Inspect isi APK release (`dist_release/Forestia1.4.4-IND.apk`):
- Pastikan tidak ada file `assets/overlay/*`
- Pastikan tidak ada file `assets/web_shims/*`
- Pastikan tidak ada file `assets/fonts/*`
- Pastikan ada `assets/game.pack`

- [ ] **Step 3: Inspect GPACK container entries**
Verifikasi bahwa `game.pack` release memuat:
- `overlay/index.html`
- `overlay/vpad.js`
- `web_shims/runtimepack_web_shims.js`

- [ ] **Step 4: Report hasil inspeksi**
Tuliskan bukti lengkap ke report.

## Report contract
Tulis report lengkap ke:
`D:\02_Development\Projects_2\RPGM-P\.worktrees\asset-hardening\.superpowers\sdd\2026-09-30-asset-hardening-overlay-lockdown\task-5-report.md`
