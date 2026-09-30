# Task 1 Brief: Packer — bundle overlay/shim/fonts into GPACK for release flavor

**Plan:** `Docs/superpowers/plans/2026-09-30-asset-hardening-overlay-lockdown.md` (Task 1)

**Workspace:** worktree `D:\02_Development\Projects_2\RPGM-P\.worktrees\asset-hardening` (branch `feature/asset-hardening`, BASE `404749c`). Windows, PowerShell 5.1.

## Goal
Pada build dengan `--flavor release`, file layout overlay (`overlay/*`), web shims (`web_shims/*`), dan fonts (`fonts/*`) **wajib dimasukkan ke dalam `game.pack` (GPACK binary)**, dan **TIDAK disalin sebagai file plain** ke folder `app/src/main/assets/` project Android.
Pada build dengan `--flavor beta`, perilaku saat ini **TIDAK BERUBAH** (file plain tetap disalin ke `assets/overlay/`, `assets/web_shims/`, `assets/fonts/` untuk kemudahan debugging).

## Architecture & Code Locations
1. **Container Packing (`cli/src/main.rs`)**:
   - `game.pack` di-pack sekitar line 1028-1065 menggunakan `GpackPacker::pack_from_index_with_options(&index, &pack_path, ...)`.
   - `index` bertipe `GameSourceIndex` (`core/src/ingest/mod.rs`), yang memiliki field `pub files: HashMap<String, IndexedFile>` dan `pub total_size_bytes: u64`.
   - Tambahkan helper (misalnya `pub fn append_runtime_overlay_assets(index: &mut GameSourceIndex, overlay_src: &Path, shims_src: &Path, fonts_src: Option<&Path>) -> Result<(), ...>` atau langsung di `cli/src/main.rs`):
     Jika `normalized_flavor == "release"`:
     - Scan file-file di template overlay (dapat dicari via `AndroidProjectGenerator::find_overlay_src()`), template web_shims (`AndroidProjectGenerator::find_template_file("android/shell/app/src/main/assets/web_shims")`), dan fonts (`find_template_file("android/shell/app/src/main/assets/fonts")`).
     - Masukkan setiap file ke `index.files` dengan `logical_path`: `"overlay/<rel>"`, `"web_shims/<rel>"`, `"fonts/<rel>"`.
     - Gunakan `IndexedFile::with_source_root(logical_path, rel_path, size_bytes, source_root)`.
     - Update `index.total_size_bytes += size_bytes`.
     - Dengan demikian, `GpackPacker` otomatis mengompresi dan mengenkripsi seluruh file layout tersebut ke dalam `game.pack`!

2. **Project Generator Asset Omission (`core/src/build_plan/generator.rs`)**:
   - Di `AndroidProjectGenerator::emit_project_with_themes` (sekitar line 700-840):
     `effective_flavor` dihitung di line 711.
   - Jika `effective_flavor == "release"`:
     - **JANGAN** copy `overlay_src` ke `overlay_dst` (line 704).
     - **JANGAN** copy `shims_src` ke `shims_dst` (line 815).
     - **JANGAN** copy `fonts_src` ke `fonts_dst` (line 821).
     - (Catatan: jika `themes` atau `cheat_profile` di-inject, pastikan tidak crash bila folder belum ada — bungkus guard).
   - Jika `effective_flavor == "beta"`:
     - Tetap copy seperti sekarang (perilaku lama 100% utuh).

3. **Integration Test (`tests/integration/build_test.rs`)**:
   - Tulis 3 test baru:
     a. `test_release_build_omits_plain_overlay_and_shims_assets`
     b. `test_beta_build_preserves_plain_overlay_and_shims_assets`
     c. `test_release_gpack_contains_overlay_and_shim_entries`

## TDD Steps

- [ ] **Step 1: Write failing integration tests**
Tambahkan di `tests/integration/build_test.rs`:
Uji pemanggilan generator atau mock index/packing dengan `flavor: "release"` vs `"beta"`.
Assert bahwa pada output generated project release, `app/src/main/assets/overlay` dan `web_shims` tidak ada; sedangkan pada beta ada.
Assert bahwa `GpackReader::open(&pack_path)` memuat `reader.has_file("overlay/index.html") == true` dan `reader.has_file("web_shims/runtimepack_web_shims.js") == true` pada release pack.
Run: `cargo test --test build_test test_release` → FAIL.

- [ ] **Step 2: Implementasi perubahan di `core/src/build_plan/generator.rs` dan `cli/src/main.rs`**
Pastikan guard flavor rapi, no regression pada beta.

- [ ] **Step 3: Jalankan seluruh test suite**
```powershell
cargo test --workspace
node android/shell/app/src/test/js/web_shims.test.js
```
Semua test harus PASS (355+ tests).

- [ ] **Step 4: Commit**
```powershell
git add core/ cli/ tests/
git commit -m "feat(pack): bundle overlay, web shims, and fonts into GPACK for release flavor"
```

## Report contract
Tulis report lengkap ke:
`D:\02_Development\Projects_2\RPGM-P\.worktrees\asset-hardening\.superpowers\sdd\2026-09-30-asset-hardening-overlay-lockdown\task-1-report.md`
Isi: file yang diubah, ringkasan kode, output failing test (Step 1), output passing test (Step 3), commit hash.

Reply ke supervisor HANYA:
- Status: (SUCCESS / FAIL)
- Commit hash
- Test summary satu baris
- Concerns (jika ada)
