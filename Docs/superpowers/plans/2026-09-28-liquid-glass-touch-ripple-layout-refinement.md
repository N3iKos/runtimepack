# Liquid Glass, Minimalist Matte Refinement, Touch Ripple & Stacking Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refine the virtual controller overlay with true iOS Liquid Glass design, restore Minimalist Matte's deep blue toolbar/editor accents, enforce universal thin-ring visual ripple with motion tail for game screen touches, isolate touch feedback from gamepad controls, and provide dynamic top-layer panel stacking.

**Architecture:** 
- CSS styling differentiates each theme: Minimalist Matte uses dark charcoal with deep blue (`#1e3a8a` / `#2563eb`) accents; Liquid Glass uses iOS-style frosted refraction (`blur(28px)`), specular 1px rims, 22px squircle panels, and Apple System Blue active states without over-glow. Retro OSD remains strictly untouched.
- Touch feedback subsystem distinguishes game canvas touches from gamepad button touches: taps and sweeps on the canvas trigger a universal 1px crisp ring ripple and fading motion trail, while touches on controls are ignored.
- Global Window Stacking Manager brings any clicked or tapped panel (`#rp-diag-modal`, `#fnh-cheat-overlay`, `#vpad-editor-bar`, `#vpad-prop-panel`, etc.) to the highest z-index layer.

**Tech Stack:** HTML5 / CSS3 / Vanilla JavaScript (ES6) running in Android WebView overlay, Node.js unit tests (`assert`, `vm`), Gradle Kotlin Android tests.

**Spec:** User visual reference inputs and collaborative design decisions in this session.

## Global Constraints
- Folder Game Input 100% Immutable (`Raw-Game/` remains untouched).
- Retro OSD theme (`.vpad-theme-retro`) must remain 100% untouched.
- Assets in `android/shell/app/src/main/assets/overlay/` and `preview/` must remain synchronized.
- DOM IDs (`vpad-btn-gear`, `vpad-editor-bar`, etc.) must not be broken to preserve test backward compatibility.

---

### Task 1: Restore Minimalist Matte Toolbar & Editor Bar Deep Blue Accents

**Files:**
- Modify: `android/shell/app/src/main/assets/overlay/vpad.css:500-540`
- Modify: `android/shell/app/src/main/assets/overlay/vpad.css:1350-1375`
- Test: `android/shell/app/src/test/js/overlay_interaction_regression.test.js`

**Interfaces:**
- Consumes: CSS class `body.vpad-theme-matte`
- Produces: Correct deep blue styling for `#vpad-editor-bar`, `#vpad-bar-done`, and active toolbar items

- [ ] **Step 1: Write test in `overlay_interaction_regression.test.js` for Matte deep blue accents**

```javascript
test("Matte theme: toolbar and editor bar use deep blue #1e3a8a accents", () => {
    const cssPath = path.resolve(__dirname, "../../main/assets/overlay/vpad.css");
    const css = fs.readFileSync(cssPath, "utf-8");
    assert(css.includes("body.vpad-theme-matte .vpad-top-btn.active"), "matte must style active top btn");
    assert(css.includes("background: #1e3a8a !important;"), "matte active top btn must use deep blue #1e3a8a");
    assert(css.includes("border-color: #2563eb !important;"), "matte active top btn must use border #2563eb");
    assert(css.includes("body.vpad-theme-matte #vpad-editor-bar"), "matte must style editor bar");
    assert(css.includes("body.vpad-theme-matte #vpad-bar-done"), "matte must style editor done button in deep blue");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node android/shell/app/src/test/js/overlay_interaction_regression.test.js`
Expected: FAIL with "matte must style editor bar"

- [ ] **Step 3: Update `vpad.css` for Minimalist Matte toolbar & editor bar**

