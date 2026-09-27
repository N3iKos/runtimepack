# Specification: Kirikiri 2 / Kirikiri Z Complete Architecture & Standalone Android Pipeline

**Date:** 2026-09-27  
**Status:** Approved  
**Scope:** Complete End-to-End Support for Kirikiri 2 / Kirikiri Z across Host CLI (Rust), Compatibility Analysis, Native Engine Runtime (C++ / `libkrkr2.so`), Android Shell (Kotlin / JNI), and Build Packaging.

---

## 1. Executive Summary & Objective

RuntimePack converts desktop visual novels and games into dedicated, standalone Android applications. This specification formalizes the architecture for supporting **Kirikiri 2 / Kirikiri Z (KAG / TVP Engine)** games across 4 distinct phases:
- **Phase 1: Rust Host Tooling** — Domain types, static detection, and in-memory 11-byte `.xp3` binary archive parsing.
- **Phase 2: Static Compatibility & Plugin Analysis** — Script parsing (`.tjs`), `Plugins.link(...)` extraction, plugin capability mapping, and custom XP3 encryption filter detection.
- **Phase 3: Headless Native C++ Runtime & Android Shell** — Direct-boot patch in C++ (`AndroidUtils.cpp` / `AppDelegate.cpp`), NDK r28 compilation for `arm64-v8a` with 16 KB page-size alignment, and `KirikiriActivity.kt` lifecycle bridge.
- **Phase 4: Packaging, VPad Overlay & End-to-End Verification** — Build plan generator, GPACK packaging, save data isolation (`filesDir/savedata`), Virtual Controller integration, and standalone APK compilation.

The final user experience must be strictly direct-boot:
```text
Install APK -> Tap Launcher Icon -> Direct Boot to Game Title Screen (Zero File Browser / Zero Launcher UI)
```

---

## 2. Global Constraints & Core Invariants

1. **Zero Game Mutation:** Game directories in `Raw-Game/` remain strictly 100% immutable.
2. **Zero Disk Extraction for Audit:** All archive inspections in Phase 1 & 2 operate in-memory via file slices and streaming decoders.
3. **No Man-in-the-Middle File Pickers:** Direct-boot is enforced at the C++ native layer; `TVPMainFileSelectorForm` is permanently disabled in production builds.
4. **App-Specific Storage:** Save data and logs are isolated exclusively to `Context.filesDir + "/savedata"`. No legacy external storage or `MANAGE_EXTERNAL_STORAGE` permissions.
5. **Android 15 (16 KB Page Alignment):** Native shared libraries (`libkrkr2.so` and dependencies) must be built with NDK r28+ ensuring 16 KB ELF segment alignment (`max-page-size=16384`).
6. **Honest Compatibility Taxonomy:** Unhandled proprietary encryption filters or missing native Windows plugins must emit deterministic diagnostics (`CMP-KRK-*`) and must never silently pretend to succeed.

---

## 3. Subsystem Specifications

### 3.1 Subsystem 1: Rust Host Core & In-Memory XP3 Parser (Phase 1)
- **Domain Identity:**
  - `EngineId::Kirikiri` (`"kirikiri"`).
  - `RuntimeFamily::KirikiriNative` (`"Kirikiri Native"`).
- **Signature & Header Validation:**
  - 11-byte magic: `58 50 33 0D 0A 20 0A 1A 8B 67 01` (`b"XP3\r\n \n\x1a\x8b\x67\x01"`).
  - Little-endian 64-bit TOC offset.
  - TOC stream zlib decompression via `flate2::read::ZlibDecoder`.
  - Chunk parser: `File` containing `info` (UTF-16LE path, sizes), `segm` (offsets, segment lengths), and `adlr` (hash).

### 3.2 Subsystem 2: Compatibility Scanner & Plugin Mapping (Phase 2)
- **Script Inspection:**
  - Parse `.tjs` and `.ks` files inside or outside XP3 for calls to `Plugins.link("*.dll")` or `Plugins.link("*.tpm")`.
