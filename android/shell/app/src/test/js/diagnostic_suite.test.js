// ============================================================================
// Unit Tests for diagnostic_suite.js
// ============================================================================

const assert = require("assert");
const fs = require("fs");
const path = require("path");
const vm = require("vm");

console.log("Running Diagnostic Suite JS Unit Tests...");

const diagPath = path.resolve(__dirname, "../../main/assets/overlay/diagnostic_suite.js");
assert(fs.existsSync(diagPath), `diagnostic_suite.js must exist at ${diagPath}`);
const diagCode = fs.readFileSync(diagPath, "utf-8");

function createDOMContext() {
    const elementsById = {};

    function registerEl(el) {
        if (el && el.id) elementsById[el.id] = el;
        return el;
    }

    function createMockElement(id, tagName = "div") {
        const el = {
            id: id || "",
            tagName: tagName.toUpperCase(),
            className: "",
            textContent: "",
            innerHTML: "",
            style: {},
            children: [],
            attributes: {},
            value: "",
            listeners: {},
            addEventListener: function (evt, fn) {
                if (!this.listeners[evt]) this.listeners[evt] = [];
                this.listeners[evt].push(fn);
            },
            dispatchEvent: function (evt, e = {}) {
                e.stopPropagation = e.stopPropagation || function () {};
                e.preventDefault = e.preventDefault || function () {};
                if (this.listeners[evt]) {
                    this.listeners[evt].forEach(fn => fn(e));
                }
            },
            appendChild: function (child) {
                this.children.push(child);
                registerEl(child);
                return child;
            }
        };
        registerEl(el);
        return el;
    }

    const doc = {
        readyState: "complete",
        createElement: function (tagName) {
            const el = createMockElement("", tagName);
            return el;
        },
        getElementById: function (id) {
            return elementsById[id] || null;
        },
        body: createMockElement("body"),
        head: createMockElement("head")
    };
    elementsById["body"] = doc.body;
    elementsById["head"] = doc.head;

    const originalAppend = doc.body.appendChild.bind(doc.body);
    doc.body.appendChild = function (child) {
        registerEl(child);
        return originalAppend(child);
    };

    const sandbox = {
        console: {
            log: console.log,
            error: function () {},
            warn: function () {},
            info: console.info
        },
        document: doc,
        performance: {
            now: function () { return Date.now(); },
            memory: { usedJSHeapSize: 50 * 1024 * 1024, totalJSHeapSize: 100 * 1024 * 1024 }
        },
        requestAnimationFrame: function (cb) { return setTimeout(cb, 16); },
        cancelAnimationFrame: function (id) { clearTimeout(id); },
        setInterval: setInterval,
        clearInterval: clearInterval,
        setTimeout: setTimeout,
        clearTimeout: clearTimeout,
        Date: Date
    };
    sandbox.window = sandbox;
    sandbox.global = sandbox;
    sandbox.globalThis = sandbox;

    vm.createContext(sandbox);
    return { sandbox, elementsById, registerEl };
}

// Test 1: Initialization and DOM Injection
{
    console.log("Test 1: DOM Elements Injection");
    const { sandbox, elementsById, registerEl } = createDOMContext();

    // Setup #vpad-topbar
    const topBar = sandbox.document.createElement("div");
    topBar.id = "vpad-topbar";
    sandbox.document.body.appendChild(topBar);

    vm.runInContext(diagCode, sandbox);

    assert(sandbox.DiagnosticSuite, "DiagnosticSuite must be defined on window");
    assert(typeof sandbox.DiagnosticSuite.init === "function", "DiagnosticSuite.init must be a function");

    sandbox.DiagnosticSuite.init();

    const bugBtn = sandbox.document.getElementById("vpad-btn-diag");
    assert(bugBtn, "Bug icon button #vpad-btn-diag must be injected into DOM");
    assert(bugBtn.textContent.includes("🐞"), "Bug button text must include 🐞 emoji");

    const modal = sandbox.document.getElementById("rp-diag-modal");
    assert(modal, "Modal #rp-diag-modal must be created");
}

