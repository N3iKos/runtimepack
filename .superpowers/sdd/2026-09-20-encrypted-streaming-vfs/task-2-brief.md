# Task 2: Android ChaCha20-Poly1305 Decryptor Core (Kotlin)

## Requirements
Implement `io.runtimepack.host.GpackCrypto` in `android/shell/app/src/main/java/io/runtimepack/host/GpackCrypto.kt`:
1. `fun derivePackageKey(packageName: String, appName: String): ByteArray`:
   - IKM: `"$packageName::$appName".toByteArray(Charsets.UTF_8)`
   - Salt: `"runtimepack_salt_v1".toByteArray(Charsets.UTF_8)`
   - Info: `"gpack-content-v1".toByteArray(Charsets.UTF_8)`
   - Use standard `javax.crypto.Mac` ("HmacSHA256") to perform HKDF-Extract and HKDF-Expand (32 bytes).
2. `fun makeNonce(fileIndex: Int, chunkIndex: Int): ByteArray`:
   - 12-byte array (96-bit nonce).
   - ByteBuffer little-endian: `fileIndex` (int), `chunkIndex` (int), `0` (int).
3. `fun decryptChunk(key: ByteArray, chunkIndex: Int, fileIndex: Int, ciphertextWithTag: ByteArray): ByteArray`:
   - Check key.size == 32 and ciphertextWithTag.size >= 16.
   - Use `Cipher.getInstance("ChaCha20-Poly1305/None/NoPadding")` (or fallback `"ChaCha20/Poly1305/NoPadding"`).
   - Initialize in `DECRYPT_MODE` with key and nonce.
   - Decrypt and return plaintext.
4. Unit tests in `android/shell/app/src/test/java/io/runtimepack/host/GpackCryptoTest.kt`:
   - Test `testDerivePackageKeyMatchesSpecification`: key size is 32, deterministic for same input.
   - Test `testDecryptTamperedChunkFails`: decrypting tampered data throws exception.
   - Test vector compatibility: verify key derived in Kotlin matches key derived in Rust for `com.example.lonarpg` / `Lona RPG`.
5. Run tests:
   `.\gradlew.bat testDebugUnitTest` in `android/shell`.
6. Write execution report to `.superpowers/sdd/2026-09-20-encrypted-streaming-vfs/task-2-report.md`.
