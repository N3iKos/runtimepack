# Task 5 Report: Zero-Disk Extraction Bypass & VFS Wiring in GpackExtractor

## Summary
Implemented zero-disk extraction bypass and streaming container preparation in `GpackExtractor`, wired `RgssActivity` to conditionally route between `prepareStreamingContainer` (when `mode: encrypted-streaming`) and loose file extraction, and added integration test coverage in `tests/integration/build_test.rs`.

## Changes Made
1. **`tests/integration/build_test.rs`**:
   - Added `test_build_manifest_supports_encrypted_streaming_mode`:
     - Verifies `BuildManifest` serializes `ContentManifestEntry.mode` to `"mode": "encrypted-streaming"`.
     - Verifies deserialization round-trip parses `content.mode == "encrypted-streaming"`.
     - Verifies static source contracts for `GpackExtractor.kt` (`isEncryptedStreamingMode`, `prepareStreamingContainer`) and `RgssActivity.kt` (`GpackExtractor.isEncryptedStreamingMode`, `GpackExtractor.prepareStreamingContainer`).
2. **`android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt`**:
   - Updated `isExtracted(destDir: File)` to recognize both loose `Game.ini` and single container `game.pack` alongside `.gpack_extracted` version marker.
   - Added `isEncryptedStreamingMode(context: Context): Boolean` reading `assets/build-manifest.json` to detect `"mode": "encrypted-streaming"`.
   - Added `prepareStreamingContainer(context: Context, destDir: File): Boolean` ensuring `destDir` exists, copying `assets/game.pack` as a single binary file without unpacking loose files, configuring runtime files (`runtimepack_bootstrap.rb`, `mkxp.json`), and marking `.gpack_extracted`.
3. **`android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`**:
   - Updated `onCreate` startup flow to check `GpackExtractor.isEncryptedStreamingMode(this)`:
     - When true, executes `GpackExtractor.prepareStreamingContainer(this, gameDir)` preventing loose disk extraction.
     - When false, falls back to `GpackExtractor.extract(this, gameDir)`.

## Verification Results
- Step 1 & 2: Ran `cargo test --test integration_build test_build_manifest_supports_encrypted_streaming_mode`: Failed as expected before implementation with `assertion failed: extractor_source.contains("fun isEncryptedStreamingMode")`.
- Step 3 & 4: Implemented `GpackExtractor` and `RgssActivity` streaming mode wiring.
- Step 5:
  - `cargo test --test integration_build test_build_manifest_supports_encrypted_streaming_mode`: PASSED (1/1).
  - `cargo test`: All 43 tests across workspace passed with 0 failures.
  - `.\gradlew.bat testDebugUnitTest` in `android/shell`: BUILD SUCCESSFUL; all 5 Android test suites passed with 0 failures.
