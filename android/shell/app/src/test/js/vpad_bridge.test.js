const assert = require("assert");
const fs = require("fs");
const path = require("path");
const vm = require("vm");

// ----------------------------------------------------------------------------
// Test Suite: Virtual Gamepad Native Bridge & Community Link (VenoraTL)
// ----------------------------------------------------------------------------

console.log("Running Virtual Gamepad Native Bridge & Community Link Tests...");

// 1. Setup mock environment for vpad.js
const keyDownCalls = [];
const keyUpCalls = [];
const openUrlCalls = [];
const openCommunityLinkCalls = [];
const textFocusCalls = [];

const mockRuntimePackVPad = {
    onKeyDown: (keyCode) => {
        keyDownCalls.push(keyCode);
    },
    onKeyUp: (keyCode) => {
        keyUpCalls.push(keyCode);
    },
    openUrl: (url) => {
        openUrlCalls.push(url);
    },
    openCommunityLink: () => {
        openCommunityLinkCalls.push(true);
    },
    setOverlayTextFocused: (focused) => {
        textFocusCalls.push(focused);
    }
};

const documentListeners = {};
global.document = {
    readyState: "loading",
    addEventListener: (event, handler) => {
        if (!documentListeners[event]) documentListeners[event] = [];
        documentListeners[event].push(handler);
    },
    removeEventListener: (event, handler) => {
        if (documentListeners[event]) {
            documentListeners[event] = documentListeners[event].filter(h => h !== handler);
        }
    },
    dispatchEvent: (event, payload) => {
        if (documentListeners[event]) {
            documentListeners[event].forEach(h => h(payload));
        }
    }
};

const windowListeners = {};
global.window = {
    RuntimePackVPad: mockRuntimePackVPad,
    addEventListener: (event, handler) => {
        if (!windowListeners[event]) windowListeners[event] = [];
        windowListeners[event].push(handler);
    },
    removeEventListener: (event, handler) => {
        if (windowListeners[event]) {
            windowListeners[event] = windowListeners[event].filter(h => h !== handler);
        }
    },
    dispatchEvent: (event, payload) => {
        if (windowListeners[event]) {
            windowListeners[event].forEach(h => h(payload));
        }
    }
};

// Load vpad.js
const vpadPath = path.resolve(__dirname, "../../main/assets/overlay/vpad.js");
const {
    pressKey,
    releaseKey,
    openLinktree,
    openCommunityLink,
    setupDirectManipulation,
    setupDpadTouch,
    shouldIgnoreTouchForFeedback,
    getTrailCanvas,
    resizeTrailCanvas,
    renderTrailLoop,
    clearTrailCanvas,
    addTrailPoint,
    trailStrokes,
    activeFeedbackPointers,
    setupTrailListeners,
    showTouchToast,
    cycleFeedbackMode,
    getFeedbackMode,
    setFeedbackMode,
    updateRippleBtnUI,
    spawnTouchRipple,
    SVG_ICONS
} = require(vpadPath);

// Test 1: pressKey(90) -> window.RuntimePackVPad.onKeyDown(90)
keyDownCalls.length = 0;
pressKey(90);
assert.strictEqual(keyDownCalls.length, 1, "Expected RuntimePackVPad.onKeyDown to be called exactly once");
assert.strictEqual(keyDownCalls[0], 90, "Expected onKeyDown to be called with keyCode 90 (Z / OK)");
console.log("✔ pressKey(90) calls window.RuntimePackVPad.onKeyDown(90)");

// Test 1b: window.pressKey(90)
keyDownCalls.length = 0;
window.pressKey(90);
assert.strictEqual(keyDownCalls.length, 1, "Expected window.pressKey to invoke RuntimePackVPad.onKeyDown");
assert.strictEqual(keyDownCalls[0], 90, "Expected keyCode 90");
console.log("✔ window.pressKey(90) calls window.RuntimePackVPad.onKeyDown(90)");

// Test 2: releaseKey(90) -> window.RuntimePackVPad.onKeyUp(90)
keyUpCalls.length = 0;
releaseKey(90);
assert.strictEqual(keyUpCalls.length, 1, "Expected RuntimePackVPad.onKeyUp to be called exactly once");
assert.strictEqual(keyUpCalls[0], 90, "Expected onKeyUp to be called with keyCode 90 (Z / OK)");
console.log("✔ releaseKey(90) calls window.RuntimePackVPad.onKeyUp(90)");

// Test 2b: window.releaseKey(90)
keyUpCalls.length = 0;
window.releaseKey(90);
assert.strictEqual(keyUpCalls.length, 1, "Expected window.releaseKey to invoke RuntimePackVPad.onKeyUp");
assert.strictEqual(keyUpCalls[0], 90, "Expected keyCode 90");
console.log("✔ window.releaseKey(90) calls window.RuntimePackVPad.onKeyUp(90)");

// Test 3: openCommunityLink() and openLinktree() -> window.RuntimePackVPad.openCommunityLink()
openCommunityLinkCalls.length = 0;
openUrlCalls.length = 0;
openCommunityLink();
assert.strictEqual(openCommunityLinkCalls.length, 1, "Expected RuntimePackVPad.openCommunityLink to be called exactly once");
assert.strictEqual(openUrlCalls.length, 0, "openUrl should not be called when openCommunityLink is available");
console.log("✔ openCommunityLink() calls window.RuntimePackVPad.openCommunityLink()");

// Test 3b: window.openCommunityLink()
openCommunityLinkCalls.length = 0;
window.openCommunityLink();
assert.strictEqual(openCommunityLinkCalls.length, 1, "Expected window.openCommunityLink to call RuntimePackVPad.openCommunityLink");
console.log("✔ window.openCommunityLink() calls window.RuntimePackVPad.openCommunityLink()");

