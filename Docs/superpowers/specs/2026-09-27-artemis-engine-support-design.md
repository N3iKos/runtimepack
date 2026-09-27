# Specification: Artemis Engine (PFS / Lua) Support & Standalone Android Pipeline

**Date:** 2026-09-27  
**Status:** Approved  
**Technical Basis:** `Docs/RuntimePack_Artemis_Research.md`  
**Scope:** Host Rust Workspace, `EngineId::Artemis`, In-Memory PFS Parser (PF8/PF6/PF2 with SHA-1 XOR stream cipher), Static Detection, `art3m1s-core` C FFI Native Runtime, and `ArtemisActivity.kt` Shell.

---

## 1. Executive Summary & Objective

Artemis Engine (developed by iMel Inc. / Mikage) is the Lua-scripted visual novel engine behind over 730 modern Japanese visual novels listed on VNDB. Desktop releases store game assets in `.pfs` binary archives, declare system parameters in `system.ini`, and define scene flow using `.ast` (Lua tables), `.iet`, or `.asb` scripts.

RuntimePack provides a direct-boot, compiler-driven path to standalone Android APKs for Artemis games:
- **Host Layer (Rust):** Zero-disk-extraction in-memory parser for `.pfs` (PF8, PF6, PF2) with automatic SHA-1 XOR payload decryption, multi-volume precedence (`root.pfs.000..099`), and static script asset auditing.
- **Runtime Layer (Native / Android):** Embedded native runner based on `art3m1s-core` (MPL-2.0) utilizing its C FFI ABI (`art3m1s_get_api_v1`), strictly bypassing launcher UIs/file choosers to boot directly to the game title screen with 16 KB ELF page alignment for Android 15.

User Experience:
```text
Install APK -> Tap Launcher Icon -> Direct Boot to Artemis Title Screen (Zero File Browser / Zero Artroid UI)
```

---

## 2. Core Invariants & Global Constraints

1. **Zero Game Mutation:** Game directories in `Raw-Game/` remain strictly 100% immutable.
2. **Zero Disk Extraction for Audit:** All archive inspections in Phase 1 & 2 operate in-memory via file slices and streaming decoders.
3. **App-Specific Storage:** Save data and runtime logs are isolated exclusively to `Context.filesDir + "/saves/<title-id>"`. No legacy external storage or `MANAGE_EXTERNAL_STORAGE` permissions.
4. **Android 15 (16 KB Page Alignment):** Native shared libraries (`libart3m1s.so`) must be built with NDK r28+ ensuring 16 KB ELF segment alignment (`max-page-size=16384`).
5. **No Blind Full Compatibility Claims:** Complex games requiring un-shimmed Windows DLLs or proprietary E-mote SDKs must emit structured diagnostics (`CMP-ART-001`) rather than pretending to succeed.

---

## 3. Subsystem Specifications

### 3.1 Domain & Engine Registration (`core/src/domain.rs`)
- Add `EngineId::Artemis`:
  - `as_str() -> "artemis"`
  - Display name: `"Artemis Engine"`
  - `runtime_family() -> Some(RuntimeFamily::ArtemisNative)`
- Update `RuntimeFamily::ArtemisNative` (`"Artemis Native"`).
- Update `schemas/build-manifest.schema.json` to include `"artemis"` and `"Artemis Native"`.

### 3.2 In-Memory PFS Binary Archive Parser (`engines/artemis/src/archive.rs`)

#### Binary Layout (PF8, PF6, PF2):
- **Magic Signature (Bytes `0x00..0x03`):** ASCII string in lowercase: `b"pf8"`, `b"pf6"`, or `b"pf2"`.
- **Index Size (Bytes `0x03..0x07`):** `index_size: u32` (little-endian), defining the byte length of the TOC block starting at `0x07`.
- **Entry Count:**
  - PF6 / PF8: `entry_count: u32` at offset `0x07..0x0B`.
  - PF2: `entry_count: u32` at offset `0x0B..0x0F`.
