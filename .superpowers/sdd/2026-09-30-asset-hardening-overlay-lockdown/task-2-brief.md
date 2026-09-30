# Task 2 Brief: Host — OverlayAssetSource to load overlay/shims from GPACK

**Plan:** `Docs/superpowers/plans/2026-09-30-asset-hardening-overlay-lockdown.md` (Task 2)

**Workspace:** worktree `D:\02_Development\Projects_2\RPGM-P\.worktrees\asset-hardening` (branch `feature/asset-hardening`). Windows, PowerShell 5.1.

## Goal
Menyediakan `OverlayAssetSource` di Kotlin host Android agar WebView overlay (`overlayWebView`) dan injeksi JS shims (`webShimsScript`) dapat memuat file `overlay/*`, `web_shims/*`, dan `fonts/*` langsung dari `game.pack` (GPACK binary) saat file plain tidak tersedia di `assets/` (seperti pada build `--flavor release`). Jika file plain ada di `assets/` (pada build `--flavor beta`), sistem tetap berfungsi normal sebagai fallback.

## Files to Touch
- Create: `android/shell/app/src/main/java/io/runtimepack/host/OverlayAssetSource.kt`
- Create (Test): `android/shell/app/src/test/java/io/runtimepack/host/OverlayAssetSourceTest.kt`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`
  - Inisialisasi `OverlayAssetSource.gpackReader = gpackReader` saat `game.pack` dibuka (sekitar line 382 / 391).
  - Update `loadWebShims()` untuk menggunakan `OverlayAssetSource.read("web_shims/runtimepack_web_shims.js", this)`.
  - Pasang interceptor di `overlayWebView.webViewClient` agar request `file:///android_asset/overlay/*` dan `file:///android_asset/fonts/*` disajikan dari `OverlayAssetSource`.
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt` dan `RenpyActivity.kt`
  - Pasang `OverlayAssetSource.gpackReader = ...` dan interceptor overlay yang sama.

## Interface: `OverlayAssetSource`

```kotlin
package io.runtimepack.host

import android.content.Context
import java.io.InputStream

object OverlayAssetSource {
    @Volatile
    var gpackReader: GpackStreamReader? = null

    fun exists(path: String, context: Context? = null): Boolean {
        val clean = path.replace('\\', '/').trimStart('/')
        if (gpackReader?.hasFile(clean) == true) return true
        if (context != null) {
            try {
                context.assets.open(clean).close()
                return true
            } catch (_: Exception) {}
        }
        return false
    }

    fun open(path: String, context: Context? = null): InputStream? {
        val clean = path.replace('\\', '/').trimStart('/')
        val gpackStream = gpackReader?.openStream(clean)
        if (gpackStream != null) return gpackStream
        if (context != null) {
            try {
                return context.assets.open(clean)
            } catch (_: Exception) {}
        }
        return null
    }

    fun read(path: String, context: Context? = null): ByteArray? {
        return open(path, context)?.use { it.readBytes() }
    }

    fun guessMimeType(path: String): String {
        val lower = path.lowercase()
        return when {
            lower.endsWith(".html") || lower.endsWith(".htm") -> "text/html"
            lower.endsWith(".js") || lower.endsWith(".mjs") -> "application/javascript"
            lower.endsWith(".css") -> "text/css"
            lower.endsWith(".json") -> "application/json"
            lower.endsWith(".png") -> "image/png"
            lower.endsWith(".jpg") || lower.endsWith(".jpeg") -> "image/jpeg"
            lower.endsWith(".svg") -> "image/svg+xml"
            lower.endsWith(".ttf") -> "font/ttf"
            lower.endsWith(".woff") -> "font/woff"
            lower.endsWith(".woff2") -> "font/woff2"
            else -> "application/octet-stream"
        }
    }
}
```

## TDD Steps

- [ ] **Step 1: Write Unit Test**
Tulis `android/shell/app/src/test/java/io/runtimepack/host/OverlayAssetSourceTest.kt` (unit test JVM).
Mock atau test behavior `OverlayAssetSource`:
- `read` dan `open` dari `gpackReader` (mocking / in-memory reader atau null).
- `guessMimeType` memetakan semua ekstensi umum dengan benar.
- Fallback ke null saat file tidak ada.
Run: `cd android\shell; .\gradlew.bat :app:testDebugUnitTest --tests "*OverlayAssetSourceTest*" --console=plain` → FAIL (class belum ada).

- [ ] **Step 2: Implementasi `OverlayAssetSource.kt`**
Tulis implementasi lengkap di `android/shell/app/src/main/java/io/runtimepack/host/OverlayAssetSource.kt`.
Pastikan test `OverlayAssetSourceTest` PASS.

- [ ] **Step 3: Integrasi ke Activities & Interceptor**
- Update `WebActivity.kt`:
  - Di `loadWebShims()`: gunakan `OverlayAssetSource.read("web_shims/runtimepack_web_shims.js", this)`.
  - Di `setupOverlayWebView()`: pastikan `wv.webViewClient` meng-intercept request `file:///android_asset/overlay/*` dan `file:///android_asset/fonts/*` dengan menyajikan stream dari `OverlayAssetSource.open(path, context)`.
  - Set `OverlayAssetSource.gpackReader = gpackReader` saat pack berhasil dibuka.
- Update `RgssActivity.kt` dan `RenpyActivity.kt` dengan pola yang sama.

- [ ] **Step 4: Verifikasi Kompilasi & Unit Tests**
```powershell
cd android\shell
.\gradlew.bat :app:compileDebugKotlin :app:testDebugUnitTest --console=plain
node app/src/test/js/web_shims.test.js
```
Semua test Kotlin dan JS harus PASS 100%.

- [ ] **Step 5: Commit**
```powershell
git add android/shell/
git commit -m "feat(android-host): load overlay and shims from GPACK with plain-asset fallback"
```

## Report contract
Tulis report lengkap ke:
`D:\02_Development\Projects_2\RPGM-P\.worktrees\asset-hardening\.superpowers\sdd\2026-09-30-asset-hardening-overlay-lockdown\task-2-report.md`
Isi: file yang dibuat/diubah, hasil test unit, hasil kompilasi, commit hash.

Reply ke supervisor HANYA:
- Status: (SUCCESS / FAIL)
- Commit hash
- Test summary satu baris
- Concerns (jika ada)
