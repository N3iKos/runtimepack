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
- Engine: Umum (MV / MZ / Tyrano / VNM) | Trigger: Pemuatan skrip engine secara asinkron atau urutan tag HTML yang bervariasi antar-game
- Symptom (tanpa patch): Shim dieksekusi sebelum modul game selesai didefinisikan, mengakibatkan patch tidak terpasang jika dieksekusi sekali di awal
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`pollPatches`) menjalankan polling berkala (10ms) untuk memeriksa dan mengaplikasikan patch segera saat modul engine tersedia, serta berhenti otomatis setelah stabil
- Evidence: Mendukung game RPG Maker MV/MZ dan Visual Novel Maker baik yang menggunakan script tag standar maupun modular/asynchronous loading
- Regression test: `web_shims.test.js` suite `pollPatches lifecycle`
- Diagnostic: Terpasangnya penanda `__runtimepack_*_patched` pada objek engine global

---

## CAP-WEB-017 — Binary Buffer polyfill for bundled libraries
- Engine: Umum / VNM | Trigger: Pemanggilan `new Buffer(data, encoding)`, `Buffer.from`, `Buffer.alloc`, `Buffer.isBuffer`, atau `slice` oleh pustaka bundel seperti JSZip
- Symptom (tanpa patch): `TypeError: Buffer is not a constructor` atau kegagalan saat JSZip memproduksi payload bertipe `nodebuffer`
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`Buffer`) mengimplementasikan callable constructor berbasis `Uint8Array`, penanganan UTF-8 multibyte via TextEncoder/TextDecoder, enkoding base64/hex/binary/ascii, dan metode `slice`/`toString`/`copy`/`concat`
- Evidence: S31; pengujian roundtrip JSZip `nodebuffer` dengan ekstraksi arsip dan byte equality
- Regression test: `android/shell/app/src/test/js/jszip_buffer_regression.test.js`
- Diagnostic: Flag `_isBuffer === true` dan `Buffer.isBuffer(buf) === true`

---

## CAP-WEB-018 — NW.js window adapter
- Engine: VNM / Umum | Trigger: Pemanggilan `nw.Window.get()` untuk `setResizable`, `enterFullscreen`, `leaveFullscreen`, `isFullscreen`, dan registrasi event `on("resize")`
- Symptom (tanpa patch): `TypeError: r.setResizable is not a function`, inkonsistensi state layar penuh, atau hilangnya event resize
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`nwShim.Window.get`) mengembalikan instance singleton persisten dengan pelacakan state `isFullscreen`/`isFullScreen`, `setResizable`, dan event emitter `on`/`emit`/`removeListener`
- Evidence: S32; verifikasi panggilan `data/ENGINE.js` terhadap `nw.Window.get()`
- Regression test: `android/shell/app/src/test/js/vnm_window_adapter.test.js`
- Diagnostic: Pelaporan state konsisten pada `nw.Window.get().isFullscreen`

---

## CAP-WEB-019 — Visual Novel Maker durable gs.GameStorage native bridge adapter
- Engine: Visual Novel Maker | Trigger: Pemuatan dan penyimpanan data permainan (`setData`, `getData`, `setObject`, `getObject`, `exists`, `remove`, `clear`) pada `gs.GameStorage`
- Symptom (tanpa patch): Data save diarahkan ke filesystem desktop melalui `require("fs")` no-op atau `localStorage` WebView yang terhapus saat proses restart
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`patchVnmStorage`) mengalihkan seluruh slot save (slots 1..n, headers, settings, globalData) ke bridge `RuntimePackNative` (`saveSlot`, `loadSlot`, `slotExists`, `deleteSlot`, `listSlots`) dengan namespace game UID `$PARAMS.uid` dan penanganan fail-loud `RT-SAVE-FAIL`
- Evidence: S29, S32; simulasi multi-session VM pada save repository dengan byte fidelity
- Regression test: `android/shell/app/src/test/js/vnm_save_storage.test.js` & `NativeSaveRepositoryTest.kt`
- Diagnostic: Error `[RuntimePack] RT-SAVE-FAIL: native saveSlot failed for slot: <name>`

---

