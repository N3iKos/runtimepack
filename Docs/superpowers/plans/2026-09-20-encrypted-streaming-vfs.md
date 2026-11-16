# Encrypted Streaming VFS Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a zero-disk encrypted streaming Virtual File System (VFS) using ChaCha20-Poly1305 AEAD that decrypts game assets on-demand directly in RAM without extracting plaintext files to device storage.

**Architecture:** The host build pipeline derives a deterministic 256-bit key per game using package-bound HKDF-SHA256 and packs assets into an authenticated `GPACK/1` binary container. At runtime, the Android host shell intercepts asset requests, decrypts chunks in-memory via ChaCha20-Poly1305, streams plaintext bytes directly to engine parsers, and guarantees strict separation between read-only VFS (`game:/`) and writable storage (`save:/`, `config:/`).

**Tech Stack:** Rust (cargo workspace: `pack`, `core`, `cli`), ChaCha20-Poly1305 AEAD, Kotlin (Android SDK API 26+), `javax.crypto.Cipher`, mkxp-z (RGSS3 / C++ / Ruby VM), Android WebView asset interceptor.

**Spec:** `Docs/superpowers/specs/2026-09-20-encrypted-streaming-vfs-design.md`

## Global Constraints
- Target minSdk: 26, targetSdk: 34.
- Cryptographic primitive: ChaCha20-Poly1305 AEAD (IETF RFC 8439), 256-bit symmetric key, 96-bit nonce, 128-bit Poly1305 authentication tag.
- Error taxonomy domains: `CNT-xxx` (Content/VFS) and `AND-xxx` (Android Runtime).
- No unpinned dependencies or floating version tags.
- Immutable source game directory (source game is never modified in-place).
- Zero broad external storage permissions (`MANAGE_EXTERNAL_STORAGE` is strictly forbidden).

---

### Task 1: Package-Bound Key Derivation & CLI Packaging Pipeline (Rust)

**Files:**
- Modify: `pack/src/crypto.rs:1-60`
- Modify: `cli/src/main.rs:420-460`
- Test: `pack/src/lib.rs`

**Interfaces:**
- Consumes: `ContentCrypto::derive_key(app_name: &str, salt: &[u8]) -> [u8; 32]` from `pack/src/crypto.rs`.
- Produces: `ContentCrypto::derive_package_key(package_name: &str, app_name: &str) -> [u8; 32]`; CLI `--content-mode encrypted-streaming` uses package-bound key derivation.

- [ ] **Step 1: Write the failing test for package-bound key derivation**

In `pack/src/lib.rs`, add a unit test asserting deterministic, domain-separated key derivation:

```rust
#[test]
fn test_derive_package_key_deterministic_and_unique() {
    let key1 = ContentCrypto::derive_package_key("com.example.lonarpg", "Lona RPG");
    let key2 = ContentCrypto::derive_package_key("com.example.lonarpg", "Lona RPG");
    let key3 = ContentCrypto::derive_package_key("com.example.othergame", "Lona RPG");
    let key4 = ContentCrypto::derive_package_key("com.example.lonarpg", "Other Game");

    assert_eq!(key1, key2, "Keys for identical package and name must be deterministic");
    assert_ne!(key1, key3, "Keys for different package names must differ");
    assert_ne!(key1, key4, "Keys for different app names must differ");
    assert_ne!(key1, [0u8; 32], "Key must not be zeroes");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p runtimepack_pack test_derive_package_key_deterministic_and_unique`
Expected: FAIL with "no function or associated item named `derive_package_key` found".

- [ ] **Step 3: Implement `derive_package_key` in `pack/src/crypto.rs`**

Add the function to `ContentCrypto`:

```rust
pub fn derive_package_key(package_name: &str, app_name: &str) -> [u8; 32] {
    let ikm = format!("{}::{}", package_name, app_name);
    let salt = b"runtimepack_salt_v1";
    let prk = hkdf::Hkdf::<sha2::Sha256>::new(Some(salt), ikm.as_bytes());
    let mut okm = [0u8; 32];
    prk.expand(b"gpack-content-v1", &mut okm)
        .expect("32 bytes is valid length for HKDF-SHA256 output");
    okm
}
```