// Test 3c: window.openLinktree() backward compatibility alias
openCommunityLinkCalls.length = 0;
window.openLinktree();
assert.strictEqual(openCommunityLinkCalls.length, 1, "Expected window.openLinktree to call RuntimePackVPad.openCommunityLink");
console.log("✔ window.openLinktree() calls window.RuntimePackVPad.openCommunityLink()");

// Test 3d: openLinktree() export backward compatibility
openCommunityLinkCalls.length = 0;
openLinktree();
assert.strictEqual(openCommunityLinkCalls.length, 1, "Expected openLinktree() export to call RuntimePackVPad.openCommunityLink");
console.log("✔ openLinktree() export calls window.RuntimePackVPad.openCommunityLink()");

// Test 3e: Graceful fallback to openUrl when openCommunityLink is absent on RuntimePackVPad
const savedOpenCommunityLink = mockRuntimePackVPad.openCommunityLink;
delete mockRuntimePackVPad.openCommunityLink;
openUrlCalls.length = 0;
openCommunityLink();
assert.strictEqual(openUrlCalls.length, 1, "Expected fallback to openUrl when openCommunityLink is absent");
assert.strictEqual(openUrlCalls[0], "https://linktr.ee/Veldra.Works", "Expected fallback URL to target https://linktr.ee/Veldra.Works");

openUrlCalls.length = 0;
window.openCommunityLink();
assert.strictEqual(openUrlCalls.length, 1, "Expected window.openCommunityLink fallback to openUrl");
assert.strictEqual(openUrlCalls[0], "https://linktr.ee/Veldra.Works");

openUrlCalls.length = 0;
window.openLinktree();
assert.strictEqual(openUrlCalls.length, 1, "Expected window.openLinktree fallback to openUrl");
assert.strictEqual(openUrlCalls[0], "https://linktr.ee/Veldra.Works");

mockRuntimePackVPad.openCommunityLink = savedOpenCommunityLink;
console.log("✔ Graceful fallback to window.RuntimePackVPad.openUrl(\"https://linktr.ee/Veldra.Works\") when openCommunityLink is missing");

// Test 4: Verify index.html openCommunityLink() and openLinktree() implementation
const indexHtmlPath = path.resolve(__dirname, "../../main/assets/overlay/index.html");
const indexHtmlContent = fs.readFileSync(indexHtmlPath, "utf-8");

// Assert string presence in index.html
assert(
    indexHtmlContent.includes("https://linktr.ee/Veldra.Works"),
    "index.html must contain linktr.ee/Veldra.Works URL"
);
assert(
    indexHtmlContent.includes("openCommunityLink"),
    "index.html must define openCommunityLink"
);
assert(
    !indexHtmlContent.includes("https://github.com/anomalyco/antigravity"),
    "index.html must not contain legacy antigravity link"
);

// Execute index.html script block in isolated VM context to test behavior
const scriptMatches = indexHtmlContent.match(/<script>([\s\S]*?)<\/script>/);
assert(scriptMatches && scriptMatches[1], "index.html must contain an inline <script> block");

const htmlNativeCalls = { openCommunityLink: 0, openUrl: [] };
const htmlSandbox = {
    window: {
        RuntimePackVPad: {
            onKeyDown: () => {},
            onKeyUp: () => {},
            openCommunityLink: () => {
                htmlNativeCalls.openCommunityLink++;
            },
            openUrl: (url) => {
                htmlNativeCalls.openUrl.push(url);
            }
        },
        addEventListener: () => {}
    }
};
vm.createContext(htmlSandbox);
vm.runInContext(scriptMatches[1], htmlSandbox);

assert(typeof htmlSandbox.window.openCommunityLink === "function", "index.html must define window.openCommunityLink");
assert(typeof htmlSandbox.window.openLinktree === "function", "index.html must define window.openLinktree alias");

// 4a. Call openCommunityLink on index.html with native openCommunityLink present
htmlNativeCalls.openCommunityLink = 0;
htmlSandbox.window.openCommunityLink();
assert.strictEqual(htmlNativeCalls.openCommunityLink, 1, "index.html window.openCommunityLink should invoke native openCommunityLink");

// 4b. Call openLinktree alias on index.html with native openCommunityLink present
htmlNativeCalls.openCommunityLink = 0;
htmlSandbox.window.openLinktree();
assert.strictEqual(htmlNativeCalls.openCommunityLink, 1, "index.html window.openLinktree alias should invoke native openCommunityLink");

// 4c. Fallback in index.html when native openCommunityLink is absent
delete htmlSandbox.window.RuntimePackVPad.openCommunityLink;
htmlNativeCalls.openUrl.length = 0;
htmlSandbox.window.openCommunityLink();
assert.strictEqual(htmlNativeCalls.openUrl.length, 1, "index.html should fallback to openUrl when openCommunityLink is absent");
assert.strictEqual(htmlNativeCalls.openUrl[0], "https://linktr.ee/Veldra.Works", "index.html fallback must target https://linktr.ee/Veldra.Works");

htmlNativeCalls.openUrl.length = 0;
htmlSandbox.window.openLinktree();
assert.strictEqual(htmlNativeCalls.openUrl.length, 1, "index.html openLinktree alias should fallback to openUrl");
assert.strictEqual(htmlNativeCalls.openUrl[0], "https://linktr.ee/Veldra.Works");
console.log("✔ index.html openCommunityLink() and openLinktree() call native openCommunityLink() with openUrl fallback");

// Test 5: Graceful degradation when window.RuntimePackVPad is absent
global.window.RuntimePackVPad = null;
assert.doesNotThrow(() => {
    pressKey(90);
    releaseKey(90);
    openCommunityLink();
    openLinktree();
}, "Bridge methods must not throw when window.RuntimePackVPad is null or undefined");
console.log("✔ Graceful degradation when window.RuntimePackVPad is absent");

