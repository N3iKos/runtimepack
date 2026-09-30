# Task 5 Brief: Hapus instrumentasi `[RP_LOAD_DEBUG]` + fail-loud RT-LOAD-FAIL

**Plan:** `Docs/superpowers/plans/2026-09-30-stability-and-diagnostics-overhaul.md` (Task 5)

**Workspace:** worktree `D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save` (branch `feature/native-mv-mz-save`). Windows, PowerShell 5.1.

## Context
Commit `be978f9` menambah fungsi `patchLoadDebug()` di `runtimepack_web_shims.js` (instrumentasi sementara rantai load dengan log `[RP_LOAD_DEBUG]`, delimited komentar `// [RP_LOAD_DEBUG] instrumentation start/end`). Misi debug selesai (root cause HalfMove ditemukan & diperbaiki commit `a14860b`). Instrumentasi harus DIHAPUS, dan digantikan diagnostik permanen yang fail-loud supaya load failure senyap (kelas bug HalfMove) tidak terulang tanpa jejak.

Aturan penting: JANGAN ikut menghapus fix HalfMove (`Sprite_Button.prototype.convertEscapeCharacters` delegation) maupun patch input (`patchEngineInput`, `RuntimePackInput`) — keduanya fungsi terpisah.

## Files
- Modify: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
- Test: `android/shell/app/src/test/js/web_shims.test.js`

## Steps

- [ ] **Step 1: Tulis failing test dulu** — tambah suite "Load Failure Diagnostics" di akhir `web_shims.test.js` (ikuti gaya suite "Engine Input Patch" yang ada: `createSandbox`, mock engine globals SETELAH shim jalan, panggil poll hook manual):

```javascript
// ============================================================
// Load Failure Diagnostics (fail-loud RT-LOAD-FAIL)
// ============================================================
{
    const sb = createSandbox();
    vm.runInContext(shimsCode, sb);
    // Capture console.error
    const errors = [];
    vm.runInContext("this.__capturedErrors = [];", sb);
    sb.console.error = (msg) => { errors.push(String(msg)); };
    vm.runInContext(`
        this.__origFailureCalled = false;
        window.Scene_Load = function() {};
        window.Scene_Load.prototype.onLoadFailure = function() {
            window.__origFailureCalled = true;
        };
        if (typeof window.__runtimepackPollLoadDiagnostics === "function") {
            window.__runtimepackPollLoadDiagnostics();
        }
    `, sb);
    const result = vm.runInContext(`
        var s = new window.Scene_Load();
        s.onLoadFailure();
        JSON.stringify({ orig: window.__origFailureCalled });
    `, sb);
    assert.strictEqual(JSON.parse(result).orig, true, "original onLoadFailure must still run");
    assert(errors.some(e => e.indexOf("[RuntimePack] RT-LOAD-FAIL") !== -1),
        "onLoadFailure must log [RuntimePack] RT-LOAD-FAIL via console.error");
    assert(vm.runInContext("typeof window.__runtimepackShowLoadError === 'function'", sb) === true,
        "fail-loud display helper must be exposed");
    console.log("Load Failure Diagnostics tests PASSED");
}
```
Catatan implementasi test: `createSandbox` di file test menaruh `console.error` default; override di sandbox object SETELAH `runInContext(shimsCode)` supaya capture berlaku saat test jalan. Sesuaikan mock (mis. `document` untuk div overlay tidak ada di sandbox — implementasi `__runtimepackShowLoadError` harus guard try/catch tanpa document, cukup log + return).

- [ ] **Step 2: Run untuk verifikasi FAIL**

```powershell
node android/shell/app/src/test/js/web_shims.test.js
```
Expected: FAIL di assertion `RT-LOAD-FAIL` (belum diimplementasi).

- [ ] **Step 3: Hapus `patchLoadDebug()`** — hapus fungsi + panggilan dari `pollPatches()` + semua kemunculan `RP_LOAD_DEBUG`. Verifikasi nol sisa:

```powershell
(Select-String -Path android\shell\app\src\main\assets\web_shims\runtimepack_web_shims.js -Pattern "RP_LOAD_DEBUG").Count
```
Expected: 0.

Juga pastikan fix HalfMove TIDAK ikut terhapus:

```powershell
(Select-String -Path android\shell\app\src\main\assets\web_shims\runtimepack_web_shims.js -Pattern "convertEscapeCharacters").Count
```
Expected: > 0.

- [ ] **Step 4: Implement `patchLoadDiagnostics()`** (pola late-binding sama seperti `patchEngineInput`, guard `__runtimepack_load_diag_patched`, dipanggil dari `pollPatches()` saat `root.Scene_Load` ada, diekspos juga sebagai `root.__runtimepackPollLoadDiagnostics` untuk test):

1. `__runtimepackShowLoadError(msg)`: `console.error("[RuntimePack] RT-LOAD-FAIL " + msg)` + coba buat div overlay on-screen (guard `typeof document !== "undefined"`): `position:fixed; left:0; right:0; top:40%; background:rgba(150,0,0,0.85); color:#fff; font:bold 14px sans-serif; text-align:center; padding:12px; z-index:99999;` isi pesan, append ke `document.body`, `setTimeout` remove 8000ms. Semua dalam try/catch (diagnostics tidak boleh crash game).
2. Wrap `Scene_Load.prototype.onLoadFailure`: panggil `__runtimepackShowLoadError("Load game gagal — cek logcat [GameConsole] untuk detail (RT-LOAD-FAIL)")` lalu call original.
3. Wrap `DataManager.loadGame` (jika `root.DataManager.loadGame` ada): hasil asli dibungkus `.catch(err => { console.error("[RuntimePack] RT-LOAD-FAIL loadGame rejection:", err && err.message ? err.message : err); throw err; })` — re-throw agar perilaku engine tidak berubah.
4. Jangan hapus/ubah `window.addEventListener("unhandledrejection")` global yang sudah ada di atas file.

- [ ] **Step 5: Run seluruh suite** → semua PASS (suite lama + Engine Input Patch + Load Failure Diagnostics).

- [ ] **Step 6: Commit**

```powershell
git add android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js android/shell/app/src/test/js/web_shims.test.js
git commit -m "fix(web-runtime): remove RP_LOAD_DEBUG instrumentation; add fail-loud RT-LOAD-FAIL diagnostics for silent load failures"
```

## Report contract
Tulis report lengkap ke:
`D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save\.superpowers\sdd\2026-09-30-stability-and-diagnostics-overhaul\task-5-report.md`

Reply ke saya HANYA: status, commit hash, jumlah sisa `RP_LOAD_DEBUG` (harus 0), ringkasan test satu baris, concerns.
