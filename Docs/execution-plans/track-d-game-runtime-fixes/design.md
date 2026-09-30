# Specification: Track D — Game Runtime Fixes (Show Off & Mizuki And Me)

**Date:** 2026-09-27  
**Status:** Approved  
**Scope:** TyranoScript runtime data-corruption fix (`Show Off`) and Ren'Py Android runtime implementation (`Mizuki And Me`).

---

## 1. Context & Objective

Two previously "built" APKs fail on the MuMu emulator. Root-cause investigation (evidence-first, sub-agent driven) established:

| Game | Engine | Symptom | Root Cause (proven) |
| :--- | :--- | :--- | :--- |
| **Show Off** | TyranoScript | Overlay UI visible, game canvas black | Every file in `game.pack` is shifted by 3 bytes |
| **Mizuki And Me** | Ren'Py | Layout flashes then blackscreen | Ren'Py Android runtime does not exist |

Objective: make **both games playable, normal, and stable** on Android, executed strictly one-by-one via sub-agents.

---

## 2. Track 1 — Show Off (TyranoScript) Root Cause & Fix

### 2.1 Root Cause
`engines/tyrano/src/asar.rs:59` computes:
```rust
let data_offset = 16 + json_len as u64;   // WRONG
```
The correct ASAR data offset is `8 + header_size`, where `header_size` is the second `u32` in the ASAR header (currently read but discarded as `_header_size` at `asar.rs:30`). The JSON header is 4-byte aligned with padding; the wrong formula ignores the padding.

- Show Off: `header_size = 431480`, `json_len = 431469` → correct `431488` vs used `431485` = **3-byte shift**.
- NTRdemic: `header_size = 61268`, `json_len = 61260` (`json_len % 4 == 0`) → delta 0, bug hidden.

Empirical proof: `game.pack[3:] == app.asar[:-3]` → True. Runtime logcat: mass `Uncaught SyntaxError: Unexpected token '}'` on all Tyrano JS.

### 2.2 Fix
1. `engines/tyrano/src/asar.rs`: compute `data_offset = 8 + header_size` (use the real `header_size`).
2. Unit tests covering both `json_len % 4 == 0` (NTRdemic-like) and `json_len % 4 != 0` (Show Off-like) padding.
3. Post-pack integrity gate: verify round-trip SHA-256 (source file ↔ extracted `game.pack` entry) to catch this class of bug automatically.

### 2.3 Verification
- Rebuild `dist/show_off/ShowOff.apk` with `--flavor beta`.
- Install + boot on MuMu; verify title screen renders, audio plays, input works, logcat free of `Uncaught SyntaxError`.

---

## 3. Track 2 — Mizuki And Me (Ren'Py) Android Runtime

### 3.1 Root Cause
RuntimePack has **no Ren'Py Android runtime**:
- No `RenpyActivity`/`PythonActivity` exists in the Android shell (grep: 0 matches).
- `generator.rs:556-560` routes Ren'Py to `BootstrapActivity` (`_` arm), whose `setContentView(View(this))` draws nothing.
- `RuntimeHost.kt:69` → Ren'Py resolves to `else -> TestBackend()` (no-op stub).
- `jniLibs/arm64-v8a/` ships RGSS libraries only; no `libpython`/`librenpython`.
- Runtime registry declares `runtime-renpy-7/8` but only TOML manifests exist — no engine/JNI.

### 3.2 Strategy — Bundle Official RAPT Runtime
Use the official Ren'Py Android Packaging Tool (RAPT) native runtime from the Ren'Py SDK:
- `libpython3.x.so`, `librenpython.so`, SDL2 libraries for `arm64-v8a`.
- Build a RuntimePack-owned `RenpyActivity.kt` adopting the RAPT `PythonActivity`/`SDLActivity` launch pattern.

### 3.3 Implementation Outline
1. **Acquire + audit RAPT runtime:** fetch `libpython`, `librenpython`, SDL2 libs for `arm64-v8a`; record version, provenance, and license into `LICENSES`/third-party notices (mandatory per `AGENTS.md`).
2. **Create `RenpyActivity.kt`:** init Python VM, mount game content from `filesDir`, GL surface, audio, input; isolate saves to `filesDir`.
3. **Bundle libs** under `runtimes/renpy/jni/arm64-v8a/` and wire the generator to copy them.
4. **Generator wiring:** map Ren'Py runtime family → `RenpyActivity`; emit `jniLibs`.
5. **Content mounting:** ensure `.rpa` archives (gui/img/sfx) and `.rpy`/`.rpyc` scripts are reachable by the runtime.
6. **Rebuild** `dist/mizuki_and_me/MizukiAndMe.apk` with `--flavor beta`.
7. **Verify on MuMu:** install → boot → title screen → navigation → save/load; check Python logcat.

---

## 4. Track 3 — Ren'Py Overlay & Touch Routing Integration