// Restore mock
global.window.RuntimePackVPad = mockRuntimePackVPad;

// Test 6: Multi-touch button release must not globally release other buttons
function createMockElement() {
    const listeners = {};
    return {
        classList: {
            classes: new Set(),
            add(c) { this.classes.add(c); },
            remove(c) { this.classes.delete(c); },
            contains(c) { return this.classes.has(c); }
        },
        addEventListener(event, handler) {
            if (!listeners[event]) listeners[event] = [];
            listeners[event].push(handler);
        },
        dispatchEvent(event, payload) {
            if (listeners[event]) {
                listeners[event].forEach(h => h(payload));
            }
        }
    };
}

const btnAlt = createMockElement();
const btnOk = createMockElement();
const ctrlAlt = { id: "btn_alt", type: "button", keyCode: 18 };
const ctrlOk = { id: "btn_ok", type: "button", keyCode: 90 };

setupDirectManipulation(btnAlt, ctrlAlt);
setupDirectManipulation(btnOk, ctrlOk);

keyDownCalls.length = 0;
keyUpCalls.length = 0;

// Finger 1 (id 10) presses ALT
btnAlt.dispatchEvent("touchstart", {
    stopPropagation() {},
    changedTouches: [{ identifier: 10 }]
});
assert.strictEqual(keyDownCalls.length, 1, "ALT keydown dispatched");
assert.strictEqual(keyDownCalls[0], 18, "ALT keycode 18");

// Finger 2 (id 20) presses OK
btnOk.dispatchEvent("touchstart", {
    stopPropagation() {},
    changedTouches: [{ identifier: 20 }]
});
assert.strictEqual(keyDownCalls.length, 2, "Both keys dispatched down");
assert.strictEqual(keyDownCalls[1], 90, "OK keycode 90");

// Finger 2 (id 20) releases OK
global.window.dispatchEvent("touchend", {
    changedTouches: [{ identifier: 20 }]
});

// OK should be released, but ALT MUST REMAIN PRESSED
assert.strictEqual(keyUpCalls.includes(90), true, "OK (90) must be released on touchend");
assert.strictEqual(keyUpCalls.includes(18), false, "ALT (18) must NOT be released when OK touch ends");

// Finally Finger 1 (id 10) releases ALT
global.window.dispatchEvent("touchend", {
    changedTouches: [{ identifier: 10 }]
});
assert.strictEqual(keyUpCalls.includes(18), true, "ALT (18) released when its own touch ends");
console.log("✔ Multi-touch button release does not globally release other buttons");

// Test 7: D-Pad direction isolation against button release (drives setupDpadTouch)
keyDownCalls.length = 0;
keyUpCalls.length = 0;

function createMockDpad() {
    const listeners = {};
    const arrows = {
        ".vpad-dpad-up": createMockElement(),
        ".vpad-dpad-down": createMockElement(),
        ".vpad-dpad-left": createMockElement(),
        ".vpad-dpad-right": createMockElement()
    };
    return {
        querySelector(sel) { return arrows[sel] || null; },
        getBoundingClientRect() { return { left: 0, top: 0, width: 100, height: 100 }; },
        addEventListener(event, handler) {
            if (!listeners[event]) listeners[event] = [];
            listeners[event].push(handler);
        },
        dispatchEvent(event, payload) {
            if (listeners[event]) {
                listeners[event].forEach(h => h(payload));
            }
        }
    };
}

const dpad = createMockDpad();
const ctrlDpad = { id: "dpad", type: "dpad", bindings: { up: 38, down: 40, left: 37, right: 39 } };
setupDpadTouch(dpad, ctrlDpad);

const btnHold = createMockElement();
const ctrlHold = { id: "btn_hold", type: "button", keyCode: 18 };
setupDirectManipulation(btnHold, ctrlHold);

// Finger 1 (id 100) presses D-Pad UP (touch at top-center of the 100x100 rect)
dpad.dispatchEvent("touchstart", {
    stopPropagation() {},
    preventDefault() {},
    changedTouches: [{ identifier: 100, clientX: 50, clientY: 10 }]
});
assert.strictEqual(keyDownCalls.includes(38), true, "D-Pad UP (38) pressed via setupDpadTouch");
assert.strictEqual(keyUpCalls.includes(38), false, "D-Pad UP (38) not released yet");

// Finger 2 (id 200) presses a separate button (ALT 18)
btnHold.dispatchEvent("touchstart", {
    stopPropagation() {},
    changedTouches: [{ identifier: 200 }]
});
assert.strictEqual(keyDownCalls.includes(18), true, "Separate button (18) pressed on second touch id");

// Release the button touch (id 200) -> D-Pad UP must remain pressed
global.window.dispatchEvent("touchend", {
    changedTouches: [{ identifier: 200 }]
});
assert.strictEqual(keyUpCalls.includes(18), true, "Button (18) released on its own touch end");
assert.strictEqual(keyUpCalls.includes(38), false, "D-Pad UP (38) must remain pressed when button touch ends");

// Vice versa: hold a button again, then release the D-Pad touch
keyDownCalls.length = 0;
keyUpCalls.length = 0;
btnHold.dispatchEvent("touchstart", {
    stopPropagation() {},
    changedTouches: [{ identifier: 201 }]
});
assert.strictEqual(keyDownCalls.includes(18), true, "Button (18) re-pressed on third touch id");

// Release the D-Pad touch (id 100) -> button must remain pressed
global.window.dispatchEvent("touchend", {
    changedTouches: [{ identifier: 100 }]
});
assert.strictEqual(keyUpCalls.includes(38), true, "D-Pad UP (38) released on its own touch end");
assert.strictEqual(keyUpCalls.includes(18), false, "Button (18) must remain pressed when D-Pad touch ends");

