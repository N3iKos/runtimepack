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
                const tagRegex = /<([a-zA-Z0-9-]+)([^>]*)>/g;
                let match;
                while ((match = tagRegex.exec(_innerHTML)) !== null) {
                    const attrStr = match[2];
                    const idMatch = attrStr.match(/id="([^"]+)"/);
                    if (idMatch) {
                        const childId = idMatch[1];
                        let childEl = elementsById[childId];
                        if (!childEl) {
                            childEl = createMockElement(childId, match[1]);
                            registerEl(childEl);
                        }
                        const classMatch = attrStr.match(/class="([^"]+)"/);
                        if (classMatch) {
                            classMatch[1].split(/\s+/).filter(Boolean).forEach(c => childEl.classList.add(c));
                        }
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

    // Advance to 65 ticks (sampled would timeout, but stress test allows up to 260 polls)
    for (let i = 8; i < 65; i++) {
        if (intervalCallback) intervalCallback();
    }
    assert.strictEqual(badge.textContent, "SCANNING", "Exhaustive/Stress test must not prematurely timeout at 65 ticks (260 polls allowed)");

    // Advance to timeout (past 260 ticks)
    for (let i = 65; i < 265; i++) {
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

// Test 10: Tab "Asset Audit & Probe" and Probe Buttons Exist
{
    console.log("Test 10: Tab Asset Audit & Probe and Probe Buttons Exist");
    const { sandbox } = createDOMContext();
    vm.runInContext(diagCode, sandbox);
    sandbox.DiagnosticSuite.init();

    // Verify Tab is named "Asset Audit & Probe"
    const modal = sandbox.document.getElementById("rp-diag-modal");
    assert(modal.innerHTML.includes("Asset Audit & Probe"), "Tab text must be Asset Audit & Probe");

    // Verify probe buttons exist in DOM and modal template
    const btnSampled = sandbox.document.getElementById("diag-btn-probe-sampled");
    assert(btnSampled, "Button diag-btn-probe-sampled must exist");
    assert(modal.innerHTML.includes("Run Sampled Probe"), "Modal must contain Run Sampled Probe button");

    const btnExhaustive = sandbox.document.getElementById("diag-btn-probe-exhaustive");
    assert(btnExhaustive, "Button diag-btn-probe-exhaustive must exist");
    assert(modal.innerHTML.includes("Run Exhaustive Probe"), "Modal must contain Run Exhaustive Probe button");

    // Verify Run Full Stress Test still exists alongside
    const btnStress = sandbox.document.getElementById("diag-btn-stress");
    assert(btnStress, "Button diag-btn-stress must exist");
    assert(modal.innerHTML.includes("Run Full Stress Test"), "Modal must contain Run Full Stress Test button");
}

// Test 11: Real Authentic Live Stage Counters
{
    console.log("Test 11: Real Authentic Live Stage Counters");
    const { sandbox } = createDOMContext();

    let intervalCallback = null;
    sandbox.setInterval = function (fn) {
        intervalCallback = fn;
        return 303;
    };
    sandbox.clearInterval = function () {
        intervalCallback = null;
    };

    let startAuditCalled = false;
    let auditMode = "";
    sandbox.RuntimePackVPad = {
        postCheatCmd: function () { return true; },
        startAudit: function (req) {
            startAuditCalled = true;
            try { auditMode = JSON.parse(req).mode; } catch (e) {}
            return "{\"status\":\"STARTED\"}";
        },
        getAuditEvents: function () {
            return JSON.stringify([
                { sequence_id: 0, stage: "REFERENCE_FOUND", outcome: "PASS", asset_path: "root", source_location: "audit_start" },
                { sequence_id: 1, stage: "FILE_EXISTS", outcome: "PASS", asset_path: "img/1.png" },
                { sequence_id: 2, stage: "DECODE_SUCCESS", outcome: "PASS", asset_path: "img/1.png" },
                { sequence_id: 3, stage: "FILE_EXISTS", outcome: "FAIL", asset_path: "img/missing.png", error_message: "Not found" },
                { sequence_id: 4, stage: "FILE_EXISTS", outcome: "FAIL", asset_path: "img/fallback.png", fallback_action: "PLACEHOLDER_SUBSTITUTED" },
                { sequence_id: 5, type: "DONE", message: "Finished" }
            ]);
        }
    };

    vm.runInContext(diagCode, sandbox);
    sandbox.DiagnosticSuite.init();

    // Trigger sampled probe
    sandbox.DiagnosticSuite.runProbe("sampled");
    assert.strictEqual(startAuditCalled, true, "startAudit must be called when triggering probe");
    assert.strictEqual(auditMode, "sampled");

    intervalCallback();

    const cntScanned = sandbox.document.getElementById("diag-cnt-scanned");
    const cntValid = sandbox.document.getElementById("diag-cnt-valid");
    const cntMissing = sandbox.document.getElementById("diag-cnt-missing");
    const cntFallback = sandbox.document.getElementById("diag-cnt-fallback");

    assert(cntScanned, "diag-cnt-scanned must exist");
    assert(cntValid, "diag-cnt-valid must exist");
    assert(cntMissing, "diag-cnt-missing must exist");
    assert(cntFallback, "diag-cnt-fallback must exist");

    assert.strictEqual(cntScanned.textContent, "4", "Scanned count should be 4 (ignoring root)");
    assert.strictEqual(cntValid.textContent, "2", "Valid count should be 2 (ignoring root)");
    assert.strictEqual(cntMissing.textContent, "1", "Missing count should be 1");
    assert.strictEqual(cntFallback.textContent, "1", "Fallback count should be 1");
}

// Test 12: exportBugReport includes combined_validation_summary
{
    console.log("Test 12: exportBugReport includes combined_validation_summary");
    const { sandbox } = createDOMContext();

    let sharedReportJson = null;
    const mockSummary = {
        schema_version: 1,
        summary_id: "test-summary-01",
        engine: "rpgmaker_web",
        static_outcome: "PASS",
        runtime_outcome: "FAIL",
        combined_outcome: "FAIL"
    };

    sandbox.RuntimePackVPad = {
        postCheatCmd: function () { return true; },
        getCombinedSummary: function () {
            return JSON.stringify(mockSummary);
        },
        shareBugReport: function (jsonStr) {
            sharedReportJson = jsonStr;
        }
    };

    vm.runInContext(diagCode, sandbox);
    sandbox.DiagnosticSuite.init();
    sandbox.DiagnosticSuite.exportBugReport();

    assert(sharedReportJson, "shareBugReport must be called");
    const parsed = JSON.parse(sharedReportJson);
    assert(parsed.combined_validation_summary, "Report must include combined_validation_summary");
    assert.strictEqual(parsed.combined_validation_summary.summary_id, "test-summary-01");
    assert.strictEqual(parsed.combined_validation_summary.combined_outcome, "FAIL");
}

// Test 13: Stress Test Real-Time Progress, Counter Sync, and Failure Enforcement
{
    console.log("Test 13: Stress Test Real-Time Progress, Counter Sync, and Failure Enforcement");
    const { sandbox } = createDOMContext();

    let intervalCallback = null;
    sandbox.setInterval = function (fn, ms) {
        intervalCallback = fn;
        return 999;
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
    const stStatus = sandbox.document.getElementById("diag-stress-status");
    const cntScanned = sandbox.document.getElementById("diag-cnt-scanned");
    const cntValid = sandbox.document.getElementById("diag-cnt-valid");
    const cntMissing = sandbox.document.getElementById("diag-cnt-missing");

    // Feed MAP line: should NOT increment scanned or valid
    mockProgress = "MAP: Scanning Map 1 (1/150)...\n";
    intervalCallback();
    assert.strictEqual(cntScanned.textContent, "0", "MAP line must not increment scannedCount");
    assert.strictEqual(cntValid.textContent, "0", "MAP line must not increment validCount");

    // Feed FAIL line: should increment failure and missing
    mockProgress = "FAIL: Missing Tileset: Inside_A2\n";
    intervalCallback();
    assert.strictEqual(cntMissing.textContent, "1", "FAIL line must increment missingCount");
    assert.strictEqual(cntScanned.textContent, "1", "FAIL line must increment scannedCount");
    assert.strictEqual(badge.textContent, "ISSUES (1)", "Badge must show ISSUES (1)");
    assert.strictEqual(badge.classList.contains("diag-badge-error"), true, "Badge must have semantic class diag-badge-error");

    // Feed DONE line with authoritative counts
    mockProgress = "DONE: Stress test completed: 150 maps, 543 exact assets, 0 case mismatches, 189 missing.\n";
    intervalCallback();

    assert.strictEqual(cntScanned.textContent, "732", "scannedCount should be exact + case + missing = 732");
    assert.strictEqual(cntValid.textContent, "543", "validCount should be 543");
    assert.strictEqual(cntMissing.textContent, "189", "missingCount should be 189");

    assert.strictEqual(badge.textContent, "FAILED", "Badge must be FAILED, not COMPLETED");
    assert.strictEqual(badge.classList.contains("diag-badge-error"), true, "Badge must have semantic class diag-badge-error");
    assert(stStatus.textContent.toLowerCase().includes("missing"), `Status text must mention missing assets, got: ${stStatus.textContent}`);
    assert(stStatus.textContent.includes("189"), `Status text must include 189 missing count, got: ${stStatus.textContent}`);

    const logs = sandbox.DiagnosticSuite.getLogs();
    const doneLogs = logs.filter(l => l.message && l.message.includes("Stress test completed"));
    assert(doneLogs.length > 0, "Must have log entry for DONE completion");
    assert.strictEqual(doneLogs[0].level, "WARN", "DONE log must have level WARN, NOT SUCCESS");
    const successLogs = logs.filter(l => l.level === "SUCCESS");
    assert.strictEqual(successLogs.length, 0, "There must be zero SUCCESS logs when missing assets exist");
}

// Test 14: Neutralized CSS & Zero Injected Hardcoded Styles
{
    console.log("Test 14: Neutralized CSS & Zero Injected Hardcoded Styles");
    const { sandbox } = createDOMContext();

    vm.runInContext(diagCode, sandbox);
    sandbox.DiagnosticSuite.init();

    // Verify no #rp-diag-styles was injected
    const styleEl = sandbox.document.getElementById("rp-diag-styles");
    assert.strictEqual(styleEl, null, "injectStyles must be neutralized and NOT inject #rp-diag-styles");

    // Verify initial badge has semantic class diag-badge-ready and no inline color
    const badge = sandbox.document.getElementById("diag-step-badge");
    assert(badge, "Badge element must exist");
    assert(badge.classList.contains("diag-badge-ready"), "Badge must have initial semantic class diag-badge-ready");
    assert.strictEqual(badge.style.background, undefined, "Badge must not have inline style.background");

    // Verify diagnostic_suite.js contains no presentation hex colors
    const hexRegex = /#[0-9a-fA-F]{3,8}\b/g;
    // Strip entity escaping if any (e.g. &#039;)
    const cleanedCode = diagCode.replace(/&#[0-9]{3,4};/g, "");
    const hexMatches = cleanedCode.match(hexRegex);
    assert.strictEqual(hexMatches, null, `diagnostic_suite.js must have zero presentation hex colors, found: ${hexMatches}`);
}

console.log("All Diagnostic Suite JS unit tests passed successfully!");
