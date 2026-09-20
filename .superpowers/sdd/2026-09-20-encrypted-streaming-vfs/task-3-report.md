# Task 3 Report: In-Memory VFS Index & On-Demand Stream Reader (Kotlin)

## Summary
Implemented `io.runtimepack.host.GpackStreamReader` in Kotlin providing parsing of GPACK/1 binary container headers and index tables, path normalization with case-insensitive lookup, and on-demand streaming decryption using `GpackCrypto.decryptChunk`. The reader supports random-access reading from files, `RandomAccessFile`, `InputStream`, and `AssetFileDescriptor`.

## Changes Made
1. **`android/shell/app/src/test/java/io/runtimepack/host/GpackStreamReaderTest.kt`**:
   - `testRejectInvalidMagicHeader`: Verifies rejection with `IllegalStateException` on corrupt or non-GPACK headers.
   - `testRejectUnsupportedVersion`: Verifies rejection with `IllegalStateException` on non-v1 container versions.
   - `testOpenUnencryptedPackAndReadFiles`: Verifies reading unencrypted files and confirms case-insensitive path querying (`Data/System.json`, `data\system.json`, `/DATA/SYSTEM.JSON`).
   - `testOpenEncryptedPackAndReadDecryptedFiles`: Verifies on-demand AEAD decryption using package-derived key for encrypted chunks.
   - `testEncryptedPackFailsWithoutKey`: Verifies explicit failure if encrypted files are accessed without providing an encryption key.
   - `testSupportRandomAccessFileAndInputStreamSources`: Verifies reading via `RandomAccessFile` and `InputStream` byte buffering.
2. **`android/shell/app/src/main/java/io/runtimepack/host/GpackStreamReader.kt`**:
   - Parsed 64-byte `GPACK/1` header: 8-byte magic (`GPACK\0\1\0` and `GPACK1\0\0`), format version, flags (`isEncrypted`), index offset, index length, and total file count.
   - Parsed binary index table: `pathLen`, UTF-8 path, offset, uncompressed size, stored size, SHA-256 hash, compression flag, and encryption flag.
   - Implemented path normalization (`replace('\\', '/').trimStart('/')`) and case-insensitive index map (`lowercase()`).
   - Implemented `openStream(logicalPath: String): InputStream?`: Seeks to stored offset, reads payload into buffer, conditionally decrypts via `GpackCrypto.decryptChunk(key, 0, entry.index, stored)` if marked encrypted, and returns `ByteArrayInputStream`.
   - Supported multiple source backends: `File`, `RandomAccessFile`, `InputStream`, and `AssetFileDescriptor` through `SeekableSource` abstraction.

## Verification Results
- Initial TDD run failed as expected: `Unresolved reference: GpackStreamReader`.
- `.\gradlew.bat testDebugUnitTest --rerun-tasks`: 24 tasks executed, BUILD SUCCESSFUL.
- All 6 tests in `io.runtimepack.host.GpackStreamReaderTest` passed (100% success rate). All existing test suites (`GpackCryptoTest`, `KeyMappingTest`, `CheatBridgeTest`) passed.
