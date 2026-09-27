# Specification: Visual Novel Maker (VNM) Engine Support & Standalone Android Pipeline

**Date:** 2026-09-27  
**Status:** Approved  
**Scope:** Host Rust Workspace, `EngineId::VisualNovelMaker`, Static Detector, Asset Reachability Auditor, NW.js Web Shims, and Build Packaging via `WebActivity.kt`.

---

## 1. Executive Summary & Objective

Visual Novel Maker (Degica / Gotcha Gotcha Games) is an HTML5-based visual novel creation suite. In desktop exports, it packages an HTML5 canvas and WebGL renderer (including Live2D Cubism support) inside an NW.js wrapper with an `index.html`, a `package.json` manifest, a digital `signature` file, and `data/` + `resources/` asset directories.

Official Android deployment for Visual Novel Maker utilizes Apache Cordova. In RuntimePack, we bypass the clumsy and insecure Cordova toolchain by compiling games directly into dedicated, hardened standalone Android APKs using our existing high-performance **Web Runtime (`WebActivity.kt`)** and on-the-fly streaming decryptor (`VfsResourceInterceptor.kt`).

User Experience:
```text
Install APK -> Tap Launcher Icon -> Direct Boot to VN Maker Title Screen (Instant WebGL / Zero Cordova Overhead)
```

---

## 2. Technical Architecture & Component Reuse

| Layer | Implementation in RuntimePack | Reused Subsystems |
| :--- | :--- | :--- |
| **Domain Identity** | `EngineId::VisualNovelMaker` (`"vnmaker"`) | `core/src/domain.rs` |
| **Runtime Backend** | `RuntimeFamily::Web` (`"web"` / `"RPG Maker Web"`) | Reuses `WebActivity.kt` without requiring new C++ NDK binaries |
| **Asset Streaming VFS** | Direct streaming from `game.pack` via `VfsResourceInterceptor.kt` | Zero-disk extraction, on-the-fly decryption, 1x1 transparent PNG / empty audio fallbacks |
| **Desktop API Shims** | `runtimepack_web_shims.js` | Polyfills `process.execPath`, Node `path`, `fs`, `nw.gui`, and Web Audio autoplay unlocks |
| **Save Data Management** | App-specific internal storage (`Context.filesDir`) | Sandboxed LocalStorage and WebStorage bridge |
| **Virtual Controls** | `vpad.js` Visual Novel Preset | Floating action buttons: Quick Save, Quick Load, Backlog, Auto, Skip, Hide UI |

---

## 3. Detailed Component Specifications

### 3.1 Domain & Engine Registration (`core/src/domain.rs`)
- Add `EngineId::VisualNovelMaker`:
  - `as_str() -> "vnmaker"`
  - Display name: `"Visual Novel Maker"`
  - `runtime_family() -> Some(RuntimeFamily::Web)`
- Update `schemas/build-manifest.schema.json` to include `"vnmaker"`.

### 3.2 Static Detection (`engines/vnmaker/src/lib.rs`)
- Scans `GameSourceIndex` for canonical Visual Novel Maker markers:
  - `signature` file present in root (+90 points, `DET-VNM-001` - canonical VN Maker digital signature file).
  - `data/System.json` containing VN Maker schema keys (e.g. `data-record` or `items.data`) (+80 points, `DET-VNM-002`).
  - `package.json` with `"main": "index.html"` and root `index.html` (+40 points, `DET-VNM-003`).
  - Presence of `data/Scenes.json` or `data/Layouts.json` (+50 points, `DET-VNM-004`).
- Distinction from RPG Maker MV/MZ and Tyrano:
  - If `js/rpg_core.js` or `js/rmmz_core.js` exists -> RPG Maker, not VNM.
  - If `tyrano/tyrano.js` or `.ks` files exist -> Tyrano, not VNM.
  - Absence of RPG Maker/Tyrano markers + presence of `signature` or VNM `data/System.json` -> Confidence `EXACT` (Score >= 90).

### 3.3 Static Asset Reachability Auditor (`engines/vnmaker/src/asset_audit.rs`)
- Inspects `data/Scenes.json` and `data/System.json` for referenced graphic and audio assets:
  - Images: `resources/Graphics/Backgrounds/`, `resources/Graphics/Characters/`, `resources/Graphics/Pictures/`.
  - Audio: `resources/Audio/BGM/`, `resources/Audio/SE/`, `resources/Audio/Voice/`.
  - Fonts: `fonts/` (.ttf, .woff).
- Reachability Filter:
  - Assets referenced only in unused template database records are classified as `CMP_AUD_POSSIBLY_UNUSED` (Severity: Info).
  - Case mismatches are automatically aliased by Android VFS and do not fail the audit.

### 3.4 Build Generator & Packaging (`core/src/build_plan/generator.rs`)
- When `manifest.runtime.family == "web"` (or engine is `vnmaker`):
  - Sets launcher activity to `io.runtimepack.host.WebActivity`.
  - Packs `index.html`, `package.json`, `data/`, `resources/`, `fonts/`, and `js/` into `game.pack`.
  - Injects `runtimepack_web_shims.js` and `vpad.js` (with Visual Novel preset enabled).

---

## 4. Verification & Testing Strategy

1. **Unit & Detection Tests (`tests/integration/detection_test.rs`):**
   - Test synthetic VN Maker project with `signature`, `package.json`, and `data/System.json` detected as `EngineId::VisualNovelMaker` with `DetectionConfidence::Exact`.
   - Test that RPG Maker MV and Tyrano games are not falsely detected as VNM.
2. **Audit Tests (`tests/integration/vnmaker_asset_audit_test.rs`):**
   - Test asset resolution for background, character, and audio references.
3. **Build Pipeline Integration (`tests/integration/build_test.rs`):**
   - Test end-to-end `runtimepack build` on synthetic VNM game, verifying APK project generation with `WebActivity`.

---

## 5. Definition of Done

- `engines/vnmaker` crate created and added to workspace `Cargo.toml`.
- `EngineId::VisualNovelMaker` registered across domain models and schemas.
- Static detector and asset auditor implemented and tested.
- Build generator wires VNM to `WebActivity.kt`.
- 100% of workspace tests pass cleanly.
