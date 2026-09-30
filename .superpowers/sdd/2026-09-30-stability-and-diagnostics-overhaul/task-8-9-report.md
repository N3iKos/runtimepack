# Task 8+9 Execution Report: Capability Registry & Test Gaps Closure

- **Workspace:** `D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save`
- **Branch:** `feature/native-mv-mz-save`
- **Commits:**
  - `b3cfc3c` — `docs(compat): capability registry for all web runtime shims (CAP-WEB-001..016)`
  - `97e9982` — `test(web-runtime): close capability registry test gaps (CAP-WEB-001, 010..014, 016)`

---

## 1. Tabel Pemetaan Patch Awal (Fungsi → CAP ID → Engine → Status Test Awal)

| No | Fungsi / Blok Shim | Baris Aktual | Guard Flag / Pola | Target Engine | Gejala Tanpa Patch | CAP ID | Status Test Awal |
|---|---|---|---|---|---|---|---|
| 1 | `fsShim.readdirSync` | 810 | Native bridge / fallback `data/MapInfos.json` | MV / MZ | `TypeError getRoadRegion`, NPC pathfinding crash (Forestia) | `CAP-WEB-001` | Parsial (bridge diuji, MapInfos fallback hilang) |
| 2 | `patchMzStorage` | 40 | `sm.__runtimepack_mz_save_patched` | MZ | Save/load tidak persist di Android WebView (IndexedDB terhapus) | `CAP-WEB-002` | Ada |
| 3 | `patchMvStorage` | 160 | `sm.__runtimepack_mv_save_patched` | MV | Save/load tidak persist di Android WebView (kuota 5MB terlampaui) | `CAP-WEB-003` | Ada |
| 4 | `patchEngineInput` (Bagian A & B) | 1240 | `TouchInput.moveThreshold === 10`, `__runtimepack_touch_ok_patched` | MV / MZ | Tap 1x hanya menyorot slot, jitter jari dianggap drag/scroll | `CAP-WEB-004` | Ada |
| 5 | `patchEngineInput` (Bagian C & D) | 1277 | `Input.__runtimepack_codemapper_patched`, `!root.RuntimePackInput` | MV / MZ | `KeyboardEvent.keyCode === 0` di Chromium modern; tombol virtual pad mati | `CAP-WEB-005` | Ada |
| 6 | `patchHalfMoveCompat` | 1432 | `root.__runtimepack_halfmove_compat_patched` | MV / MZ | `DataManager.loadGame` reject `TypeError: button.convertEscapeCharacters is not a function` | `CAP-WEB-006` | Ada |
| 7 | `__runtimepackShowLoadError` + `patchLoadDiagnostics` | 1368, 1393 | `__runtimepack_load_diag_patched`, `__runtimepack_loadgame_diag_patched` | MV / MZ | Kegagalan load senyap, freeze atau kembali ke title tanpa jejak logcat | `CAP-WEB-007` | Ada |
| 8 | `patchStorageManager` (`isLocalMode`) | 16 | defineProperty getter | MV / MZ | Game memaksa mode desktop, mencoba path Windows `save/` yang tidak ada di Android | `CAP-WEB-008` | Ada |
| 9 | `normalizePath`, `pathShim`, `nwShim`, `process`, `Buffer`, `cryptoShim`, `greenworksShim`, `root.require` | 581, 660, 685, 708, 730, 853, 873 | Module dictionary lookup | Umum | Game desktop NW.js/Steam crash dengan ReferenceError | `CAP-WEB-009` | Ada |
| 10 | `patchGraphics` (`_clearUpperCanvas`, `_stretchWidth`, `_stretchHeight`) | 1162, 1170, 1190 | `_clearUpperCanvas.__patched`, `__rp_fit_patched` | MZ / MV | Game MZ mobile terpotong border 10% (faktor 0.9); TypeError jika upperCanvas null | `CAP-WEB-010` | Parsial (scaling diuji, null upperCanvas hilang) |
| 11 | `patchAudioManager` (`audioFileExt`) | 1142 | `am.audioFileExt.__patched` | MV / MZ | Ekstensi format audio desktop tidak didukung / hening di WebView | `CAP-WEB-011` | Hilang (Missing) |
| 12 | Global `error` & `unhandledrejection` listeners | 3, 9 | `root.window.addEventListener` | Umum | Error unhandled Promise / uncaught error JavaScript gagal senyap tanpa masuk logcat | `CAP-WEB-012` | Hilang (Missing) |
| 13 | `patchSceneManager` (`catchException`) | 1230 | `sm.catchException.__patched` | MV / MZ | Logcat tidak menangkap stack trace detail saat crash terjadi | `CAP-WEB-013` | Hilang (Missing) |
| 14 | `setupTyranoHooks` | 895 | `__rp_patched` | Tyrano | Stall audio (isNeedClickAudio), missing scenario crash (`cg.ks`), placeholder jump crash (`*未定`) | `CAP-WEB-014` | Parsial (`loadScenario` diuji, storage & jump hilang) |
| 15 | `executeCheatCmd` | 291 | Type validation | MV / MZ | Overlay diagnostik dan runner otomatis tidak dapat mengubah state game | `CAP-WEB-015` | Ada |
| 16 | `pollPatches` | 1499 | Loop interval tick guard | Umum | Modul game yang dimuat asinkron tidak menerima patch jika dieksekusi sekali di awal | `CAP-WEB-016` | Parsial (diuji via manual poll hook, lifecycle hilang) |