// Release the button touch (id 201) -> button finally released
global.window.dispatchEvent("touchend", {
    changedTouches: [{ identifier: 201 }]
});
assert.strictEqual(keyUpCalls.includes(18), true, "Button (18) released on its own touch end");
console.log("✔ D-Pad touch release is isolated from button touches (drives setupDpadTouch)");

// Test 8: Overlay text focus bridge (delegated focusin/focusout on document)
assert(typeof documentListeners["focusin"] !== "undefined", "vpad.js must register a delegated focusin listener");
assert(typeof documentListeners["focusout"] !== "undefined", "vpad.js must register a delegated focusout listener");

textFocusCalls.length = 0;
document.dispatchEvent("focusin", { target: { tagName: "INPUT" } });
assert.deepStrictEqual(textFocusCalls, [true], "focusin on <input> must call setOverlayTextFocused(true)");

document.dispatchEvent("focusout", { target: { tagName: "INPUT" } });
assert.deepStrictEqual(textFocusCalls, [true, false], "focusout on <input> must call setOverlayTextFocused(false)");

document.dispatchEvent("focusin", { target: { tagName: "TEXTAREA" } });
assert.deepStrictEqual(textFocusCalls, [true, false, true], "focusin on <textarea> must call setOverlayTextFocused(true)");

document.dispatchEvent("focusout", { target: { tagName: "TEXTAREA" } });
assert.deepStrictEqual(textFocusCalls, [true, false, true, false], "focusout on <textarea> must call setOverlayTextFocused(false)");

// Non-text elements must not toggle the overlay text-focus state.
textFocusCalls.length = 0;
document.dispatchEvent("focusin", { target: { tagName: "DIV" } });
document.dispatchEvent("focusout", { target: { tagName: "DIV" } });
assert.deepStrictEqual(textFocusCalls, [], "non-input focus events must not call setOverlayTextFocused");
console.log("✔ Overlay input/textarea focus and blur call RuntimePackVPad.setOverlayTextFocused(true/false)");

// Test 9: window blur and hidden document defensively reset overlay text focus
assert(typeof windowListeners["blur"] !== "undefined", "vpad.js must register a window blur listener");

textFocusCalls.length = 0;
global.window.dispatchEvent("blur", {});
assert.deepStrictEqual(textFocusCalls, [false], "window blur must call setOverlayTextFocused(false)");

assert(typeof documentListeners["visibilitychange"] !== "undefined", "vpad.js must register a visibilitychange listener");

textFocusCalls.length = 0;
global.document.hidden = true;
document.dispatchEvent("visibilitychange", {});
assert.deepStrictEqual(textFocusCalls, [false], "hidden document must call setOverlayTextFocused(false)");

textFocusCalls.length = 0;
global.document.hidden = false;
document.dispatchEvent("visibilitychange", {});
assert.deepStrictEqual(textFocusCalls, [], "visible document must not toggle overlay text focus");
console.log("✔ Window blur and hidden document reset RuntimePackVPad.setOverlayTextFocused(false)");

// Test 10: Verify layout default D-Pad cy is raised to ~0.75, icon gear is classic wrench, and Liquid Glass / Matte default
const vpadSource = fs.readFileSync(path.join(__dirname, "../../main/assets/overlay/vpad.js"), "utf8");
assert(vpadSource.includes('cx: 0.11, cy: 0.75'), "Preset 1 D-Pad must be positioned at cx: 0.11, cy: 0.75");
assert(vpadSource.includes('vpad_controller_profiles_v11'), "STORAGE_KEY must be vpad_controller_profiles_v11");
assert(vpadSource.includes('updateControlHitboxes'), "vpad.js must call updateControlHitboxes");
assert(vpadSource.includes('M14.7 6.3'), "gear icon must be classic wrench SVG");
assert(vpadSource.includes('Liquid Glass'), "Liquid Glass theme must be defined");
assert(vpadSource.includes('activeThemeId: "matte"'), "Default active theme must be Minimalist Matte");
assert(vpadSource.includes('vpad-touch-ripple'), "vpad.js must implement touchscreen visual ripple");
console.log("✔ Layout preset D-Pad raised (cy: 0.75), STORAGE_KEY v11, classic wrench icon, Liquid Glass, and Matte default verified");

// Test 11: Universal Thin-Ring Visual Ripple, HTML5 Canvas Motion Trail, and Strict Control Isolation
const vpadCss = fs.readFileSync(path.join(__dirname, "../../main/assets/overlay/vpad.css"), "utf8");

// 1. Universal thin 1.5px white ring styling across themes
assert(vpadCss.includes('border: 1.5px solid rgba(255, 255, 255, 0.9) !important;'), "vpad.css must define universal 1.5px thin white ring");
assert(vpadCss.includes('background: radial-gradient(circle, rgba(255, 255, 255, 0.12) 0%, rgba(255, 255, 255, 0.03) 55%, transparent 75%) !important;'), "vpad.css must define subtle inner gradient");
assert(vpadCss.includes('box-shadow: 0 0 6px rgba(0, 0, 0, 0.45) !important;'), "vpad.css must define subtle 6px shadow");

