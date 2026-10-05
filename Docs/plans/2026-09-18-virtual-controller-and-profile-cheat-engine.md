# Virtual Controller Overlay & Profile-Driven Cheat Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate the canonical `rpgmaker-android-suite` virtual controller overlay (Winlator/JoiPlay style with 4-button vertical topbar and in-game layout editor), a two-layer cheat engine (universal + game-profile-driven), and bridge touch input to Android host runtimes.

**Architecture:** Embed `vpad.js` and `cheat_engine.js` into the Android web assets/overlay pipeline. Web engines (MV/MZ) consume touch and DOM events directly, while RGSS/native engines receive mapped key events via `InputService` and JNI. Game profiles in `profiles/games/<game_id>.json` provide game-specific mechanics and forbidden feature gates.

**Tech Stack:** Rust (RuntimePack CLI/Core), Kotlin (Android Host Shell), JavaScript/CSS (VPad Touch Overlay & Cheat Engine), JSON (Game Profiles).

**Spec:** `Docs/ANDROID_RUNTIME.md`, `Docs/RUNTIME_CONTRACT.md`, and `C:\Users\Nekooz\.config\opencode\skills\rpgmaker-android-suite\SKILL.md`.

## Global Constraints

- Never modify user's source game directly; all assets are generated into build directories.
- UI text must be in English with clean, accessible contrast.
- 2 official visual themes: Clean Slate (`#141416`) and Retro OSD (`#0a0a0c` / `#ff7700`).
- Touch isolation: D-Pad holding must not cancel or block action buttons.
- Storage permissions: Zero broad storage permissions (`MANAGE_EXTERNAL_STORAGE` forbidden).

---

### Task 1: Virtual Controller Overlay & In-Game Layout Editor

**Files:**
- Create: `android/shell/app/src/main/assets/vpad/vpad.js`
- Create: `android/shell/app/src/main/assets/vpad/vpad.css`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt`
- Test: `tests/integration/vpad_asset_test.rs`

**Interfaces:**
- Consumes: `android/shell/app/src/main/assets/` asset packaging pipeline.
- Produces: Injected `#vpad-topbar` (Toggle, Gear, Cheat, Linktree), `#vpad-controls`, and `#vpad-editor-bar` into the game view.

- [ ] **Step 1: Write the failing test for VPad assets existence and integrity**

```rust
#[test]
fn test_vpad_assets_bundled() {
    let vpad_js = std::path::Path::new("android/shell/app/src/main/assets/vpad/vpad.js");
    assert!(vpad_js.exists(), "vpad.js must be bundled in Android shell assets");
    let content = std::fs::read_to_string(vpad_js).unwrap();
    assert!(content.contains("vpad-topbar"), "vpad.js must contain topbar toolbar");
    assert!(content.contains("vpad-btn-gear"), "vpad.js must contain layout editor trigger");
    assert!(content.contains("vpad-btn-cheat"), "vpad.js must contain cheat menu trigger");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test integration_vpad -p runtimepack`
Expected: FAIL (file not found)

- [ ] **Step 3: Port and bundle `vpad.js` and styling into Android assets**

Copy the canonical `vpad.js` from `rpgmaker-android-suite/template/vpad.js` into `android/shell/app/src/main/assets/vpad/vpad.js`. Ensure CSS styles for both Clean Slate and Retro OSD themes are included with inline SVG icons.

- [ ] **Step 4: Update WebBackend to auto-inject VPad overlay on page finish**

In `WebBackend.kt`, ensure `vpad.js` is automatically loaded into the document DOM on `onPageFinished`.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p runtimepack`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add android/shell/app/src/main/assets/vpad/ android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt tests/
git commit -m "feat: integrate canonical VPad overlay with in-game editor and 4 topbar icons"
```

---

### Task 2: Two-Layer In-Game Cheat Engine (Universal Layer)

**Files:**
- Create: `android/shell/app/src/main/assets/vpad/cheat_engine.js`
- Create: `android/shell/app/src/main/assets/vpad/cheat_menu.js`
- Test: `tests/integration/cheat_engine_test.rs`

**Interfaces:**
- Consumes: In-game RPG Maker objects (`$gameParty`, `$gamePlayer`, `$gameSwitches`, `$gameVariables`, `SceneManager`).
- Produces: `window.RuntimePackCheat` API exposing:
  - `executeAction(actionKey)`
  - `getSwitches(range)`
  - `setSwitch(id, value)`
  - `getVariables(range)`
  - `setVariable(id, value)`

- [ ] **Step 1: Write test verifying cheat engine script bundling and syntax**

```rust
#[test]
fn test_cheat_engine_bundled() {
    let path = std::path::Path::new("android/shell/app/src/main/assets/vpad/cheat_engine.js");
    assert!(path.exists());
    let js = std::fs::read_to_string(path).unwrap();
    assert!(js.contains("executeAction"));
    assert!(js.contains("heal_all"));
    assert!(js.contains("save_anywhere"));
}
```

