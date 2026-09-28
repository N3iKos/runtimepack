const assert = require("assert");
const fs = require("fs");
const path = require("path");
const vm = require("vm");

const overlayDir = path.resolve(__dirname, "../../main/assets/overlay");
const themeLoaderCode = fs.readFileSync(path.join(overlayDir, "theme_loader.js"), "utf8");
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

test("Matte theme: active button and editor bar deep blue accent styling", () => {
    const cssPath = path.resolve(__dirname, "../../main/assets/overlay/vpad.css");
    const css = fs.readFileSync(cssPath, "utf-8");

    // Verify matte active button styles
    assert(css.includes("body.vpad-theme-matte .vpad-top-btn.active"), "matte must have active top btn styling");
    assert(css.includes("body.vpad-theme-matte #vpad-main-trigger-btn.active"), "matte must have active trigger btn styling");

    const matteActiveBtnRegex = /body\.vpad-theme-matte\s+\.vpad-top-btn\.active,\s*body\.vpad-theme-matte\s+#vpad-main-trigger-btn\.active\s*\{([\s\S]*?)\}/;
    const activeBtnMatch = css.match(matteActiveBtnRegex);
    assert(activeBtnMatch, "must match body.vpad-theme-matte active top/trigger button rule");
    assert(activeBtnMatch[1].includes("background: #1e3a8a !important;"), "matte active btn must use deep blue #1e3a8a");
    assert(activeBtnMatch[1].includes("border-color: #2563eb !important;"), "matte active btn must use blue border #2563eb");
    assert(activeBtnMatch[1].includes("color: #dbeafe !important;"), "matte active btn must use light blue text #dbeafe");

    // Verify matte active svg stroke
    const matteSvgRegex = /body\.vpad-theme-matte\s+\.vpad-top-btn\.active svg,\s*body\.vpad-theme-matte\s+#vpad-main-trigger-btn\.active svg\s*\{([\s\S]*?)\}/;
    const svgMatch = css.match(matteSvgRegex);
    assert(svgMatch, "must match matte active svg rule");
    assert(svgMatch[1].includes("stroke: #dbeafe !important;"), "matte active svg must use stroke #dbeafe");

    // Verify matte active pressed state
    const matteActivePressedRegex = /body\.vpad-theme-matte\s+\.vpad-top-btn\.active:active[\s\S]*?\{([\s\S]*?)\}/;
    const pressedMatch = css.match(matteActivePressedRegex);
    assert(pressedMatch, "must have explicit rule for body.vpad-theme-matte .vpad-top-btn.active:active");
    const pressedBody = pressedMatch[1];
    assert(pressedBody.includes("background: #172554 !important;"), "active pressed state must use deep navy #172554");
    assert(pressedBody.includes("border-color: #3b82f6 !important;"), "active pressed state must use bright blue border #3b82f6");
    assert(pressedBody.includes("color: #ffffff !important;"), "active pressed state must use white text");
    assert(pressedBody.includes("scale(0.88)"), "active pressed state must scale");

    // Verify matte editor bar
    const editorBarRegex = /body\.vpad-theme-matte\s+#vpad-editor-bar\s*\{([\s\S]*?)\}/;
    const editorBarMatch = css.match(editorBarRegex);
    assert(editorBarMatch, "must have rule for body.vpad-theme-matte #vpad-editor-bar");
    assert(editorBarMatch[1].includes("background: #141416 !important;"), "matte editor bar must use charcoal #141416");
    assert(editorBarMatch[1].includes("border: 1.5px solid #2563eb !important;"), "matte editor bar must use border #2563eb");
    assert(editorBarMatch[1].includes("box-shadow: 0 16px 36px rgba(0, 0, 0, 0.85) !important;"), "matte editor bar must have elevated shadow");

    // Verify matte done button
    const doneBtnRegex = /body\.vpad-theme-matte\s+#vpad-bar-done\s*\{([\s\S]*?)\}/;
    const doneBtnMatch = css.match(doneBtnRegex);
    assert(doneBtnMatch, "must have rule for body.vpad-theme-matte #vpad-bar-done");
    assert(doneBtnMatch[1].includes("background: #1e3a8a !important;"), "matte done btn must use background #1e3a8a");
    assert(doneBtnMatch[1].includes("border-color: #2563eb !important;"), "matte done btn must use border-color #2563eb");
    assert(doneBtnMatch[1].includes("color: #dbeafe !important;"), "matte done btn must use text color #dbeafe");

    // Verify matte done button active state
    const doneBtnActiveRegex = /body\.vpad-theme-matte\s+#vpad-bar-done:active\s*\{([\s\S]*?)\}/;
    const doneBtnActiveMatch = css.match(doneBtnActiveRegex);
    assert(doneBtnActiveMatch, "must have rule for body.vpad-theme-matte #vpad-bar-done:active");
    assert(doneBtnActiveMatch[1].includes("background: #172554 !important;"), "matte done btn active must use background #172554");
    assert(doneBtnActiveMatch[1].includes("border-color: #3b82f6 !important;"), "matte done btn active must use border-color #3b82f6");
});

