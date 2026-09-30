# Task 3 Brief: Integrity gate — SHA-256 startup verification + RUNTIME_CORRUPTED

**Plan:** `Docs/superpowers/plans/2026-09-30-asset-hardening-overlay-lockdown.md` (Task 3)

**Workspace:** worktree `D:\02_Development\Projects_2\RPGM-P\.worktrees\asset-hardening` (branch `feature/asset-hardening`). Windows, PowerShell 5.1.

## Goal
Mengimplementasikan gate verifikasi integritas startup:
1. **Build generator (`core/src/build_plan/generator.rs`)**: Menghitung SHA-256 gabungan deterministik dari seluruh file `overlay/*`, `web_shims/*`, dan `fonts/*`, lalu menuliskannya ke dalam generated Kotlin file `RuntimeBuildInfo.kt` (dan manifest).
2. **Kotlin host (`RuntimeIntegrity.kt`)**: Saat startup (`onCreate`), verifikasi hash file-file tersebut.
   - Pada `release`: Jika hash mismatch atau hilang, tampilkan layar deterministik "RUNTIME CORRUPTED" dengan kode `AND-401`, log `[RuntimePack] AND-401`, dan tombol Exit yang menghentikan aplikasi secara rapi.
   - Pada `beta`: Hanya log INFO `[RuntimePack] integrity=...` tanpa memblokir game.

## Definisi Hash SHA-256 Deterministik
Daftar path file `overlay/*`, `web_shims/*`, `fonts/*` diurutkan secara alfabetis (case-insensitive / canonical lowercase).
Untuk setiap path:
- Update SHA-256 dengan bytes dari `path` (UTF-8)
- Update byte `0x00`
- Update bytes dari isi file
- Update byte `0x00`
Output: hex lowercase string 64 karakter (e.g. `a1b2c3...`).

## Files to Touch
- Create: `android/shell/app/src/main/java/io/runtimepack/host/RuntimeBuildInfo.kt` (template default di source tree)
- Create: `android/shell/app/src/main/java/io/runtimepack/host/RuntimeIntegrity.kt`
- Create: `android/shell/app/src/main/res/layout/activity_corrupted.xml`
- Create (Test): `android/shell/app/src/test/java/io/runtimepack/host/RuntimeIntegrityTest.kt`
- Modify: `core/src/build_plan/generator.rs` — hitung SHA-256 saat build release (atau beta) dan tulis `RuntimeBuildInfo.kt` ke generated project `src/main/java/io/runtimepack/host/RuntimeBuildInfo.kt`.
- Modify: `WebActivity.kt`, `RgssActivity.kt`, `RenpyActivity.kt` — panggil `if (!RuntimeIntegrity.checkOrShowCorrupted(this)) return` di awal `onCreate`.

## Interface: `RuntimeIntegrity`

```kotlin
package io.runtimepack.host

import android.app.Activity
import android.content.Context
import android.util.Log
import java.security.MessageDigest

sealed class IntegrityResult {
    data class Ok(val hash: String) : IntegrityResult()
    data class Mismatch(val expected: String, val actual: String) : IntegrityResult()
    data class Unreadable(val code: String, val detail: String) : IntegrityResult()
}

object RuntimeIntegrity {
    private const val TAG = "RuntimeIntegrity"

    fun computeHash(pathsAndBytes: List<Pair<String, ByteArray>>): String {
        val sorted = pathsAndBytes.sortedBy { it.first.lowercase() }
        val md = MessageDigest.getInstance("SHA-256")
        for ((path, bytes) in sorted) {
            val normPath = path.replace('\\', '/').trimStart('/').lowercase()
            md.update(normPath.toByteArray(Charsets.UTF_8))
            md.update(0.toByte())
            md.update(bytes)
            md.update(0.toByte())
        }
        return md.digest().joinToString("") { "%02x".format(it) }
    }

    fun verify(context: Context): IntegrityResult {
        val expected = RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256
        val flavor = RuntimeBuildInfo.BUILD_FLAVOR

        // Kumpulkan file overlay, web_shims, fonts yang ada
        val files = mutableListOf<Pair<String, ByteArray>>()
        val reader = OverlayAssetSource.gpackReader

        if (reader != null) {
            // Baca dari GPACK
            for (key in reader.fileIndexMap.keys) {
                if (key.startsWith("overlay/") || key.startsWith("web_shims/") || key.startsWith("fonts/")) {
                    val bytes = OverlayAssetSource.read(key, context)
                    if (bytes != null) {
                        files.add(key to bytes)
                    }
                }
            }
        } else {
            // Fallback plain assets jika gpack belum/tidak dibuka
            // (biasanya pada unit test atau beta)
        }

        if (files.isEmpty() && flavor == "release") {
            return IntegrityResult.Unreadable("AND-402", "No runtime overlay assets found in container")
        }

        val actual = computeHash(files)

        if (flavor != "release") {
            Log.i(TAG, "[RuntimePack] (Beta) Runtime integrity: $actual (expected: $expected)")
            return IntegrityResult.Ok(actual)
        }

        if (expected.isNotEmpty() && !actual.equals(expected, ignoreCase = true)) {
            Log.e(TAG, "[RuntimePack] AND-401 Runtime integrity mismatch: expected $expected, got $actual")
            return IntegrityResult.Mismatch(expected, actual)
        }

        return IntegrityResult.Ok(actual)
    }

    fun checkOrShowCorrupted(activity: Activity): Boolean {
        val result = verify(activity)
        return when (result) {
            is IntegrityResult.Ok -> true
            is IntegrityResult.Mismatch, is IntegrityResult.Unreadable -> {
                val code = if (result is IntegrityResult.Unreadable) result.code else "AND-401"
                val detail = if (result is IntegrityResult.Mismatch) "Hash mismatch" else (result as IntegrityResult.Unreadable).detail
                showCorruptedScreen(activity, code, detail)
                false
            }
        }
    }

    private fun showCorruptedScreen(activity: Activity, code: String, detail: String) {
        activity.runOnUiThread {
            try {
                activity.setContentView(R.layout.activity_corrupted)
                // Binding view atau fallback programmatic jika layout gagal
                val tvCode = activity.findViewById<android.widget.TextView>(R.id.tv_error_code)
                tvCode?.text = "ERROR: $code ($detail)"
                val btnExit = activity.findViewById<android.widget.Button>(R.id.btn_exit)
                btnExit?.setOnClickListener {
                    activity.finishAffinity()
                    kotlin.system.exitProcess(1)
                }
            } catch (e: Exception) {
                Log.e(TAG, "Failed to inflate activity_corrupted: ${e.message}", e)
                activity.finishAffinity()
                kotlin.system.exitProcess(1)
            }
        }
    }
}
```

