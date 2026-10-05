# RuntimePack Stability & Diagnostics Overhaul — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mengubah RuntimePack dari "wrapper yang bisa jalan game" menjadi runtime yang stabil native-ish: diagnostik yang jujur (tanpa false positive), compat layer terdokumentasi sebagai capability registry, dan runtime probe CLI otomatis (boot→save→load) yang mencegah kelas bug seperti HalfMove lolos ke tangan user.

**Architecture:** Tiga fase berurutan. Phase 1 membereskan fondasi diagnostik (false positive 1158, engine_family salah, instrumentasi debug menempel, load failure senyap) supaya semua alat ukur bisa dipercaya. Phase 2 merestrukturisasi shim compat menjadi capability registry terdokumentasi (CAP-xxx). Phase 3 membangun `runtimepack probe` di CLI dengan device-agnostic interface dan implementasi MuMu pertama. Probe Phase 3 memakai infrastruktur probe yang dibenahi di Phase 1 — itulah urutan dependensinya.

**Tech Stack:** Rust (CLI `cli/`, core `core/`), Kotlin (Android host `android/shell/`), JavaScript vanilla (shim `runtimepack_web_shims.js`, overlay `diagnostic_suite.js`), Node.js `vm`+`assert` (test suite JS), MuMu emulator via MCP/ADB (probe target).

**Spec:** `Docs/FORESTIA_SAVE_LOAD_HANDOVER.md`, hasil riset deepresearch 2026-09-30, hasil explorasi false-positive probe (sub-agent explore, 2026-09-30), `AGENTS.md` §9 (shim butuh capability ID + implementation + test + diagnostic behavior) dan §16 (test hierarchy hingga load), diskusi arah dengan user 2026-09-30 (MuMu first, device-agnostic interface).

---

## Global Constraints

- **Input game immutable** — jangan pernah memodifikasi `Raw-Game/`. Semua perubahan hanya di `core/`, `cli/`, `engines/`, `android/shell/`.
- **Zero-dependency JavaScript** di shim & overlay; tidak ada library eksternal baru.
- **Semua patch shim idempotent** (guard `__runtimepack_*_patched`) dan **late-binding** (pola `pollPatches`).
- **Error/log prefix `[RuntimePack]`**, kode diagnostik stabil sesuai domain AGENTS.md §18 (`CMP-xxx` untuk compat, `RT-xxx` runtime, `WEB-xxx` web, `BLD-xxx` build).
- **Build APK wajib `--flavor beta`** kecuali instruksi eksplisit lain.
- **Build command baku**: `cargo run -q --bin runtimepack -- build "D:\02_Development\Projects_2\RPGM-P\Raw-Game\Forestia1.4.4-IND" --flavor beta --output dist` (dari root repo yang berisi CLI). Package default derive nama folder: `io.runtimepack.game.forestia144ind`. SELALU verifikasi `dist\build-manifest.json` + `pm list packages` sebelum menyimpulkan hasil uji.
- **Test commands baku**: `node android/shell/app/src/test/js/web_shims.test.js`; `gradlew.bat :app:compileDebugKotlin --console=plain` dari `android/shell`; `cargo test --workspace -q` untuk Rust.
- **Dua workspace git**: repo utama `D:\02_Development\Projects_2\RPGM-P` (branch `master`, MEMILIH banyak file uncommitted dari Track 3/Track D) dan worktree `.worktrees/native-mv-mz-save` (branch `feature/native-mv-mz-save`, HEAD `a14860b` — berisi semua fix save/load). **Stage file secara selektif** — jangan `git add -A` di master karena ada 19 file uncommitted yang tidak semua milik plan ini.
- Emulator: MuMu vmindex 0, ADB port 16384, root tersedia. App terinstall saat ini: `io.runtimepack.game.forestia144ind` (fixed) dan `com.example.forestia` (lama).
- Semua pekerjaan eksekusi didelegasikan ke sub-agent; supervisor tidak menulis kode.

---

## File Structure (peta perubahan)