test("Liquid Glass theme: panels styled with squircle 24px, blur(30px), and specular rim", () => {
    const cssPath = path.resolve(__dirname, "../../main/assets/overlay/vpad.css");
    const css = fs.readFileSync(cssPath, "utf-8");

    // Verify selectors for Liquid Glass panels
    assert(css.includes("body.vpad-theme-clean #vpad-editor-bar"), "must style clean #vpad-editor-bar");
    assert(css.includes("body.vpad-theme-clean #fnh-cheat-overlay"), "must style clean #fnh-cheat-overlay");
    assert(css.includes("body.vpad-theme-clean #rp-diag-modal"), "must style clean #rp-diag-modal");
    assert(css.includes("body.vpad-theme-clean #vpad-prop-panel"), "must style clean #vpad-prop-panel");
    assert(css.includes("body.vpad-theme-clean #vpad-key-modal"), "must style clean #vpad-key-modal");

    // Match the panels rule block
    const panelRuleRegex = /body\.vpad-theme-clean\s+#vpad-editor-bar[\s\S]*?\{([\s\S]*?)\}/;
    const panelMatch = css.match(panelRuleRegex);
    assert(panelMatch, "must match liquid glass panels rule block");
    const panelBody = panelMatch[1];

    assert(panelBody.includes("border-radius: var(--theme-radius-panel, 24px) !important;"), "panels must have 24px squircle border-radius");
    assert(panelBody.includes("backdrop-filter: blur(30px) saturate(180%) !important;"), "panels must have blur(30px) saturate(180%)");
    assert(panelBody.includes("-webkit-backdrop-filter: blur(30px) saturate(180%) !important;"), "panels must have -webkit-backdrop-filter blur(30px)");
    assert(panelBody.includes("linear-gradient(135deg, rgba(255, 255, 255, 0.16) 0%, rgba(15, 23, 42, 0.55) 100%) !important;"), "panels must have frosted translucent gradient");
    assert(panelBody.includes("border: 1px solid rgba(255, 255, 255, 0.28) !important;"), "panels must have 1px specular rim");
    assert(panelBody.includes("box-shadow: 0 20px 48px rgba(0, 0, 0, 0.4), inset 0 1px 1px rgba(255, 255, 255, 0.45) !important;"), "panels must have clean shadow with specular inset");

    // Verify editor done button in Liquid Glass
    const doneBtnRegex = /body\.vpad-theme-clean\s+#vpad-bar-done[\s\S]*?\{([\s\S]*?)\}/;
    const doneBtnMatch = css.match(doneBtnRegex);
    assert(doneBtnMatch, "must match liquid glass #vpad-bar-done rule");
    const doneBtnBody = doneBtnMatch[1];
    assert(doneBtnBody.includes("background: #007aff !important;"), "done button must use Apple system blue #007aff");
    assert(doneBtnBody.includes("border: 1px solid rgba(255, 255, 255, 0.35) !important;"), "done button must use clean border");
    assert(doneBtnBody.includes("border-radius: var(--theme-radius-pill, 9999px) !important;"), "done button must have pill border-radius");
    assert(doneBtnBody.includes("color: #ffffff !important;"), "done button must have white text");
    assert(doneBtnBody.includes("box-shadow: 0 4px 16px rgba(0, 122, 255, 0.35)"), "done button must have subtle shadow");

    // Verify controls in Liquid Glass
    const btnRegex = /body\.vpad-theme-clean\s+\.vpad-btn,\s*body:not\(\[class\*="vpad-theme-"\]\)\s+\.vpad-btn\s*\{([\s\S]*?)\}/;
    const btnMatch = css.match(btnRegex);
    assert(btnMatch, "must match liquid glass .vpad-btn rule");
    const btnBody = btnMatch[1];
    assert(btnBody.includes("border-radius: var(--theme-radius-button, 9999px) !important;"), "buttons must have pill border-radius");
    assert(btnBody.includes("backdrop-filter: blur(30px) saturate(180%) !important;"), "buttons must have blur(30px) saturate(180%)");
    assert(btnBody.includes("border: 1px solid rgba(255, 255, 255, 0.25) !important;"), "buttons must have 1px solid rim");

    const dpadRegex = /body\.vpad-theme-clean\s+\.vpad-dpad-container[\s\S]*?\{([\s\S]*?)\}/;
    const dpadMatch = css.match(dpadRegex);
    assert(dpadMatch, "must match liquid glass dpad rule");
    const dpadBody = dpadMatch[1];
    assert(dpadBody.includes("backdrop-filter: blur(30px) saturate(180%) !important;"), "dpad must have blur(30px) saturate(180%)");
    assert(dpadBody.includes("border: 1px solid rgba(255, 255, 255, 0.25) !important;"), "dpad must have 1px solid rim");

    const joyRegex = /body\.vpad-theme-clean\s+\.vpad-joystick-container[\s\S]*?\{([\s\S]*?)\}/;
    const joyMatch = css.match(joyRegex);
    assert(joyMatch, "must match liquid glass joystick rule");
    const joyBody = joyMatch[1];
    assert(joyBody.includes("backdrop-filter: blur(30px) saturate(180%) !important;"), "joystick must have blur(30px) saturate(180%)");
    assert(joyBody.includes("border: 1px solid rgba(255, 255, 255, 0.25) !important;"), "joystick must have 1px solid rim");
});