- [ ] **Step 4: Update `cli/src/main.rs` to invoke `derive_package_key`**

In `cli/src/main.rs` (lines 428–432), replace the single-argument derivation:

```rust
    let encryption_key = if content_mode == "encrypted-streaming" {
        Some(runtimepack_pack::ContentCrypto::derive_package_key(&package_name, &app_name))
    } else {
        None
    };
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p runtimepack_pack test_derive_package_key_deterministic_and_unique`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add pack/src/crypto.rs pack/src/lib.rs cli/src/main.rs
git commit -m "feat(pack): add package-bound key derivation for encrypted-streaming content"
```

---

### Task 2: Android ChaCha20-Poly1305 Decryptor Core (Kotlin)

**Files:**
- Create: `android/shell/app/src/main/java/io/runtimepack/host/GpackCrypto.kt`
- Create: `android/shell/app/src/test/java/io/runtimepack/host/GpackCryptoTest.kt`

**Interfaces:**
- Consumes: Raw encrypted chunk bytes + 16-byte Poly1305 tag, 256-bit symmetric key, chunk index, file index.
- Produces: `GpackCrypto.decryptChunk(key: ByteArray, chunkIndex: Int, fileIndex: Int, ciphertextWithTag: ByteArray): ByteArray`.

- [ ] **Step 1: Write the failing unit test for Android ChaCha20-Poly1305 decryption**

In `android/shell/app/src/test/java/io/runtimepack/host/GpackCryptoTest.kt`:

```kotlin
package io.runtimepack.host

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class GpackCryptoTest {

    @Test
    fun testDerivePackageKeyMatchesSpecification() {
        val key = GpackCrypto.derivePackageKey("com.example.lonarpg", "Lona RPG")
        assertEquals(32, key.size)
        val key2 = GpackCrypto.derivePackageKey("com.example.lonarpg", "Lona RPG")
        assertArrayEquals(key, key2)
    }

    @Test
    fun testDecryptTamperedChunkFails() {
        val key = ByteArray(32) { 0x42 }
        val dummyCiphertextWithTag = ByteArray(48) { 0xAA.toByte() }
        assertThrows(Exception::class.java) {
            GpackCrypto.decryptChunk(key, 0, 0, dummyCiphertextWithTag)
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `.\gradlew.bat testDebugUnitTest --tests io.runtimepack.host.GpackCryptoTest`
Expected: FAIL with "Unresolved reference: GpackCrypto".

- [ ] **Step 3: Implement `GpackCrypto.kt`**

Create `android/shell/app/src/main/java/io/runtimepack/host/GpackCrypto.kt`:

```kotlin
package io.runtimepack.host

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import javax.crypto.Cipher
import javax.crypto.Mac
import javax.crypto.spec.IvParameterSpec
import javax.crypto.spec.SecretKeySpec

object GpackCrypto {

    fun derivePackageKey(packageName: String, appName: String): ByteArray {
        val ikm = "$packageName::$appName".toByteArray(Charsets.UTF_8)
        val salt = "runtimepack_salt_v1".toByteArray(Charsets.UTF_8)
        
        // HKDF-Extract using HmacSHA256
        val hmacExtract = Mac.getInstance("HmacSHA256")
        hmacExtract.init(SecretKeySpec(salt, "HmacSHA256"))
        val prk = hmacExtract.doFinal(ikm)

        // HKDF-Expand (32 bytes)
        val hmacExpand = Mac.getInstance("HmacSHA256")
        hmacExpand.init(SecretKeySpec(prk, "HmacSHA256"))
        hmacExpand.update("gpack-content-v1".toByteArray(Charsets.UTF_8))
        hmacExpand.update(0x01.toByte())
        return hmacExpand.doFinal()
    }

    fun makeNonce(fileIndex: Int, chunkIndex: Int): ByteArray {
        val nonce = ByteArray(12)
        val bb = ByteBuffer.wrap(nonce).order(ByteOrder.LITTLE_ENDIAN)
        bb.putInt(fileIndex)
        bb.putInt(chunkIndex)
        bb.putInt(0) // Session / salt counter
        return nonce
    }

    fun decryptChunk(key: ByteArray, chunkIndex: Int, fileIndex: Int, ciphertextWithTag: ByteArray): ByteArray {
        require(key.size == 32) { "Key must be 256 bits" }
        require(ciphertextWithTag.size >= 16) { "Ciphertext must contain at least 16-byte Poly1305 tag" }

        val nonce = makeNonce(fileIndex, chunkIndex)
        val cipher = try {
            Cipher.getInstance("ChaCha20-Poly1305/None/NoPadding")
        } catch (e: Exception) {
            Cipher.getInstance("ChaCha20/Poly1305/NoPadding")
        }

        val keySpec = SecretKeySpec(key, "ChaCha20")
        val ivSpec = IvParameterSpec(nonce)
        cipher.init(Cipher.DECRYPT_MODE, keySpec, ivSpec)
        return cipher.doFinal(ciphertextWithTag)
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `.\gradlew.bat testDebugUnitTest --tests io.runtimepack.host.GpackCryptoTest`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/java/io/runtimepack/host/GpackCrypto.kt android/shell/app/src/test/java/io/runtimepack/host/GpackCryptoTest.kt
git commit -m "feat(android): add GpackCrypto engine with ChaCha20-Poly1305 and HKDF key derivation"
```

---

### Task 3: In-Memory VFS Index & On-Demand Stream Reader

**Files:**
- Create: `android/shell/app/src/main/java/io/runtimepack/host/GpackStreamReader.kt`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/GpackStreamReaderTest.kt`

**Interfaces:**
- Consumes: Random-access `InputStream` or `RandomAccessFile` of `game.pack`, 256-bit encryption key.
- Produces: `GpackStreamReader.open(file: File, key: ByteArray?): GpackStreamReader`; `reader.openStream(logicalPath: String): InputStream?`; `reader.hasFile(logicalPath: String): Boolean`.

- [ ] **Step 1: Write the failing unit test for `GpackStreamReader`**

In `android/shell/app/src/test/java/io/runtimepack/host/GpackStreamReaderTest.kt`:

```kotlin
package io.runtimepack.host

import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.io.FileOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder

class GpackStreamReaderTest {

    @get:Rule
    val tempFolder = TemporaryFolder()

    @Test
    fun testParseUnencryptedHeaderAndRejectInvalidMagic() {
        val invalidFile = tempFolder.newFile("invalid.pack")
        invalidFile.writeBytes(ByteArray(64) { 0x00 })

        assertThrows(IllegalStateException::class.java) {
            GpackStreamReader.open(invalidFile, null)
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `.\gradlew.bat testDebugUnitTest --tests io.runtimepack.host.GpackStreamReaderTest`
Expected: FAIL with "Unresolved reference: GpackStreamReader".

- [ ] **Step 3: Implement `GpackStreamReader.kt`**

Create `android/shell/app/src/main/java/io/runtimepack/host/GpackStreamReader.kt`:

```kotlin
package io.runtimepack.host

import java.io.File
import java.io.InputStream
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.ByteOrder

class GpackStreamReader private constructor(
    private val raf: RandomAccessFile,
    private val key: ByteArray?,
    val isEncrypted: Boolean,
    val totalFiles: Int,
    private val fileIndexMap: Map<String, IndexEntry>
) : AutoCloseable {

    data class IndexEntry(
        val index: Int,
        val logicalPath: String,
        val offset: Long,
        val size: Long,
        val storedSize: Long,
        val encryption: Byte
    )

    companion object {
        private val GPACK_MAGIC = byteArrayOf('G'.code.toByte(), 'P'.code.toByte(), 'A'.code.toByte(), 'C'.code.toByte(), 'K'.code.toByte(), 0, 1, 0)

        fun open(packFile: File, key: ByteArray?): GpackStreamReader {
            val raf = RandomAccessFile(packFile, "r")
            try {
                val header = ByteArray(64)
                raf.readFully(header)

                for (i in 0 until 8) {
                    if (header[i] != GPACK_MAGIC[i]) {
                        throw IllegalStateException("Invalid GPACK magic header")
                    }
                }

                val headerWrap = ByteBuffer.wrap(header).order(ByteOrder.LITTLE_ENDIAN)
                headerWrap.position(8)
                val version = headerWrap.int
                check(version == 1) { "Unsupported GPACK version: $version" }

                val flags = headerWrap.int
                val isEncrypted = (flags and 1) != 0
                val totalFiles = headerWrap.int
                val indexOffset = headerWrap.long
                val indexLength = headerWrap.long

                raf.seek(indexOffset)
                val indexBytes = ByteArray(indexLength.toInt())
                raf.readFully(indexBytes)

                val indexWrap = ByteBuffer.wrap(indexBytes).order(ByteOrder.LITTLE_ENDIAN)
                val map = mutableMapOf<String, IndexEntry>()

                for (i in 0 until totalFiles) {
                    val pathLen = indexWrap.short.toInt() and 0xFFFF
                    val pathBytes = ByteArray(pathLen)
                    indexWrap.get(pathBytes)
                    val logicalPath = String(pathBytes, Charsets.UTF_8).replace('\\', '/').trimStart('/')
                    val offset = indexWrap.long
                    val size = indexWrap.long
                    val storedSize = indexWrap.long
                    val hash = ByteArray(32)
                    indexWrap.get(hash)
                    val compression = indexWrap.get()
                    val encryption = indexWrap.get()

                    val entry = IndexEntry(i, logicalPath, offset, size, storedSize, encryption)
                    map[logicalPath.lowercase()] = entry
                }

                return GpackStreamReader(raf, key, isEncrypted, totalFiles, map)
            } catch (e: Exception) {
                raf.close()
                throw e
            }
        }
    }

    fun hasFile(path: String): Boolean {
        val canonical = path.replace('\\', '/').trimStart('/').lowercase()
        return fileIndexMap.containsKey(canonical)
    }

    fun openStream(path: String): InputStream? {
        val canonical = path.replace('\\', '/').trimStart('/').lowercase()
        val entry = fileIndexMap[canonical] ?: return null

        synchronized(raf) {
            raf.seek(entry.offset)
            val stored = ByteArray(entry.storedSize.toInt())
            raf.readFully(stored)

            val plaintext = if (entry.encryption.toInt() == 1) {
                checkNotNull(key) { "Container chunk is encrypted but no key provided" }
                GpackCrypto.decryptChunk(key, 0, entry.index, stored)
            } else {
                stored
            }
            return plaintext.inputStream()
        }
    }

    override fun close() {
        raf.close()
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `.\gradlew.bat testDebugUnitTest --tests io.runtimepack.host.GpackStreamReaderTest`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/java/io/runtimepack/host/GpackStreamReader.kt android/shell/app/src/test/java/io/runtimepack/host/GpackStreamReaderTest.kt
git commit -m "feat(android): add GpackStreamReader for on-demand chunk streaming"
```

---

### Task 4: In-Memory Web VFS Stream Interceptor (MV / MZ / Tyrano)

**Files:**
- Create: `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt`

**Interfaces:**
- Consumes: `GpackStreamReader`, URL string.
- Produces: `VfsResourceInterceptor.intercept(url: String): WebResourceResponse?` (returns decrypted stream directly to WebView).

- [ ] **Step 1: Write failing unit test for URL path resolution and MIME type detection**

In `android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt`:

```kotlin
package io.runtimepack.host

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class VfsResourceInterceptorTest {

    @Test
    fun testDetectMimeTypeFromExtensions() {
        assertEquals("image/png", VfsResourceInterceptor.getMimeType("img/characters/Actor.png"))
        assertEquals("audio/ogg", VfsResourceInterceptor.getMimeType("audio/bgm/Theme.ogg"))
        assertEquals("application/javascript", VfsResourceInterceptor.getMimeType("js/rpg_core.js"))
        assertEquals("application/json", VfsResourceInterceptor.getMimeType("data/System.json"))
        assertEquals("text/html", VfsResourceInterceptor.getMimeType("index.html"))
    }

    @Test
    fun testSanitizeUrlToLogicalPath() {
        val path1 = VfsResourceInterceptor.urlToLogicalPath("https://game.local/img/characters/Actor.png")
        assertEquals("img/characters/Actor.png", path1)

        val path2 = VfsResourceInterceptor.urlToLogicalPath("file:///android_asset/game/data/System.json")
        assertEquals("data/System.json", path2)

        val external = VfsResourceInterceptor.urlToLogicalPath("https://linktr.ee/Veldra.Works")
        assertNull(external)
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `.\gradlew.bat testDebugUnitTest --tests io.runtimepack.host.VfsResourceInterceptorTest`
Expected: FAIL with "Unresolved reference: VfsResourceInterceptor".

- [ ] **Step 3: Implement `VfsResourceInterceptor.kt`**

Create `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`:

```kotlin
package io.runtimepack.host

import android.webkit.WebResourceResponse
import java.io.InputStream

class VfsResourceInterceptor(private val reader: GpackStreamReader) {

    companion object {
        fun getMimeType(path: String): String {
            val ext = path.substringAfterLast('.', "").lowercase()
            return when (ext) {
                "png" -> "image/png"
                "jpg", "jpeg" -> "image/jpeg"
                "webp" -> "image/webp"
                "ogg" -> "audio/ogg"
                "mp3" -> "audio/mpeg"
                "wav" -> "audio/wav"
                "m4a" -> "audio/mp4"
                "js" -> "application/javascript"
                "json" -> "application/json"
                "css" -> "text/css"
                "html", "htm" -> "text/html"
                else -> "application/octet-stream"
            }
        }

        fun urlToLogicalPath(url: String): String? {
            if (url.startsWith("https://game.local/")) {
                return url.removePrefix("https://game.local/").substringBefore('?').substringBefore('#')
            }
            if (url.startsWith("file:///android_asset/game/")) {
                return url.removePrefix("file:///android_asset/game/").substringBefore('?').substringBefore('#')
            }
            return null
        }
    }

    fun intercept(url: String): WebResourceResponse? {
        val logicalPath = urlToLogicalPath(url) ?: return null
        val stream = reader.openStream(logicalPath) ?: return null
        val mime = getMimeType(logicalPath)
        return WebResourceResponse(mime, "UTF-8", stream)
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `.\gradlew.bat testDebugUnitTest --tests io.runtimepack.host.VfsResourceInterceptorTest`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt
git commit -m "feat(web): add VfsResourceInterceptor for on-demand streaming to WebView"
```

---

### Task 5: Zero-Disk Extraction Bypass & VFS Wiring in `GpackExtractor`

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt:150-250`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt:420-450`
- Test: `tests/integration/build_test.rs`

**Interfaces:**
- Consumes: `game.pack` in `assets/`, `BuildManifest.content.mode`.
- Produces: If `content.mode == "encrypted-streaming"`, skip plaintext file extraction, keep `game.pack` intact in app storage, and initialize `GpackStreamReader`.

- [ ] **Step 1: Write integration test verifying manifest declares encrypted-streaming content mode**

In `tests/integration/build_test.rs`, add:

```rust
#[test]
fn test_build_manifest_supports_encrypted_streaming_mode() {
    let manifest = BuildManifest {
        schema_version: "1.0.0".to_string(),
        app: AppMetadata {
            id: "com.example.testvfs".to_string(),
            name: "Test VFS".to_string(),
            package: "com.example.testvfs".to_string(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineMetadata {
            family: "RGSS".to_string(),
            variant: "VX_Ace".to_string(),
            version: Some("RGSS3".to_string()),
            confidence: "EXACT".to_string(),
        },
        runtime: RuntimeResolution {
            family: "RGSS".to_string(),
            backend: "mkxp-z".to_string(),
            version: "2.4.2".to_string(),
            abi: "arm64-v8a".to_string(),
        },
        content: ContentPackaging {
            mode: "encrypted-streaming".to_string(),
            container_path: "assets/game.pack".to_string(),
            hash: "dummy_hash".to_string(),
            total_bytes: 1024,
            file_count: 1,
        },
        compatibility: CompatibilityPlan {
            level: "COMPATIBLE".to_string(),
            active_shims: vec![],
            diagnostics: vec![],
        },
        android: AndroidTarget {
            min_sdk: 26,
            target_sdk: 34,
            compile_sdk: 34,
            permissions: vec![],
            screen_orientation: "sensorLandscape".to_string(),
        },
        provenance: BuildProvenance {
            build_timestamp: "2026-09-20T00:00:00Z".to_string(),
            builder_version: "0.1.0".to_string(),
            toolchain: ToolchainProvenance {
                agp_version: None,
                gradle_version: None,
                jdk_version: None,
                android_sdk_platform: None,
                ndk_version: None,
            },
        },
    };

    let serialized = serde_json::to_string_pretty(&manifest).unwrap();
    assert!(serialized.contains("\"mode\": \"encrypted-streaming\""));
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test --test integration_build test_build_manifest_supports_encrypted_streaming_mode`
Expected: PASS.

- [ ] **Step 3: Update `GpackExtractor.kt` to support encrypted streaming mode**

In `android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt`:

```kotlin
    fun isEncryptedStreamingMode(context: Context): Boolean {
        return try {
            val manifestJson = context.assets.open("build-manifest.json").bufferedReader().use { it.readText() }
            manifestJson.contains("\"mode\": \"encrypted-streaming\"") || manifestJson.contains("\"mode\":\"encrypted-streaming\"")
        } catch (e: Exception) {
            false
        }
    }

    fun prepareStreamingContainer(context: Context, destDir: File): Boolean {
        destDir.mkdirs()
        val targetPack = File(destDir, "game.pack")
        if (!targetPack.exists()) {
            context.assets.open("game.pack").use { input ->
                FileOutputStream(targetPack).use { output ->
                    input.copyTo(output)
                }
            }
        }
        installRuntimeFiles(context, destDir)
        File(destDir, ".gpack_extracted").writeText(EXTRACTION_VERSION)
        return true
    }
```

In `RgssActivity.kt`, check `GpackExtractor.isEncryptedStreamingMode(this)`: if true, call `prepareStreamingContainer` (which preserves the single `game.pack` file and avoids loose file extraction).

- [ ] **Step 4: Run unit tests**

Run: `.\gradlew.bat testDebugUnitTest` and `cargo test`
Expected: ALL PASS.

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt tests/integration/build_test.rs
git commit -m "feat(vfs): add zero-disk streaming bypass to GpackExtractor and RgssActivity"
```

---

### Task 6: Full Multi-Engine Build Verification & Test Corpus Check

**Files:**
- Execute: `cargo run --bin runtimepack -- build "Raw-Game\LONA-RPG-B.0.10.8.05" -o dist/lona --package com.example.lonarpg --name "Lona RPG" --content-mode encrypted-streaming`
- Test: Verify `dist/lona/LonaRPG.apk` exists and hash matches.

- [ ] **Step 1: Execute `runtimepack build` in `encrypted-streaming` mode**

Run:
`cargo run --bin runtimepack -- build "Raw-Game\LONA-RPG-B.0.10.8.05" -o dist/lona --package com.example.lonarpg --name "Lona RPG" --content-mode encrypted-streaming`
Expected: Build succeeds with Exit 0; `game.pack` is packed with ChaCha20-Poly1305 encryption.

- [ ] **Step 2: Install and verify on MuMu Player**

Run:
```powershell
adb -s 127.0.0.1:16384 install -r dist/lona/LonaRPG.apk
adb -s 127.0.0.1:16384 shell pm clear com.example.lonarpg
adb -s 127.0.0.1:16384 shell am start -n com.example.lonarpg/io.runtimepack.host.RgssActivity
```
Verify: Title screen boots smoothly, storage space is ~780 MB (no loose files on disk).

- [ ] **Step 3: Document findings in `Docs/test_ports/rpgvxa_lonarpg/report.md`**

Record test results, memory usage, and encryption integrity report.

- [ ] **Step 4: Commit**

```bash
git add Docs/test_ports/rpgvxa_lonarpg/report.md
git commit -m "docs: record encrypted-streaming VFS verification results for Lona RPG"
```
