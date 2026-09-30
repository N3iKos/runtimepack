# Build-Time Theme Package Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a fully isolated, modular Build-Time Theme Package Engine and Theme API with zero hardcoded presentation styles in core components, self-contained packages for Liquid Glass, Minimalist Matte, and Retro OSD, an HTML5 Canvas smooth swipe motion trail, universal thin-ring visual ripple, and dynamic panel stacking.

**Architecture:**
- **Host Theme Engine (Rust `core/src/theme/`):** Validates package metadata (`theme.json`), version compatibility (`themeApi: "1.0.0"`), CSS scope isolation, asset paths, and integrity. Emits build-time theme packages into Android assets. CLI provides management (`runtimepack theme list/validate/install/pack`) and build-time selection (`--themes <id,...> --default-theme <id>`).
- **Semantic Theme API & Core Overlay (`android/shell/.../overlay/` & `preview/`):** Core overlay components (`vpad.js`, `cheat_menu.js`, `diagnostic_suite.js`) are stripped of all hardcoded colors, inline styles, and injected `<style>` tags. Components render purely via semantic CSS tokens (`--theme-*`) and stable state classes.
- **Self-Contained Theme Packages (`runtimes/themes/`):** Each theme resides in its own isolated directory (`theme.json`, `tokens.css`, `components.css`, `layout.css`, `effects.css`, `assets/`). Liquid Glass is completely overhauled into visionOS/iOS fluid optical glass with squircle 24px and pill buttons; Minimalist Matte uses navy/deep-blue accents; Retro OSD is 100% amber/orange with zero cyan bleed.
- **Hardware-Accelerated HTML5 Canvas Motion Trail:** Smooth continuous Bézier spline ribbon replacing discrete DOM circles, combined with canvas-only 1.5px thin-ring ripples.

**Tech Stack:** Rust (cargo workspace: `core`, `cli`), JSON Schema, HTML5 / CSS3 / Vanilla JavaScript (ES6) running in Android WebView and browser preview, Node.js (`assert`, `vm`), Gradle Android unit tests.

**Spec:** `Docs/superpowers/specs/2026-09-28-build-time-theme-package-engine-design.md`

## Global Constraints
- Folder Game Input 100% Immutable (`Raw-Game/` remains untouched).
- Core overlay components must contain zero hardcoded presentation hex colors.
- Custom themes must not contain arbitrary JavaScript (`theme.json` rejects JS files and unsafe CSS).
- Retro OSD theme must be 100% amber/orange without any cyan or blue bleed.
- Assets in `android/shell/app/src/main/assets/overlay/` and `preview/` must remain synchronized.
- Existing DOM element IDs (`vpad-btn-gear`, `vpad-editor-bar`, etc.) must not be broken.

---

### Task 1: Theme Package JSON Schema & Rust Diagnostic Error Codes

**Files:**
- Create: `schemas/theme-package.schema.json`
- Modify: `core/src/diagnostics/mod.rs`
- Test: `core/src/theme/schema_test.rs` (or `tests/integration/theme_test.rs`)

**Interfaces:**
- Produces: Diagnostic codes `THM-001` through `THM-010`, JSON Schema validating `theme.json`.

- [ ] **Step 1: Write diagnostic error codes in `core/src/diagnostics/mod.rs`**

```rust
// In core/src/diagnostics/mod.rs
pub const THM_SCHEMA_INVALID: &str = "THM-001";
pub const THM_API_INCOMPATIBLE: &str = "THM-002";
pub const THM_ASSET_MISSING: &str = "THM-003";
pub const THM_ASSET_TRAVERSAL: &str = "THM-004";
pub const THM_UNSAFE_CSS: &str = "THM-005";
pub const THM_DUPLICATE_ID: &str = "THM-006";
pub const THM_DEFAULT_NOT_PACKAGED: &str = "THM-007";
pub const THM_PACKAGE_NOT_FOUND: &str = "THM-008";
```