test("Matte theme: active dpad and joystick use #2563eb and zero #38bdf8", () => {
    const cssPath = path.resolve(__dirname, "../../main/assets/overlay/vpad.css");
    const css = fs.readFileSync(cssPath, "utf-8");

    // Check dpad arrow active in Matte
    const dpadArrowRegex = /body\.vpad-theme-matte\s+\.vpad-dpad-arrow\.active\s*\{([\s\S]*?)\}/;
    const dpadArrowMatch = css.match(dpadArrowRegex);
    assert(dpadArrowMatch, "must match body.vpad-theme-matte .vpad-dpad-arrow.active");
    const dpadArrowBody = dpadArrowMatch[1];
    assert(dpadArrowBody.includes("border-color: #2563eb !important;"), "matte active dpad arrow must have border-color #2563eb");
    assert(dpadArrowBody.includes("color: #2563eb !important;"), "matte active dpad arrow must have color #2563eb");
    assert(dpadArrowBody.includes("box-shadow: 0 0 12px rgba(37, 99, 235, 0.5) !important;"), "matte active dpad arrow must have box-shadow rgba(37, 99, 235, 0.5)");
    assert(!dpadArrowBody.includes("#38bdf8"), "matte active dpad arrow must have zero #38bdf8");

    // Check joystick stick active in Matte
    const joyStickRegex = /body\.vpad-theme-matte\s+\.vpad-joystick-container\.active\s+\.vpad-joystick-stick\s*\{([\s\S]*?)\}/;
    const joyStickMatch = css.match(joyStickRegex);
    assert(joyStickMatch, "must match body.vpad-theme-matte .vpad-joystick-container.active .vpad-joystick-stick");
    const joyStickBody = joyStickMatch[1];
    assert(joyStickBody.includes("border-color: #2563eb !important;"), "matte active joystick stick must have border-color #2563eb");
    assert(joyStickBody.includes("box-shadow: 0 0 12px rgba(37, 99, 235, 0.5) !important;"), "matte active joystick stick must have box-shadow rgba(37, 99, 235, 0.5)");
    assert(!joyStickBody.includes("#38bdf8"), "matte active joystick stick must have zero #38bdf8");

    // Ensure entire Matte section in vpad.css has zero #38bdf8
    const matteSectionRegex = /\/\* =+\s*THEME 2: MINIMALIST MATTE[\s\S]*?(?=\/\* =+\s*THEME 3: RETRO OSD)/;
    const matteSectionMatch = css.match(matteSectionRegex);
    assert(matteSectionMatch, "must match matte theme section");
    assert(!matteSectionMatch[0].includes("#38bdf8"), "entire matte theme section must contain zero #38bdf8");
});

