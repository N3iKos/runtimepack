# Asset Hardening & Overlay Lockdown — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Menghilangkan layout/overlay/shim JS terbuka dari APK (masuk ke `game.pack` terenkripsi), mengikat integritasnya dengan hash startup (hilang/berubah = aplikasi menolak jalan dengan kode diagnostik), dan mengeraskan DEX Kotlin — aktif penuh hanya pada flavor `release`, flavor `beta` tetap plain untuk kemudahan debugging.

**Architecture:** Tiga lapis. (B) Packer Rust memasukkan `overlay/`, `web_shims/`, dan `fonts/` ke dalam container GPACK; host Android memuat overlay dari GPACK via `VfsResourceInterceptor`/bridge yang sudah ada, bukan lagi dari `assets/` fisik — pada flavor release file-file itu DIHAPUS dari `assets/` sehingga tidak ada salinan polos di APK. (C) Build menghitung SHA-256 keseluruhan payload runtime (overlay+shim+fonts dari container) lalu menyuntikkannya ke generated Kotlin; startup memverifikasi sebelum WebView apa pun dimuat; mismatch → layar "RUNTIME CORRUPTED" + kode `AND-xxx` + exit rapi. (D) `isMinifyEnabled = true` untuk release build + aturan keep R8 yang aman untuk `@JavascriptInterface`.

**Tech Stack:** Rust (packer `pack/`, generator `core/src/build_plan/`, CLI `cli/`), Kotlin (host Android), SHA-256 (infrastruktur GPACK yang sudah ada — tidak ada primitif kripto baru), Gradle/R8.

**Spec:** Diskusi desain dengan user 2026-09-30 (B+C+D, performa dianalisis: dampak nyata hanya +0.3-1s startup), `AGENTS.md` §17 (istilah "asset hardening", bukan janji mustahil) & §5 (tidak boleh mengarang primitif kripto — pakai implementasi ter-audit yang sudah ada), `memory/decisions_log.md` [2026-09-30] #5-#7.

## Keputusan desain yang sudah disepakati (binding)

1. **Flavor split**: `beta` = overlay & shim tetap file plain di `assets/` (debug mudah, tanpa integrity gate keras — hanya INFO log). `release` = overlay/shim/fonts HANYA di GPACK terenkripsi + integrity gate keras + R8 minify.
2. **Perilaku integrity gagal (release)**: BUKAN crash keras — tampilkan layar error deterministik `RUNTIME_CORRUPTED` + kode `AND-401` + tombol keluar; proses berhenti rapi. Log `[RuntimePack] AND-401` di logcat. (Crash keras menyulitkan diagnosis field.)
3. **Scope D**: ikut — `isMinifyEnabled=true` release only, dengan keep-rules untuk semua `@JavascriptInterface` + kelas yang diakses refleksi (kalau ada).
4. **Semua game otomatis**: ini perubahan pipeline + template host — setiap `runtimepack build` per game apapun mendapat proteksi yang sama tanpa langkah per game.
5. **BatAS jujur (untuk dokumentasi)**: perangkat root penuh + ahli tetap bisa menangkap konten dari memori runtime; hardening menaikkan biaya, bukan kebal absolut (AGENTS.md §17: sebut "asset hardening"/"encrypted content protection").

## Global Constraints

- **Input game immutable** — `Raw-Game/` tidak boleh tersentuh.
- **Tidak ada primitif kripto baru** — SHA-256 & AEAD pakai implementasi yang sudah ada di GPACK/`pack/` (ter-audit); larangan AGENTS.md §17.
- **Zero-dependency JS** tetap; overlay tidak boleh tahu dia sedang di-serve dari GPACK (transparan).
- **Kode diagnostik stabil**: `AND-401` runtime integrity mismatch, `AND-402` overlay entry hilang dari container, `AND-403` GPACK unreadable (reuse jika sudah ada kode serupa — cek dulu).
- **Build command baku**: `cargo run -q --bin runtimepack -- build "D:\02_Development\Projects_2\RPGM-P\Raw-Game\Forestia1.4.4-IND" --flavor beta|release --output dist` (package default `io.runtimepack.game.forestia144ind`).
- **Test commands baku**: `cargo test --workspace`; `node android/shell/app/src/test/js/web_shims.test.js`; `cd android\shell; gradlew.bat :app:testDebugUnitTest :app:compileReleaseKotlin --console=plain`.
- Workspace: worktree `.worktrees/native-mv-mz-save` SETELAH branch save/load di-merge ke master (prasyarat; jalankan `finishing-a-development-branch` dulu). Semua task di plan ini dijalankan pada branch baru `feature/asset-hardening` dari master yang sudah up-to-date.
- Executor harus selalu verifikasi `dist\build-manifest.json` (flavor, package) sebelum menyimpulkan hasil build.