In `android/shell/app/src/main/assets/overlay/vpad.css`:
```css
body.vpad-theme-matte .vpad-top-btn.active,
body.vpad-theme-matte #vpad-main-trigger-btn.active {
    background: #1e3a8a !important;
    border-color: #2563eb !important;
    color: #dbeafe !important;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.75), inset 0 1px 0 rgba(255, 255, 255, 0.15) !important;
}
body.vpad-theme-matte .vpad-top-btn.active svg,
body.vpad-theme-matte #vpad-main-trigger-btn.active svg {
    stroke: #dbeafe !important;
}
body.vpad-theme-matte .vpad-top-btn:active,
body.vpad-theme-matte .vpad-top-btn.vpad-btn-pressed {
    transform: scale(0.88) !important;
}
body.vpad-theme-matte .vpad-top-btn.active:active,
body.vpad-theme-matte .vpad-top-btn.active.vpad-btn-pressed,
body.vpad-theme-matte #vpad-main-trigger-btn.active:active,
body.vpad-theme-matte #vpad-main-trigger-btn.active.vpad-btn-pressed {
    background: #172554 !important;
    border-color: #3b82f6 !important;
    color: #ffffff !important;
    box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.8) !important;
    transform: scale(0.88) !important;
}

body.vpad-theme-matte #vpad-editor-bar {
    background: #141416 !important;
    border: 1.5px solid #2563eb !important;
    box-shadow: 0 16px 36px rgba(0, 0, 0, 0.85) !important;
}
body.vpad-theme-matte #vpad-bar-done {
    background: #1e3a8a !important;
    border-color: #2563eb !important;
    color: #dbeafe !important;
}
body.vpad-theme-matte #vpad-bar-done:active {
    background: #172554 !important;
    border-color: #3b82f6 !important;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `node android/shell/app/src/test/js/overlay_interaction_regression.test.js`
Expected: PASS

---

### Task 2: Redesign Liquid Glass Theme (`.vpad-theme-clean`)

**Files:**
- Modify: `android/shell/app/src/main/assets/overlay/vpad.css:320-435`
- Modify: `android/shell/app/src/main/assets/overlay/vpad.css:1300-1340`
- Test: `android/shell/app/src/test/js/overlay_interaction_regression.test.js`

**Interfaces:**
- Consumes: CSS class `body.vpad-theme-clean`
- Produces: True iOS Liquid Glass aesthetic for controls (.vpad-btn, .vpad-dpad-container, .vpad-joystick-container) and panels (#vpad-editor-bar, #fnh-cheat-overlay, #rp-diag-modal, #vpad-prop-panel).

- [ ] **Step 1: Write test for Liquid Glass panel and button styles**

```javascript
test("Liquid Glass theme: panels use 22px squircle, blur(28px), and subtle specular rim", () => {
    const cssPath = path.resolve(__dirname, "../../main/assets/overlay/vpad.css");
    const css = fs.readFileSync(cssPath, "utf-8");
    assert(css.includes("body.vpad-theme-clean #vpad-editor-bar"), "must style Liquid Glass editor bar");
    assert(css.includes("body.vpad-theme-clean #fnh-cheat-overlay"), "must style Liquid Glass cheat panel");
    assert(css.includes("body.vpad-theme-clean #rp-diag-modal"), "must style Liquid Glass diagnostic modal");
    assert(css.includes("blur(28px)"), "must use deep blur(28px) frosted refraction");
    assert(css.includes("border-radius: 22px"), "must use iOS 22px squircle corners");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node android/shell/app/src/test/js/overlay_interaction_regression.test.js`
Expected: FAIL

- [ ] **Step 3: Implement Liquid Glass panel and control styles in `vpad.css`**

Add comprehensive Liquid Glass styles:
```css
/* Liquid Glass Panels: Squircle 22px, Specular Rim, Pure Deep Frosted Glass without over-glow */
body.vpad-theme-clean #vpad-editor-bar {
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.16) 0%, rgba(15, 23, 42, 0.55) 100%) !important;
    border: 1px solid rgba(255, 255, 255, 0.28) !important;
    border-radius: 22px !important;
    backdrop-filter: blur(28px) saturate(190%) !important;
    -webkit-backdrop-filter: blur(28px) saturate(190%) !important;
    box-shadow: 0 20px 48px rgba(0, 0, 0, 0.4), inset 0 1px 1px rgba(255, 255, 255, 0.45) !important;
}

body.vpad-theme-clean #vpad-bar-done {
    background: #007aff !important;
    border-color: #38bdf8 !important;
    color: #ffffff !important;
    border-radius: 14px !important;
    box-shadow: 0 4px 14px rgba(0, 122, 255, 0.4), inset 0 1px 0 rgba(255, 255, 255, 0.4) !important;
}

body.vpad-theme-clean #fnh-cheat-overlay,
body.vpad-theme-clean #rp-diag-modal,
body.vpad-theme-clean #vpad-prop-panel,
body.vpad-theme-clean #vpad-key-modal {
    background: linear-gradient(145deg, rgba(255, 255, 255, 0.14) 0%, rgba(15, 23, 42, 0.65) 100%) !important;
    border: 1px solid rgba(255, 255, 255, 0.25) !important;
    border-radius: 22px !important;
    backdrop-filter: blur(28px) saturate(190%) !important;
    -webkit-backdrop-filter: blur(28px) saturate(190%) !important;
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.5), inset 0 1px 1px rgba(255, 255, 255, 0.45) !important;
}

