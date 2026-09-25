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
        let _innerHTML = "";
        const el = {
            id: id || "",
            tagName: tagName.toUpperCase(),
            className: "",
            classList: {
                classes: new Set(),
                add: function (...cls) { cls.forEach(c => this.classes.add(c)); },
                remove: function (...cls) { cls.forEach(c => this.classes.delete(c)); },
                contains: function (c) { return this.classes.has(c); },
                toggle: function (c) {
                    if (this.classes.has(c)) { this.classes.delete(c); return false; }
                    else { this.classes.add(c); return true; }
                }
            },
            textContent: "",
            get innerHTML() {
                return _innerHTML;
            },
            set innerHTML(val) {
                _innerHTML = String(val);
                const idMatches = _innerHTML.matchAll(/id="([^"]+)"/g);
                for (const match of idMatches) {
                    const childId = match[1];
                    if (!elementsById[childId]) {
                        const childEl = createMockElement(childId);
                        registerEl(childEl);
                    }
                }
            },
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
        head: createMockElement("head"),
        listeners: {},
        addEventListener: function (evt, fn) {
            if (!this.listeners[evt]) this.listeners[evt] = [];
            this.listeners[evt].push(fn);
        },
        querySelectorAll: function () {
            return [];
        }
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
    assert(bugBtn.innerHTML.includes("<svg"), "Bug button must render SVG icon (zero emojis)");

    const modal = sandbox.document.getElementById("rp-diag-modal");
    assert(modal, "Modal #rp-diag-modal must be created");
    assert(modal.innerHTML.includes("<svg"), "Modal must render SVG icons");
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
    let sharedPayload = null;
    sandbox.RuntimePackVPad.shareBugReport = function (jsonStr) {
        sharedPayload = JSON.parse(jsonStr);
    };

    sandbox.DiagnosticSuite.exportBugReport();
    assert.strictEqual(dispatchedCmds.length, 3);
    assert.strictEqual(dispatchedCmds[2].action, "export_bug_report", "exportBugReport should dispatch export_bug_report");
    assert(sharedPayload, "shareBugReport must be called with JSON report payload");
    assert(Array.isArray(sharedPayload.logs), "Report payload must contain logs array");
}

// Test 5: Empty Progress Never Triggers Fake Success
{
    console.log("Test 5: Empty Progress Never Triggers Fake Success");
    const { sandbox } = createDOMContext();

    let intervalCallback = null;
    sandbox.setInterval = function (fn, ms) {
        intervalCallback = fn;
        return 123;
    };
    sandbox.clearInterval = function (id) {
        intervalCallback = null;
    };

    sandbox.RuntimePackVPad = {
        postCheatCmd: function () { return true; },
        getAuditProgress: function () { return ""; } // Always empty
    };

    vm.runInContext(diagCode, sandbox);
    sandbox.DiagnosticSuite.init();
    sandbox.DiagnosticSuite.runStressTest();

    assert(intervalCallback, "Interval callback should have been registered");

    // Advance 8 ticks (where old fake progress used to claim SUCCESS)
    for (let i = 0; i < 8; i++) {
        if (intervalCallback) intervalCallback();
    }

    const badge = sandbox.document.getElementById("diag-step-badge");
    assert(badge, "Badge element must exist");
    assert.notStrictEqual(badge.textContent, "COMPLETED", "Empty progress must NEVER set status to COMPLETED at poll count 8!");

    // Advance to timeout (past 60 ticks)
    for (let i = 8; i < 65; i++) {
        if (intervalCallback) intervalCallback();
    }

    assert.notStrictEqual(badge.textContent, "COMPLETED", "Timed out audit must NEVER set status to COMPLETED!");
    assert(badge.textContent === "TIMEOUT" || badge.textContent === "INDETERMINATE", `Badge should be TIMEOUT or INDETERMINATE on timeout, got: ${badge.textContent}`);
}

// Test 6: Structured JSON Events Parsing & Error Handling (Continues Scanning on Failure)
{
    console.log("Test 6: Structured JSON Events Parsing & Error Handling (Continues Scanning on Failure)");
    const { sandbox } = createDOMContext();

    let intervalCallback = null;
    sandbox.setInterval = function (fn, ms) {
        intervalCallback = fn;
        return 456;
    };
    sandbox.clearInterval = function (id) {
        intervalCallback = null;
    };

    let mockProgress = "";
    sandbox.RuntimePackVPad = {
        postCheatCmd: function () { return true; },
        getAuditProgress: function () { return mockProgress; }
    };

    vm.runInContext(diagCode, sandbox);
    sandbox.DiagnosticSuite.init();
    sandbox.DiagnosticSuite.runStressTest();

    const pbar = sandbox.document.getElementById("diag-pbar-fill");
    const badge = sandbox.document.getElementById("diag-step-badge");

    // Step 1: START event
    mockProgress = JSON.stringify([{ type: "START", message: "Audit started", pct: 15 }]);
    intervalCallback();
    assert.strictEqual(pbar.style.width, "15%");

    // Step 2: Asset FAIL event - should increment issues but NOT abort interval
    mockProgress = JSON.stringify([{ outcome: "FAIL", asset_path: "audio/se/Crash.ogg", error_message: "Corrupted audio chunk" }]);
    intervalCallback();
    assert.strictEqual(badge.textContent, "ISSUES (1)", "Badge must show ISSUES (1)");
    assert.notStrictEqual(intervalCallback, null, "Audit must NOT abort on individual asset failure");

    // Step 3: Subsequent PASS event - should continue scanning
    mockProgress = JSON.stringify([{ stage: "FILE_READABLE", outcome: "PASS", asset_path: "audio/bgm/Theme.ogg" }]);
    intervalCallback();
    assert.notStrictEqual(intervalCallback, null, "Audit must still be scanning");

    // Step 4: DONE event - should complete and mark overall outcome as FAILED due to earlier issue
    mockProgress = JSON.stringify([{ type: "DONE", message: "All maps scanned" }]);
    intervalCallback();
    assert.strictEqual(badge.textContent, "FAILED", "Final badge must be FAILED due to earlier issue");
    assert.strictEqual(intervalCallback, null, "Interval must be cleared on DONE");
}

