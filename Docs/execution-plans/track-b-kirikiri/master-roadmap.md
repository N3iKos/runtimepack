# Master Roadmap: Kirikiri 2 / Kirikiri Z Standalone Android Pipeline

**Date:** 2026-09-27  
**Status:** Approved  
**Spec Reference:** `Docs/execution-plans/track-b-kirikiri/architecture-spec.md`  
**Technical Basis:** `Docs/research/RuntimePack_Kirikiri_Android_Technical_Report.md`

---

## 1. Overview & Objective

This roadmap governs the end-to-end implementation of **Kirikiri 2 / Kirikiri Z** engine support in RuntimePack. The objective is to convert Windows Kirikiri visual novels into dedicated standalone Android APKs that boot directly into the game without launcher interfaces or file selectors.

Execution is strictly divided into 4 sequential phases:

```text
Phase 1: Rust Host Tooling & In-Memory XP3 Parser
   │ (Detects Kirikiri games, parses 11-byte .xp3 archives in RAM, catalogs files)
   ▼
Phase 2: Compatibility Scanner & Plugin Validator
   │ (Scans TJS Plugins.link, maps built-in C++ shims vs unsupported Windows DLLs)
   ▼
Phase 3: Headless Native C++ Runtime & Android Shell
   │ (Bypasses TVP file selector in C++, builds libkrkr2.so on NDK r28 16KB, implements KirikiriActivity)
   ▼
Phase 4: Packaging Pipeline, VPad Overlay & End-to-End Build
   │ (Build plan generator, game.pack staging, VN virtual controller overlay, standalone APK compilation)
   ▼
Verification & Production Delivery
```

---

## 2. Phase Plans Directory

| Phase | Plan Document | Scope & Key Artifacts | Status |
| :---: | :--- | :--- | :---: |
| **Phase 1** | `Docs/execution-plans/track-b-kirikiri/phase-1-plan.md` | Domain registration, `engines/kirikiri`, 11-byte XP3 binary parser, static detector | **Ready** |
| **Phase 2** | `Docs/execution-plans/track-b-kirikiri/phase-2-plan.md` | `KirikiriAnalyzer`, `Plugins.link` AST scanner, C++ plugin mapping, encryption probe | **Ready** |
| **Phase 3** | `Docs/execution-plans/track-b-kirikiri/phase-3-plan.md` | Direct-boot C++ patches (`AndroidUtils.cpp`), NDK r28 toolchain, `KirikiriActivity.kt` | **Ready** |
| **Phase 4** | `Docs/execution-plans/track-b-kirikiri/phase-4-plan.md` | `generator.rs` emitter, VPad VN preset, save isolation, end-to-end APK verification | **Ready** |

---

## 3. Strict Sequential Completion Gates

Before advancing from Phase N to Phase N+1, the following gates must be satisfied:

1. **Gate 1 -> 2:**
   - `cargo test -p runtimepack-engine-kirikiri` passes 100%.
   - Synthetic uncompressed and zlib-compressed `.xp3` archives are parsed cleanly in RAM without disk extraction.
   - `runtimepack inspect <game>` accurately detects `EngineId::Kirikiri`.
2. **Gate 2 -> 3:**
   - `KirikiriAnalyzer` passes integration tests.
   - Calls to built-in plugins (`csvParser`, `fstat`, `windowEx`, etc.) resolve to `CompatibleWithShims`.
   - Calls to un-shimmed Windows DLLs (`DrawDeviceD3D.dll`, `wuvorbis.dll`) emit `CMP-102` with `UnsupportedDependency`.
3. **Gate 3 -> 4:**
   - Native library `libkrkr2.so` compiled from pinned source `2468785842/krkr2` using NDK r28.
   - ELF alignment verified to be 16 KB (`llvm-readelf -Wl libkrkr2.so` has `p_align >= 16384`).
   - `TVPCheckStartupArg()` and `AppDelegate.cpp` patches eliminate file picker fallback.
4. **Gate 4 -> Production:**
   - Standalone APK successfully builds with `--flavor beta`.
   - Boot verified on Android (MuMu Player / ARM64): direct transition to title screen.
   - Save/load verified to write exclusively to `Context.filesDir + "/savedata"`.