---

## File Structure

| File | Aksi | Tanggung Jawab |
| :--- | :--- | :--- |
| `pack/src/gpack.rs` / `core/src/build_plan/generator.rs` (atau tempat packer mengumpulkan file container — verifikasi lokasi aktual) | Modify | Menambahkan `overlay/*`, `web_shims/*`, `fonts/*` ke daftar entry GPACK saat flavor release; beta tetap copy ke assets/ |
| `core/src/build_plan/generator.rs` | Modify | Emit flag `runtimeIntegrity` (sha256) ke build-manifest + generated config untuk Kotlin |
| `android/shell/app/src/main/assets/` | Modify (build-time) | Release: TIDAK menulis overlay/web_shims/fonts (hanya game.pack); Beta: seperti sekarang |
| `android/shell/.../host/RuntimeIntegrity.kt` | Create | Hash verifikasi startup + layar RUNTIME_CORRUPTED |
| `android/shell/.../host/WebActivity.kt`, `RgssActivity.kt`, `RenpyActivity.kt` (+ base activity jika ada) | Modify | Muat overlay dari GPACK saat plain file tidak ada (bridge `loadOverlayAsset(path): ByteArray?`); panggil gate integrity di `onCreate` sebelum UI |
| `android/shell/.../host/WebBackend.kt` / `VfsResourceInterceptor.kt` | Modify | Interceptor menyajikan `file:///android_asset/overlay/*` & `web_shims/*` dari GPACK reader |
| `android/shell/app/build.gradle.kts` + `proguard-rules.pro` | Modify | R8 untuk release: minify + keep `@JavascriptInterface`, JNI, SDL bridge |
| `cli/src/main.rs` | Modify (kecil) | Print status hardening saat build release |
| `android/shell/app/src/test/java/io/runtimepack/host/RuntimeIntegrityTest.kt` | Create | Test unit hash & keputusan gate |
| `tests/integration/build_test.rs` | Modify | Test bahwa build release TIDAK mengandung `assets/overlay/index.html` di APK & beta tetap mengandung |

---

### Task 0: PRASYARAT — Merge branch save/load

- [ ] Jalankan `finishing-a-development-branch` untuk `feature/native-mv-mz-save` (17 commit, final review APPROVED_FOR_MERGE) → merge ke master.
- [ ] Buat worktree baru `feature/asset-hardening` dari master ter-update.
- [ ] Catat BASE commit untuk ledger SDD plan ini.

### Task 1: Packer — overlay/shim/fonts masuk GPACK (flavor release)

**Files:**
- Modify: lokasi packer yang mengumpulkan file untuk container (verifikasi: `pack/src/gpack.rs`, `core/src/build_plan/generator.rs` — cari tempat `game.pack` entry list dibangun; ikuti pola yang ada)
- Modify: `cli/src/main.rs` (hanya jika perlu meneruskan flavor ke packer — cek: flavor sudah sampai generator?)

**Interfaces:**
- Produces: build release menghasilkan `game.pack` berisi entry `overlay/index.html`, `overlay/vpad.js`, `overlay/vpad.css`, `overlay/cheat_menu.js`, `overlay/cheat_engine.js`, `overlay/cheat_profile.js`, `overlay/diagnostic_suite.js`, `overlay/theme_loader.js`, `web_shims/runtimepack_web_shims.js`, `web_shims/runtimepack_asset_audit.js`, `fonts/*` — dengan prefix path persis seperti yang akan diminta WebView (`overlay/...`, `web_shims/...`, `fonts/...`).
- Produces: build release TIDAK menyalin folder `assets/overlay`, `assets/web_shims`, `assets/fonts` ke generated Android project. Build beta TIDAK berubah (plain, seperti sekarang).