// Test 7: Structured Line Events (START, STAGE, DONE)
{
    console.log("Test 7: Structured Line Events (START, STAGE, DONE)");
    const { sandbox } = createDOMContext();

    let intervalCallback = null;
    sandbox.setInterval = function (fn, ms) {
        intervalCallback = fn;
        return 789;
    };
    sandbox.clearInterval = function (id) {
        intervalCallback = null;
    };

    let mockProgress = "";
    sandbox.RuntimePackVPad = {
        postCheatCmd: function () { return true; },
        getAuditProgress: function () { return mockProgress; }
    };

    vm.runInContext(diagCode, sandbox);
    sandbox.DiagnosticSuite.init();
    sandbox.DiagnosticSuite.runStressTest();

    const pbar = sandbox.document.getElementById("diag-pbar-fill");
    const badge = sandbox.document.getElementById("diag-step-badge");

    mockProgress = "START: Initiating scan\nSTAGE: Checking maps\n";
    intervalCallback();

    mockProgress = "DONE: Verified all items\n";
    intervalCallback();

    assert.strictEqual(badge.textContent, "COMPLETED", "Badge must be COMPLETED on DONE");
    assert.strictEqual(pbar.style.width, "100%", "Progress bar should reach 100% on DONE");
}

// Test 8: Stream of Passing Events Does NOT Prematurely Stop Audit
{
    console.log("Test 8: Stream of Passing Events Does NOT Prematurely Stop Audit");
    const { sandbox } = createDOMContext();

    let intervalCallback = null;
    sandbox.setInterval = function (fn, ms) {
        intervalCallback = fn;
        return 101;
    };
    sandbox.clearInterval = function (id) {
        intervalCallback = null;
    };

    let mockProgress = "";
    sandbox.RuntimePackVPad = {
        postCheatCmd: function () { return true; },
        getAuditProgress: function () { return mockProgress; }
    };

    vm.runInContext(diagCode, sandbox);
    sandbox.DiagnosticSuite.init();
    sandbox.DiagnosticSuite.runStressTest();

    const badge = sandbox.document.getElementById("diag-step-badge");

    // Feed multiple PASS events
    for (let i = 1; i <= 5; i++) {
        mockProgress = JSON.stringify([{
            stage: "FILE_READABLE",
            outcome: "PASS",
            asset_path: `img/characters/Hero_${i}.png`
        }]);
        intervalCallback();
        assert.notStrictEqual(intervalCallback, null, `Passing event ${i} must not stop audit`);
        assert.notStrictEqual(badge.textContent, "COMPLETED", `Passing event ${i} must not mark COMPLETED`);
    }

    // Now send explicit DONE
    mockProgress = JSON.stringify([{ type: "DONE", message: "Audit complete" }]);
    intervalCallback();
    assert.strictEqual(badge.textContent, "COMPLETED", "Badge must be COMPLETED only on DONE");
    assert.strictEqual(intervalCallback, null, "Audit should stop after DONE");
}

// Test 9: getAuditEvents Bridge Queries with lastEventSeq
{
    console.log("Test 9: getAuditEvents Bridge Queries with lastEventSeq");
    const { sandbox } = createDOMContext();

    let intervalCallback = null;
    sandbox.setInterval = function (fn, ms) {
        intervalCallback = fn;
        return 202;
    };
    sandbox.clearInterval = function (id) {
        intervalCallback = null;
    };

    let requestedSeq = -1;
    sandbox.RuntimePackVPad = {
        postCheatCmd: function () { return true; },
        getAuditEvents: function (afterSeq) {
            requestedSeq = afterSeq;
            if (afterSeq === 0) {
                return JSON.stringify([
                    { sequence_id: 1, stage: "FILE_EXISTS", outcome: "PASS", asset_path: "data/System.json" },
                    { sequence_id: 2, stage: "FILE_READABLE", outcome: "PASS", asset_path: "data/System.json" }
                ]);
            } else if (afterSeq === 2) {
                return JSON.stringify([
                    { sequence_id: 3, type: "DONE", message: "Finished" }
                ]);
            }
            return "[]";
        }
    };

    vm.runInContext(diagCode, sandbox);
    sandbox.DiagnosticSuite.init();
    sandbox.DiagnosticSuite.runStressTest();

    const badge = sandbox.document.getElementById("diag-step-badge");

    // First poll (afterSeq = 0)
    intervalCallback();
    assert.strictEqual(requestedSeq, 0);
    assert.notStrictEqual(badge.textContent, "COMPLETED");

    // Second poll (afterSeq should now be 2)
    intervalCallback();
    assert.strictEqual(requestedSeq, 2);
    assert.strictEqual(badge.textContent, "COMPLETED");
}

console.log("All Diagnostic Suite JS unit tests passed successfully!");