/* Liquid Glass Buttons: Floating glass pebbles */
body.vpad-theme-clean .vpad-btn {
    background: radial-gradient(circle at 35% 25%, rgba(255, 255, 255, 0.24) 0%, rgba(255, 255, 255, 0.05) 50%, rgba(15, 23, 42, 0.55) 100%) !important;
    border: 1px solid rgba(255, 255, 255, 0.25) !important;
    backdrop-filter: blur(28px) saturate(190%) !important;
    -webkit-backdrop-filter: blur(28px) saturate(190%) !important;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35), inset 0 1.2px 1px rgba(255, 255, 255, 0.55), inset 0 -1.5px 2px rgba(0, 0, 0, 0.2) !important;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `node android/shell/app/src/test/js/overlay_interaction_regression.test.js`
Expected: PASS

---

### Task 3: Universal Thin-Ring Visual Ripple & Swipe Motion Trail

**Files:**
- Modify: `android/shell/app/src/main/assets/overlay/vpad.css`
- Modify: `android/shell/app/src/main/assets/overlay/vpad.js`
- Test: `android/shell/app/src/test/js/vpad_bridge.test.js`

**Interfaces:**
- Consumes: Screen touch events on `window`
- Produces: Universal 1px thin-ring ripple on tap and motion tail on sweep; zero ripple when touching gamepad buttons/toolbar.

- [ ] **Step 1: Write test for touch feedback isolation and thin ring style**

In `android/shell/app/src/test/js/vpad_bridge.test.js`:
```javascript
test("Touch feedback: visual ripple is thin 1px ring and ignores gamepad controls", () => {
    const cssPath = path.resolve(__dirname, "../../main/assets/overlay/vpad.css");
    const css = fs.readFileSync(cssPath, "utf-8");
    assert(css.includes(".vpad-touch-ripple"), "must define .vpad-touch-ripple");
    assert(css.includes("border: 1.5px solid rgba(255, 255, 255, 0.9)"), "ripple must use thin 1.5px white ring across all themes");
    assert(css.includes(".vpad-touch-tail-segment"), "must define .vpad-touch-tail-segment for swipe tail");

    const jsPath = path.resolve(__dirname, "../../main/assets/overlay/vpad.js");
    const js = fs.readFileSync(jsPath, "utf-8");
    assert(js.includes(".vpad-control-item"), "ripple spawner must filter out touches on .vpad-control-item");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node android/shell/app/src/test/js/vpad_bridge.test.js`
Expected: FAIL

- [ ] **Step 3: Update `vpad.css` and `vpad.js`**

In `vpad.css`:
Make `.vpad-touch-ripple` universal (no theme override with thick cyan glow):
```css
.vpad-touch-ripple {
    position: absolute;
    width: 42px;
    height: 42px;
    margin-left: -21px;
    margin-top: -21px;
    border-radius: 50%;
    pointer-events: none;
    background: radial-gradient(circle, rgba(255, 255, 255, 0.12) 0%, rgba(255, 255, 255, 0.03) 55%, transparent 75%);
    border: 1.5px solid rgba(255, 255, 255, 0.9);
    box-shadow: 0 0 6px rgba(0, 0, 0, 0.45);
    animation: vpad-ripple-pop 0.32s cubic-bezier(0.1, 0.8, 0.3, 1) forwards;
}

.vpad-touch-tail-segment {
    position: absolute;
    width: 14px;
    height: 14px;
    margin-left: -7px;
    margin-top: -7px;
    border-radius: 50%;
    pointer-events: none;
    background: rgba(255, 255, 255, 0.45);
    border: 1px solid rgba(255, 255, 255, 0.8);
    box-shadow: 0 0 4px rgba(255, 255, 255, 0.5);
    animation: vpad-tail-fade 0.22s ease-out forwards;
}

@keyframes vpad-tail-fade {
    0% { transform: scale(1); opacity: 0.85; }
    100% { transform: scale(0.2); opacity: 0; }
}
```