## CAP-WEB-020 — Guarded voiceDuration delegation for Component_MessageBehavior
- Engine: Visual Novel Maker | Trigger: Kalkulasi durasi pesan suara pada `vn.Component_MessageBehavior.prototype.voiceDuration`
- Symptom (tanpa patch): Saat dialog suara selesai diputar dan referensi suara di-dispose, pemanggilan `voiceDuration` memicu `TypeError: Cannot read properties of null (reading 'buffer')` atau crash saat skip
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`patchVnmMessageCompat`) membungkus `voiceDuration`: mengevaluasi `tempSettings.skip` segera (mengembalikan 1 frame sebelum akses null), mengembalikan timing degraded 0 dengan one-shot diagnostic jika voice gate aktif namun source/buffer null, mendelegasikan pemanggilan orisinal pada kondisi normal, dan memetakan hasil non-finite ke 0
- Evidence: S34; verifikasi baris 11501 `data/SCRIPTS.js`
- Regression test: `android/shell/app/src/test/js/vnm_startup_lifecycle.test.js`
- Diagnostic: Peringatan one-shot `[CAP-WEB-020] voice source or buffer absent during voice duration calculation, returning degraded timing 0`

---

## CAP-WEB-021 — Synchronous two-boundary served HTML startup transform
- Engine: Visual Novel Maker | Trigger: Penyajian berkas awal `index.html` dan bootstrap script permainan
- Symptom (tanpa patch): Pemuatan skrip asynchronously atau injeksi shim yang terlambat menyebabkan crash sebelum library atau shim tersedia
- Implementation: `VfsResourceInterceptor.kt` (`transformVnmHtml`) menyisipkan Boundary 1 (tag script `runtimepack_web_shims.js`) secara sinkron sebelum library game/ENGINE dieksekusi, dan Boundary 2 (hook `__runtimepackPollVnmMessageCompat()`) tepat setelah tag blocking `data/SCRIPTS.js` sebelum DOM-ready; memvalidasi ketiadaan atribut `async`/`defer` pada script krusial dan memeriksa duplikasi marker
- Evidence: S29, S35; inspeksi urutan script `index.html`
- Regression test: `VfsResourceInterceptorTest.kt` & `android/shell/app/src/test/js/vnm_startup_lifecycle.test.js`
- Diagnostic: `IllegalArgumentException` terstruktur jika anchor script hilang atau terdapat tag `async`/`defer` ilegal

---

## CAP-WEB-022 — Web Audio lifecycle and WebIDL onended setter ownership
- Engine: Visual Novel Maker | Trigger: Siklus hidup pemutaran dan penghentian suara (`gs.AudioBuffer`, `gs.AudioBufferReference`, `gs.AudioBufferStream`), serta penugasan handler `onended`
- Symptom (tanpa patch): Pemanggilan `stop()` sebelum `start()` melempar `InvalidStateError`; callback `onended` lama yang dieksekusi setelah referensi di-dispose mengakses properti `null` atau membangkitkan loop lama yang mati; referensi pending (`audioBuffer === null`) melempar `TypeError: Cannot read properties of null (reading 'error')` saat `Audio.pause()` (kolom 49411) atau `stopAllSounds` (kolom 52688)
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`patchVnmAudio`) membungkus `gs.Audio.addAudioBuffer` dan meregistrasi per-instance non-enumerable descriptor (`isPlaying`, `volume`, `playbackRate`) pada instance VNM yang dikenali tanpa mengubah descriptor prototype non-configurable; mengembalikan false/inactive pada pending null buffer; menyimpan setter pending di WeakMap sidecar untuk diaplikasikan setelah inisialisasi; membungkus `stop`, `play`, `resume`, `initialize`, dan `dispose` dengan validasi owner/invalidation, membatasi `stop()` pada unstarted native node, serta menjaga delegasi orisinal pada valid state
- Evidence: S33; verifikasi siklus hidup `gs.AudioBuffer`, `gs.AudioBufferReference`, dan `gs.AudioBufferStream` di `data/ENGINE.js` (hash `928372395b6e82020d693653468aa1a02e1326a924dcfac7b7c6d0739b107885`)
- Regression test: `android/shell/app/src/test/js/vnm_audio_lifecycle.test.js`
- Diagnostic: Peringatan one-shot `[CAP-WEB-022]` terstruktur

---

## CAP-WEB-023 — Virtual Pad exclusive touch stream and focus lifecycle
- Engine: Umum / Android Host | Trigger: Interaksi sentuhan virtual overlay (`vpad.js`), pergeseran hitbox kontrol, pembukaan modal, dan transisi aplikasi
- Symptom (tanpa patch): Sentuhan pada kontrol game overlay tercuri oleh canvas game di bawahnya; perubahan koordinat hitbox di tengah gesture memutus stream sentuhan; sentuhan eye toggle / toolbar mencuri fokus game; deferred focus runnable dari modal lama mencuri fokus setelah modal baru dibuka; key stuck saat transisi
- Implementation: `WebActivity.kt` menginisialisasi `overlayWebView` dengan `isFocusable = false` dan `isFocusableInTouchMode = false` secara default; request focus pada overlay hanya diberikan saat input teks aktif di UI thread; pelepasan kunci virtual (`releaseActiveKeys`) saat modal dibuka atau eye toggle diaktifkan tanpa injeksi Space (key 32); pengembalian fokus game yang aman via `requestGameFocusSafe` dengan pelacakan generasi monotonik `focusGeneration`; gesture ownership tetap eksklusif sepanjang `MOVE`/`UP`/`CANCEL`
- Evidence: S24; pengujian event gesture dan fokus WebView
- Regression test: `WebActivityTest.kt` & `android/shell/app/src/test/js/vnm_overlay_focus_lifecycle.test.js`
- Diagnostic: Pelacakan generasi fokus monotonik dan flag `isTouchCapturedByGame`

