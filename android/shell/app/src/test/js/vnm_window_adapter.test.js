const assert = require("assert");
const fs = require("fs");
const path = require("path");
const vm = require("vm");

const shimPath = path.resolve(__dirname, "../../main/assets/web_shims/runtimepack_web_shims.js");
const shimCode = fs.readFileSync(shimPath, "utf8");

function createSandbox() {
    const sandbox = {
        console,
        setTimeout,
        clearTimeout,
        setInterval,
        clearInterval,
        Uint8Array,
        ArrayBuffer,
        DataView,
        TextEncoder,
        TextDecoder
    };
    sandbox.window = sandbox;
    sandbox.global = sandbox;
    sandbox.self = sandbox;
    vm.createContext(sandbox);
    vm.runInContext(shimCode, sandbox);
    return sandbox;
}

const ctx = createSandbox();
assert(ctx.nw, "nw must be defined on sandbox");
assert(ctx.nw.Window, "nw.Window must be defined");
assert(typeof ctx.nw.Window.get === "function", "nw.Window.get must be a function");

const win = ctx.nw.Window.get();
assert(win, "nw.Window.get() must return a window instance");

assert(typeof win.setResizable === "function", "win.setResizable must be a function");
win.setResizable(true);
win.setResizable(false);

assert.strictEqual(typeof win.isFullscreen, "boolean", "win.isFullscreen must be a boolean");
win.enterFullscreen();
assert.strictEqual(win.isFullscreen, true, "isFullscreen must be true after enterFullscreen()");
win.leaveFullscreen();
assert.strictEqual(win.isFullscreen, false, "isFullscreen must be false after leaveFullscreen()");

assert(typeof win.on === "function", "win.on must be a function");
let resizeCount = 0;
let lastW = 0;
let lastH = 0;
const onResize = function(w, h) {
    resizeCount++;
    lastW = w;
    lastH = h;
};
win.on("resize", onResize);
assert(typeof win.emit === "function", "win.emit must be a function");
win.emit("resize", 1280, 720);
assert.strictEqual(resizeCount, 1);
assert.strictEqual(lastW, 1280);
assert.strictEqual(lastH, 720);

if (typeof win.removeListener === "function") {
    win.removeListener("resize", onResize);
    win.emit("resize", 1920, 1080);
    assert.strictEqual(resizeCount, 1);
}

function authenticVnmEngineResize(nw) {
    let resized = false;
    nw.Window.get().on("resize", function() {
        resized = true;
    });
    nw.Window.get().emit("resize", 800, 600);
    return resized;
}

function authenticVnmEngineFullscreen(nw) {
    var r = nw.Window.get();
    r.setResizable(true);
    r.enterFullscreen();
    var fs1 = r.isFullscreen;
    r.leaveFullscreen();
    var fs2 = r.isFullscreen;
    return fs1 === true && fs2 === false;
}

assert.strictEqual(authenticVnmEngineResize(ctx.nw), true);
assert.strictEqual(authenticVnmEngineFullscreen(ctx.nw), true);

console.log("vnm_window_adapter tests PASSED");
