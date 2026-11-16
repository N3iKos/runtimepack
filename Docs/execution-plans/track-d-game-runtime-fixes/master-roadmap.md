# Master Roadmap: Track D — Game Runtime Fixes & Universal Runtime Infrastructure

**Date:** 2026-09-29  
**Status:** Approved  
**Spec:** `Docs/execution-plans/track-d-game-runtime-fixes/design.md`

---

## 1. Objective

Make ported games playable, normal, and stable across all supported engines on Android (MuMu emulator), followed by hardening the universal VFS and overlay layer to guarantee zero regressions for future ports. All implementation work is executed strictly sequentially and delegated to sub-agents.

## 2. Execution Order & Gates

```text
Track 1: Show Off (TyranoScript)               [COMPLETED & PLAYABLE]
   │  Gate: ASAR 3-byte offset fix + SHA-256 integrity gate. Verified on MuMu.
   ▼
Track 2: Mizuki And Me (Ren'Py 8 / Python 3.9) [COMPLETED & PLAYABLE]
   │  Gate: Official RAPT 8.3.7 arm64 runtime + RenpyBootstrapActivity + pyjnius bridge. Verified on MuMu.
   ▼
Track 3: Ren'Py Overlay & Touch Routing        [COMPLETED]
   │  Gate: Transparent overlayWebView on mLayout, RuntimePackVPad bridge,
   │        precision hit-tested touch pass-through to mSurface, lifecycle sync.
   ▼
Track 4: 4 Pillars of Universal RPG Maker VFS  [COMPLETED]
   │  Gate: In-memory readdirSync, fast stat/exists bridge, O(1) VFS HashMap resolver,
   │        VFS-aware AssetProbeRunner, and compiler path sanitizer fix.
   ▼
Done
```

## 3. Plan Directory

| Track | Plan Document | Scope | Status |
| :---: | :--- | :--- | :---: |
| 1 | `./track-1-show-off-plan.md` | TyranoScript ASAR offset fix + integrity gate | **COMPLETED** |
| 2 | `./track-2-mizuki-renpy-plan.md` | Official RAPT 8.3.7 runtime + `RenpyActivity.kt` | **COMPLETED** |
| 3 | `./track-3-renpy-overlay-and-rpgm-universal-vfs-plan.md` (Part 1) | Ren'Py overlay integration & touch routing | **COMPLETED** |
| 4 | `./track-3-renpy-overlay-and-rpgm-universal-vfs-plan.md` (Part 2) | 4 Pillars of Universal RPG Maker VFS | **COMPLETED** |

## 4. Governance

- One-by-one execution: sub-agents execute tasks incrementally with strict review gates.
- `Raw-Game/` directories remain strictly 100% immutable.
- All APK builds use `--flavor beta`.
- All native bridge, shims, and compiler changes must have automated unit tests before emulator verification.
