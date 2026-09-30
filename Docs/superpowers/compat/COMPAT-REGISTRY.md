# RuntimePack Web Runtime — Capability Compatibility Registry

> Sumber kebenaran tunggal untuk shim compat engine web (MV/MZ/Tyrano).
> Setiap entri wajib punya: ID, trigger, symptom, implementation, evidence, regression test, diagnostic.
> Marker di kode: `// [CAP-WEB-xxx]` (grep-able).

---

## CAP-WEB-001 — fs.readdirSync native bridge and MapInfos fallback
- Engine: RPG Maker MV / MZ | Trigger: `fs.readdirSync("data")` atau direktori lain dipanggil plugin navigasi / minimap (mis. GlobalMap Forestia)
- Symptom (tanpa patch): `TypeError getRoadRegion`, NPC pathfinding crash saat game membaca peta atau daftar file data
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`fsShim.readdirSync`) menjembatani ke `RuntimePackNative.readdir` (`WebBackend.kt`) dengan fallback parsing `data/MapInfos.json` menjadi daftar `MapXXX.json`
- Evidence: Forestia NPC pathfinding crash teratasi pada sesi 2026-09-30; navigasi peta normal
- Regression test: `web_shims.test.js` suite `RuntimePackNative.readdir` & suite `readdirSync MapInfos fallback`
- Diagnostic: Warning `[RuntimePack fsShim] syncFetch error` atau array kosong jika bridge dan fallback tidak menemukan file

---

## CAP-WEB-002 — RPG Maker MZ universal storage adapter
- Engine: RPG Maker MZ | Trigger: `StorageManager.saveToForage` / `loadFromForage` saat auto-save, save manual, atau load game
- Symptom (tanpa patch): Save dan load data tidak persist di Android WebView; penyimpanan IndexedDB / localForage terhapus saat app restart atau cache dibersihkan
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`patchMzStorage`) menjembatani saveSlot/loadSlot/slotExists/deleteSlot/listSlots ke `RuntimePackNative` (`NativeSaveRepository.kt`) dengan enkapsulasi binary Base64
- Evidence: Save/load Forestia di MuMu tervalidasi di sesi 2026-09-30; file save tersimpan aman di direktori aplikasi host Android
- Regression test: `web_shims.test.js` suite `RPG Maker MZ Universal Save Adapter`
- Diagnostic: Error `[RuntimePack] Native save failed for slot: <name>` atau `[RuntimePack] Native save bridge unavailable`

---

## CAP-WEB-003 — RPG Maker MV universal webstorage adapter
- Engine: RPG Maker MV | Trigger: `StorageManager.saveToWebStorage` / `loadFromWebStorage` saat save/load/config game
- Symptom (tanpa patch): Save/load tidak persist di Android WebView; batas kuota localStorage (5MB di WebView) terlampaui atau data dihapus oleh OS Android
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`patchMvStorage`) memetakan slot MV (fileN, config, global, fileN.bak) ke `RuntimePackNative` dengan kompresi base64 LZString
- Evidence: AGENTS.md §6.5 & §9 pemisahan save game mutable dari konten statis; slot config dan global teruji
- Regression test: `web_shims.test.js` suite `RPG Maker MV Universal Save Adapter`
- Diagnostic: Mengembalikan `false` atau `null` jika bridge native tidak merespons

---

## CAP-WEB-004 — Touch input jitter tolerance and single-tap mobile OK
- Engine: RPG Maker MV / MZ | Trigger: Interaksi sentuh layar pengguna pada menu save/load (`Window_SavefileList`) atau selectable window (`Window_Selectable`), serta gesture sentuh layar sentuh
- Symptom (tanpa patch): Tap 1x pada slot save/load hanya menyorot item tanpa mengeksekusi load/save (membutuhkan double-tap canggung); getaran jari pada layar sentuh high-DPI dideteksi sebagai drag/scroll karena threshold 10px terlalu sensitif
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`patchEngineInput` bagian A & B) mengatur `TouchInput.moveThreshold = 24` dan menimpa `isTouchOkEnabled` pada `Window_SavefileList` dan `Window_Selectable` agar langsung mengembalikan `this.isOkEnabled()`
- Evidence: Pengujian di MuMu Forestia: tap 1x pada slot save langsung memuat game tanpa double tap; menu game mobile-friendly
- Regression test: `web_shims.test.js` suite `Engine Input Patch (Touch Ok + moveThreshold + codeMapper + RuntimePackInput)`
- Diagnostic: Flag properti `__runtimepack_touch_ok_patched` pada prototipe window dan `TouchInput.moveThreshold === 24`

