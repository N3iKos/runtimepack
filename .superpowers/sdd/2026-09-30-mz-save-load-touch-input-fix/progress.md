# SDD ledger — plan: Docs/superpowers/plans/2026-09-30-mz-save-load-touch-input-fix.md

## Pre-flight

- Worktree: `.worktrees/native-mv-mz-save`, branch `feature/native-mv-mz-save`, BASE `385df60a36c01503ac158a42ff41e08a93840711`.
- Uncommitted work present on entry (Save Adapter dari plan sebelumnya: `runtimepack_web_shims.js`, `WebBackend.kt`, `web_shims.test.js` termodifikasi; `NativeSaveRepository.kt` + test baru untracked). Ini adalah deliverable plan `native-mv-mz-save` Task 5 yang sudah dibuild APK-nya. TIDAK di-commit oleh sesi ini; plan baru hanya menumpuk perubahan di atasnya. COMMIT PLAN BARU HARUS HANYA MENAMBAH PERUBAHAN TASK INI — hati-hati `git add` selektif, jangan commit file adapter lama secara tidak sengaja.
- Spec: `Docs/FORESTIA_SAVE_LOAD_HANDOVER.md` + riset deepresearch 2026-09-30 (diringkas di section Evidence plan). Plan self-contained; rulings tanpa spec eksternal bersifat provisional.

## Conflict scan (pre-flight rulings)

| Pair / Task | Produces vs Consumes | Finding | Ruling |
| :--- | :--- | :--- | :--- |
| Task 1 vs Task 2 | Task 1 produces `window.RuntimePackInput` (press/release/trigger/isAvailable) + `Input.codeMapper`; Task 2 `dispatchKeyToGame` mengonsumsi keduanya | Konsisten; key format `"KeyZ"`/`"Enter"` identik antara `codeMapper` JS dan `codeMap` Kotlin | Tidak ada konflik |
| Task 1 vs uncommitted adapter lama | Task 1 memodifikasi `runtimepack_web_shims.js` + `web_shims.test.js` yang SUDAH punya diff uncommitted dari plan sebelumnya | Diff akan tercampur dalam satu file. Test suite lama harus tetap hijau (Task 1 Step 4 menjalankan seluruh file) | Diterima: perubahan plan lama belum di-commit tapi sudah terverifikasi (APK dibuild, save fisik valid). Task 1 reviewer harus melihat diff `BASE..HEAD` yang mengandung keduanya — diberitahu di brief bahwa bagian save-adapter dalam diff adalah kerjaan plan sebelumnya yang sudah diverifikasi terpisah, reviewer fokus pada blok `patchEngineInput` |
| Task 1 vs Global Constraints | "Jangan menyentuh jalur save yang sudah bekerja" vs file yang sama berisi `patchMzStorage` | Task 1 hanya MENAMBAH fungsi `patchEngineInput` + hook `pollPatches`; tidak mengubah `patchMzStorage`/`patchStorageManager`/`isLocalMode` | Sesuai |
| Task 1 internal | Test mock `Window_SavefileList.prototype.isTouchOkEnabled` override — test mendefinisikan mock engine SETELAH shim jalan, lalu memanggil `__runtimepackPollEngineInput()` | Konsisten dengan implementasi late-binding polling | Tidak ada konflik |
| Task 2 internal | `dispatchKeyToGame` baru mempertahankan signature `(jsKeyCode: Int, type: String)`; dua call-site (line ~622 gamepad shortcut, ~801 `dispatchKeyEvent`) tidak berubah | Konsisten | Tidak ada konflik |
| Task 4 | Build command `python -m runtimepack build ... --flavor beta` mungkin berbeda dari command baku sesi sebelumnya | Eksekutor harus cek `docs/IMPLEMENTATION_PLAN.md` / entry point CLI aktual sebelum build; jika command berbeda, pakai command yang terbukti dari sesi sebelumnya dengan `--flavor beta` | Ruling: command build bersifat "best-known", eksekutor memverifikasi — Ruling ini murah: salah command hanya menyita satu iterasi build |

Scan bersih untuk konflik blocking; rulings tercatat di atas.

## Progress

Task 1: complete (commits 385df60..dc09d92, review clean)
- Spec ✅, quality Approved. Implementer: executor (ses_f117a5d80ffeQhhI01tOKRAETk).
- TDD evidence: red (moveThreshold assertion) → green ("Engine Input Patch tests PASSED"); seluruh suite web_shims.test.js hijau.
- Ruling pre-flight diterapkan: commit menyertakan perubahan save-adapter uncommitted plan sebelumnya pada 2 file JS yang sama (disetujui; file Kotlin & WebBackend.kt TIDAK ikut).
Task 1: minor (deferred): `RuntimePackInput.trigger` reset state secara sinkron sehingga tidak efektif lintas frame — dipakai Task 2? Tidak (Task 2 hanya press/release). Final review wajib triage.
Task 1: minor (deferred): `pollPatches` clearInterval predicate tidak mengecek `Window_Selectable` saat polling (hanya Input & TouchInput). Impact kecil: patch tetap jalan tiap 10ms sampai Input+TouchInput ada. Final review wajib triage.