- [ ] **Step 2: Implement `cheat_engine.js` with Universal Layer**

Implement universal safe hooks:
- Max Currency: `$gameParty.gainGold(999999)`
- Full Recovery: `$gameParty.allMembers().forEach(a => a.recoverAll())`
- Save Anywhere: `SceneManager.push(Scene_Save)`
- No-Clip: `$gamePlayer._through = !$gamePlayer._through`
- Switch & Variable Inspector: query `$gameSwitches` and `$gameVariables`

- [ ] **Step 3: Implement `cheat_menu.js` UI modal dialog**

Connect `#vpad-btn-cheat` to open a modal dialog with tabs:
1. Quick Cheats (Gold, Heal, Save, No-Clip)
2. Switches & Variables (Search by ID or name, toggle on/off, edit values)
3. Custom Game Mechanics (dynamically populated from Layer 2)

- [ ] **Step 4: Run tests to verify**

Run: `cargo test -p runtimepack`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/assets/vpad/cheat_engine.js android/shell/app/src/main/assets/vpad/cheat_menu.js tests/
git commit -m "feat: implement universal standard in-game cheat engine"
```

---

### Task 3: Profile-Driven Game-Specific Cheats (Layer 2)

**Files:**
- Create: `profiles/games/` directory for game profiles
- Modify: `core/src/build_plan/generator.rs`
- Modify: `cli/src/main.rs`
- Test: `tests/integration/game_profile_test.rs`

**Interfaces:**
- Consumes: `profiles/games/<game_id>.json` defining game-specific mechanics (e.g. F&H2 limb recovery, Lona stamina/trauma, custom EXP/items).
- Produces: Injected `window.__gameProfile` into game assets during `runtimepack build`.

- [ ] **Step 1: Write test for profile detection and validation**

```rust
#[test]
fn test_profile_loader() {
    let profile_json = r#"{
        "id": "sample_game",
        "title": "Sample Game",
        "mechanics": {
            "custom_heal": { "type": "gain_gold", "amount": 500 }
        },
        "forbidden_features": ["global_speed_hack"]
    }"#;
    let parsed: serde_json::Value = serde_json::from_str(profile_json).unwrap();
    assert_eq!(parsed["id"], "sample_game");
}
```

- [ ] **Step 2: Add game profile discovery and copying in `runtimepack build`**

When packaging a game, check if matching profile exists in `profiles/games/` by engine/title. If found, write it to `assets/game_profile.json` so `cheat_engine.js` loads it automatically.

- [ ] **Step 3: Test profile execution with forbidden features gating**

Verify that actions listed in `forbidden_features` are blocked by `cheat_engine.js`.

- [ ] **Step 4: Commit**

```bash
git add core/src/ profiles/ cli/ tests/
git commit -m "feat: implement profile-driven game-specific cheat injector and validation"
```

---

### Task 4: RGSS Android Native Runtime Hookup (Blackscreen Fix)

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/BootstrapActivity.kt`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/RuntimeHost.kt`
- Create: `android/shell/app/src/main/java/io/runtimepack/host/RgssBackend.kt`
- Test: `tests/integration/rgss_runtime_test.rs`

**Interfaces:**
- Consumes: `game.pack` containing `Data/Scripts.rvdata2` (or rxdata/rvdata) + assets.
- Produces: Native surface view / WebView fallback that mounts VFS and initializes RGSS execution loop without blank screen.

- [ ] **Step 1: Write integration test for RGSS runtime backend resolution**

```rust
#[test]
fn test_rgss_backend_manifest_resolution() {
    let registry = runtimepack_core::runtime_registry::RuntimeRegistry::new();
    let runtime = registry.get("rgss-android").unwrap();
    assert_eq!(runtime.family, runtimepack_core::domain::RuntimeFamily::Rgss);
    assert!(runtime.capabilities.contains(&"graphics.surface".to_string()));
}
```

- [ ] **Step 2: Implement `RgssBackend.kt` in Android shell**

Implement `RgssBackend`:
- Sets up rendering surface (`SurfaceView` / texture rendering).
- Extracts and loads RGSS scripts through `ContentService`.
- Binds touch events from VPad overlay to RGSS keyboard events (`Input.trigger?`, `Input.press?`).
- Emits `HealthMarker.FIRST_FRAME` when the first scene is presented.

- [ ] **Step 3: Update `BootstrapActivity.kt` to choose correct surface by engine family**

For Web (MV/MZ), attach `WebView`.
For RGSS (XP/VX/Ace), attach native rendering surface with on-screen VPad overlay.

- [ ] **Step 4: Build and test end-to-end**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add android/shell/ tests/
git commit -m "feat: implement RGSS native surface backend and VPad touch binding"
```