| File | Aksi | Fase | Tanggung Jawab |
| :--- | :--- | :--- | :--- |
| `core/src/asset_audit/report.rs` | Commit fix yang sudah ada di master working tree (`rposition`→`position` + regression test), lalu merge ke worktree | 1 | Path sanitization mempertahankan prefix `js/` pada `js/plugins/...` |
| `android/shell/.../AssetProbeRunner.kt`, `AssetAuditService.kt` | Commit fix GPACK lookup dari master working tree, merge ke worktree | 1 | Probe cek file di dalam `game.pack`, bukan hanya disk |
| `android/shell/.../overlay/diagnostic_suite.js` | Fix `engine_family` via bridge, hapus fallback hardcoded RGSS | 1 | Deteksi engine jujur di overlay WebView |
| `android/shell/.../web_shims/runtimepack_web_shims.js` (worktree) | Hapus instrumentasi `[RP_LOAD_DEBUG]`; tambah fail-loud `onLoadFailure`; tambah header capability registry | 1, 2 | Shim bersih + load failure tidak senyap + registry |
| `android/shell/.../host/WebBackend.kt`, `NativeSaveRepository.kt`, `NativeSaveRepositoryTest.kt` (worktree) | Commit (masih untracked/unstaged dari plan lama) | 1 | Menutup utang git worktree |
| `Docs/superpowers/compat/COMPAT-REGISTRY.md` | Create | 2 | Dokumen capability registry CAP-xxx untuk semua shim |
| `cli/src/...` (probe subcommand), `core/src/probe/...` atau `cli/src/probe/` | Create | 3 | `runtimepack probe` CLI + trait `ProbeTarget` + `MumuTarget` |
| `memory/progress.md`, `memory/technical_findings.md`, `memory/decisions_log.md` | Update tiap akhir fase | 1-3 | Ledger sesuai Memory Protocol |

---

# PHASE 1 — FONDASI: DIAGNOSTIK JUJUR

> Checkpoint: setelah Phase 1 selesai, APK dibuild ulang dan probe di MuMu harus menampilkan 0 false positive FILE_EXISTS, `engine_family` benar (RPG Maker MZ web), load game tetap PASS, dan tidak ada log `[RP_LOAD_DEBUG]`.

---

### Task 1: Commit fix false-positive audit di MASTER (selective stage)

**Workspace:** repo utama `D:\02_Development\Projects_2\RPGM-P` (branch `master`).

**Files:**
- Commit (sudah ada di working tree): `core/src/asset_audit/report.rs`
- Verifikasi: `git diff HEAD -- core/src/asset_audit/report.rs` harus berisi `rposition` → `position` dan test `test_sanitize_path_str_preserves_js_plugins_prefix`

**Catatan penting:** master punya 19 file uncommitted. HANYA stage file di atas. `report.rs` milik fix "1158 false positive" Track 3. JANGAN menyentuh `AGENTS.md`, `RenpyActivity.kt`, `memory/*`, dll — itu milik pekerjaan lain (Track D).

- [ ] **Step 1: Verifikasi diff yang akan di-commit**

```bash
git -C "D:\02_Development\Projects_2\RPGM-P" diff HEAD -- core/src/asset_audit/report.rs
```
Expected: perubahan `rposition(|segment|` → `position(|segment|` di fungsi sanitasi path (sekitar line 148-154), plus tambahan `#[test] fn test_sanitize_path_str_preserves_js_plugins_prefix()` yang meng-assert `js/plugins/inazuma/Activity.js` → `js/plugins/inazuma/Activity.js` (prefix dipertahankan).

- [ ] **Step 2: Jalankan test Rust terkait**

```bash
cargo test -p runtimepack-core asset_audit -- --nocapture
```
Expected: semua test `asset_audit` PASS termasuk test baru.

- [ ] **Step 3: Commit selective**

```bash
git -C "D:\02_Development\Projects_2\RPGM-P" add core/src/asset_audit/report.rs
git -C "D:\02_Development\Projects_2\RPGM-P" commit -m "fix(asset-audit): preserve js/ prefix for js/plugins paths in logical path sanitization (1158 false positives)"
```

### Task 2: Commit fix GPACK probe lookup di MASTER (selective stage)

**Workspace:** repo utama (branch `master`).

**Files:**
- Commit (sudah ada di working tree): `android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt`, `android/shell/app/src/main/java/io/runtimepack/host/AssetAuditService.kt`
- Verifikasi juga apakah `AssetProbeRunnerTest.kt` dan `VfsResourceInterceptor.kt` termasuk bagian fix ini (cek diff-nya; stage HANYA yang benar-benar bagian probe fix).

**Interfaces:**
- Produces (untuk APK build): `AssetProbeRunner` menerima `gpackReader`, cek `reader?.hasFile(relPath)` sebelum melaporkan "File not found".

