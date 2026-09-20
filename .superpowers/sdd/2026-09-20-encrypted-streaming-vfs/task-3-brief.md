# Task 3: In-Memory VFS Index & On-Demand Stream Reader (Kotlin)

## Requirements
Implement `io.runtimepack.host.GpackStreamReader` in `android/shell/app/src/main/java/io/runtimepack/host/GpackStreamReader.kt`:
1. Parse `GPACK/1` header:
   - Magic: 8 bytes `byteArrayOf('G'.code.toByte(), 'P'.code.toByte(), 'A'.code.toByte(), 'C'.code.toByte(), 'K'.code.toByte(), 0, 1, 0)`.
   - `version` (int = 1), `flags` (int), `indexOffset` (long), `indexLength` (long), `fileCount` (int).
2. Parse Index Table:
   - For each file:
     - `pathLen` (int)
     - `pathBytes` (ByteArray(pathLen))
     - `offset` (long), `size` (long), `storedSize` (long)
     - `hash` (32 bytes), `compression` (byte), `encryption` (byte).
   - Normalize path (`replace('\\', '/').trimStart('/')`). Store case-insensitive map.
3. On-demand streaming:
   - `openStream(path: String): InputStream?`:
     - Seek to `entry.offset`.
     - Read `entry.storedSize` bytes.
     - If `entry.encryption.toInt() == 1`, decrypt using `GpackCrypto.decryptChunk(key, 0, entry.index, stored)`.
     - Return `plaintext.inputStream()`.
   - `hasFile(path: String): Boolean`
4. Support both `File` / `RandomAccessFile` and `InputStream` / `AssetFileDescriptor` for reading from assets or storage.
5. Unit tests in `android/shell/app/src/test/java/io/runtimepack/host/GpackStreamReaderTest.kt`:
   - Test opening valid pack and reading unencrypted file.
   - Test opening encrypted pack and reading decrypted plaintext file.
   - Test `hasFile` case-insensitivity.
   - Test rejection of invalid magic header.
6. Run tests:
   `.\gradlew.bat testDebugUnitTest` in `android/shell`.
7. Write execution report to `.superpowers/sdd/2026-09-20-encrypted-streaming-vfs\task-3-report.md`.
