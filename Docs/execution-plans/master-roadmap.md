# Master Roadmap: Visual Novel Engines Expansion (VNM, Kirikiri 2/Z & Artemis Engine)

**Date:** 2026-09-27  
**Status:** Approved  
**Specs:**
- `Docs/execution-plans/track-a-visual-novel-maker/design.md`
- `Docs/execution-plans/track-b-kirikiri/architecture-spec.md`
- `Docs/execution-plans/track-c-artemis/design.md`

---

## 1. Executive Summary & Objective

This master roadmap establishes the complete architecture and delivery plan for expanding RuntimePack to support the **Top 3 Dedicated Japanese Visual Novel Engines**:

1. **Visual Novel Maker (VNM):** HTML5/WebGL visual novel suite (Degica / Gotcha Gotcha Games). Directly compiled into standalone Android APKs via the high-performance **Web Runtime (`WebActivity.kt`)**, streaming VFS decrypter, and NW.js polyfills (bypassing Apache Cordova).
2. **Kirikiri 2 / Kirikiri Z (KAG / TVP Engine):** C++/TJS2 visual novel engine. Powered by native `libkrkr2.so` (NDK r28, 16 KB page-size alignment) with in-memory 11-byte `.xp3` binary parsing and direct-boot execution (zero file browser UI).
3. **Artemis Engine (PFS / Lua):** Lua-scripted visual novel engine (iMel Inc. / Mikage) behind 730+ commercial visual novels. Powered by in-memory `.pfs` (PF8, PF6, PF2) binary parsing with SHA-1 XOR stream cipher decryption in Rust, coupled with the clean `art3m1s-core` C FFI native runtime (`ArtemisActivity.kt`) and save data isolation.

All 3 engine tracks guarantee **dedicated, direct-boot standalone APKs**:
```text
Install APK -> Tap Launcher Icon -> Direct Boot to Title Screen (Zero File Browser / Zero Multi-Engine Launcher)
```

---

## 2. Track & Phase Architecture

```text
Track A: Visual Novel Maker (VNM) [Fast-Track: High Feasibility, WebGL/NW.js Reuse]
└── Plan: Docs/execution-plans/track-a-visual-novel-maker/plan.md
    ├── Task 1: Domain & Engine Registration (EngineId::VisualNovelMaker, RuntimeFamily::Web)
    ├── Task 2: Static Detector (engines/vnmaker, signature, data/System.json, package.json)
    ├── Task 3: Static Asset Reachability Auditor (data/Scenes.json, resources/)
    └── Task 4: Generator & CLI Wiring (WebActivity.kt, game.pack, VN VPad overlay)

Track B: Kirikiri 2 / Kirikiri Z [Deep Native C++ Engine Pipeline]
├── Phase 1: Rust Host Tooling & In-Memory XP3 Parser
│   └── Plan: Docs/execution-plans/track-b-kirikiri/phase-1-plan.md
├── Phase 2: Compatibility Scanner & Plugin Validator
│   └── Plan: Docs/execution-plans/track-b-kirikiri/phase-2-plan.md
├── Phase 3: Headless Native C++ Runtime & Android Shell (libkrkr2.so, 16KB ELF, KirikiriActivity)
│   └── Plan: Docs/execution-plans/track-b-kirikiri/phase-3-plan.md
└── Phase 4: Packaging Pipeline, VN VPad Overlay & End-to-End Build
    └── Plan: Docs/execution-plans/track-b-kirikiri/phase-4-plan.md

Track C: Artemis Engine (PFS / Lua) [In-Memory PFS & art3m1s-core Native Runner]
└── Plan: Docs/execution-plans/track-c-artemis/plan.md
    ├── Task 1: Domain & Engine Registration (EngineId::Artemis, RuntimeFamily::ArtemisNative)
    ├── Task 2: In-Memory PFS Parser (PF8/PF6/PF2 with SHA-1 XOR stream cipher & multi-volume precedence)
    ├── Task 3: Static Detector & AST Script Auditor (root.pfs, system.ini, .ast token parsing)
    └── Task 4: Android Shell Activity & Generator Wiring (art3m1s-core C FFI, ArtemisActivity.kt)
```

---

## 3. Directory of Expansion Plans

| Track / Phase | Plan File | Engine & Target Runtime | Status |
| :--- | :--- | :--- | :---: |
| **Track A (VNM)** | `Docs/execution-plans/track-a-visual-novel-maker/plan.md` | Visual Novel Maker (WebActivity / Chromium) | **Ready** |
| **Track B - Phase 1** | `Docs/execution-plans/track-b-kirikiri/phase-1-plan.md` | Kirikiri (Rust XP3 parser & detector) | **Ready** |
| **Track B - Phase 2** | `Docs/execution-plans/track-b-kirikiri/phase-2-plan.md` | Kirikiri (Plugins.link & XP3 filter probe) | **Ready** |
| **Track B - Phase 3** | `Docs/execution-plans/track-b-kirikiri/phase-3-plan.md` | Kirikiri (C++ libkrkr2.so & KirikiriActivity) | **Ready** |
| **Track B - Phase 4** | `Docs/execution-plans/track-b-kirikiri/phase-4-plan.md` | Kirikiri (Packaging, VPad VN, full APK) | **Ready** |
| **Track C (Artemis)** | `Docs/execution-plans/track-c-artemis/plan.md` | Artemis Engine (art3m1s-core C FFI / Android) | **Ready** |

---

## 4. Execution Policy & Governance

1. **Independent Decoupled Tracks:** Each track is architecturally self-contained. Track A (VNM), Track B (Kirikiri), and Track C (Artemis) can be executed in any order according to priority.
2. **Strict Immortality:** Input games in `Raw-Game/` remain strictly 100% immutable.
3. **Mandatory Flavor Beta:** Autonomous builds must always enforce `--flavor beta`.
4. **Zero-Disk Extraction for Host Inspection:** All archive parsing (XP3, PFS) and script reachability analysis operate strictly in-memory.
5. **100% Test Passing Gate:** No phase is marked completed without passing all integration tests across Cargo and Android Gradle.
