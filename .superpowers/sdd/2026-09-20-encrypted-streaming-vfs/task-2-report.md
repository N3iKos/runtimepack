# Task 2 Report: Android ChaCha20-Poly1305 Decryptor Core (Kotlin)

## Summary
Implemented `io.runtimepack.host.GpackCrypto` in Kotlin providing package-bound HKDF-SHA256 key derivation, 96-bit little-endian nonce generation, and ChaCha20-Poly1305 AEAD decryption and encryption. Full vector verification matches the Rust implementation for `com.example.lonarpg` / `Lona RPG`.

## Changes Made
1. **`android/shell/app/src/test/java/io/runtimepack/host/GpackCryptoTest.kt`**:
   - `testDerivePackageKeyMatchesSpecification`: Verifies 32-byte output length, determinism for identical input, and domain-separated key differentiation across packages and app names.
   - `testDerivePackageKeyHkdfVector`: Validates hex vector `f966700e5e7bc80873900e2c2db5d08d464a34d92485397b3ba2b28cffa1006a` for `("com.example.lonarpg", "Lona RPG")`.
   - `testMakeNonceSpecification`: Verifies little-endian layout (`fileIndex` [0..3], `chunkIndex` [4..7], `0` [8..11]).
   - `testEncryptDecryptRoundtrip`: Verifies roundtrip encryption and decryption with ChaCha20-Poly1305.
   - `testDecryptTamperedChunkFails`: Verifies decryption failure on corrupted ciphertext/tag.
   - `testDecryptValidation`: Verifies validation constraints (key length must be 32, ciphertext must contain at least 16-byte Poly1305 tag).
2. **`android/shell/app/src/main/java/io/runtimepack/host/GpackCrypto.kt`**:
   - Implemented `derivePackageKey(packageName, appName)`: HKDF-Extract using `HmacSHA256` with salt `"runtimepack_salt_v1"`, HKDF-Expand with info `"gpack-content-v1"`.
   - Implemented `makeNonce(fileIndex, chunkIndex)`: 12-byte little-endian `ByteBuffer` containing `fileIndex`, `chunkIndex`, and counter `0`.
   - Implemented `decryptChunk(key, chunkIndex, fileIndex, ciphertextWithTag)`: Initializes `Cipher` with `ChaCha20` `SecretKeySpec` and 12-byte `IvParameterSpec(nonce)` in `DECRYPT_MODE`, verifying Poly1305 integrity.
   - Implemented `encryptChunk(key, chunkIndex, fileIndex, plaintext)`: Symmetric AEAD encryption supporting in-memory container authoring and testing.

## Verification Results
- Initial TDD run failed as expected prior to implementation: `Unresolved reference: GpackCrypto`.
- `.\gradlew.bat testDebugUnitTest --tests io.runtimepack.host.GpackCryptoTest`: 6 passed, 0 failed.
- `.\gradlew.bat testDebugUnitTest`: All test suites passed (`GpackCryptoTest`, `KeyMappingTest`, `CheatBridgeTest`).
