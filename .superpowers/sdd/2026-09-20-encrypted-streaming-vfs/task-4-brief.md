# Task 4: In-Memory Web VFS Stream Interceptor (MV / MZ / Tyrano)

## Requirements
Implement `io.runtimepack.host.VfsResourceInterceptor` in `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`:
1. MIME type resolver `getMimeType(path: String): String`:
   - png -> image/png, jpg/jpeg -> image/jpeg, webp -> image/webp
   - ogg -> audio/ogg, mp3 -> audio/mpeg, wav -> audio/wav, m4a -> audio/mp4
   - js -> application/javascript, json -> application/json, css -> text/css, html/htm -> text/html
   - default: application/octet-stream.
2. URL to logical path mapping `urlToLogicalPath(url: String): String?`:
   - `https://game.local/*` -> strip prefix, query parameters (`?`), hash (`#`).
   - `file:///android_asset/game/*` -> strip prefix, query parameters, hash.
   - `http://localhost:*/*` -> handle local origin URLs.
   - External URLs (e.g. `https://linktr.ee/Veldra.Works`) -> return null (pass through untouched).
3. Interception method:
   `fun intercept(url: String): WebResourceResponse?`:
   - Map URL to logical path.
   - Query `reader.openStream(logicalPath)`.
   - If stream exists, wrap with `WebResourceResponse(mime, "UTF-8", stream)`.
   - If stream does not exist or URL is external, return null.
4. Unit tests in `android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt`:
   - Test MIME type detection for all supported game assets.
   - Test URL sanitization and prefix stripping.
   - Test mock stream interception round-trip.
5. Run tests:
   `.\gradlew.bat testDebugUnitTest` in `android/shell`.
6. Write execution report to `.superpowers/sdd/2026-09-20-encrypted-streaming-vfs/task-4-report.md`.