- [ ] **Step 1: Verifikasi diff**

```bash
git -C "D:\02_Development\Projects_2\RPGM-P" diff HEAD -- android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt android/shell/app/src/main/java/io/runtimepack/host/AssetAuditService.kt
```
Expected: tambahan param `gpackReader`, lookup `reader?.hasFile(relPath)` di probe loop, discovery `asset-dependency-graph.json` dari stream GPACK, fallback scan `reader.entries`.

- [ ] **Step 2: Verifikasi kompilasi + test Kotlin**

```bash
cd android/shell; gradlew.bat :app:compileDebugKotlin :app:testDebugUnitTest --console=plain
```
Expected: BUILD SUCCESSFUL.

- [ ] **Step 3: Commit selective**

```bash
git -C "D:\02_Development\Projects_2\RPGM-P" add android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt android/shell/app/src/main/java/io/runtimepack/host/AssetAuditService.kt
git -C "D:\02_Development\Projects_2\RPGM-P" commit -m "fix(android-host): probe resolves assets inside GPACK instead of disk-only lookup"
```

### Task 3: Merge master → worktree feature/native-mv-mz-save

**Workspace:** worktree `.worktrees/native-mv-mz-save`.

**Files:**
- Merge: semua commit master baru (Task 1-2) masuk ke branch worktree.

- [ ] **Step 1: Merge**

```bash
git -C "D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save" merge master -m "merge: bring probe false-positive fixes from master"
```
Jika konflik pada `runtimepack_web_shims.js` / `web_shims.test.js` (karena worktree punya versi sendiri): pertahankan versi worktree (HEAD) untuk shim — semua perubahan master pada shim sudah tercakup oleh commit worktree `dc09d92`. Konflik lain diselesaikan manual dengan membaca kedua sisi; jangan timpa fix save/load.

- [ ] **Step 2: Verifikasi post-merge**

```bash
node android/shell/app/src/test/js/web_shims.test.js
cd android/shell; gradlew.bat :app:compileDebugKotlin --console=plain
cargo test -p runtimepack-core asset_audit
```
Expected: semua hijau.

### Task 4: Fix `engine_family` di `diagnostic_suite.js`

**Workspace:** worktree.

**Files:**
- Modify: `android/shell/app/src/main/assets/overlay/diagnostic_suite.js` (deteksi engine sekitar line 1138-1153)
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt` (bridge expose engine, di `createVPadBridge()` / `RuntimePackVPad` sekitar line 613)

**Root cause (dari explorasi):** diagnostic suite jalan di overlay WebView terpisah (`file:///android_asset/overlay/index.html`) — di context itu `$gameMap`/`SceneManager` undefined, jadi deteksi jatuh ke fallback hardcoded `"RGSS / mkxp-z"`. Padahal `AssetAuditService` sudah terinisialisasi dengan engine family dari build manifest (`RuntimeApplication.kt:21`).

**Interfaces:**
- Produces: `RuntimePackVPad.getEngineFamily()` → string engine dari `AssetAuditService` (mis. `"RPG Maker Web"`), fallback `"unknown"`. Diagnostic suite memakai nilai bridge; hapus hardcoded RGSS.

- [ ] **Step 1: Tambah method di Kotlin bridge** (`createVPadBridge()` di WebActivity.kt):

```kotlin
@JavascriptInterface
fun getEngineFamily(): String {
    return try {
        AssetAuditService.engineFamily ?: "unknown"
    } catch (e: Exception) {
        "unknown"
    }
}
```
(Verifikasi nama property/field engine family aktual di `AssetAuditService` via grep `engineFamily|engine_family|_engine` — pakai nama yang benar, jangan mengarang. Jika perlu, tambahkan accessor/field di `AssetAuditService` yang menyimpan family dari init.)

- [ ] **Step 2: Ubah deteksi di `diagnostic_suite.js`** (ganti blok fallback):