---

## 2. Daftar Capability Final (16 CAPs)

1. `CAP-WEB-001` — `fs.readdirSync native bridge and MapInfos fallback`
2. `CAP-WEB-002` — `RPG Maker MZ universal storage adapter`
3. `CAP-WEB-003` — `RPG Maker MV universal webstorage adapter`
4. `CAP-WEB-004` — `Touch input jitter tolerance and single-tap mobile OK`
5. `CAP-WEB-005` — `W3C Keyboard codeMapper and virtual gamepad input bridge`
6. `CAP-WEB-006` — `HalfMove plugin Sprite_Button escape character delegation`
7. `CAP-WEB-007` — `Fail-loud load failure diagnostics`
8. `CAP-WEB-008` — `StorageManager.isLocalMode virtualization`
9. `CAP-WEB-009` — `Desktop shell, Node.js built-ins, and Steamworks mocks`
10. `CAP-WEB-010` — `Graphics canvas clearing and viewport stretch fit`
11. `CAP-WEB-011` — `AudioManager Web Audio format enforcement`
12. `CAP-WEB-012` — `Global unhandled error and rejection listeners`
13. `CAP-WEB-013` — `SceneManager caught exception diagnostics`
14. `CAP-WEB-014` — `TyranoBuilder engine compatibility hooks`
15. `CAP-WEB-015` — `In-game cheat, state dump, and diagnostic command bridge`
16. `CAP-WEB-016` — `Late-binding polling and lifecycle coordinator`

---

## 3. Test Gaps yang Ditutup (Task 9)

Sebanyak 7 test suite baru ditambahkan ke `android/shell/app/src/test/js/web_shims.test.js`:

1. **CAP-WEB-012 (Global Error & Unhandled Rejection Logger):**
   - Menguji pendaftaran listener window `error` dan `unhandledrejection`.
   - Menguji bahwa dispatch event error mencetak tag `[RuntimePack Global Error]` dengan filename, lineno, colno, dan stack trace ke `console.error`.
   - Menguji bahwa dispatch rejection mencetak tag `[RuntimePack Unhandled Rejection]` dengan pesan error ke `console.error`.
2. **CAP-WEB-011 (AudioManager audioFileExt):**
   - Menguji bahwa `AudioManager.audioFileExt()` dipatch menjadi `'.ogg'` saat `Utils.canPlayOgg()` atau `WebAudio.canPlayOgg()` true.
   - Menguji bahwa nilai default tetap `'.ogg'` meskipun pengecekan engine mengembalikan false.
3. **CAP-WEB-013 (SceneManager.catchException):**
   - Menguji pembungkusan `SceneManager.catchException`.
   - Menguji bahwa stack trace error dicatat dengan tag `[RuntimePack Caught Error]`.
   - Menguji bahwa penangan error asli tetap dipanggil dan return value asli dipertahankan.
