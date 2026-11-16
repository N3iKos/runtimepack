const assert = require("assert");
const fs = require("fs");
const path = require("path");
const vm = require("vm");

const vpadPath = path.resolve(__dirname, "../../main/assets/overlay/vpad.js");
const vpadCode = fs.readFileSync(vpadPath, "utf8");

function createMockEnvironment() {
    const keyDownCalls = [];
    const keyUpCalls = [];
    const modalStateCalls = [];
    const gamepadHiddenCalls = [];
    const hitboxesReported = [];

    const textFocusedCalls = [];

    const mockVPad = {
        onKeyDown: (k) => { keyDownCalls.push(k); },
        onKeyUp: (k) => { keyUpCalls.push(k); },
        setOverlayModalOpen: (open) => { modalStateCalls.push(open); },
        setGamepadHidden: (hidden) => { gamepadHiddenCalls.push(hidden); },
        updateControlHitboxes: (json) => { hitboxesReported.push(json); },
        setOverlayTextFocused: (focused) => { textFocusedCalls.push(focused); }
    };

    const listeners = {};
    const win = {
        RuntimePackVPad: mockVPad,
        innerWidth: 1280,
        innerHeight: 720,
        devicePixelRatio: 1,
        localStorage: {
            getItem: () => null,
            setItem: () => {},
            removeItem: () => {}
        },
        addEventListener: (evt, fn) => {
            if (!listeners[evt]) listeners[evt] = [];
            listeners[evt].push(fn);
        },
        removeEventListener: (evt, fn) => {
            if (listeners[evt]) {
                listeners[evt] = listeners[evt].filter(f => f !== fn);
            }
        },
        dispatchEvent: (e) => {
            const list = listeners[e.type] || [];
            list.forEach(f => f(e));
        }
    };

    const docListeners = {};
    const elements = new Map();

    function createElement(tag) {
        const classes = new Set();
        const style = {};
        const elListeners = {};
        return {
            tagName: tag.toUpperCase(),
            classList: {
                add: (c) => classes.add(c),
                remove: (c) => classes.delete(c),
                contains: (c) => classes.has(c),
                toggle: (c, force) => {
                    if (force !== undefined) {
                        if (force) classes.add(c); else classes.delete(c);
                        return force;
                    }
                    if (classes.has(c)) { classes.delete(c); return false; }
                    classes.add(c); return true;
                }
            },
            style,
            innerHTML: "",
            textContent: "",
            value: "",
            title: "",
            children: [],
            appendChild: function(c) { this.children.push(c); return c; },
            removeChild: function(c) {
                this.children = this.children.filter(x => x !== c);
                return c;
            },
            addEventListener: function(evt, fn) {
                if (!elListeners[evt]) elListeners[evt] = [];
                elListeners[evt].push(fn);
            },
            removeEventListener: function(evt, fn) {
                if (elListeners[evt]) elListeners[evt] = elListeners[evt].filter(f => f !== fn);
            },
            querySelector: () => null,
            querySelectorAll: () => [],
            getBoundingClientRect: () => ({ left: 10, top: 10, right: 100, bottom: 100, width: 90, height: 90 })
        };
    }

    const doc = {
        readyState: "loading",
        body: createElement("body"),
        createElement,
        getElementById: (id) => {
            if (!elements.has(id)) {
                elements.set(id, createElement("div"));
            }
            return elements.get(id);
        },
        addEventListener: (evt, fn) => {
            if (!docListeners[evt]) docListeners[evt] = [];
            docListeners[evt].push(fn);
        },
        removeEventListener: (evt, fn) => {
            if (docListeners[evt]) docListeners[evt] = docListeners[evt].filter(f => f !== fn);
        },
        dispatchEvent: (e) => {
            const list = docListeners[e.type] || [];
            list.forEach(f => f(e));
        }
    };

    return {
        win,
        doc,
        mockVPad,
        keyDownCalls,
        keyUpCalls,
        modalStateCalls,
        gamepadHiddenCalls,
        hitboxesReported,
        textFocusedCalls
    };
}

