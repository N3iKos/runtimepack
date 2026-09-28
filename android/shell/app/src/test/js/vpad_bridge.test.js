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
const { pressKey, releaseKey, openLinktree, setupDirectManipulation, setupDpadTouch, shouldIgnoreTouchForFeedback } = require(vpadPath);

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

// Test 3: openLinktree() -> window.RuntimePackVPad.openUrl("https://linktr.ee/Veldra.Works")
openUrlCalls.length = 0;
openLinktree();
assert.strictEqual(openUrlCalls.length, 1, "Expected RuntimePackVPad.openUrl to be called exactly once");
assert.strictEqual(openUrlCalls[0], "https://linktr.ee/Veldra.Works", "Expected openUrl to target https://linktr.ee/Veldra.Works");
console.log("✔ openLinktree() calls window.RuntimePackVPad.openUrl(\"https://linktr.ee/Veldra.Works\")");

// Test 3b: window.openLinktree()
openUrlCalls.length = 0;
window.openLinktree();
assert.strictEqual(openUrlCalls.length, 1, "Expected window.openLinktree to call RuntimePackVPad.openUrl");
assert.strictEqual(openUrlCalls[0], "https://linktr.ee/Veldra.Works", "Expected URL to be https://linktr.ee/Veldra.Works");
console.log("✔ window.openLinktree() calls window.RuntimePackVPad.openUrl(\"https://linktr.ee/Veldra.Works\")");

// Test 4: Verify index.html openLinktree() implementation
const indexHtmlPath = path.resolve(__dirname, "../../main/assets/overlay/index.html");
const indexHtmlContent = fs.readFileSync(indexHtmlPath, "utf-8");

// Assert string presence in index.html
assert(
    indexHtmlContent.includes("https://linktr.ee/Veldra.Works"),
    "index.html must contain linktr.ee/Veldra.Works URL"
);
assert(
    !indexHtmlContent.includes("https://github.com/anomalyco/antigravity"),
    "index.html must not contain legacy antigravity link"
);

// Execute index.html script block in isolated VM context to test behavior
const scriptMatches = indexHtmlContent.match(/<script>([\s\S]*?)<\/script>/);
assert(scriptMatches && scriptMatches[1], "index.html must contain an inline <script> block");

const htmlOpenUrlCalls = [];
const htmlSandbox = {
    window: {
        RuntimePackVPad: {
            onKeyDown: () => {},
            onKeyUp: () => {},
            openUrl: (url) => {
                htmlOpenUrlCalls.push(url);
            }
        },
        addEventListener: () => {}
    }
};
vm.createContext(htmlSandbox);
vm.runInContext(scriptMatches[1], htmlSandbox);

assert(typeof htmlSandbox.window.openLinktree === "function", "index.html must define window.openLinktree");
htmlSandbox.window.openLinktree();
assert.strictEqual(htmlOpenUrlCalls.length, 1, "index.html window.openLinktree should call openUrl once");
assert.strictEqual(htmlOpenUrlCalls[0], "https://linktr.ee/Veldra.Works", "index.html window.openLinktree must call openUrl with https://linktr.ee/Veldra.Works");
console.log("✔ index.html openLinktree() calls window.RuntimePackVPad.openUrl(\"https://linktr.ee/Veldra.Works\")");

// Test 5: Graceful degradation when window.RuntimePackVPad is absent
global.window.RuntimePackVPad = null;
assert.doesNotThrow(() => {
    pressKey(90);
    releaseKey(90);
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

// Test 11: Universal Thin-Ring Visual Ripple, Swipe Motion Trail, and Touch Feedback Filtering
const vpadCss = fs.readFileSync(path.join(__dirname, "../../main/assets/overlay/vpad.css"), "utf8");
// 1. Universal thin 1.5px white ring styling across themes
assert(vpadCss.includes('border: 1.5px solid rgba(255, 255, 255, 0.9) !important;'), "vpad.css must define universal 1.5px thin white ring");
assert(vpadCss.includes('background: radial-gradient(circle, rgba(255, 255, 255, 0.12) 0%, rgba(255, 255, 255, 0.03) 55%, transparent 75%) !important;'), "vpad.css must define subtle inner gradient");
assert(vpadCss.includes('box-shadow: 0 0 6px rgba(0, 0, 0, 0.45) !important;'), "vpad.css must define subtle 6px shadow");
// 2. Swipe motion trail styling and animation
assert(vpadCss.includes('.vpad-touch-tail-segment'), "vpad.css must define .vpad-touch-tail-segment");
assert(vpadCss.includes('background: rgba(255, 255, 255, 0.45);'), "vpad.css must style tail segment with 0.45 background opacity");
assert(vpadCss.includes('border: 1px solid rgba(255, 255, 255, 0.8);'), "vpad.css must style tail segment with 1px solid white border");
assert(vpadCss.includes('animation: vpad-tail-fade 0.22s ease-out forwards;'), "vpad.css must define vpad-tail-fade 0.22s animation");
assert(vpadCss.includes('@keyframes vpad-tail-fade'), "vpad.css must define @keyframes vpad-tail-fade");

// 3. Filter controls from touch feedback in vpad.js
assert(vpadSource.includes('shouldIgnoreTouchForFeedback'), "vpad.js must implement shouldIgnoreTouchForFeedback");
assert(vpadSource.includes('.vpad-control-item'), "vpad.js must check .vpad-control-item to ignore button touches for ripples/trails");
assert(vpadSource.includes('vpad-touch-tail-segment'), "vpad.js must reference vpad-touch-tail-segment");

const makeMockTarget = (matchedSelector) => ({
    closest: (sel) => sel === matchedSelector ? { className: matchedSelector } : null
});

assert.strictEqual(typeof shouldIgnoreTouchForFeedback, "function", "shouldIgnoreTouchForFeedback must be a function");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget(".vpad-control-item")), true, "must ignore .vpad-control-item");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget("#vpad-topbar")), true, "must ignore #vpad-topbar");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget("#vpad-editor-bar")), true, "must ignore #vpad-editor-bar");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget("#vpad-prop-panel")), true, "must ignore #vpad-prop-panel");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget("#vpad-key-modal")), true, "must ignore #vpad-key-modal");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget("#fnh-cheat-overlay")), true, "must ignore #fnh-cheat-overlay");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget("#rp-diag-modal")), true, "must ignore #rp-diag-modal");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget(".vpad-popover")), true, "must ignore .vpad-popover");
assert.strictEqual(shouldIgnoreTouchForFeedback(makeMockTarget("canvas")), false, "must NOT ignore game canvas");
assert.strictEqual(shouldIgnoreTouchForFeedback(null), false, "must NOT ignore null target");
assert.strictEqual(shouldIgnoreTouchForFeedback({}), false, "must NOT ignore plain object without closest/matches");
console.log("✔ Universal thin-ring ripple, .vpad-touch-tail-segment, and control feedback filtering verified");

console.log("\nAll test assertions passed successfully.");