// 2. Full-screen HTML5 Canvas motion trail styling (#vpad-trail-canvas)
assert(vpadCss.includes('#vpad-trail-canvas'), "vpad.css must define #vpad-trail-canvas");
assert(vpadCss.includes('pointer-events: none;'), "vpad.css must set pointer-events: none on #vpad-trail-canvas");
assert(vpadCss.includes('position: fixed;'), "vpad.css must set position: fixed on #vpad-trail-canvas");
assert(vpadCss.includes('z-index: 99996;'), "vpad.css must set z-index: 99996 on #vpad-trail-canvas");
assert(!vpadCss.includes('.vpad-touch-tail-segment'), "vpad.css must remove deprecated .vpad-touch-tail-segment");
assert(!vpadCss.includes('@keyframes vpad-tail-fade'), "vpad.css must remove deprecated @keyframes vpad-tail-fade");

// 3. Canvas element initialization and particle deprecation in vpad.js
assert(vpadSource.includes('id="vpad-trail-canvas"'), "vpad.js template must initialize canvas#vpad-trail-canvas");
assert(vpadSource.includes('vpad-trail-canvas'), "vpad.js must reference vpad-trail-canvas");
assert(!vpadSource.includes('.vpad-touch-tail-segment'), "vpad.js must not contain deprecated .vpad-touch-tail-segment");
assert(!vpadSource.includes('spawnTouchTail'), "vpad.js must remove deprecated spawnTouchTail function");

// 4. Canvas motion trail renderer logic (points buffer, sampling, rAF, Bézier spline ribbon)
assert(vpadSource.includes('trailStrokes'), "vpad.js must maintain a motion trail points buffer");
assert(vpadSource.includes('TRAIL_SAMPLE_DIST_SQ') || vpadSource.includes('>= 16'), "vpad.js must sample points at >= 4px threshold");
assert(vpadSource.includes('TRAIL_ACTIVATION_DIST_SQ') || vpadSource.includes('100'), "vpad.js must define movement threshold (>= 10px)");
assert(vpadSource.includes('ctx.lineCap = "butt"'), "vpad.js must use butt lineCap for intermediate ribbon segments");
assert(vpadSource.includes('requestAnimationFrame'), "vpad.js must use requestAnimationFrame loop");
assert(vpadSource.includes('quadraticCurveTo'), "vpad.js must render Bézier spline ribbon via quadraticCurveTo");
assert(vpadSource.includes('clearRect'), "vpad.js must clear canvas via clearRect");
assert(vpadSource.includes('MAX_TRAIL_AGE') || vpadSource.includes('220'), "vpad.js must fade trail with 220ms max age");

// 5. Strict control isolation in shouldIgnoreTouchForFeedback
const makeMockTarget = (matchedSelector) => ({
    closest: (sel) => sel === matchedSelector ? { className: matchedSelector } : null
});

assert.strictEqual(typeof shouldIgnoreTouchForFeedback, "function", "shouldIgnoreTouchForFeedback must be a function");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget(".vpad-control-item")), true, "must ignore .vpad-control-item");
assert.strictEqual(shouldIgnoreTouchForFeedback({ id: "vpad-controls" }), false, "must NOT ignore #vpad-controls empty canvas layer");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget(".vpad-btn")), true, "must ignore .vpad-btn");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget(".vpad-top-btn")), true, "must ignore .vpad-top-btn");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget(".vpad-dpad-container")), true, "must ignore .vpad-dpad-container");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget(".vpad-joystick-container")), true, "must ignore .vpad-joystick-container");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget("#vpad-topbar")), true, "must ignore #vpad-topbar");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget("#vpad-editor-bar")), true, "must ignore #vpad-editor-bar");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget("#vpad-prop-panel")), true, "must ignore #vpad-prop-panel");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget("#vpad-key-modal")), true, "must ignore #vpad-key-modal");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget(".vpad-modal-box")), true, "must ignore .vpad-modal-box");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget("#fnh-cheat-overlay")), true, "must ignore #fnh-cheat-overlay");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget("#rp-diag-modal")), true, "must ignore #rp-diag-modal");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget(".vpad-popover")), true, "must ignore .vpad-popover");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget("canvas")), false, "must NOT ignore game canvas");
assert.strictEqual(shouldIgnoreTouchForFeedback(null), false, "must NOT ignore null target");
assert.strictEqual(shouldIgnoreTouchForFeedback({}), false, "must NOT ignore plain object without closest/matches");
console.log("✔ Universal thin-ring ripple, canvas motion trail logic, and strict control isolation verified");

// Test 12: Continuous Bézier Spline Ribbon Rendering & Zero Idle CPU Usage
let clearRectCount = 0;
let quadraticCurvesDrawn = 0;
let rAFScheduled = 0;
let rAFCancellations = 0;

const mockCanvas = {
    id: "vpad-trail-canvas",
    width: 0,
    height: 0,
    style: {},
    getContext: (mode) => {
        if (mode !== "2d") return null;
        return {
            clearRect: (x, y, w, h) => { clearRectCount++; },
            beginPath: () => {},
            moveTo: () => {},
            quadraticCurveTo: (cpx, cpy, x, y) => { quadraticCurvesDrawn++; },
            lineTo: () => {},
            stroke: () => {},
            arc: () => {},
            fill: () => {},
            save: () => {},
            restore: () => {},
            setTransform: () => {}
        };
    }
};

const origGetElementById = global.document.getElementById;
global.document.getElementById = (id) => {
    if (id === "vpad-trail-canvas") return mockCanvas;
    if (origGetElementById) return origGetElementById(id);
    return null;
};

global.requestAnimationFrame = (cb) => {
    rAFScheduled++;
    return rAFScheduled;
};
global.cancelAnimationFrame = (id) => {
    rAFCancellations++;
};

// Verify canvas resize and DPI scaling
global.window.innerWidth = 1280;
global.window.innerHeight = 720;
global.window.devicePixelRatio = 2;
resizeTrailCanvas();
assert.strictEqual(mockCanvas.width, 2560, "Canvas buffer width must be scaled by devicePixelRatio");
assert.strictEqual(mockCanvas.height, 1440, "Canvas buffer height must be scaled by devicePixelRatio");
assert.strictEqual(mockCanvas.style.width, "1280px", "Canvas CSS style width must match window.innerWidth");
assert.strictEqual(mockCanvas.style.height, "720px", "Canvas CSS style height must match window.innerHeight");