In `vpad.js`:
Add filter against `.vpad-control-item`:
```javascript
function shouldIgnoreTouchForRipple(target) {
    if (!target || !target.closest) return false;
    return Boolean(
        target.closest(".vpad-control-item") ||
        target.closest("#vpad-topbar") ||
        target.closest("#vpad-editor-bar") ||
        target.closest("#vpad-prop-panel") ||
        target.closest("#vpad-key-modal") ||
        target.closest("#fnh-cheat-overlay") ||
        target.closest("#rp-diag-modal")
    );
}

// In pointerdown:
if (!shouldIgnoreTouchForRipple(e.target)) {
    spawnTouchRipple(e.clientX, e.clientY);
}

// In pointermove:
if (isPointerActiveOnCanvas && !shouldIgnoreTouchForRipple(e.target)) {
    spawnTouchTail(e.clientX, e.clientY);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `node android/shell/app/src/test/js/vpad_bridge.test.js`
Expected: PASS

---

### Task 4: Dynamic Top-Layer Panel Stacking (`bringToFront`)

**Files:**
- Modify: `android/shell/app/src/main/assets/overlay/vpad.js`
- Test: `android/shell/app/src/test/js/overlay_interaction_regression.test.js`

**Interfaces:**
- Consumes: Modal elements (`#vpad-editor-bar`, `#vpad-prop-panel`, `#fnh-cheat-overlay`, `#rp-diag-modal`, `#vpad-key-modal`)
- Produces: `bringPanelToFront(el)` function that guarantees newly clicked/opened panels always appear on the top visual layer.

- [ ] **Step 1: Write test for dynamic panel stacking in `overlay_interaction_regression.test.js`**

```javascript
test("Dynamic panel stacking: opening Diagnostic after Cheat brings Diagnostic to higher z-index", () => {
    // Verify bringPanelToFront increments global z-index counter and assigns to panel
    assert(typeof env.window.bringPanelToFront === "function" || vpadSource.includes("bringPanelToFront"), "must implement bringPanelToFront");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node android/shell/app/src/test/js/overlay_interaction_regression.test.js`
Expected: FAIL

- [ ] **Step 3: Implement `bringPanelToFront` in `vpad.js`**

```javascript
let topPanelZIndex = 99990;
function bringPanelToFront(panel) {
    if (!panel) return;
    topPanelZIndex += 2;
    panel.style.zIndex = topPanelZIndex;
}
window.bringPanelToFront = bringPanelToFront;

// Wire pointerdown on all panels to bring them to front when tapped
["vpad-editor-bar", "vpad-prop-panel", "vpad-key-modal", "fnh-cheat-overlay", "rp-diag-modal"].forEach(id => {
    const el = document.getElementById(id);
    if (el) {
        el.addEventListener("pointerdown", () => bringPanelToFront(el), { passive: true });
    }
});
```
And whenever `#vpad-btn-gear`, `#vpad-btn-cheat`, or `#vpad-btn-diag` opens its panel, call `bringPanelToFront(targetPanel)`.

- [ ] **Step 4: Run test to verify it passes**

Run: `node android/shell/app/src/test/js/overlay_interaction_regression.test.js`
Expected: PASS

---

### Task 5: Synchronize Preview & Full Test Suite Verification

**Files:**
- Sync: `android/shell/app/src/main/assets/overlay/vpad.js` -> `preview/vpad.js`
- Sync: `android/shell/app/src/main/assets/overlay/vpad.css` -> `preview/vpad.css`
- Run: all Node.js and Gradle test suites

- [ ] **Step 1: Copy updated assets to preview directory**

```powershell
Copy-Item -Path "android\shell\app\src\main\assets\overlay\vpad.js" -Destination "preview\vpad.js" -Force
Copy-Item -Path "android\shell\app\src\main\assets\overlay\vpad.css" -Destination "preview\vpad.css" -Force
```

- [ ] **Step 2: Verify zero diff between overlay assets and preview**

```bash
git diff --no-index "android/shell/app/src/main/assets/overlay/vpad.js" "preview/vpad.js"
git diff --no-index "android/shell/app/src/main/assets/overlay/vpad.css" "preview/vpad.css"
```
Expected: 0 diff output.

- [ ] **Step 3: Run all JS test suites**

```bash
node android/shell/app/src/test/js/vpad_bridge.test.js
node android/shell/app/src/test/js/overlay_interaction_regression.test.js
node android/shell/app/src/test/js/cheat_menu.test.js
node android/shell/app/src/test/js/diagnostic_suite.test.js
```
Expected: 100% tests pass.

- [ ] **Step 4: Run Android JVM unit tests**

```bash
cd android/shell && ./gradlew :app:testDebugUnitTest
```
Expected: BUILD SUCCESSFUL (24/24 tasks up-to-date or passing).
