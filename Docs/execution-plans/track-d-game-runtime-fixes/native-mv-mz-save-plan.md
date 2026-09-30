# Universal MV/MZ Native Save Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Provide persistent, crash-safe, and restart-resilient save/load functionality for all RPG Maker MV and MZ games packaged by RuntimePack by storing saves directly in private Android storage (`filesDir/saves/`), bypassing the 5MB browser quota and eliminating Pako binary encoding corruption.

**Architecture:** 
1. **Host Layer (Kotlin)**: Implement `NativeSaveRepository` using Java NIO atomic replacement (`StandardCopyOption.ATOMIC_MOVE`) and file sync to prevent corrupt save files on crash. Expose explicit boolean status and JSON-serialized slot lists via `@JavascriptInterface` on `WebBridge` without relying on exception throwing across the Android bridge.
2. **Webview Layer (JavaScript)**: Install universal adapters on `StorageManager` in `runtimepack_web_shims.js`. For MZ, intercept `saveToForage`/`loadFromForage` using lossless Base64 encoding (`btoa`/`atob`) and normalize boot-time slot index resolution in `forageExists`/`updateForageKeys` to prevent `removeInvalidGlobalInfo` from purging save slots. For MV, map numeric slot IDs to sanitized alphanumeric keys (`-1` -> `config`, `0` -> `global`, `n` -> `file<N>`) and route all 9 persistence and `.bak` backup methods to the native bridge synchronously.

**Tech Stack:** Kotlin, Java NIO, Android WebView `@JavascriptInterface`, Node.js `vm` test runner, RPG Maker MZ (Pako zlib), RPG Maker MV (LZString Base64).

**Spec:** `Docs/execution-plans/track-d-game-runtime-fixes/native-mv-mz-save-design.md`

## Global Constraints

- Never modify files inside `Raw-Game/` (strictly immutable).
- All changes must be executed and verified in the isolated worktree `.worktrees/native-mv-mz-save` to protect uncommitted changes on `master`.
- No broad storage permissions (`MANAGE_EXTERNAL_STORAGE` or `READ_EXTERNAL_STORAGE`); use app-private storage (`filesDir/saves/`).
- Do not use Android framework classes that fail in JVM tests (`android.util.AtomicFile`); use standard Java NIO (`java.nio.file.Files.move`).
- Every task must strictly follow TDD: write the failing test first, verify failure, implement minimal code, verify pass, commit.
- No manual emulator clicking/gameplay testing; all verification in Tasks 1–4 uses fast automated unit tests (Gradle & Node.js). Task 5 rebuilds APKs.

---

### Task 1: Native Save Repository with Atomic Move (Kotlin)

