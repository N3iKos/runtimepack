# Task 2 Execution Report: Host — OverlayAssetSource to load overlay/shims from GPACK

**Plan:** `Docs/superpowers/plans/2026-09-30-asset-hardening-overlay-lockdown.md` (Task 2)
**Brief:** `task-2-brief.md`
**Branch:** `feature/asset-hardening`
**Commit Hash:** `f6185e961d14d046fe361de0e84583d7e168c629`
**Status:** SUCCESS

---

## 1. Files Created and Modified

### Created:
- `android/shell/app/src/main/java/io/runtimepack/host/OverlayAssetSource.kt`:
  - Singleton `OverlayAssetSource` with thread-safe `@Volatile var gpackReader: GpackStreamReader?`.
  - Methods `exists(path, context)`, `open(path, context)`, `read(path, context)`.
  - Extension mime-type resolver `guessMimeType(path)` handling `.html`, `.htm`, `.js`, `.mjs`, `.css`, `.json`, `.png`, `.jpg`, `.jpeg`, `.svg`, `.ttf`, `.woff`, `.woff2`, and binary fallback.
  - Interceptor method `shouldIntercept(url, context)` and `createOverlayWebViewClient(context)` for intercepting `file:///android_asset/overlay/*` and `file:///android_asset/fonts/*` requests.
- `android/shell/app/src/test/java/io/runtimepack/host/OverlayAssetSourceTest.kt`:
  - Unit tests covering MIME type mapping, null reader fallback, in-memory GPACK reading/opening with normalization, and `file:///android_asset/` request interception.

### Modified:
- `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`:
  - Assigned `OverlayAssetSource.gpackReader = gpackReader` upon opening `game.pack`.
  - Intercepts overlay and fonts requests via `OverlayAssetSource.createOverlayWebViewClient(this)`.
  - Updated `loadWebShims()` to read `runtimepack_web_shims.js` and `runtimepack_asset_audit.js` through `OverlayAssetSource.read(...)`.
  - Cleans up `OverlayAssetSource.gpackReader = null` in `onDestroy()`.
- `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`:
  - Added `initOverlayAssetSource(gameDir)` to initialize reader from `game.pack`.
  - Configured `overlayWebView.webViewClient` using `OverlayAssetSource.createOverlayWebViewClient(this)`.
  - Added reader cleanup in `onDestroy()`.
- `android/shell/app/src/main/java/io/runtimepack/host/RenpyActivity.kt`:
  - Added `initOverlayAssetSource()` to initialize reader from `game.pack`.
  - Configured `overlayWebView.webViewClient` using `OverlayAssetSource.createOverlayWebViewClient(this)`.
  - Added reader cleanup in `onDestroy()`.

---

## 2. Compilation and Test Results

### Kotlin Compilation & Unit Tests:
```powershell
cd android\shell
.\gradlew.bat :app:compileDebugKotlin :app:testDebugUnitTest --console=plain
```
**Result:** BUILD SUCCESSFUL.
- All test suites passed:
  - `OverlayAssetSourceTest`: 4/4 passed (100%)
  - `VfsResourceInterceptorTest`: passed
  - `RenpyActivityTest`: passed
  - `NativeSaveRepositoryTest`: passed
  - `KeyMappingTest`: passed
  - `GpackStreamReaderTest`: passed
  - `GpackExtractorTest`: passed
  - `GpackCryptoTest`: passed
  - `CheatBridgeTest`: passed
  - `AssetProbeRunnerTest`: passed
  - `AssetAuditServiceTest`: passed
  - `AssetAuditBridgeTest`: passed

### JavaScript Shims Test:
```powershell
cd android\shell
node app/src/test/js/web_shims.test.js
```
**Result:** 100% PASSED (All 10 test suites passed successfully).

---

## 3. Ready for QA
The changes strictly adhere to TDD Steps 1-5 and are ready for Task 3 or audit by `qa-tester`.