const env = createMockEnvironment();
const sandbox = {
    window: env.win,
    document: env.doc,
    console,
    setTimeout,
    clearTimeout,
    setInterval,
    clearInterval,
    KeyboardEvent: function(type, init) {
        this.type = type;
        this.keyCode = init ? init.keyCode : 0;
    }
};
sandbox.global = sandbox;
sandbox.self = env.win;
vm.createContext(sandbox);

vm.runInContext(vpadCode, sandbox);
assert(typeof sandbox.window.pressKey === "function", "window.pressKey must be defined");
assert(typeof sandbox.window.releaseKey === "function", "window.releaseKey must be defined");

sandbox.window.pressKey(90);
assert(env.keyDownCalls.includes(90), "pressKey(90) must notify onKeyDown(90)");

sandbox.window.releaseKey(90, false);
assert(env.keyUpCalls.includes(90), "releaseKey(90) must notify onKeyUp(90)");

assert(!env.keyDownCalls.includes(32), "Controls eye toggle must never inject Space (key 32)");

if (typeof sandbox.window.setModalState === "function") {
    sandbox.window.setModalState("custom_modal", true);
    assert(env.modalStateCalls.includes(true), "Opening modal must signal setOverlayModalOpen(true)");
    sandbox.window.setModalState("custom_modal", false);
    assert(env.modalStateCalls.includes(false), "Closing modal must signal setOverlayModalOpen(false)");
}

if (typeof sandbox.window.toggleGamepadHidden === "function") {
    sandbox.window.toggleGamepadHidden();
    assert(env.gamepadHiddenCalls.length > 0, "Toggling eye must signal setGamepadHidden");
}

assert(!env.keyDownCalls.includes(32), "Controls eye toggle must never inject Space");

const webActivityPath = path.resolve(__dirname, "../../main/java/io/runtimepack/host/WebActivity.kt");
const webActivityCode = fs.readFileSync(webActivityPath, "utf8");
const marker = "open fun buildKeyDispatchScript(jsKeyCode: Int, type: String): String {";
const scriptStart = webActivityCode.indexOf("(function()", webActivityCode.indexOf(marker));
const scriptEnd = webActivityCode.indexOf('""".trimIndent()', scriptStart);
assert(scriptStart !== -1 && scriptEnd !== -1, "buildKeyDispatchScript must exist in WebActivity.kt");

const rawTemplate = webActivityCode.slice(scriptStart, scriptEnd);
const keydownScript = rawTemplate.replace(/\$type/g, "keydown").replace(/\$jsKeyCode/g, "65");

let windowHandlerCalls = 0;
let documentHandlerCalls = 0;
let capturedEvent = null;

const domListeners = {
    window: [],
    document: []
};

const domWin = {
    addEventListener: (t, fn) => { domListeners.window.push(fn); },
    dispatchEvent: (e) => {
        domListeners.window.forEach(fn => fn(e));
        return true;
    }
};

const domDoc = {
    addEventListener: (t, fn) => { domListeners.document.push(fn); },
    dispatchEvent: (e) => {
        domListeners.document.forEach(fn => fn(e));
        if (e && e.bubbles) {
            domWin.dispatchEvent(e);
        }
        return true;
    }
};

domWin.addEventListener("keydown", (e) => {
    windowHandlerCalls++;
    capturedEvent = e;
});
domDoc.addEventListener("keydown", (e) => {
    documentHandlerCalls++;
});

let engineInputOnKeyDownCalled = false;
let runtimePackInputPressed = null;

const gameSandbox = {
    window: domWin,
    document: domDoc,
    KeyboardEvent: function(type, init) {
        this.type = type;
        this.bubbles = init ? Boolean(init.bubbles) : false;
        this.cancelable = init ? Boolean(init.cancelable) : false;
        this.keyCode = init ? init.keyCode : 0;
    },
    Input: {
        _onKeyDown: (e) => { engineInputOnKeyDownCalled = true; },
        keyMapper: { 65: "ok" }
    },
    RuntimePackInput: {
        isAvailable: () => true,
        press: (action) => { runtimePackInputPressed = action; },
        release: () => {}
    }
};
gameSandbox.window.window = domWin;
gameSandbox.window.document = domDoc;
gameSandbox.window.Input = gameSandbox.Input;
gameSandbox.window.RuntimePackInput = gameSandbox.RuntimePackInput;
gameSandbox.window.KeyboardEvent = gameSandbox.KeyboardEvent;