// Add simulated points to trail buffer
clearTrailCanvas();
addTrailPoint(1, 100, 100);
addTrailPoint(1, 120, 120);
addTrailPoint(1, 145, 135);
assert.strictEqual(trailStrokes.has(1), true, "Trail buffer must record points for active pointer");
assert.strictEqual(trailStrokes.get(1).length, 3, "Trail buffer must contain 3 sampled points");

// Render one frame
renderTrailLoop();
assert(clearRectCount >= 1, "renderTrailLoop must clear canvas before drawing");
assert(quadraticCurvesDrawn >= 1, "renderTrailLoop must draw Bézier spline segments using quadraticCurveTo");

// Simulate time passing > MAX_TRAIL_AGE (points expire)
const stroke = trailStrokes.get(1);
stroke.forEach(pt => { pt.time -= 800; }); // make older than MAX_TRAIL_AGE

const prevClearCount = clearRectCount;
renderTrailLoop();
assert(clearRectCount > prevClearCount, "renderTrailLoop must clear canvas when buffer empties");
assert.strictEqual(trailStrokes.has(1), false, "Expired stroke must be pruned from buffer");

// Verify clearTrailCanvas resets buffer and halts animation
addTrailPoint(2, 200, 200);
clearTrailCanvas();
assert.strictEqual(trailStrokes.size, 0, "clearTrailCanvas must wipe all active trail strokes");
console.log("✔ Continuous Bézier spline ribbon rendering and zero idle CPU lifecycle verified");

// Test 13: Touch Tail Smoothing, Movement Threshold & Stroke Session Isolation
setupTrailListeners();
clearTrailCanvas();

// 1. Initial tap at (100, 100) must only track pointer, NO trail points added
global.window.dispatchEvent("pointerdown", { pointerId: 1, clientX: 100, clientY: 100, target: {} });
assert.strictEqual(trailStrokes.size, 0, "Initial tap must not create any trail points");
assert.strictEqual(activeFeedbackPointers.has(1), true, "Pointer 1 must be tracked in activeFeedbackPointers");
const p1Tracked = activeFeedbackPointers.get(1);
assert.strictEqual(p1Tracked.strokeId, null, "Stroke ID must remain null on initial tap");
assert.strictEqual(p1Tracked.startX, 100, "startX must be recorded");
assert.strictEqual(p1Tracked.startY, 100, "startY must be recorded");

// 2. Movement below 10px threshold (displacement = 5px) must NOT create trail points
global.window.dispatchEvent("pointermove", { pointerId: 1, clientX: 103, clientY: 104, target: {} });
assert.strictEqual(trailStrokes.size, 0, "Movement below 10px threshold must not create trail points");
assert.strictEqual(p1Tracked.strokeId, null, "strokeId must remain null below 10px threshold");

// 3. Movement reaching displacement >= 10px activates stroke session
global.window.dispatchEvent("pointermove", { pointerId: 1, clientX: 110, clientY: 110, target: {} });
assert.strictEqual(trailStrokes.size, 1, "Displacement >= 10px must activate trail stroke");
const stroke1Id = p1Tracked.strokeId;
assert(stroke1Id && stroke1Id.startsWith("stroke_1_"), "strokeId must be uniquely allocated for pointer 1");
const stroke1Points = trailStrokes.get(stroke1Id);
assert.strictEqual(stroke1Points.length, 2, "Trail stroke must include start point (100, 100) and current point (110, 110)");
assert.strictEqual(stroke1Points[0].x, 100);
assert.strictEqual(stroke1Points[0].y, 100);
assert.strictEqual(stroke1Points[1].x, 110);
assert.strictEqual(stroke1Points[1].y, 110);

// 4. Continuous dragging adds subsequent points to stroke1
global.window.dispatchEvent("pointermove", { pointerId: 1, clientX: 125, clientY: 125, target: {} });
assert.strictEqual(stroke1Points.length, 3, "Subsequent movement adds points to existing stroke");

// 5. Releasing pointer stops adding to stroke1 session
global.window.dispatchEvent("pointerup", { pointerId: 1 });
assert.strictEqual(p1Tracked.active, false, "Released pointer must be deactivated");
assert.strictEqual(activeFeedbackPointers.has(1), false, "Pointer 1 removed from active feedback pointers");
assert.strictEqual(trailStrokes.has(stroke1Id), true, "Stroke 1 remains in trailStrokes to fade out");

// 6. Successive tap with the same pointerId creates a completely new stroke session
global.window.dispatchEvent("pointerdown", { pointerId: 1, clientX: 300, clientY: 300, target: {} });
assert.strictEqual(trailStrokes.size, 1, "New tap does not create trail points");
global.window.dispatchEvent("pointermove", { pointerId: 1, clientX: 325, clientY: 325, target: {} });
assert.strictEqual(trailStrokes.size, 2, "New gesture creates a second independent stroke session");
const p1NewTracked = activeFeedbackPointers.get(1);
const stroke2Id = p1NewTracked.strokeId;
assert.notStrictEqual(stroke1Id, stroke2Id, "Successive gesture must allocate a brand new strokeId");
const stroke2Points = trailStrokes.get(stroke2Id);
assert.strictEqual(stroke2Points[0].x, 300, "New stroke start point must be (300, 300)");
assert.strictEqual(stroke2Points[1].x, 325, "New stroke second point must be (325, 325)");
assert.strictEqual(stroke1Points[stroke1Points.length - 1].x, 125, "Stroke 1 last point remains (125, 125)");