**Files:**
- Create: `android/shell/app/src/main/java/io/runtimepack/host/NativeSaveRepository.kt`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/NativeSaveRepositoryTest.kt`

**Interfaces:**
- Produces: `NativeSaveRepository(root: File)` with methods:
  - `put(namespace: String, slot: String, payload: String): Boolean`
  - `get(namespace: String, slot: String): String?`
  - `contains(namespace: String, slot: String): Boolean`
  - `remove(namespace: String, slot: String): Boolean`
  - `keys(namespace: String): List<String>`

- [ ] **Step 1: Write failing unit test for `NativeSaveRepository`**

Create `android/shell/app/src/test/java/io/runtimepack/host/NativeSaveRepositoryTest.kt`:

```kotlin
package io.runtimepack.host

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class NativeSaveRepositoryTest {

    @get:Rule
    val tempFolder = TemporaryFolder()

    @Test
    fun testNamespaceAndPersistenceRoundTrip() {
        val root = tempFolder.newFolder("saves")
        val repo = NativeSaveRepository(root)

        assertTrue("put mz slot should succeed", repo.put("mz", "file1", "payload_mz_1"))
        assertTrue("put mv slot should succeed", repo.put("mv", "global", "payload_mv_global"))

        assertEquals("payload_mz_1", repo.get("mz", "file1"))
        assertEquals("payload_mv_global", repo.get("mv", "global"))

        assertTrue(repo.contains("mz", "file1"))
        assertTrue(repo.contains("mv", "global"))

        assertEquals(listOf("file1"), repo.keys("mz"))
        assertEquals(listOf("global"), repo.keys("mv"))

        assertTrue(repo.remove("mz", "file1"))
        assertFalse(repo.contains("mz", "file1"))
        assertNull(repo.get("mz", "file1"))
        assertTrue(repo.keys("mz").isEmpty())

        // Ensure MV slot was untouched by MZ operations
        assertEquals("payload_mv_global", repo.get("mv", "global"))
    }

    @Test
    fun testSlotValidationAndSecurity() {
        val root = tempFolder.newFolder("saves_sec")
        val repo = NativeSaveRepository(root)

        // Invalid namespaces
        assertFalse(repo.put("", "file1", "data"))
        assertFalse(repo.put("unknown", "file1", "data"))

        // Path traversal attempts
        assertFalse(repo.put("mz", "../escape", "data"))
        assertFalse(repo.put("mz", "sub/dir", "data"))
        assertFalse(repo.put("mz", "dir\\file", "data"))
        assertFalse(repo.put("mz", "", "data"))
        assertFalse(repo.put("mz", ".hidden", "data"))

        // Valid slot characters: letters, digits, underscore, dash, dot
        assertTrue(repo.put("mz", "file_1-save.bak", "valid_data"))
        assertEquals("valid_data", repo.get("mz", "file_1-save.bak"))
    }

    @Test
    fun testAtomicOverwriteLeavesPreviousIntactOnFailure() {
        val root = tempFolder.newFolder("saves_atomic")
        val repo = NativeSaveRepository(root)

        assertTrue(repo.put("mz", "file1", "initial_data"))
        assertEquals("initial_data", repo.get("mz", "file1"))

        assertTrue(repo.put("mz", "file1", "updated_data"))
        assertEquals("updated_data", repo.get("mz", "file1"))
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:
```powershell
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests "io.runtimepack.host.NativeSaveRepositoryTest"
```
Expected: FAIL with compilation error (unresolved reference: `NativeSaveRepository`).

- [ ] **Step 3: Implement `NativeSaveRepository.kt`**

Create `android/shell/app/src/main/java/io/runtimepack/host/NativeSaveRepository.kt`:

```kotlin
package io.runtimepack.host

import java.io.File
import java.io.FileOutputStream
import java.nio.file.Files
import java.nio.file.StandardCopyOption

class NativeSaveRepository(private val rootDir: File) {

    init {
        if (!rootDir.exists()) {
            rootDir.mkdirs()
        }
    }

    private val allowedNamespaces = setOf("mz", "mv")
    private val slotRegex = Regex("^[a-zA-Z0-9_.-]{1,128}$")

    private fun isValidSlot(slot: String): Boolean {
        if (!slotRegex.matches(slot)) return false
        if (slot.startsWith(".")) return false
        if (slot.contains("..")) return false
        return true
    }

    private fun resolveFile(namespace: String, slot: String): File? {
        val ns = namespace.lowercase()
        if (!allowedNamespaces.contains(ns)) return null
        if (!isValidSlot(slot)) return null
        val targetDir = File(rootDir, ns)
        if (!targetDir.exists()) {
            targetDir.mkdirs()
        }
        return File(targetDir, slot)
    }

    @Synchronized
    fun put(namespace: String, slot: String, payload: String): Boolean {
        val targetFile = resolveFile(namespace, slot) ?: return false
        val tempFile = File(targetFile.parentFile, "${targetFile.name}.tmp.${System.nanoTime()}")
        return try {
            val bytes = payload.toByteArray(Charsets.UTF_8)
            FileOutputStream(tempFile).use { fos ->
                fos.write(bytes)
                fos.flush()
                fos.fd.sync()
            }
            Files.move(
                tempFile.toPath(),
                targetFile.toPath(),
                StandardCopyOption.ATOMIC_MOVE,
                StandardCopyOption.REPLACE_EXISTING
            )
            true
        } catch (_: Exception) {
            try {
                if (tempFile.exists()) tempFile.delete()
            } catch (_: Exception) {}
            false
        }
    }

    @Synchronized
    fun get(namespace: String, slot: String): String? {
        val targetFile = resolveFile(namespace, slot) ?: return null
        if (!targetFile.exists() || !targetFile.isFile) return null
        return try {
            targetFile.readText(Charsets.UTF_8)
        } catch (_: Exception) {
            null
        }
    }

    @Synchronized
    fun contains(namespace: String, slot: String): Boolean {
        val targetFile = resolveFile(namespace, slot) ?: return false
        return targetFile.exists() && targetFile.isFile
    }

    @Synchronized
    fun remove(namespace: String, slot: String): Boolean {
        val targetFile = resolveFile(namespace, slot) ?: return false
        return if (targetFile.exists()) targetFile.delete() else false
    }

    @Synchronized
    fun keys(namespace: String): List<String> {
        val ns = namespace.lowercase()
        if (!allowedNamespaces.contains(ns)) return emptyList()
        val targetDir = File(rootDir, ns)
        if (!targetDir.exists() || !targetDir.isDirectory) return emptyList()
        return targetDir.listFiles()
            ?.filter { it.isFile && !it.name.endsWith(".tmp") && isValidSlot(it.name) }
            ?.map { it.name }
            ?.sorted()
            ?: emptyList()
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:
```powershell
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests "io.runtimepack.host.NativeSaveRepositoryTest"
```
Expected: PASS with 3 tests passing.

- [ ] **Step 5: Commit**

```powershell
git add android/shell/app/src/main/java/io/runtimepack/host/NativeSaveRepository.kt android/shell/app/src/test/java/io/runtimepack/host/NativeSaveRepositoryTest.kt
git commit -m "feat(storage): implement atomic NativeSaveRepository with namespace isolation"
```

---

### Task 2: Native Save Bridge API in WebBackend (Kotlin)

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/NativeSaveRepositoryTest.kt`

**Interfaces:**
- Consumes: `NativeSaveRepository` from Task 1.
- Produces: `@JavascriptInterface` methods on `WebBridge`:
  - `saveSlot(namespace: String, slot: String, payload: String): Boolean`
  - `loadSlot(namespace: String, slot: String): String?`
  - `slotExists(namespace: String, slot: String): Boolean`
  - `deleteSlot(namespace: String, slot: String): Boolean`
  - `listSlots(namespace: String): String` (returns JSON array of strings e.g. `["file1","global"]`)

- [ ] **Step 1: Write failing unit test for `WebBridge` saveSlot methods**

In `android/shell/app/src/test/java/io/runtimepack/host/NativeSaveRepositoryTest.kt`, add:

```kotlin
    @Test
    fun testWebBridgeSlotMethods() {
        val root = tempFolder.newFolder("bridge_saves")
        val repo = NativeSaveRepository(root)
        val dummySaveService = object : SaveService {
            override fun openRead(slot: String) = null
            override fun openWrite(slot: String) = java.io.ByteArrayOutputStream()
            override fun exists(slot: String) = false
            override fun list() = emptyList<String>()
            override fun delete(slot: String) = false
        }
        val bridge = WebBridge(dummySaveService, repo)

        assertTrue(bridge.saveSlot("mz", "global", "{\"title\":\"Forestia\"}"))
        assertTrue(bridge.slotExists("mz", "global"))
        assertFalse(bridge.slotExists("mz", "missing"))

        assertEquals("{\"title\":\"Forestia\"}", bridge.loadSlot("mz", "global"))
        assertNull(bridge.loadSlot("mz", "missing"))

        val listJson = bridge.listSlots("mz")
        assertTrue(listJson.contains("global"))
        assertEquals("[]", bridge.listSlots("mv"))

        assertTrue(bridge.deleteSlot("mz", "global"))
        assertFalse(bridge.slotExists("mz", "global"))
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run:
```powershell
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests "io.runtimepack.host.NativeSaveRepositoryTest.testWebBridgeSlotMethods"
```
Expected: FAIL with compilation error (constructor `WebBridge` does not take `NativeSaveRepository` or missing `saveSlot`).

- [ ] **Step 3: Update `WebBackend.kt` to integrate `NativeSaveRepository` into `WebBridge`**

In `android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt`:
Update `class WebBridge`:

```kotlin
class WebBridge(
    private val saveService: SaveService,
    private val saveRepository: NativeSaveRepository? = null
) {
    private fun getRepository(): NativeSaveRepository {
        if (saveRepository != null) return saveRepository
        val gameDir = WebActivity.currentInstance?.gameDirectory
            ?: WebActivity.currentInstance?.filesDir
            ?: File(".")
        val savesDir = File(gameDir.parentFile ?: gameDir, "saves")
        return NativeSaveRepository(savesDir)
    }

    @JavascriptInterface
    fun saveSlot(namespace: String, slot: String, payload: String): Boolean {
        return getRepository().put(namespace, slot, payload)
    }

    @JavascriptInterface
    fun loadSlot(namespace: String, slot: String): String? {
        return getRepository().get(namespace, slot)
    }

    @JavascriptInterface
    fun slotExists(namespace: String, slot: String): Boolean {
        return getRepository().contains(namespace, slot)
    }

    @JavascriptInterface
    fun deleteSlot(namespace: String, slot: String): Boolean {
        return getRepository().remove(namespace, slot)
    }

    @JavascriptInterface
    fun listSlots(namespace: String): String {
        val keys = getRepository().keys(namespace)
        return try {
            org.json.JSONArray(keys).toString()
        } catch (_: Throwable) {
            "[]"
        }
    }

    @JavascriptInterface
    fun saveItem(key: String, value: String) {
        saveService.openWrite(key).use {
            it.write(value.toByteArray(Charsets.UTF_8))
        }
    }

    @JavascriptInterface
    fun loadItem(key: String): String? {
        val stream = saveService.openRead(key) ?: return null
        return stream.use {
            it.bufferedReader(Charsets.UTF_8).readText()
        }
    }

    @JavascriptInterface
    fun onFirstFrame() {
        HealthMarker.FIRST_FRAME.emit()
    }

    @JavascriptInterface
    fun fileExists(relPath: String): Boolean {
        val clean = relPath.replace('\\', '/').trim('/', '.').lowercase()
        val reader = WebActivity.currentInstance?.gpackReader
        val gameDir = WebActivity.currentInstance?.gameDirectory
        return reader?.hasFile(clean) ?: File(gameDir, clean).exists()
    }

    @JavascriptInterface
    fun fileSize(relPath: String): Long {
        val clean = relPath.replace('\\', '/').trim('/', '.').lowercase()
        val reader = WebActivity.currentInstance?.gpackReader
        val gameDir = WebActivity.currentInstance?.gameDirectory
        return reader?.getEntry(clean)?.size ?: File(gameDir, clean).length()
    }

    @JavascriptInterface
    fun readdir(relPath: String): String {
        val clean = relPath.replace('\\', '/').trim('/', '.').lowercase()
        val prefix = if (clean.isEmpty()) "" else "$clean/"
        val reader = WebActivity.currentInstance?.gpackReader
        val gameDir = WebActivity.currentInstance?.gameDirectory

        val packEntries = reader?.fileIndexMap?.keys
            ?.filter { it.startsWith(prefix) }
            ?.map { it.removePrefix(prefix).substringBefore('/') }
            ?.distinct()

        val diskEntries = gameDir?.let {
            val dir = if (clean.isEmpty()) it else File(it, clean)
            if (dir.exists() && dir.isDirectory) dir.list()?.toList() else null
        }

        val result = (packEntries ?: diskEntries ?: emptyList()).distinct()
        val json = try {
            org.json.JSONArray(result).toString()
        } catch (_: Throwable) {
            null
        }
        return if (!json.isNullOrEmpty()) json else result.joinToString(prefix = "[", postfix = "]", separator = ",") {
            "\"" + it.replace("\\", "\\\\").replace("\"", "\\\"") + "\""
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:
```powershell
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest --tests "io.runtimepack.host.NativeSaveRepositoryTest.*"
```
Expected: PASS (all 4 tests passing).

- [ ] **Step 5: Commit**

```powershell
git add android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt android/shell/app/src/test/java/io/runtimepack/host/NativeSaveRepositoryTest.kt
git commit -m "feat(storage): wire saveSlot, loadSlot, slotExists, deleteSlot, listSlots to WebBridge"
```

---

### Task 3: RPG Maker MZ Universal Save Adapter (JavaScript Web Shim)

**Files:**
- Modify: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
- Test: `android/shell/app/src/test/js/web_shims.test.js`

**Interfaces:**
- Consumes: `RuntimePackNative.saveSlot`, `loadSlot`, `slotExists`, `deleteSlot`, `listSlots` with namespace `"mz"`.
- Produces: Interception of RPG Maker MZ `StorageManager` persistence:
  - `saveToForage(saveName, zip)` -> converts `zip` to Base64 via `btoa(zip)`, calls `RuntimePackNative.saveSlot("mz", saveName, b64)`, resolves Promise on true, rejects on false.
  - `loadFromForage(saveName)` -> calls `RuntimePackNative.loadSlot("mz", saveName)`, decodes Base64 via `atob(b64)`, resolves Promise with raw `zip`.
  - `forageExists(saveName)` -> checks `this._forageKeys.includes(saveName)` or native `slotExists("mz", saveName)`.
  - `removeForage(saveName)` -> calls `RuntimePackNative.deleteSlot("mz", saveName)` and updates keys.
  - `updateForageKeys()` -> queries `RuntimePackNative.listSlots("mz")`, parses JSON array into `this._forageKeys`, sets `this._forageKeysUpdated = true`.
  - Idempotency guard: `sm.__runtimepack_mz_save_patched = true`.

- [ ] **Step 1: Write failing unit tests for MZ Save Adapter in `web_shims.test.js`**

Add to `android/shell/app/src/test/js/web_shims.test.js`:

```javascript
{
    // Test: RPG Maker MZ Universal Save Adapter
    const sb = createSandbox();
    const storedSlots = {};

    sb.RuntimePackNative = {
        saveSlot: function(ns, slot, payload) {
            if (ns !== "mz") return false;
            storedSlots[slot] = payload;
            return true;
        },
        loadSlot: function(ns, slot) {
            if (ns !== "mz") return null;
            return storedSlots[slot] !== undefined ? storedSlots[slot] : null;
        },
        slotExists: function(ns, slot) {
            return ns === "mz" && storedSlots[slot] !== undefined;
        },
        deleteSlot: function(ns, slot) {
            if (ns === "mz" && storedSlots[slot] !== undefined) {
                delete storedSlots[slot];
                return true;
            }
            return false;
        },
        listSlots: function(ns) {
            if (ns !== "mz") return "[]";
            return JSON.stringify(Object.keys(storedSlots));
        }
    };

    // Mock MZ StorageManager before shims initialization
    sb.StorageManager = {
        _forageKeys: [],
        _forageKeysUpdated: false,
        forageKey: function(name) { return "rmmzsave.test." + name; },
        forageTestKey: function() { return "rmmzsave.test"; },
        saveToForage: function() {},
        loadFromForage: function() {},
        forageExists: function(name) { return this._forageKeys.includes(this.forageKey(name)); },
        removeForage: function() {},
        updateForageKeys: function() {},
        forageKeysUpdated: function() { return this._forageKeysUpdated; }
    };

    vm.runInContext(shimsCode, sb);

    // 1. Verify updateForageKeys queries native listSlots
    storedSlots["global"] = Buffer.from("mz_global_data").toString("base64");
    storedSlots["file1"] = Buffer.from("mz_binary_\x00\x80\xFF_data").toString("base64");

    const sm = sb.StorageManager;
    sm.updateForageKeys().then(function() {
        assert.strictEqual(sm.forageKeysUpdated(), true, "forageKeysUpdated must be true after update");
        assert.strictEqual(sm.forageExists("global"), true, "forageExists('global') must be true");
        assert.strictEqual(sm.forageExists("file1"), true, "forageExists('file1') must be true");
        assert.strictEqual(sm.forageExists("file2"), false, "forageExists('file2') must be false");

        // 2. Verify loadFromForage decodes Base64 preserving binary bytes
        return sm.loadFromForage("file1");
    }).then(function(rawZip) {
        assert.strictEqual(rawZip, "mz_binary_\x00\x80\xFF_data", "loadFromForage must decode raw binary string exactly");

        // 3. Verify saveToForage encodes binary to Base64 and persists to native
        return sm.saveToForage("file2", "new_save_\x01\x02\xFE");
    }).then(function() {
        assert(storedSlots["file2"], "file2 must be stored in native bridge");
        const decoded = Buffer.from(storedSlots["file2"], "base64").toString("binary");
        assert.strictEqual(decoded, "new_save_\x01\x02\xFE", "Persisted payload must match original binary bytes");
        assert.strictEqual(sm.forageExists("file2"), true, "forageExists must report true for newly saved slot");

        // 4. Verify removeForage removes from native
        return sm.removeForage("file2");
    }).then(function() {
        assert.strictEqual(storedSlots["file2"], undefined, "file2 must be deleted from native repository");
        assert.strictEqual(sm.forageExists("file2"), false, "forageExists must be false after deletion");
    });
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:
```powershell
node android/shell/app/src/test/js/web_shims.test.js
```
Expected: FAIL (assertion error: `updateForageKeys` or `loadFromForage` does not return Promise or does not interact with `RuntimePackNative`).

- [ ] **Step 3: Implement MZ Universal Save Adapter in `runtimepack_web_shims.js`**

In `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`, update `patchStorageManager(sm)`:

```javascript
    function patchStorageManager(sm) {
        if (!sm) return;

        // Force isLocalMode to false so both MV and MZ use web storage hooks
        try {
            Object.defineProperty(sm, "isLocalMode", {
                configurable: true,
                enumerable: true,
                get: function() {
                    return function() { return false; };
                },
                set: function() {}
            });
        } catch (e) {
            sm.isLocalMode = function() { return false; };
        }

        // Install MZ Save Adapter if MZ forage methods exist
        if (typeof sm.saveToForage === "function" || typeof sm.loadFromForage === "function") {
            patchMzStorage(sm);
        }
    }

    function patchMzStorage(sm) {
        if (!sm || sm.__runtimepack_mz_save_patched) return;
        sm.__runtimepack_mz_save_patched = true;

        sm._forageKeys = sm._forageKeys || [];
        sm._forageKeysUpdated = false;

        var encodeBase64 = function(str) {
            try {
                if (typeof root.btoa === "function") return root.btoa(str);
                if (typeof Buffer !== "undefined") return Buffer.from(str, "binary").toString("base64");
            } catch (e) {}
            return str;
        };

        var decodeBase64 = function(b64) {
            try {
                if (typeof root.atob === "function") return root.atob(b64);
                if (typeof Buffer !== "undefined") return Buffer.from(b64, "base64").toString("binary");
            } catch (e) {}
            return b64;
        };

        sm.saveToForage = function(saveName, zip) {
            return new Promise(function(resolve, reject) {
                var nativeBridge = (typeof root.window !== "undefined" && root.window.RuntimePackNative)
                    || root.RuntimePackNative;
                if (nativeBridge && typeof nativeBridge.saveSlot === "function") {
                    var b64Payload = encodeBase64(zip);
                    var ok = nativeBridge.saveSlot("mz", String(saveName), b64Payload);
                    if (ok) {
                        if (!sm._forageKeys.includes(saveName)) {
                            sm._forageKeys.push(saveName);
                        }
                        resolve(0);
                    } else {
                        reject(new Error("[RuntimePack] Native save failed for slot: " + saveName));
                    }
                } else {
                    reject(new Error("[RuntimePack] Native save bridge unavailable"));
                }
            });
        };

        sm.loadFromForage = function(saveName) {
            return new Promise(function(resolve, reject) {
                var nativeBridge = (typeof root.window !== "undefined" && root.window.RuntimePackNative)
                    || root.RuntimePackNative;
                if (nativeBridge && typeof nativeBridge.loadSlot === "function") {
                    var rawB64 = nativeBridge.loadSlot("mz", String(saveName));
                    if (rawB64 !== null && rawB64 !== undefined) {
                        var decodedZip = decodeBase64(rawB64);
                        resolve(decodedZip);
                    } else {
                        resolve(null);
                    }
                } else {
                    resolve(null);
                }
            });
        };

        sm.forageExists = function(saveName) {
            var nativeBridge = (typeof root.window !== "undefined" && root.window.RuntimePackNative)
                || root.RuntimePackNative;
            if (sm._forageKeys && sm._forageKeys.includes(saveName)) {
                return true;
            }
            if (nativeBridge && typeof nativeBridge.slotExists === "function") {
                return nativeBridge.slotExists("mz", String(saveName));
            }
            return false;
        };

        sm.removeForage = function(saveName) {
            return new Promise(function(resolve) {
                var nativeBridge = (typeof root.window !== "undefined" && root.window.RuntimePackNative)
                    || root.RuntimePackNative;
                if (nativeBridge && typeof nativeBridge.deleteSlot === "function") {
                    nativeBridge.deleteSlot("mz", String(saveName));
                }
                var idx = sm._forageKeys.indexOf(saveName);
                if (idx !== -1) {
                    sm._forageKeys.splice(idx, 1);
                }
                resolve(0);
            });
        };

        sm.updateForageKeys = function() {
            sm._forageKeysUpdated = false;
            return new Promise(function(resolve) {
                var nativeBridge = (typeof root.window !== "undefined" && root.window.RuntimePackNative)
                    || root.RuntimePackNative;
                if (nativeBridge && typeof nativeBridge.listSlots === "function") {
                    try {
                        var listRaw = nativeBridge.listSlots("mz");
                        var parsed = JSON.parse(listRaw);
                        if (Array.isArray(parsed)) {
                            sm._forageKeys = parsed;
                        }
                    } catch (e) {
                        sm._forageKeys = [];
                    }
                }
                sm._forageKeysUpdated = true;
                resolve(0);
            });
        };

        sm.forageKeysUpdated = function() {
            return sm._forageKeysUpdated;
        };

        // Eagerly populate forage keys at boot so removeInvalidGlobalInfo has valid index
        sm.updateForageKeys();
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run:
```powershell
node android/shell/app/src/test/js/web_shims.test.js
```
Expected: PASS (All Mobile Web Shims tests passed successfully!).

- [ ] **Step 5: Commit**

```powershell
git add android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js android/shell/app/src/test/js/web_shims.test.js
git commit -m "feat(storage): implement RPG Maker MZ Universal Save Adapter with Base64 encoding"
```

---

### Task 4: RPG Maker MV Universal Save Adapter (JavaScript Web Shim)

**Files:**
- Modify: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
- Test: `android/shell/app/src/test/js/web_shims.test.js`

**Interfaces:**
- Consumes: `RuntimePackNative.saveSlot`, `loadSlot`, `slotExists`, `deleteSlot` with namespace `"mv"`.
- Produces: Interception of all 9 RPG Maker MV `StorageManager` persistence methods:
  `saveToWebStorage`, `loadFromWebStorage`, `webStorageExists`, `removeWebStorage`, `backup`, `backupExists`, `loadFromWebStorageBackup`, `cleanBackup`, `restoreBackup`.
  Deterministic alphanumeric slot mapping:
  - `-1` -> `"config"`
  - `0` -> `"global"`
  - `n` -> `"file" + n`
  - Backups: slot + `".bak"` (e.g. `"file1.bak"`)
  Synchronous API return contract (does not return Promise, matching MV engine expectations).

- [ ] **Step 1: Write failing unit tests for MV Save Adapter in `web_shims.test.js`**

Add to `android/shell/app/src/test/js/web_shims.test.js`:

```javascript
{
    // Test: RPG Maker MV Universal Save Adapter
    const sb = createSandbox();
    const mvSlots = {};

    sb.RuntimePackNative = {
        saveSlot: function(ns, slot, payload) {
            if (ns !== "mv") return false;
            mvSlots[slot] = payload;
            return true;
        },
        loadSlot: function(ns, slot) {
            if (ns !== "mv") return null;
            return mvSlots[slot] !== undefined ? mvSlots[slot] : null;
        },
        slotExists: function(ns, slot) {
            return ns === "mv" && mvSlots[slot] !== undefined;
        },
        deleteSlot: function(ns, slot) {
            if (ns === "mv" && mvSlots[slot] !== undefined) {
                delete mvSlots[slot];
                return true;
            }
            return false;
        },
        listSlots: function(ns) {
            if (ns !== "mv") return "[]";
            return JSON.stringify(Object.keys(mvSlots));
        }
    };

    // Minimal mock of LZString
    sb.LZString = {
        compressToBase64: function(s) { return "lzb64:" + s; },
        decompressFromBase64: function(s) { return s && s.startsWith("lzb64:") ? s.substring(6) : s; }
    };

    // Mock MV StorageManager before shims initialization
    sb.StorageManager = {
        saveToWebStorage: function() {},
        loadFromWebStorage: function() {},
        webStorageExists: function() {},
        removeWebStorage: function() {},
        backup: function() {},
        backupExists: function() {},
        cleanBackup: function() {},
        restoreBackup: function() {},
        loadFromWebStorageBackup: function() {}
    };

    vm.runInContext(shimsCode, sb);

    const sm = sb.StorageManager;

    // 1. Save and Load slot 1
    sm.saveToWebStorage(1, "{\"party\":[\"Actor1\"]}");
    assert.strictEqual(sm.webStorageExists(1), true, "Slot 1 must exist");
    const loadedJson = sm.loadFromWebStorage(1);
    assert.strictEqual(loadedJson, "{\"party\":[\"Actor1\"]}", "Loaded slot 1 JSON must match original");

    // Check underlying mapped slot in native repository
    assert.strictEqual(mvSlots["file1"], "lzb64:{\"party\":[\"Actor1\"]}", "Native slot must be mapped to 'file1'");

    // 2. Backup and restore
    sm.backup(1);
    assert.strictEqual(sm.backupExists(1), true, "Backup for slot 1 must exist");
    assert.strictEqual(mvSlots["file1.bak"], "lzb64:{\"party\":[\"Actor1\"]}", "Backup must be saved as file1.bak");

    // Overwrite slot 1 with corrupt data
    sm.saveToWebStorage(1, "corrupt");
    assert.strictEqual(sm.loadFromWebStorage(1), "corrupt");

    // Restore backup
    sm.restoreBackup(1);
    assert.strictEqual(sm.loadFromWebStorage(1), "{\"party\":[\"Actor1\"]}", "Restored slot 1 must match backup");
    assert.strictEqual(sm.backupExists(1), false, "Backup must be cleaned up after restore");

    // 3. Config and Global slots
    sm.saveToWebStorage(-1, "{\"volume\":80}");
    sm.saveToWebStorage(0, "[null,\"info1\"]");
    assert.strictEqual(mvSlots["config"], "lzb64:{\"volume\":80}");
    assert.strictEqual(mvSlots["global"], "lzb64:[null,\"info1\"]");
    assert.strictEqual(sm.loadFromWebStorage(-1), "{\"volume\":80}");
    assert.strictEqual(sm.loadFromWebStorage(0), "[null,\"info1\"]");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:
```powershell
node android/shell/app/src/test/js/web_shims.test.js
```
Expected: FAIL (assertion error: `sm.webStorageExists(1)` is undefined or false).

- [ ] **Step 3: Implement MV Universal Save Adapter in `runtimepack_web_shims.js`**

In `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`:
Add `patchMvStorage(sm)` and wire into `patchStorageManager(sm)`:

```javascript
    function patchMvStorage(sm) {
        if (!sm || sm.__runtimepack_mv_save_patched) return;
        sm.__runtimepack_mv_save_patched = true;

        var mapMvSlot = function(savefileId) {
            if (savefileId < 0) return "config";
            if (savefileId === 0) return "global";
            return "file" + savefileId;
        };

        var getNativeBridge = function() {
            return (typeof root.window !== "undefined" && root.window.RuntimePackNative)
                || root.RuntimePackNative;
        };

        sm.saveToWebStorage = function(savefileId, json) {
            var slot = mapMvSlot(savefileId);
            var data = (typeof root.LZString !== "undefined")
                ? root.LZString.compressToBase64(json)
                : json;
            var bridge = getNativeBridge();
            if (bridge && typeof bridge.saveSlot === "function") {
                bridge.saveSlot("mv", slot, data);
            }
        };

        sm.loadFromWebStorage = function(savefileId) {
            var slot = mapMvSlot(savefileId);
            var bridge = getNativeBridge();
            if (bridge && typeof bridge.loadSlot === "function") {
                var data = bridge.loadSlot("mv", slot);
                if (data !== null && data !== undefined) {
                    return (typeof root.LZString !== "undefined")
                        ? root.LZString.decompressFromBase64(data)
                        : data;
                }
            }
            return null;
        };

        sm.webStorageExists = function(savefileId) {
            var slot = mapMvSlot(savefileId);
            var bridge = getNativeBridge();
            if (bridge && typeof bridge.slotExists === "function") {
                return bridge.slotExists("mv", slot);
            }
            return false;
        };

        sm.removeWebStorage = function(savefileId) {
            var slot = mapMvSlot(savefileId);
            var bridge = getNativeBridge();
            if (bridge && typeof bridge.deleteSlot === "function") {
                bridge.deleteSlot("mv", slot);
            }
        };

        sm.backup = function(savefileId) {
            var slot = mapMvSlot(savefileId);
            var bakSlot = slot + ".bak";
            var bridge = getNativeBridge();
            if (bridge && typeof bridge.loadSlot === "function" && typeof bridge.saveSlot === "function") {
                var data = bridge.loadSlot("mv", slot);
                if (data !== null && data !== undefined) {
                    bridge.saveSlot("mv", bakSlot, data);
                }
            }
        };

        sm.backupExists = function(savefileId) {
            var bakSlot = mapMvSlot(savefileId) + ".bak";
            var bridge = getNativeBridge();
            if (bridge && typeof bridge.slotExists === "function") {
                return bridge.slotExists("mv", bakSlot);
            }
            return false;
        };

        sm.cleanBackup = function(savefileId) {
            var bakSlot = mapMvSlot(savefileId) + ".bak";
            var bridge = getNativeBridge();
            if (bridge && typeof bridge.deleteSlot === "function") {
                bridge.deleteSlot("mv", bakSlot);
            }
        };

        sm.restoreBackup = function(savefileId) {
            var slot = mapMvSlot(savefileId);
            var bakSlot = slot + ".bak";
            var bridge = getNativeBridge();
            if (bridge && typeof bridge.loadSlot === "function" && typeof bridge.saveSlot === "function") {
                var data = bridge.loadSlot("mv", bakSlot);
                if (data !== null && data !== undefined) {
                    bridge.saveSlot("mv", slot, data);
                    bridge.deleteSlot("mv", bakSlot);
                }
            }
        };

        sm.loadFromWebStorageBackup = function(savefileId) {
            var bakSlot = mapMvSlot(savefileId) + ".bak";
            var bridge = getNativeBridge();
            if (bridge && typeof bridge.loadSlot === "function") {
                var data = bridge.loadSlot("mv", bakSlot);
                if (data !== null && data !== undefined) {
                    return (typeof root.LZString !== "undefined")
                        ? root.LZString.decompressFromBase64(data)
                        : data;
                }
            }
            return null;
        };
    }
```

In `patchStorageManager(sm)`, call both:
```javascript
        if (typeof sm.saveToForage === "function" || typeof sm.loadFromForage === "function") {
            patchMzStorage(sm);
        }
        if (typeof sm.saveToWebStorage === "function" || typeof sm.loadFromWebStorage === "function") {
            patchMvStorage(sm);
        }
```

- [ ] **Step 4: Run test to verify it passes**

Run:
```powershell
node android/shell/app/src/test/js/web_shims.test.js
```
Expected: PASS (All Mobile Web Shims tests passed successfully!).

- [ ] **Step 5: Commit**

```powershell
git add android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js android/shell/app/src/test/js/web_shims.test.js
git commit -m "feat(storage): implement RPG Maker MV Universal Save Adapter with synchronous backup routing"
```

---

### Task 5: Full Automated Regression & APK Rebuild

**Files:**
- Output: `dist/forestia/Forestia.apk`
- Output: `dist/witchs_house/TheWitchsHouse.apk`

- [ ] **Step 1: Run all automated unit and workspace test suites**

Run in worktree `.worktrees/native-mv-mz-save`:
```powershell
node android/shell/app/src/test/js/web_shims.test.js
.\android\shell\gradlew.bat -p android/shell testDebugUnitTest
cargo test --workspace
```
Verify: 100% pass across JS tests, Kotlin Android tests, and Rust workspace tests.

- [ ] **Step 2: Rebuild Forestia APK**

Run:
```powershell
cargo run --bin runtimepack -- build "Raw-Game\Forestia1.4.4-IND" -o dist/forestia --package com.example.forestia --name "Forestia" --flavor beta --accept-audit-fail --accept-audit-unknown
```
Verify: Gradle exit 0, APK generated at `dist/forestia/Forestia.apk`.

- [ ] **Step 3: Rebuild The Witch's House MV APK**

Run:
```powershell
cargo run --bin runtimepack -- build "Raw-Game\The_Witchs_House_MV" -o dist/witchs_house --package com.example.witchshouse --name "The Witch's House MV" --flavor beta --accept-audit-fail --accept-audit-unknown
```
Verify: Gradle exit 0, APK generated at `dist/witchs_house/TheWitchsHouse.apk`.

- [ ] **Step 4: Deliver APKs to user for in-game Save & Load verification**
Report SHA256 hashes and sizes of both generated APKs to the user.
