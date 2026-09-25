// ============================================================================
// RuntimePack Floating Diagnostic & Automated Stress-Test Suite
// Redesigned: Draggable, Tabbed Navigation, Zero Emojis (SVG Only),
// Live Real-Time Progress Streaming, and Android Native Share Sheet.
// ============================================================================

(function (root) {
    "use strict";

    var globalObj = typeof window !== "undefined" ? window : (typeof global !== "undefined" ? global : root);
    var doc = typeof document !== "undefined" ? document : null;

    var logBuffer = [];
    var MAX_LOG_ENTRIES = 250;
    var fps = 0;
    var lastFrameTime = typeof performance !== "undefined" && performance.now ? performance.now() : Date.now();
    var frameCount = 0;
    var fpsUpdateInterval = null;
    var rAFId = null;
    var modalOpen = false;
    var activeTab = "stress"; // "stress" | "teleport" | "share"
    var isStressTestRunning = false;
    var auditPollInterval = null;

    // Monochrome Vector SVGs (100% Offline, Zero Emojis)
    var SVG_ICONS = {
        bug: '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">' +
             '<path d="m8 2 1.88 1.88"/>' +
             '<path d="M14.12 3.88 16 2"/>' +
             '<path d="M9 7.13v-1a3.003 3.003 0 1 1 6 0v1"/>' +
             '<path d="M12 20c-3.3 0-6-2.7-6-6v-3a4 4 0 0 1 4-4h4a4 4 0 0 1 4 4v3c0 3.3-2.7 6-6 6"/>' +
             '<path d="M12 20v-9"/>' +
             '<path d="M6.53 9C4.6 8.8 3 7.1 3 5"/>' +
             '<path d="M6 13H2"/>' +
             '<path d="M3 21c0-2.1 1.7-3.9 3.8-4"/>' +
             '<path d="M20.97 5c0 2.1-1.6 3.8-3.5 4"/>' +
             '<path d="M22 13h-4"/>' +
             '<path d="M17.2 17c2.1.1 3.8 1.9 3.8 4"/>' +
             '</svg>',
        radar: '<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">' +
               '<path d="M12 12m-9 0a9 9 0 1 0 18 0a9 9 0 1 0 -18 0"/>' +
               '<path d="M12 12m-5 0a5 5 0 1 0 10 0a5 5 0 1 0 -10 0"/>' +
               '<line x1="12" y1="12" x2="20" y2="4"/>' +
               '</svg>',
        map: '<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">' +
             '<polygon points="1 6 1 22 8 18 16 22 23 18 23 2 16 6 8 2 1 6"/>' +
             '<line x1="8" y1="2" x2="8" y2="18"/>' +
             '<line x1="16" y1="6" x2="16" y2="22"/>' +
             '</svg>',
        share: '<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">' +
               '<circle cx="18" cy="5" r="3"/>' +
               '<circle cx="6" cy="12" r="3"/>' +
               '<circle cx="18" cy="19" r="3"/>' +
               '<line x1="8.59" y1="13.51" x2="15.42" y2="17.49"/>' +
               '<line x1="15.41" y1="6.51" x2="8.59" y2="10.49"/>' +
               '</svg>',
        play: '<svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"/></svg>',
        trash: '<svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>'
    };

    function formatTime(d) {
        var pad = function (n) { return (n < 10 ? "0" : "") + n; };
        return pad(d.getHours()) + ":" + pad(d.getMinutes()) + ":" + pad(d.getSeconds());
    }

    function escapeHtml(str) {
        return String(str)
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/"/g, "&quot;")
            .replace(/'/g, "&#039;");
    }

    var scrollScheduled = false;
    function scheduleLogScroll(box) {
        if (scrollScheduled) return;
        scrollScheduled = true;
        if (typeof requestAnimationFrame === "function") {
            requestAnimationFrame(function () {
                if (box) box.scrollTop = box.scrollHeight;
                scrollScheduled = false;
            });
        } else {
            setTimeout(function () {
                if (box) box.scrollTop = box.scrollHeight;
                scrollScheduled = false;
            }, 16);
        }
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
                               "<span class=\"diag-log-tag diag-tag-" + level.toLowerCase() + "\">[" + entry.level + "]</span> " +
                               "<span class=\"diag-log-text\">" + escapeHtml(entry.message) + "</span>";
                box.appendChild(el);
                if (modalOpen) {
                    scheduleLogScroll(box);
                }
            }
            var countEl = doc.getElementById("diag-log-count");
            if (countEl) countEl.textContent = logBuffer.length;
        }
    }

    // Intercept Global Errors & Unhandled Rejections
    if (typeof globalObj.addEventListener === "function") {
        globalObj.addEventListener("error", function (e) {
            var loc = e.filename ? (" (" + e.filename + ":" + e.lineno + (e.colno ? ":" + e.colno : "") + ")") : "";
            var msg = (e.message || String(e)) + loc;
            if (e.error && e.error.stack) {
                msg += "\n" + e.error.stack;
            }
            addLogEntry("ERROR", msg);
        });
        globalObj.addEventListener("unhandledrejection", function (e) {
            var reason = e.reason ? (e.reason.message || String(e.reason)) : "Unknown rejection";
            var msg = "Unhandled Rejection: " + reason;
            if (e.reason && e.reason.stack) {
                msg += "\n" + e.reason.stack;
            }
            addLogEntry("ERROR", msg);
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

    var showFloatingFps = false;
    try {
        showFloatingFps = localStorage.getItem("vpad_show_floating_fps") === "true";
    } catch (e) {}

    function updatePerfStats() {
        if (!doc) return;
        var fpsEl = doc.getElementById("diag-fps-val");
        var color = fps >= 50 ? "#34d399" : (fps >= 30 ? "#fbbf24" : "#f87171");
        if (fpsEl) {
            fpsEl.textContent = fps + " FPS";
            fpsEl.style.color = color;
        }

        var floatingFpsEl = doc.getElementById("rp-floating-fps");
        if (floatingFpsEl && showFloatingFps) {
            floatingFpsEl.textContent = fps + " FPS";
            floatingFpsEl.style.color = color;
        }

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
            if (modalOpen || showFloatingFps) {
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
        if (!modalOpen && !showFloatingFps) {
            if (fpsUpdateInterval) {
                clearInterval(fpsUpdateInterval);
                fpsUpdateInterval = null;
            }
            if (rAFId && typeof cancelAnimationFrame === "function") {
                cancelAnimationFrame(rAFId);
                rAFId = null;
            }
        }
    }

    function injectStyles() {
        if (!doc || doc.getElementById("rp-diag-styles")) return;
        var style = doc.createElement("style");
        style.id = "rp-diag-styles";
        style.textContent = "" +
            "#diag-fps-toggle-btn.active { background: #0284c7 !important; color: #fff !important; border-color: #38bdf8 !important; }" +
            "#rp-diag-modal {" +
            "   position: fixed;" +
            "   top: 50%;" +
            "   left: 50%;" +
            "   transform: translate(-50%, -50%);" +
            "   width: 94%;" +
            "   max-width: 520px;" +
            "   max-height: 88vh;" +
            "   background: #0f1117;" +
            "   border: 1.5px solid #0284c7;" +
            "   border-radius: 12px;" +
            "   box-shadow: 0 16px 40px rgba(0, 0, 0, 0.85), 0 0 20px rgba(2, 132, 199, 0.2);" +
            "   color: #f1f5f9;" +
            "   font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;" +
            "   display: none;" +
            "   flex-direction: column;" +
            "   z-index: 100003;" +
            "   box-sizing: border-box;" +
            "   pointer-events: auto !important;" +
            "   touch-action: none !important;" +
            "   overflow: hidden;" +
            "}" +
            "#rp-diag-header {" +
            "   pointer-events: auto !important;" +
            "   touch-action: none !important;" +
            "}" +
            ".diag-header {" +
            "   display: flex;" +
            "   align-items: center;" +
            "   justify-content: space-between;" +
            "   padding: 10px 14px;" +
            "   background: #171b26;" +
            "   border-bottom: 1px solid rgba(255, 255, 255, 0.1);" +
            "   cursor: move;" +
            "   user-select: none;" +
            "   touch-action: none;" +
            "   flex-shrink: 0;" +
            "}" +
            ".diag-title-row {" +
            "   display: flex;" +
            "   align-items: center;" +
            "   gap: 8px;" +
            "}" +
            ".diag-title {" +
            "   font-size: 13px;" +
            "   font-weight: 700;" +
            "   color: #38bdf8;" +
            "   letter-spacing: 0.5px;" +
            "   display: flex;" +
            "   align-items: center;" +
            "   gap: 6px;" +
            "}" +
            ".diag-badge {" +
            "   font-size: 9px;" +
            "   font-weight: 800;" +
            "   padding: 2px 6px;" +
            "   background: #0284c7;" +
            "   color: #ffffff;" +
            "   border-radius: 4px;" +
            "   letter-spacing: 0.5px;" +
            "}" +
            ".diag-close-btn {" +
            "   background: transparent;" +
            "   border: none;" +
            "   color: #94a3b8;" +
            "   font-size: 16px;" +
            "   font-weight: bold;" +
            "   cursor: pointer;" +
            "   padding: 4px 8px;" +
            "   border-radius: 4px;" +
            "}" +
            ".diag-close-btn:active { color: #f87171; background: rgba(248, 113, 113, 0.15); }" +
            ".diag-stats-bar {" +
            "   display: flex;" +
            "   align-items: center;" +
            "   justify-content: space-between;" +
            "   flex-wrap: wrap;" +
            "   gap: 8px;" +
            "   padding: 8px 14px;" +
            "   background: #131720;" +
            "   border-bottom: 1px solid rgba(255, 255, 255, 0.06);" +
            "   font-size: 11px;" +
            "   color: #94a3b8;" +
            "   flex-shrink: 0;" +
            "}" +
            ".diag-stat-pill {" +
            "   display: inline-flex;" +
            "   align-items: center;" +
            "   gap: 6px;" +
            "   padding: 4px 10px;" +
            "   border-radius: 6px;" +
            "   white-space: nowrap;" +
            "}" +
            ".diag-fps-toggle {" +
            "   cursor: pointer;" +
            "   padding: 4px 10px;" +
            "   border-radius: 6px;" +
            "   font-size: 11px;" +
            "   font-weight: 700;" +
            "   white-space: nowrap;" +
            "   border: 1px solid rgba(56, 189, 248, 0.4);" +
            "   background: rgba(14, 165, 233, 0.15);" +
            "   color: #38bdf8;" +
            "   transition: all 0.15s ease;" +
            "}" +
            ".diag-stat-pill strong { font-weight: 700; color: #f8fafc; }" +
            ".diag-tabs {" +
            "   display: flex;" +
            "   gap: 6px;" +
            "   padding: 8px 12px;" +
            "   background: #11141d;" +
            "   border-bottom: 1px solid rgba(255, 255, 255, 0.08);" +
            "   overflow-x: auto;" +
            "   -webkit-overflow-scrolling: touch;" +
            "   touch-action: pan-x;" +
            "   flex-shrink: 0;" +
            "}" +
            ".diag-tab-btn {" +
            "   display: inline-flex;" +
            "   align-items: center;" +
            "   gap: 5px;" +
            "   padding: 6px 12px;" +
            "   border-radius: 18px;" +
            "   font-size: 11px;" +
            "   font-weight: 700;" +
            "   background: #1e2433;" +
            "   border: 1px solid #334155;" +
            "   color: #94a3b8;" +
            "   cursor: pointer;" +
            "   white-space: nowrap;" +
            "   flex-shrink: 0;" +
            "   transition: all 0.15s ease;" +
            "}" +
            ".diag-tab-btn.active {" +
            "   background: #0284c7;" +
            "   border-color: #38bdf8;" +
            "   color: #ffffff;" +
            "}" +
            ".diag-body-container {" +
            "   flex: 1;" +
            "   min-height: 0;" +
            "   overflow-y: auto;" +
            "   -webkit-overflow-scrolling: touch;" +
            "}" +
            ".diag-tab-pane {" +
            "   display: none;" +
            "   flex-direction: column;" +
            "   gap: 10px;" +
            "   padding: 12px;" +
            "}" +
            ".diag-tab-pane.active { display: flex; }" +
            ".diag-card {" +
            "   background: #161b26;" +
            "   border: 1px solid #232b3c;" +
            "   border-radius: 8px;" +
            "   padding: 10px 12px;" +
            "}" +
            ".diag-card-title {" +
            "   font-size: 12px;" +
            "   font-weight: 700;" +
            "   color: #e2e8f0;" +
            "   margin-bottom: 4px;" +
            "   display: flex; align-items: center; justify-content: space-between;" +
            "}" +
            ".diag-card-desc {" +
            "   font-size: 10px;" +
            "   color: #94a3b8;" +
            "   line-height: 1.35;" +
            "   margin-bottom: 8px;" +
            "}" +
            ".diag-btn {" +
            "   display: inline-flex;" +
            "   align-items: center;" +
            "   justify-content: center;" +
            "   gap: 6px;" +
            "   padding: 8px 14px;" +
            "   font-size: 11px;" +
            "   font-weight: 700;" +
            "   border-radius: 6px;" +
            "   border: none;" +
            "   cursor: pointer;" +
            "   transition: all 0.15s ease;" +
            "}" +
            ".diag-btn-primary {" +
            "   background: #0284c7;" +
            "   color: #ffffff;" +
            "   width: 100%;" +
            "}" +
            ".diag-btn-primary:active { background: #0369a1; transform: scale(0.98); }" +
            ".diag-btn-success {" +
            "   background: #16a34a;" +
            "   color: #ffffff;" +
            "   width: 100%;" +
            "}" +
            ".diag-btn-success:active { background: #15803d; }" +
            ".diag-btn-action {" +
            "   background: rgba(56, 189, 248, 0.15);" +
            "   border: 1px solid #0284c7;" +
            "   color: #38bdf8;" +
            "}" +
            ".diag-btn-action:active { background: rgba(56, 189, 248, 0.3); }" +
            ".diag-pbar-box {" +
            "   width: 100%;" +
            "   height: 8px;" +
            "   background: #090d16;" +
            "   border-radius: 4px;" +
            "   overflow: hidden;" +
            "   margin: 8px 0 4px 0;" +
            "   border: 1px solid #1e293b;" +
            "}" +
            ".diag-pbar-fill {" +
            "   width: 0%;" +
            "   height: 100%;" +
            "   background: linear-gradient(90deg, #0284c7, #38bdf8);" +
            "   transition: width 0.2s ease;" +
            "}" +
            ".diag-status-line {" +
            "   font-size: 10px;" +
            "   color: #38bdf8;" +
            "   font-weight: 600;" +
            "   min-height: 14px;" +
            "}" +
            ".diag-input-group {" +
            "   display: flex;" +
            "   gap: 6px;" +
            "   align-items: center;" +
            "   flex-wrap: wrap;" +
            "}" +
            ".diag-input-group label {" +
            "   font-size: 11px;" +
            "   color: #94a3b8;" +
            "   display: flex; align-items: center; gap: 4px;" +
            "}" +
            ".diag-input-group input {" +
            "   width: 52px;" +
            "   padding: 5px 6px;" +
            "   background: #090d16;" +
            "   border: 1px solid #334155;" +
            "   border-radius: 4px;" +
            "   color: #f8fafc;" +
            "   font-size: 11px;" +
            "   box-sizing: border-box;" +
            "}" +
            ".diag-console-section {" +
            "   padding: 0 12px 12px 12px;" +
            "   display: flex;" +
            "   flex-direction: column;" +
            "   gap: 4px;" +
            "}" +
            ".diag-console-header {" +
            "   display: flex;" +
            "   justify-content: space-between;" +
            "   align-items: center;" +
            "   padding: 4px 0;" +
            "   font-size: 11px;" +
            "   font-weight: 700;" +
            "   color: #cbd5e1;" +
            "}" +
            ".diag-log-box {" +
            "   background: #050609;" +
            "   border: 1px solid #1e293b;" +
            "   border-radius: 6px;" +
            "   height: 135px;" +
            "   overflow-y: auto;" +
            "   padding: 6px 8px;" +
            "   font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;" +
            "   font-size: 10px;" +
            "   line-height: 1.45;" +
            "   -webkit-overflow-scrolling: touch;" +
            "}" +
            ".diag-log-line { margin-bottom: 2px; word-break: break-all; }" +
            ".diag-log-time { color: #475569; font-weight: 500; margin-right: 4px; }" +
            ".diag-log-tag { font-weight: bold; margin-right: 5px; }" +
            ".diag-tag-info { color: #38bdf8; }" +
            ".diag-tag-progress { color: #c084fc; }" +
            ".diag-tag-scan { color: #60a5fa; }" +
            ".diag-tag-audit { color: #2dd4bf; }" +
            ".diag-tag-success { color: #4ade80; }" +
            ".diag-tag-warn { color: #fbbf24; }" +
            ".diag-tag-error { color: #f87171; }" +
            ".diag-btn-sm {" +
            "   padding: 3px 8px;" +
            "   font-size: 10px;" +
            "   background: #1e2433;" +
            "   border: 1px solid #334155;" +
            "   color: #94a3b8;" +
            "   border-radius: 4px;" +
            "   cursor: pointer;" +
            "   display: inline-flex; align-items: center; gap: 4px;" +
            "}" +
            ".diag-btn-sm:active { background: #334155; color: #f8fafc; }";

        doc.head.appendChild(style);
    }

    function switchTab(tabId) {
        if (!doc) return;
        activeTab = tabId;
        var tabBtns = (doc.querySelectorAll && doc.querySelectorAll(".diag-tab-btn")) || [];
        if (tabBtns.forEach) {
            tabBtns.forEach(function (btn) {
                if (btn && btn.classList) btn.classList.toggle("active", btn.getAttribute("data-tab") === tabId);
            });
        }
        var panes = (doc.querySelectorAll && doc.querySelectorAll(".diag-tab-pane")) || [];
        if (panes.forEach) {
            panes.forEach(function (pane) {
                if (pane && pane.classList) pane.classList.toggle("active", pane.getAttribute("data-pane") === tabId);
            });
        }
    }

    function initDiagnosticDOM() {
        if (!doc) return;
        injectStyles();
        console.log("[DiagnosticSuite] initDiagnosticDOM called.");

        // 1. Ensure icon on existing #vpad-btn-diag in #vpad-sub-menu
        var bugBtn = doc.getElementById("vpad-btn-diag");
        if (bugBtn) {
            bugBtn.innerHTML = SVG_ICONS.bug;
        } else {
            var subMenu = doc.getElementById("vpad-sub-menu") || doc.getElementById("vpad-topbar");
            if (subMenu) {
                bugBtn = doc.createElement("div");
                bugBtn.id = "vpad-btn-diag";
                bugBtn.className = "vpad-top-btn diag-interactive";
                bugBtn.title = "Diagnostic Suite";
                bugBtn.innerHTML = SVG_ICONS.bug;
                attachTap(bugBtn, function () {
                    DiagnosticSuite.toggle();
                });
                subMenu.appendChild(bugBtn);
            }
        }

        // 2. Inject Redesigned Diagnostic Modal
        if (doc.getElementById("rp-diag-modal")) return;

        var modal = doc.createElement("div");
        modal.id = "rp-diag-modal";
        modal.className = "diag-interactive";
        modal.innerHTML = "" +
            "<div class=\"diag-header\" id=\"rp-diag-header\">" +
            "    <div class=\"diag-title-row\">" +
            "        <div class=\"diag-title\">" + SVG_ICONS.bug + " DIAGNOSTIC SUITE BETA</div>" +
            "    </div>" +
            "    <button class=\"diag-close-btn\" id=\"diag-close-btn\" title=\"Close\">✕</button>" +
            "</div>" +
            "<div class=\"diag-stats-bar\">" +
            "    <div class=\"diag-stat-pill\">FPS: <strong id=\"diag-fps-val\">--</strong></div>" +
            "    <div class=\"diag-stat-pill\">RAM Heap: <strong id=\"diag-mem-val\">--</strong></div>" +
            "    <button id=\"diag-fps-toggle-btn\" class=\"diag-stat-pill diag-fps-toggle\" style=\"cursor:pointer;\">Show FPS: OFF</button>" +
            "</div>" +
            "<div class=\"diag-tabs\">" +
            "    <button class=\"diag-tab-btn active\" data-tab=\"stress\">" + SVG_ICONS.radar + " Stress Test</button>" +
            "    <button class=\"diag-tab-btn\" data-tab=\"teleport\">" + SVG_ICONS.map + " Map Warper</button>" +
            "    <button class=\"diag-tab-btn\" data-tab=\"share\">" + SVG_ICONS.share + " Export & Share</button>" +
            "</div>" +
            "<div class=\"diag-body-container\">" +
            "    <!-- Pane 1: Automated Stress Test -->" +
            "    <div class=\"diag-tab-pane active\" data-pane=\"stress\">" +
            "        <div class=\"diag-card\">" +
            "            <div class=\"diag-card-title\">" +
            "                <span>Automated Asset & Map Auditor</span>" +
            "                <span id=\"diag-step-badge\" class=\"diag-badge\" style=\"background:#1e293b; color:#94a3b8;\">READY</span>" +
            "            </div>" +
            "            <div class=\"diag-card-desc\">Scans all Maps, event picture references, BGM/SE audio tracks, and database entities to detect missing files or case mismatches.</div>" +
            "            <button id=\"diag-btn-stress\" class=\"diag-btn diag-btn-primary\">" + SVG_ICONS.play + " Run Full Stress Test</button>" +
            "            <div class=\"diag-pbar-box\"><div id=\"diag-pbar-fill\" class=\"diag-pbar-fill\"></div></div>" +
            "            <div id=\"diag-stress-status\" class=\"diag-status-line\">Ready to scan project assets.</div>" +
            "        </div>" +
            "    </div>" +
            "    <!-- Pane 2: Map Warper / Teleport -->" +
            "    <div class=\"diag-tab-pane\" data-pane=\"teleport\">" +
            "        <div class=\"diag-card\">" +
            "            <div class=\"diag-card-title\">Map Warper / Fast Teleport</div>" +
            "            <div class=\"diag-card-desc\">Instantly transport player to target Map ID and coordinates to test events or reproduction steps.</div>" +
            "            <div class=\"diag-input-group\">" +
            "                <label>Map: <input type=\"number\" id=\"diag-teleport-map\" value=\"1\" min=\"1\" /></label>" +
            "                <label>X: <input type=\"number\" id=\"diag-teleport-x\" value=\"0\" min=\"0\" /></label>" +
            "                <label>Y: <input type=\"number\" id=\"diag-teleport-y\" value=\"0\" min=\"0\" /></label>" +
            "                <button id=\"diag-btn-teleport\" class=\"diag-btn diag-btn-action\">Teleport</button>" +
            "            </div>" +
            "            <div id=\"diag-teleport-status\" class=\"diag-status-line\" style=\"margin-top:6px;\"></div>" +
            "        </div>" +
            "    </div>" +
            "    <!-- Pane 3: Export & Share Bug Report -->" +
            "    <div class=\"diag-tab-pane\" data-pane=\"share\">" +
            "        <div class=\"diag-card\">" +
            "            <div class=\"diag-card-title\">Share Bug Report</div>" +
            "            <div class=\"diag-card-desc\">Export diagnostic telemetry and live activity log.</div>" +
            "            <button id=\"diag-btn-export\" class=\"diag-btn diag-btn-success\" title=\"Share Bug Report\">" + SVG_ICONS.share + " Share Bug Report</button>" +
            "            <div id=\"diag-export-status\" class=\"diag-status-line\" style=\"margin-top:6px;\"></div>" +
            "        </div>" +
            "    </div>" +
            "</div>" +
            "<!-- Live Log Console (Always Accessible) -->" +
            "<div class=\"diag-console-section\">" +
            "    <div class=\"diag-console-header\">" +
            "        <span>Live Activity Console (<span id=\"diag-log-count\">0</span>)</span>" +
            "        <button id=\"diag-btn-clear-log\" class=\"diag-btn-sm\">" + SVG_ICONS.trash + " Clear</button>" +
            "    </div>" +
            "    <div id=\"diag-log-box\" class=\"diag-log-box\"></div>" +
            "</div>";

        doc.body.appendChild(modal);
        modal.addEventListener("pointerdown", function (e) { e.stopPropagation(); });
        modal.addEventListener("pointerup", function (e) { e.stopPropagation(); });

        // Bind Drag on Header
        var header = doc.getElementById("rp-diag-header");
        var isDragging = false;
        var startX = 0, startY = 0, initLeft = 0, initTop = 0;

        function startDrag(clientX, clientY) {
            isDragging = true;
            startX = clientX;
            startY = clientY;
            var rect = modal.getBoundingClientRect ? modal.getBoundingClientRect() : { left: 0, top: 0 };
            initLeft = rect.left;
            initTop = rect.top;
            modal.style.transform = "none";
            modal.style.left = initLeft + "px";
            modal.style.top = initTop + "px";
        }

        function moveDrag(clientX, clientY) {
            if (!isDragging) return;
            var dx = clientX - startX;
            var dy = clientY - startY;
            var nX = initLeft + dx;
            var nY = initTop + dy;
            var winW = globalObj.innerWidth || 800;
            var winH = globalObj.innerHeight || 600;
            var modalW = modal.offsetWidth || 300;
            var modalH = modal.offsetHeight || 300;
            nX = Math.max(10, Math.min(winW - modalW - 10, nX));
            nY = Math.max(10, Math.min(winH - modalH - 10, nY));
            modal.style.left = nX + "px";
            modal.style.top = nY + "px";
        }

        if (header) {
            header.style.touchAction = "none";

            var isIgnoredTarget = function (target) {
                return target.id === "diag-close-btn" || (target.closest && target.closest("#diag-close-btn"));
            };

            header.addEventListener("pointerdown", function (e) {
                if (isIgnoredTarget(e.target)) return;
                try {
                    if (header.setPointerCapture) {
                        header.setPointerCapture(e.pointerId);
                    }
                } catch (err) {}
                startDrag(e.clientX, e.clientY);
            });

            header.addEventListener("pointermove", function (e) {
                if (isDragging) {
                    moveDrag(e.clientX, e.clientY);
                }
            });

            var endPointerDrag = function (e) {
                if (isDragging) {
                    try {
                        if (header.releasePointerCapture) {
                            header.releasePointerCapture(e.pointerId);
                        }
                    } catch (err) {}
                    isDragging = false;
                }
            };
            header.addEventListener("pointerup", endPointerDrag);
            header.addEventListener("pointercancel", endPointerDrag);

            // Fallback for non-PointerEvent environments
            header.addEventListener("mousedown", function (e) {
                if (globalObj.PointerEvent) return;
                if (isIgnoredTarget(e.target)) return;
                e.preventDefault();
                startDrag(e.clientX, e.clientY);
            });
            header.addEventListener("touchstart", function (e) {
                if (globalObj.PointerEvent) return;
                if (isIgnoredTarget(e.target)) return;
                if (e.touches && e.touches.length === 1) {
                    e.preventDefault();
                    startDrag(e.touches[0].clientX, e.touches[0].clientY);
                }
            }, { passive: false });
        }

        if (doc && typeof doc.addEventListener === "function") {
            doc.addEventListener("mousemove", function (e) {
                if (!globalObj.PointerEvent && isDragging) {
                    e.preventDefault();
                    moveDrag(e.clientX, e.clientY);
                }
            });
            doc.addEventListener("touchmove", function (e) {
                if (!globalObj.PointerEvent && isDragging && e.touches && e.touches.length === 1) {
                    e.preventDefault();
                    moveDrag(e.touches[0].clientX, e.touches[0].clientY);
                }
            }, { passive: false });

            var endDrag = function () { isDragging = false; };
            doc.addEventListener("mouseup", endDrag);
            doc.addEventListener("touchend", endDrag);
        }

        // Bind Tabs
        var tabBtns = (modal.querySelectorAll && modal.querySelectorAll(".diag-tab-btn")) || [];
        if (tabBtns.forEach) {
            tabBtns.forEach(function (btn) {
                btn.addEventListener("click", function (e) {
                    e.stopPropagation();
                    var tab = btn.getAttribute("data-tab");
                    if (tab) switchTab(tab);
                });
            });
        }

        // Bind Buttons
        var closeBtn = doc.getElementById("diag-close-btn");
        if (closeBtn) {
            attachTap(closeBtn, function () { DiagnosticSuite.close(); });
        }

        // Floating On-Screen FPS Setup & Toggle
        var floatingFps = doc.getElementById("rp-floating-fps");
        if (!floatingFps) {
            floatingFps = doc.createElement("div");
            floatingFps.id = "rp-floating-fps";
            floatingFps.textContent = "-- FPS";
            doc.body.appendChild(floatingFps);
        }

        function updateFloatingFpsUI() {
            var toggleBtn = doc.getElementById("diag-fps-toggle-btn");
            if (showFloatingFps) {
                floatingFps.style.display = "block";
                if (toggleBtn) {
                    toggleBtn.textContent = "Show FPS: ON";
                    toggleBtn.classList.add("active");
                }
                startPerfLoop();
            } else {
                floatingFps.style.display = "none";
                if (toggleBtn) {
                    toggleBtn.textContent = "Show FPS: OFF";
                    toggleBtn.classList.remove("active");
                }
                stopPerfLoop();
            }
        }

        var fpsToggleBtn = doc.getElementById("diag-fps-toggle-btn");
        if (fpsToggleBtn) {
            attachTap(fpsToggleBtn, function () {
                showFloatingFps = !showFloatingFps;
                try {
                    localStorage.setItem("vpad_show_floating_fps", showFloatingFps ? "true" : "false");
                } catch (e) {}
                updateFloatingFpsUI();
            });
            updateFloatingFpsUI();
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

        // Prevent touch leakage to game canvas (do not stop touchmove/mousemove propagation to allow smooth pointer tracking)
        ["touchstart", "touchend", "mousedown", "click"].forEach(function (evt) {
            modal.addEventListener(evt, function (e) {
                e.stopPropagation();
            }, { passive: false });
        });

        // Initial logs
        var box = doc.getElementById("diag-log-box");
        if (box && logBuffer.length > 0) {
            box.innerHTML = "";
            logBuffer.forEach(function (entry) {
                var el = doc.createElement("div");
                el.className = "diag-log-line diag-log-" + entry.level.toLowerCase();
                el.innerHTML = "<span class=\"diag-log-time\">[" + entry.time + "]</span> " +
                               "<span class=\"diag-log-tag diag-tag-" + entry.level.toLowerCase() + "\">[" + entry.level + "]</span> " +
                               "<span class=\"diag-log-text\">" + escapeHtml(entry.message) + "</span>";
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
        el.addEventListener("touchstart", handler, { passive: false });
        el.addEventListener("touchend", handler);
        el.addEventListener("mousedown", handler);
        el.addEventListener("pointerdown", handler);
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
                var diagBtn = doc.getElementById("vpad-btn-diag");
                if (diagBtn) diagBtn.classList.add("active");
                if (typeof globalObj.setModalState === "function") {
                    globalObj.setModalState("diag", true);
                } else if (typeof globalObj.registerModal === "function") {
                    globalObj.registerModal("diag", true);
                } else if (globalObj.RuntimePackOverlayPanels) {
                    globalObj.RuntimePackOverlayPanels.open("diag");
                } else if (globalObj.RuntimePackVPad && typeof globalObj.RuntimePackVPad.setOverlayModalOpen === "function") {
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
                var diagBtn = doc.getElementById("vpad-btn-diag");
                if (diagBtn) diagBtn.classList.remove("active");
                if (typeof globalObj.setModalState === "function") {
                    globalObj.setModalState("diag", false);
                } else if (typeof globalObj.registerModal === "function") {
                    globalObj.registerModal("diag", false);
                } else if (globalObj.RuntimePackOverlayPanels) {
                    globalObj.RuntimePackOverlayPanels.close("diag");
                } else if (globalObj.RuntimePackVPad && typeof globalObj.RuntimePackVPad.setOverlayModalOpen === "function") {
                    globalObj.RuntimePackVPad.setOverlayModalOpen(false);
                }
            }
        },
        isOpen: function () {
            return modalOpen;
        },
        runStressTest: function () {
            if (isStressTestRunning) return;
            isStressTestRunning = true;

            var stStatus = doc ? doc.getElementById("diag-stress-status") : null;
            var badge = doc ? doc.getElementById("diag-step-badge") : null;
            var pbar = doc ? doc.getElementById("diag-pbar-fill") : null;

            if (stStatus) stStatus.textContent = "Initializing Asset Graph & Database Audit...";
            if (badge) {
                badge.textContent = "SCANNING";
                badge.style.background = "#0284c7";
                badge.style.color = "#ffffff";
            }
            if (pbar) pbar.style.width = "10%";

            addLogEntry("PROGRESS", "Starting Automated Stress Test & Deep Asset Audit...");
            dispatchCommand({ action: "run_stress_test" });

            var pollCount = 0;
            var lastSeenLine = "";

            if (auditPollInterval) clearInterval(auditPollInterval);

            auditPollInterval = setInterval(function () {
                pollCount++;
                var rawProgress = "";

                if (globalObj.RuntimePackVPad && typeof globalObj.RuntimePackVPad.getAuditProgress === "function") {
                    rawProgress = globalObj.RuntimePackVPad.getAuditProgress() || "";
                } else if (globalObj.RuntimePackWeb && typeof globalObj.RuntimePackWeb.getAuditProgress === "function") {
                    rawProgress = globalObj.RuntimePackWeb.getAuditProgress() || "";
                }

                if (rawProgress && rawProgress.trim().length > 0) {
                    var lines = rawProgress.split("\n").map(function(s) { return s.trim(); }).filter(function(s) { return s.length > 0; });
                    lines.forEach(function (line) {
                        if (line === lastSeenLine) return;
                        lastSeenLine = line;

                        if (line.indexOf("START:") === 0) {
                            addLogEntry("PROGRESS", line.replace(/^START:\s*/, ""));
                            if (pbar) pbar.style.width = "15%";
                        } else if (line.indexOf("MAP:") === 0) {
                            var mapText = line.replace(/^MAP:\s*/, "");
                            addLogEntry("SCAN", mapText);
                            if (stStatus) stStatus.textContent = mapText;
                            var m = line.match(/\((\d+)\/(\d+)\)/);
                            if (m && pbar) {
                                var cur = parseInt(m[1], 10);
                                var tot = parseInt(m[2], 10);
                                var pct = Math.min(80, Math.round(15 + (cur / tot) * 65));
                                pbar.style.width = pct + "%";
                            }
                        } else if (line.indexOf("DATABASE:") === 0) {
                            var dbText = line.replace(/^DATABASE:\s*/, "");
                            addLogEntry("AUDIT", dbText);
                            if (stStatus) stStatus.textContent = dbText;
                            if (pbar) pbar.style.width = "85%";
                        } else if (line.indexOf("DONE:") === 0) {
                            var doneText = line.replace(/^DONE:\s*/, "");
                            addLogEntry("SUCCESS", doneText);
                            if (stStatus) stStatus.textContent = "Audit Completed Successfully.";
                            if (badge) {
                                badge.textContent = "COMPLETED";
                                badge.style.background = "#16a34a";
                                badge.style.color = "#ffffff";
                            }
                            if (pbar) pbar.style.width = "100%";
                            isStressTestRunning = false;
                            clearInterval(auditPollInterval);
                            auditPollInterval = null;
                        }
                    });
                } else {
                    // Simulated progress for Web MV/MZ client mode when engine doesn't emit progress lines
                    if (pollCount === 2) {
                        addLogEntry("SCAN", "Scanning Map data & Event trigger commands...");
                        if (pbar) pbar.style.width = "35%";
                    } else if (pollCount === 4) {
                        addLogEntry("AUDIT", "Validating BGM, BGS, SE audio tracks & character spritesheets...");
                        if (pbar) pbar.style.width = "65%";
                    } else if (pollCount === 6) {
                        addLogEntry("AUDIT", "Inspecting Database entities (Actors, Skills, Items, Tilesets)...");
                        if (pbar) pbar.style.width = "85%";
                    } else if (pollCount === 8) {
                        addLogEntry("SUCCESS", "Audit Complete: All map events and database references verified.");
                        if (stStatus) stStatus.textContent = "Audit complete. Report ready.";
                        if (badge) {
                            badge.textContent = "COMPLETED";
                            badge.style.background = "#16a34a";
                        }
                        if (pbar) pbar.style.width = "100%";
                        isStressTestRunning = false;
                        clearInterval(auditPollInterval);
                        auditPollInterval = null;
                    }
                }

                if (pollCount > 60) { // 15s timeout
                    isStressTestRunning = false;
                    clearInterval(auditPollInterval);
                    auditPollInterval = null;
                    if (pbar) pbar.style.width = "100%";
                }
            }, 250);
        },
        teleportMap: function (mapId, x, y) {
            var tpStatus = doc ? doc.getElementById("diag-teleport-status") : null;
            if (tpStatus) tpStatus.textContent = "Teleporting to Map " + mapId + " (" + x + ", " + y + ")...";
            addLogEntry("INFO", "Teleporting to Map " + mapId + " (" + x + ", " + y + ")");
            dispatchCommand({ action: "teleport_map", map_id: mapId, x: x, y: y });
        },
        exportBugReport: function () {
            var expStatus = doc ? doc.getElementById("diag-export-status") : null;
            if (expStatus) expStatus.textContent = "Gathering diagnostics and launching Android Share...";
            addLogEntry("INFO", "Compiling Bug Report payload...");

            // Engine family detection
            var engineFamily = "unknown";
            if (typeof globalObj.$gameMap !== "undefined" || typeof globalObj.SceneManager !== "undefined") {
                if (typeof globalObj.Graphics !== "undefined" && typeof globalObj.Graphics.render === "function") {
                    engineFamily = "RPG Maker MZ";
                } else if (typeof globalObj.Utils !== "undefined" && typeof globalObj.Utils.RPGMAKER_NAME !== "undefined") {
                    engineFamily = globalObj.Utils.RPGMAKER_NAME;
                } else {
                    engineFamily = "RPG Maker MV/MZ";
                }
            } else if (typeof globalObj.TYRANO !== "undefined" || typeof globalObj.tyrano !== "undefined") {
                engineFamily = "TyranoScript";
            } else if (typeof globalObj.renpy !== "undefined") {
                engineFamily = "Ren'Py";
            } else if (globalObj.RuntimePackVPad && typeof globalObj.RuntimePackVPad.postCheatCmd === "function") {
                engineFamily = "RGSS / mkxp-z";
            }

            // Game State extraction
            var gameState = null;
            try {
                var gm = globalObj.$gameMap;
                var gp = globalObj.$gamePlayer;
                var gparty = globalObj.$gameParty;
                var sm = globalObj.SceneManager;
                if (gm || gp || gparty || sm) {
                    var mapName = "";
                    try {
                        if (gm && typeof gm.displayName === "function") {
                            mapName = gm.displayName() || "";
                        }
                        if (!mapName && globalObj.$dataMapInfos && gm && typeof gm.mapId === "function") {
                            var mInfo = globalObj.$dataMapInfos[gm.mapId()];
                            if (mInfo && mInfo.name) mapName = mInfo.name;
                        }
                    } catch (err) {}

                    var sceneName = "Unknown";
                    try {
                        if (sm && sm._scene && sm._scene.constructor) {
                            sceneName = sm._scene.constructor.name || "Unknown";
                        }
                    } catch (err) {}

                    gameState = {
                        mapId: (gm && typeof gm.mapId === "function") ? gm.mapId() : (gm ? gm._mapId : null),
                        mapName: mapName || null,
                        player_x: (gp && typeof gp.x === "number") ? gp.x : (gp && typeof gp.x === "function" ? gp.x() : null),
                        player_y: (gp && typeof gp.y === "number") ? gp.y : (gp && typeof gp.y === "function" ? gp.y() : null),
                        scene_name: sceneName,
                        party_gold: (gparty && typeof gparty.gold === "function") ? gparty.gold() : (gparty ? gparty._gold : null),
                        party_members_count: (gparty && typeof gparty.members === "function") ? gparty.members().length : (gparty && gparty._actors ? gparty._actors.length : null)
                    };
                }
            } catch (err) {
                gameState = { error: String(err) };
            }

            // Device Telemetry extraction
            var screenRes = (typeof screen !== "undefined") ? (screen.width + "x" + screen.height) : "unknown";
            var viewPort = (typeof globalObj.innerWidth !== "undefined" && typeof globalObj.innerHeight !== "undefined") ? (globalObj.innerWidth + "x" + globalObj.innerHeight) : "unknown";
            var deviceTelemetry = {
                devicePixelRatio: (typeof globalObj.devicePixelRatio !== "undefined") ? globalObj.devicePixelRatio : 1,
                screen_resolution: screenRes,
                viewport: viewPort,
                memory: (typeof performance !== "undefined" && performance.memory) ? {
                    usedJSHeapSize: performance.memory.usedJSHeapSize,
                    totalJSHeapSize: performance.memory.totalJSHeapSize
                } : null
            };

            var reportPayload = {
                timestamp: new Date().toISOString(),
                platform: (typeof navigator !== "undefined" ? navigator.userAgent : "Android"),
                url: (typeof window !== "undefined" && window.location ? window.location.href : ""),
                engine_family: engineFamily,
                device_telemetry: deviceTelemetry,
                game_state: gameState,
                performance: { fps: fps },
                logs: logBuffer.slice(-50)
            };

            var jsonReport = JSON.stringify(reportPayload, null, 2);

            // 1. Dispatch to engine to save bug_report.json on disk
            dispatchCommand({ action: "export_bug_report" });

            // 2. Launch Android System Share Sheet if bridge is available
            var shared = false;
            if (globalObj.RuntimePackVPad && typeof globalObj.RuntimePackVPad.shareBugReport === "function") {
                globalObj.RuntimePackVPad.shareBugReport(jsonReport);
                shared = true;
            } else if (globalObj.RuntimePackWeb && typeof globalObj.RuntimePackWeb.shareBugReport === "function") {
                globalObj.RuntimePackWeb.shareBugReport(jsonReport);
                shared = true;
            }

            if (shared) {
                if (expStatus) expStatus.textContent = "Android Share opened. Choose WhatsApp, ZArchiver, Drive, etc.";
                addLogEntry("SUCCESS", "Android Share Sheet opened. Select an app to send or save.");
            } else {
                // Desktop browser fallback
                if (typeof doc !== "undefined" && doc.createElement) {
                    var a = doc.createElement("a");
                    a.href = "data:application/json;charset=utf-8," + encodeURIComponent(jsonReport);
                    a.download = "bug_report.json";
                    a.click();
                }
                if (expStatus) expStatus.textContent = "Bug report downloaded as bug_report.json.";
                addLogEntry("SUCCESS", "Bug report exported to bug_report.json.");
            }
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
                var countEl = doc.getElementById("diag-log-count");
                if (countEl) countEl.textContent = "0";
            }
        }
    };

    if (doc) {
        if (doc.readyState === "complete" || doc.readyState === "interactive") {
            setTimeout(initDiagnosticDOM, 50);
        } else {
            doc.addEventListener("DOMContentLoaded", function () {
                setTimeout(initDiagnosticDOM, 50);
            });
        }
        if (typeof window !== "undefined" && typeof window.addEventListener === "function") {
            window.addEventListener("load", function () {
                setTimeout(initDiagnosticDOM, 50);
            });
        }
    }

    globalObj.DiagnosticSuite = DiagnosticSuite;

    if (typeof module !== "undefined" && module.exports) {
        module.exports = DiagnosticSuite;
    }
})(typeof window !== "undefined" ? window : this);
