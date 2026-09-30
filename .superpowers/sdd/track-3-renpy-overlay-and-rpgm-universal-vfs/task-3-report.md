# Task 3 Report: Implement Pilar 1 — In-Memory Native VFS Bridge readdirSync

**Status:** DONE

## Overview
Implemented Pilar 1 of the Universal VFS architecture for RPG Maker Web runtimes:
1. Exposed `fileIndexMap` in `GpackStreamReader` to allow O(1) directory entry querying.
2. Maintained `WebActivity.currentInstance` and `WebActivity.gameDirectory` references for native bridge access.
3. Added `@JavascriptInterface fun readdir(relPath: String): String` to `WebBridge` in `WebBackend.kt`, parsing path prefixes and returning distinct folder contents from `gpackReader` and disk directory.
4. Upgraded `readdirSync` in `runtimepack_web_shims.js` to first query `RuntimePackNative.readdir(clean)`, falling back to `MapInfos.json` parsing for data directory file synthesis.

## Files Touched
1. `android/shell/app/src/main/java/io/runtimepack/host/GpackStreamReader.kt`
   - Exposed `val fileIndexMap: Map<String, IndexEntry>` as public.
2. `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`
   - Added `@Volatile var currentInstance: WebActivity? = null` in `companion object`.
   - Added `var gameDirectory: File? = null`.
   - Wired `currentInstance` and `gameDirectory` in `onCreate()` and cleaned up in `onDestroy()`.
3. `android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt`
   - Added `@JavascriptInterface fun readdir(relPath: String): String` to `WebBridge`.
4. `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
   - Upgraded `readdirSync` to bridge to `RuntimePackNative.readdir` with fallback to `MapInfos.json`.
5. `android/shell/app/src/test/js/web_shims.test.js`
   - Added unit test suite validating `fs.readdirSync` with mock `RuntimePackNative.readdir` for both `data` and `img/pictures`.
6. `android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt`
   - Added `testWebBridgeReaddirWithGpackReader` unit test asserting `WebBridge.readdir` behavior against real in-memory `GpackStreamReader`.

## Test Evidence
### 1. JS Failure Confirmation (Red Stage)
Command:
```powershell
node android/shell/app/src/test/js/web_shims.test.js
```
Output:
```
AssertionError [ERR_ASSERTION]: readdirSync('data') must return 3 files
0 !== 3
```

### 2. JS Pass Confirmation (Green Stage)
Command:
```powershell
node android/shell/app/src/test/js/web_shims.test.js
```
Output:
```
Running Mobile Web Shims Unit Tests...
All Mobile Web Shims tests passed successfully!
```

### 3. Android / Gradle Test Suite Pass
Command:
```powershell
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest
```
Output:
```
BUILD SUCCESSFUL in 6s
24 actionable tasks: 2 executed, 22 up-to-date
```
Unit test `io.runtimepack.host.VfsResourceInterceptorTest.testWebBridgeReaddirWithGpackReader` passed.