---

## CAP-WEB-005 — W3C Keyboard codeMapper and virtual gamepad input bridge
- Engine: RPG Maker MV / MZ | Trigger: Input keyboard fisik/virtual dengan `KeyboardEvent.keyCode === 0` di WebView Chromium modern, atau penekanan tombol virtual pad overlay Android
- Symptom (tanpa patch): `KeyboardEvent.keyCode === 0` di Chromium modern menyebabkan event input diabaikan oleh RPG Maker (yang memeriksa keyCode numerik); tombol virtual gamepad mati
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`patchEngineInput` bagian C & D) menyuntikkan `Input.codeMapper` fallback berbasis W3C `event.code` (`KeyZ`, `Enter`, `Escape`, `ArrowUp`, dll.) serta mengekspos API `root.RuntimePackInput` (`press`, `release`, `trigger`, `isAvailable`)
- Evidence: Tombol gamepad virtual pad A/B/D-Pad dan keyboard USB/Bluetooth berfungsi penuh di emulator MuMu
- Regression test: `web_shims.test.js` suite `Engine Input Patch (Touch Ok + moveThreshold + codeMapper + RuntimePackInput)`
- Diagnostic: Flag `Input.__runtimepack_codemapper_patched` dan pemeriksaan status `root.RuntimePackInput.isAvailable()`

---

## CAP-WEB-006 — HalfMove plugin Sprite_Button escape character delegation
- Engine: RPG Maker MV / MZ | Trigger: `DataManager.loadGame` saat plugin HalfMove aktif dan scene me-restore button sprite
- Symptom (tanpa patch): `DataManager.loadGame` me-reject Promise dengan `TypeError: button.convertEscapeCharacters is not a function`, memicu kegagalan load game secara senyap (bug Forestia)
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`patchHalfMoveCompat`) menambahkan `Sprite_Button.prototype.convertEscapeCharacters` yang mendelegasikan pemrosesan teks ke `scene._helpWindow`, anak `_windowLayer`, atau dummy prototype `Window_Base`
- Evidence: Stack trace bug Forestia 2026-09-30 terselesaikan dan lolos verifikasi load game
- Regression test: `web_shims.test.js` suite `HalfMove Compat tests`
- Diagnostic: Warning `[RuntimePack] Sprite_Button.convertEscapeCharacters delegation error:` jika evaluasi delegasi teks gagal

---

## CAP-WEB-007 — Fail-loud load failure diagnostics
- Engine: RPG Maker MV / MZ | Trigger: Kegagalan deserialisasi data save, save corrupt, atau unhandled rejection saat proses pemuatan save file
- Symptom (tanpa patch): Kegagalan pemuatan save file terjadi secara senyap (silent failure), game kembali ke title screen atau freeze tanpa feedback error di logcat maupun layar
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`patchLoadDiagnostics` & `__runtimepackShowLoadError`) mengaitkan logging `[RuntimePack] RT-LOAD-FAIL` pada `Scene_Load.prototype.onLoadFailure` dan `DataManager.loadGame`, serta menampilkan overlay peringatan merah on-screen
- Evidence: Sesi 2026-09-30 Task 5: simulasi save corrupt langsung menampilkan pesan diagnostik visual dan logcat terstruktur
- Regression test: `web_shims.test.js` suite `Load Failure Diagnostics (fail-loud RT-LOAD-FAIL)`
- Diagnostic: Kode log domain AGENTS.md §18 `[RuntimePack] RT-LOAD-FAIL <detail>` dan overlay banner on-screen visual

---

## CAP-WEB-008 — StorageManager.isLocalMode virtualization
- Engine: RPG Maker MV / MZ | Trigger: Pengecekan runtime mode pada `StorageManager.isLocalMode()` saat inisialisasi penyimpanan
- Symptom (tanpa patch): Engine mendeteksi keberadaan `process` / `require` lalu mengaktifkan mode lokal desktop (Node/NW.js), mencoba mengakses path filesystem desktop Windows yang tidak ada di Android WebView
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`patchStorageManager`) mengunci `StorageManager.isLocalMode` via getter `Object.defineProperty` agar selalu bernilai `false`
- Evidence: Memaksa engine mengeksekusi branch browser storage (Forage / WebStorage) yang telah dijembatani ke native repository
- Regression test: `web_shims.test.js` suite `StorageManager.isLocalMode` (lines 154-178)
- Diagnostic: Pemeriksaan `StorageManager.isLocalMode()` menghasilkan `false`