---

## CAP-WEB-024 — Visual Novel Maker touchcancel safety and passive direct touch dispatch
- Engine: Visual Novel Maker / Android Host | Trigger: Pembatalan gesture sentuh (`MotionEvent.ACTION_CANCEL`), interupsi modal/toolbar, atau penggunaan profil build `--overlay-mode passive-watermark`
- Symptom (tanpa patch): Pada VNM `ENGINE.js`, ketiadaan listener `touchcancel` menyebabkan `Mouse.buttons[1]` tersangkut pada nilai 1 permanen jika gesture dibatalkan sebelum `touchend`, mengunci status klik mouse game dan menggagalkan tap berikutnya; pada mode overlay penuh, IPC feedback tanpa batas frekuensi membebani rendering thread
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`installVnmTouchCancelSafety`) mengaitkan listener `touchcancel` idempoten pada window capture phase untuk mereset `Input.Mouse` (`clear()`, `buttonDown = false`, `buttonUp = false`, `buttons` array di-nol-kan) tanpa manipulasi paksa focus/blur atau keyup; `WebActivity.kt` mendukung `overlayMode: "passive-watermark"` dengan direct return `dispatchTouchEventToGame(event)` melewati resolusi hitbox/feedback; mode `full` menerapkan latest-sample coalescing pada event `MOVE` (<= 60 dispatches/detik) dengan preservasi event terminal `DOWN`/`UP`/`CANCEL`
- Evidence: S24, S29; pembongkaran bytecode DEX APK referensi `TFPC-TLFMB` (`isPassiveOverlay`) dan verifikasi `Input.Mouse` di `data/ENGINE.js`
- Regression test: `WebActivityTest.kt` (`testPassiveOverlayDirectTouchDispatch`, `testFullModeBoundedMoveFeedbackCoalescing`, `testHistoricalManifestMissingOverlayModeDefaultsToFull`), `android/shell/app/src/test/js/vnm_overlay_focus_lifecycle.test.js` (suite VNM TouchCancel Safety), `tests/integration/build_test.rs` (`test_cli_overlay_mode_defaults_to_full_and_validates_flags`, `test_vnm_beta_passive_emits_minimal_overlay_and_deterministic_hash`)
- Diagnostic: Flag `isPassiveOverlay` pada `WebActivity`, `__runtimepack_vnm_touchcancel_installed` pada `window`, dan `overlay_mode` pada `build-manifest.json`

---