- [ ] **Step 1: Tulis failing test integration** di `tests/integration/build_test.rs` (ikuti gaya test build yang ada):
  1. `release_build_packs_overlay_into_gpack` — build fixture game dengan `--flavor release` → buka `game.pack` (via reader yang ada di test/util Rust) → assert entry `overlay/index.html` ADA.
  2. `release_build_omits_plain_overlay_assets` — pada generated project release → assert `app/src/main/assets/overlay/` TIDAK ada (folder tidak dibuat).
  3. `beta_build_keeps_plain_overlay` — build `--flavor beta` → assert `assets/overlay/index.html` ADA (regresi: beta tidak boleh berubah).
  Run: `cargo test --test build_test` → 3 test baru FAIL.
- [ ] **Step 2: Implement di packer/generator** — saat flavor release: tambahkan sumber file overlay (dari template `android/shell/app/src/main/assets/` di repo) ke entry list GPACK; skip copy plain ke generated assets. Beta: jalur lama.
- [ ] **Step 3: `cargo test --workspace`** semua hijau (termasuk 355 test lama).
- [ ] **Step 4: Commit** `feat(pack): bundle overlay, web shims, and fonts into GPACK for release flavor`

### Task 2: Host — muat overlay dari GPACK (fallback ke plain assets)

**Files:**
- Create: `android/shell/app/src/main/java/io/runtimepack/host/OverlayAssetSource.kt`
- Modify: `WebActivity.kt`, `RgssActivity.kt`, `RenpyActivity.kt` (+ base activity umum jika ada — verifikasi) — tempat `wv.loadUrl("file:///android_asset/overlay/index.html")` dan `webShimsScript` dimuat
- Modify: `VfsResourceInterceptor.kt` / `WebBackend.kt` — interceptor `shouldInterceptRequest` untuk prefix `overlay/`, `web_shims/`, `fonts/` saat file plain tidak ada

**Interfaces:**
- Produces: `OverlayAssetSource` — API tunggal:
  ```kotlin
  object OverlayAssetSource {
      /** null = sumber tidak tersedia (harus fallback plain) */
      fun read(path: String): ByteArray?          // "overlay/index.html", "web_shims/runtimepack_web_shims.js", "fonts/xxx.ttf"
      fun exists(path: String): Boolean
  }
  ```
  Implementasi: cek GPACK reader (via `ContentService`/`GpackStreamReader` yang sudah ada — ikuti pola `game/` yang sudah ter-intercept di `VfsResourceInterceptor.kt:117`); inject reader saat host membuka `game.pack` (pola `WebActivity.kt:360-394`).
- Overlay WebView (LOADING): dua jalur — (a) `WebViewAssetLoader`/`shouldInterceptRequest` menyajikan bytes dari `OverlayAssetSource` untuk URL `file:///android_asset/overlay/*` & `fonts/*`; (b) fallback: `loadDataWithBaseURL` jika interceptor route tidak memungkinkan (pilih yang paling minim diff; interceptor sudah ada di WebActivity line ~509/524 — reuse).
- Shim injection: `WebActivity.webShimsScript` saat ini dibaca dari `assets/web_shims/...` — tambah fallback `OverlayAssetSource.read("web_shims/runtimepack_web_shims.js")` baca dari GPACK.
- Transparansi: tidak ada perubahan JS overlay — dia tetap meminta URL yang sama.

- [ ] **Step 1: TDD** — `OverlayAssetSourceTest.kt` (JVM, mock reader): `read` mengembalikan bytes entry; `read` entry tidak ada → null; `exists` benar. Failing dulu → implement → `gradlew.bat :app:testDebugUnitTest` hijau.
- [ ] **Step 2: Integrasi ke 3 activity + interceptor** (beta path TIDAK berubah perilaku — plain assets tetap dipakai bila ada; GPACK dipakai bila plain tidak ada).
- [ ] **Step 3: Test wiring**: unit test kecil bahwa fallback bekerja (activity-level cukup di-mock; device test menyusul Task 4).
- [ ] **Step 4: `gradlew.bat :app:compileDebugKotlin :app:testDebugUnitTest`** hijau.
- [ ] **Step 5: Commit** `feat(android-host): load overlay and shims from GPACK with plain-asset fallback`

### Task 3: Integrity gate — SHA-256 startup + RUNTIME_CORRUPTED