---

## CAP-WEB-009 — Desktop shell, Node.js built-ins, and Steamworks mocks
- Engine: RPG Maker MV / MZ / Umum | Trigger: Game memanggil API Node.js (`path`, `fs`, `process`, `Buffer`, `crypto`), NW.js (`nw.gui`, `nw`), atau Steamworks (`greenworks`, `Greenworks`)
- Symptom (tanpa patch): Game crash saat startup atau gameplay dengan `ReferenceError: require is not defined`, `process is not defined`, atau kegagalan pemanggilan `nw.Window.get()`
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`normalizePath`, `pathShim`, `nwShim`, `process` mock, `BufferMock`, `cryptoShim`, `fsShim`, `greenworksShim`, `root.require`)
- Evidence: Game rilisan platform Steam dan game bertarget NW.js dapat boot dan berjalan tanpa error modul desktop
- Regression test: `web_shims.test.js` suite `require('path')`, `require('nw.gui')`, `require('fs')`, `process`, `Buffer`, `require('crypto')`, `Greenworks`
- Diagnostic: Warning `[RuntimePack] Unhandled require module: <mod>` untuk modul native pihak ketiga yang tidak dikenali

---

## CAP-WEB-010 — Graphics canvas clearing and viewport stretch fit
- Engine: RPG Maker MZ / MV | Trigger: Rendering canvas tampilan dan kalkulasi scaling viewport pada layar Android
- Symptom (tanpa patch): Pada RPG Maker MZ mobile, ukuran canvas terkena penalti penskalaan 0.9 sehingga menyisakan border hitam 10%; pemanggilan `_clearUpperCanvas` sebelum kanvas terpasang memicu crash `TypeError`
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`patchGraphics`) memaksa `Graphics._stretchWidth` dan `_stretchHeight` menggunakan 100% dimensi window (`window.innerWidth`/`innerHeight`), serta menambahkan guard `if (!this._upperCanvas) return;` pada `_clearUpperCanvas`
- Evidence: Kanvas game di MuMu mengisi layar secara optimal edge-to-edge dengan rasio scaling 100%
- Regression test: `web_shims.test.js` suite `RPG Maker MZ screen fit` & suite `Graphics._clearUpperCanvas safety`
- Diagnostic: Flag `__rp_fit_patched` pada `Graphics._stretchWidth` dan `Graphics._stretchHeight`

---

## CAP-WEB-011 — AudioManager Web Audio format enforcement
- Engine: RPG Maker MV / MZ | Trigger: Evaluasi format ekstensi file suara oleh `AudioManager.audioFileExt()` saat memutar BGM, BGS, ME, atau SE
- Symptom (tanpa patch): Ekstensi format audio salah untuk WebView (mis. mencoba memutar `.m4a` atau ekstensi desktop) yang tidak didukung atau decoding-nya gagal di Android WebView
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`patchAudioManager`) menimpa `AudioManager.audioFileExt` agar mengembalikan `'.ogg'` yang didukung native oleh browser Android
- Evidence: File BGM dan SE berformat `.ogg` terputar stabil di WebView Android tanpa error audio stream
- Regression test: `web_shims.test.js` suite `AudioManager audioFileExt`
- Diagnostic: Flag properti `am.audioFileExt.__patched === true`

---