### 4.1 Context & Objective
Mizuki And Me now boots to the title screen and plays in Ren'Py 8.3.7. However, the RuntimePack UI overlay (`vpad.js`, diagnostic suite, cheat overlay, feedback ripple/trail) is not yet attached to `RenpyActivity`.
Objective: Attach `overlayWebView` on top of `mLayout`, provide the `RuntimePackVPad` JavascriptInterface, and implement selective touch pass-through so VN dialogue taps flow directly to `mSurface` while virtual buttons receive input.

### 4.2 Architecture & Touch Flow
1. **Overlay Attachment (`setupOverlayWebView`)**:
   - Create a transparent WebView over `mLayout` pointing to `file:///android_asset/overlay/index.html`.
   - Register `RuntimePackVPad` JavascriptInterface implementing:
     - `updateControlHitboxes(json: String)` (stores active `ControlHitbox` bounds).
     - `openCommunityLink()` (routes to `openUrl("https://linktr.ee/Veldra.Works")`).
     - Virtual gamepad button injection to SDL via `onNativeKeyDown(keyCode)` and `onNativeKeyUp(keyCode)`.
     - Modal and visibility state (`isOverlayModalOpen`, `isGamepadHidden`).
2. **Precision Touch Routing (`dispatchTouchEvent`)**:
   - When a touch lands on screen:
     - If `isOverlayModalOpen == true`, all touches belong to `overlayWebView`.
     - If touch is inside any active control hitbox or toolbar, dispatch to `overlayWebView`.
     - If touch is outside all control hitboxes (the visual novel reading area), dispatch directly to `mSurface` so Ren'Py advances dialogue and handles choices.
3. **Lifecycle Synchronization**:
   - `overlayWebView?.onPause()`, `overlayWebView?.onResume()`, `overlayWebView?.destroy()` synced to `RenpyActivity` lifecycle.

---

## 5. Track 4 — 4 Pillars of Universal RPG Maker VFS Architecture

### 5.1 Problem Statement
In `Forestia.apk` (and potentially any complex RPG Maker MV/MZ game):
1. Plugin code calls `fs.readdirSync("data")` to dynamically enumerate maps/files. In Android WebView, `runtimepack_web_shims.js` hardcodes `readdirSync: function() { return []; }`, corrupting `$globalMap._data` and throwing `TypeError: Cannot read properties of undefined (reading 'getRoadRegion')`.
2. `fs.existsSync` and `fs.statSync` use synchronous `syncFetch`, downloading multi-megabyte audio files into RAM just to check existence.
3. `VfsResourceInterceptor.kt` performs O(N) linear scans (`reader.entries.firstOrNull`) on every single asset request.
4. `AssetProbeRunner.kt` only tests `File(gameDir, relPath).exists()`, failing all 1,158 assets in packed mode with false-positive `File not found`.
5. Compiler path sanitization in `core/src/asset_audit/report.rs` strips `"js/"` when matching `"plugins"` via `rposition`.

### 5.2 The 4 Pillars
1. **Pilar 1: In-Memory Native VFS Bridge `readdirSync`**:
   - Native method `RuntimePackNative.readdir(relPath: String): String` returns entries directly from `GpackStreamReader.fileIndexMap.keys` with disk fallback.
   - `runtimepack_web_shims.js` calls `RuntimePackNative.readdir()` with dynamic `MapInfos.json` parsing as secondary fallback.
2. **Pilar 2: Fast Stat and Exists Sync Bridge**:
   - Native methods `RuntimePackNative.fileExists(relPath)` and `RuntimePackNative.fileSize(relPath)` return metadata in O(1) without HTTP downloads.
   - `runtimepack_web_shims.js` delegates `fs.existsSync` and `fs.statSync` to these native methods.
3. **Pilar 3: O(1) Case-Insensitive Hash Map Resolver**:
   - In `VfsResourceInterceptor.kt`, replace `firstOrNull` linear scan with `reader.getEntry(canonical)` O(1) hash map lookup.
   - Pre-index plugin script basenames in a lookup table for O(1) plugin script fallback.
4. **Pilar 4: VFS-Aware Diagnostics & Fix Path Sanitizer Compiler**:
   - In `AssetProbeRunner.kt`, probe files via `gpackReader?.hasFile(relPath) ?: file.exists()` and open streams via `gpackReader?.openStream(relPath)`.
   - In `core/src/asset_audit/report.rs`, fix `sanitize_path_str()` to use first matching prefix or relative root instead of `rposition` on `WELL_KNOWN_DIRS`.

---

## 6. Global Constraints

- Source games in `Raw-Game/` remain strictly 100% immutable.
- All autonomous builds use `--flavor beta`.
- All runtime verification performed on the MuMu emulator (`127.0.0.1:16384`).
- Third-party native binaries require pinned version + provenance + license notice before bundling.
- Each track ends only when the game boots and is playable on the emulator.
