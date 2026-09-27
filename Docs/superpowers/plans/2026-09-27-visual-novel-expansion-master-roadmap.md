# Master Roadmap: Visual Novel Engines Expansion (Kirikiri 2/Z & Visual Novel Maker)

**Date:** 2026-09-27  
**Status:** Approved  
**Specs:**
- `Docs/superpowers/specs/2026-09-27-kirikiri-complete-architecture-spec.md`
- `Docs/superpowers/specs/2026-09-27-visual-novel-maker-support-design.md`

---

## 1. Executive Summary & Objective

This master roadmap establishes the end-to-end plan for expanding RuntimePack to support major dedicated Japanese Visual Novel engines:
1. **Kirikiri 2 / Kirikiri Z (KAG / TVP Engine):** Native C++ runtime (`libkrkr2.so` via NDK r28, 16 KB page-size alignment) with in-memory `.xp3` binary archive parsing and direct-boot execution (zero file browser UI).
2. **Visual Novel Maker (VNM):** WebGL/HTML5 engine compiled to dedicated standalone Android APKs via the existing high-performance **Web Runtime (`WebActivity.kt`)** and streaming decrypter, bypassing Apache Cordova.

Both engines produce dedicated single-game APKs that boot directly into the game title screen upon tapping the Android launcher icon.

---

## 2. Track & Phase Decomposition

```text
Track A: Visual Novel Maker (VNM) [Fast-Track: High Feasibility, WebGL/NW.js Reuse]
└── Plan: Docs/superpowers/plans/2026-09-27-visual-novel-maker-support-plan.md
    ├── Task 1: Domain & Engine Registration (EngineId::VisualNovelMaker, RuntimeFamily::Web)
    ├── Task 2: Static Detector (engines/vnmaker, signature, data/System.json, package.json)
    ├── Task 3: Static Asset Reachability Auditor (data/Scenes.json, resources/)
    └── Task 4: Generator & CLI Wiring (WebActivity.kt, game.pack, VN VPad overlay)

Track B: Kirikiri 2 / Kirikiri Z [Deep Native C++ Engine Pipeline]
├── Phase 1: Rust Host Tooling & In-Memory XP3 Parser
│   └── Plan: Docs/superpowers/plans/2026-09-27-kirikiri-engine-support-phase-1-plan.md
├── Phase 2: Compatibility Scanner & Plugin Validator
│   └── Plan: Docs/superpowers/plans/2026-09-27-kirikiri-engine-support-phase-2-plan.md
├── Phase 3: Headless Native C++ Runtime & Android Shell (libkrkr2.so, 16KB ELF, KirikiriActivity)
│   └── Plan: Docs/superpowers/plans/2026-09-27-kirikiri-engine-support-phase-3-plan.md
└── Phase 4: Packaging Pipeline, VN VPad Overlay & End-to-End Build
    └── Plan: Docs/superpowers/plans/2026-09-27-kirikiri-engine-support-phase-4-plan.md
```

---

## 3. Directory of Plans

| Track / Phase | Plan File | Engine & Target Runtime | Status |
| :--- | :--- | :--- | :---: |
| **Track A (VNM)** | `Docs/superpowers/plans/2026-09-27-visual-novel-maker-support-plan.md` | Visual Novel Maker (WebActivity / Chromium) | **Ready** |
| **Track B - Phase 1** | `Docs/superpowers/plans/2026-09-27-kirikiri-engine-support-phase-1-plan.md` | Kirikiri (Rust XP3 parser & detector) | **Ready** |
| **Track B - Phase 2** | `Docs/superpowers/plans/2026-09-27-kirikiri-engine-support-phase-2-plan.md` | Kirikiri (Plugins.link & XP3 filter probe) | **Ready** |
| **Track B - Phase 3** | `Docs/superpowers/plans/2026-09-27-kirikiri-engine-support-phase-3-plan.md` | Kirikiri (C++ libkrkr2.so & KirikiriActivity) | **Ready** |
| **Track B - Phase 4** | `Docs/superpowers/plans/2026-09-27-kirikiri-engine-support-phase-4-plan.md` | Kirikiri (Packaging, VPad VN, full APK) | **Ready** |

---

## 4. Execution Policy & Constraints

1. **Independent Parallel Execution:** Track A (Visual Novel Maker) and Track B (Kirikiri) are completely decoupled. Track A can be executed first as a fast-win without waiting for C++ NDK compilation.
2. **Strict Immortality:** Input games in `Raw-Game/` remain strictly 100% immutable.
3. **Mandatory Flavor Beta:** Autonomous builds must always enforce `--flavor beta`.
4. **Verification Requirement:** Each task must be accompanied by comprehensive tests and 100% workspace passing status before being marked completed.