```javascript
// SEBELUM: else-if chain dengan hardcoded "RGSS / mkxp-z"
// SESUDAH: cek bridge dulu, game globals hanya sebagai pelengkap
var engineFamily = "unknown";
try {
    if (globalObj.RuntimePackVPad && typeof globalObj.RuntimePackVPad.getEngineFamily === "function") {
        var bridged = globalObj.RuntimePackVPad.getEngineFamily();
        if (bridged && typeof bridged === "string" && bridged.length > 0) {
            engineFamily = bridged;
        }
    }
} catch (e) {}
if (engineFamily === "unknown") {
    // fallback: deteksi dari globals game (context overlay umumnya tidak punya akses)
    if (typeof globalObj.$gameMap !== "undefined" || typeof globalObj.SceneManager !== "undefined") {
        engineFamily = "rpgmaker_web";
    }
}
```
Jangan hapus deteksi globals lain yang masih benar; hanya ganti hardcoded RGSS fallback.

- [ ] **Step 3: Kompilasi + test JS**

```bash
cd android/shell; gradlew.bat :app:compileDebugKotlin --console=plain
node ../app/src/test/js/web_shims.test.js   # dari android/shell; atau jalankan dari root worktree
```
Expected: hijau. (diagnostic_suite.js tidak punya test suite khusus — cukup pastikan sintaks valid via `node --check`.)

```bash
node --check android/shell/app/src/main/assets/overlay/diagnostic_suite.js
```

- [ ] **Step 4: Commit**

```bash
git add android/shell/app/src/main/assets/overlay/diagnostic_suite.js android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt
git commit -m "fix(diagnostic): engine_family reported from build manifest bridge, not hardcoded RGSS fallback"
```

### Task 5: Hapus instrumentasi `[RP_LOAD_DEBUG]` + fail-loud load failure

**Workspace:** worktree.

**Files:**
- Modify: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` — hapus seluruh fungsi `patchLoadDebug()` dan pemanggilannya di `pollPatches()` (delimited `// [RP_LOAD_DEBUG] instrumentation start/end`, dari commit `be978f9`).
- Modify (tambah baru): fail-loud wrapper di shim.
- Test: `android/shell/app/src/test/js/web_shims.test.js`.

**Interfaces:**
- Produces: `Scene_Load.prototype.onLoadFailure` wrapper yang console.error `[RuntimePack] RT-LOAD-FAIL <reason>` + tampilkan pesan on-screen di flavor beta (elemen div overlay sederhana, auto-hide 8 detik, non-blocking).
- Produces: unhandled rejection `DataManager.loadGame` ter-logging via handler global yang sudah ada (`[RuntimePack Unhandled Rejection]`) — verifikasi rantainya, tambahkan `.catch` logging di wrapper `DataManager.loadGame` bila rejection belum ter-logging dengan context savefileId.

- [ ] **Step 1: Hapus `patchLoadDebug()`** — hapus fungsi + panggilannya + kondisi polling-nya. Cari string `RP_LOAD_DEBUG` dan pastikan 0 sisa: `Select-String -Path <shim> -Pattern RP_LOAD_DEBUG` → kosong.

- [ ] **Step 2: Tulis failing test untuk fail-loud** (tambah suite "Load Failure Diagnostics" di web_shims.test.js):

```javascript
{
    const sb = createSandbox();
    vm.runInContext(shimsCode, sb);
    vm.runInContext(`
        var failures = [];
        window.Scene_Load = function() {};
        window.Scene_Load.prototype.onLoadFailure = function() { failures.push("orig"); };
        window.SceneManager = { _scene: null };
        window.__runtimepackPollEngineInput_check = function() {};
        if (typeof window.__runtimepackPollLoadDiagnostics === "function") {
            window.__runtimepackPollLoadDiagnostics();
        }
        window.__testFailures = failures;
    `, sb);
    const called = vm.runInContext(`
        var s = new window.Scene_Load();
        s.onLoadFailure();
        window.__testFailures.join(",");
    `, sb);
    assert.strictEqual(called, "orig", "original onLoadFailure must still run");
    assert(vm.runInContext("typeof window.__runtimepackShowLoadError === 'function'", sb) === true,
        "fail-loud display helper must be exposed for testing");
    console.log("Load Failure Diagnostics tests PASSED");
}
```
(Penamaan poll hook disesuaikan dengan implementasi — yang diuji: original handler tetap terpanggil, helper display tersedia, log `[RuntimePack] RT-LOAD-FAIL` terpanggil — assert via mock console.error yang dicapture dalam sandbox.)

