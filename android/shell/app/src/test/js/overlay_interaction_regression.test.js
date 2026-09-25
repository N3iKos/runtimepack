const assert = require("assert");
const fs = require("fs");
const path = require("path");
const vm = require("vm");

const overlayDir = path.resolve(__dirname, "../../main/assets/overlay");
const vpadCode = fs.readFileSync(path.join(overlayDir, "vpad.js"), "utf8");
const cheatCode = fs.readFileSync(path.join(overlayDir, "cheat_menu.js"), "utf8");
const diagnosticCode = fs.readFileSync(path.join(overlayDir, "diagnostic_suite.js"), "utf8");

function createEnvironment(options = {}) {
    const ids = new Map();
    const pointerCapture = new Map();
    let now = 1000;

    function parseStyle(style, value) {
        String(value).split(";").forEach(rule => {
            const colon = rule.indexOf(":");
            if (colon < 0) return;
            const name = rule.slice(0, colon).trim().replace(/-([a-z])/g, (_, c) => c.toUpperCase());
            style[name] = rule.slice(colon + 1).trim();
        });
    }

    function selectorMatches(el, selector) {
        if (!el || !el.tagName || !el.classList) return false;
        return selector.split(",").some(raw => {
            const sel = raw.trim();
            if (!sel) return false;
            const descendant = sel.lastIndexOf(" ");
            if (descendant >= 0) {
                const parentSel = sel.slice(0, descendant).trim();
                const childSel = sel.slice(descendant + 1).trim();
                if (!selectorMatches(el, childSel)) return false;
                let parent = el.parentElement;
                while (parent) {
                    if (selectorMatches(parent, parentSel)) return true;
                    parent = parent.parentElement;
                }
                return false;
            }
            const attr = sel.match(/^(.*?)\[([\w:-]+)(?:=["']?([^\]"']+)["']?)?\]$/);
            if (attr) {
                if (attr[1] && !selectorMatches(el, attr[1])) return false;
                const actual = el.getAttribute(attr[2]);
                return attr[3] === undefined ? actual !== null : actual === attr[3];
            }
            if (sel.startsWith("#")) return el.id === sel.slice(1);
            if (sel.startsWith(".")) return el.classList.contains(sel.slice(1));
            const parts = sel.split(".");
            if (parts.length === 2) return el.tagName.toLowerCase() === parts[0].toLowerCase() && el.classList.contains(parts[1]);
            return el.tagName.toLowerCase() === sel.toLowerCase();
        });
    }

    function unregister(node) {
        if (node.id) ids.delete(node.id);
        node.children.forEach(unregister);
    }

    function register(node) {
        if (node.id) ids.set(node.id, node);
        node.children.forEach(register);
    }

    function parseHTML(html, parent, doc) {
        parent.children.forEach(unregister);
        parent.children = [];
        parent._text = "";
        const stack = [parent];
        const voids = new Set(["input", "br", "hr", "img", "meta", "link"]);
        const tokens = /<(\/)?([\w-]+)([^>]*)>|([^<]+)/g;
        let match;
        while ((match = tokens.exec(String(html)))) {
            if (match[4]) {
                stack[stack.length - 1]._text += match[4];
                continue;
            }
            if (match[1]) {
                if (stack.length > 1) stack.pop();
                continue;
            }
            const el = doc.createElement(match[2]);
            const attrs = match[3] || "";
            const attrPattern = /([\w:-]+)(?:=(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g;
            let attr;
            while ((attr = attrPattern.exec(attrs))) {
                el.setAttribute(attr[1], attr[2] !== undefined ? attr[2] : attr[3] !== undefined ? attr[3] : attr[4] !== undefined ? attr[4] : "");
            }
            stack[stack.length - 1].appendChild(el);
            if (!voids.has(match[2].toLowerCase()) && !attrs.trim().endsWith("/")) stack.push(el);
        }
    }

    function makeEventTarget(target) {
        target._listeners = {};
        target.addEventListener = function (type, handler, opts) {
            if (!this._listeners[type]) this._listeners[type] = [];
            this._listeners[type].push({ handler, capture: opts === true || !!(opts && opts.capture) });
        };
        target.removeEventListener = function (type, handler) {
            if (!this._listeners[type]) return;
            this._listeners[type] = this._listeners[type].filter(item => item.handler !== handler);
        };
        target.listenerCount = function (type) {
            return (this._listeners[type] || []).length;
        };
        return target;
    }

    function normalizeEvent(event, target) {
        const evt = typeof event === "string" ? { type: event } : event;
        if (!evt.type) throw new Error("Event type is required");
        if (!evt.target) evt.target = target;
        evt.currentTarget = null;
        evt.cancelBubble = false;
        evt.defaultPrevented = false;
        evt.cancelable = evt.cancelable !== false;
        evt.stopPropagation = evt.stopPropagation || function () { this.cancelBubble = true; };
        evt.preventDefault = evt.preventDefault || function () { if (this.cancelable) this.defaultPrevented = true; };
        return evt;
    }

    function invoke(node, event, capture) {
        event.currentTarget = node;
        (node._listeners[event.type] || []).slice().filter(item => item.capture === capture).forEach(item => item.handler.call(node, event));
        if (!capture && typeof node["on" + event.type] === "function") node["on" + event.type].call(node, event);
    }

    function dispatchFrom(target, event) {
        const evt = normalizeEvent(event, target);
        if (evt.pointerId !== undefined && evt.type.startsWith("pointer") && evt.type !== "pointerdown" && pointerCapture.has(evt.pointerId)) {
            target = pointerCapture.get(evt.pointerId);
            evt.target = target;
        }
        const chain = [];
        let node = target;
        while (node) {
            chain.unshift(node);
            node = node.parentElement || (node === doc ? win : null);
        }
        if (!chain.includes(doc)) chain.unshift(doc);
        if (!chain.includes(win)) chain.unshift(win);
        for (const current of chain) {
            invoke(current, evt, true);
            if (evt.cancelBubble) return !evt.defaultPrevented;
        }
        for (let i = chain.length - 1; i >= 0; i--) {
            invoke(chain[i], evt, false);
            if (evt.cancelBubble) break;
        }
        return !evt.defaultPrevented;
    }

    function createElement(tagName) {
        const el = makeEventTarget({
            tagName: String(tagName).toUpperCase(),
            id: "",
            className: "",
            attributes: {},
            children: [],
            parentElement: null,
            parentNode: null,
            _text: "",
            _innerHTML: "",
            value: "",
            title: "",
            offsetWidth: 300,
            offsetHeight: 200,
            style: {
                setProperty(name, value) { this[name] = String(value); },
                getPropertyValue(name) { return this[name] || ""; }
            },
            setAttribute(name, value) {
                const text = String(value);
                this.attributes[name] = text;
                if (name === "id") {
                    if (this.id) ids.delete(this.id);
                    this.id = text;
                    ids.set(text, this);
                } else if (name === "class") this.className = text;
                else if (name === "style") parseStyle(this.style, text);
                else if (name === "value") this.value = text;
            },
            getAttribute(name) { return this.attributes[name] !== undefined ? this.attributes[name] : null; },
            removeAttribute(name) { delete this.attributes[name]; },
            appendChild(child) {
                if (child.parentElement) child.parentElement.removeChild(child);
                child.parentElement = this;
                child.parentNode = this;
                this.children.push(child);
                register(child);
                return child;
            },
            removeChild(child) {
                const index = this.children.indexOf(child);
                if (index >= 0) this.children.splice(index, 1);
                unregister(child);
                child.parentElement = null;
                child.parentNode = null;
                return child;
            },
            querySelectorAll(selector) {
                const result = [];
                const visit = current => current.children.forEach(child => {
                    if (selectorMatches(child, selector)) result.push(child);
                    visit(child);
                });
                visit(this);
                return result;
            },
            querySelector(selector) { return this.querySelectorAll(selector)[0] || null; },
            closest(selector) {
                let current = this;
                while (current) {
                    if (selectorMatches(current, selector)) return current;
                    current = current.parentElement;
                }
                return null;
            },
            getBoundingClientRect() {
                const left = parseFloat(this.style.left) || this._rectLeft || 0;
                const top = parseFloat(this.style.top) || this._rectTop || 0;
                const width = this._rectWidth || this.offsetWidth;
                const height = this._rectHeight || this.offsetHeight;
                return { left, top, width, height, right: left + width, bottom: top + height };
            },
            setPointerCapture(pointerId) { pointerCapture.set(pointerId, this); },
            releasePointerCapture(pointerId) { if (pointerCapture.get(pointerId) === this) pointerCapture.delete(pointerId); },
            hasPointerCapture(pointerId) { return pointerCapture.get(pointerId) === this; },
            dispatchEvent(event) { return dispatchFrom(this, event); },
            click() { return this.dispatchEvent({ type: "click", cancelable: true }); }
        });
        Object.defineProperties(el, {
            classList: { get() {
                const self = this;
                return {
                    add(...names) { const values = new Set(self.className.split(/\s+/).filter(Boolean)); names.forEach(name => values.add(name)); self.className = [...values].join(" "); },
                    remove(...names) { const values = new Set(self.className.split(/\s+/).filter(Boolean)); names.forEach(name => values.delete(name)); self.className = [...values].join(" "); },
                    contains(name) { return self.className.split(/\s+/).filter(Boolean).includes(name); },
                    toggle(name, force) { const enabled = force === undefined ? !this.contains(name) : force; enabled ? this.add(name) : this.remove(name); return enabled; }
                };
            } },
            textContent: { get() { return this._text + this.children.map(child => child.textContent).join(""); }, set(value) { this.children.forEach(unregister); this.children = []; this._text = String(value); } },
            innerHTML: { get() { return this._innerHTML; }, set(value) { this._innerHTML = String(value); parseHTML(value, this, doc); } }
        });
        return el;
    }

    const doc = makeEventTarget({
        readyState: options.readyState || "loading",
        hidden: false,
        createElement,
        getElementById(id) { return ids.get(id) || null; },
        querySelectorAll(selector) { return this.body.querySelectorAll(selector).concat(this.head.querySelectorAll(selector)); },
        querySelector(selector) { return this.querySelectorAll(selector)[0] || null; },
        dispatchEvent(event) { return dispatchFrom(this, event); }
    });
    doc.head = createElement("head");
    doc.body = createElement("body");
    doc.head.parentElement = doc;
    doc.body.parentElement = doc;

    const storage = { data: {}, getItem(key) { return this.data[key] !== undefined ? this.data[key] : null; }, setItem(key, value) { this.data[key] = String(value); }, removeItem(key) { delete this.data[key]; } };
    const timers = [];
    const win = makeEventTarget({
        document: doc,
        innerWidth: 1000,
        innerHeight: 700,
        PointerEvent: function PointerEvent() {},
        localStorage: storage,
        dispatchEvent(event) { return dispatchFrom(this, event); }
    });
    win.window = win;
    doc.defaultView = win;

    const sandbox = {
        window: win,
        document: doc,
        global: win,
        globalThis: win,
        localStorage: storage,
        console: { log() {}, warn() {}, error() {}, info() {} },
        Date: class extends Date { static now() { return now; } },
        Math,
        JSON,
        Set,
        Map,
        parseInt,
        parseFloat,
        isNaN,
        confirm: () => true,
        KeyboardEvent: function KeyboardEvent(type, init) { return { type, ...init }; },
        performance: { now: () => now, memory: {} },
        requestAnimationFrame: () => 1,
        cancelAnimationFrame() {},
        setInterval: () => 1,
        clearInterval() {},
        setTimeout(fn, delay) { timers.push({ fn, delay }); return timers.length; },
        clearTimeout() {},
        module: { exports: {} },
        exports: {},
        require
    };
    win.setTimeout = sandbox.setTimeout;
    win.clearTimeout = sandbox.clearTimeout;
    win.setInterval = sandbox.setInterval;
    win.clearInterval = sandbox.clearInterval;
    win.requestAnimationFrame = sandbox.requestAnimationFrame;
    win.cancelAnimationFrame = sandbox.cancelAnimationFrame;
    vm.createContext(sandbox);

    return {
        sandbox,
        window: win,
        document: doc,
        advance(ms) { now += ms; },
        runTimers(delay) {
            const pending = timers.splice(0, timers.length);
            pending.filter(timer => delay === undefined || timer.delay === delay).forEach(timer => timer.fn());
            pending.filter(timer => delay !== undefined && timer.delay !== delay).forEach(timer => timers.push(timer));
        },
        run(code, filename) {
            sandbox.module = { exports: {} };
            sandbox.exports = sandbox.module.exports;
            vm.runInContext(code, sandbox, { filename });
            return sandbox.module.exports;
        }
    };
}

function physicalTap(element, env) {
    element.dispatchEvent({ type: "pointerdown", pointerId: 7, clientX: 20, clientY: 20, cancelable: true });
    env.advance(220);
    element.dispatchEvent({ type: "touchstart", touches: [{ identifier: 7, clientX: 20, clientY: 20 }], changedTouches: [{ identifier: 7, clientX: 20, clientY: 20 }], cancelable: true });
    env.advance(220);
    element.dispatchEvent({ type: "mousedown", clientX: 20, clientY: 20, cancelable: true });
    env.advance(220);
    element.dispatchEvent({ type: "click", clientX: 20, clientY: 20, cancelable: true });
}

function test(name, fn) {
    try {
        fn();
        console.log(`PASS ${name}`);
    } catch (error) {
        console.error(`FAIL ${name}`);
        console.error(error.stack);
        process.exitCode = 1;
    }
}

test("one Android WebView physical tap invokes a VPad toolbar action once", () => {
    const env = createEnvironment({ readyState: "loading" });
    let toggles = 0;
    env.window.CheatMenu = { toggle() { toggles++; } };
    env.run(vpadCode, "vpad.js");
    env.document.dispatchEvent({ type: "DOMContentLoaded" });
    physicalTap(env.document.getElementById("vpad-btn-cheat"), env);
    assert.strictEqual(toggles, 1, "pointerdown/touchstart/mousedown/click from one tap must produce one toggle");
});

test("concurrent panels retain native modal ownership until the final owner closes", () => {
    const env = createEnvironment({ readyState: "complete" });
    const bridgeCalls = [];
    env.window.RuntimePackVPad = {
        setOverlayModalOpen(active) { bridgeCalls.push(active); }
    };
    env.run(vpadCode, "vpad.js");
    env.document.dispatchEvent({ type: "DOMContentLoaded" });
    const CheatMenu = env.run(cheatCode, "cheat_menu.js");
    const DiagnosticSuite = env.run(diagnosticCode, "diagnostic_suite.js");
    env.runTimers();
    CheatMenu.initDOM();
    DiagnosticSuite.init();

    env.window.RuntimePackOverlayPanels.open("settings");
    CheatMenu.open();
    DiagnosticSuite.open();
    DiagnosticSuite.close();
    assert.strictEqual(bridgeCalls[bridgeCalls.length - 1], true, "closing Diagnostic must preserve Settings + Cheat ownership");
    CheatMenu.close();
    assert.strictEqual(bridgeCalls[bridgeCalls.length - 1], true, "closing Cheat must preserve Settings ownership");
    env.window.RuntimePackOverlayPanels.close("settings");
    assert.strictEqual(bridgeCalls[bridgeCalls.length - 1], false, "closing the final Settings owner must release native modal routing");
});

test("duplicate panel open and close calls are idempotent", () => {
    const env = createEnvironment({ readyState: "loading" });
    const bridgeCalls = [];
    env.window.RuntimePackVPad = {
        setOverlayModalOpen(active) { bridgeCalls.push(active); }
    };
    env.run(vpadCode, "vpad.js");

    const registry = env.window.RuntimePackOverlayPanels;
    registry.open("custom-control");
    registry.open("custom-control");
    assert.strictEqual(registry.size(), 1, "duplicate open must retain one owner");
    registry.close("custom-control");
    registry.close("custom-control");
    assert.strictEqual(registry.size(), 0, "duplicate close must keep registry empty");
    assert.deepStrictEqual(bridgeCalls, [true, false], "native bridge must only receive aggregate state transitions");
});

test("CheatMenu DOM-ready and load startup attach one cheat-button action", () => {
    const env = createEnvironment({ readyState: "loading" });
    env.run(vpadCode, "vpad.js");
    env.document.dispatchEvent({ type: "DOMContentLoaded" });
    const CheatMenu = env.run(cheatCode, "cheat_menu.js");
    env.document.dispatchEvent({ type: "DOMContentLoaded" });
    env.runTimers(250);
    env.window.dispatchEvent({ type: "load" });
    env.runTimers(800);
    const button = env.document.getElementById("vpad-btn-cheat");
    const before = env.document.getElementById("fnh-cheat-overlay").style.display;
    button.dispatchEvent({ type: "click", cancelable: true });
    const after = env.document.getElementById("fnh-cheat-overlay").style.display;
    assert.notStrictEqual(after, before, "one click must toggle the CheatMenu exactly once");
    assert.strictEqual(button.listenerCount("click"), 2, "button must have one VPad click listener and one CheatMenu click listener");
    assert(CheatMenu);
});

test("repeated preset switches keep window control listener counts stable", () => {
    const env = createEnvironment({ readyState: "loading" });
    env.run(vpadCode, "vpad.js");
    env.document.dispatchEvent({ type: "DOMContentLoaded" });
    const tracked = ["touchmove", "touchend", "touchcancel", "mousemove", "mouseup"];
    const baseline = Object.fromEntries(tracked.map(type => [type, env.window.listenerCount(type)]));
    for (let index = 0; index < 8; index++) env.window.vpadSwitchPreset(index % 2 ? "preset1" : "preset2");
    const actual = Object.fromEntries(tracked.map(type => [type, env.window.listenerCount(type)]));
    assert.deepStrictEqual(actual, baseline, "renderControls must not retain window listeners from discarded controls");
});

test("preset and theme popovers stack above an elevated editor bar", () => {
    const env = createEnvironment({ readyState: "loading" });
    env.run(vpadCode, "vpad.js");
    env.document.dispatchEvent({ type: "DOMContentLoaded" });
    const gear = env.document.getElementById("vpad-btn-gear");
    gear.dispatchEvent({ type: "click", cancelable: true });
    const editor = env.document.getElementById("vpad-editor-bar");
    editor.dispatchEvent({ type: "pointerdown", pointerId: 1, cancelable: true });
    for (const pair of [["vpad-bar-preset-btn", "vpad-preset-popover"], ["vpad-bar-theme-btn", "vpad-theme-popover"]]) {
        env.advance(300);
        env.document.getElementById(pair[0]).dispatchEvent({ type: "click", cancelable: true });
        const popover = env.document.getElementById(pair[1]);
        assert(popover.classList.contains("show"), `${pair[1]} must open`);
        assert(Number(popover.style.zIndex || 0) > Number(editor.style.zIndex || 0), `${pair[1]} must stack above the editor bar`);
    }
});

function assertCapturedPointerDrag(code, filename, initExpression, modalId, headerId) {
    const env = createEnvironment({ readyState: "complete" });
    env.run(code, filename);
    env.runTimers();
    vm.runInContext(`window.${initExpression}`, env.sandbox);
    const modal = env.document.getElementById(modalId);
    const header = env.document.getElementById(headerId);
    modal._rectLeft = 120;
    modal._rectTop = 90;
    header.dispatchEvent({ type: "pointerdown", pointerId: 42, clientX: 150, clientY: 110, target: header, cancelable: true });
    assert(header.hasPointerCapture(42), `${headerId} must capture the active pointer`);
    env.window.dispatchEvent({ type: "pointermove", pointerId: 42, clientX: 210, clientY: 160, cancelable: true });
    assert.strictEqual(modal.style.transform, "none", `${modalId} must remove centering transform at drag start`);
    assert.strictEqual(modal.style.left, "180px", `${modalId} must update left during the first captured pointermove`);
    assert.strictEqual(modal.style.top, "140px", `${modalId} must update top during the first captured pointermove`);
}

test("Diagnostic panel updates position on the first captured pointermove", () => {
    assertCapturedPointerDrag(diagnosticCode, "diagnostic_suite.js", "DiagnosticSuite.init()", "rp-diag-modal", "rp-diag-header");
});

test("Cheat panel updates position on the first captured pointermove", () => {
    assertCapturedPointerDrag(cheatCode, "cheat_menu.js", "CheatMenu.initDOM()", "fnh-cheat-overlay", "fnh-cheat-header");
});

test("Panel registry coordinates settings, cheat, and diag modal bridge states", () => {
    const env = createEnvironment({ readyState: "complete" });
    const bridgeCalls = [];
    env.window.RuntimePackVPad = {
        setOverlayModalOpen(active) { bridgeCalls.push(active); }
    };
    env.run(vpadCode, "vpad.js");
    env.document.dispatchEvent({ type: "DOMContentLoaded" });
    const CheatMenu = env.run(cheatCode, "cheat_menu.js");
    const DiagnosticSuite = env.run(diagnosticCode, "diagnostic_suite.js");
    env.runTimers();
    CheatMenu.initDOM();
    DiagnosticSuite.init();

    assert(env.window.RuntimePackModalRegistry instanceof Set, "RuntimePackModalRegistry must be a Set instance");
    
    // Open settings (via setModalState)
    env.window.setModalState("settings", true);
    assert.strictEqual(bridgeCalls[bridgeCalls.length - 1], true, "open settings must set bridge true");
    assert(env.window.RuntimePackModalRegistry.has("settings"), "settings must be registered");

    // Open cheat
    CheatMenu.open();
    assert.strictEqual(bridgeCalls[bridgeCalls.length - 1], true, "open cheat keeps bridge true");
    assert(env.window.RuntimePackModalRegistry.has("cheat"), "cheat must be registered");

    // Open diag
    DiagnosticSuite.open();
    assert.strictEqual(bridgeCalls[bridgeCalls.length - 1], true, "open diag keeps bridge true");
    assert(env.window.RuntimePackModalRegistry.has("diag"), "diag must be registered");
    assert.strictEqual(env.window.RuntimePackModalRegistry.size, 3, "registry has 3 panels open");

    // Close diag -> bridge still true
    DiagnosticSuite.close();
    assert.strictEqual(bridgeCalls[bridgeCalls.length - 1], true, "close diag keeps bridge true");
    assert.strictEqual(env.window.RuntimePackModalRegistry.has("diag"), false, "diag was removed");
    assert.strictEqual(env.window.RuntimePackModalRegistry.size, 2, "2 panels remain");

    // Close cheat -> bridge still true
    CheatMenu.close();
    assert.strictEqual(bridgeCalls[bridgeCalls.length - 1], true, "close cheat keeps bridge true");
    assert.strictEqual(env.window.RuntimePackModalRegistry.has("cheat"), false, "cheat was removed");
    assert.strictEqual(env.window.RuntimePackModalRegistry.size, 1, "1 panel remains");

    // Settings remains open and interactive
    assert(env.window.RuntimePackModalRegistry.has("settings"), "settings is still registered");

    // Close settings -> bridge false
    env.window.setModalState("settings", false);
    assert.strictEqual(bridgeCalls[bridgeCalls.length - 1], false, "close settings releases bridge to false");
    assert.strictEqual(env.window.RuntimePackModalRegistry.size, 0, "registry is empty");
});

test("Ghost click: tapping collapse button does not trigger done button", () => {
    const env = createEnvironment({ readyState: "complete" });
    env.run(vpadCode, "vpad.js");
    env.document.dispatchEvent({ type: "DOMContentLoaded" });

    // Enter edit mode
    const gearBtn = env.document.getElementById("vpad-btn-gear");
    gearBtn.dispatchEvent({ type: "pointerdown", pointerId: 1, clientX: 10, clientY: 10, cancelable: true });
    gearBtn.dispatchEvent({ type: "pointerup", pointerId: 1, clientX: 10, clientY: 10, cancelable: true });
    
    const editorBar = env.document.getElementById("vpad-editor-bar");
    assert(editorBar.classList.contains("show"), "editor bar must be visible");

    const collapseBtn = env.document.getElementById("vpad-bar-collapse");
    const doneBtn = env.document.getElementById("vpad-bar-done");

    // Tap collapse button: pointerdown, pointerup
    collapseBtn.dispatchEvent({ type: "pointerdown", pointerId: 2, clientX: 50, clientY: 50, cancelable: true });
    collapseBtn.dispatchEvent({ type: "pointerup", pointerId: 2, clientX: 50, clientY: 50, cancelable: true });

    // Toolbar is collapsed
    assert.strictEqual(collapseBtn.textContent, "≡", "toolbar should be collapsed showing ≡");
    assert.strictEqual(doneBtn.style.display, "none", "doneBtn should be hidden when collapsed");

    // Advance beyond collapse transition guard window
    env.advance(200);

    // Tap collapse button again to expand
    collapseBtn.dispatchEvent({ type: "pointerdown", pointerId: 3, clientX: 50, clientY: 50, cancelable: true });
    collapseBtn.dispatchEvent({ type: "pointerup", pointerId: 3, clientX: 50, clientY: 50, cancelable: true });

    // Ensure done button was NOT triggered (editor bar still open in edit mode)
    assert(editorBar.classList.contains("show"), "editor bar must remain open; doneBtn must not be triggered");
    assert.strictEqual(collapseBtn.textContent, "─", "toolbar is expanded");

    // Any synthetic pointerup or click landing on doneBtn immediately during transition is guarded
    doneBtn.dispatchEvent({ type: "pointerup", pointerId: 3, clientX: 50, clientY: 50, cancelable: true });
    doneBtn.dispatchEvent({ type: "click", cancelable: true });

    // Editor bar must STILL be shown (edit mode not exited)
    assert(editorBar.classList.contains("show"), "editor bar must still be open after ghost click on done button");
});

test("Matte theme: active button press scales without color clash or flickering to gray", () => {
    const cssPath = path.resolve(__dirname, "../../main/assets/overlay/vpad.css");
    const css = fs.readFileSync(cssPath, "utf-8");

    // Verify matte active button styles
    assert(css.includes("body.vpad-theme-matte .vpad-top-btn.active"), "matte must have active top btn styling");
    assert(css.includes("background: #1e3a8a !important;"), "matte active top btn must use deep blue #1e3a8a");
    assert(css.includes("border-color: #3b82f6 !important;"), "matte active top btn must use blue border #3b82f6");

    // Verify matte active pressed state keeps theme color and does NOT set background: #27272a
    const matteActiveRegex = /body\.vpad-theme-matte\s+\.vpad-top-btn\.active:active[\s\S]*?\{([\s\S]*?)\}/;
    const match = css.match(matteActiveRegex);
    assert(match, "must have explicit rule for body.vpad-theme-matte .vpad-top-btn.active:active");
    const ruleBody = match[1];
    assert(ruleBody.includes("#1e3a8a"), "active pressed state must keep blue #1e3a8a background");
    assert(ruleBody.includes("#3b82f6"), "active pressed state must keep #3b82f6 border");
    assert(!ruleBody.includes("#27272a"), "active pressed state must NOT have gray #27272a");

    // Verify general matte top-btn:active does NOT set gray background/border
    const matteTopBtnActive = /body\.vpad-theme-matte\s+\.vpad-top-btn:active[\s\S]*?\{([\s\S]*?)\}/;
    const matchTopBtn = css.match(matteTopBtnActive);
    if (matchTopBtn) {
        assert(!matchTopBtn[1].includes("#27272a"), "matte .vpad-top-btn:active must not override background to gray");
    }
});
