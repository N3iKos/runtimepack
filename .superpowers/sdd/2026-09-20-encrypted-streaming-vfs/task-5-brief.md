# Task 5: Zero-Disk Extraction Bypass & VFS Wiring in GpackExtractor

## Requirements
1. In `tests/integration/build_test.rs`:
   - Add unit test `test_build_manifest_supports_encrypted_streaming_mode` verifying that BuildManifest serialization and deserialization support `"mode": "encrypted-streaming"`.
2. In `android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt`:
   - Add `isEncryptedStreamingMode(context: Context): Boolean`:
     Checks if `assets/build-manifest.json` contains `"mode": "encrypted-streaming"` (or `"mode":"encrypted-streaming"`).
   - Add `prepareStreamingContainer(context: Context, destDir: File): Boolean`:
     Ensures `destDir` exists, copies `assets/game.pack` as a single binary if not present, installs runtime files (`runtimepack_bootstrap.rb`, `mkxp.json`), and writes `.gpack_extracted`.
3. In `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`:
   - In `onCreate`, check:
     ```kotlin
     if (GpackExtractor.isEncryptedStreamingMode(this)) {
         val success = GpackExtractor.prepareStreamingContainer(this, gameDir)
         if (!success) Log.e(TAG, "Streaming container preparation failed!")
     } else {
         val success = GpackExtractor.extract(this, gameDir)
         if (!success) Log.e(TAG, "Extraction failed!")
     }
     ```
4. Unit tests:
   - Run `cargo test` to verify Rust tests pass.
   - Run `.\gradlew.bat testDebugUnitTest` in `android/shell` to verify all Android tests pass.
5. Write execution report to `.superpowers/sdd/2026-09-20-encrypted-streaming-vfs/task-5-report.md`.
