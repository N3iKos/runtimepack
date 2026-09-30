# Task 8+9 Brief: Capability Registry + test gaps

**Plan:** `Docs/superpowers/plans/2026-09-30-stability-and-diagnostics-overhaul.md` (Task 8 + Task 9, digabung: satu bentuk kerja dokumentasi + test)

**Workspace:** worktree `D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save` (branch `feature/native-mv-mz-save`, HEAD `f3b3e20`). Windows, PowerShell 5.1.

## Context
Kontrak AGENTS.md §9: setiap compatibility shim butuh capability ID + implementation + test + diagnostic behavior. Saat ini semua patch di `runtimepack_web_shims.js` bekerja tapi tidak terdokumentasi terstruktur. Anda membuat registry dokumen + marker kode + menutup test gaps.

## Files
- Create: `Docs/superpowers/compat/COMPAT-REGISTRY.md`
- Modify: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (hanya komentar marker `// [CAP-WEB-xxx] ...` per fungsi patch — JANGAN ubah logika)
- Modify (Task 9, bila ada gap): `android/shell/app/src/test/js/web_shims.test.js` (test baru untuk patch yang belum teruji)

## Langkah

### Bagian A — Pemetaan (WAJIB sebelum menulis)
1. Baca `runtimepack_web_shims.js` penuh (file ~1500 baris). Buat daftar SETIAP fungsi patch: nama fungsi, guard flag, apa yang di-patch, gejala tanpa patch (dari komentar/knowledge di bawah), engine target (MV/MZ/umum).
2. Cek `web_shims.test.js` untuk tiap patch: ada suite yang mengujinya? Catat yang TIDAK punya test.

Knowledge gejala (dari riwayat sesi — jangan mengarang gejala lain):
- readdirSync bridge → tanpa patch: `TypeError getRoadRegion`, NPC pathfinding crash (plugin GlobalMap Forestia)
- MZ storage adapter (`saveToForage`/`loadFromForage` native bridge) → tanpa patch: save/load tidak persist di WebView
- MV webstorage adapter → padanan MV
- isLocalMode virtualization (`isLocalMode() = false`) → memaksa jalur forage/webstorage yang kita bridge
- single-tap touch OK (`Window_SavefileList`/`Window_Selectable.isTouchOkEnabled`) + `TouchInput.moveThreshold` 24 → tanpa patch: tap 1x di menu load hanya highlight, load tidak pernah tereksekusi; jitter jari dianggap scroll
- `Input.codeMapper` + `RuntimePackInput` → tanpa patch: `KeyboardEvent.keyCode === 0` di Chromium modern, tombol virtual gamepad mati
- HalfMove compat (`Sprite_Button.prototype.convertEscapeCharacters` delegasi ke helpWindow/Window_Base) → tanpa patch: `DataManager.loadGame` reject TypeError, load gagal senyap (bug Forestia)
- Load failure fail-loud (`Scene_Load.onLoadFailure` wrapper + `DataManager.loadGame` catch + `__runtimepackShowLoadError`) → tanpa patch: kegagalan load senyap, sulit didiagnosis
- nw.gui / nw / Greenworks / Steamworks mocks → tanpa patch: game yang memanggil NW.js API crash
- Graphics stretch fit (`_stretchWidth/_stretchHeight`) → tanpa patch: ukuran canvas tidak fit viewport Android
- AudioManager ogg ext → tanpa patch: ekstensi audio salah untuk WebView
- unhandledrejection/global error handler → logging diagnostik
- SceneManager.catchException wrapper → logging diagnostik

### Bagian B — Registry dokumen
Tulis `Docs/superpowers/compat/COMPAT-REGISTRY.md`:

```markdown
# RuntimePack Web Runtime — Capability Compatibility Registry

> Sumber kebenaran tunggal untuk shim compat engine web (MV/MZ).
> Setiap entri wajib punya: ID, trigger, symptom, implementation, evidence, regression test, diagnostic.
> Marker di kode: `// [CAP-WEB-xxx]` (grep-able).

## CAP-WEB-001 — <nama>
- Engine: ... | Trigger: ...
- Symptom (tanpa patch): ...
- Implementation: <file + fungsi>
- Evidence: ...
- Regression test: <suite name di web_shims.test.js> ATAU `[missing — Task 9]`
- Diagnostic: <kode/log jika bridge gagal>
```

Alokasikan ID sesuai urutan fungsi di file (001 untuk readdirSync dst — finalisasi urutan terserah Anda, yang penting konsisten dokumen↔kode).

### Bagian C — Marker kode
Tambahkan komentar satu baris `// [CAP-WEB-xxx] <nama capability>` tepat di atas tiap fungsi patch di shim. JANGAN mengubah logika sama sekali.

### Bagian D — Task 9: tutup test gaps
Untuk tiap patch yang tercatat `[missing]` di Bagian A: tulis test suite di `web_shims.test.js` (gaya suite yang ada: sandbox vm, mock globals, poll hook manual, assert perilaku nyata bukan tautologi). Fokus pada patch yang RISKAN dan belum teruji. Patch yang mock trivial (nw.gui kosong, Greenworks) boleh cukup assert "terdefinisi dan tidak throw saat diakses".

### Bagian E — Verifikasi + commit
1. Grep mapping dua arah: setiap CAP di dokumen ada marker-nya di kode, sebaliknya setiap marker ada di dokumen. (Tulis hasil hitungan di report.)
2. `node android/shell/app/src/test/js/web_shims.test.js` → semua hijau (lama + baru).
3. `node --check` shim tetap valid.
4. Commit (boleh satu atau dua commit):
```
docs(compat): capability registry for all web runtime shims (CAP-WEB-xxx..yyy)
test(web-runtime): close capability registry test gaps (CAP-WEB-xxx..)
```

## Report contract
Tulis report lengkap ke:
`D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save\.superpowers\sdd\2026-09-30-stability-and-diagnostics-overhaul\task-8-9-report.md`
isi: tabel pemetaan patch (fungsi → CAP ID → ada/tidak test), daftar CAP final, test gaps yang ditutup, hasil grep dua arah, commit hash.

Reply ke saya HANYA: status, commit hash, jumlah CAP terdaftar, jumlah test gap yang ditutup, ringkasan test satu baris, concerns.
