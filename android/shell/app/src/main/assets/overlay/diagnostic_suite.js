// ============================================================================
// RuntimePack Floating Diagnostic & Automated Stress-Test Suite
// Overlay module for beta testing, real-time diagnostics, and engine audits.
// ============================================================================

(function (root) {
    "use strict";

    var globalObj = typeof window !== "undefined" ? window : (typeof global !== "undefined" ? global : root);
    var doc = typeof document !== "undefined" ? document : null;

    var logBuffer = [];
    var MAX_LOG_ENTRIES = 200;
    var fps = 0;
    var lastFrameTime = typeof performance !== "undefined" && performance.now ? performance.now() : Date.now();
    var frameCount = 0;
    var fpsUpdateInterval = null;
    var rAFId = null;
    var modalOpen = false;

    function formatTime(d) {
        var pad = function(n) { return (n < 10 ? "0" : "") + n; };
        return pad(d.getHours()) + ":" + pad(d.getMinutes()) + ":" + pad(d.getSeconds());
    }

    function addLogEntry(level, msg) {
        var entry = {
            time: formatTime(new Date()),
            level: level,
            message: typeof msg === "string" ? msg : (JSON.stringify(msg) || String(msg))
        };
        logBuffer.push(entry);
        if (logBuffer.length > MAX_LOG_ENTRIES) {
            logBuffer.shift();
        }
        if (doc) {
            var box = doc.getElementById("diag-log-box");
            if (box) {
                var el = doc.createElement("div");
                el.className = "diag-log-line diag-log-" + level.toLowerCase();
                el.innerHTML = "<span class=\"diag-log-time\">[" + entry.time + "]</span> " +
                               "<span class=\"diag-log-tag\">[" + entry.level + "]</span> " +
                               escapeHtml(entry.message);
                box.appendChild(el);
                box.scrollTop = box.scrollHeight;
            }
        }
    }

    function escapeHtml(str) {
        return String(str)
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/"/g, "&quot;")
            .replace(/'/g, "&#039;");
    }

    // Intercept Global Errors & Unhandled Rejections
    if (typeof globalObj.addEventListener === "function") {
        globalObj.addEventListener("error", function (e) {
            var loc = e.filename ? (" (" + e.filename + ":" + e.lineno + ")") : "";
            addLogEntry("ERROR", (e.message || String(e)) + loc);
        });
        globalObj.addEventListener("unhandledrejection", function (e) {
            var reason = e.reason ? (e.reason.message || String(e.reason)) : "Unknown rejection";
            addLogEntry("ERROR", "Unhandled Rejection: " + reason);
        });
    }

    // Wrap console.error and console.warn transparently
    if (typeof console !== "undefined") {
        var origError = console.error;
        console.error = function () {
            var args = Array.prototype.slice.call(arguments);
            var text = args.map(function (a) { return typeof a === "object" ? JSON.stringify(a) : String(a); }).join(" ");
            addLogEntry("ERROR", text);
            if (origError) origError.apply(console, arguments);
        };

        var origWarn = console.warn;
        console.warn = function () {
            var args = Array.prototype.slice.call(arguments);
            var text = args.map(function (a) { return typeof a === "object" ? JSON.stringify(a) : String(a); }).join(" ");
            addLogEntry("WARN", text);
            if (origWarn) origWarn.apply(console, arguments);
        };
    }

    function dispatchCommand(cmd) {
        if (typeof globalObj.RuntimePackVPad !== "undefined" && typeof globalObj.RuntimePackVPad.postCheatCmd === "function") {
            return globalObj.RuntimePackVPad.postCheatCmd(JSON.stringify(cmd));
        }
        if (typeof globalObj.executeCheatCmd === "function") {
            globalObj.executeCheatCmd(cmd);
            return true;
        }
        if (typeof globalObj.__cheatEngine !== "undefined" && typeof globalObj.__cheatEngine.postCommand === "function") {
            return globalObj.__cheatEngine.postCommand(cmd);
        }
        if (typeof globalObj.RuntimePackWeb !== "undefined" && typeof globalObj.RuntimePackWeb.postCheatCmd === "function") {
            return globalObj.RuntimePackWeb.postCheatCmd(JSON.stringify(cmd));
        }
        return false;
    }

    function updatePerfStats() {
        if (!doc) return;
        var fpsEl = doc.getElementById("diag-fps-val");
        if (fpsEl) fpsEl.textContent = fps + " FPS";

        var memEl = doc.getElementById("diag-mem-val");
        if (memEl) {
            if (typeof performance !== "undefined" && performance.memory) {
                var used = Math.round(performance.memory.usedJSHeapSize / (1024 * 1024));
                var total = Math.round(performance.memory.totalJSHeapSize / (1024 * 1024));
                memEl.textContent = used + "MB / " + total + "MB";
            } else {
                memEl.textContent = "N/A";
            }
        }
    }

    function startPerfLoop() {
        if (fpsUpdateInterval) return;
        frameCount = 0;
        lastFrameTime = (typeof performance !== "undefined" && performance.now) ? performance.now() : Date.now();

        var onFrame = function () {
            frameCount++;
            if (modalOpen) {
                rAFId = requestAnimationFrame(onFrame);
            }
        };
        rAFId = requestAnimationFrame(onFrame);

        fpsUpdateInterval = setInterval(function () {
            var now = (typeof performance !== "undefined" && performance.now) ? performance.now() : Date.now();
            var delta = (now - lastFrameTime) / 1000;
            if (delta > 0) {
                fps = Math.round(frameCount / delta);
            }
            frameCount = 0;
            lastFrameTime = now;
            updatePerfStats();
        }, 500);
    }

    function stopPerfLoop() {
        if (fpsUpdateInterval) {
            clearInterval(fpsUpdateInterval);
            fpsUpdateInterval = null;
        }
        if (rAFId && typeof cancelAnimationFrame === "function") {
            cancelAnimationFrame(rAFId);
            rAFId = null;
        }
    }

    function injectStyles() {
        if (!doc || doc.getElementById("rp-diag-styles")) return;
        var style = doc.createElement("style");
        style.id = "rp-diag-styles";
        style.textContent = "" +
            "#vpad-btn-diag {" +
            "   color: #38bdf8;" +
            "   font-size: 17px;" +
            "}" +
            "#rp-diag-modal {" +
            "   position: fixed;" +
            "   top: 50%;" +
            "   left: 50%;" +
            "   transform: translate(-50%, -50%);" +
            "   width: 92%;" +
            "   max-width: 500px;" +
            "   max-height: 86vh;" +
            "   background: rgba(18, 18, 24, 0.96);" +
            "   border: 1px solid rgba(56, 189, 248, 0.35);" +
            "   border-radius: 12px;" +
            "   box-shadow: 0 12px 36px rgba(0, 0, 0, 0.75);" +
            "   color: #e4e4e7;" +
            "   font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;" +
            "   display: none;" +
            "   flex-direction: column;" +
            "   z-index: 100003;" +
            "   box-sizing: border-box;" +
            "   pointer-events: auto;" +
            "   overflow: hidden;" +
            "}" +
            ".diag-header {" +
            "   display: flex;" +
            "   align-items: center;" +
            "   justify-content: space-between;" +
            "   padding: 10px 14px;" +
            "   background: rgba(255, 255, 255, 0.04);" +
            "   border-bottom: 1px solid rgba(255, 255, 255, 0.1);" +
            "}" +
            ".diag-title {" +
            "   font-size: 14px;" +
            "   font-weight: 700;" +
            "   color: #38bdf8;" +
            "   display: flex;" +
            "   align-items: center;" +
            "   gap: 6px;" +
            "}" +
            ".diag-badge {" +
            "   font-size: 9px;" +
            "   font-weight: 700;" +
            "   padding: 2px 5px;" +
            "   background: #0284c7;" +
            "   color: #ffffff;" +
            "   border-radius: 4px;" +
            "   text-transform: uppercase;" +
            "}" +
            ".diag-close-btn {" +
            "   background: transparent;" +
            "   border: none;" +
            "   color: #94a3b8;" +
            "   font-size: 16px;" +
            "   cursor: pointer;" +
            "   padding: 2px 6px;" +
            "}" +
            ".diag-close-btn:active { color: #f87171; }" +
            ".diag-perf-bar {" +
            "   display: flex;" +
            "   justify-content: space-around;" +
            "   padding: 8px 12px;" +
            "   background: rgba(0, 0, 0, 0.3);" +
            "   border-bottom: 1px solid rgba(255, 255, 255, 0.08);" +
            "   font-size: 11px;" +
            "   color: #94a3b8;" +
            "}" +
            ".diag-perf-item strong { color: #38bdf8; font-weight: 600; }" +
            ".diag-body {" +
            "   padding: 12px;" +
            "   overflow-y: auto;" +
            "   flex: 1;" +
            "   display: flex;" +
            "   flex-direction: column;" +
            "   gap: 12px;" +
            "}" +
            ".diag-section {" +
            "   background: rgba(255, 255, 255, 0.03);" +
            "   border: 1px solid rgba(255, 255, 255, 0.08);" +
            "   border-radius: 8px;" +
            "   padding: 10px;" +
            "}" +
            ".diag-sec-title {" +
            "   font-size: 12px;" +
            "   font-weight: 600;" +
            "   color: #f1f5f9;" +
            "   margin-bottom: 4px;" +
            "}" +
            ".diag-sec-desc {" +
            "   font-size: 10px;" +
            "   color: #94a3b8;" +
            "   margin-bottom: 8px;" +
            "}" +
            ".diag-btn {" +
            "   padding: 7px 12px;" +
            "   font-size: 11px;" +
            "   font-weight: 600;" +
            "   border-radius: 6px;" +
            "   border: none;" +
            "   cursor: pointer;" +
            "   transition: background 0.15s ease;" +
            "}" +
            ".diag-btn-primary {" +
            "   background: #0284c7;" +
            "   color: #ffffff;" +
            "   width: 100%;" +
            "}" +
            ".diag-btn-primary:active { background: #0369a1; }" +
            ".diag-btn-action {" +
            "   background: rgba(56, 189, 248, 0.15);" +
            "   border: 1px solid #38bdf8;" +
            "   color: #38bdf8;" +
            "}" +
            ".diag-btn-action:active { background: rgba(56, 189, 248, 0.3); }" +
            ".diag-btn-sm {" +
            "   padding: 3px 8px;" +
            "   font-size: 10px;" +
            "   background: rgba(255, 255, 255, 0.1);" +
            "   border: 1px solid rgba(255, 255, 255, 0.15);" +
            "   color: #cbd5e1;" +
            "   border-radius: 4px;" +
            "   cursor: pointer;" +
            "}" +
            ".diag-input-row {" +
            "   display: flex;" +
            "   gap: 6px;" +
            "   align-items: center;" +
            "   flex-wrap: wrap;" +
            "}" +
            ".diag-input-row label {" +
            "   font-size: 10px;" +
            "   color: #94a3b8;" +
            "   display: flex;" +
            "   align-items: center;" +
            "   gap: 4px;" +
            "}" +
            ".diag-input-row input {" +
            "   width: 48px;" +
            "   padding: 4px 6px;" +
            "   background: rgba(0, 0, 0, 0.4);" +
            "   border: 1px solid rgba(255, 255, 255, 0.15);" +
            "   border-radius: 4px;" +
            "   color: #ffffff;" +
            "   font-size: 11px;" +
            "   box-sizing: border-box;" +
            "}" +
            ".diag-status-text {" +
            "   font-size: 10px;" +
            "   color: #38bdf8;" +
            "   margin-top: 6px;" +
            "   min-height: 14px;" +
            "}" +
            ".diag-log-header {" +
            "   display: flex;" +
            "   justify-content: space-between;" +
            "   align-items: center;" +
            "   margin-bottom: 6px;" +
            "}" +
            ".diag-log-box {" +
            "   background: #09090b;" +
            "   border: 1px solid rgba(255, 255, 255, 0.08);" +
            "   border-radius: 6px;" +
            "   height: 120px;" +
            "   overflow-y: auto;" +
            "   padding: 6px 8px;" +
            "   font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;" +
            "   font-size: 10px;" +
            "   line-height: 1.4;" +
            "}" +
            ".diag-log-line { margin-bottom: 2px; word-break: break-all; }" +
            ".diag-log-time { color: #64748b; }" +
            ".diag-log-tag { font-weight: bold; margin-right: 4px; }" +
            ".diag-log-error { color: #f87171; }" +
            ".diag-log-warn { color: #fbbf24; }" +
            ".diag-log-info { color: #38bdf8; }";
        doc.head.appendChild(style);
    }

    function initDiagnosticDOM() {
        if (!doc) return;
        injectStyles();

        // 1. Inject Floating Bug Icon [ 🐞 ] in #vpad-topbar
        var topBar = doc.getElementById("vpad-topbar");
        if (!topBar) {
            topBar = doc.createElement("div");
            topBar.id = "vpad-topbar";
            doc.body.appendChild(topBar);
        }

        var bugBtn = doc.getElementById("vpad-btn-diag");
        if (!bugBtn) {
            bugBtn = doc.createElement("div");
            bugBtn.id = "vpad-btn-diag";
            bugBtn.className = "vpad-top-btn diag-interactive";
            bugBtn.title = "Diagnostic Suite";
            bugBtn.textContent = "🐞";
            attachTap(bugBtn, function () {
                DiagnosticSuite.toggle();
            });
            topBar.appendChild(bugBtn);
        }

        // 2. Inject Diagnostic Modal
        if (doc.getElementById("rp-diag-modal")) return;

        var modal = doc.createElement("div");
        modal.id = "rp-diag-modal";
        modal.className = "diag-interactive";
        modal.innerHTML = "" +
            "<div class=\"diag-header\">" +
            "    <div class=\"diag-title\">🐞 Diagnostic & Stress Suite <span class=\"diag-badge\">BETA</span></div>" +
            "    <button class=\"diag-close-btn\" id=\"diag-close-btn\">✕</button>" +
            "</div>" +
            "<div class=\"diag-perf-bar\">" +
            "    <div class=\"diag-perf-item\">FPS: <strong id=\"diag-fps-val\">--</strong></div>" +
            "    <div class=\"diag-perf-item\">Memory: <strong id=\"diag-mem-val\">--</strong></div>" +
            "</div>" +
            "<div class=\"diag-body\">" +
            "    <!-- Section: Automated Stress Test -->" +
            "    <div class=\"diag-section\">" +
            "        <div class=\"diag-sec-title\">Automated Stress Test</div>" +
            "        <div class=\"diag-sec-desc\">Scans all Maps and Database files, verifies disk assets, and writes audit_report.json.</div>" +
            "        <button id=\"diag-btn-stress\" class=\"diag-btn diag-btn-primary\">Run Full Stress Test</button>" +
            "        <div id=\"diag-stress-status\" class=\"diag-status-text\"></div>" +
            "    </div>" +
            "    <!-- Section: Map Warper / Teleport -->" +
            "    <div class=\"diag-section\">" +
            "        <div class=\"diag-sec-title\">Map Warper / Teleport</div>" +
            "        <div class=\"diag-sec-desc\">Instantly transport player to target Map ID and coordinates.</div>" +
            "        <div class=\"diag-input-row\">" +
            "            <label>Map: <input type=\"number\" id=\"diag-teleport-map\" value=\"1\" min=\"1\" /></label>" +
            "            <label>X: <input type=\"number\" id=\"diag-teleport-x\" value=\"0\" min=\"0\" /></label>" +
            "            <label>Y: <input type=\"number\" id=\"diag-teleport-y\" value=\"0\" min=\"0\" /></label>" +
            "            <button id=\"diag-btn-teleport\" class=\"diag-btn diag-btn-action\">Teleport</button>" +
            "        </div>" +
            "        <div id=\"diag-teleport-status\" class=\"diag-status-text\"></div>" +
            "    </div>" +
            "    <!-- Section: Bug Report Export -->" +
            "    <div class=\"diag-section\">" +
            "        <div class=\"diag-sec-title\">Export Bug Report</div>" +
            "        <div class=\"diag-sec-desc\">Captures scene name, map ID, coordinates, gold, and error logs into bug_report.json.</div>" +
            "        <button id=\"diag-btn-export\" class=\"diag-btn diag-btn-action\" style=\"width:100%\">Export Bug Report</button>" +
            "        <div id=\"diag-export-status\" class=\"diag-status-text\"></div>" +
            "    </div>" +
            "    <!-- Section: Live Error Log -->" +
            "    <div class=\"diag-section\">" +
            "        <div class=\"diag-log-header\">" +
            "            <span class=\"diag-sec-title\">Live Error Log</span>" +
            "            <button id=\"diag-btn-clear-log\" class=\"diag-btn-sm\">Clear Log</button>" +
            "        </div>" +
            "        <div id=\"diag-log-box\" class=\"diag-log-box\"></div>" +
            "    </div>" +
            "</div>";

        doc.body.appendChild(modal);

        // Bind interactive events
        var closeBtn = doc.getElementById("diag-close-btn");
        if (closeBtn) {
            attachTap(closeBtn, function () { DiagnosticSuite.close(); });
        }

        var stressBtn = doc.getElementById("diag-btn-stress");
        if (stressBtn) {
            attachTap(stressBtn, function () { DiagnosticSuite.runStressTest(); });
        }

        var teleportBtn = doc.getElementById("diag-btn-teleport");
        if (teleportBtn) {
            attachTap(teleportBtn, function () {
                var mapId = parseInt(doc.getElementById("diag-teleport-map").value, 10) || 1;
                var x = parseInt(doc.getElementById("diag-teleport-x").value, 10) || 0;
                var y = parseInt(doc.getElementById("diag-teleport-y").value, 10) || 0;
                DiagnosticSuite.teleportMap(mapId, x, y);
            });
        }

        var exportBtn = doc.getElementById("diag-btn-export");
        if (exportBtn) {
            attachTap(exportBtn, function () { DiagnosticSuite.exportBugReport(); });
        }

        var clearBtn = doc.getElementById("diag-btn-clear-log");
        if (clearBtn) {
            attachTap(clearBtn, function () { DiagnosticSuite.clearLogs(); });
        }

        // Prevent touches/clicks from propagating to game canvas or virtual controls
        ["touchstart", "touchmove", "touchend", "mousedown", "click"].forEach(function (evt) {
            modal.addEventListener(evt, function (e) {
                e.stopPropagation();
            }, { passive: false });
        });

        // Populate existing logs
        var box = doc.getElementById("diag-log-box");
        if (box && logBuffer.length > 0) {
            box.innerHTML = "";
            logBuffer.forEach(function (entry) {
                var el = doc.createElement("div");
                el.className = "diag-log-line diag-log-" + entry.level.toLowerCase();
                el.innerHTML = "<span class=\"diag-log-time\">[" + entry.time + "]</span> " +
                               "<span class=\"diag-log-tag\">[" + entry.level + "]</span> " +
                               escapeHtml(entry.message);
                box.appendChild(el);
            });
            box.scrollTop = box.scrollHeight;
        }
    }

    function attachTap(el, cb) {
        if (!el) return;
        var last = 0;
        var handler = function (e) {
            e.stopPropagation();
            if (e.preventDefault && e.cancelable) e.preventDefault();
            var now = Date.now();
            if (now - last < 250) return;
            last = now;
            cb(e);
        };
        el.addEventListener("touchend", handler);
        el.addEventListener("click", handler);
    }

    var DiagnosticSuite = {
        init: function () {
            initDiagnosticDOM();
        },
        toggle: function () {
            if (modalOpen) {
                this.close();
            } else {
                this.open();
            }
        },
        open: function () {
            if (!doc) return;
            var modal = doc.getElementById("rp-diag-modal");
            if (modal) {
                modal.style.display = "flex";
                modalOpen = true;
                startPerfLoop();
                if (globalObj.RuntimePackVPad && typeof globalObj.RuntimePackVPad.setOverlayModalOpen === "function") {
                    globalObj.RuntimePackVPad.setOverlayModalOpen(true);
                }
            }
        },
        close: function () {
            if (!doc) return;
            var modal = doc.getElementById("rp-diag-modal");
            if (modal) {
                modal.style.display = "none";
                modalOpen = false;
                stopPerfLoop();
                if (globalObj.RuntimePackVPad && typeof globalObj.RuntimePackVPad.setOverlayModalOpen === "function") {
                    globalObj.RuntimePackVPad.setOverlayModalOpen(false);
                }
            }
        },
        isOpen: function () {
            return modalOpen;
        },
        runStressTest: function () {
            var stStatus = doc ? doc.getElementById("diag-stress-status") : null;
            if (stStatus) stStatus.textContent = "Dispatched stress test to engine. Scanning...";
            addLogEntry("INFO", "Running Automated Stress Test...");
            var dispatched = dispatchCommand({ action: "run_stress_test" });
            if (dispatched) {
                if (stStatus) stStatus.textContent = "Stress test running. Check audit_report.json upon completion.";
            } else {
                if (stStatus) stStatus.textContent = "Stress test command dispatched (awaiting engine response).";
            }
        },
        teleportMap: function (mapId, x, y) {
            var tpStatus = doc ? doc.getElementById("diag-teleport-status") : null;
            if (tpStatus) tpStatus.textContent = "Teleporting to Map " + mapId + " (" + x + ", " + y + ")...";
            addLogEntry("INFO", "Teleporting to Map " + mapId + " (" + x + ", " + y + ")");
            dispatchCommand({ action: "teleport_map", map_id: mapId, x: x, y: y });
        },
        exportBugReport: function () {
            var expStatus = doc ? doc.getElementById("diag-export-status") : null;
            if (expStatus) expStatus.textContent = "Exporting bug report to bug_report.json...";
            addLogEntry("INFO", "Exporting bug report...");
            dispatchCommand({ action: "export_bug_report" });
            if (expStatus) expStatus.textContent = "Bug report exported to bug_report.json.";
        },
        log: function (msg, level) {
            addLogEntry(level || "INFO", msg);
        },
        getLogs: function () {
            return logBuffer.slice();
        },
        clearLogs: function () {
            logBuffer = [];
            if (doc) {
                var box = doc.getElementById("diag-log-box");
                if (box) box.innerHTML = "";
            }
        }
    };

    if (doc) {
        if (doc.readyState === "loading") {
            doc.addEventListener("DOMContentLoaded", initDiagnosticDOM);
        } else {
            initDiagnosticDOM();
        }
    }

    globalObj.DiagnosticSuite = DiagnosticSuite;

    if (typeof module !== "undefined" && module.exports) {
        module.exports = DiagnosticSuite;
    }
})(typeof window !== "undefined" ? window : this);
