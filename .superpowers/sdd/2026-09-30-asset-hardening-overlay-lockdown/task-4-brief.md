# Task 4 Brief: R8/DEX hardening + JS-bridge keep rules

**Plan:** `Docs/superpowers/plans/2026-09-30-asset-hardening-overlay-lockdown.md` (Task 4)

**Workspace:** worktree `D:\02_Development\Projects_2\RPGM-P\.worktrees\asset-hardening` (branch `feature/asset-hardening`). Windows, PowerShell 5.1.

## Goal
Mengaktifkan R8 minification / DEX hardening untuk build release (`isMinifyEnabled = true`), menyediakan file `proguard-rules.pro` yang melindungi seluruh method `@JavascriptInterface`, Activities, JNI native methods, dan komponen `RuntimeIntegrity` dari penghapusan atau penggantian nama (obfuscation breakage).

## Files to Touch
- Create: `android/shell/app/proguard-rules.pro`
- Modify: `android/shell/app/build.gradle.kts` (`release { isMinifyEnabled = true ... }`)
- Modify: `core/src/build_plan/generator.rs`:
  - Di `generate_build_gradle_kts`: set `isMinifyEnabled = true` di blok `release`.
  - Di `emit_project_with_themes`: salin atau tulis `app_dir.join("proguard-rules.pro")`.
- Test: `tests/integration/build_test.rs` — verifikasi bahwa `proguard-rules.pro` di-emit dengan keep-rules yang valid dan `build.gradle.kts` memiliki `isMinifyEnabled = true`.

## R8 / Proguard Rules (`proguard-rules.pro`)
```proguard
# Preserve all JavascriptInterface methods and their enclosing classes
-keepattributes JavascriptInterface
-keepclassmembers class * {
    @android.webkit.JavascriptInterface <methods>;
}
-keepclasseswithmembernames class * {
    @android.webkit.JavascriptInterface <methods>;
}

# Preserve Activities registered in AndroidManifest.xml
-keep public class * extends android.app.Activity

# Preserve native JNI methods (SDL, Ruby, mkxp-z, Ren'Py)
-keepclasseswithmembernames class * {
    native <methods>;
}

# Preserve RuntimeBuildInfo and RuntimeIntegrity constants & classes
-keep class io.runtimepack.host.RuntimeBuildInfo { *; }
-keep class io.runtimepack.host.RuntimeIntegrity { *; }
-keep class io.runtimepack.host.OverlayAssetSource { *; }

# Preserve Gpack models & AssetAudit serialization models
-keep class io.runtimepack.host.BuildManifestData** { *; }
-keep class io.runtimepack.host.RuntimeAudit** { *; }
```

## TDD Steps
- [ ] **Step 1: Write integration tests** in `tests/integration/build_test.rs`:
  - `test_release_project_emits_proguard_rules_and_enables_minify`:
    Generate project, assert `app/build.gradle.kts` contains `isMinifyEnabled = true`, and `app/proguard-rules.pro` exists and contains `@android.webkit.JavascriptInterface`.
  Run: `cargo test --test build_test test_release_project_emits_proguard` → FAIL.

- [ ] **Step 2: Implementasi perubahan di `core/src/build_plan/generator.rs`, `android/shell/app/build.gradle.kts`, dan `android/shell/app/proguard-rules.pro`**
  Pastikan test Rust PASS.

- [ ] **Step 3: Verifikasi Kompilasi Gradle Release**
  ```powershell
  cd android\shell
  .\gradlew.bat :app:compileReleaseKotlin :app:testDebugUnitTest --console=plain
  ```
  Expected: BUILD SUCCESSFUL.

- [ ] **Step 4: Jalankan seluruh test suite**
  ```powershell
  cargo test --workspace
  node app/src/test/js/web_shims.test.js
  ```
  Semua test harus PASS 100%.

- [ ] **Step 5: Commit**
  ```powershell
  git add core/ android/shell/ tests/
  git commit -m "build(android): enable R8 minify for release with comprehensive JS-bridge keep rules"
  ```

## Report contract
Tulis report lengkap ke:
`D:\02_Development\Projects_2\RPGM-P\.worktrees\asset-hardening\.superpowers\sdd\2026-09-30-asset-hardening-overlay-lockdown\task-4-report.md`
Isi: file yang dibuat/diubah, hasil test unit, hasil kompilasi release, commit hash.

Reply ke supervisor HANYA:
- Status: (SUCCESS / FAIL)
- Commit hash
- Test summary satu baris
- Concerns (jika ada)
