# Specification: Encrypted Streaming Virtual File System (VFS) for RGSS & Mobile Runtimes

- **Date:** 2026-09-20
- **Status:** Draft / Proposed
- **Author:** RuntimePack Core Engineering
- **Target Milestones:** Milestone 4 (RGSS Native Slice Hardening) & Milestone 7 (Encrypted-Streaming Content VFS)

---

## 1. Overview & Problem Statement

In the baseline prototype deployment of RuntimePack for RPG Maker VX Ace (RGSS3 / Lona RPG), 15,000+ loose asset files (~780 MB) are extracted from `game.pack` onto the target device's internal storage (`filesDir/game`).

While functional, this approach presents several critical disadvantages:
1. **Zero Asset Hardening:** All game graphics, audio, and compiled scripts (`Scripts.rvdata2`) reside as plaintext files on disk, exposing them to casual browsing via mobile file managers (ZArchiver, etc.).
2. **High Storage Footprint:** The application occupies double storage space (~1.6 GB) due to maintaining both the installed APK (`assets/game.pack`) and the unpacked plaintext tree on disk.
3. **First-Launch Delay:** The initial extraction takes 10–15 seconds on physical devices, degrading user onboarding experience.

This specification defines the architecture for **Zero-Disk Encrypted Streaming Virtual File System (VFS)**: game assets remain encrypted inside a single authenticated container (`game.pack`) using **ChaCha20-Poly1305 AEAD**. During gameplay, assets are decrypted on-demand directly into RAM by the native engine layer without ever hitting disk storage.

---

## 2. Goals and Non-Goals

### Goals
- **Zero-Disk Plaintext:** Game assets are never unpacked onto physical or emulated flash storage.
- **Deterministic 60 FPS Streaming:** Decryption latency per 64–256 KB chunk must stay below 1 millisecond on ARM64 processors.
- **Automated Key Generation:** 256-bit encryption keys are derived deterministically per game build using package-bound HKDF-SHA256, requiring zero manual configuration from developers.
- **Anti-Tampering Integrity:** Tampering with container bytes or replacing encrypted files triggers cryptographic authentication failure (`CNT-201`) and fails closed.
- **Clean Isolation of Mutable Data:** Immutable game content (`game:/`) and writable user data (`save:/`, `config:/`) remain strictly decoupled.

### Non-Goals
- **Client Secrecy Against Kernel Debugging:** This architecture does not claim uncrackable DRM against rooted devices running kernel memory dumpers or dynamic instrumentation (Frida, GDB). Per `AGENTS.md` Section 17, it is classified strictly as **Asset Hardening**.
- **Universal Transpilation:** Does not alter game script semantics; runtime hooks intercept standard filesystem requests transparently.

---

## 3. Container Specification (`GPACK/1` with ChaCha20-Poly1305)

The `GPACK/1` binary container stores files sequentially with an upfront index table and per-chunk AEAD tags.

### 3.1 Binary Layout
```text
+----------------------------------------------------------------+
| Container Header (64 bytes)                                    |
+----------------------------------------------------------------+
| Index Table (File Metadatas, Sizes, Hashes, Encrypted Flags)   |
+----------------------------------------------------------------+
| Chunk Payload #0 (Encrypted + 16-byte Poly1305 Tag)            |
+----------------------------------------------------------------+
| Chunk Payload #1 (Encrypted + 16-byte Poly1305 Tag)            |
+----------------------------------------------------------------+
| ...                                                            |
+----------------------------------------------------------------+
```

### 3.2 Container Header Structure
| Offset | Type | Field | Description |
|---|---|---|---|
| 0x00 | `u32` | `magic` | `0x4B435047` (`GPACK` in little-endian) |
| 0x04 | `u16` | `version` | `0x0001` (Version 1) |
| 0x06 | `u16` | `flags` | Bit 0: Encrypted (`1`), Bit 1: Streaming Linear Index |
| 0x08 | `u64` | `index_offset` | Byte offset where the file index table begins |
| 0x10 | `u64` | `index_size` | Length of the index table in bytes |
| 0x18 | `u64` | `payload_offset`| Byte offset where data chunks start |
| 0x20 | `u64` | `total_files` | Total number of files indexed |
| 0x28 | `u32` | `chunk_size` | Base chunk size (typically 65,536 bytes) |
| 0x2C | `[u8; 36]` | `reserved` | Reserved for future alignment / cryptographic salt |

### 3.3 Chunk AEAD Specification
- **Algorithm:** ChaCha20-Poly1305 (IETF RFC 8439).
- **Key:** 256-bit symmetric key ($K$).
- **Nonce Formulation (96-bit):**
  - Bytes 0..3: File Index in container (`u32` LE)
  - Bytes 4..7: Chunk Index within file (`u32` LE)
  - Bytes 8..11: Session / Salt Counter (`u32` LE)
- **Tag:** 128-bit Poly1305 MAC appended directly to the end of each chunk.

---

## 4. Key Derivation & Injection Architecture

Key generation is fully automated during the host build pipeline:

```text
[Build Configuration]
  - app.package (e.g. "com.example.lonarpg")
  - app.name    (e.g. "Lona RPG")
  - salt        (Static project salt b"runtimepack_salt_v1")
       │
       ▼
  HKDF-Extract(salt, IKM: app.package + "::" + app.name)
       │
       ▼
  HKDF-Expand(PRK, info: "gpack-content-v1", length: 32)
       │
       ▼
  256-bit Master Content Key
```

### Key Provisioning into Android Shell
1. **Rust Build Tool:** The build pipeline writes the derived key into an obfuscated native C/C++ header or injects it into a secure static symbol table inside `libmkxp-z.so`.
2. **Zero Plaintext Storage:** The key is never emitted into `AndroidManifest.xml`, `strings.xml`, or `build-manifest.json`.

---

## 5. Native Android C++ VFS Decryptor Hook (mkxp-z / PhysFS)

### 5.1 Native Layer Architecture
In `libmkxp-z.so`, all file access routes through PhysFS (PhysicsFS) abstraction or SDL_RWops streams:
- `PhysFS::openRead(path)`
- `SDL_RWFromFile(path, "rb")`
- Ruby `rb_f_open`, `File.binread`

### 5.2 Decryption Pipeline
```text
Engine requests: "Graphics/Characters/Lona.png"
                    │
                    ▼
          [PhysFS / VFS Driver]
                    │
   Lookup "Graphics/Characters/Lona.png" in Index Map (in-memory)
                    │
       Entry found: offset 0x48A000, size 42,100 bytes, chunk #18
                    │
                    ▼
       Seek in APK assets/game.pack
                    │
       Read encrypted chunk into 64KB RAM buffer
                    │
       ChaCha20-Poly1305 Decrypt In-Memory
                    │
       Verify 16-byte Poly1305 Tag
         ├─ Fails ──> Emit Diagnostic CNT-201 -> Abort Read
         └─ Passes ─> Return Plaintext Byte Pointer to Image Parser
                    │
 Plaintext buffer cleared from RAM once loaded into GPU texture
```

### 5.3 Fallback and Bootstrapping
- `runtimepack_bootstrap.rb` and `mkxp.json` are mounted as system overrides from `app/src/main/assets/` or decrypted on boot into memory before Ruby VM evaluation.
- `Scripts.rvdata2` is read as a memory stream directly by `libruby.so` during engine initialization.

---

## 6. Read-Only VFS vs. Writable Mounts

To guarantee immutability of the base container and prevent state corruption:

| Mount Point | Underlying Storage | Access Mode | Contents |
|---|---|---|---|
| `game:/` | `assets/game.pack` | **Read-Only** | Sprites, Maps, Audio, Scripts, System data |
| `save:/` | `filesDir/game/UserData/` | **Read-Write** | `Save01.rvdata2`, `SavQuick.rvdata2`, screenshots |
| `config:/` | `filesDir/game/UserData/` | **Read-Write** | `GameLona.ini`, audio volume, control bindings |
| `ipc:/` | `filesDir/` | **Read-Write** | `rp_input_state.bin`, `rp_cheat_cmd.json`, state dumps |

The game's native save and load scripts (`DataManager.save_game`, `DataManager.load_game`) write exclusively to `save:/` without touching `game.pack`.

---

## 7. Error Taxonomy & Anti-Tamper Handling

Errors related to content integrity and cryptographic streaming use the structured domain `CNT-xxx`:

| Diagnostic Code | Severity | Trigger Condition | Reaction |
|---|---|---|---|
| `CNT-101` | Fatal | Container magic `GPACK` invalid or version unsupported | Game aborts with explicit error dialog |
| `CNT-102` | Fatal | Index table size or checksum mismatch | Engine refuses to initialize |
| `CNT-201` | Fatal | Poly1305 tag verification failure (tampered chunk) | Read aborted, process terminates fail-closed |
| `CNT-202` | Recoverable | File not found in virtual index | Returns standard `ENOENT` / `nil` to engine |
| `CNT-203` | Fatal | Out of memory during chunk streaming | Graceful memory cleanup and recovery |

---

## 8. Verification & Acceptance Testing Plan

A feature implementing this specification is complete only when all criteria pass:

1. **Rust Workspace Tests:**
   - `test_encrypted_streaming_roundtrip`: Pack directory with ChaCha20-Poly1305 $\rightarrow$ verify 100% byte fidelity upon reading $\rightarrow$ tamper 1 byte $\rightarrow$ verify decryption failure.
2. **Android Execution Verification:**
   - `GpackExtractor` is modified or bypassed: `game.pack` remains in APK assets or is copied directly as a single binary without file extraction.
   - APK size and device storage consumption remain equal (~780 MB total disk usage).
3. **In-Game Playability (Lona RPG Benchmark):**
   - Game boots to Title Screen within 2 seconds without unpacking files.
   - Audio (BGM, SE) plays smoothly from encrypted chunks.
   - Map transitions and enemy spawns exhibit zero frame stuttering.
   - Saving and loading continue to function through `save:/`.