test("dynamic panel stacking: opening panel B over panel A, and tapping panel A brings it to front", () => {
    const env = createEnvironment({ readyState: "complete" });
    env.run(vpadCode, "vpad.js");
    env.document.dispatchEvent({ type: "DOMContentLoaded" });
    const CheatMenu = env.run(cheatCode, "cheat_menu.js");
    const DiagnosticSuite = env.run(diagnosticCode, "diagnostic_suite.js");
    env.runTimers();
    CheatMenu.initDOM();
    DiagnosticSuite.init();

    const cheatOverlay = env.document.getElementById("fnh-cheat-overlay");
    const diagModal = env.document.getElementById("rp-diag-modal");

    // 1. Open Panel A (Cheat Menu)
    CheatMenu.open();
    assert(cheatOverlay.style.display !== "none", "Cheat overlay must be open");
    const cheatZ1 = Number(cheatOverlay.style.zIndex || 0);
    assert(cheatZ1 >= 100010, "Cheat overlay must have dynamic elevated z-index");

    // 2. Open Panel B (Diagnostic Suite)
    DiagnosticSuite.open();
    assert(diagModal.style.display !== "none", "Diagnostic modal must be open");
    const diagZ1 = Number(diagModal.style.zIndex || 0);
    assert(diagZ1 > cheatZ1, "Panel B (Diagnostic Suite) must stack above Panel A (Cheat Menu)");

    // 3. Tap Panel A (pointerdown on cheatOverlay) brings Panel A to front
    cheatOverlay.dispatchEvent({ type: "pointerdown", cancelable: true });
    const cheatZ2 = Number(cheatOverlay.style.zIndex || 0);
    assert(cheatZ2 > diagZ1, "Tapping Panel A (pointerdown) must bring it above Panel B");

    // 4. Tap Panel B (mousedown on diagModal) brings Panel B to front
    diagModal.dispatchEvent({ type: "mousedown", cancelable: true });
    const diagZ2 = Number(diagModal.style.zIndex || 0);
    assert(diagZ2 > cheatZ2, "Tapping Panel B (mousedown) must bring it back above Panel A");

    // 5. Test window.bringPanelToFront by ID string without '#'
    env.window.bringPanelToFront("fnh-cheat-overlay");
    const cheatZ3 = Number(cheatOverlay.style.zIndex || 0);
    assert(cheatZ3 > diagZ2, "bringPanelToFront(id) without '#' must bring Cheat overlay to front");

    // 6. Test window.bringPanelToFront by selector string with '#'
    env.window.bringPanelToFront("#rp-diag-modal");
    const diagZ3 = Number(diagModal.style.zIndex || 0);
    assert(diagZ3 > cheatZ3, "bringPanelToFront('#id') with '#' must bring Diagnostic modal to front");

    // 7. Test window.elevatePanel alias with DOM element
    env.window.elevatePanel(cheatOverlay);
    const cheatZ4 = Number(cheatOverlay.style.zIndex || 0);
    assert(cheatZ4 > diagZ3, "window.elevatePanel(element) must function identically to bringPanelToFront");
});

test("editor bar, property panel, and key modal dynamic stacking and toolbar interaction", () => {
    const env = createEnvironment({ readyState: "complete" });
    env.run(vpadCode, "vpad.js");
    env.document.dispatchEvent({ type: "DOMContentLoaded" });

    const gearBtn = env.document.getElementById("vpad-btn-gear");
    const editorBar = env.document.getElementById("vpad-editor-bar");
    const propPanel = env.document.getElementById("vpad-prop-panel");
    const keyModal = env.document.getElementById("vpad-key-modal");

    // 1. Enter edit mode via gear button -> elevates editor bar
    gearBtn.dispatchEvent({ type: "click", cancelable: true });
    assert(editorBar.classList.contains("show"), "Editor bar must show");
    const editorZ1 = Number(editorBar.style.zIndex || 0);
    assert(editorZ1 >= 100010, "Editor bar must be elevated on open");

    // 2. Select a control to show property panel -> elevates property panel
    const ctrlEl = env.document.querySelector(".vpad-control-item");
    assert(ctrlEl, "Must have control item");
    ctrlEl.dispatchEvent({ type: "mousedown", clientX: 100, clientY: 100, cancelable: true });
    assert(propPanel.classList.contains("show"), "Property panel must show");
    const propZ1 = Number(propPanel.style.zIndex || 0);
    assert(propZ1 > editorZ1, "Property panel must stack above editor bar");

    // 3. Tap editor bar -> elevates editor bar above property panel
    editorBar.dispatchEvent({ type: "pointerdown", cancelable: true });
    const editorZ2 = Number(editorBar.style.zIndex || 0);
    assert(editorZ2 > propZ1, "Tapping editor bar must elevate it above property panel");

    // 4. Tap property panel -> elevates property panel above editor bar
    propPanel.dispatchEvent({ type: "pointerdown", cancelable: true });
    const propZ2 = Number(propPanel.style.zIndex || 0);
    assert(propZ2 > editorZ2, "Tapping property panel must elevate it above editor bar");

    // 5. Open key modal (via Add Custom Control button) -> elevates key modal
    const addBtn = env.document.getElementById("vpad-bar-add");
    addBtn.dispatchEvent({ type: "click", cancelable: true });
    assert(keyModal.classList.contains("show"), "Key modal must show");
    const keyZ1 = Number(keyModal.style.zIndex || 0);
    assert(keyZ1 > propZ2, "Key modal must stack above property panel");

    // 6. Tap editor bar -> elevates editor bar above key modal
    editorBar.dispatchEvent({ type: "pointerdown", cancelable: true });
    const editorZ3 = Number(editorBar.style.zIndex || 0);
    assert(editorZ3 > keyZ1, "Tapping editor bar must elevate it above key modal");

    // 7. Tap key modal -> elevates key modal back to top
    keyModal.dispatchEvent({ type: "pointerdown", cancelable: true });
    const keyZ2 = Number(keyModal.style.zIndex || 0);
    assert(keyZ2 > editorZ3, "Tapping key modal must elevate it back to top");
});

