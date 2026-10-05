# Technical Findings

## [2026-10-05] Visual Novel Maker Candidate APK Build & Artifact Hardening (Gate G Task 3 & Gate G1)

### Finding 11: Gate G1 Post-Watermark Asset Integrity Digest Alignment
- Pada `--flavor beta`, pipeline generator `AndroidProjectGenerator::emit_project_with_themes` kini mengeksekusi perhitungan integritas setelah seluruh mutasi aset selesai (`inject_cheat_profile`, `configure_overlay_watermark`, penambahan tema, shims, dan font ke `assets/`).
- Hal ini menyelaraskan konstanta biner `RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256` dengan digest kalkulasi runtime Kotlin `RuntimeIntegrity.computeHash` pada nilai `5812be9a3e656d633333667bf05593fb8085361e8a7bbdfcbc8693327431035d` (`EQUAL: True`).
- Aplikasi tidak lagi bergantung pada toleransi exception beta di runtime Android; verifikasi integritas valid secara kriptografis.

### Finding 9: Gradle Cold Daemon vs Clean Cycle on Windows

- Pemanggilan pertama Gradle subprocess melalui CLI dapat mengembalikan non-zero exit code jika Gradle daemon sedang melakukan cold spawn atau terhambat file locks sementara saat task `clean`.
- Begitu daemon tersinkronisasi, pemanggilan `assembleDebug` berjalan deterministik (36s, 38 actionable tasks executed) dan menghasilkan APK valid tanpa memerlukan modifikasi kode atau bypass build.

### Finding 10: VNM Candidate APK Characteristics (Flavor Beta)
- GPACK container `game.pack` (1,385,946,515 bytes) memiliki index dengan 100% `encryption=0`, sesuai spesifikasi `--flavor beta` di mana enkripsi ChaCha20-Poly1305 dinonaktifkan untuk memfasilitasi audit transparan.
- File web shims `assets/web_shims/runtimepack_web_shims.js` di dalam APK terbukti 100% byte-exact terhadap source template (`247aed00...`).
- Bytecode DEX (`classes3.dex`) memuat class `BoundedEntryStream`, `GpackStreamReader`, `GpackExtractor`, dan `RuntimeIntegrity`.
- Baseline raw game source berisi 1,976 file (1,433.87 MB) vs referensi lama 2,279 file; user mengonfirmasi versi game adalah versi yang SAMA (bukan perbedaan versi resmi), dan selisih 303 file merupakan unknown inventory difference tanpa klaim byte parity.
- Sesuai pembagian peran, status verifikasi fungsional dan gameplay kandidat ini adalah `NOT_TESTED`.


## [2026-09-30] Asset Hardening & Encrypted Overlay Architecture (B + C + D)

### Finding 7: Overlay & Web Shims Serving via GPACK In-Memory Decryption
- Di mode release, file `overlay/*`, `web_shims/*`, dan `fonts/*` dikemas ke dalam container biner `game.pack` (GPACK) dengan algoritma ChaCha20-Poly1305.
- Host Android menggunakan `OverlayAssetSource` untuk menyajikan byte aset langsung dari stream terdekripsi ke WebView via `shouldInterceptRequest` (`VfsResourceInterceptor.kt` dan `WebActivity.kt`).
- Hal ini menghilangkan 100% file HTML, CSS, JS, dan font dari folder `assets/` APK rilis, sehingga alat ekstraksi file ZIP tidak dapat menemukan kode tata letak maupun shim.

### Finding 8: Startup Integrity Verification & R8 Keep Rules
- SHA-256 deterministik dihitung dari pasangan `(canonical_path, bytes)` terurut secara alfabetis dengan pemisah null byte `0x00`.
- Hash disematkan ke dalam objek Kotlin terkompilasi `RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256` di dalam bytecode DEX, bukan file JSON plaintext di assets.
- R8 minification (`isMinifyEnabled = true`) diaktifkan untuk build release. Keep rules komprehensif di `proguard-rules.pro` memastikan seluruh method `@JavascriptInterface` pada `WebBridge`, `RuntimePackNative`, `RuntimePackVPad`, dan `RuntimePackAssetAudit` tidak dihapus atau di-obfuscate oleh R8 compiler.
- Jika integritas hash tidak cocok atau aset dihapus, `RuntimeIntegrity` menampilkan layar `APPLICATION CORRUPTED` (`AND-401` / `AND-402`) dan memanggil `finishAffinity()` serta `exitProcess(1)` secara deterministik.
- Resolusi layout `activity_corrupted` dilakukan secara dinamis via `resources.getIdentifier` dengan fallback layout programmatic murni, mencegah crash `NoClassDefFoundError` atau R package mismatch pada standalone game packages yang dihasilkan.

## [2026-09-30] RPG Maker MZ di Android WebView: 2 akar masalah "load game macet" (Forestia 1.4.4)

### Finding 1: Window_Selectable double-touch trap (rmmz_windows.js)
`Window_Selectable.prototype.isTouchOkEnabled` hanya mengembalikan true jika `isOkEnabled() && (_cursorFixed || _cursorAll || _doubleTouch)`.
- **PC/NW.js**: mouse hover memicu `onTouchSelect(false)` tiap frame → `hitIndex === index()` → `_doubleTouch=true` SEBELUM klik → 1 klik langsung `processOk()`.
- **Android WebView**: tidak ada hover → single tap hanya `select()` (highlight ungu), `_doubleTouch` tetap false → `onTouchOk()` batal → `Scene_Load.executeLoad()` TIDAK PERNAH dipanggil (gejala: diam, tanpa error, tanpa buzzer).
- **Aggravator**: `TouchInput.moveThreshold = 10` (rmmz_core.js) — jitter jari layar DPI tinggi > 10px → `_moved=true` → `_clicked=false` → dianggap scroll. Double-tap pun sering gagal.
- **Fix**: override `Window_SavefileList`/`Window_Selectable.isTouchOkEnabled` → `return this.isOkEnabled()` (guard: hanya non-NWjs untuk Window_Selectable global) + `moveThreshold = 24`. Terpasang di `runtimepack_web_shims.js` (CAP-WEB registry).