vm.createContext(gameSandbox);
vm.runInContext(keydownScript, gameSandbox);

assert.strictEqual(documentHandlerCalls, 1, "Document receives event exactly once");
assert.strictEqual(windowHandlerCalls, 1, "Window receives bubbling event exactly once without duplicate");
assert(capturedEvent !== null, "Captured event must not be null");
assert.strictEqual(capturedEvent.keyCode, 65, "keyCode getter returns 65");
assert.strictEqual(capturedEvent.which, 65, "which getter returns 65");
assert.strictEqual(capturedEvent.code, "KeyA", "code getter returns KeyA");
assert.strictEqual(engineInputOnKeyDownCalled, true, "Engine Input._onKeyDown must be called");
assert.strictEqual(runtimePackInputPressed, "ok", "RuntimePackInput.press must receive mapped action");

const webShimsPath = path.resolve(__dirname, "../../main/assets/web_shims/runtimepack_web_shims.js");
const webShimsCode = fs.readFileSync(webShimsPath, "utf8");

function GenuineVnmMouse() {
    this.buttons = new Uint8Array(new ArrayBuffer(4));
    this.buttonDown = false;
    this.buttonUp = false;
}
GenuineVnmMouse.prototype.press = function(t) { return 1 === this.buttons[t]; };
GenuineVnmMouse.prototype.update = function() {
    for (var t = 0; t < this.buttons.length; t++) {
        if (2 === this.buttons[t]) this.buttons[t] = 0;
    }
    this.buttonDown = false;
    this.buttonUp = false;
};
GenuineVnmMouse.prototype.clear = function() {
    for (var t = 0; t < this.buttons.length; t++) {
        this.buttons[t] = 0;
    }
};

const vnmCancelListeners = [];
const vnmWindow = {
    addEventListener: (t, fn, capture) => {
        vnmCancelListeners.push({ type: t, fn: fn, capture: capture });
    },
    dispatchEvent: (e) => {
        vnmCancelListeners.filter(l => l.type === e.type).forEach(l => l.fn(e));
        return true;
    }
};

const mouse = new GenuineVnmMouse();
vnmWindow.Input = { Mouse: mouse };
vnmWindow.window = vnmWindow;

const vnmSandbox = {
    window: vnmWindow,
    root: vnmWindow,
    globalThis: vnmWindow,
    setTimeout: (fn) => {},
    clearTimeout: () => {},
    setInterval: (fn) => {},
    clearInterval: () => {}
};
vm.createContext(vnmSandbox);
vm.runInContext(webShimsCode, vnmSandbox);

mouse.buttons[1] = 1;
mouse.buttonDown = true;
assert.strictEqual(mouse.press(1), true, "Mouse button 1 pressed on touchstart");
assert.strictEqual(mouse.buttonDown, true, "buttonDown set on touchstart");

mouse.update();
assert.strictEqual(mouse.buttonDown, false, "buttonDown resets to false on frame update");
assert.strictEqual(mouse.press(1), true, "Without cancel, buttons[1] remains stuck at 1");

vnmWindow.dispatchEvent({ type: "touchcancel" });
assert.strictEqual(mouse.press(1), false, "touchcancel cleared pressed state");
assert.strictEqual(mouse.buttons[1], 0, "buttons[1] cleared to 0");
assert.strictEqual(mouse.buttonDown, false, "buttonDown is false after cancel");
assert.strictEqual(mouse.buttonUp, false, "buttonUp is false after cancel");

mouse.buttons[1] = 1;
mouse.buttonDown = true;
assert.strictEqual(mouse.press(1), true, "Next tap works: button 1 pressed");
mouse.buttons[1] = 2;
mouse.buttonUp = true;
mouse.update();
assert.strictEqual(mouse.press(1), false, "Next tap touchend completed and released");

const cancelHandlerCount = vnmCancelListeners.filter(l => l.type === "touchcancel").length;
assert.strictEqual(cancelHandlerCount, 1, "Idempotent touchcancel registration attached exactly once");

console.log("vnm_overlay_focus_lifecycle tests PASSED");