## CAP-WEB-025 — Visual Novel Maker AudioManager in-place null/hole reference sanitization
- Engine: Visual Novel Maker / Web | Trigger: Pemutaran atau penghentian audio (`playSound`, `playVoice`, `stopSound`, `stopAllVoices`, `stopAllSounds`) pasca-muat save data dengan entri `soundReferences` null/undefined/hole
- Symptom (tanpa patch): Pada VNM `SCRIPTS.js` baris 3811, iterasi `soundReferences[name]` langsung mengevaluasi `!r.isPlaying`; jika array memuat elemen `null` hasil deserialisasi save lama, terjadi crash seketika `TypeError: Cannot read properties of null (reading 'isPlaying')`
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` menyematkan wrapper idempoten pada `gs.AudioManager.prototype` dan singleton `window.AudioManager` yang secara in-place membersihkan HANYA elemen `null`/`undefined`/holes sebelum mendelegasikan panggilan asli tanpa menulis ulang method, mempertahankan identitas array, pending reference `audioBuffer=null`, volume/rate, dan membiarkan native error asli tetap terlempar
- Evidence: S24, S30; rekonstruksi crash Chapter 5 dan audit kode sumber `data/SCRIPTS.js`
- Regression test: `android/shell/app/src/test/js/vnm_audio_lifecycle.test.js` (suite Task 2A)
- Diagnostic: Peringatan one-shot `[CAP-WEB-025]` terstruktur dengan log diagnostic error `WEB-025`

---

## CAP-WEB-026 — Visual Novel Maker legacy read-only Base64 save fallback with non-destructive preservation
- Engine: Visual Novel Maker / Android Host | Trigger: Pembacaan save data legacy format Base64 UTF-8 dari namespace `vnm` ketika record UID kontemporer belum ada
- Symptom (tanpa patch): Save lama tersimpan dalam namespace `vnm` dengan key `encodeURIComponent(uid + "_" + slotKey)` berformat Base64 UTF-8; runtime kontemporer hanya memeriksa namespace `uid` plain JSON sehingga progres lama hilang atau tidak terdeteksi
- Implementation: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (`patchVnmStorage`) mengimplementasikan fallback baca-saja berurutan: record kontemporer (termasuk record kosong/korup) mendominasi mutlak tanpa fallback; fallback ke `vnm` hanya dieksekusi jika record kontemporer absen, dengan validasi Base64 ketat dan decode UTF-8; tidak ada mutasi tulis, cache setItem, atau penghapusan ke namespace legacy; `NativeSaveRepository.kt` memisahkan path baca (`createDir = false`) agar pencarian slot tidak menghasilkan folder kosong
- Evidence: S24, S30; pembongkaran bytecode DEX APK referensi `TFPC-TLFMB` (`NativeSaveRepository`) dan dump asset `runtimepack_web_shims.js`
- Regression test: `android/shell/app/src/test/js/vnm_save_storage.test.js` (suite Task 2B), `android/shell/app/src/test/java/io/runtimepack/host/NativeSaveRepositoryTest.kt` (`testGetAndContainsDoNotCreateDirectoriesOnMissingLookup`, `testVnmLegacySlotNameReadingWithoutRegisterNamespace`)
- Diagnostic: Log `[CAP-WEB-026]` jika payload Base64 corrupt, error code `WEB-026`

---

# Runtime Hardening & Asset Lockdown Specifications

> Spesifikasi proteksi aset dan hardening runtime untuk mencegah pencurian tata letak, modifikasi tidak sah (repack), dan inspeksi aset plaintext pada rilis produksi.

## 1. Mekanisme Proteksi Berjenjang (B + C + D)
- **Containerization (B)**: Pada build `--flavor release`, file `overlay/*`, `web_shims/*`, dan `fonts/*` dikemas ke dalam biner terenkripsi `game.pack` (GPACK) dengan algoritma AEAD ChaCha20-Poly1305. File-file tersebut **100% dihilangkan** dari direktori `assets/` APK. Host Android (`OverlayAssetSource`) memuat aset runtime langsung dari stream GPACK terdekripsi dalam memori.
- **Startup Integrity Verification (C)**: Generator build menghitung hash SHA-256 deterministik dari seluruh aset runtime dan menyematkannya ke dalam kode sumber Kotlin terkompilasi (`RuntimeBuildInfo.kt`). Pada `onCreate`, `RuntimeIntegrity` memvalidasi payload sebelum WebView dimuat. Jika terdapat modifikasi atau penghapusan aset, aplikasi menampilkan layar `APPLICATION CORRUPTED` dan keluar secara rapi.
- **DEX Hardening / R8 (D)**: Mengaktifkan R8 minification (`isMinifyEnabled = true`) dengan aturan `-keep` komprehensif pada `@android.webkit.JavascriptInterface`, Activity, dan metode native JNI.

## 2. Definisi Hash Deterministik
Hash dihitung dari daftar entri runtime yang diurutkan secara alfabetis (case-insensitive lowercase):
- Setiap entri: `SHA256.update(canonical_path) -> update(0x00) -> update(file_bytes) -> update(0x00)`
- Digest: String heksadesimal lowercase 64 karakter.

## 3. Taksonomi Kode Diagnostik
- `AND-401`: Runtime asset integrity mismatch (hash aktual tidak sesuai hash build).
- `AND-402`: Runtime overlay assets missing from container (container kosong pada mode release).
- `AND-403`: Expected integrity hash missing in release build / GPACK unreadable.

## 4. Batas Keamanan Nyata (Threat Model)
Hardening ini melindungi dari 99% upaya ekstraksi statis (unzip APK, penyalinan layout, pengubahan watermark atau cheat profile oleh pihak ketiga). Pada perangkat yang di-root penuh dengan instrumentasi dinamis tingkat lanjut (Frida/Xposed) atau dump memori WebView runtime, aset pada akhirnya berada dalam memori dalam bentuk terdekripsi. Sesuai AGENTS.md §17, sistem ini dirancang sebagai *asset hardening & encrypted content protection*, bukan klaim proteksi mutlak yang mustahil ditembus.

