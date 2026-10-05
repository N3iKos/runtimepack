# Touch Feedback 3-Mode, Mobile Landscape Fit, Screen Fit & Overlay Hardening Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Perfect the touch feedback system (smooth non-overlapping ribbon tail, no line jump between multi-touch or successive taps, 3-state cycling toggle with bottom pill toast), compact mobile landscape modal sizing, eliminate RPG Maker MZ 10% black bars at top and bottom, and harden overlay assets with native URL bridge and integrity verification.

**Architecture:**
- **Canvas Ribbon Smoothing & Touch Isolation (`vpad.js`):** Eliminate bead-like overlapping dots by rendering a single continuous stroke per touch gesture using flat caps/miters and gradient alpha. Multi-touch fingers maintain separate gesture session IDs; lifting a finger freezes and fades its stroke independently so subsequent taps cannot connect lines to previous strokes. Tapping without dragging spawns only a ripple, never a tail.
- **3-Mode Cycling Toggle & 1s Bottom Pill Toast:** `#vpad-btn-ripple` cycles through `all` (Ripple + Tail) -> `ripple_only` -> `off`, updating the SVG icon (`gesture-sweep` -> `circle-dot` -> `circle-slash`). When tapped, an elegant rounded pill toast appears for 1 second at the bottom center of the screen matching the active theme's styling.
- **Mobile Landscape Modal Sizing (`vpad.css` & themes):** Compact modal containers (`#rp-diag-modal`, `#fnh-cheat-overlay`, `#vpad-prop-panel`, `#vpad-key-modal`) sized with `max-height: min(78vh, 320px)` and `width: min(84vw, 460px)`, with balanced paddings and scroll heights so they float gracefully over the game without dominating the screen.
- **Full Screen Canvas Fit (`runtimepack_web_shims.js`):** Patch RPG Maker MZ's `Graphics._stretchHeight` to eliminate the default 0.9 mobile scaling penalty, allowing the game canvas to fit 100% of the screen height without aspect ratio distortion or top/bottom black bars.
- **Native URL Bridge & Overlay Hardening (`WebActivity.kt`, `RgssActivity.kt`, `RenpyActivity.kt`, `vpad.js`):** Remove hardcoded community URLs from JavaScript; overlay invokes native `RuntimePackVPad.openCommunityLink()`, resolving the URL securely in compiled bytecode. Enforce overlay asset integrity checking during build.

**Tech Stack:** JavaScript ES6, HTML5 Canvas 2D API, CSS3 (CSS Variables & Flexbox), Kotlin (Android Activity bridges), Rust (build pipeline & GPACK verification), Node.js unit tests, Gradle unit tests.

**Spec:** User requirements and design discussions in this session.

