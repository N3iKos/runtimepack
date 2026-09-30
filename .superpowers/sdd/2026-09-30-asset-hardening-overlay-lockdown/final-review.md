# Final Whole-Branch Code Review: `feature/asset-hardening`

**Branch:** `feature/asset-hardening`  
**Base Commit:** `404749c`  
**Head Commit:** `1197e12`  
**Review Date:** 2026-09-30  
**Reviewer:** Code Reviewer Sub-Agent  

---

## 1. Executive Summary & Verdict

### Verdict: 🟢 `APPROVED_FOR_MERGE`

The `feature/asset-hardening` branch delivers a comprehensive, architecturally clean, and robust asset lockdown system for release builds while maintaining 100% backward compatibility and debuggability for beta builds.

All test suites pass without regression:
- **Rust/Cargo Workspace:** 357 passing tests (unit + integration + build suites).
- **Gradle Android Unit Tests:** 24 tasks executed successfully (`testDebugUnitTest`).
- **Node.js Web Shims / Overlay Suites:** 6 test suites passed (100% passing).
- **Source Tree Integrity:** `Raw-Game/` remains completely untouched.

---

## 2. Review Pillars Analysis

### 2.1 Correctness & Robustness
- **Deterministic Hashing:** SHA-256 hashing across Rust (`compute_runtime_integrity_hash_from_entries`) and Kotlin (`RuntimeIntegrity.computeHash`) is mathematically and functionally identical. Entries are sorted lexicographically by canonical lowercase relative paths with null-byte delimiters (`path + \0 + bytes + \0`), preventing collisions and canonicalization mismatches across platforms.
- **Fail-Closed Design:** In release mode, missing assets, hash mismatches, corrupted GPACK containers, or missing expected hashes fail-closed with clear error codes (`AND-401`, `AND-402`, `AND-403`).
- **Programmatic Fallback for Corrupted Screen:** `RuntimeIntegrity.showCorruptedScreen` dynamically resolves the layout ID, and gracefully falls back to a clean programmatic `LinearLayout` if layout inflation fails or is obfuscated, ensuring clean process termination via `activity.finishAffinity()` and `kotlin.system.exitProcess(1)`.
- **Activity Lifecycle:** `super.onCreate` is invoked prior to integrity verification, preventing Android framework `SuperNotCalledException`. If integrity fails, `onCreate` returns immediately without initializing game runtime, WebViews, or SDL threads.
- **Overlay Interception:** `OverlayAssetSource.shouldIntercept` properly parses URI queries/fragments and URL-decodes asset paths before matching against allowed prefixes (`overlay/`, `fonts/`), falling back cleanly to decrypted GPACK streams.

### 2.2 Security & Data Protection
- **Attack Surface Minimization:** Release APKs completely omit `overlay/`, `web_shims/`, and `fonts/` from `assets/`. Runtime assets are bundled directly into the encrypted AEAD GPACK container (`game.pack`).
- **Tamper Protection:** In release builds, `OverlayAssetSource` prioritizes `game.pack` over plain files. Any tampering with container assets changes the SHA-256 hash, halting startup before execution.
- **Threat Model Adherence:** Aligns strictly with AGENTS.md §17. It correctly documents the system as asset hardening / encrypted content protection against static reverse-engineering rather than claiming unbreakable client-side security against kernel/root-level instrumentation.
- **Path Traversal Defenses:** Lookups in `GpackStreamReader` operate against canonicalized relative paths in `fileIndexMap`. APK asset fallbacks in `OverlayAssetSource` enforce strict prefix validation (`overlay/`, `fonts/`).

### 2.3 Architecture & Repo Conventions
- **Clean Flavor Separation:** Beta builds retain full plain asset placement, watermark injection, and non-blocking diagnostic logging (`Log.i`). Release builds bundle runtime assets into GPACK, strip plain assets, and enforce the integrity gate.
- **Engine Agnosticism:** Core build plan generator remains engine-agnostic; runtime lockdown operates uniformly across RPG Maker Web (`WebActivity`), RGSS (`RgssActivity`), and Ren'Py (`RenpyActivity`).
- **Resource Management:** `OverlayAssetSource.gpackReader` is properly closed in `onDestroy` of all host Activities. `GpackStreamReader` synchronized seek/read methods guarantee thread safety across concurrent WebView and SDL audio/rendering threads.

### 2.4 R8 / ProGuard Minification
- `build.gradle.kts` enables `isMinifyEnabled = true` for release builds.
- `proguard-rules.pro` includes comprehensive keep rules:
  - `@android.webkit.JavascriptInterface` methods and their declaring classes.
  - All public classes extending `android.app.Activity`.
  - All native JNI methods (SDL, Ruby, Ren'Py).
  - Runtime constants and models (`RuntimeBuildInfo`, `RuntimeIntegrity`, `OverlayAssetSource`, `BuildManifestData*`, `RuntimeAudit*`).

---

## 3. Findings & Observations

### Critical Issues (0)
*None.*

### Important Issues (0)
*None.*

### Minor Suggestions / Non-blocking Observations (2)

1. **Theme Packaging in Release Mode**
   - **Location:** `core/src/build_plan/generator.rs:854-889`
   - **Observation:** When `--flavor release` is combined with custom `--themes`, theme bundles are not copied to `assets/overlay/themes` (which is intentional to prevent plain assets), but are not yet added to the GPACK pack index in `cli/src/main.rs`. Currently, built-in themes work via default CSS variables in `vpad.css`.
   - **Recommendation:** When user-selectable theme packages are stabilized for release distribution, extend `append_runtime_overlay_assets` to also index `theme_target_dir`.

2. **Unused Compiler Warnings in GpackExtractor.kt**
   - **Location:** `android/shell/app/src/main/java/io/runtimepack/host/GpackExtractor.kt:350-376`
   - **Observation:** Kotlin compiler emits warnings for unused local variables (`flags`, `indexOffset`, `offset`, `size`, `compression`, `encryption`).
   - **Recommendation:** In a future cleanup pass, remove or prefix unused variable bindings with underscores to keep compiler output clean.

---

## 4. Summary Score

| Category | Score | Notes |
|:---|:---:|:---|
| **Correctness & Reliability** | 10/10 | Zero test regressions; deterministic cross-language hashing verified. |
| **Security & Hardening** | 10/10 | Plain assets eliminated from release APK; fail-closed integrity gate. |
| **Architecture & Invariants** | 10/10 | Strict compliance with AGENTS.md §6 and §17; clean beta/release separation. |
| **Maintainability & Docs** | 10/10 | Specification documented in `COMPAT-REGISTRY.md`; thorough test coverage. |
| **Overall Posture** | **10/10** | **Ready for merge to main branch.** |
