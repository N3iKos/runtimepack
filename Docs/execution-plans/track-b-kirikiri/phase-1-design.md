# Specification: Kirikiri Engine Integration — Phase 1: Static Detection & In-Memory XP3 Parser

**Date:** 2026-09-27  
**Status:** Approved  
**Scope:** Host Rust Workspace, `EngineId::Kirikiri`, `engines/kirikiri`, In-Memory XP3 Binary Archive Parser, and Static Detector.

---

## 1. Context & Objective

RuntimePack currently supports RPG Maker (XP, VX, VX Ace, MV, MZ), Ren'Py (7/8), and TyranoScript. To expand support to Japanese visual novel engines, we are adding support for **Kirikiri 2 / Kirikiri Z (KAG / TVP Engine)**, inspired by the architecture of Kirikiroid2 as documented in `Docs/research/RuntimePack_Kirikiri_Android_Technical_Report.md`.

Phase 1 focuses strictly on host-side capabilities:
1. Registering `EngineId::Kirikiri` and `RuntimeFamily::KirikiriNative` in `core`.
2. Implementing the static detector in `engines/kirikiri` to detect Kirikiri games by signature files (`data.xp3`, `system.xp3`, `startup.tjs`, etc.).
3. Implementing an in-memory `.xp3` binary archive parser in Rust to read the Table of Contents (TOC) and catalog internal files (`.tlg`, `.png`, `.ogg`, `.ks`, `.tjs`) without disk extraction.
4. Integrating XP3 archive discovery into `GameSourceIndex` and static asset auditing.

---

## 2. Global Constraints & Principles

- **Zero Mutation:** Game sources remain strictly immutable.
- **Zero Disk Extraction:** `.xp3` archives are parsed in-memory via file slice reads / memory buffers; no temporary files are written to disk during audit or detection.
- **Evidence-First:** Binary parsing strictly adheres to the official Kirikiri Z / TVP specification as documented in `XP3Archive.h` and `XP3Archive.cpp`.
- **Deterministic Error Taxonomy:** Unknown or encrypted archive layouts emit stable diagnostic codes (`CMP-XP3-ENCRYPTED` or `ING-103`).

---

## 3. Detailed Component Design

### 3.1 Domain Types & Engine Registration
- **`core/src/domain.rs`:**
  - Add `EngineId::Kirikiri` with display name `"Kirikiri 2 / Kirikiri Z"`.
  - Add `RuntimeFamily::KirikiriNative`.
- **`core/src/detection/mod.rs` & `engines/mod.rs`:**
  - Register `KirikiriDetector` in the default detector registry.

### 3.2 Static Detector (`engines/kirikiri/src/lib.rs`)
- Scans `GameSourceIndex` for canonical Kirikiri markers:
  - `data.xp3`, `system.xp3`, `patch.xp3`, `*.xp3` (weight: +80 to +90).
  - `startup.tjs`, `initialize.tjs` (weight: +60).
  - Executables containing Kirikiri / TVP strings (e.g. `tvpwin32.exe`, `kagex.exe`).
- Assigns detection confidence (`EXACT` if `data.xp3` or `startup.tjs` present; `HIGH` if other `.xp3` archives found).

### 3.3 In-Memory XP3 Archive Parser (`engines/kirikiri/src/archive.rs`)

#### Binary Layout Specification:
1. **11-Byte Header:**
   - Bytes `0..11`: Full magic signature `b"XP3\r\n \n\x1a\x8b\x67\x01"` (`58 50 33 0D 0A 20 0A 1A 8B 67 01`).
   - Bytes `11..19`: `u64 index_offset` (little-endian), relative to `base`.
2. **Index Block Loop:**
   - Byte `+0`: `flags: u8`.
     - `flags & 0x07 == 0`: Raw uncompressed index.
     - `flags & 0x07 == 1`: Compressed with zlib (`uncompress()`).
     - `flags & 0x80 != 0`: Continuation block present (`next_index_offset` follows payload).
   - If raw: `index_size: u64` followed by raw chunk bytes.
   - If zlib: `compressed_size: u64`, `uncompressed_size: u64` followed by compressed stream.
   - Decompress zlib stream using `flate2::read::ZlibDecoder` into memory buffer.
3. **Chunk Structure (Nested in TOC payload):**
   - Each chunk: `tag: [u8; 4]` + `payload_len: u64` + `payload`.
   - Top-level chunk: `b"File"`. Contains sub-chunks:
     - `b"info"`: `flags: u32`, `original_size: u64`, `archived_size: u64`, `name_units: u16`, followed by `name_units * 2` bytes of UTF-16LE filename string.
     - `b"segm"`: 28-byte segments: `flags: u32`, `offset: u64`, `orig_len: u64`, `arch_len: u64`.
     - `b"adlr"`: `adler: u32`.
4. **Parsed Output Structure:**
   ```rust
   pub struct Xp3Entry {
       pub path: String,
       pub original_size: u64,
       pub archived_size: u64,
       pub offset: u64,
       pub is_compressed: bool,
   }

   pub struct Xp3ArchiveIndex {
       pub entries: HashMap<String, Xp3Entry>,
   }
   ```
5. **Safety & Bounds Verification:**
   - Bounded index read (reject index payload > 64 MB to prevent decompression bombs).
   - Validate UTF-16 decoding and normalize path separators (`\` -> `/`).
   - Reject path traversal attempts (`../`, absolute paths).

### 3.4 Integration with Host Tooling
- Expose `Xp3ArchiveIndex::parse(reader)` and `parse_bytes(&[u8])`.
- In `core/src/ingest`: When encountering `.xp3` files in a game directory, allow virtual indexing of internal files for static asset auditing.

---

## 4. Verification & Test Plan

1. **Unit Tests (`engines/kirikiri/src/archive.rs`):**
   - `test_xp3_valid_synthetic_archive_uncompressed`: Test synthetic XP3 with raw TOC containing multiple files.
   - `test_xp3_valid_synthetic_archive_zlib_compressed`: Test synthetic XP3 with zlib-compressed TOC.
   - `test_xp3_invalid_magic_rejected`: Test non-XP3 file rejected with `ArchiveError::InvalidMagic`.
   - `test_xp3_path_normalization_and_traversal_rejection`: Test traversal and Windows backslashes normalized.
2. **Integration Tests (`tests/integration/detection_test.rs`):**
   - `test_detect_kirikiri_game_exact_confidence`: Verify a fixture containing `data.xp3` and `startup.tjs` is detected as `EngineId::Kirikiri` with `DetectionConfidence::Exact`.
3. **Workspace Integrity:**
   - `cargo test --workspace` must pass with 100% success rate.

---

## 5. Definition of Done

- `engines/kirikiri` crate created and added to workspace `Cargo.toml`.
- `EngineId::Kirikiri` and `RuntimeFamily::KirikiriNative` registered.
- In-memory XP3 parser implemented with full 11-byte signature validation and zlib TOC decompression.
- All unit and integration tests passing cleanly.
- Documentation and memory updated.
