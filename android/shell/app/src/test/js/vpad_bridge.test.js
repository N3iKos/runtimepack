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

const mockRuntimePackVPad = {
    onKeyDown: (keyCode) => {
        keyDownCalls.push(keyCode);
    },
    onKeyUp: (keyCode) => {
        keyUpCalls.push(keyCode);
    },
    openUrl: (url) => {
        openUrlCalls.push(url);
    }
};

global.window = {
    RuntimePackVPad: mockRuntimePackVPad
};

// Load vpad.js
const vpadPath = path.resolve(__dirname, "../../main/assets/overlay/vpad.js");
const { pressKey, releaseKey, openLinktree } = require(vpadPath);

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

// Test 3: openLinktree() -> window.RuntimePackVPad.openUrl("https://linktr.ee/VenoraTL")
openUrlCalls.length = 0;
openLinktree();
assert.strictEqual(openUrlCalls.length, 1, "Expected RuntimePackVPad.openUrl to be called exactly once");
assert.strictEqual(openUrlCalls[0], "https://linktr.ee/VenoraTL", "Expected openUrl to target https://linktr.ee/VenoraTL");
console.log("✔ openLinktree() calls window.RuntimePackVPad.openUrl(\"https://linktr.ee/VenoraTL\")");

// Test 3b: window.openLinktree()
openUrlCalls.length = 0;
window.openLinktree();
assert.strictEqual(openUrlCalls.length, 1, "Expected window.openLinktree to call RuntimePackVPad.openUrl");
assert.strictEqual(openUrlCalls[0], "https://linktr.ee/VenoraTL", "Expected URL to be https://linktr.ee/VenoraTL");
console.log("✔ window.openLinktree() calls window.RuntimePackVPad.openUrl(\"https://linktr.ee/VenoraTL\")");

// Test 4: Verify index.html openLinktree() implementation
const indexHtmlPath = path.resolve(__dirname, "../../main/assets/overlay/index.html");
const indexHtmlContent = fs.readFileSync(indexHtmlPath, "utf-8");

// Assert string presence in index.html
assert(
    indexHtmlContent.includes("https://linktr.ee/VenoraTL"),
    "index.html must contain linktr.ee/VenoraTL URL"
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
assert.strictEqual(htmlOpenUrlCalls[0], "https://linktr.ee/VenoraTL", "index.html window.openLinktree must call openUrl with https://linktr.ee/VenoraTL");
console.log("✔ index.html openLinktree() calls window.RuntimePackVPad.openUrl(\"https://linktr.ee/VenoraTL\")");

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

console.log("\nAll 7 test assertions passed successfully (100% pass).");