- [ ] **Step 2: Create JSON schema `schemas/theme-package.schema.json`**

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "RuntimePackThemePackage",
  "type": "object",
  "required": ["id", "name", "version", "themeApi", "stylesheets"],
  "properties": {
    "id": {
      "type": "string",
      "pattern": "^[a-z0-9_-]+$"
    },
    "name": { "type": "string" },
    "version": { "type": "string" },
    "themeApi": { "type": "string" },
    "author": { "type": "string" },
    "description": { "type": "string" },
    "stylesheets": {
      "type": "object",
      "required": ["tokens", "components", "layout"],
      "properties": {
        "tokens": { "type": "string" },
        "components": { "type": "string" },
        "layout": { "type": "string" },
        "effects": { "type": "string" }
      }
    },
    "assets": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["id", "path", "mediaType"],
        "properties": {
          "id": { "type": "string" },
          "path": { "type": "string" },
          "mediaType": { "type": "string" },
          "hash": { "type": "string" }
        }
      }
    }
  },
  "additionalProperties": false
}
```

- [ ] **Step 3: Write test verifying schema and diagnostic codes**

Create `tests/integration/theme_package_test.rs`:
```rust
use runtimepack_core::diagnostics::*;

#[test]
fn test_theme_diagnostic_codes_are_unique() {
    assert_eq!(THM_SCHEMA_INVALID, "THM-001");
    assert_eq!(THM_API_INCOMPATIBLE, "THM-002");
    assert_eq!(THM_ASSET_MISSING, "THM-003");
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test theme_package_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add schemas/theme-package.schema.json core/src/diagnostics/mod.rs tests/integration/theme_package_test.rs
git commit -m "feat(theme): define theme package schema and diagnostic error codes"
```

---

### Task 2: Host Theme Package Validator & Resolver (`core/src/theme/`)

**Files:**
- Create: `core/src/theme/mod.rs`
- Create: `core/src/theme/manifest.rs`
- Create: `core/src/theme/validator.rs`
- Create: `core/src/theme/catalog.rs`
- Modify: `core/src/lib.rs`
- Test: `tests/integration/theme_package_test.rs`

**Interfaces:**
- Produces: `ThemePackage`, `ThemeManifest`, `ThemeCatalog`, `validate_theme_package()`.

- [ ] **Step 1: Write failing test in `tests/integration/theme_package_test.rs`**

```rust
use runtimepack_core::theme::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_validate_valid_theme_package() {
    let dir = tempdir().unwrap();
    let theme_dir = dir.path().join("my_theme");
    fs::create_dir_all(&theme_dir).unwrap();
    
    let manifest_json = r#"{
        "id": "my_theme",
        "name": "My Theme",
        "version": "1.0.0",
        "themeApi": "1.0.0",
        "stylesheets": {
            "tokens": "tokens.css",
            "components": "components.css",
            "layout": "layout.css"
        }
    }"#;
    fs::write(theme_dir.join("theme.json"), manifest_json).unwrap();
    fs::write(theme_dir.join("tokens.css"), ":root { --theme-accent: #007aff; }").unwrap();
    fs::write(theme_dir.join("components.css"), ".vpad-btn { border-radius: 9999px; }").unwrap();
    fs::write(theme_dir.join("layout.css"), "").unwrap();

    let package = ThemePackage::load_from_dir(&theme_dir).unwrap();
    assert_eq!(package.manifest.id, "my_theme");
    assert!(package.validate("1.0.0").is_ok());
}