**Files:**
- Create: `android/shell/app/src/main/java/io/runtimepack/host/RuntimeIntegrity.kt`
- Create: `android/shell/app/src/main/res/layout/activity_corrupted.xml` (sederhana: TextView pesan + tombol exit)
- Modify: generator build (`core/src/build_plan/generator.rs`) — hitung SHA-256 gabungan (urutan entry tetap: sorted paths; hash = SHA256 dari `path\0bytes\0` beruntun — definisi deterministik, dokumentasikan di KDoc & generator) untuk `overlay/*`,`web_shims/*`,`fonts/*` DARI CONTAINER yang barusan di-pack (release) atau dari plain assets (beta) → tulis ke generated `BuildConfig`-style file (mis. `RuntimeBuildInfo.kt` dengan `val RUNTIME_INTEGRITY_SHA256: String`).
- Modify: 3 Activity `onCreate` (paling atas, sebelum setContentView/WebView) — beta: hitung & log INFO saja. Release: hitung → bandingkan → mismatch tampil `activity_corrupted` + log `AND-401` + `finish()`.

**Interfaces:**
- Produces: `RuntimeIntegrity.verify(context): IntegrityResult` dengan `sealed IntegrityResult { Ok(hash), Mismatch(expected, actual), Unreadable(code) }`. Entry GPACK hilang → `Unreadable("AND-402")`; GPACK tak terbuka → `Unreadable("AND-403")`.
- Gate hanya aktif `release`; beta hanya log (`[RuntimePack] integrity=OK sha=...` atau mismatch WARNING).

- [ ] **Step 1: TDD** — `RuntimeIntegrityTest.kt`: hash deterministik sama untuk input sama; beda 1 byte → mismatch; entry hilang → AND-402; reader null → AND-403. Failing → implement (SHA-256 pakai `java.security.MessageDigest` — ter-audit, bukan primitif baru).
- [ ] **Step 2: Generator Rust** — emit `RuntimeBuildInfo.kt` berisi hash (release & beta sama-sama di-emit supaya beta bisa log; nilai dihitung dari sumber aktual per flavor).
- [ ] **Step 3: Integrasi gate di 3 Activity + layout corrupted + tombol exit.**
- [ ] **Step 4: Semua test hijau** (`cargo test --workspace` + `gradlew` unit).
- [ ] **Step 5: Commit** `feat(runtime): startup integrity gate with AND-40x codes and RUNTIME_CORRUPTED screen (release-enforced)`

### Task 4: R8/DEX hardening (release only)

**Files:**
- Modify: `android/shell/app/build.gradle.kts` (`buildTypes.release`: `isMinifyEnabled = true`, `isShrinkResources = true`)
- Create/Modify: `android/shell/app/proguard-rules.pro` — keep rules:

```proguard
# JavaScript bridge — every class with @JavascriptInterface must survive
-keepclassmembers class * {
    @android.webkit.JavascriptInterface <methods>;
}
-keepclasseswithmembernames class * {
    @android.webkit.JavascriptInterface <methods>;
}
# Activities referenced from AndroidManifest (generated) — keep names
-keep public class * extends android.app.Activity
# SDL / native bridge (RGSS runtime, libruby, OpenAL) — JNI entry points
-keep class io.runtimepack.** { native <methods>; }
-keepclasseswithmembernames class * { native <methods>; }
# WebView JS-evaluated class names must not be renamed (overlay references)
-keep class io.runtimepack.host.RuntimePackNative { *; }
-keep class io.runtimepack.host.**$*Bridge* { *; }
```
(Executor WAJIB verifikasi nama kelas aktual via grep `@JavascriptInterface` dan daftar kelas yang direferensi dari JS overlay/shim (`RuntimePackNative`, `RuntimePackVPad`, `RuntimePackAssetAudit`, dll — kumpulkan dari `addJavascriptInterface` calls) lalu tulis keep rules sesuai nama nyata — jangan pakai nama tebakan dari template di atas.)

- [ ] **Step 1: Kumpulkan daftar kelas bridge nyata** — grep `addJavascriptInterface` di host → daftar kelas + nama JS-nya.
- [ ] **Step 2: Aktifkan minify + tulis proguard-rules.pro sesuai daftar nyata.**
- [ ] **Step 3: Build release fixture** → `gradlew.bat :app:assembleRelease` sukses; buka APK (unzip) → assert `classes.dex` menyusut signifikan vs debug; assert `assets/overlay/` TIDAK ada (regresi Task 1).
- [ ] **Step 4: Test device** (butuh emulator — lihat Task 5).
- [ ] **Step 5: Commit** `build(android): enable R8 minify + resource shrink for release with JS-bridge keep rules`

