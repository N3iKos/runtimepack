# Specification: Multi-Game Stabilization & Guardrail Removal

**Date:** 2026-09-26  
**Status:** Approved  
**Scope:** Universal RuntimePack v1 Pipeline & 8 Target Games in `Raw-Game/`  

---

## 1. Context & Philosophy

RuntimePack is a build-time compiler and packaging platform, designed to convert desktop/web games into standalone Android packages.

### Principle 1: Compiler Model, Not Policy Enforcer
Compilers (like GCC, Clang, Rustc, Webpack) emit warnings and diagnostics, but they do not arbitrarily refuse to compile code based on external licensing terms or the presence of unused desktop wrapper libraries. RuntimePack informs developers transparently, but honors the developer's intent to build.

### Principle 2: No Hard Guardrails
Artificial hard blockers that refuse to package games despite user override flags (`--accept-audit-fail`, `--accept-audit-unknown`) are eliminated:
1. **Tyrano Redistribution Gate (`TYR-101`):** Converted from a hard blocker to an informational finding. Tyrano packaging proceeds normally.
2. **Windows Desktop DLLs (`CMP-102`):** Inert Windows DLLs in portable engines (e.g. Ren'Py `lib/py3-windows-x86_64/*.dll`, `steam_api64.dll`) are recognized as desktop artifacts, excluded from Android packaging, and downgraded from hard build blockers to warnings.
3. **Gradle Project Name Sanitization:** Illegal characters in project names (e.g. `:` in `"Fear & Hunger 2: Termina"`) are automatically sanitized for Gradle's `rootProject.name` while preserving the application display name.

### Principle 3: Game-by-Game Stabilization
Each target game is audited, diagnosed, patched with appropriate engine shims or asset resolvers, compiled into an APK, and re-audited until it reaches a clean or minor-only status.

---

## 2. Target Game Queue & Diagnostics Baseline

| # | Game | Engine | Current Issue | Stabilization Target |
|:---:|:---|:---|:---|:---|
| **0** | **Core Guardrail Removal** | Core / CLI | `TYR-101` hard block, `CMP-102` hard block, Gradle colon crash | Unblock all games from artificial build refusals |
| **1** | **Fear & Hunger 2: Termina** | RPG Maker MV | Gradle syntax crash (`:`), 571 missing assets, 15 case mismatches | Clean build, Gradle name sanitized, case mismatches & missing assets handled |
| **2** | **[Kimochi] NTRdemic** | TyranoScript | `TYR-101` legal block, 438 missing assets, 92 dynamic expressions | ASAR extraction, dynamic expression fallback, APK build |
| **3** | **[Kimochi] show_off** | TyranoScript | `TYR-101` legal block, 891 path traversal (`../`), 34 missing assets | Path traversal normalization, ASAR limits handling |
| **4** | **Mizuki And Me** | Ren'Py | `CMP-102` Windows DLLs, missing python imports | Windows DLL exclusion, Android Python runtime resolution |
| **5** | **Forestia** | RPG Maker MZ | 22 plugin root dependencies, 1 case mismatch | Subfolder plugin resolution (`js/plugins/develop/`) |
| **6** | **The Witch's House MV** | RPG Maker MV | 34 missing assets (audio/img), 5 case mismatches | Default RTP/MV asset resolution and case normalization |
| **7** | **Hello Charlotte EP1** | RPG Maker VX Ace | `Game.rgss3a` encrypted archive (`INDETERMINATE`) | Encrypted archive validation and boot stability |
| **8** | **To the Moon** | RPG Maker XP | `To the Moon.rgssad` encrypted archive (`INDETERMINATE`) | RGSS1 archive validation and boot stability |

---

## 3. Technical Requirements

### 3.1 Phase 0: Guardrail Removal

1. **`core/src/build_plan/audit_policy.rs` & `cli/src/main.rs`:**
   - In `evaluate_audit_policy`: Remove `TYR_LEGAL_GATE_BLOCKED` (`TYR-101`) from `hard_blockers`. It remains an informational finding in audit reports but does not block build generation.
   - When `--accept-audit-fail` or `--accept-audit-unknown` is passed, allow builds to proceed even if findings exist.
2. **`engines/renpy/src/scanner.rs` & `core/src/compatibility/mod.rs`:**
   - Windows PE DLLs under `lib/py3-windows-x86_64/` are classified as `CMP-DESKTOP-EXCLUDED` (warning), not `CMP-102` (fatal blocker), matching how `nw.dll` and `node.dll` are handled in RPG Maker MV/MZ.
3. **`core/src/build_plan/generator.rs`:**
   - In `generate_settings_gradle`: Sanitize `rootProject.name` using regex `[^a-zA-Z0-9_-]` -> replace with `_` or strip, preventing Gradle `InvalidUserDataException`.

### 3.2 Plan 1: Fear & Hunger 2: Termina Stabilization

1. **Gradle Project Name Sanitization:**
   `"Fear & Hunger 2: Termina"` -> `rootProject.name = "Fear_Hunger_2_Termina"` in `settings.gradle.kts`.
2. **Case Sensitivity Normalization:**
   Handle 15 case mismatches (e.g. `Occultist1_1.rpgmvp` vs `occultist1_1.rpgmvp`) in `VfsResourceInterceptor.kt` and static audit resolver.
3. **Missing Asset Shimming:**
   Ensure transparent/dummy fallbacks in `VfsResourceInterceptor.kt` prevent game freeze or webview error on 571 missing assets.
4. **Build Verification:**
   Produce valid `dist/termina/Termina.apk` and verify build success.

---

## 4. Execution Workflow

1. Document approved design spec.
2. Produce detailed implementation plan for Phase 0 + Game 1 (*Fear & Hunger 2: Termina*).
3. Execute via Sub-Agent following SDD and TDD.
4. Verify build succeeds and audit errors are classified.
5. Advance sequentially to next game plan.
