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

## 4. Global Constraints

- Source games in `Raw-Game/` remain strictly 100% immutable.
- All autonomous builds use `--flavor beta`.
- All runtime verification performed on the MuMu emulator (`127.0.0.1:16384`).
- Third-party native binaries require pinned version + provenance + license notice before bundling.
- Each track ends only when the game boots and is playable on the emulator.