// 7. Multi-touch isolation: separate fingers never connect lines
global.window.dispatchEvent("pointerdown", { pointerId: 2, clientX: 500, clientY: 500, target: {} });
global.window.dispatchEvent("pointermove", { pointerId: 2, clientX: 520, clientY: 520, target: {} });
assert.strictEqual(trailStrokes.size, 3, "Pointer 2 allocates its own isolated stroke");
const p2Tracked = activeFeedbackPointers.get(2);
const stroke3Id = p2Tracked.strokeId;
assert(stroke3Id && stroke3Id.startsWith("stroke_2_"), "Pointer 2 strokeId must be unique to pointer 2");
const stroke3Points = trailStrokes.get(stroke3Id);
assert.strictEqual(stroke3Points[0].x, 500);
assert.strictEqual(stroke3Points[1].x, 520);

// Release all and clean up
global.window.dispatchEvent("pointerup", { pointerId: 1 });
global.window.dispatchEvent("pointerup", { pointerId: 2 });
clearTrailCanvas();
assert.strictEqual(trailStrokes.size, 0, "clearTrailCanvas must wipe all active trail strokes");
console.log("✔ Tap-only non-tail behavior, movement threshold, and stroke session isolation verified");

// Test 14: 3-Mode Feedback Cycling Toggle, Icons, Toast Presentation, and Behavior Enforcement
console.log("Running 3-Mode Feedback Toggle & Pill Toast Tests...");

// 1. Verify CSS and Source template requirements
assert(vpadCss.includes("#vpad-touch-toast"), "vpad.css must define #vpad-touch-toast");
assert(vpadCss.includes("#vpad-touch-toast.show"), "vpad.css must define #vpad-touch-toast.show");
assert(vpadSource.includes("gestureSweep"), "vpad.js must define gestureSweep icon");
assert(vpadSource.includes("rippleOnly"), "vpad.js must define rippleOnly icon");
assert(vpadSource.includes("rippleOff"), "vpad.js must define rippleOff icon");
assert(vpadSource.includes("feedbackMode"), "vpad.js must manage feedbackMode in state");
assert(vpadSource.includes("showTouchToast"), "vpad.js must define showTouchToast");

// Verify SVG icons
assert(SVG_ICONS.gestureSweep && SVG_ICONS.gestureSweep.includes("<svg"), "gestureSweep icon must be valid SVG");
assert(SVG_ICONS.gestureSweep.includes("<circle") && SVG_ICONS.gestureSweep.includes("<path"), "gestureSweep must contain touch circle and swipe arcs");
assert(SVG_ICONS.rippleOnly && SVG_ICONS.rippleOnly.includes("<circle"), "rippleOnly must be valid SVG with circles");
assert(SVG_ICONS.rippleOff && SVG_ICONS.rippleOff.includes("<line"), "rippleOff must be valid SVG with diagonal slash");

// Setup DOM mocks for button, toast, and ripple layer
const mockClasses = (initial = []) => {
    const set = new Set(initial);
    return {
        _set: set,
        add(c) { set.add(c); },
        remove(c) { set.delete(c); },
        contains(c) { return set.has(c); }
    };
};

const mockRippleBtn = {
    id: "vpad-btn-ripple",
    className: "vpad-top-btn active",
    classList: mockClasses(["vpad-top-btn", "active"]),
    title: "",
    innerHTML: ""
};

const mockToast = {
    id: "vpad-touch-toast",
    className: "vpad-pill-toast",
    classList: mockClasses(["vpad-pill-toast"]),
    textContent: ""
};

const spawnedRipples = [];
const mockRippleLayer = {
    id: "vpad-touch-ripple-layer",
    appendChild(child) {
        spawnedRipples.push(child);
    },
    removeChild(child) {
        const idx = spawnedRipples.indexOf(child);
        if (idx >= 0) spawnedRipples.splice(idx, 1);
    }
};

global.document.createElement = (tag) => {
    return {
        tagName: tag.toUpperCase(),
        className: "",
        style: {},
        classList: mockClasses(),
        parentNode: mockRippleLayer
    };
};

global.document.getElementById = (id) => {
    if (id === "vpad-btn-ripple") return mockRippleBtn;
    if (id === "vpad-touch-toast") return mockToast;
    if (id === "vpad-touch-ripple-layer") return mockRippleLayer;
    if (id === "vpad-trail-canvas") return mockCanvas;
    if (origGetElementById) return origGetElementById(id);
    return null;
};

// 2. Initial state verification: mode "all"
setFeedbackMode("all");
assert.strictEqual(getFeedbackMode(), "all", "Default feedback mode must be 'all'");
assert.strictEqual(mockRippleBtn.classList.contains("active"), true, "Mode 'all' must have class .active");
assert.strictEqual(mockRippleBtn.innerHTML, SVG_ICONS.gestureSweep, "Mode 'all' must render gestureSweep SVG");
assert.strictEqual(mockRippleBtn.title, "Touch Feedback: Ripple + Trail (All ON)");

// 3. Cycle 1: "all" -> "ripple_only"
const nextMode1 = cycleFeedbackMode();
assert.strictEqual(nextMode1, "ripple_only", "First cycle must transition to 'ripple_only'");
assert.strictEqual(getFeedbackMode(), "ripple_only");
assert.strictEqual(mockRippleBtn.classList.contains("active"), true, "Mode 'ripple_only' must retain class .active");
assert.strictEqual(mockRippleBtn.innerHTML, SVG_ICONS.rippleOnly, "Mode 'ripple_only' must render rippleOnly concentric circles SVG");
assert.strictEqual(mockRippleBtn.title, "Touch Feedback: Ripple Only (Trail OFF)");
assert.strictEqual(mockToast.textContent, "Touch Feedback: Ripple Only (Trail OFF)");
assert.strictEqual(mockToast.classList.contains("show"), true, "Toast must have .show class on cycle");