## Template `RuntimeBuildInfo.kt` (di template shell)
```kotlin
package io.runtimepack.host

object RuntimeBuildInfo {
    const val RUNTIME_INTEGRITY_SHA256: String = ""
    const val BUILD_FLAVOR: String = "beta"
}
```

## Layout `activity_corrupted.xml`
Buat layout sederhana latar belakang gelap (`#121212`), teks merah `"APPLICATION CORRUPTED"`, teks putih `tv_error_code` `"AND-401"`, dan tombol `btn_exit` `"Exit"`.

## Generator Rust (`core/src/build_plan/generator.rs`)
Hitung SHA-256 dengan algoritma yang sama dari file-file yang di-pack/di-copy ke overlay/web_shims/fonts, dan emit file `src/main/java/io/runtimepack/host/RuntimeBuildInfo.kt` ke dalam generated project!
Pastikan algoritma hashing di Rust dan di Kotlin menghasilkan string yang persis sama.

## TDD Steps
- [ ] **Step 1: Write Unit Test**
`RuntimeIntegrityTest.kt` menguji `computeHash` dengan pasangan test vectors, `verify` dengan mock mismatch (return Mismatch), dan `verify` saat flavor "beta" (return Ok).
Run: `cd android\shell; .\gradlew.bat :app:testDebugUnitTest --tests "*RuntimeIntegrityTest*" --console=plain` → FAIL.

- [ ] **Step 2: Implementasi `RuntimeIntegrity.kt` & `activity_corrupted.xml`**
Pastikan test unit Kotlin PASS.

- [ ] **Step 3: Update `core/src/build_plan/generator.rs`**
Implementasikan emisi `RuntimeBuildInfo.kt` dengan hash SHA-256 yang dihitung.
Tulis unit test Rust di `tests/integration/build_test.rs` memverifikasi file `RuntimeBuildInfo.kt` di-emit dengan hash dan flavor yang sesuai.

- [ ] **Step 4: Integrasi ke Activities**
Tambahkan `if (!RuntimeIntegrity.checkOrShowCorrupted(this)) return` di awal `onCreate` `WebActivity.kt`, `RgssActivity.kt`, dan `RenpyActivity.kt`.

- [ ] **Step 5: Verifikasi Semua Test**
```powershell
cargo test --workspace
cd android\shell; .\gradlew.bat :app:compileDebugKotlin :app:testDebugUnitTest --console=plain
node app/src/test/js/web_shims.test.js
```
Semua test harus PASS 100%.

- [ ] **Step 6: Commit**
```powershell
git add core/ android/shell/ tests/
git commit -m "feat(runtime): startup integrity gate with AND-40x codes and RUNTIME_CORRUPTED screen (release-enforced)"
```

## Report contract
Tulis report lengkap ke:
`D:\02_Development\Projects_2\RPGM-P\.worktrees\asset-hardening\.superpowers\sdd\2026-09-30-asset-hardening-overlay-lockdown\task-3-report.md`
Isi: file yang dibuat/diubah, hasil test unit, hasil kompilasi, commit hash.

Reply ke supervisor HANYA:
- Status: (SUCCESS / FAIL)
- Commit hash
- Test summary satu baris
- Concerns (jika ada)