- [ ] **Step 3: Implement fail-loud** — fungsi `patchLoadDiagnostics()` (pola late-binding sama seperti `patchEngineInput`):
  - Wrap `Scene_Load.prototype.onLoadFailure`: log `console.error("[RuntimePack] RT-LOAD-FAIL load game gagal (slot terakhir: " + (this._savefileId !== undefined ? this._savefileId : "unknown") + ")")`, tampilkan `__runtimepackShowLoadError(msg)` (div overlay `position:fixed`, `z-index:99999`, `background:rgba(180,0,0,0.85)`, teks putih, auto-remove 8000ms, dibungkus try/catch), lalu call original.
  - Wrap `DataManager.loadGame` (jika ada di global): `.catch(err => { console.error("[RuntimePack] RT-LOAD-FAIL loadGame rejection:", err && err.message ? err.message : err); throw err; })` — re-throw supaya perilaku engine tidak berubah.
  - Guard `__runtimepack_load_diag_patched`, dipanggil dari `pollPatches()` saat `root.Scene_Load || root.DataManager`.

- [ ] **Step 4: Jalankan seluruh suite** → hijau.

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js android/shell/app/src/test/js/web_shims.test.js
git commit -m "fix(web-runtime): remove RP_LOAD_DEBUG instrumentation; add fail-loud RT-LOAD-FAIL diagnostics for silent load failures"
```

### Task 6: Commit file save-adapter yang tersisa di worktree

**Workspace:** worktree.

**Files:**
- Commit: `android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt` (modified), `android/shell/app/src/main/java/io/runtimepack/host/NativeSaveRepository.kt` (untracked), `android/shell/app/src/test/java/io/runtimepack/host/NativeSaveRepositoryTest.kt` (untracked)

- [ ] **Step 1: Verifikasi diff WebBackend.kt hanya 5 bridge method save** (`saveSlot`, `loadSlot`, `slotExists`, `deleteSlot`, `listSlots`).

- [ ] **Step 2: Jalankan test JVM**

```bash
cd android/shell; gradlew.bat :app:testDebugUnitTest --console=plain
```
Expected: `NativeSaveRepositoryTest` PASS.

- [ ] **Step 3: Commit**

```bash
git add android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt android/shell/app/src/main/java/io/runtimepack/host/NativeSaveRepository.kt android/shell/app/src/test/java/io/runtimepack/host/NativeSaveRepositoryTest.kt
git commit -m "feat(android-host): atomic native save repository with JVM tests (save adapter finalization)"
```

### Task 7: Rebuild APK final Phase 1 + verifikasi QA di MuMu (sub-agent qa-tester)

- [ ] **Step 1: Rebuild full** (dari root worktree — graph audit di-regenerate, bukan hanya rebuild Kotlin):

```bash
cargo run -q --bin runtimepack -- build "D:\02_Development\Projects_2\RPGM-P\Raw-Game\Forestia1.4.4-IND" --flavor beta --output dist
```

- [ ] **Step 2: Verifikasi bundle** — `dist\android-project\...\runtimepack_web_shims.js` bebas `RP_LOAD_DEBUG`, mengandung `RT-LOAD-FAIL`; `AssetProbeRunner.kt` project mengandung `hasFile`.

- [ ] **Step 3: Dispatch sub-agent qa-tester** (vmindex 0): install (reinstall preserve save), jalankan diagnostic overlay, verifikasi: (a) probe EXHAUSTIVE 0 failure (atau failure hilang untuk plugins), (b) `engine_family` = "RPG Maker Web" (atau string manifest yang benar, bukan RGSS), (c) load test: tap 1x File 1 → masuk map, (d) gamepad A → PASS, (e) logcat bebas `RP_LOAD_DEBUG`. Laporan lengkap + screenshot.

- [ ] **Step 4: Update memory** (`progress.md`, `technical_findings.md`) + commit.

---

# PHASE 2 — STRUKTUR: CAPABILITY COMPAT REGISTRY

> Checkpoint: setiap patch shim punya entri CAP-xxx terdokumentasi + test regression; `Docs/superpowers/compat/COMPAT-REGISTRY.md` jadi sumber kebenaran tunggal.

### Task 8: Tulis COMPAT-REGISTRY.md + header registry di shim

**Files:**
- Create: `Docs/superpowers/compat/COMPAT-REGISTRY.md`
- Modify: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (header komentar registry per patch — dokumentasi inline yang merujuk CAP ID)

**Format entri registry (dipakai untuk SEMUA shim):**

```markdown
## CAP-WEB-001 — readdirSync native bridge (getRoadRegion fix)
- Engine: MV/MZ | Trigger: fs.readdirSync("data") dipanggil plugin navigasi
- Symptom (tanpa patch): TypeError getRoadRegion, NPC pathfinding crash
- Implementation: WebBackend.kt readdir bridge + fallback data/MapInfos.json
- Evidence: Forestia crash repro → fix verified (sesi sebelumnya)
- Regression test: web_shims.test.js suite "readdir bridge"
- Diagnostic: WEB-FS-READDIR saat bridge unavailable
```

Entri yang wajib terdokumentasi (dari shim yang ada saat ini): readdirSync bridge (CAP-WEB-001), MZ storage adapter native save/load (CAP-WEB-002), MV webstorage adapter (CAP-WEB-003), single-tap touch OK + moveThreshold (CAP-WEB-004), Input codeMapper + RuntimePackInput (CAP-WEB-005), HalfMove convertEscapeCharacters delegation (CAP-WEB-006), load failure fail-loud (CAP-WEB-007), isLocalMode virtualization (CAP-WEB-008), nw.gui/Greenworks/Steamworks mocks (CAP-WEB-009), Graphics stretch fit (CAP-WEB-010), AudioManager ogg ext (CAP-WEB-011).

- [ ] Step 1: Tulis registry lengkap (dokumen; setiap entri merujuk test suite yang sudah ada di `web_shims.test.js` — bila ada patch yang TIDAK punya test, catat sebagai `[ ] missing regression test` dan buat task lanjutan).
- [ ] Step 2: Tambahkan komentar `// [CAP-WEB-xxx]` pada tiap fungsi patch di shim (idempotent marker yang bisa digrep).
- [ ] Step 3: Verifikasi grep mapping: setiap CAP di dokumen punya marker di kode, sebaliknya.
- [ ] Step 4: Commit: `docs(compat): capability registry for all web runtime shims (CAP-WEB-001..011)`