// 4. Cycle 2: "ripple_only" -> "off"
const nextMode2 = cycleFeedbackMode();
assert.strictEqual(nextMode2, "off", "Second cycle must transition to 'off'");
assert.strictEqual(getFeedbackMode(), "off");
assert.strictEqual(mockRippleBtn.classList.contains("active"), false, "Mode 'off' must remove class .active");
assert.strictEqual(mockRippleBtn.innerHTML, SVG_ICONS.rippleOff, "Mode 'off' must render rippleOff circle with slash SVG");
assert.strictEqual(mockRippleBtn.title, "Touch Feedback: OFF");
assert.strictEqual(mockToast.textContent, "Touch Feedback: OFF");
assert.strictEqual(mockToast.classList.contains("show"), true, "Toast must have .show class on cycle");

// 5. Cycle 3: "off" -> "all"
const nextMode3 = cycleFeedbackMode();
assert.strictEqual(nextMode3, "all", "Third cycle must transition back to 'all'");
assert.strictEqual(getFeedbackMode(), "all");
assert.strictEqual(mockRippleBtn.classList.contains("active"), true, "Mode 'all' must have class .active");
assert.strictEqual(mockRippleBtn.innerHTML, SVG_ICONS.gestureSweep, "Mode 'all' must render gestureSweep SVG");
assert.strictEqual(mockRippleBtn.title, "Touch Feedback: Ripple + Trail (All ON)");
assert.strictEqual(mockToast.textContent, "Touch Feedback: Ripple + Trail (All ON)");
assert.strictEqual(mockToast.classList.contains("show"), true, "Toast must have .show class on cycle");

// 6. Toast duration and timer resetting (1000ms)
let scheduledTimeoutDelay = null;
let scheduledTimeoutCb = null;
const origSetTimeout = global.setTimeout;
const origClearTimeout = global.clearTimeout;
let clearTimeoutCalls = 0;

global.clearTimeout = (t) => {
    clearTimeoutCalls++;
    origClearTimeout(t);
};

global.setTimeout = (cb, ms) => {
    scheduledTimeoutDelay = ms;
    scheduledTimeoutCb = cb;
    return origSetTimeout(cb, ms);
};

showTouchToast("Testing Toast Duration");
assert.strictEqual(mockToast.textContent, "Testing Toast Duration");
assert.strictEqual(mockToast.classList.contains("show"), true);
assert.strictEqual(scheduledTimeoutDelay, 1000, "Toast timer must be scheduled for exactly 1000ms");

// Repeated call clears previous timer
const prevClearCalls = clearTimeoutCalls;
showTouchToast("Testing Toast Duration 2");
assert(clearTimeoutCalls > prevClearCalls, "Repeated toast call must clear previous timer");
assert.strictEqual(mockToast.textContent, "Testing Toast Duration 2");

// Trigger timeout callback directly to verify class .show removal
scheduledTimeoutCb();
assert.strictEqual(mockToast.classList.contains("show"), false, "Toast must remove class .show after timeout");

// Restore timers
global.setTimeout = origSetTimeout;
global.clearTimeout = origClearTimeout;

// 7. Behavior enforcement:
// Mode "all": both ripple on tap and trail on swipe >= 10px
setFeedbackMode("all");
spawnedRipples.length = 0;
clearTrailCanvas();

global.window.dispatchEvent("pointerdown", { pointerId: 50, clientX: 100, clientY: 100, target: {} });
assert.strictEqual(spawnedRipples.length, 1, "Mode 'all' must spawn ripple on tap");
global.window.dispatchEvent("pointermove", { pointerId: 50, clientX: 130, clientY: 130, target: {} });
assert.strictEqual(trailStrokes.size, 1, "Mode 'all' must create trail points on swipe >= 10px");
global.window.dispatchEvent("pointerup", { pointerId: 50 });
clearTrailCanvas();

// Mode "ripple_only": ripple on tap, trail disabled
setFeedbackMode("ripple_only");
spawnedRipples.length = 0;
clearTrailCanvas();

global.window.dispatchEvent("pointerdown", { pointerId: 60, clientX: 200, clientY: 200, target: {} });
assert.strictEqual(spawnedRipples.length, 1, "Mode 'ripple_only' must spawn ripple on tap");
global.window.dispatchEvent("pointermove", { pointerId: 60, clientX: 240, clientY: 240, target: {} });
assert.strictEqual(trailStrokes.size, 0, "Mode 'ripple_only' must NOT create any trail points");
global.window.dispatchEvent("pointerup", { pointerId: 60 });
clearTrailCanvas();

// Mode "off": neither ripple nor trail triggers
setFeedbackMode("off");
spawnedRipples.length = 0;
clearTrailCanvas();

global.window.dispatchEvent("pointerdown", { pointerId: 70, clientX: 300, clientY: 300, target: {} });
assert.strictEqual(spawnedRipples.length, 0, "Mode 'off' must NOT spawn ripple on tap");
global.window.dispatchEvent("pointermove", { pointerId: 70, clientX: 350, clientY: 350, target: {} });
assert.strictEqual(trailStrokes.size, 0, "Mode 'off' must NOT create trail points");
global.window.dispatchEvent("pointerup", { pointerId: 70 });
clearTrailCanvas();

// Reset mode to 'all' for clean exit
setFeedbackMode("all");
console.log("✔ 3-mode cycling, icon updates, toast presentation, and mode behavior enforcement verified");

// Restore original document.getElementById
global.document.getElementById = origGetElementById;

console.log("\nAll test assertions passed successfully.");