4. **CAP-WEB-010 (Graphics._clearUpperCanvas safety):**
   - Menguji pemanggilan `Graphics._clearUpperCanvas()` saat `_upperCanvas` bernilai `null` aman tanpa melempar `TypeError`.
   - Menguji pemanggilan saat `_upperCanvas` terpasang meneruskan ke metode asli.
5. **CAP-WEB-001 (readdirSync MapInfos fallback):**
   - Menguji saat bridge native tidak tersedia, `fs.readdirSync("data")` mem-fetch `data/MapInfos.json` via sync XMLHttpRequest.
   - Menguji file standar database (`System.json`, `Actors.json`) dan file peta dinamis (`Map001.json`, `Map005.json`, `Map012.json`) terurai dalam array hasil.
6. **CAP-WEB-014 (TyranoBuilder extended hooks):**
   - Menguji penonaktifan flag desktop `$.isNWJS() === false`, `$.isElectron() === false`, dan `$.isNeedClickAudio() === false`.
   - Menguji pengalihan penyimpanan `$.setStorage` dan `$.getStorage` dengan tipe `'file'` ke `setStorageWeb` dan `getStorageWeb`.
   - Menguji penanganan aman tag jump placeholder (`*未定`, `*todo`) yang tidak memiliki target label agar langsung memanggil `kag.ftag.nextOrder()` tanpa crash, sementara target label valid tetap menjalankan pemrosesan asli.
7. **CAP-WEB-016 (Late-binding polling coordinator):**
   - Menguji pendaftaran timer interval `pollPatches`.
   - Menguji penyuntikan late-binding manajer engine (`StorageManager`, `AudioManager`, `Graphics`, `SceneManager`) yang belum ada saat shim pertama kali dieksekusi, lalu memverifikasi seluruh manajer tersebut otomatis ter-patch saat interval polling berjalan.

---

## 4. Hasil Grep Dua Arah (Bi-directional Mapping Verification)

- **Total CAP IDs di Dokumen Registry (`COMPAT-REGISTRY.md`):** 16
- **Total Marker di Kode Shim (`runtimepack_web_shims.js`):** 16
- **Discrepancy:** 0 (100% matched, bijection)
- **Daftar ID Terverifikasi:**
  `CAP-WEB-001`, `CAP-WEB-002`, `CAP-WEB-003`, `CAP-WEB-004`, `CAP-WEB-005`, `CAP-WEB-006`, `CAP-WEB-007`, `CAP-WEB-008`, `CAP-WEB-009`, `CAP-WEB-010`, `CAP-WEB-011`, `CAP-WEB-012`, `CAP-WEB-013`, `CAP-WEB-014`, `CAP-WEB-015`, `CAP-WEB-016`

---

## 5. Ringkasan Eksekusi Test & Verifikasi

- `node --check android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` → Syntax valid (PASS).
- `node --check android/shell/app/src/test/js/web_shims.test.js` → Syntax valid (PASS).
- `node android/shell/app/src/test/js/web_shims.test.js` → Semua suite unit test PASS:
  - Mobile Web Shims Unit Tests (path, nw.gui, nw, fs, Buffer, process, crypto, Greenworks, StorageManager.isLocalMode) PASSED
  - Cheat commands & Stress test PASSED
  - Asset audit instrumentation PASSED
  - Tyrano scenario stub PASSED
  - RPG Maker MZ screen fit PASSED
  - fs.readdirSync native bridge PASSED
  - RPG Maker MV Universal Save Adapter PASSED
  - RPG Maker MZ Universal Save Adapter PASSED
  - Engine Input Patch (Touch Ok + moveThreshold + codeMapper + RuntimePackInput) PASSED
  - HalfMove Compat tests PASSED
  - Load Failure Diagnostics tests PASSED
  - Global Error & Rejection Logger tests PASSED
  - AudioManager audioFileExt tests PASSED
  - SceneManager.catchException tests PASSED
  - Graphics._clearUpperCanvas safety tests PASSED
  - readdirSync MapInfos fallback tests PASSED
  - TyranoBuilder extended hooks tests PASSED
  - Late-binding polling coordinator tests PASSED
