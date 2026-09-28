// ============================================================================
// RuntimePack Floating Diagnostic & Automated Stress-Test Suite
// Redesigned: Draggable, Tabbed Navigation, Zero Emojis (SVG Only),
// Live Real-Time Progress Streaming, and Android Native Share Sheet.
// ============================================================================

(function (root) {
    "use strict";

    var globalObj = typeof window !== "undefined" ? window : (typeof global !== "undefined" ? global : root);
    var doc = typeof document !== "undefined" ? document : null;

    function elevatePanel(panelOrId) {
        if (typeof globalObj.bringPanelToFront === "function" && globalObj.bringPanelToFront !== elevatePanel) {
            return globalObj.bringPanelToFront(panelOrId);
        }
        if (!panelOrId) return null;
        var el = panelOrId;
        if (typeof panelOrId === "string" && doc) {
            var cleanId = panelOrId.replace(/^#/, "");
            el = doc.getElementById(cleanId) || (doc.querySelector ? doc.querySelector(panelOrId) : null);
        }
        if (!el || !el.style) return null;
        var cur = (typeof globalObj.__topPanelZIndex === "number") ? globalObj.__topPanelZIndex : 100010;
        cur += 2;
        globalObj.__topPanelZIndex = cur;
        el.style.zIndex = cur;
        return el;
    }
    if (typeof globalObj.bringPanelToFront !== "function") {
        globalObj.bringPanelToFront = elevatePanel;
        globalObj.elevatePanel = elevatePanel;
    }

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
        if (fpsEl) {
            fpsEl.textContent = fps + " FPS";
            if (fpsEl.classList) {
                fpsEl.classList.remove("diag-fps-high", "diag-fps-mid", "diag-fps-low");
                if (fps >= 50) {
                    fpsEl.classList.add("diag-fps-high");
                } else if (fps >= 30) {
                    fpsEl.classList.add("diag-fps-mid");
                } else {
                    fpsEl.classList.add("diag-fps-low");
                }
            }
        }

        var floatingFpsEl = doc.getElementById("rp-floating-fps");
        if (floatingFpsEl && showFloatingFps) {
            floatingFpsEl.textContent = fps + " FPS";
            if (floatingFpsEl.classList) {
                floatingFpsEl.classList.remove("diag-fps-high", "diag-fps-mid", "diag-fps-low");
                if (fps >= 50) {
                    floatingFpsEl.classList.add("diag-fps-high");
                } else if (fps >= 30) {
                    floatingFpsEl.classList.add("diag-fps-mid");
                } else {
                    floatingFpsEl.classList.add("diag-fps-low");
                }
            }
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
        // No-op: base structural styles and theme variables are now centrally managed in vpad.css
    }

    function setBadgeState(badgeEl, text, stateClass) {
        if (!badgeEl) return;
        badgeEl.textContent = text;
        if (badgeEl.classList) {
            badgeEl.classList.remove("diag-badge-scanning", "diag-badge-ready", "diag-badge-success", "diag-badge-error", "diag-badge-warn");
            if (stateClass) {
                badgeEl.classList.add(stateClass);
            }
        }
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
            "    <button class=\"diag-tab-btn active\" data-tab=\"stress\">" + SVG_ICONS.radar + " Asset Audit & Probe</button>" +
            "    <button class=\"diag-tab-btn\" data-tab=\"teleport\">" + SVG_ICONS.map + " Map Warper</button>" +
            "    <button class=\"diag-tab-btn\" data-tab=\"share\">" + SVG_ICONS.share + " Export & Share</button>" +
            "</div>" +
            "<div class=\"diag-body-container\">" +
            "    <!-- Pane 1: Automated Asset Audit & Probe -->" +
            "    <div class=\"diag-tab-pane active\" data-pane=\"stress\">" +
            "        <div class=\"diag-card\">" +
            "            <div class=\"diag-card-title\">" +
            "                <span>Automated Asset & Map Auditor</span>" +
            "                <span id=\"diag-step-badge\" class=\"diag-badge diag-badge-ready\">READY</span>" +
            "            </div>" +
            "            <div class=\"diag-card-desc\">Scans maps, event picture references, audio tracks, and verifies project assets with deterministic probes and checkpoints.</div>" +
            "            <div style=\"display:flex; flex-wrap:wrap; gap:8px; margin-bottom:8px;\">" +
            "                <button id=\"diag-btn-probe-sampled\" class=\"diag-btn diag-btn-action\" style=\"flex:1; min-width:130px;\">" + SVG_ICONS.radar + " Run Sampled Probe</button>" +
            "                <button id=\"diag-btn-probe-exhaustive\" class=\"diag-btn diag-btn-action\" style=\"flex:1; min-width:130px;\">" + SVG_ICONS.play + " Run Exhaustive Probe</button>" +
            "                <button id=\"diag-btn-stress\" class=\"diag-btn diag-btn-primary\" style=\"flex:1; min-width:130px;\">" + SVG_ICONS.play + " Run Full Stress Test</button>" +
            "            </div>" +
            "            <div id=\"diag-probe-counters\" class=\"diag-counters-row\">" +
            "                <span>Scanned: <strong id=\"diag-cnt-scanned\">0</strong></span> | " +
            "                <span>Valid: <strong id=\"diag-cnt-valid\">0</strong></span> | " +
            "                <span>Missing: <strong id=\"diag-cnt-missing\">0</strong></span> | " +
            "                <span>Fallback: <strong id=\"diag-cnt-fallback\">0</strong></span>" +
            "            </div>" +
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
        modal.addEventListener("pointerdown", function (e) {
            if (typeof globalObj.bringPanelToFront === "function") {
                globalObj.bringPanelToFront(modal);
            } else {
                elevatePanel(modal);
            }
            e.stopPropagation();
        });
        modal.addEventListener("mousedown", function () {
            if (typeof globalObj.bringPanelToFront === "function") {
                globalObj.bringPanelToFront(modal);
            } else {
                elevatePanel(modal);
            }
        });
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
                if (typeof globalObj.bringPanelToFront === "function") {
                    globalObj.bringPanelToFront(modal);
                } else {
                    elevatePanel(modal);
                }
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

        var probeSampledBtn = doc.getElementById("diag-btn-probe-sampled");
        if (probeSampledBtn) {
            attachTap(probeSampledBtn, function () { DiagnosticSuite.runProbe("sampled"); });
        }

        var probeExhaustiveBtn = doc.getElementById("diag-btn-probe-exhaustive");
        if (probeExhaustiveBtn) {
            attachTap(probeExhaustiveBtn, function () { DiagnosticSuite.runProbe("exhaustive"); });
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
                if (typeof globalObj.bringPanelToFront === "function") {
                    globalObj.bringPanelToFront(modal);
                } else if (typeof globalObj.elevatePanel === "function") {
                    globalObj.elevatePanel(modal);
                } else {
                    elevatePanel(modal);
                }
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
        runProbe: function (mode) {
            var actionName = (mode === "exhaustive") ? "run_exhaustive_probe" : "run_probe";
            this.executeAuditOrProbe(mode || "sampled", actionName);
        },
        runStressTest: function () {
            this.executeAuditOrProbe("exhaustive", "run_stress_test");
        },
        executeAuditOrProbe: function (mode, actionName) {
            if (isStressTestRunning) return;
            isStressTestRunning = true;

            var probeMode = mode || "sampled";
            var resolvedAction = actionName || ((probeMode === "exhaustive") ? "run_exhaustive_probe" : "run_probe");
            var maxPollCount = (resolvedAction === "run_exhaustive_probe" || resolvedAction === "run_stress_test" || probeMode === "exhaustive") ? 260 : 60;
            var timeoutSec = Math.round(maxPollCount * 0.25);

            var stStatus = doc ? doc.getElementById("diag-stress-status") : null;
            var badge = doc ? doc.getElementById("diag-step-badge") : null;
            var pbar = doc ? doc.getElementById("diag-pbar-fill") : null;

            var initMsg = (probeMode === "sampled")
                ? "Initializing Sampled Asset Probe..."
                : (resolvedAction === "run_stress_test" ? "Initializing Asset Graph & Database Audit..." : "Initializing Exhaustive Asset Probe...");

            if (stStatus) stStatus.textContent = initMsg;
            if (badge) {
                setBadgeState(badge, "SCANNING", "diag-badge-scanning");
            }
            if (pbar) pbar.style.width = "10%";

            addLogEntry("PROGRESS", initMsg);
            dispatchCommand({ action: resolvedAction, mode: probeMode });

            if (globalObj.RuntimePackVPad && typeof globalObj.RuntimePackVPad.startAudit === "function") {
                globalObj.RuntimePackVPad.startAudit(JSON.stringify({ mode: probeMode }));
            } else if (globalObj.RuntimePackWeb && typeof globalObj.RuntimePackWeb.startAudit === "function") {
                globalObj.RuntimePackWeb.startAudit(JSON.stringify({ mode: probeMode }));
            }

            var pollCount = 0;
            var lastSeenLine = "";
            var lastEventSeq = 0;
            var failureCount = 0;
            var totalPassedCount = 0;

            var scannedCount = 0;
            var validCount = 0;
            var missingCount = 0;
            var fallbackCount = 0;

            function updateCountersUI() {
                if (!doc) return;
                var elScanned = doc.getElementById("diag-cnt-scanned");
                var elValid = doc.getElementById("diag-cnt-valid");
                var elMissing = doc.getElementById("diag-cnt-missing");
                var elFallback = doc.getElementById("diag-cnt-fallback");
                if (elScanned) elScanned.textContent = String(scannedCount);
                if (elValid) elValid.textContent = String(validCount);
                if (elMissing) elMissing.textContent = String(missingCount);
                if (elFallback) elFallback.textContent = String(fallbackCount);
            }
            updateCountersUI();

            var STAGE_LABELS = {
                "REFERENCE_FOUND": "Reference Found",
                "PATH_RESOLVED": "Path Resolved",
                "FILE_EXISTS": "VFS File Exists",
                "FILE_READABLE": "File Readable",
                "INTEGRITY_VERIFIED": "Integrity Verified",
                "DECODE_SUCCESS": "Decode Succeeded",
                "ENGINE_LOAD_SUCCESS": "Engine Load Succeeded",
                "RUNTIME_USE_SUCCESS": "Runtime Use Verified"
            };

            if (auditPollInterval) clearInterval(auditPollInterval);

            auditPollInterval = setInterval(function () {
                pollCount++;
                var rawEvents = null;
                var rawProgress = "";

                if (globalObj.RuntimePackVPad && typeof globalObj.RuntimePackVPad.getAuditEvents === "function") {
                    rawEvents = globalObj.RuntimePackVPad.getAuditEvents(lastEventSeq);
                } else if (globalObj.RuntimePackWeb && typeof globalObj.RuntimePackWeb.getAuditEvents === "function") {
                    rawEvents = globalObj.RuntimePackWeb.getAuditEvents(lastEventSeq);
                }

                if (!rawEvents || rawEvents === "[]" || rawEvents.length === 0) {
                    if (globalObj.RuntimePackVPad && typeof globalObj.RuntimePackVPad.getAuditProgress === "function") {
                        rawProgress = globalObj.RuntimePackVPad.getAuditProgress() || "";
                    } else if (globalObj.RuntimePackWeb && typeof globalObj.RuntimePackWeb.getAuditProgress === "function") {
                        rawProgress = globalObj.RuntimePackWeb.getAuditProgress() || "";
                    }
                }

                function handleEventItem(ev) {
                    var seq = ev.sequence_id || ev.sequenceId || 0;
                    if (seq > lastEventSeq) lastEventSeq = seq;

                    var evType = (ev.type || ev.event || ev.stage || "").toUpperCase();
                    var evOutcome = (ev.outcome || ev.status || "").toUpperCase();
                    var evMsg = ev.message || ev.error_message || ev.errorMessage || ev.text || "";
                    var targetPath = ev.asset_path || ev.assetPath || "";
                    var fallbackAct = ev.fallback_action || ev.fallbackAction || "";

                    var isFallback = (!!fallbackAct);
                    var isMissing = !isFallback && (ev.stage === "FILE_EXISTS" && (evOutcome === "FAIL" || evOutcome === "FAILED" || (evMsg && evMsg.toLowerCase().indexOf("not found") !== -1)));
                    var isFailure = (evOutcome === "FAIL" || evOutcome === "FAILED" || isFallback || evType === "FAIL" || evType === "ERROR");
                    var isDone = (evType === "DONE" || ev.is_complete === true || ev.isComplete === true);
                    var isSynthetic = (targetPath === "root" || targetPath === "all" || evType === "START");

                    if (isDone) {
                        var reportStr = null;
                        if (globalObj.RuntimePackVPad && typeof globalObj.RuntimePackVPad.getAuditReport === "function") {
                            reportStr = globalObj.RuntimePackVPad.getAuditReport();
                        } else if (globalObj.RuntimePackWeb && typeof globalObj.RuntimePackWeb.getAuditReport === "function") {
                            reportStr = globalObj.RuntimePackWeb.getAuditReport();
                        }
                        if (reportStr && reportStr !== "{}" && reportStr.trim().length > 0) {
                            try {
                                var report = JSON.parse(reportStr);
                                if (report && report.stats) {
                                    var s = report.stats;
                                    var rExact = typeof s.exact_count === "number" ? s.exact_count : 0;
                                    var rCase = typeof s.case_mismatch_count === "number" ? s.case_mismatch_count : 0;
                                    var rMissing = typeof s.missing_count === "number" ? s.missing_count : 0;
                                    validCount = rExact;
                                    missingCount = rMissing;
                                    scannedCount = rExact + rCase + rMissing;
                                    failureCount = rMissing + rCase;
                                    updateCountersUI();
                                }
                            } catch (e) {
                            }
                        }

                        var doneMsg = evMsg || ((failureCount > 0 || missingCount > 0)
                            ? "Audit Complete: " + (missingCount > 0 ? missingCount + " missing asset(s)" : failureCount + " failure(s)") + " detected."
                            : "Audit Complete: All references verified.");
                        if (failureCount > 0 || missingCount > 0) {
                            addLogEntry("WARN", doneMsg);
                            if (stStatus) {
                                stStatus.textContent = missingCount > 0
                                    ? "Audit Complete with " + missingCount + " missing asset(s)!"
                                    : "Audit Complete with " + failureCount + " failure(s).";
                            }
                            if (badge) {
                                setBadgeState(badge, "FAILED", "diag-badge-error");
                            }
                        } else {
                            addLogEntry("SUCCESS", doneMsg);
                            if (stStatus) stStatus.textContent = "Audit Completed Successfully.";
                            if (badge) {
                                setBadgeState(badge, "COMPLETED", "diag-badge-success");
                            }
                        }
                        if (pbar) pbar.style.width = "100%";
                        isStressTestRunning = false;
                        clearInterval(auditPollInterval);
                        auditPollInterval = null;
                        return;
                    }

                    if (!isSynthetic && (ev.stage || targetPath)) {
                        scannedCount++;
                    }
                    if (!isSynthetic && isMissing) {
                        missingCount++;
                    }
                    if (!isSynthetic && isFallback) {
                        fallbackCount++;
                    }

                    if (isFailure) {
                        if (!isSynthetic) {
                            failureCount++;
                        }
                        var note = fallbackAct ? " [" + fallbackAct + "]" : "";
                        var desc = targetPath ? targetPath + note + (evMsg ? ": " + evMsg : "") : (evMsg || "Asset verification failure");
                        addLogEntry("ERROR", desc);
                        if (stStatus) {
                            stStatus.textContent = "Scanning... (" + failureCount + " failure" + (failureCount > 1 ? "s" : "") + " detected)";
                        }
                        if (badge) {
                            setBadgeState(badge, "ISSUES (" + failureCount + ")", "diag-badge-error");
                        }
                        updateCountersUI();
                        return;
                    }

                    if (evType === "START") {
                        addLogEntry("PROGRESS", evMsg || "Audit started");
                        if (pbar) pbar.style.width = (ev.pct ? ev.pct + "%" : "15%");
                    } else if (evType === "PROGRESS" || evType === "SCAN") {
                        addLogEntry("SCAN", evMsg || "Scanning project assets...");
                        if (stStatus && failureCount === 0) stStatus.textContent = evMsg || "Scanning project assets...";
                        if (ev.pct && pbar) pbar.style.width = ev.pct + "%";
                    } else if (evType === "STAGE" || evType === "AUDIT" || ev.stage) {
                        totalPassedCount++;
                        if (!isSynthetic) {
                            validCount++;
                        }
                        var stageDesc = STAGE_LABELS[ev.stage] || ev.stage || evType;
                        var displayMsg = targetPath ? stageDesc + ": " + targetPath : (evMsg || stageDesc);
                        addLogEntry("AUDIT", displayMsg);
                        if (stStatus && failureCount === 0) stStatus.textContent = displayMsg;
                        if (ev.pct && pbar) pbar.style.width = ev.pct + "%";
                    } else {
                        totalPassedCount++;
                        if (!isSynthetic) {
                            validCount++;
                        }
                        if (targetPath) {
                            var stageDesc2 = STAGE_LABELS[ev.stage] || ev.stage || "Verified";
                            addLogEntry("AUDIT", stageDesc2 + ": " + targetPath);
                        }
                    }
                    updateCountersUI();
                }

                if (rawEvents && rawEvents.trim().length > 0 && rawEvents.trim() !== "[]") {
                    try {
                        var parsedEvents = JSON.parse(rawEvents.trim());
                        var evList = Array.isArray(parsedEvents) ? parsedEvents : [parsedEvents];
                        evList.forEach(function (ev) {
                            handleEventItem(ev);
                        });
                    } catch (e) {
                    }
                }

                if (rawProgress && rawProgress.trim().length > 0) {
                    var trimmed = rawProgress.trim();
                    var isJson = (trimmed.charAt(0) === "{" || trimmed.charAt(0) === "[");

                    if (isJson) {
                        try {
                            var parsed = JSON.parse(trimmed);
                            var events = Array.isArray(parsed) ? parsed : [parsed];
                            events.forEach(function (ev) {
                                handleEventItem(ev);
                            });
                        } catch (e) {
                            isJson = false;
                        }
                    }

                    if (!isJson) {
                        var lines = trimmed.split("\n").map(function(s) { return s.trim(); }).filter(function(s) { return s.length > 0; });
                        lines.forEach(function (line) {
                            if (line === lastSeenLine) return;
                            lastSeenLine = line;

                            if (line.indexOf("START:") === 0) {
                                addLogEntry("PROGRESS", line.replace(/^START:\s*/, ""));
                                if (pbar) pbar.style.width = "15%";
                            } else if (line.indexOf("PROGRESS:") === 0) {
                                var progText = line.replace(/^PROGRESS:\s*/, "");
                                addLogEntry("SCAN", progText);
                                if (stStatus && failureCount === 0) stStatus.textContent = progText;
                            } else if (line.indexOf("STAGE:") === 0) {
                                scannedCount++;
                                validCount++;
                                updateCountersUI();
                                var stageText = line.replace(/^STAGE:\s*/, "");
                                addLogEntry("AUDIT", stageText);
                                if (stStatus && failureCount === 0) stStatus.textContent = stageText;
                            } else if (line.indexOf("MAP:") === 0) {
                                var mapText = line.replace(/^MAP:\s*/, "");
                                addLogEntry("SCAN", mapText);
                                if (stStatus && failureCount === 0) stStatus.textContent = mapText;
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
                                if (stStatus && failureCount === 0) stStatus.textContent = dbText;
                                if (pbar) pbar.style.width = "85%";
                            } else if (line.indexOf("FAIL:") === 0 || line.indexOf("ERROR:") === 0) {
                                failureCount++;
                                missingCount++;
                                scannedCount++;
                                updateCountersUI();
                                var errText = line.replace(/^(FAIL|ERROR):\s*/, "");
                                addLogEntry("ERROR", errText);
                                if (stStatus) {
                                    stStatus.textContent = "Scanning... (" + failureCount + " failure" + (failureCount > 1 ? "s" : "") + " detected)";
                                }
                                if (badge) {
                                    setBadgeState(badge, "ISSUES (" + failureCount + ")", "diag-badge-error");
                                }
                            } else if (line.indexOf("DONE:") === 0) {
                                var doneText = line.replace(/^DONE:\s*/, "");
                                var m = line.match(/(\d+)\s+maps.*?(\d+)\s+exact assets.*?(\d+)\s+case mismatches.*?(\d+)\s+missing/i);
                                if (m) {
                                    var mapsCnt = parseInt(m[1], 10);
                                    var exactCnt = parseInt(m[2], 10);
                                    var caseCnt = parseInt(m[3], 10);
                                    var missingCnt = parseInt(m[4], 10);
                                    validCount = exactCnt;
                                    missingCount = missingCnt;
                                    scannedCount = exactCnt + caseCnt + missingCnt;
                                    failureCount = missingCnt + caseCnt;
                                } else {
                                    var reportStr = null;
                                    if (globalObj.RuntimePackVPad && typeof globalObj.RuntimePackVPad.getAuditReport === "function") {
                                        reportStr = globalObj.RuntimePackVPad.getAuditReport();
                                    } else if (globalObj.RuntimePackWeb && typeof globalObj.RuntimePackWeb.getAuditReport === "function") {
                                        reportStr = globalObj.RuntimePackWeb.getAuditReport();
                                    }
                                    if (reportStr && reportStr !== "{}" && reportStr.trim().length > 0) {
                                        try {
                                            var report = JSON.parse(reportStr);
                                            if (report && report.stats) {
                                                var s = report.stats;
                                                var rExact = typeof s.exact_count === "number" ? s.exact_count : 0;
                                                var rCase = typeof s.case_mismatch_count === "number" ? s.case_mismatch_count : 0;
                                                var rMissing = typeof s.missing_count === "number" ? s.missing_count : 0;
                                                validCount = rExact;
                                                missingCount = rMissing;
                                                scannedCount = rExact + rCase + rMissing;
                                                failureCount = rMissing + rCase;
                                            }
                                        } catch (e) {
                                        }
                                    }
                                }
                                updateCountersUI();

                                if (failureCount > 0 || missingCount > 0) {
                                    if (badge) {
                                        setBadgeState(badge, "FAILED", "diag-badge-error");
                                    }
                                    if (stStatus) {
                                        stStatus.textContent = missingCount > 0
                                            ? "Audit Complete with " + missingCount + " missing asset(s)!"
                                            : "Audit Complete with " + failureCount + " failure(s).";
                                    }
                                    var warnPrefix = missingCount > 0
                                        ? "Audit Complete: " + missingCount + " missing asset(s) detected. "
                                        : "Audit Complete: " + failureCount + " failure(s) detected. ";
                                    addLogEntry("WARN", warnPrefix + doneText);
                                } else {
                                    if (badge) {
                                        setBadgeState(badge, "COMPLETED", "diag-badge-success");
                                    }
                                    if (stStatus) {
                                        stStatus.textContent = "Audit Completed Successfully.";
                                    }
                                    addLogEntry("SUCCESS", doneText);
                                }
                                if (pbar) pbar.style.width = "100%";
                                isStressTestRunning = false;
                                clearInterval(auditPollInterval);
                                auditPollInterval = null;
                            }
                        });
                    }
                }

                if (pollCount > maxPollCount) {
                    isStressTestRunning = false;
                    clearInterval(auditPollInterval);
                    auditPollInterval = null;
                    if (pbar) pbar.style.width = "100%";
                    if (badge && badge.textContent !== "COMPLETED" && badge.textContent !== "FAILED") {
                        setBadgeState(badge, failureCount > 0 ? "FAILED" : "TIMEOUT", failureCount > 0 ? "diag-badge-error" : "diag-badge-warn");
                    }
                    if (stStatus && stStatus.textContent !== "Audit Completed Successfully.") {
                        stStatus.textContent = failureCount > 0
                            ? "Audit timed out with " + failureCount + " failure(s). Status: FAIL"
                            : "Audit timed out. Status: INDETERMINATE";
                    }
                    addLogEntry("WARN", "Audit timed out after " + timeoutSec + "s with no completion signal. Status: " + (failureCount > 0 ? "FAIL" : "INDETERMINATE"));
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

            // Combined Validation Summary extraction
            var combinedSummary = null;
            if (globalObj.RuntimePackVPad && typeof globalObj.RuntimePackVPad.getCombinedSummary === "function") {
                try {
                    var rawSummary = globalObj.RuntimePackVPad.getCombinedSummary();
                    if (rawSummary && rawSummary !== "{}") combinedSummary = JSON.parse(rawSummary);
                } catch (err) {}
            } else if (globalObj.RuntimePackWeb && typeof globalObj.RuntimePackWeb.getCombinedSummary === "function") {
                try {
                    var rawSummary = globalObj.RuntimePackWeb.getCombinedSummary();
                    if (rawSummary && rawSummary !== "{}") combinedSummary = JSON.parse(rawSummary);
                } catch (err) {}
            }

            var reportPayload = {
                timestamp: new Date().toISOString(),
                platform: (typeof navigator !== "undefined" ? navigator.userAgent : "Android"),
                url: (typeof window !== "undefined" && window.location ? window.location.href : ""),
                engine_family: engineFamily,
                device_telemetry: deviceTelemetry,
                game_state: gameState,
                combined_validation_summary: combinedSummary,
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