### Task 9: Isi test regression yang hilang (jika ada dari Task 8)

- [ ] Per temuan Task 8: tulis test untuk patch yang belum teruji (TDD style), jalankan suite, commit `test(web-runtime): close capability registry test gaps (CAP-WEB-xxx..)`.

---

# PHASE 3 — OTOMASI: RUNTIME PROBE CLI (REVISI: SCOPE T1 ONLY)

> REVISI 2026-09-30 (disepakati user setelah diskusi): probe disederhanakan ke **T1 (universal smoke test)** saja. Save/load round-trip (T2: suntik save template per engine; T3: save in-game) ditunda sebagai **future work** — arsitektur `ProbeTarget` sudah menampung, tapi tidak dibangun di plan ini. Alasan: (a) menulis save itu bagian tersulit (prolog, naming, sistem save gacha seperti F&H) padahal jalur save sudah ter-cover test suite adapter + fail-loud RT-LOAD-FAIL sebagai jaring pengaman load; (b) T1 menangkap ~80% bug porting dengan kalibrasi nol; (c) probe adalah fitur pelengkap, bukan core — nilai per usaha maksimal di T1.
>
> REVISI KEDUA (disepakati user): **device TIDAK hardcode**. Bukan `MumuTarget`, melainkan `AdbTarget` generik; device/emulator didefinisikan dari **config + argumen CLI**.

> Checkpoint: `runtimepack probe` mengeksekusi boot→new game→input→error scan di device ADB pilihan user, melaporkan PASS/FAIL per langkah dengan kode RT-xxx, T1 berlaku universal untuk semua game tanpa kalibrasi.

### Definisi tingkat probe (konteks keputusan)

| Tingkat | Isi | Status |
| :--- | :--- | :--- |
| **T1 — universal smoke** | boot, first frame, new game via D-pad+Enter, masuk map, tombol gamepad, audio aktif, error scan, FPS | ✅ **Scope plan ini** |
| T2 — load via suntikan save template per engine (MZ/MV: file JSON-compressed; RGSS: Marshal) | future work (catatan desain di registry) |
| T3 — save round-trip in-game (autosave/inject/cheat fallback) | future work |
| Save-anywhere capability (cheat menu) | future work — entry capability manual per game-profile, dikerjakan agent saat build profil, bukan prasyarat probe |

### Task 10: Core probe trait + AdbTarget dari config CLI (Rust)