test("popovers stack at least topPanelZIndex + 10 above the editor bar", () => {
    const env = createEnvironment({ readyState: "complete" });
    env.run(vpadCode, "vpad.js");
    env.document.dispatchEvent({ type: "DOMContentLoaded" });

    const gearBtn = env.document.getElementById("vpad-btn-gear");
    const editorBar = env.document.getElementById("vpad-editor-bar");
    gearBtn.dispatchEvent({ type: "click", cancelable: true });
    editorBar.dispatchEvent({ type: "pointerdown", cancelable: true });
    const editorZ = Number(editorBar.style.zIndex || 0);

    const presetBtn = env.document.getElementById("vpad-bar-preset-btn");
    const presetPopover = env.document.getElementById("vpad-preset-popover");
    env.advance(300);
    presetBtn.dispatchEvent({ type: "click", cancelable: true });
    assert(presetPopover.classList.contains("show"), "Preset popover must show");
    const presetZ = Number(presetPopover.style.zIndex || 0);
    assert(presetZ >= editorZ + 10, "Preset popover must stack at least editorBar.zIndex + 10");

    const themeBtn = env.document.getElementById("vpad-bar-theme-btn");
    const themePopover = env.document.getElementById("vpad-theme-popover");
    env.advance(300);
    themeBtn.dispatchEvent({ type: "click", cancelable: true });
    assert(themePopover.classList.contains("show"), "Theme popover must show");
    const themeZ = Number(themePopover.style.zIndex || 0);
    assert(themeZ >= editorZ + 10, "Theme popover must stack at least editorBar.zIndex + 10");
});