- **Capability Database (`compatibility-db/runtimes/kirikiri-native.json`):**
  - **Supported Built-in Plugins:**
    `csvParser`, `dirlist`, `fftgraph`, `addFont`, `getSample`, `getabout`, `saveStruct`, `varfile`, `wutcwf`, `win32dialog`, `xp3filter`, `layerExMovie`, `perspective`/`layerExPerspective`, `scriptsEx`, `windowEx`, `fstat`, `TextRender`, `layerExDraw`, `motionplayer`, `emoteplayer`.
  - **Unsupported / Blocked Plugins:**
    `DrawDeviceD3D` (Windows DirectX only), `wuvorbis`, `extNag`, uncompiled proprietary DLLs. Emits `CMP_NATIVE_DLL_DEPENDENCY` (`CMP-102`).
- **Encrypted Archive Detection:**
  - If `.xp3` fails 11-byte signature check but has `.xp3` extension, or requires `XP3ArchiveExtractionFilter` without matching key, emit `CMP_AUD_POLICY_BLOCKED` with diagnostic `DET_KRK_ENCRYPTED_XP3`.

### 3.3 Subsystem 3: Native Engine Runtime (`libkrkr2.so`) & Android Shell (Phase 3)
- **Upstream Pin:** Base repository `2468785842/krkr2` (commit `dca7264572a63e753c1b07ca7053513d1751ce70`).
- **Toolchain:** NDK r28, C++17, CMake 3.31.1, AGP 8.7.3, `c++_shared`. Target ABI: `arm64-v8a` (with optional `x86_64` for emulator).
- **Direct-Boot C++ Patches:**
  1. `AndroidUtils.cpp`:
     ```cpp
     bool TVPCheckStartupArg() {
         // Register event dispatcher...
         const std::string entry = GetInternalStoragePath() + "/game" + RUNTIMEPACK_ENTRY_SUFFIX;
         return TVPMainScene::GetInstance()->startupFrom(entry);
     }
     ```
  2. `AppDelegate.cpp`:
     ```cpp
     if (!TVPCheckStartupArg()) {
         TVPShowSimpleMessageBox("Game startup failed: invalid package or missing startup script", "RuntimePack Error");
         exit(1);
     }
     ```
- **Android Shell Activity:**
  - `KirikiriActivity.kt` inherits Cocos2dx / KR2 lifecycle, manages SurfaceView, intercepts touches, and isolates `-datapath` to `context.filesDir + "/savedata"`.

### 3.4 Subsystem 4: Build Plan, GPACK & VPad Overlay (Phase 4)
- **Build Generator (`core/src/build_plan/generator.rs`):**
  - Generates Android project configuring `KirikiriActivity` as the main launcher activity.
  - Packages native binaries (`libkrkr2.so`, `libc++_shared.so`).
- **Aset Staging & Packaging:**
  - Packages game assets into `game.pack` (or directly stages `.xp3` archives if stored uncompressed).
  - Staging manager extracts/links `.xp3` files to `filesDir/game/` during bootstrap.
- **Overlay Integration:**
  - Transparent HTML5/CSS overlay hosting `vpad.js` (Visual Novel preset: Quick Save, Quick Load, Backlog / History, Auto, Skip, Hide Textbox).
  - Coordinates mapped to TVP touch gestures (1 finger = Left Click, 2 fingers = Right Click / Menu, Wheel = Backlog).

---

## 4. Milestone Matrix & Delivery Sequence

| Milestone | Deliverables | Verification Gates |
| :--- | :--- | :--- |
| **Phase 1** | Crate `engines/kirikiri`, `EngineId::Kirikiri`, `archive.rs` (XP3 parser), static detector | Unit tests on synthetic XP3 (raw & zlib), detection tests on mock Kirikiri directories. |
| **Phase 2** | `KirikiriAnalyzer`, plugin scanner (`Plugins.link`), compatibility DB, encryption probe | Compatibility test asserting supported plugins pass and unsupported DLLs emit `CMP-102`. |
| **Phase 3** | C++ direct-boot patches, NDK r28 build, `KirikiriActivity.kt`, JNI bridge | `llvm-readelf -Wl libkrkr2.so` verifies 16 KB alignment; direct boot unit test passes. |
| **Phase 4** | Build plan generator, GPACK integration, VPad overlay, end-to-end APK packaging | Full APK compiled with `--flavor beta`, boots directly to title screen without file picker. |