**Files:**
- Create: `cli/src/probe/mod.rs`, `cli/src/probe/target.rs` (trait `ProbeTarget`), `cli/src/probe/adb.rs` (implementasi ADB generik), `cli/src/probe/scenario.rs` (skenario T1), `probe-profiles/` (dir config)
- Modify: `cli/src/main.rs` (subcommand `probe`)
- Config device: `devices` section di `runtimepack.toml` (atau `probe-profiles/devices.toml` — ikuti konvensi config CLI yang ada; verifikasi saat eksekusi)

**Interface trait (kontrak untuk semua implementasi device):**

```rust
pub trait ProbeTarget {
    fn name(&self) -> &str;
    fn ensure_device(&mut self) -> Result<(), ProbeError>;      // adb connect, boot state
    fn install_apk(&mut self, apk: &Path) -> Result<String, ProbeError>;  // -> package id
    fn launch(&mut self, package: &str) -> Result<(), ProbeError>;
    fn tap(&mut self, x: u32, y: u32) -> Result<(), ProbeError>;
    fn key(&mut self, keycode: &str) -> Result<(), ProbeError>; // "KEYCODE_DPAD_DOWN", dst.
    fn screenshot(&mut self, out: &Path) -> Result<(), ProbeError>;
    fn logcat_since(&mut self, tag: &str) -> Result<Vec<String>, ProbeError>;
    fn clear_logcat(&mut self) -> Result<(), ProbeError>;
}
```

**Implementasi `AdbTarget`** — SATU implementasi generik via `adb -s <address> <cmd>` (subprocess `Command`). MuMu/LDPlayer/BlueStacks/Nox/AVD hanya beda `address`, tanpa perubahan kode.

**Config device (bukan hardcode):**

```toml
# probe-profiles/devices.toml
[devices.mumu]
address = "127.0.0.1:16384"

# contoh penambahan emulator lain (nanti, tanpa sentuh kode):
# [devices.ldplayer]
# address = "127.0.0.1:5555"
```

**CLI:**
```
runtimepack probe <apk> --device mumu              # pakai entry config
runtimepack probe <apk> --device 127.0.0.1:21503   # alamat ADB ad-hoc
runtimepack probe <apk>                            # tanpa --device: scan `adb devices`; 1 device aktif → pakai; >1 → error minta eksplisit
runtimepack probe <apk> --list-devices             # tampilkan device aktif
```

**Skenario T1 (universal, tanpa kalibrasi per game):**
1. install → clear logcat → launch → tunggu first frame: screenshot 2x (poll 2s, timeout 60s), bukti layar tidak hitam/berubah
2. deteksi title screen: screenshot stabil ≥5 detik → PASS "boot"
3. new game via keyboard nav: `KEYCODE_DPAD_DOWN` × N (N dari profil, default 0) + `KEYCODE_ENTER` → tunggu scene berubah drastis (pixel-diff threshold) ≤60s → PASS "new_game"
4. test input sentuh: tap 1x di area tengah layar → layar tidak error/crash (log scan)
5. test gamepad: `KEYCODE_BUTTON_A` / Z → log engine merespons (atau minimal tidak crash)
6. error scan: logcat `[GameConsole]`/`[RuntimePack]`/chromium ERROR → klasifikasi; error fatal = FAIL dengan kode
7. FPS/stability: total durasi sesi + tidak ada ANR/crash process (`pidof <package>` masih hidup)
8. laporan: PASS/FAIL per langkah + `probe-report.json` di dist

**Profil per-game opsional** `probe-profiles/<game>.toml`: `dpad_down_count` (default 0 = New Game di posisi kursor awal), `package_id` override, `tap_probe_point` (default tengah layar). Kalibrasi minimal; mayoritas game tanpa profil.

- [ ] Step 1: TDD — `cargo test` untuk `scenario` logic dengan `MockTarget` (in-memory ProbeTarget) yang mensimulasikan device; scenario harus menghasilkan urutan langkah + verdict benar, termasuk kasus: boot timeout → FAIL RT-BOOT-TIMEOUT; new game tidak berubah layar → FAIL RT-NEWGAME-STALL; error log muncul → FAIL RT-CONSOLE-ERROR.
- [ ] Step 2: Implement `AdbTarget` + config loader + subcommand `probe`.
- [ ] Step 3: `cargo test -p runtimepack-cli` hijau + `cargo build` sukses.
- [ ] Step 4: Commit: `feat(cli): runtime probe subcommand (T1 universal) with config-driven AdbTarget`