### Finding 2: KeyboardEvent keyCode selalu 0 di Chromium modern (110+, Android WebView)
`new KeyboardEvent('keydown', {keyCode: 90, which: 90})` — Blink MEMBUANG `keyCode`/`which` dari KeyboardEventInit (deprecated W3C) → `evt.keyCode === 0` → `Input.keyMapper[0]` undefined → tombol virtual gamepad A/Z/Enter mati total di semua UI game.
- **Fix (WebActivity.kt dispatchKeyToGame)**: (a) `Object.defineProperty(evt, 'keyCode'/'which'/'code', {get: ...})` agar properti bertahan; (b) panggil `Input._onKeyDown(evt)` langsung dengan event yang sudah dipatch; (c) safety net via `window.RuntimePackInput.press/release(action)` (shim JS) menggunakan `Input.keyMapper[jsKeyCode]` → fallback `Input.codeMapper[evt.code]`.

### Finding 3: ROOT CAUSE FINAL load macet: plugin HalfMove + touchUI
- `DataManager.loadGame` REJECT: `windowLayer.children[0].convertEscapeCharacters is not a function` → `onLoadFailure()` senyap (buzzer tak terdengar karena AudioContext suspended → terlihat seperti game diam).
- Plugin `HalfMove.js` memanggil `convertEscapeCharacters` pada `SceneManager._scene._windowLayer.children[0]` saat ekstraksi save di `Scene_Load`. Di WebView `touchUI` default `true` → `_cancelButton` (`Sprite_Button`) menempati `children[0]` → method tidak ada.
- **Kenapa PC/JoiPlay aman**: `save/config.rmmzsave` bawaan PC berisi `touchUI:false` → tidak ada `_cancelButton` → `children[0]` = `Window_Help`. KEBETULAN, bukan keunggulan.
- **Fix**: shim `Sprite_Button.prototype.convertEscapeCharacters` delegasi ke `_helpWindow` → Window_Base pertama → fallback `Window_Base.prototype`.

## [2026-09-30] Audit graph & cache gotchas (Rust)

### Finding 4: Sanitasi path `rposition` memotong prefix `js/`
`core/src/asset_audit/report.rs` memakai `rposition` (well-known dir TERAKHIR): `js/plugins/x.js` → `plugins/x.js`; `js/plugins/inazuma/core/audio/WebAudioEx.js` → `audio/WebAudioEx.js`. Fix: `position` (well-known dir PERTAMA). Regression test ada.

### Finding 5: Audit cache key TIDAK memasukkan versi kode audit
`AuditCacheKey::to_sha256` hanya hash source_hash/engine/mode/ruleset_version/toolchain_version — kode audit tidak. Fix perilaku audit TANPA bump `CURRENT_AUDIT_RULESET_VERSION` → cache hit menyajikan graph lama (166 false positive lolos ke APK). **PELUSSA/ATURAN: setiap perubahan perilaku audit WAJIB bump `CURRENT_AUDIT_RULESET_VERSION`** (kini 11). Hardening future: hash kode audit di cache key.

### Finding 6: Probe race condition — launch async vs pidof instan
`adb shell monkey` kembali SEBELUM proses Android spawn (~500ms+). Cek `pidof` seketika setelah launch → false "RT-PROCESS-CRASH" padahal app hidup. Fix: grace period menunggu proses muncul (default 8s) + kode jujur `RT-LAUNCH-NO-PROCESS` jika tak pernah muncul. Berlaku umum untuk semua launcher async (am start juga async).

## [2026-09-30] Gotcha proses & operasional

- **Uji APK**: build tanpa `--package` derive package dari nama folder (`io.runtimepack.game.forestia144ind`) ≠ app lama (`com.example.forestia`). SELALU cek `pm list packages` + `dist\build-manifest.json` sebelum menyimpulkan hasil uji.
- **QA agent bisa salah klaim** ("0 failure" padahal 166) — data user di device adalah verifikasi akhir yang mengikat.
- **Build command**: `cargo run -q --bin runtimepack -- build <game-path> --flavor beta --output dist`. Probe: `runtimepack probe <apk> --device mumu` (device via `probe-profiles/devices.toml`, nol hardcode).
- **Asumsi plan MZ salah**: `isLoadableSlot`/`onLoadInvalid` TIDAK ADA di MZ core 1.4.4 (dari riset yang mengarang dari source lain). Verifikasi ke engine asli sebelum pakai.
- **Overlay WebView ≠ game WebView**: diagnostic_suite jalan di overlay terpisah — globals game tidak terlihat di sana; engine_family harus dari bridge `AssetAuditService.currentEngine`, bukan deteksi globals.
- **Cordova legacy** (komparasi): file:// scheme + case-sensitive fs = crash asset (jump1.ogg); MV silent empty bitmap = sprite hilang (kasus F&H). RuntimePack menutup via GPACK case-insensitive + audit + `--rtp-path`.

## Referensi
- Registry capability: `Docs/superpowers/compat/COMPAT-REGISTRY.md` (16 CAP + marker kode + test).
- Plan: `Docs/superpowers/plans/2026-09-30-mz-save-load-touch-input-fix.md`, `Docs/superpowers/plans/2026-09-30-stability-and-diagnostics-overhaul.md`.
- Handover lengkap: `Docs/FORESTIA_SAVE_LOAD_HANDOVER.md`.