test("Theme-Neutral Core Architecture: CSS variables, neutralized components, zero hardcoded hex", () => {
    const cssPath = path.resolve(__dirname, "../../main/assets/overlay/vpad.css");
    const css = fs.readFileSync(cssPath, "utf-8");

    // 1. Verify CSS variables in :root
    assert(css.includes("--theme-accent:"), "vpad.css must declare --theme-accent");
    assert(css.includes("--theme-accent-contrast:"), "vpad.css must declare --theme-accent-contrast");
    assert(css.includes("--theme-bg-panel:"), "vpad.css must declare --theme-bg-panel");
    assert(css.includes("--theme-border-panel:"), "vpad.css must declare --theme-border-panel");
    assert(css.includes("--theme-radius-panel:"), "vpad.css must declare --theme-radius-panel");

    // 2. Verify structural rules referencing CSS variables in vpad.css
    assert(css.includes("var(--theme-accent"), "vpad.css must use var(--theme-accent)");
    assert(css.includes("var(--theme-bg-panel"), "vpad.css must use var(--theme-bg-panel)");
    assert(css.includes("var(--theme-border-panel"), "vpad.css must use var(--theme-border-panel)");
    assert(css.includes("var(--theme-radius-panel"), "vpad.css must use var(--theme-radius-panel)");

    // 3. Verify semantic classes in vpad.css
    assert(css.includes(".diag-badge-scanning"), "vpad.css must define .diag-badge-scanning");
    assert(css.includes(".diag-badge-ready"), "vpad.css must define .diag-badge-ready");
    assert(css.includes(".diag-badge-success"), "vpad.css must define .diag-badge-success");
    assert(css.includes(".diag-badge-error"), "vpad.css must define .diag-badge-error");
    assert(css.includes(".diag-fps-high"), "vpad.css must define .diag-fps-high");
    assert(css.includes(".diag-fps-mid"), "vpad.css must define .diag-fps-mid");
    assert(css.includes(".diag-fps-low"), "vpad.css must define .diag-fps-low");
    assert(css.includes(".vpad-text-accent"), "vpad.css must define .vpad-text-accent");
    assert(css.includes("#btn-cheat-mode-toggle"), "vpad.css must style #btn-cheat-mode-toggle");

    // 4. Verify zero hardcoded presentation hex colors in JS files
    const hexPattern = /#[0-9a-fA-F]{3,8}\b/g;

    const diagCodeClean = diagnosticCode.replace(/&#[0-9]{3,4};/g, "");
    const diagHex = diagCodeClean.match(hexPattern);
    assert.strictEqual(diagHex, null, `diagnostic_suite.js must have 0 hex colors, found: ${diagHex}`);

    const cheatHex = cheatCode.match(hexPattern);
    assert.strictEqual(cheatHex, null, `cheat_menu.js must have 0 hex colors, found: ${cheatHex}`);

    const vpadHex = vpadCode.match(hexPattern);
    assert.strictEqual(vpadHex, null, `vpad.js must have 0 hex colors, found: ${vpadHex}`);

    const themeLoaderHex = themeLoaderCode.match(hexPattern);
    assert.strictEqual(themeLoaderHex, null, `theme_loader.js must have 0 hex colors, found: ${themeLoaderHex}`);

    // 5. Verify DOM initializes without injecting style tags
    const env = createEnvironment({ readyState: "complete" });
    env.run(themeLoaderCode, "theme_loader.js");
    env.run(vpadCode, "vpad.js");
    env.run(cheatCode, "cheat_menu.js");
    env.run(diagnosticCode, "diagnostic_suite.js");
    env.document.dispatchEvent({ type: "DOMContentLoaded" });

    env.window.CheatMenu.initDOM();
    env.window.DiagnosticSuite.init();

    assert.strictEqual(env.document.getElementById("fnh-cheat-styles"), null, "must not inject #fnh-cheat-styles");
    assert.strictEqual(env.document.getElementById("rp-diag-styles"), null, "must not inject #rp-diag-styles");
    assert(env.document.getElementById("fnh-cheat-overlay"), "Cheat overlay DOM must exist");
    assert(env.document.getElementById("rp-diag-modal"), "Diagnostic modal DOM must exist");
});

test("RuntimePackTheme: dynamic theme list query and active theme retrieval", () => {
    const env = createEnvironment({ readyState: "complete" });
    env.run(themeLoaderCode, "theme_loader.js");
    assert(env.window.RuntimePackTheme, "window.RuntimePackTheme must exist");

    const themes = env.window.RuntimePackTheme.getAvailableThemes();
    assert(Array.isArray(themes), "getAvailableThemes must return array");
    assert(themes.length >= 3, "must have at least 3 packaged themes");

    const expectedIds = ["matte", "liquid_glass", "retro_osd"];
    expectedIds.forEach(id => {
        const found = themes.find(t => t.id === id);
        assert(found, `must include theme '${id}'`);
        assert(found.name, `theme '${id}' must have name`);
        assert(found.version, `theme '${id}' must have version`);
    });

    assert.strictEqual(env.window.RuntimePackTheme.getCurrentTheme(), "matte", "default theme must be matte");
});

test("RuntimePackTheme: applyTheme unmounts previous theme sheets before mounting new ones and updates attributes", () => {
    const env = createEnvironment({ readyState: "complete" });
    env.run(themeLoaderCode, "theme_loader.js");

    let themeChangeDispatched = false;
    let dispatchedThemeId = null;
    env.window.addEventListener("vpad:themechange", (e) => {
        themeChangeDispatched = true;
        dispatchedThemeId = e.detail && e.detail.themeId;
    });

    const res = env.window.RuntimePackTheme.applyTheme("liquid_glass");
    assert.strictEqual(res, true, "applyTheme must return true for valid theme");
    assert.strictEqual(env.window.RuntimePackTheme.getCurrentTheme(), "liquid_glass");
    assert.strictEqual(env.document.body.getAttribute("data-theme"), "liquid_glass");
    assert(env.document.body.classList.contains("vpad-theme-liquid_glass"), "body must have vpad-theme-liquid_glass");
    assert(env.document.body.classList.contains("vpad-theme-clean"), "body must have vpad-theme-clean for backwards compatibility");
    assert.strictEqual(themeChangeDispatched, true, "vpad:themechange event must be dispatched");
    assert.strictEqual(dispatchedThemeId, "liquid_glass");
    assert.strictEqual(env.window.localStorage.getItem(env.window.RuntimePackTheme.STORAGE_KEY), "liquid_glass");

    const links1 = env.document.head.querySelectorAll("link[data-theme-sheet]");
    assert.strictEqual(links1.length, 4, "must mount exactly 4 theme stylesheets");
    const hrefs1 = links1.map(l => l.href);
    assert(hrefs1.some(h => h.endsWith("/tokens.css")), "must mount tokens.css");
    assert(hrefs1.some(h => h.endsWith("/components.css")), "must mount components.css");
    assert(hrefs1.some(h => h.endsWith("/layout.css")), "must mount layout.css");
    assert(hrefs1.some(h => h.endsWith("/effects.css")), "must mount effects.css");

    // Switch to Retro OSD
    const res2 = env.window.RuntimePackTheme.applyTheme("retro_osd");
    assert.strictEqual(res2, true);
    assert.strictEqual(env.window.RuntimePackTheme.getCurrentTheme(), "retro_osd");
    assert.strictEqual(env.document.body.getAttribute("data-theme"), "retro_osd");
    assert(env.document.body.classList.contains("vpad-theme-retro_osd"));
    assert(env.document.body.classList.contains("vpad-theme-retro"), "body must have vpad-theme-retro for backwards compatibility");
    assert.strictEqual(env.document.body.classList.contains("vpad-theme-liquid_glass"), false, "old theme class must be removed");
    assert.strictEqual(env.window.localStorage.getItem(env.window.RuntimePackTheme.STORAGE_KEY), "retro_osd");

    const links2 = env.document.head.querySelectorAll("link[data-theme-sheet]");
    assert.strictEqual(links2.length, 4, "previous sheets must be unmounted, keeping exactly 4 sheets");
    links2.forEach(l => {
        assert.strictEqual(l.getAttribute("data-theme-sheet"), "retro_osd", "links must have retro_osd data-theme-sheet");
    });
});

test("Theme Popover in vpad.js dynamically populates from RuntimePackTheme and updates bar text", () => {
    const env = createEnvironment({ readyState: "complete" });
    env.run(themeLoaderCode, "theme_loader.js");
    env.run(vpadCode, "vpad.js");
    env.document.dispatchEvent({ type: "DOMContentLoaded" });

    const themeNameEl = env.document.getElementById("vpad-bar-theme-name");
    assert(themeNameEl, "vpad-bar-theme-name element must exist");
    assert(themeNameEl.textContent.includes("Minimalist Matte"), "initial theme text must reflect default theme");

    const themeBtn = env.document.getElementById("vpad-bar-theme-btn");
    const themePopover = env.document.getElementById("vpad-theme-popover");
    env.advance(300);
    themeBtn.dispatchEvent({ type: "click", cancelable: true });
    assert(themePopover.classList.contains("show"), "Theme popover must show");

    const items = themePopover.querySelectorAll(".vpad-popover-item");
    assert(items.length >= 3, "Popover must have items for all packaged themes");
    const retroItem = items.find(it => it.getAttribute("data-id") === "retro_osd" || it.getAttribute("data-id") === "retro");
    assert(retroItem, "Retro OSD item must exist in popover");

    retroItem.dispatchEvent({ type: "click", cancelable: true });
    assert.strictEqual(env.window.RuntimePackTheme.getCurrentTheme(), "retro_osd");
    assert(themeNameEl.textContent.includes("Retro OSD"), "Theme button text must update to 'Retro OSD'");
});

test("Panel stacking hierarchy: opening Diagnostic Suite over Cheat Menu elevates Diagnostic Suite above Cheat Menu", () => {
    const env = createEnvironment({ readyState: "complete" });
    env.run(themeLoaderCode, "theme_loader.js");
    env.run(vpadCode, "vpad.js");
    const CheatMenu = env.run(cheatCode, "cheat_menu.js");
    const DiagnosticSuite = env.run(diagnosticCode, "diagnostic_suite.js");
    env.document.dispatchEvent({ type: "DOMContentLoaded" });
    env.runTimers();
    CheatMenu.initDOM();
    DiagnosticSuite.init();

    const cheatBtn = env.document.getElementById("vpad-btn-cheat");
    const diagBtn = env.document.getElementById("vpad-btn-diag");
    assert(cheatBtn, "#vpad-btn-cheat must exist");
    assert(diagBtn, "#vpad-btn-diag must exist");

    const cheatOverlay = env.document.getElementById("fnh-cheat-overlay");
    const diagModal = env.document.getElementById("rp-diag-modal");
    assert(cheatOverlay, "#fnh-cheat-overlay must exist");
    assert(diagModal, "#rp-diag-modal must exist");

    // 1. Open Cheat Menu
    cheatBtn.dispatchEvent({ type: "click", cancelable: true });
    assert.notStrictEqual(cheatOverlay.style.display, "none", "Cheat overlay must be open");
    const cheatZ1 = Number(cheatOverlay.style.zIndex || 0);
    assert(cheatZ1 >= 100010, "Cheat overlay must have base z-index");

    // 2. Open Diagnostic Suite while Cheat Menu is open
    env.advance(300);
    diagBtn.dispatchEvent({ type: "click", cancelable: true });
    assert.notStrictEqual(diagModal.style.display, "none", "Diagnostic modal must be open");
    const diagZ1 = Number(diagModal.style.zIndex || 0);
    assert(diagZ1 > cheatZ1, "Opening Diagnostic Suite over Cheat Menu must elevate Diagnostic Suite above Cheat Menu");

    // 3. Tap Cheat Menu -> brings Cheat Menu above Diagnostic Suite
    cheatOverlay.dispatchEvent({ type: "pointerdown", cancelable: true });
    const cheatZ2 = Number(cheatOverlay.style.zIndex || 0);
    assert(cheatZ2 > diagZ1, "Tapping Cheat Menu must bring Cheat Menu above Diagnostic Suite");

    // 4. Tap Diagnostic Suite -> brings Diagnostic Suite back above Cheat Menu
    diagModal.dispatchEvent({ type: "pointerdown", cancelable: true });
    const diagZ2 = Number(diagModal.style.zIndex || 0);
    assert(diagZ2 > cheatZ2, "Tapping Diagnostic Suite must bring Diagnostic Suite back above Cheat Menu");
});

test("Dynamic panel stacking: single clean event capture without triple-increment on pointerdown", () => {
    const env = createEnvironment({ readyState: "complete" });
    env.run(themeLoaderCode, "theme_loader.js");
    env.run(vpadCode, "vpad.js");
    const DiagnosticSuite = env.run(diagnosticCode, "diagnostic_suite.js");
    env.document.dispatchEvent({ type: "DOMContentLoaded" });
    env.runTimers();
    DiagnosticSuite.init();

    const diagBtn = env.document.getElementById("vpad-btn-diag");
    diagBtn.dispatchEvent({ type: "click", cancelable: true });
    const diagModal = env.document.getElementById("rp-diag-modal");
    const diagHeader = env.document.getElementById("rp-diag-header");
    assert(diagModal && diagHeader, "Diagnostic modal and header must exist");

    const zBefore = Number(diagModal.style.zIndex || 0);

    // Advance time past initial open so header tap is a distinct user gesture
    env.advance(200);

    // Simulate physical tap on header: pointerdown fires on header, bubbles to document
    diagHeader.dispatchEvent({ type: "pointerdown", pointerId: 1, cancelable: true });
    const zAfterPointer = Number(diagModal.style.zIndex || 0);
    assert.strictEqual(zAfterPointer, zBefore + 2, "Header pointerdown must increment z-index by exactly 2 (single clean elevation)");

    // Browser delivers subsequent touchstart / mousedown for the same gesture (< 100ms)
    diagHeader.dispatchEvent({ type: "touchstart", cancelable: true });
    diagHeader.dispatchEvent({ type: "mousedown", cancelable: true });
    const zAfterGesture = Number(diagModal.style.zIndex || 0);
    assert.strictEqual(zAfterGesture, zAfterPointer, "Subsequent touch/mouse events from same gesture must not triple-increment z-index");
});

test("Compact Mobile Landscape Modal Sizing: #rp-diag-modal and #fnh-cheat-overlay compact boundaries", () => {
    const cssPath = path.resolve(__dirname, "../../main/assets/overlay/vpad.css");
    const vpadCss = fs.readFileSync(cssPath, "utf-8");

    // 1. Assert #rp-diag-modal and #fnh-cheat-overlay dimensions in vpad.css
    assert(vpadCss.includes("max-height: min(78vh, 320px)"), "vpad.css must declare max-height: min(78vh, 320px)");
    assert(vpadCss.includes("width: min(84vw, 460px)"), "vpad.css must declare width: min(84vw, 460px)");

    // Verify #rp-diag-modal rule
    const diagMatch = vpadCss.match(/#rp-diag-modal\s*\{[^}]+\}/);
    assert(diagMatch, "vpad.css must define #rp-diag-modal");
    assert(diagMatch[0].includes("max-height: min(78vh, 320px)"), "#rp-diag-modal must have max-height: min(78vh, 320px)");
    assert(diagMatch[0].includes("width: min(84vw, 460px)"), "#rp-diag-modal must have width: min(84vw, 460px)");

    // Verify #fnh-cheat-overlay rule
    const cheatMatch = vpadCss.match(/#fnh-cheat-overlay\s*\{[^}]+\}/);
    assert(cheatMatch, "vpad.css must define #fnh-cheat-overlay");
    assert(cheatMatch[0].includes("max-height: min(78vh, 320px)"), "#fnh-cheat-overlay must have max-height: min(78vh, 320px)");
    assert(cheatMatch[0].includes("width: min(84vw, 460px)"), "#fnh-cheat-overlay must have width: min(84vw, 460px)");

    // Verify key modal, prop panel, console log, and cheat content compact boundaries
    assert(vpadCss.includes("min(84vw, 440px)"), "vpad.css must limit key modal box width to min(84vw, 440px)");
    assert(vpadCss.includes("min(80vw, 290px)"), "vpad.css must limit prop panel width to min(80vw, 290px)");
    assert(vpadCss.includes("max-height: min(78vh, 310px)"), "vpad.css must limit prop panel max-height to min(78vh, 310px)");
    assert(vpadCss.includes("max-height: 90px"), "vpad.css must adjust console log box max-height to 90px");
    assert(vpadCss.includes("max-height: 180px"), "vpad.css must adjust cheat content max-height to 180px");
    assert(vpadCss.includes("max-height: 140px"), "vpad.css must adjust key modal scroll area max-height to 140px");

    // 2. Verify synchronization in theme packages
    const themes = ["matte", "liquid_glass", "retro_osd"];
    themes.forEach(theme => {
        const themeCssPath = path.resolve(__dirname, "../../../../../../runtimes/themes", theme, "components.css");
        const themeCss = fs.readFileSync(themeCssPath, "utf-8");
        assert(themeCss.includes("max-height: min(78vh, 320px)"), `${theme} components.css must declare max-height: min(78vh, 320px)`);
        assert(themeCss.includes("width: min(84vw, 460px)"), `${theme} components.css must declare width: min(84vw, 460px)`);
        assert(themeCss.includes("width: min(80vw, 290px)"), `${theme} components.css must limit prop panel width`);
        assert(themeCss.includes("max-height: 180px"), `${theme} components.css must adjust cheat content max-height`);
    });
});