Task 2: complete (commits dc09d92..e054bcb, review clean)
- Spec ✅, quality Approved, 0 issue. Implementer: executor (ses_f11757c30ffexn1MG3MroijWHF).
- Verifikasi: `:app:compileDebugKotlin` BUILD SUCCESSFUL (15/15 tasks), 5 suite JS hijau exit 0.

Task 3 (final review): complete — APPROVED_FOR_BUILD
- 0 Critical, 0 Important, tidak ada temuan baru.
- Triage deferred minor 1 (`RuntimePackInput.trigger` sync reset): ACCEPTABLE — tidak dikonsumsi siapa pun (Kotlin hanya pakai press/release).
- Triage deferred minor 2 (clearInterval predicate tanpa Window_Selectable): ACCEPTABLE — script core MZ dimuat sinkron, patch tetap terpasang saat inisialisasi.

Task 4 (build): complete
- Command: `cargo run -q --bin runtimepack -- build "D:\02_Development\Projects_2\RPGM-P\Raw-Game\Forestia1.4.4-IND" --flavor beta --output dist`
- Output: `dist\Forestia1.4.4-IND.apk` (856.240.549 bytes, 816.57 MB), manifest `"flavor": "beta"`, audit PASS (cache hit), engine RPG Maker MZ EXACT, package `io.runtimepack.game.forestia144ind`.
- Verifikasi bundle: `dist\android-project\...\assets\web_shims\runtimepack_web_shims.js` mengandung `patchEngineInput`/`RuntimePackInput`/`moveThreshold = 24` (8 hits); `WebActivity.kt` project mengandung `defineProperty`/`RuntimePackInput`/`codeMap` (11 hits).
- Ruling pre-flight build command terpenuhi: `python -m runtimepack` salah — actual CLI adalah Rust binary via `cargo run --bin runtimepack`; plan command dianggap best-known, tidak jadi masalah.
- Pengujian manual di MuMu: diserahkan ke user (checklist 6 poin di plan Task 4 Step 2).

## Tindak lanjut sesi 30 Sep (debug lanjutan setelah uji manual user "masih sama")

Ruling: uji manual user pertama TIDAK VALID — bukti `pm list packages` di MuMu hanya menampilkan `com.example.forestia` (APK lama); APK baru (package `io.runtimepack.game.forestia144ind`) belum terinstall. Semua debug berikutnya via sub-agent (instruksi user: jangan debug inline).

- QA sub-agent (ses_f11538dc5ffewt2FEIxbBlElIm): install APK baru + migrasi save dari app lama (root, file0/file1/global/system terverifikasi). LOAD TEST FAIL tapi dengan bukti baru: D-pad menggerakkan kursor (input injection bekerja), 60 FPS stabil, tanpa error JS, ada preload Map JSON via VFS. → gejala bergeser: input OK, load pipeline macet diam-diam.
- Diag executor (ses_f114107feffeHei5KO6bChZibt, commit `be978f9`): instrumentasi rantai load `[RP_LOAD_DEBUG]` di shim. BUKTI: tap 1x → `processOk idx=1` → `executeLoad id=1` → `loadObject` resolve 30-property save ~150ms → **`loadGame rejected: windowLayer.children[0].convertEscapeCharacters is not a function`** → `onLoadFailure`. Input & storage 100% sehat. (Anomali: `isLoadableSlot`/`onLoadInvalid` tidak ada di MZ core — asumsi plan salah, tidak berdampak.)
- Fix executor (ses_f112748caffeJ7M0cwbZ0dryCI, commit `a14860b`): analisis plugin HalfMove.js/HalfMoveEx.js — PC aman karena `save/config.rmmzsave` bawaan berisi `touchUI:false` → tanpa `_cancelButton` Sprite_Button → `children[0]` = Window_Help. Fix: `Sprite_Button.prototype.convertEscapeCharacters` delegasi ke `_helpWindow` → Window_Base pertama di `_windowLayer.children` → fallback `Window_Base.prototype` (option B, preserving pemrosesan teks plugin). **Verifikasi emulator: PASS — tap 1x masuk map; virtual gamepad A PASS.**
- Review fix (ses_f11151a4effeDPDr1Y1tSCQwSt): **Approved**. 2 minor deferred:
  1. Banyak tombol tanpa helpWindow berisiko mutual recursion (delegasi Sprite_Button→Sprite_Button).
  2. Fallback `Window_Base.prototype.convertEscapeCharacters.call(spriteButton)` mengikat receiver non-window.

Task 5 (diagnostic): complete. Task 6 (root-cause fix): complete (commits be978f9..a14860b, review approved).

## Sisa pekerjaan (belum done)
- [ ] Hapus instrumentasi `[RP_LOAD_DEBUG]` (commit be978f9) + rebuild final APK bersih.
- [ ] Uji manual user di APK final bersih.
- [ ] Commit file uncommitted plan save-adapter lama (`WebBackend.kt`, `NativeSaveRepository.kt`, `NativeSaveRepositoryTest.kt`).
- [ ] Update memory/technical_findings.md repo utama dengan root cause HalfMove (SUDAH dikerjakan supervisor — lihat catatan di bawah).
- [ ] Triage 2 minor deferred di atas saat cleanup final.