- **TOC Entry Structure:**
  - Length of filename: `name_len: u32`.
  - Filename bytes: `name[name_len]` (decoded via CP932 / Shift-JIS or UTF-8).
  - Reserved bytes: 4 bytes (PF6/PF8) or 12 bytes (PF2).
  - Absolute data offset: `absolute_offset: u32`.
  - Stored data length: `stored_size: u32`.
  *(Note: Header and TOC structures are plaintext).*

#### PF8 Payload Decryption (SHA-1 XOR Stream Cipher):
1. **Key Generation:**
   Read exact `index_size` bytes from `0x07` to `0x07 + index_size`.
   Compute 20-byte key:
   $$K = \text{SHA-1}(\text{file}[0x07 \dots 0x07 + index\_size])$$
2. **Payload Decryption:**
   For each file starting at `absolute_offset`, decrypt byte $i$ (0-indexed from file start):
   $$\text{plaintext}[i] = \text{stored}[i] \oplus K[i \pmod{20}]$$
   *(Auditor tests file magic both raw and de-XORed to support unencrypted media exceptions like MP4).*

#### Multi-Volume Patch Precedence:
- Volume numbering convention: `root.pfs`, `root.pfs.000`, `root.pfs.001` ... `root.pfs.099`.
- Higher numbered volumes override identical filenames from lower volumes. Loose files in directory act as fallback.

### 3.3 Static Detection (`engines/artemis/src/lib.rs`)
- Scans `GameSourceIndex` for canonical Artemis markers:
  - `root.pfs` or `*.pfs` archives (+90 pts, `DET-ART-001`).
  - `system.ini` with `[Artemis]` or `[Project]` sections (+80 pts, `DET-ART-002`).
  - Script files `.ast`, `.asb`, `.iet` (+60 pts, `DET-ART-003`).
- Confidence: `Exact` if score >= 90, `High` if score >= 60, `Medium` if score >= 30, else `None`.

### 3.4 Static Asset Reachability Auditor (`engines/artemis/src/asset_audit.rs`)
- In-memory parsing of `.ast` scripts (lightweight Lua table token parser extracting `bg`, `fg`, `bgm`, `voice` without running arbitrary Lua).
- Resolves asset references against the merged multi-volume `PfsArchiveIndex`.
- Emits `CMP-AUD-002` for missing required assets, and flags `.psb` / `.mdf` E-mote dependencies as `CMP_AUD_DYNAMIC_REFERENCE_UNRESOLVED` (Warning).

### 3.5 Headless Native Runtime Integration (`art3m1s-core` C FFI)
- Upstream base: `art3m1s-core` (MPL-2.0).
- C FFI integration in `ArtemisActivity.kt`:
  ```text
  api = art3m1s_get_api_v1()
  res = api.resources_create()
  api.resources_mount_directory(res, "filesDir/game")
  api.resources_set_save_dir(res, "filesDir/saves/<title-id>")
  runtime = api.runtime_create(width, height, GLES_BACKEND)
  api.runtime_set_resources(runtime, res)
  api.runtime_load_project_bytes(runtime, systemIniBytes, "ANDROID")
  ```
- Native library: `libart3m1s.so` built with NDK r28 and verified 16 KB page-size alignment (`llvm-readelf -Wl`).

---

## 4. Testing & Verification Plan

1. **Unit Tests (`engines/artemis/src/archive.rs`):**
   - Test synthetic PF8 archive with SHA-1 XOR stream cipher verification.
   - Test synthetic PF6 and PF2 unencrypted archives.
   - Test multi-volume override resolution (`.pfs.000` vs `.pfs.001`).
2. **Integration Tests (`tests/integration/artemis_test.rs`):**
   - Test detection of synthetic Artemis game directory.
   - Test AST script resource reference parsing.
3. **Build Pipeline Integration (`tests/integration/build_test.rs`):**
   - Test generation of Artemis Android project configuring `ArtemisActivity.kt`.