### Task 5: Verifikasi end-to-end di device (qa-tester)

- [ ] **Step 1: Build release + beta Forestia.**
- [ ] **Step 2: Inspeksi APK release (host-side, tanpa emulator)**: unzip → TIDAK ada `assets/overlay/`, `assets/web_shims/`, `assets/fonts/`; `game.pack` ada; `classes.dex` minified.
- [ ] **Step 3: Install release di MuMu → launch → overlay vpad TAMPIL & berfungsi (tap tombol A respons) → diagnostic overlay jalan → game play normal → probe T1: `runtimepack probe <release-apk> --device mumu` = PASS.**
- [ ] **Step 4: Test negatif integrity**: modif `game.pack` (flip 1 byte via util/host) → install → launch → muncul layar RUNTIME_CORRUPTED + `AND-401` di logcat + app exit rapi.
- [ ] **Step 5: Install beta → semua seperti sekarang (overlay plain, tanpa gate keras, load game tetap PASS) — regresi penuh.**
- [ ] **Step 6: Laporan QA + update ledger.**

### Task 6: Review akhir + dokumentasi + penutupan

- [ ] **Step 1: Final whole-branch review (code-reviewer)** — fokus: kebenaran keep-rules R8 (risk terbesar), keamanan integrity gate (tidak bisa dilewati dengan gampang — misal gate harus jalan SEBELUM WebView memuat apa pun), tidak adanya dead code jalur beta.
- [ ] **Step 2: Update `Docs/superpowers/compat/COMPAT-REGISTRY.md`** — entri baru bukan CAP-WEB (bukan shim JS) melainkan section "Runtime Hardening": dokumentasikan mekanisme, kode AND-40x, definisi hash, dan batas jujur (rooted device).
- [ ] **Step 3: Update memory** (`progress.md`, `decisions_log.md`, `technical_findings.md`).
- [ ] **Step 4: Presentasi hasil ke user → keputusan merge via `finishing-a-development-branch`.**

---

## Urutan Eksekusi & Dispatch Map

| Task | Sub-agent role | Butuh emulator? |
| :--- | :--- | :--- |
| 0 (merge + worktree) | supervisor (bukan sub-agent — koordinasi git) | ❌ |
| 1 (packer) | executor (Rust TDD) | ❌ |
| 2 (host loader) | executor (Kotlin TDD) | ❌ |
| 3 (integrity gate) | executor (Kotlin+Rust TDD) | ❌ |
| 4 (R8) | executor (verifikasi nama bridge via grep dulu) | ❌ (assembleRelease lokal) |
| 5 (E2E device) | qa-tester | ✅ |
| 6 (review+docs) | code-reviewer + supervisor | ❌ |

Review task tetap via code-reviewer per task (SDD loop standar), final review di Task 6.

## Self-Review

1. **Coverage**: B = Task 1+2; C = Task 3; D = Task 4; verifikasi = Task 5; keputusan desain (beta-plain/release-hardened, exit rapi AND-40x, semua game otomatis) tercakup. Prasyarat merge di Task 0. ✅
2. **Placeholder**: dua titik bergantung verifikasi nama/lokasi aktual (entry-list packer, nama kelas bridge) — keduanya diberi instruksi grep/verify eksplisit sebelum menulis kode, sesuai aturan anti-halusinasi. Template proguard diberi disclaimer verifikasi. ✅
3. **Konsistensi**: `OverlayAssetSource.read/exists` dipakai Task 2 & 3; `RuntimeBuildInfo.RUNTIME_INTEGRITY_SHA256` dari generator (Task 3) dikonsumsi gate; kode `AND-401/402/403` konsisten Task 3, 5, 6. Hash definition `SHA256(sorted path\0bytes\0)` didefinisikan sekali di Task 3 dan dirujuk Task 6 dokumentasi. ✅
4. **Risiko terbesar**: R8 memutus bridge JS (mitigasi: Task 4 Step 1 kumpulkan nama nyata + Task 5 verifikasi device); overlay WebView gagal load dari GPACK (mitigasi: fallback plain dulu di Task 2, device test Task 5 Step 3). ✅