## CAP-WEB-012 — Global unhandled error and rejection listeners
- Engine: Umum (MV / MZ / Tyrano) | Trigger: Uncaught error pada skrip JavaScript atau unhandled Promise rejection di WebView
- Symptom (tanpa patch): Error fatal JavaScript di dalam callback atau promise gagal secara senyap tanpa muncul di logcat Android, menyulitkan debugging
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` mendaftarkan event listener `window.addEventListener("error")` dan `window.addEventListener("unhandledrejection")` yang mencetak log terstruktur ke `console.error`
- Evidence: Android logcat menangkap log error runtime game dengan nomor baris, file, dan stack trace
- Regression test: `web_shims.test.js` suite `Global Error & Unhandled Rejection Logger`
- Diagnostic: Prefix logcat `[RuntimePack Global Error]` dan `[RuntimePack Unhandled Rejection]`

---

## CAP-WEB-013 — SceneManager caught exception diagnostics
- Engine: RPG Maker MV / MZ | Trigger: Error logika scene yang ditangkap oleh penangan error bawaan `SceneManager.catchException`
- Symptom (tanpa patch): Exception hanya menampilkan prompt grafis sederhana tanpa mencatat rincian stack trace ke logcat Android
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`patchSceneManager`) membungkus `SceneManager.catchException` untuk mencatat stack trace via `console.error` dengan tag terstruktur
- Evidence: Stack trace bug runtime tertangkap jelas di logcat sebelum error handler engine menampilkan layar crash
- Regression test: `web_shims.test.js` suite `SceneManager.catchException Logger`
- Diagnostic: Prefix logcat `[RuntimePack Caught Error] <stack>`

---

## CAP-WEB-014 — TyranoBuilder engine compatibility hooks
- Engine: TyranoBuilder / TyranoScript | Trigger: Inisialisasi engine Tyrano, pemuatan skenario melalui `kag.loadScenario`, pemutaran audio, dan penyimpanan save
- Symptom (tanpa patch): Game Tyrano menunggu klik sebelum memutar audio (`isNeedClickAudio`), mencoba memakai filesystem desktop (`isNWJS`/`isElectron`), crash saat skenario opsional tidak ditemukan (`cg.ks`, `extra.ks`, dll.), atau crash saat jump ke placeholder (`*未定`, `*tbd`, dll.)
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`setupTyranoHooks`) menonaktifkan flag desktop, mengalihkan storage ke web storage, mengatur `ready_audio = true`, membuat stub skenario fallback otomatis, dan melewati tag jump placeholder
- Evidence: Game TyranoScript dapat melakukan boot dan transisi skenario tanpa stall audio atau crash file skenario opsional
- Regression test: `web_shims.test.js` suite lines 505-529 & suite `TyranoBuilder extended hooks`
- Diagnostic: Warning `[RuntimePack] Using safety stub for optional/missing scenario:` dan `[RuntimePack] Safe handling for missing placeholder jump target:`

---

## CAP-WEB-015 — In-game cheat, state dump, and diagnostic command bridge
- Engine: RPG Maker MV / MZ | Trigger: Pemanggilan `window.executeCheatCmd` dari diagnostic overlay WebView atau automated probe runner
- Symptom (tanpa patch): Overlay diagnostik dan runner otomatis tidak dapat mengubah variabel/switch/gold, memicu teleport, atau menjalankan stress test audit aset
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`executeCheatCmd`) mengeksekusi perintah in-game (`set_variable`, `set_switch`, `gain_gold`, `recover_all`, `gain_item`, `eval`, `dump_state`, `teleport_map`, `export_bug_report`, `run_stress_test`) dan melaporkan hasil ke `RuntimePackWeb`
- Evidence: Pengujian diagnostic suite dan stress testing di worktree berjalan lancar; dump_state melaporkan variabel dan gold secara akurat
- Regression test: `web_shims.test.js` suite `executeCheatCmd` (lines 203-356)
- Diagnostic: Kode error `[CHT-002]` untuk eval failure dan laporan audit di `root.__runtimepack_audit_report`

---

## CAP-WEB-016 — Late-binding polling and lifecycle coordinator
- Engine: Umum (MV / MZ / Tyrano) | Trigger: Pemuatan skrip engine secara asinkron atau urutan tag HTML yang bervariasi antar-game
- Symptom (tanpa patch): Shim dieksekusi sebelum modul game selesai didefinisikan, mengakibatkan patch tidak terpasang jika dieksekusi sekali di awal
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`pollPatches`) menjalankan polling berkala (10ms) untuk memeriksa dan mengaplikasikan patch segera saat modul engine tersedia, serta berhenti otomatis setelah stabil
- Evidence: Mendukung game RPG Maker MV/MZ baik yang menggunakan script tag standar maupun modular/asynchronous loading
- Regression test: `web_shims.test.js` suite `pollPatches lifecycle`
- Diagnostic: Terpasangnya penanda `__runtimepack_*_patched` pada objek engine global