// Test 2: Modal Toggle and VPad Bridge
{
    console.log("Test 2: Modal Toggle & VPad Overlay Hook");
    const { sandbox } = createDOMContext();

    let modalOpenCalls = [];
    sandbox.RuntimePackVPad = {
        setOverlayModalOpen: function (isOpen) {
            modalOpenCalls.push(isOpen);
        }
    };

    vm.runInContext(diagCode, sandbox);
    sandbox.DiagnosticSuite.init();

    const modal = sandbox.document.getElementById("rp-diag-modal");

    // Open modal
    sandbox.DiagnosticSuite.open();
    assert.strictEqual(modal.style.display, "flex", "Modal style.display should be 'flex' when open");
    assert.strictEqual(sandbox.DiagnosticSuite.isOpen(), true, "DiagnosticSuite.isOpen() should return true");
    assert.strictEqual(modalOpenCalls[modalOpenCalls.length - 1], true, "setOverlayModalOpen should be called with true");

    // Close modal
    sandbox.DiagnosticSuite.close();
    assert.strictEqual(modal.style.display, "none", "Modal style.display should be 'none' when closed");
    assert.strictEqual(sandbox.DiagnosticSuite.isOpen(), false, "DiagnosticSuite.isOpen() should return false");
    assert.strictEqual(modalOpenCalls[modalOpenCalls.length - 1], false, "setOverlayModalOpen should be called with false");

    // Toggle
    sandbox.DiagnosticSuite.toggle();
    assert.strictEqual(sandbox.DiagnosticSuite.isOpen(), true, "toggle() from closed should open modal");
    sandbox.DiagnosticSuite.toggle();
    assert.strictEqual(sandbox.DiagnosticSuite.isOpen(), false, "toggle() from open should close modal");
}

// Test 3: Live Error Logging
{
    console.log("Test 3: Live Error Logging");
    const { sandbox } = createDOMContext();

    vm.runInContext(diagCode, sandbox);
    sandbox.DiagnosticSuite.init();

    sandbox.DiagnosticSuite.log("Warning: test warning", "WARN");
    sandbox.DiagnosticSuite.log("Fatal: asset missing", "ERROR");

    const logs = sandbox.DiagnosticSuite.getLogs();
    assert.strictEqual(logs.length, 2, "There should be 2 log entries");
    assert.strictEqual(logs[0].level, "WARN");
    assert(logs[0].message.includes("test warning"));
    assert.strictEqual(logs[1].level, "ERROR");
    assert(logs[1].message.includes("asset missing"));

    sandbox.DiagnosticSuite.clearLogs();
    assert.strictEqual(sandbox.DiagnosticSuite.getLogs().length, 0, "clearLogs() must empty the log buffer");
}

// Test 4: Action Dispatches
{
    console.log("Test 4: Action Dispatches (stress test, teleport, bug report)");
    const { sandbox } = createDOMContext();

    const dispatchedCmds = [];
    sandbox.RuntimePackVPad = {
        postCheatCmd: function (cmdStr) {
            dispatchedCmds.push(JSON.parse(cmdStr));
            return true;
        }
    };

    vm.runInContext(diagCode, sandbox);
    sandbox.DiagnosticSuite.init();

    // 1. Run Stress Test
    sandbox.DiagnosticSuite.runStressTest();
    assert.strictEqual(dispatchedCmds.length, 1);
    assert.strictEqual(dispatchedCmds[0].action, "run_stress_test", "runStressTest should dispatch run_stress_test");

    // 2. Teleport Map
    sandbox.DiagnosticSuite.teleportMap(12, 5, 8);
    assert.strictEqual(dispatchedCmds.length, 2);
    assert.strictEqual(dispatchedCmds[1].action, "teleport_map", "teleportMap should dispatch teleport_map");
    assert.strictEqual(dispatchedCmds[1].map_id, 12);
    assert.strictEqual(dispatchedCmds[1].x, 5);
    assert.strictEqual(dispatchedCmds[1].y, 8);

    // 3. Export Bug Report
    sandbox.DiagnosticSuite.exportBugReport();
    assert.strictEqual(dispatchedCmds.length, 3);
    assert.strictEqual(dispatchedCmds[2].action, "export_bug_report", "exportBugReport should dispatch export_bug_report");
}

console.log("All Diagnostic Suite JS unit tests passed successfully!");