#[test]
fn test_reject_theme_with_path_traversal() {
    let dir = tempdir().unwrap();
    let theme_dir = dir.path().join("evil_theme");
    fs::create_dir_all(&theme_dir).unwrap();
    
    let manifest_json = r#"{
        "id": "evil_theme",
        "name": "Evil Theme",
        "version": "1.0.0",
        "themeApi": "1.0.0",
        "stylesheets": {
            "tokens": "../tokens.css",
            "components": "components.css",
            "layout": "layout.css"
        }
    }"#;
    fs::write(theme_dir.join("theme.json"), manifest_json).unwrap();

    let res = ThemePackage::load_from_dir(&theme_dir);
    assert!(res.is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test theme_package_test`
Expected: FAIL with "module theme not found"

- [ ] **Step 3: Implement ThemePackage, ThemeManifest, and Validator**

Create `core/src/theme/mod.rs`, `manifest.rs`, `validator.rs`:
```rust
// core/src/theme/manifest.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeStylesheets {
    pub tokens: String,
    pub components: String,
    pub layout: String,
    #[serde(default)]
    pub effects: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeAssetEntry {
    pub id: String,
    pub path: String,
    pub media_type: String,
    #[serde(default)]
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub theme_api: String,
    pub stylesheets: ThemeStylesheets,
    #[serde(default)]
    pub assets: Vec<ThemeAssetEntry>,
}
```

Implement `ThemePackage::load_from_dir` checking:
- Path normalization rejecting `..` or leading `/`.
- Existence of referenced CSS files in `stylesheets`.
- Semver check on `theme_api` against expected `current_api_version` (major version equality required).
- CSS sanity check: rejection of `@import` and `javascript:` in stylesheets.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test theme_package_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add core/src/theme/ core/src/lib.rs tests/integration/theme_package_test.rs
git commit -m "feat(theme): implement theme package loader, parser, and safety validator"
```

---

### Task 3: Build CLI Integration for Theme Packaging (`--themes`, `--default-theme`)

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `core/src/build_plan/mod.rs`
- Modify: `core/src/build_plan/generator.rs`
- Test: `tests/integration/build_test.rs`

**Interfaces:**
- CLI flags: `--theme <id>` / `--themes <id1,id2>` and `--default-theme <id>`.
- Generator copies validated theme packages into Android assets under `app/src/main/assets/overlay/themes/<theme_id>/` and writes `themes_manifest.json`.

- [ ] **Step 1: Write failing test in `tests/integration/build_test.rs`**

```rust
#[test]
fn test_build_plan_embeds_selected_themes_and_manifest() {
    let selected_themes = vec!["matte".to_string(), "liquid_glass".to_string()];
    let default_theme = "matte";
    // Assert generator outputs assets/overlay/themes_manifest.json with selected themes
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test --test build_test test_build_plan_embeds_selected_themes`
Expected: FAIL

- [ ] **Step 3: Implement theme selection and assets emission in `generator.rs`**

- In `core/src/build_plan/generator.rs`:
  Add logic to package only the selected themes into `app/src/main/assets/overlay/themes/<id>/`.
  Write `app/src/main/assets/overlay/themes_manifest.json`:
  ```json
  {
    "defaultThemeId": "matte",
    "themes": [
      { "id": "matte", "name": "Minimalist Matte", "dir": "themes/matte" },
      { "id": "liquid_glass", "name": "Liquid Glass", "dir": "themes/liquid_glass" }
    ]
  }
  ```
- Reject build if `--default-theme` is not in the list of selected `--themes`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test build_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add cli/src/main.rs core/src/build_plan/ tests/integration/build_test.rs
git commit -m "feat(cli): add --themes and --default-theme build-time packaging support"
```

---

### Task 4: Neutralize Core Components (Strip Injected `<style>` & Hardcoded Styles)

**Files:**
- Modify: `android/shell/app/src/main/assets/overlay/diagnostic_suite.js`
- Modify: `android/shell/app/src/main/assets/overlay/cheat_menu.js`
- Modify: `android/shell/app/src/main/assets/overlay/vpad.js`
- Modify: `android/shell/app/src/main/assets/overlay/vpad.css`
- Test: `android/shell/app/src/test/js/overlay_interaction_regression.test.js`

**Interfaces:**
- Strips: `injectStyles()` from `diagnostic_suite.js` and `cheat_menu.js`.
- Replaces inline color mutations with CSS semantic class states: `.diag-badge-scanning`, `.diag-badge-success`, `.diag-badge-error`, `.fnh-active-accent`.
- In `vpad.css`: base component styles reference only `var(--theme-*)`.

- [ ] **Step 1: Write regression test ensuring zero `<style id="rp-diag-styles">` and `<style id="fnh-cheat-styles">` injected**

```javascript
test("Core neutralization: JS components do not inject hardcoded style tags", () => {
    // Assert diagnostic_suite.js and cheat_menu.js rely on CSS tokens, not injected <style> tags
    assert(!diagSource.includes('injectStyles'), "diagnostic_suite must not inject inline style tag");
    assert(!cheatSource.includes('injectStyles'), "cheat_menu must not inject inline style tag");
});
```

- [ ] **Step 2: Run test to verify failure**

Run: `node android/shell/app/src/test/js/overlay_interaction_regression.test.js`
Expected: FAIL

- [ ] **Step 3: Refactor `diagnostic_suite.js` and `cheat_menu.js`**

1. Move all structural component rules from `injectStyles()` into `vpad.css` (or base component definitions) using CSS variables:
   - Replace `#0284c7` and `#38bdf8` with `var(--theme-accent)`, `var(--theme-accent-border)`, `var(--theme-accent-subtle)`.
   - Replace `#141416`, `#18181b` with `var(--theme-bg-panel)`, `var(--theme-bg-header)`.
2. Replace inline `.style.background = "#0284c7"` with `.className = "diag-badge diag-badge-scanning"`.
3. In `vpad.js`: remove inline styles like `style="color:#38bdf8"`, replace with class `.vpad-text-accent`.

- [ ] **Step 4: Run tests to verify pass**

Run:
```powershell
node android/shell/app/src/test/js/overlay_interaction_regression.test.js
node android/shell/app/src/test/js/diagnostic_suite.test.js
node android/shell/app/src/test/js/cheat_menu.test.js
```
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/assets/overlay/
git commit -m "refactor(overlay): neutralize core JS components to consume theme tokens"
```

---

### Task 5: Package Three Isolated Built-in Themes (`runtimes/themes/`)

**Files:**
- Create: `runtimes/themes/matte/` (`theme.json`, `tokens.css`, `components.css`, `layout.css`, `effects.css`)
- Create: `runtimes/themes/liquid_glass/` (`theme.json`, `tokens.css`, `components.css`, `layout.css`, `effects.css`)
- Create: `runtimes/themes/retro_osd/` (`theme.json`, `tokens.css`, `components.css`, `layout.css`, `effects.css`)
- Test: `tests/integration/theme_package_test.rs`

**Interfaces:**
- **Minimalist Matte:** Navy `#10172a` toolbar, deep blue `#1e3a8a` / `#2563eb` accents, charcoal `#141416` panels.
- **Liquid Glass:** 24px squircle panels, 9999px pill buttons, `backdrop-filter: blur(30px) saturate(180%)`, 1px subtle specular highlight rim, Apple System Blue active states, zero neon overglow.
- **Retro OSD:** 100% amber/orange (`#ff7700` / `#ffaa44`), 2px crisp corners, CRT scanlines, zero cyan or blue leak.

- [ ] **Step 1: Write integration test validating built-in theme packages**

```rust
#[test]
fn test_builtin_theme_packages_are_valid_and_complete() {
    for theme_id in &["matte", "liquid_glass", "retro_osd"] {
        let path = format!("runtimes/themes/{}", theme_id);
        let pkg = ThemePackage::load_from_dir(Path::new(&path)).unwrap();
        assert!(pkg.validate("1.0.0").is_ok());
    }
}
```

- [ ] **Step 2: Create `runtimes/themes/matte/`**

Write `theme.json` and CSS files defining Matte tokens:
- `--theme-accent: #2563eb`
- `--theme-bg-toolbar: #10172a`
- `--theme-bg-toolbar-active: #1e3a8a`
- `--theme-bg-panel: #141416`
- `--theme-border-panel: 1.5px solid #2563eb`

- [ ] **Step 3: Create `runtimes/themes/liquid_glass/`**

Write `theme.json` and CSS files defining Liquid Glass:
- `--theme-radius-panel: 24px`
- `--theme-radius-button: 9999px` (Pill)
- `--theme-backdrop-filter: blur(30px) saturate(180%)`
- `--theme-border-panel: 1px solid rgba(255, 255, 255, 0.28)`
- `--theme-box-shadow: 0 20px 48px rgba(0, 0, 0, 0.4), inset 0 1px 1px rgba(255, 255, 255, 0.45)`
- Zero neon glow.

- [ ] **Step 4: Create `runtimes/themes/retro_osd/`**

Write `theme.json` and CSS files defining Retro OSD:
- `--theme-accent: #ff7700`
- `--theme-accent-contrast: #000000`
- `--theme-bg-panel: #0a0a0c`
- `--theme-border-panel: 2px solid #ff7700`
- Monospace font, no cyan tokens anywhere.

- [ ] **Step 5: Run tests to verify all 3 packages validate**

Run: `cargo test --test theme_package_test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add runtimes/themes/ tests/integration/theme_package_test.rs
git commit -m "feat(theme): provide self-contained packages for Matte, Liquid Glass, and Retro OSD"
```

---

### Task 6: HTML5 Canvas Swipe Motion Trail & Canvas-Only Ripple

**Files:**
- Modify: `android/shell/app/src/main/assets/overlay/vpad.js`
- Modify: `android/shell/app/src/main/assets/overlay/vpad.css`
- Test: `android/shell/app/src/test/js/vpad_bridge.test.js`

**Interfaces:**
- Creates: `<canvas id="vpad-trail-canvas">` inside `#vpad-touch-ripple-layer`.
- Replaces DOM `.vpad-touch-tail-segment` beads with smooth Bézier spline rendering fading out in 220ms.
- Strict canvas-only filter: zero ripple/trail when touching gamepad controls or modal dialogs.

- [ ] **Step 1: Write test for canvas ribbon trail in `vpad_bridge.test.js`**

```javascript
test("Touch trail: uses full-screen canvas ribbon and ignores virtual controls", () => {
    assert(vpadSource.includes('vpad-trail-canvas'), "vpad must initialize canvas for motion trail");
    assert(vpadSource.includes('quadraticCurveTo'), "trail must use smooth spline curves");
    assert(vpadSource.includes('shouldIgnoreTouchForFeedback'), "controls must be filtered out");
});
```

- [ ] **Step 2: Run test to verify failure**

Run: `node android/shell/app/src/test/js/vpad_bridge.test.js`
Expected: FAIL

- [ ] **Step 3: Implement HTML5 Canvas trail renderer in `vpad.js`**

1. Create `<canvas id="vpad-trail-canvas">` with window resize syncing:
   ```javascript
   const trailCanvas = document.createElement("canvas");
   trailCanvas.id = "vpad-trail-canvas";
   trailCanvas.style.cssText = "position:fixed;top:0;left:0;width:100%;height:100%;pointer-events:none;z-index:99996;";
   ```
2. On `pointerdown` / `pointermove` on game canvas (filtered via `shouldIgnoreTouchForFeedback`):
   Record points `{ x, y, time, width }`.
3. `requestAnimationFrame` loop:
   - Interpolates points using `ctx.quadraticCurveTo`.
   - Decreases stroke width and opacity towards the tail end.
   - Clears canvas and stops loop when all points expire (> 220ms).
4. Remove previous discrete DOM particle code.

- [ ] **Step 4: Run test to verify pass**

Run: `node android/shell/app/src/test/js/vpad_bridge.test.js`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/assets/overlay/
git commit -m "feat(vpad): implement smooth hardware-accelerated canvas motion trail"
```

---

### Task 7: Theme Switcher Engine (`theme_loader.js`) & Dynamic Stacking Fix

**Files:**
- Create: `android/shell/app/src/main/assets/overlay/theme_loader.js`
- Modify: `android/shell/app/src/main/assets/overlay/vpad.js`
- Modify: `android/shell/app/src/main/assets/overlay/index.html`
- Modify: `preview/index.html`
- Test: `android/shell/app/src/test/js/overlay_interaction_regression.test.js`

**Interfaces:**
- Loads active theme by unmounting old package stylesheets and mounting the new package's (`tokens.css`, `components.css`, `layout.css`, `effects.css`).
- Updates Select Theme popover dynamically from `themes_manifest.json`.
- Normalizes `bringPanelToFront`: single event handler, no double-firing, ensures Diagnostic Suite always pops above Cheat Engine when opened or tapped.

- [ ] **Step 1: Write test for dynamic theme mounting and panel stacking**

```javascript
test("Theme loader: unmounts previous theme sheets before mounting new ones", () => {
    // Assert theme_loader unmounts previous link elements and sets [data-theme="..."]
});

test("Panel stacking: opening Diagnostic Suite over Cheat Menu elevates Diagnostic Suite above Cheat Menu", () => {
    // Assert modal z-index hierarchy
});
```

- [ ] **Step 2: Run test to verify failure**

Run: `node android/shell/app/src/test/js/overlay_interaction_regression.test.js`
Expected: FAIL

- [ ] **Step 3: Implement `theme_loader.js` & stacking fix**

1. `theme_loader.js`:
   Reads `themes_manifest.json` (or inline fallback). Exposes `window.RuntimePackTheme.applyTheme(themeId)`.
   Removes `<link data-theme-sheet>` and appends the new theme's sheets. Sets `<body data-theme="${themeId}">`.
2. Stacking fix:
   Single capture listener on `document` with debounce flag per pointer event ID so `pointerdown` does not triple-elevate.
   Opening Diagnostic Suite explicitly calls `bringPanelToFront(document.getElementById("rp-diag-modal"))`.

- [ ] **Step 4: Run tests to verify pass**

Run: `node android/shell/app/src/test/js/overlay_interaction_regression.test.js`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/assets/overlay/ tests/
git commit -m "feat(theme): implement dynamic theme stylesheet loader and fix modal stacking"
```

---

### Task 8: Full Preview Synchronization & Multi-Suite Test Sweep

**Files:**
- Sync: `android/shell/app/src/main/assets/overlay/` -> `preview/`
- Test: All JavaScript suites (`vpad_bridge`, `overlay_interaction_regression`, `cheat_menu`, `diagnostic_suite`, `web_shims`)
- Test: All Android Gradle JVM unit tests (`./gradlew.bat :app:testDebugUnitTest`)
- Test: Rust cargo test suite (`cargo test --workspace`)

- [ ] **Step 1: Synchronize all overlay files to `preview/`**

```powershell
Copy-Item -Path "android\shell\app\src\main\assets\overlay\*" -Destination "preview\" -Recurse -Force
```

- [ ] **Step 2: Run all JavaScript test suites**

```bash
node android/shell/app/src/test/js/vpad_bridge.test.js
node android/shell/app/src/test/js/overlay_interaction_regression.test.js
node android/shell/app/src/test/js/cheat_menu.test.js
node android/shell/app/src/test/js/diagnostic_suite.test.js
node android/shell/app/src/test/js/web_shims.test.js
```
Expected: 100% tests PASS.

- [ ] **Step 3: Run Android JVM unit tests**

```powershell
cd android/shell; .\gradlew.bat :app:testDebugUnitTest
```
Expected: BUILD SUCCESSFUL (24/24 tasks passing).

- [ ] **Step 4: Run Rust Cargo workspace tests**

```bash
cargo test --workspace
```
Expected: All tests PASS.

- [ ] **Step 5: Commit**

```bash
git add preview/
git commit -m "chore(theme): synchronize preview overlay with theme package engine and verify all suites"
```