### Task 11: Verifikasi probe positif (device pilihan user)

- [ ] Step 1: Jalankan `runtimepack probe dist\Forestia1.4.4-IND.apk --device mumu` terhadap APK final Phase 1 → EXPECTED: semua langkah T1 PASS, `dist\probe-report.json` tersimpan. (QA agent/executor menjalankan; bila RAM host sibuk, user menjalankan manual dan melaporkan output — fleksibel.)
- [ ] Step 2: Smoke test argumen: `--list-devices` menampilkan device; `--device` dengan alamat ad-hoc jalan.
- [ ] Step 3: Commit kalibrasi/fix kecil hasil verifikasi: `fix(probe): <temuan verifikasi>` bila ada.

### Task 12: Integrasi opsional ke pipeline build

- [ ] Step 1: Tambah flag `--probe [--device <id>]` pada `build` (setelah APK jadi, jalankan probe T1; skip dengan INFO bila tidak ada device aktif). Default build TANPA probe.
- [ ] Step 2: `cargo test` + build smoke → commit `feat(cli): optional --probe post-build T1 verification`.
- [ ] Step 3: Update AGENTS.md §16 pointer kecil: runtime validation tersedia via `runtimepack probe` (commit terpisah).

### Task 13: Penutupan plan

- [ ] Step 1: Final whole-branch review (code-reviewer, model paling mampu) atas seluruh diff plan di worktree — termasuk triage minor deferred (RgssActivity bridge, mojibake path entry, cache-key hardening opsional).
- [ ] Step 2: Update `memory/progress.md`, `memory/technical_findings.md`, `memory/decisions_log.md` (keputusan: T1-first probe; device via config bukan hardcode; capability registry format; fail-loud policy; save-anywhere = capability manual per profil).
- [ ] Step 3: Presentasikan hasil semua fase ke user → keputusan merge/finishing branch via skill `finishing-a-development-branch`.

---

## Urutan Eksekusi & Dispatch Map (revisi post-Phase-1)

| Task | Sub-agent role | Workspace | Status | Checkpoint user |
| :--- | :--- | :--- | :--- | :--- |
| 1-2 | executor (selective commit) | master | ✅ DONE | — |
| 3 (6) | executor (merge; T6 digeser sebelum merge) | worktree | ✅ DONE | — |
| 4-5 | executor (TDD shim) | worktree | ✅ DONE | — |
| 6 | executor (commit adapter) | worktree | ✅ DONE | — |
| 7 (+7b cache fix) | executor + user manual verify | worktree | ✅ DONE (user verifikasi 0 error) | ✅ CP1 lewat |
| 8-9 | executor (docs + test) | worktree | ⬜ TODO | — |
| 10 | executor (Rust TDD, T1 scope, AdbTarget config) | worktree | ⬜ TODO | — |
| 11 | executor (probe positif; QA bila RAM lega) | worktree | ⬜ TODO | **CP: review probe T1** |
| 12 | executor (--probe flag) | worktree | ⬜ TODO | — |
| 13 | code-reviewer + supervisor | worktree | ⬜ TODO | **CP: final, presentasi merge** |

Setiap task tetap melewati task-review (spec + quality) sebelum complete; final review menyapu seluruh branch.

## Self-Review (revisi)

1. **Coverage**: 4 item prioritas user tetap terpetakan — (1) registry = Task 8-9, (2) fail-loud = Task 5 ✅, (3) probe CLI = Task 10-12 (scope T1 revisi), (4) hapus instrumentasi = Task 5 ✅. Tambah: cache-bust `f3b3e20` (Task 7b) ✅. ✅
2. **Placeholder**: command/kode konkret; verifikasi nama field aktual tetap diinstruksikan eksplisit. ✅
3. **Konsistensi**: kode verdict T1 kini `RT-BOOT-TIMEOUT`/`RT-NEWGAME-STALL`/`RT-CONSOLE-ERROR` (RT-LOAD-STALL hanya untuk T2 future work). `ProbeTarget` dipakai Task 10-12; `AdbTarget` menggantikan `MumuTarget` di semua titik. ✅
4. **Risiko terbesar tersisa**: Task 10 pixel-diff heuristik bisa false-positive/negative — mitigasi: threshold longgar + verdict direview manual di CP; Task 11 negative test build kedua DIHAPUS dari scope (T2 future work), sehingga biaya turun drastis. ✅
