# Task 4 Brief: Fix `engine_family` di `diagnostic_suite.js`

**Plan:** `Docs/superpowers/plans/2026-09-30-stability-and-diagnostics-overhaul.md` (Task 4)

**Workspace:** worktree `D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save` (branch `feature/native-mv-mz-save`). Windows, PowerShell 5.1.

## Root cause (dari explorasi sub-agent 2026-09-30)
Diagnostic suite berjalan di overlay WebView terpisah (`file:///android_asset/overlay/index.html`, di-load `WebActivity.kt` `setupOverlayWebView` ~line 585-616 dengan bridge `RuntimePackVPad`). Di context overlay, `$gameMap`/`SceneManager`/`Utils` undefined → deteksi engine jatuh ke fallback hardcoded `"RGSS / mkxp-z"` (diagnostic_suite.js ~line 1138-1153). Padahal `AssetAuditService` sudah di-init dengan engine family dari build manifest (`RuntimeApplication.kt:21` — `AssetAuditService.init(filesDir, manifest.runtime.family, this)`).

## Files
- Modify: `android/shell/app/src/main/assets/overlay/diagnostic_suite.js` (blok deteksi engine ~line 1138-1153)
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt` (`createVPadBridge()` / objek `RuntimePackVPad` ~line 613)

## Steps

- [ ] **Step 1: Cari nama field/property engine family aktual di `AssetAuditService`**

```powershell
Select-String -Path android\shell\app\src\main\java\io\runtimepack\host\AssetAuditService.kt -Pattern "engine|family|init"
```
Identifikasi bagaimana engine family tersimpan (property `engineFamily`? companion object? param init?). Jika tersimpan privat tanpa accessor, tambahkan accessor sederhana. JANGAN mengarang nama — pakai nama aktual.

- [ ] **Step 2: Tambah method bridge di Kotlin** (di objek yang di-expose sebagai `RuntimePackVPad`):

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
(Sesuaikan `AssetAuditService.engineFamily` dengan nama field aktual dari Step 1; bila perlu instance, gunakan instance yang tersedia di Activity.)

- [ ] **Step 3: Ubah blok deteksi di `diagnostic_suite.js`** — ganti fallback hardcoded RGSS dengan bridge-first detection:

```javascript
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
    if (typeof globalObj.$gameMap !== "undefined" || typeof globalObj.SceneManager !== "undefined") {
        engineFamily = "rpgmaker_web";
    }
}
```
Pertahankan cabang deteksi lain yang masih benar (misal deteksi TYRANO/renpy globals bila ada); HANYA ganti cabang yang menghasilkan `"RGSS / mkxp-z"` hardcoded (string itu harus hilang dari file — kecuali benar-benar dipakai untuk deteksi RGSS asli via globals game, bukan fallback blind; kalau begitu biarkan cabang RGSS tapi letakkan SETELAH deteksi globals game, bukan sebagai else-if blind).

- [ ] **Step 4: Verifikasi**

```powershell
node --check android\shell\app\src\main\assets\overlay\diagnostic_suite.js
cd android\shell; gradlew.bat :app:compileDebugKotlin --console=plain
```
Expected: sintaks JS valid, Kotlin kompilasi sukses.

- [ ] **Step 5: Commit**

```powershell
git add android/shell/app/src/main/assets/overlay/diagnostic_suite.js android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt
git commit -m "fix(diagnostic): engine_family reported from build manifest bridge, not hardcoded RGSS fallback"
```

## Report contract
Tulis report lengkap ke:
`D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save\.superpowers\sdd\2026-09-30-stability-and-diagnostics-overhaul\task-4-report.md`

Reply ke saya HANYA: status, commit hash, nama field engine family aktual yang dipakai, concerns.