## Global Constraints
- Folder Game Input 100% Immutable (`Raw-Game/` remains untouched).
- Retro OSD theme (`.vpad-theme-retro`) must retain its 100% amber/orange styling.
- All 7 supported engine families (RGSS1/2/3, Web MV/MZ, Ren'Py, Tyrano) must be compatible with the native bridge and overlay enhancements.
- Assets in `android/shell/app/src/main/assets/overlay/` and `preview/` must remain byte-identical.
- All existing unit tests (JS, Kotlin, Rust) must pass.

---

### Task 1: Touch Tail Smoothing, Gesture Isolation & Multi-Touch Fix

**Files:**
- Modify: `android/shell/app/src/main/assets/overlay/vpad.js`
- Test: `android/shell/app/src/test/js/vpad_bridge.test.js`

**Interfaces:**
- Consumes: Pointer and touch events on game canvas.
- Produces: Isolated gesture stroke sessions (`Map<strokeId, Stroke>`), strictly single-path Bézier ribbon without bead overlap artifacts, zero tail on initial tap, and zero line-jumping between fingers or across taps.

- [ ] **Step 1: Write test for gesture stroke isolation and tap non-tail behavior**

In `android/shell/app/src/test/js/vpad_bridge.test.js`:
```javascript
test("Touch tail: gesture stroke isolation and tap-only non-tail", () => {
    // Assert tap at point A does not add trail points until pointermove threshold exceeded
    // Assert lift finger A and tap point B creates separate stroke without connecting line
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node android/shell/app/src/test/js/vpad_bridge.test.js`
Expected: FAIL

- [ ] **Step 3: Implement stroke session isolation and non-overlapping ribbon in `vpad.js`**

1. Replace `pointerId` reuse in `trailStrokes` with unique session IDs: `const strokeId = pid + "_" + Date.now();`.
2. On `pointerdown` / `touchstart`:
   - Spawn ripple indicator.
   - Record active pointer info `{ startX, startY, strokeId: null, active: true }`.
   - DO NOT call `addTrailPoint` yet!
3. On `pointermove` / `touchmove`:
   - Compute displacement from `startX, startY`.
   - If distance >= 10px:
     - If `tracked.strokeId` is null, allocate fresh `strokeId` and add initial point.
     - Add moving points to that specific `strokeId`.
4. On `pointerup` / `touchend` / `pointercancel`:
   - Mark stroke as completed (`active = false`), allowing existing points to naturally fade and expire without accepting any further points.
5. In `renderTrailLoop`:
   - Render each stroke with `ctx.lineCap = "butt"` and `ctx.lineJoin = "round"` along the body to prevent overlapping round-cap bead artifacts. Only the leading head segment gets a rounded tip.

- [ ] **Step 4: Run test to verify it passes**

Run: `node android/shell/app/src/test/js/vpad_bridge.test.js`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/assets/overlay/vpad.js android/shell/app/src/test/js/vpad_bridge.test.js
git commit -m "fix(touch): isolate touch stroke sessions and eliminate bead-overlap artifacts"
```

---

### Task 2: 3-Mode Feedback Toggle & 1-Second Bottom Pill Toast

**Files:**
- Modify: `android/shell/app/src/main/assets/overlay/vpad.js`
- Modify: `android/shell/app/src/main/assets/overlay/vpad.css`
- Test: `android/shell/app/src/test/js/vpad_bridge.test.js`

**Interfaces:**
- Consumes: `#vpad-btn-ripple` click/tap events.
- Produces: 3-state cycling (`all` -> `ripple_only` -> `off`), dynamic SVG icon changes (`gesture-sweep`, `circle-dot`, `circle-slash`), and floating `#vpad-touch-toast` bottom pill.

- [ ] **Step 1: Write test for 3-mode feedback cycle and bottom toast**

In `android/shell/app/src/test/js/vpad_bridge.test.js`:
```javascript
test("3-Mode Feedback Toggle: cycles all -> ripple_only -> off and triggers toast", () => {
    // Assert 3 modes cycle correctly
    // Assert icons change
    // Assert toast element #vpad-touch-toast is created and populated
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node android/shell/app/src/test/js/vpad_bridge.test.js`
Expected: FAIL

- [ ] **Step 3: Implement 3-mode feedback toggle and toast in `vpad.js` & `vpad.css`**

1. Define SVG icons in `SVG_ICONS`:
   - `rippleAll`: gesture sweep icon (dot with 2 dynamic curved arcs).
   - `rippleOnly`: circle dot icon (center dot with concentric circles).
   - `rippleOff`: circle slash icon (circle with diagonal slash).
2. In `state`:
   - `feedbackMode: "all"` (`"all"`, `"ripple_only"`, `"off"`).
   - Persist in `vpad_controller_profiles_v11`.
3. In `vpad.css`:
   - Style `#vpad-touch-toast`:
     ```css
     #vpad-touch-toast {
         position: fixed;
         bottom: 18px;
         left: 50%;
         transform: translateX(-50%) translateY(20px);
         background: var(--theme-bg-panel, rgba(15, 23, 42, 0.9));
         border: var(--theme-border-panel, 1.5px solid rgba(255, 255, 255, 0.3));
         border-radius: 9999px;
         padding: 6px 18px;
         color: var(--theme-text-primary, #ffffff);
         font-size: 12px;
         font-weight: 600;
         letter-spacing: 0.3px;
         box-shadow: 0 8px 24px rgba(0, 0, 0, 0.6);
         backdrop-filter: var(--theme-backdrop-filter, blur(16px));
         opacity: 0;
         pointer-events: none;
         z-index: 100050;
         transition: opacity 0.2s ease, transform 0.2s var(--vpad-spring-enter);
     }
     #vpad-touch-toast.show {
         opacity: 1;
         transform: translateX(-50%) translateY(0);
     }
     ```
4. Add helper `showTouchToast(text)` displaying the pill for exactly 1000ms.
5. In `#vpad-btn-ripple` tap handler:
   - Cycle mode: `all` -> `ripple_only` -> `off` -> `all`.
   - Update button icon and title.
   - Trigger `showTouchToast`.

- [ ] **Step 4: Run test to verify it passes**

Run: `node android/shell/app/src/test/js/vpad_bridge.test.js`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/assets/overlay/vpad.js android/shell/app/src/main/assets/overlay/vpad.css android/shell/app/src/test/js/vpad_bridge.test.js
git commit -m "feat(vpad): add 3-mode feedback cycling toggle with 1s bottom pill toast"
```

---

### Task 3: Compact Mobile Landscape Modal Sizing

**Files:**
- Modify: `android/shell/app/src/main/assets/overlay/vpad.css`
- Modify: `runtimes/themes/matte/components.css`
- Modify: `runtimes/themes/liquid_glass/components.css`
- Modify: `runtimes/themes/retro_osd/components.css`
- Test: `android/shell/app/src/test/js/overlay_interaction_regression.test.js`

**Interfaces:**
- Consumes: Modal element selectors (`#rp-diag-modal`, `#fnh-cheat-overlay`, `#vpad-prop-panel`, `#vpad-key-modal`).
- Produces: Responsive landscape modal styling with `max-height: min(78vh, 320px)` and `width: min(84vw, 460px)` preventing screen engulfment on mobile devices.

- [ ] **Step 1: Write test in `overlay_interaction_regression.test.js` for mobile landscape modal sizing**

```javascript
test("Modal sizing: mobile landscape dimensions fit within 320px max-height and 460px width", () => {
    const cssPath = path.resolve(__dirname, "../../main/assets/overlay/vpad.css");
    const css = fs.readFileSync(cssPath, "utf-8");
    assert(css.includes("max-height: min(78vh, 320px)"), "diag modal must use compact landscape height limit");
    assert(css.includes("max-width: 460px") || css.includes("width: min(84vw, 460px)"), "diag modal must use compact landscape width limit");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node android/shell/app/src/test/js/overlay_interaction_regression.test.js`
Expected: FAIL

- [ ] **Step 3: Update modal rules in `vpad.css` and theme packages**

1. In `vpad.css`:
   - `#rp-diag-modal`:
     `width: min(84vw, 460px);`
     `max-height: min(78vh, 320px);`
     Adjust `.diag-log-box` max-height to `90px` to fit comfortably.
   - `#fnh-cheat-overlay`:
     `width: min(84vw, 460px);`
     `max-height: min(78vh, 320px);`
     Adjust `#fnh-cheat-content` max-height to `180px`.
   - `#vpad-key-modal .vpad-modal-box`:
     `width: min(84vw, 440px);`
     `max-height: min(78vh, 320px);`
   - `#vpad-prop-panel`:
     `width: min(80vw, 290px);`
     `max-height: min(78vh, 310px);`
2. Update media query `@media (max-height: 480px)` for compact padding and font scaling.
3. Synchronize `runtimes/themes/` component CSS files.

- [ ] **Step 4: Run test to verify it passes**

Run: `node android/shell/app/src/test/js/overlay_interaction_regression.test.js`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/assets/overlay/vpad.css runtimes/themes/ android/shell/app/src/test/js/overlay_interaction_regression.test.js
git commit -m "fix(overlay): compact modal dimensions for mobile landscape screens"
```

---

### Task 4: Full-Screen Game Canvas Fit (Remove RPG Maker MZ 10% Black Bars)

**Files:**
- Modify: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`
- Test: `android/shell/app/src/test/js/web_shims.test.js`

**Interfaces:**
- Consumes: RPG Maker MZ `Graphics` object.
- Produces: Patched `Graphics._stretchHeight` returning `window.innerHeight * 1.0` (eliminating the `0.9` rate penalty), filling the entire screen height cleanly without top/bottom black bars.

- [ ] **Step 1: Write test in `web_shims.test.js` verifying `Graphics._stretchHeight` patch**

```javascript
test("RPG Maker MZ screen fit: Graphics._stretchHeight uses 1.0 scale and window.innerHeight", () => {
    // Assert patchGraphics updates _stretchHeight to return window.innerHeight without 0.9 penalty
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node android/shell/app/src/test/js/web_shims.test.js`
Expected: FAIL

- [ ] **Step 3: Implement `Graphics._stretchHeight` override in `runtimepack_web_shims.js`**

In `patchGraphics(g)`:
```javascript
if (typeof g._stretchHeight === "function" && !g._stretchHeight.__rp_fit_patched) {
    g._stretchHeight = function() {
        return (typeof window !== "undefined" && window.innerHeight) ?
            window.innerHeight : (document.documentElement ? document.documentElement.clientHeight : 600);
    };
    g._stretchHeight.__rp_fit_patched = true;
}

if (typeof g._stretchWidth === "function" && !g._stretchWidth.__rp_fit_patched) {
    g._stretchWidth = function() {
        return (typeof window !== "undefined" && window.innerWidth) ?
            window.innerWidth : (document.documentElement ? document.documentElement.clientWidth : 800);
    };
    g._stretchWidth.__rp_fit_patched = true;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `node android/shell/app/src/test/js/web_shims.test.js`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js android/shell/app/src/test/js/web_shims.test.js
git commit -m "fix(web): patch RPG Maker MZ Graphics._stretchHeight to fit 100% screen height"
```

---

### Task 5: Native URL Bridge & Overlay Hardening

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/RenpyActivity.kt`
- Modify: `android/shell/app/src/main/assets/overlay/vpad.js`
- Test: `android/shell/app/src/test/js/vpad_bridge.test.js`
- Test: `android/shell/app/src/test/java/io/runtimepack/host/KeyMappingTest.kt`

**Interfaces:**
- Removes: Raw `https://linktr.ee/Veldra.Works` string from JavaScript.
- Adds: `@JavascriptInterface fun openCommunityLink()` in native activity bridges dispatching secure intent.
- VPad invokes `window.RuntimePackVPad.openCommunityLink()`.

- [ ] **Step 1: Write test for native community link bridge**

In `vpad_bridge.test.js`:
```javascript
test("Native Community Link: invokes RuntimePackVPad.openCommunityLink without hardcoded URL in JS", () => {
    assert(!vpadSource.includes('"https://linktr.ee/Veldra.Works"'), "vpad.js must not contain raw linktree URL");
    assert(vpadSource.includes('openCommunityLink'), "vpad.js must invoke openCommunityLink");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node android/shell/app/src/test/js/vpad_bridge.test.js`
Expected: FAIL

- [ ] **Step 3: Implement native bridge and update `vpad.js`**

1. In `WebActivity.kt`, `RgssActivity.kt`, `RenpyActivity.kt`:
   Add `@JavascriptInterface fun openCommunityLink()` executing:
   ```kotlin
   @JavascriptInterface
   fun openCommunityLink() {
       val url = "https://linktr.ee/Veldra.Works"
       this@WebActivity.openUrl(url)
   }
   ```
2. In `vpad.js`:
   Replace `openLinktree` implementation:
   ```javascript
   function openCommunityLink() {
       if (typeof window !== "undefined" && window.RuntimePackVPad && typeof window.RuntimePackVPad.openCommunityLink === "function") {
           window.RuntimePackVPad.openCommunityLink();
       } else if (typeof window !== "undefined" && window.RuntimePackVPad && typeof window.RuntimePackVPad.openUrl === "function") {
           window.RuntimePackVPad.openUrl("https://linktr.ee/Veldra.Works");
       }
   }
   ```
   (In production build, fallback raw URL is omitted or encoded).

- [ ] **Step 4: Run tests to verify pass**

Run:
```powershell
node android/shell/app/src/test/js/vpad_bridge.test.js
cd android/shell; .\gradlew.bat :app:testDebugUnitTest
```
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/java/ android/shell/app/src/main/assets/overlay/vpad.js android/shell/app/src/test/
git commit -m "feat(security): move community link resolution to native Kotlin bridge"
```

---

### Task 6: Preview Synchronization, Full Test Sweep & Forestia Verification Build

**Files:**
- Sync: `android/shell/app/src/main/assets/overlay/` -> `preview/`
- Test: All JavaScript suites
- Test: Android Gradle JVM test suite
- Test: Rust Cargo workspace test suite
- Build: Live Forestia APK build

- [ ] **Step 1: Synchronize all overlay files to `preview/`**

```powershell
Copy-Item -Path "android\shell\app\src\main\assets\overlay\*" -Destination "preview\" -Recurse -Force
```

- [ ] **Step 2: Run all JavaScript test suites**

```powershell
node android/shell/app/src/test/js/vpad_bridge.test.js
node android/shell/app/src/test/js/overlay_interaction_regression.test.js
node android/shell/app/src/test/js/cheat_menu.test.js
node android/shell/app/src/test/js/diagnostic_suite.test.js
node android/shell/app/src/test/js/web_shims.test.js
```
Expected: 100% PASS.

- [ ] **Step 3: Run Android JVM unit tests**

```powershell
cd android/shell; .\gradlew.bat :app:testDebugUnitTest
```
Expected: BUILD SUCCESSFUL.

- [ ] **Step 4: Run Rust Cargo workspace tests**

```powershell
cargo test --workspace
```
Expected: 100% PASS.

- [ ] **Step 5: Compile Forestia APK and verify**

```powershell
cargo run --bin runtimepack -- build "Raw-Game\Forestia1.4.4-IND" -o dist/forestia --package com.example.forestia --name "Forestia" --flavor beta --themes matte,liquid_glass,retro_osd --default-theme matte --accept-audit-fail --accept-audit-unknown
```
Expected: Build exit code 0, updated APK emitted.

- [ ] **Step 6: Commit**

```bash
git add preview/ dist/forestia/
git commit -m "chore: synchronize preview assets and verify Forestia build"
```
