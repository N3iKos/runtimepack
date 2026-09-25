/**
 * In-Game Cheat Overlay Suite (M-Tool & JoiPlay Style)
 * Dynamic Engine Variables & Switches Browser, Item Spawner & Quick Cheats,
 * M-Tool Interactive Script Console & LocalStorage Snippet Manager.
 * Supports 2 Themes: Clean Slate & Retro OSD.
 */

(function () {
    "use strict";

    const globalObj = typeof window !== "undefined" ? window : (typeof global !== "undefined" ? global : this);

    // State Variables
    let godModeActive = false;
    let noclipActive = false;
    let freezeEnemiesActive = false;
    let currentSpeed = 4;
    let activeTab = "variables";
    let currentViewMode = "engine"; // "engine" | "menu"
    let activeProfileSecIdx = 0;
    const profileToggles = {};
    let isMenuOpen = false;
    let currentVarFilter = "";
    let currentSwitchFilter = "";

    // Safely retrieve CheatEngine
    function getCheatEngine() {
        if (typeof globalObj.CheatEngine !== "undefined") return globalObj.CheatEngine;
        if (typeof globalObj.__cheatEngine !== "undefined") return globalObj.__cheatEngine;
        return null;
    }

    // Safely query state dump
    function callDumpState() {
        const ce = getCheatEngine();
        if (!ce) return false;
        if (typeof ce.dumpState === "function") return ce.dumpState();
        if (typeof ce.requestStateDump === "function") return ce.requestStateDump();
        return false;
    }

    // Safely query loaded state
    function getCheatState() {
        const ce = getCheatEngine();
        if (!ce) return { gold: 0, variables: {}, switches: {} };
        if (typeof ce.getState === "function") return ce.getState();
        if (typeof ce.getLoadedState === "function") return ce.getLoadedState();
        return { gold: 0, variables: {}, switches: {} };
    }

    // Variable and switch name resolvers
    function getVariableName(id) {
        if (typeof globalObj.$dataSystem !== "undefined" && Array.isArray(globalObj.$dataSystem.variables)) {
            const n = globalObj.$dataSystem.variables[id];
            if (n && String(n).trim().length > 0) return String(n).trim();
        }
        const state = getCheatState();
        if (state && state.variableNames && state.variableNames[id]) {
            return String(state.variableNames[id]);
        }
        return "Variable " + id;
    }

    function getSwitchName(id) {
        if (typeof globalObj.$dataSystem !== "undefined" && Array.isArray(globalObj.$dataSystem.switches)) {
            const n = globalObj.$dataSystem.switches[id];
            if (n && String(n).trim().length > 0) return String(n).trim();
        }
        const state = getCheatState();
        if (state && state.switchNames && state.switchNames[id]) {
            return String(state.switchNames[id]);
        }
        return "Switch " + id;
    }

    // Helper: Escape HTML
    function escapeHtml(str) {
        if (str === undefined || str === null) return "";
        return String(str)
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/"/g, "&quot;");
    }

    // Hook RPG Maker MV TouchInput agar interaksi di overlay tidak diblokir
    function hookTouchInput(ti) {
        if (!ti || ti._cheatHooked) return;
        ti._cheatHooked = true;
        const origTouchStart = ti._onTouchStart;
        ti._onTouchStart = function (event) {
            if (event.target && event.target.closest && (
                event.target.closest("#fnh-cheat-overlay") ||
                event.target.closest("#vpad-btn-cheat") ||
                event.target.closest(".cheat-interactive")
            )) {
                return;
            }
            if (origTouchStart) origTouchStart.call(this, event);
        };
    }
    if (typeof globalObj.TouchInput !== "undefined") {
        hookTouchInput(globalObj.TouchInput);
    }

    // Hook Scene_Base.update untuk persistent God Mode & Speed
    function hookGameLoop() {
        if (typeof globalObj.Scene_Base !== "undefined" && !globalObj.Scene_Base.prototype._cheatHooked) {
            globalObj.Scene_Base.prototype._cheatHooked = true;
            const origUpdate = globalObj.Scene_Base.prototype.update;
            globalObj.Scene_Base.prototype.update = function () {
                origUpdate.call(this);

                if (godModeActive && typeof globalObj.$gameParty !== "undefined" && globalObj.$gameParty.allMembers) {
                    try {
                        const members = globalObj.$gameParty.allMembers();
                        for (let i = 0; i < members.length; i++) {
                            const actor = members[i];
                            if (actor.hp < actor.mhp) actor.setHp(actor.mhp);
                            if (actor.mp < actor.mmp) actor.setMp(actor.mmp);
                            actor.removeState(1);
                            [39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 4, 5, 17, 55, 56, 80, 81].forEach(st => {
                                if (actor.isStateAffected(st)) actor.removeState(st);
                            });
                        }
                    } catch (e) {}
                }

                if (noclipActive && typeof globalObj.$gamePlayer !== "undefined") {
                    if (!$gamePlayer.isThrough()) {
                        $gamePlayer.setThrough(true);
                    }
                }
            };
        }

        if (typeof globalObj.Game_Action !== "undefined" && !globalObj.Game_Action.prototype._cheatDamageHooked) {
            globalObj.Game_Action.prototype._cheatDamageHooked = true;
            const origExecuteDamage = globalObj.Game_Action.prototype.executeDamage;
            globalObj.Game_Action.prototype.executeDamage = function (target, value) {
                if (godModeActive && target && target.isActor && target.isActor()) {
                    value = 0;
                }
                origExecuteDamage.call(this, target, value);
            };
        }

        if (typeof globalObj.Game_Player !== "undefined" && !globalObj.Game_Player.prototype._cheatThroughHooked) {
            globalObj.Game_Player.prototype._cheatThroughHooked = true;
            const origIsThrough = globalObj.Game_Player.prototype.isThrough;
            globalObj.Game_Player.prototype.isThrough = function () {
                if (noclipActive) return true;
                return origIsThrough.call(this);
            };
        }

        if (typeof globalObj.Game_Event !== "undefined" && !globalObj.Game_Event.prototype._cheatChaseHooked) {
            globalObj.Game_Event.prototype._cheatChaseHooked = true;
            const origCanSee = globalObj.Game_Event.prototype.canSeePlayer;
            if (origCanSee) {
                globalObj.Game_Event.prototype.canSeePlayer = function () {
                    if (freezeEnemiesActive) return false;
                    return origCanSee.call(this);
                };
            }
        }
    }

    // CSS Styling
    function injectStyles() {
        if (typeof document === "undefined") return;
        if (document.getElementById("fnh-cheat-styles")) return;
        const style = document.createElement("style");
        style.id = "fnh-cheat-styles";
        style.textContent = `
            /* Overlay Container */
            #fnh-cheat-overlay {
                position: fixed;
                top: 50%; left: 50%;
                transform: translate(-50%, -50%);
                width: 92vw;
                max-width: 520px;
                max-height: 86vh;
                background: #141416;
                border: 1px solid #27272a;
                box-shadow: 0 16px 36px rgba(0, 0, 0, 0.85);
                border-radius: 10px;
                color: #f4f4f5;
                font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
                display: none;
                flex-direction: column;
                z-index: 100010;
                overflow: hidden;
                user-select: none;
                -webkit-user-select: none;
                pointer-events: auto !important;
                touch-action: none !important;
            }

            /* Header */
            #fnh-cheat-header {
                display: flex;
                align-items: center;
                justify-content: space-between;
                padding: 10px 14px;
                background: #18181b;
                border-bottom: 1px solid #27272a;
                cursor: grab;
                flex-shrink: 0;
                pointer-events: auto !important;
                touch-action: none !important;
            }
            .fnh-cheat-title {
                font-size: 13px;
                font-weight: 700;
                letter-spacing: 0.5px;
                color: #f4f4f5;
                display: flex;
                align-items: center;
                gap: 8px;
            }
            .fnh-cheat-title svg {
                width: 15px; height: 15px;
                stroke: #f59e0b;
                fill: none;
                stroke-width: 2;
                stroke-linecap: round;
                stroke-linejoin: round;
            }
            .fnh-cheat-close {
                width: 26px; height: 26px;
                display: flex; align-items: center; justify-content: center;
                border-radius: 6px;
                background: rgba(255, 255, 255, 0.06);
                border: 1px solid rgba(255, 255, 255, 0.12);
                color: #a1a1aa;
                cursor: pointer;
                font-size: 14px;
            }
            .fnh-cheat-close:active {
                background: rgba(239, 68, 68, 0.4);
                color: #fff;
            }

            /* Tab Bar */
            #fnh-cheat-tabs {
                display: flex;
                background: #09090b;
                border-bottom: 1px solid #27272a;
                overflow-x: auto;
                -webkit-overflow-scrolling: touch;
                touch-action: pan-x;
                scrollbar-width: none;
                flex-shrink: 0;
            }
            #fnh-cheat-tabs::-webkit-scrollbar { display: none; }
            .fnh-cheat-tab {
                flex: 1;
                flex-shrink: 0;
                min-width: 78px;
                padding: 10px 4px;
                text-align: center;
                font-size: 11px;
                font-weight: 600;
                color: #71717a;
                cursor: pointer;
                border-bottom: 2px solid transparent;
                transition: all 0.15s ease;
                white-space: nowrap;
                display: flex; align-items: center; justify-content: center; gap: 5px;
            }
            .fnh-cheat-tab svg {
                width: 14px; height: 14px;
                stroke: currentColor;
                fill: none;
                stroke-width: 2;
                stroke-linecap: round;
                stroke-linejoin: round;
            }
            .fnh-cheat-tab.active {
                color: #f4f4f5;
                border-bottom-color: #0088cc;
                background: rgba(0, 136, 204, 0.08);
            }

            /* Body */
            #fnh-cheat-body {
                flex: 1;
                overflow-y: auto;
                touch-action: pan-y;
                -webkit-overflow-scrolling: touch;
                padding: 10px 12px;
                display: flex;
                flex-direction: column;
                gap: 8px;
                min-height: 260px;
            }

            /* Tab Panes */
            .fnh-tab-pane {
                width: 100%;
                display: flex;
                flex-direction: column;
                gap: 8px;
            }

            /* Common Row */
            .fnh-cheat-row {
                background: #18181b;
                border: 1px solid #27272a;
                border-radius: 6px;
                padding: 8px 10px;
                display: flex;
                align-items: center;
                justify-content: space-between;
                gap: 10px;
            }
            .fnh-cheat-left {
                display: flex;
                align-items: center;
                gap: 8px;
                font-size: 12px;
                font-weight: 600;
                color: #e4e4e7;
                overflow: hidden;
                text-overflow: ellipsis;
                white-space: nowrap;
                flex: 1;
            }

            /* Form Elements */
            .fnh-input, .fnh-select, .fnh-textarea {
                background: #18181b;
                border: 1px solid #27272a;
                color: #f4f4f5;
                border-radius: 6px;
                padding: 6px 10px;
                font-size: 12px;
                box-sizing: border-box;
            }
            .fnh-input:focus, .fnh-select:focus, .fnh-textarea:focus {
                border-color: #0088cc;
                outline: none;
            }

            /* Buttons */
            .fnh-cheat-btn {
                padding: 6px 14px;
                border-radius: 5px;
                font-size: 11px;
                font-weight: 600;
                border: 1px solid #3f3f46;
                background: #27272a;
                color: #f4f4f5;
                cursor: pointer;
                transition: background 0.1s, transform 0.05s;
                white-space: nowrap;
                text-align: center;
            }
            .fnh-cheat-btn:active {
                transform: scale(0.95);
            }
            .fnh-btn-action {
                background: #1e3a8a;
                border-color: #2563eb;
                color: #dbeafe;
            }
            .fnh-btn-action:active {
                background: #1d4ed8;
            }
            .fnh-btn-toggle-off {
                background: #27272a;
                color: #71717a;
                border-color: #3f3f46;
                min-width: 60px;
            }
            .fnh-btn-toggle-on {
                background: #15803d;
                color: #dcfce7;
                border-color: #22c55e;
                min-width: 60px;
            }

            /* Cards Container */
            .fnh-cards-container {
                display: flex;
                flex-direction: column;
                gap: 6px;
                max-height: 52vh;
                overflow-y: auto;
                padding-right: 2px;
            }

            /* Console Terminal */
            .fnh-console-pre {
                background: #09090b;
                border: 1px solid #27272a;
                border-radius: 6px;
                padding: 8px 10px;
                color: #a1a1aa;
                font-family: Consolas, "Roboto Mono", "Courier New", monospace;
                font-size: 11px;
                max-height: 120px;
                overflow-y: auto;
                white-space: pre-wrap;
                word-break: break-all;
                margin: 0;
            }

            /* Toast Notification */
            #fnh-cheat-toast {
                position: fixed;
                bottom: 24px; left: 50%;
                transform: translateX(-50%) translateY(20px);
                background: #18181b;
                border: 1px solid #3f3f46;
                color: #f4f4f5;
                padding: 8px 16px;
                border-radius: 20px;
                font-size: 11px;
                font-weight: 600;
                box-shadow: 0 6px 20px rgba(0,0,0,0.8);
                pointer-events: none;
                opacity: 0;
                transition: all 0.2s ease-out;
                z-index: 100020;
                text-align: center;
            }
            #fnh-cheat-toast.show {
                transform: translateX(-50%) translateY(0);
                opacity: 1;
            }

            /* Mode Toggle & Profile Section */
            #btn-cheat-mode-toggle {
                padding: 3px 8px;
                font-size: 10px;
                font-weight: 700;
                background: #2563eb;
                color: #ffffff;
                border: 1px solid #3b82f6;
                border-radius: 4px;
                cursor: pointer;
            }
            .fnh-cat-nav {
                display: flex;
                gap: 6px;
                overflow-x: auto;
                padding-bottom: 6px;
                border-bottom: 1px solid #27272a;
                margin-bottom: 8px;
                -webkit-overflow-scrolling: touch;
                touch-action: pan-x;
            }
            .fnh-cat-btn {
                display: inline-flex;
                align-items: center;
                gap: 6px;
                padding: 6px 12px;
                border-radius: 16px;
                font-size: 11px;
                font-weight: 700;
                background: #1f1f23;
                border: 1px solid #3f3f46;
                color: #a1a1aa;
                cursor: pointer;
                white-space: nowrap;
                transition: all 0.15s ease;
                flex-shrink: 0;
            }
            .fnh-cat-btn svg {
                width: 13px;
                height: 13px;
                stroke: currentColor;
                fill: none;
                stroke-width: 2;
                stroke-linecap: round;
                stroke-linejoin: round;
            }
            .fnh-cat-btn.active {
                background: #2563eb;
                border-color: #3b82f6;
                color: #ffffff;
            }
            .fnh-profile-sec {
                display: flex;
                flex-direction: column;
                gap: 6px;
            }

            /* ================================================================
               THEME 2: RETRO OSD (ORANGE TERMINAL) OVERRIDES
               ================================================================ */
            body.vpad-theme-retro #fnh-cheat-overlay {
                background: #0a0a0c !important;
                border: 1.5px solid #ff7700 !important;
                border-radius: 2px !important;
                box-shadow: 0 0 20px rgba(255, 119, 0, 0.25) !important;
                font-family: Consolas, "Roboto Mono", "Courier New", monospace !important;
            }
            body.vpad-theme-retro #fnh-cheat-header {
                background: #141418 !important;
                border-bottom: 1px solid #ff7700 !important;
            }
            body.vpad-theme-retro .fnh-cheat-title {
                color: #ff7700 !important;
            }
            body.vpad-theme-retro .fnh-cheat-title svg {
                stroke: #ff7700 !important;
            }
            body.vpad-theme-retro .fnh-cheat-tab.active {
                color: #ff7700 !important;
                border-bottom-color: #ff7700 !important;
                background: rgba(255, 119, 0, 0.12) !important;
            }
            body.vpad-theme-retro .fnh-cheat-row {
                background: #0e0e12 !important;
                border-color: #333 !important;
                border-radius: 2px !important;
            }
            body.vpad-theme-retro .fnh-cheat-btn {
                border-radius: 2px !important;
                background: #16161c !important;
                border-color: #444 !important;
                font-family: Consolas, monospace !important;
            }
            body.vpad-theme-retro #btn-cheat-mode-toggle {
                background: #ff7700 !important;
                color: #000000 !important;
                border-color: #ffa244 !important;
            }
            body.vpad-theme-retro .fnh-cat-nav {
                border-bottom-color: #333 !important;
            }
            body.vpad-theme-retro .fnh-cat-btn {
                background: #141418 !important;
                border-color: #444 !important;
                color: #888 !important;
                border-radius: 2px !important;
                font-family: Consolas, monospace !important;
            }
            body.vpad-theme-retro .fnh-cat-btn.active {
                background: rgba(255, 119, 0, 0.25) !important;
                border-color: #ff7700 !important;
                color: #ffaa44 !important;
            }
            body.vpad-theme-retro .fnh-btn-action {
                background: rgba(255, 119, 0, 0.2) !important;
                border-color: #ff7700 !important;
                color: #ffaa44 !important;
            }
            body.vpad-theme-retro .fnh-btn-toggle-on {
                background: #ff7700 !important;
                color: #000 !important;
                border-color: #ffa244 !important;
                font-weight: bold !important;
            }
            body.vpad-theme-retro .fnh-input,
            body.vpad-theme-retro .fnh-select,
            body.vpad-theme-retro .fnh-textarea,
            body.vpad-theme-retro .fnh-var-input {
                background: #0e0e12 !important;
                border-color: #ff7700 !important;
                color: #ffaa44 !important;
                font-family: Consolas, monospace !important;
                border-radius: 2px !important;
            }
            body.vpad-theme-retro .fnh-console-pre {
                background: #09090b !important;
                border-color: #ff7700 !important;
                color: #ffaa44 !important;
                border-radius: 2px !important;
            }
            body.vpad-theme-retro #fnh-cheat-toast {
                background: #0a0a0c !important;
                border-color: #ff7700 !important;
                color: #ffaa44 !important;
                border-radius: 2px !important;
            }
        `;
        document.head.appendChild(style);
    }

    let toastTimer = null;
    function showToast(msg) {
        if (typeof document === "undefined") return;
        let toast = document.getElementById("fnh-cheat-toast");
        if (!toast) {
            toast = document.createElement("div");
            toast.id = "fnh-cheat-toast";
            document.body.appendChild(toast);
        }
        toast.textContent = msg;
        toast.classList.add("show");
        if (toastTimer) clearTimeout(toastTimer);
        toastTimer = setTimeout(() => {
            toast.classList.remove("show");
        }, 2200);
    }

    // SVG Helper Icons (Monochrome, Inline, 100% Offline)
    const ICONS = {
        variable: `<svg viewBox="0 0 24 24"><path d="M4 20h4l10.5 -10.5a2.828 2.828 0 1 0 -4 -4l-10.5 10.5v4"></path><path d="M13.5 6.5l4 4"></path></svg>`,
        switch: `<svg viewBox="0 0 24 24"><rect x="2" y="6" width="20" height="12" rx="6"></rect><circle cx="8" cy="12" r="3"></circle></svg>`,
        bag: `<svg viewBox="0 0 24 24"><path d="M6 2L3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4z"></path><line x1="3" y1="6" x2="21" y2="6"></line><path d="M16 10a4 4 0 0 1-8 0"></path></svg>`,
        terminal: `<svg viewBox="0 0 24 24"><polyline points="4 17 10 11 4 5"></polyline><line x1="12" y1="19" x2="20" y2="19"></line></svg>`
    };

    // ------------------------------------------------------------------------
    // TAB 1: VARIABLES BROWSER (#fnh-tab-variables)
    // ------------------------------------------------------------------------
    function renderVariables(filterText) {
        if (typeof document === "undefined") return;
        if (filterText !== undefined) {
            currentVarFilter = String(filterText).trim().toLowerCase();
        }
        const container = document.getElementById("fnh-var-list");
        if (!container) return;

        const state = getCheatState();
        const vars = (state && state.variables) ? state.variables : {};

        const idSet = new Set();
        for (let i = 1; i <= 100; i++) idSet.add(i);
        Object.keys(vars).forEach(k => {
            const n = parseInt(k, 10);
            if (!isNaN(n) && n > 0) idSet.add(n);
        });
        if (typeof globalObj.$dataSystem !== "undefined" && Array.isArray(globalObj.$dataSystem.variables)) {
            for (let i = 1; i < globalObj.$dataSystem.variables.length; i++) {
                if (globalObj.$dataSystem.variables[i]) idSet.add(i);
            }
        }
        if (/^\d+$/.test(currentVarFilter)) {
            idSet.add(parseInt(currentVarFilter, 10));
        }

        const sortedIds = Array.from(idSet).sort((a, b) => a - b);
        let html = "";
        let matchCount = 0;

        for (let i = 0; i < sortedIds.length; i++) {
            const id = sortedIds[i];
            const name = getVariableName(id);
            const val = vars[String(id)] !== undefined ? vars[String(id)] : (vars[id] !== undefined ? vars[id] : 0);

            if (currentVarFilter) {
                const idStr = String(id);
                const nameLower = name.toLowerCase();
                if (!idStr.includes(currentVarFilter) && !nameLower.includes(currentVarFilter)) {
                    continue;
                }
            }
            matchCount++;

            html += `
                <div class="fnh-var-card fnh-cheat-row" data-id="${id}" id="fnh-var-card-${id}">
                    <div class="fnh-cheat-left">
                        <span class="fnh-var-label" id="fnh-var-label-${id}">[#${id}] ${escapeHtml(name)} = ${escapeHtml(val)}</span>
                    </div>
                    <div class="fnh-var-controls" style="display:flex; gap:6px; align-items:center;">
                        <input type="text" class="fnh-var-input fnh-input" id="fnh-var-input-${id}" data-id="${id}" value="${escapeHtml(val)}" style="width:75px; padding:4px 6px; font-size:11px;">
                        <button class="fnh-cheat-btn fnh-btn-action fnh-var-set-btn" data-id="${id}" id="fnh-var-set-${id}">Set</button>
                    </div>
                </div>
            `;
        }

        if (matchCount === 0) {
            html = `<div style="text-align:center; padding:20px; color:#71717a; font-size:12px;">No variables found.</div>`;
        }

        container.innerHTML = html;
    }

    function applyVariableSet(id) {
        if (typeof document === "undefined") return;
        const input = document.getElementById("fnh-var-input-" + id);
        if (!input) return;
        const raw = input.value !== undefined ? String(input.value).trim() : "";
        let val = (!isNaN(Number(raw)) && raw !== "") ? Number(raw) : raw;

        const ce = getCheatEngine();
        if (ce && typeof ce.setVariable === "function") {
            ce.setVariable(id, val);
        }

        const state = getCheatState();
        if (state && state.variables) {
            state.variables[String(id)] = val;
        }

        const label = document.getElementById("fnh-var-label-" + id);
        const name = getVariableName(id);
        if (label) {
            label.textContent = `[#${id}] ${name} = ${val}`;
        }
        showToast(`Variable #${id} set to ${val}`);
    }

    // ------------------------------------------------------------------------
    // TAB 2: SWITCHES BROWSER (#fnh-tab-switches)
    // ------------------------------------------------------------------------
    function renderSwitches(filterText) {
        if (typeof document === "undefined") return;
        if (filterText !== undefined) {
            currentSwitchFilter = String(filterText).trim().toLowerCase();
        }
        const container = document.getElementById("fnh-switch-list");
        if (!container) return;

        const state = getCheatState();
        const switches = (state && state.switches) ? state.switches : {};

        const idSet = new Set();
        for (let i = 1; i <= 100; i++) idSet.add(i);
        Object.keys(switches).forEach(k => {
            const n = parseInt(k, 10);
            if (!isNaN(n) && n > 0) idSet.add(n);
        });
        if (typeof globalObj.$dataSystem !== "undefined" && Array.isArray(globalObj.$dataSystem.switches)) {
            for (let i = 1; i < globalObj.$dataSystem.switches.length; i++) {
                if (globalObj.$dataSystem.switches[i]) idSet.add(i);
            }
        }
        if (/^\d+$/.test(currentSwitchFilter)) {
            idSet.add(parseInt(currentSwitchFilter, 10));
        }

        const sortedIds = Array.from(idSet).sort((a, b) => a - b);
        let html = "";
        let matchCount = 0;

        for (let i = 0; i < sortedIds.length; i++) {
            const id = sortedIds[i];
            const name = getSwitchName(id);
            const isOn = Boolean(switches[String(id)] !== undefined ? switches[String(id)] : switches[id]);

            if (currentSwitchFilter) {
                const idStr = String(id);
                const nameLower = name.toLowerCase();
                if (!idStr.includes(currentSwitchFilter) && !nameLower.includes(currentSwitchFilter)) {
                    continue;
                }
            }
            matchCount++;

            html += `
                <div class="fnh-switch-card fnh-cheat-row" data-id="${id}" id="fnh-switch-card-${id}">
                    <div class="fnh-cheat-left">
                        <span class="fnh-switch-label" id="fnh-switch-label-${id}">[#${id}] ${escapeHtml(name)}</span>
                    </div>
                    <div class="fnh-switch-controls">
                        <button class="fnh-cheat-btn fnh-switch-toggle-btn ${isOn ? 'fnh-btn-toggle-on' : 'fnh-btn-toggle-off'}" data-id="${id}" id="fnh-switch-toggle-${id}">
                            ${isOn ? 'ON' : 'OFF'}
                        </button>
                    </div>
                </div>
            `;
        }

        if (matchCount === 0) {
            html = `<div style="text-align:center; padding:20px; color:#71717a; font-size:12px;">No switches found.</div>`;
        }

        container.innerHTML = html;
    }

    function toggleSwitch(id) {
        if (typeof document === "undefined") return;
        const btn = document.getElementById("fnh-switch-toggle-" + id);
        const state = getCheatState();
        const switches = (state && state.switches) ? state.switches : {};
        const currentOn = Boolean(switches[String(id)] !== undefined ? switches[String(id)] : switches[id]);
        const nextOn = !currentOn;

        const ce = getCheatEngine();
        if (ce && typeof ce.setSwitch === "function") {
            ce.setSwitch(id, nextOn);
        }

        if (state && state.switches) {
            state.switches[String(id)] = nextOn;
        }

        if (btn) {
            btn.textContent = nextOn ? "ON" : "OFF";
            btn.className = "fnh-cheat-btn fnh-switch-toggle-btn " + (nextOn ? "fnh-btn-toggle-on" : "fnh-btn-toggle-off");
        }
        showToast(`Switch #${id} -> ${nextOn ? "ON" : "OFF"}`);
    }

    // ------------------------------------------------------------------------
    // TAB 3: ITEM SPAWNER & QUICK CHEATS (#fnh-tab-spawner)
    // ------------------------------------------------------------------------
    function maxGold() {
        const ce = getCheatEngine();
        if (ce && typeof ce.gainGold === "function") {
            ce.gainGold(999999);
        }
        showToast("+999,999 Gold added!");
    }

    function recoverAll() {
        const ce = getCheatEngine();
        if (ce && typeof ce.recoverAll === "function") {
            ce.recoverAll();
        }
        showToast("Party fully recovered!");
    }

    function spawnItem(category, id, count) {
        const cat = category || (document.getElementById("fnh-spawner-category") ? document.getElementById("fnh-spawner-category").value : "item");
        const numId = id !== undefined ? Number(id) : (document.getElementById("fnh-spawner-id") ? Number(document.getElementById("fnh-spawner-id").value) : 1);
        const numCount = count !== undefined ? Number(count) : (document.getElementById("fnh-spawner-count") ? Number(document.getElementById("fnh-spawner-count").value) : 1);

        const ce = getCheatEngine();
        if (ce && typeof ce.gainItem === "function") {
            ce.gainItem(cat, numId, numCount);
        }
        showToast(`Spawned ${numCount}x ${cat} #${numId}`);
    }

    // ------------------------------------------------------------------------
    // TAB 4: M-TOOL SCRIPT CONSOLE & SNIPPET MANAGER (#fnh-tab-console)
    // ------------------------------------------------------------------------
    function runScript() {
        if (typeof document === "undefined") return;
        const inputEl = document.getElementById("fnh-console-input");
        const outputEl = document.getElementById("fnh-console-output");
        if (!inputEl) return;
        const code = inputEl.value;
        if (!code || !code.trim()) {
            if (outputEl) outputEl.textContent = "(empty script)";
            return;
        }
        if (outputEl) outputEl.textContent = "Executing...";
        const ce = getCheatEngine();
        if (ce && typeof ce.evalScript === "function") {
            ce.evalScript(code, function (result) {
                if (outputEl) {
                    outputEl.textContent = (result !== undefined && result !== null) ? String(result) : "(nil)";
                }
            });
        } else {
            if (outputEl) outputEl.textContent = "[CHT-001] CheatEngine.evalScript is unavailable.";
        }
    }

    function getSavedSnippets() {
        try {
            const ls = (typeof localStorage !== "undefined") ? localStorage : (typeof globalObj.localStorage !== "undefined" ? globalObj.localStorage : null);
            if (ls && typeof ls.getItem === "function") {
                const raw = ls.getItem("vpad_user_cheat_snippets");
                if (raw) {
                    const parsed = JSON.parse(raw);
                    if (Array.isArray(parsed)) return parsed;
                }
            }
        } catch (e) {
            console.warn("Failed to get saved snippets:", e);
        }
        return [];
    }

    function saveSnippet(name, code) {
        try {
            const ls = (typeof localStorage !== "undefined") ? localStorage : (typeof globalObj.localStorage !== "undefined" ? globalObj.localStorage : null);
            if (ls) {
                let snippets = getSavedSnippets();
                const existingIdx = snippets.findIndex(s => s.name === name);
                if (existingIdx >= 0) {
                    snippets[existingIdx].code = code;
                } else {
                    snippets.push({ name: name, code: code });
                }
                ls.setItem("vpad_user_cheat_snippets", JSON.stringify(snippets));
                refreshSnippetSelect();
                return true;
            }
        } catch (e) {
            console.warn("Failed to save snippet:", e);
        }
        return false;
    }

    function deleteSnippet(name) {
        try {
            const ls = (typeof localStorage !== "undefined") ? localStorage : (typeof globalObj.localStorage !== "undefined" ? globalObj.localStorage : null);
            if (ls) {
                let snippets = getSavedSnippets();
                snippets = snippets.filter(s => s.name !== name);
                ls.setItem("vpad_user_cheat_snippets", JSON.stringify(snippets));
                refreshSnippetSelect();
                return true;
            }
        } catch (e) {
            console.warn("Failed to delete snippet:", e);
        }
        return false;
    }

    function refreshSnippetSelect() {
        if (typeof document === "undefined") return;
        const select = document.getElementById("fnh-snippet-select");
        if (!select) return;
        const snippets = getSavedSnippets();
        let html = '<option value="">-- Select Saved Snippet --</option>';
        snippets.forEach(s => {
            html += `<option value="${escapeHtml(s.name)}">${escapeHtml(s.name)}</option>`;
        });
        select.innerHTML = html;
    }

    function handleSaveSnippet() {
        if (typeof document === "undefined") return;
        const nameEl = document.getElementById("fnh-snippet-name");
        const codeEl = document.getElementById("fnh-console-input");
        const name = nameEl ? nameEl.value.trim() : "";
        const code = codeEl ? codeEl.value : "";
        if (!name) {
            showToast("Please enter a snippet name!");
            return;
        }
        if (!code || !code.trim()) {
            showToast("Please enter script code to save!");
            return;
        }
        saveSnippet(name, code);
        showToast(`Snippet '${name}' saved!`);
    }

    function handleLoadSnippet() {
        if (typeof document === "undefined") return;
        const select = document.getElementById("fnh-snippet-select");
        const selectedName = select ? select.value : "";
        if (!selectedName) {
            showToast("No snippet selected!");
            return;
        }
        const snippets = getSavedSnippets();
        const found = snippets.find(s => s.name === selectedName);
        if (found) {
            const inputEl = document.getElementById("fnh-console-input");
            const nameEl = document.getElementById("fnh-snippet-name");
            if (inputEl) inputEl.value = found.code;
            if (nameEl) nameEl.value = found.name;
            showToast(`Loaded snippet '${found.name}'`);
        }
    }

    function handleDeleteSnippet() {
        if (typeof document === "undefined") return;
        const select = document.getElementById("fnh-snippet-select");
        const selectedName = select ? select.value : "";
        if (!selectedName) {
            showToast("No snippet selected to delete!");
            return;
        }
        deleteSnippet(selectedName);
        showToast(`Deleted snippet '${selectedName}'`);
    }

    // ------------------------------------------------------------------------
    // CHEAT MENU PROFILE (READY-TO-USE PRESETS FOR AI AGENT / DEVELOPER)
    // ------------------------------------------------------------------------
    function renderCheatMenuProfile() {
        if (typeof document === "undefined") return;
        const container = document.getElementById("fnh-tab-profile-menu");
        if (!container) return;

        const profile = globalObj.GAME_CHEATS;
        if (!profile || !profile.sections || !profile.sections.length) {
            container.innerHTML = `
                <div style="text-align:center; padding:30px 15px; color:#71717a;">
                    <div style="font-size:13px; font-weight:700; color:#e4e4e7; margin-bottom:6px;">Belum Ada Cheat Siap Pakai</div>
                    <div style="font-size:11px; line-height:1.5;">
                        Konfigurasi cheat siap pakai dapat diatur melalui <code>overlay/cheat_profile.js</code>.
                    </div>
                </div>
            `;
            return;
        }

        if (activeProfileSecIdx >= profile.sections.length) {
            activeProfileSecIdx = 0;
        }

        const state = getCheatState() || {};
        const switches = state.switches || {};
        const variables = state.variables || {};

        // 1. Horizontal Category Nav Bar
        let navHtml = `<div class="fnh-cat-nav">`;
        profile.sections.forEach((sec, sIdx) => {
            const isActive = sIdx === activeProfileSecIdx;
            navHtml += `
                <button class="fnh-cat-btn ${isActive ? "active" : ""}" data-idx="${sIdx}">
                    ${sec.icon || ""} <span>${escapeHtml(sec.name || "Category")}</span>
                </button>
            `;
        });
        navHtml += `</div>`;

        // 2. Active Category Content (Only the selected section!)
        const curSec = profile.sections[activeProfileSecIdx];
        let itemsHtml = `<div class="fnh-profile-sec">`;

        (curSec.items || []).forEach((item, iIdx) => {
            const itemId = `fnh-prof-${activeProfileSecIdx}-${iIdx}`;
            if (item.type === "toggle") {
                let isOn = false;
                if (item.key && state[item.key] !== undefined) {
                    isOn = Boolean(state[item.key]);
                } else if (profileToggles[item.id] !== undefined) {
                    isOn = Boolean(profileToggles[item.id]);
                }
                itemsHtml += `
                    <div class="fnh-cheat-row">
                        <div class="fnh-cheat-left">
                            <span>${escapeHtml(item.name || "Toggle")}</span>
                        </div>
                        <button class="fnh-cheat-btn ${isOn ? "fnh-btn-toggle-on" : ""} fnh-prof-toggle-btn" data-sec="${activeProfileSecIdx}" data-idx="${iIdx}" style="min-width:60px; padding:6px 12px; font-size:11px;">
                            ${isOn ? "ON" : "OFF"}
                        </button>
                    </div>
                `;
            } else if (item.type === "action" || item.type === "script") {
                itemsHtml += `
                    <div class="fnh-cheat-row">
                        <div class="fnh-cheat-left">
                            <span>${escapeHtml(item.name || "Action")}</span>
                        </div>
                        <button class="fnh-cheat-btn fnh-btn-action fnh-prof-action-btn" data-sec="${activeProfileSecIdx}" data-idx="${iIdx}" style="padding:6px 14px; font-size:11px;">
                            Execute
                        </button>
                    </div>
                `;
            } else if (item.type === "switch") {
                const sId = item.switchId;
                const isOn = Boolean(switches[String(sId)] !== undefined ? switches[String(sId)] : switches[sId]);
                itemsHtml += `
                    <div class="fnh-cheat-row">
                        <div class="fnh-cheat-left">
                            <span>${escapeHtml(item.name || "Switch #" + sId)}</span>
                        </div>
                        <button class="fnh-cheat-btn ${isOn ? "fnh-btn-toggle-on" : ""} fnh-prof-switch-btn" data-sec="${activeProfileSecIdx}" data-idx="${iIdx}" data-sid="${sId}" style="min-width:60px; padding:6px 12px; font-size:11px;">
                            ${isOn ? "ON" : "OFF"}
                        </button>
                    </div>
                `;
            } else if (item.type === "variable") {
                const vId = item.variableId;
                const currentVal = variables[String(vId)] !== undefined ? variables[String(vId)] : (variables[vId] !== undefined ? variables[vId] : 0);
                itemsHtml += `
                    <div class="fnh-cheat-row">
                        <div class="fnh-cheat-left">
                            <span>${escapeHtml(item.name || "Variable #" + vId)}</span>
                        </div>
                        <div style="display:flex; gap:6px; align-items:center;">
                            <input type="number" class="fnh-input fnh-prof-var-input" id="${itemId}-input" value="${currentVal}" min="${item.min !== undefined ? item.min : 0}" max="${item.max !== undefined ? item.max : 999999}" step="${item.step || 1}" style="width:70px; padding:4px 6px; font-size:11px;">
                            <button class="fnh-cheat-btn fnh-btn-action fnh-prof-var-btn" data-sec="${activeProfileSecIdx}" data-idx="${iIdx}" data-vid="${vId}" data-input-id="${itemId}-input" style="padding:6px 10px; font-size:11px;">Set</button>
                        </div>
                    </div>
                `;
            }
        });

        itemsHtml += `</div>`;
        container.innerHTML = navHtml + itemsHtml;
    }

    function toggleCheatMode() {
        if (currentViewMode === "engine") {
            setCheatMode("menu");
        } else {
            setCheatMode("engine");
        }
    }

    function setCheatMode(mode) {
        currentViewMode = mode;
        const btn = document.getElementById("btn-cheat-mode-toggle");
        const tabs = document.getElementById("fnh-cheat-tabs");
        const titleSpan = document.querySelector("#fnh-cheat-header .fnh-cheat-title span");
        const profilePane = document.getElementById("fnh-tab-profile-menu");
        const validTabs = ["variables", "switches", "spawner", "console"];

        if (mode === "menu") {
            if (btn) {
                btn.textContent = "Cheat Engine";
                btn.style.background = "#4f46e5";
                btn.style.borderColor = "#6366f1";
            }
            if (titleSpan) {
                titleSpan.textContent = (globalObj.GAME_CHEATS && globalObj.GAME_CHEATS.title) ? globalObj.GAME_CHEATS.title.toUpperCase() : "CHEAT MENU";
            }
            if (tabs) tabs.style.display = "none";
            validTabs.forEach(t => {
                const p = document.getElementById("fnh-tab-" + t);
                if (p) p.style.display = "none";
            });
            if (profilePane) {
                profilePane.style.display = "flex";
                callDumpState();
                getCheatState();
                renderCheatMenuProfile();
            }
        } else {
            if (btn) {
                btn.textContent = "Cheat Menu";
                btn.style.background = "#2563eb";
                btn.style.borderColor = "#3b82f6";
            }
            if (titleSpan) titleSpan.textContent = "CHEAT ENGINE";
            if (tabs) tabs.style.display = "flex";
            if (profilePane) profilePane.style.display = "none";
            switchTab(activeTab || "variables");
        }
    }

    // ------------------------------------------------------------------------
    // TAB NAVIGATION & MODAL CONTROLS
    // ------------------------------------------------------------------------
    function switchTab(tabId) {
        if (typeof document === "undefined" || !tabId) return;
        const modal = document.getElementById("fnh-cheat-overlay");
        if (!modal) return;

        let key = tabId.replace(/^#?fnh-tab-|^tab-/, "").toLowerCase();
        const validTabs = ["variables", "switches", "spawner", "console"];
        if (!validTabs.includes(key)) {
            key = "variables";
        }

        activeTab = key;

        // Update Tab Header Buttons
        const tabs = modal.querySelectorAll(".fnh-cheat-tab");
        if (tabs && tabs.forEach) {
            tabs.forEach(t => {
                const bTab = (t.getAttribute("data-tab") || "").replace(/^#?fnh-tab-|^tab-/, "").toLowerCase();
                t.classList.toggle("active", bTab === key);
            });
        }

        // Toggle Tab Panes
        validTabs.forEach(t => {
            const pane = document.getElementById("fnh-tab-" + t);
            if (pane) {
                pane.style.display = (t === key) ? "flex" : "none";
            }
        });

        if (key === "variables") {
            renderVariables();
        } else if (key === "switches") {
            renderSwitches();
        } else if (key === "console") {
            refreshSnippetSelect();
        }
    }

    function toggleCheatMenu() {
        if (isMenuOpen) closeCheatMenu();
        else openCheatMenu();
    }

    function openCheatMenu() {
        if (typeof document === "undefined") return;
        const overlay = document.getElementById("fnh-cheat-overlay");
        if (!overlay) return;
        setCheatMode(currentViewMode || "engine");
        overlay.style.display = "flex";
        isMenuOpen = true;

        if (typeof globalObj.setModalState === "function") {
            globalObj.setModalState("cheat", true);
        } else if (typeof globalObj.registerModal === "function") {
            globalObj.registerModal("cheat", true);
        } else if (globalObj.RuntimePackOverlayPanels) {
            globalObj.RuntimePackOverlayPanels.open("cheat");
        } else if (globalObj.RuntimePackVPad && typeof globalObj.RuntimePackVPad.setOverlayModalOpen === "function") {
            globalObj.RuntimePackVPad.setOverlayModalOpen(true);
        }

        const cheatBtn = document.getElementById("vpad-btn-cheat");
        if (cheatBtn) cheatBtn.classList.add("active");
    }

    function closeCheatMenu() {
        if (typeof document === "undefined") return;
        const overlay = document.getElementById("fnh-cheat-overlay");
        if (!overlay) return;
        overlay.style.display = "none";
        isMenuOpen = false;

        if (typeof globalObj.setModalState === "function") {
            globalObj.setModalState("cheat", false);
        } else if (typeof globalObj.registerModal === "function") {
            globalObj.registerModal("cheat", false);
        } else if (globalObj.RuntimePackOverlayPanels) {
            globalObj.RuntimePackOverlayPanels.close("cheat");
        } else if (globalObj.RuntimePackVPad && typeof globalObj.RuntimePackVPad.setOverlayModalOpen === "function") {
            globalObj.RuntimePackVPad.setOverlayModalOpen(false);
        }

        const cheatBtn = document.getElementById("vpad-btn-cheat");
        if (cheatBtn) cheatBtn.classList.remove("active");
    }

    const THEME_CYCLE = ["clean", "retro", "matte"];

    function toggleTheme() {
        if (typeof document === "undefined") return;
        let current = "clean";
        if (document.body.classList.contains("vpad-theme-retro")) current = "retro";
        else if (document.body.classList.contains("vpad-theme-matte")) current = "matte";
        else if (document.body.classList.contains("vpad-theme-clean")) current = "clean";

        const nextIdx = (THEME_CYCLE.indexOf(current) + 1) % THEME_CYCLE.length;
        const nextTheme = THEME_CYCLE[nextIdx];

        if (typeof globalObj.applyVpadTheme === "function") {
            globalObj.applyVpadTheme(nextTheme);
        } else {
            document.body.classList.remove("vpad-theme-clean", "vpad-theme-matte", "vpad-theme-retro");
            document.body.classList.add("vpad-theme-" + nextTheme);
            document.body.classList.toggle("vpad-theme-retro", nextTheme === "retro");
        }

        const themeLabels = {
            clean: "Minimalist Glass",
            matte: "Minimalist Matte",
            retro: "Retro OSD"
        };
        showToast("UI Theme: " + (themeLabels[nextTheme] || nextTheme));
    }

    // ------------------------------------------------------------------------
    // DOM INITIALIZATION
    // ------------------------------------------------------------------------
    function initCheatDOM() {
        if (typeof document === "undefined") return;
        if (document.getElementById("fnh-cheat-overlay")) return;
        injectStyles();

        let topBar = document.getElementById("vpad-topbar");
        if (!topBar) {
            topBar = document.createElement("div");
            topBar.id = "vpad-topbar";
            document.body.appendChild(topBar);
        }

        function attachTap(el, cb) {
            if (!el) return;
            let last = 0;
            const handler = (e) => {
                e.stopPropagation();
                if (e.preventDefault && e.cancelable) e.preventDefault();
                const now = Date.now();
                if (now - last < 250) return;
                last = now;
                cb(e);
            };
            el.addEventListener("touchend", handler);
            el.addEventListener("click", handler);
        }

        let cheatBtn = document.getElementById("vpad-btn-cheat");
        if (!cheatBtn) {
            cheatBtn = document.createElement("div");
            cheatBtn.id = "vpad-btn-cheat";
            cheatBtn.className = "vpad-top-btn cheat-interactive";
            cheatBtn.title = "Cheat Menu";
            cheatBtn.innerHTML = `<svg viewBox="0 0 24 24"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>`;
            attachTap(cheatBtn, function () { CheatMenu.toggle(); });
            topBar.appendChild(cheatBtn);
        }

        const modal = document.createElement("div");
        modal.id = "fnh-cheat-overlay";
        modal.className = "cheat-interactive";
        modal.innerHTML = `
            <div id="fnh-cheat-header">
                <div class="fnh-cheat-title">
                    <svg viewBox="0 0 24 24"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
                    <span>CHEAT ENGINE</span>
                </div>
                <div style="display:flex; align-items:center; gap:8px;">
                    <button id="btn-cheat-mode-toggle" class="fnh-cheat-btn">Cheat Menu</button>
                    <button id="btn-cheat-theme-toggle" class="fnh-cheat-btn" style="padding:3px 8px; font-size:10px;">Theme</button>
                    <div class="fnh-cheat-close" id="fnh-cheat-close-btn" title="Close">✕</div>
                </div>
            </div>
            <div id="fnh-cheat-tabs">
                <div class="fnh-cheat-tab active" data-tab="variables" id="fnh-tab-btn-variables">
                    ${ICONS.variable} <span>Variables</span>
                </div>
                <div class="fnh-cheat-tab" data-tab="switches" id="fnh-tab-btn-switches">
                    ${ICONS.switch} <span>Switches</span>
                </div>
                <div class="fnh-cheat-tab" data-tab="spawner" id="fnh-tab-btn-spawner">
                    ${ICONS.bag} <span>Spawner</span>
                </div>
                <div class="fnh-cheat-tab" data-tab="console" id="fnh-tab-btn-console">
                    ${ICONS.terminal} <span>Console</span>
                </div>
            </div>
            <div id="fnh-cheat-body">
                <!-- TAB 1: VARIABLES BROWSER -->
                <div id="fnh-tab-variables" class="fnh-tab-pane" style="display:flex;">
                    <div style="display:flex; gap:6px; align-items:center;">
                        <input type="text" id="fnh-var-search" placeholder="Search variable by ID or Name..." class="fnh-input" style="flex:1;">
                        <button id="fnh-var-refresh" class="fnh-cheat-btn">Refresh</button>
                    </div>
                    <div id="fnh-var-list" class="fnh-cards-container"></div>
                </div>

                <!-- TAB 2: SWITCHES BROWSER -->
                <div id="fnh-tab-switches" class="fnh-tab-pane" style="display:none;">
                    <div style="display:flex; gap:6px; align-items:center;">
                        <input type="text" id="fnh-switch-search" placeholder="Search switch by ID or Name..." class="fnh-input" style="flex:1;">
                        <button id="fnh-switch-refresh" class="fnh-cheat-btn">Refresh</button>
                    </div>
                    <div id="fnh-switch-list" class="fnh-cards-container"></div>
                </div>

                <!-- TAB 3: ITEM SPAWNER & QUICK CHEATS -->
                <div id="fnh-tab-spawner" class="fnh-tab-pane" style="display:none;">
                    <div style="display:grid; grid-template-columns: 1fr 1fr; gap:8px;">
                        <button id="fnh-btn-max-gold" class="fnh-cheat-btn fnh-btn-action" style="padding:10px 0;">Max Gold</button>
                        <button id="fnh-btn-recover-all" class="fnh-cheat-btn fnh-btn-action" style="padding:10px 0;">Recover All</button>
                    </div>
                    <div class="fnh-cheat-row" style="margin-top:6px;">
                        <div class="fnh-cheat-left">Category</div>
                        <select id="fnh-spawner-category" class="fnh-select" style="min-width:110px;">
                            <option value="item">item</option>
                            <option value="weapon">weapon</option>
                            <option value="armor">armor</option>
                        </select>
                    </div>
                    <div class="fnh-cheat-row">
                        <div class="fnh-cheat-left">Item ID</div>
                        <input type="number" id="fnh-spawner-id" value="1" min="1" class="fnh-input" style="width:80px;">
                    </div>
                    <div class="fnh-cheat-row">
                        <div class="fnh-cheat-left">Count</div>
                        <div style="display:flex; gap:6px; align-items:center;">
                            <input type="number" id="fnh-spawner-count" value="1" min="1" class="fnh-input" style="width:60px;">
                            <button class="fnh-cheat-btn fnh-btn-quick-count" data-add="1">+1</button>
                            <button class="fnh-cheat-btn fnh-btn-quick-count" data-add="5">+5</button>
                            <button class="fnh-cheat-btn fnh-btn-quick-count" data-add="50">+50</button>
                        </div>
                    </div>
                    <button id="fnh-btn-spawn-item" class="fnh-cheat-btn fnh-btn-action" style="width:100%; padding:10px 0; margin-top:6px;">Spawn Item</button>
                </div>

                <!-- TAB 4: M-TOOL SCRIPT CONSOLE & SNIPPET MANAGER -->
                <div id="fnh-tab-console" class="fnh-tab-pane" style="display:none;">
                    <textarea id="fnh-console-input" class="fnh-textarea" rows="4" placeholder="e.g. $game_player.move_speed = 6"></textarea>
                    <div style="display:flex; gap:8px;">
                        <button id="fnh-btn-run-script" class="fnh-cheat-btn fnh-btn-action" style="flex:1; padding:8px 0;">▶ Run Script</button>
                    </div>
                    <pre id="fnh-console-output" class="fnh-console-pre">Ready.</pre>
                    <div class="fnh-cheat-row" style="flex-direction:column; align-items:stretch; gap:6px;">
                        <div style="font-size:11px; font-weight:700; color:#a1a1aa; letter-spacing:0.5px;">SAVED SNIPPETS</div>
                        <div style="display:flex; gap:6px;">
                            <input type="text" id="fnh-snippet-name" placeholder="Snippet Name..." class="fnh-input" style="flex:1;">
                            <button id="fnh-btn-save-snippet" class="fnh-cheat-btn">💾 Save Snippet</button>
                        </div>
                        <div style="display:flex; gap:6px;">
                            <select id="fnh-snippet-select" class="fnh-select" style="flex:1;">
                                <option value="">-- Select Saved Snippet --</option>
                            </select>
                            <button id="fnh-btn-load-snippet" class="fnh-cheat-btn">Load</button>
                            <button id="fnh-btn-delete-snippet" class="fnh-cheat-btn" style="background:#7f1d1d; border-color:#991b1b; color:#fecaca;">Delete</button>
                        </div>
                    </div>
                </div>

                <!-- VIEW 5: PROFILE CHEAT MENU (READY-TO-USE PRESETS) -->
                <div id="fnh-tab-profile-menu" class="fnh-tab-pane" style="display:none;"></div>
            </div>
        `;
        document.body.appendChild(modal);
        modal.addEventListener("pointerdown", (e) => e.stopPropagation());
        modal.addEventListener("pointerup", (e) => e.stopPropagation());

        // Header and Modal Buttons
        const closeBtn = document.getElementById("fnh-cheat-close-btn");
        if (closeBtn) attachTap(closeBtn, function () { CheatMenu.close(); });

        const themeBtn = document.getElementById("btn-cheat-theme-toggle");
        if (themeBtn) attachTap(themeBtn, function () { CheatMenu.theme(); });

        const modeBtn = document.getElementById("btn-cheat-mode-toggle");
        if (modeBtn) attachTap(modeBtn, function () { toggleCheatMode(); });

        // Profile Cheat Menu Listeners
        const profilePane = document.getElementById("fnh-tab-profile-menu");
        if (profilePane) {
            profilePane.addEventListener("click", function (e) {
                const target = e.target;

                // Category pill tab switch
                const catBtn = target.closest ? target.closest(".fnh-cat-btn") : null;
                if (catBtn) {
                    activeProfileSecIdx = parseInt(catBtn.getAttribute("data-idx"), 10) || 0;
                    renderCheatMenuProfile();
                    return;
                }

                // Toggle button (ON/OFF)
                const togBtn = target.closest ? target.closest(".fnh-prof-toggle-btn") : null;
                if (togBtn) {
                    const sec = parseInt(togBtn.getAttribute("data-sec"), 10);
                    const idx = parseInt(togBtn.getAttribute("data-idx"), 10);
                    if (globalObj.GAME_CHEATS && globalObj.GAME_CHEATS.sections && globalObj.GAME_CHEATS.sections[sec]) {
                        const item = globalObj.GAME_CHEATS.sections[sec].items[idx];
                        const isCurrentlyOn = togBtn.classList.contains("fnh-btn-toggle-on");
                        const nextOn = !isCurrentlyOn;
                        profileToggles[item.id] = nextOn;
                        const ce = getCheatEngine();
                        if (item.action && ce && typeof ce.postCommand === "function" && ce.isNative()) {
                            ce.postCommand({ action: item.action, value: nextOn });
                        } else if (item.scriptOn && ce && typeof ce.evalScript === "function") {
                            ce.evalScript(nextOn ? item.scriptOn : item.scriptOff);
                        }
                        togBtn.classList.toggle("fnh-btn-toggle-on", nextOn);
                        togBtn.textContent = nextOn ? "ON" : "OFF";
                        showToast(item.name + ": " + (nextOn ? "ON" : "OFF"));
                    }
                    return;
                }

                const actBtn = target.closest ? target.closest(".fnh-prof-action-btn") : null;
                if (actBtn) {
                    const sec = parseInt(actBtn.getAttribute("data-sec"), 10);
                    const idx = parseInt(actBtn.getAttribute("data-idx"), 10);
                    if (globalObj.GAME_CHEATS && globalObj.GAME_CHEATS.sections && globalObj.GAME_CHEATS.sections[sec]) {
                        const item = globalObj.GAME_CHEATS.sections[sec].items[idx];
                        const ce = getCheatEngine();
                        if (item.action === "recover_all") {
                            if (ce && ce.recoverAll) ce.recoverAll();
                            showToast("Executed: " + item.name);
                        } else if (item.action === "gain_gold") {
                            if (ce && ce.gainGold) ce.gainGold(item.value || 10000);
                            showToast("Executed: " + item.name);
                        } else if (item.action === "save_anywhere" && !ce.isNative()) {
                            if (typeof SceneManager !== "undefined" && typeof Scene_Save !== "undefined") {
                                SceneManager.push(Scene_Save);
                            }
                            showToast("Executed: " + item.name);
                        } else if (item.action && ce && typeof ce.postCommand === "function" && ce.isNative()) {
                            ce.postCommand({ action: item.action });
                            showToast("Executed: " + item.name);
                        } else if (item.script) {
                            if (ce && ce.evalScript) ce.evalScript(item.script);
                            showToast("Executed: " + item.name);
                        }
                    }
                    return;
                }

                const swBtn = target.closest ? target.closest(".fnh-prof-switch-btn") : null;
                if (swBtn) {
                    const sId = parseInt(swBtn.getAttribute("data-sid"), 10);
                    const isOn = swBtn.classList.contains("fnh-btn-toggle-on");
                    const ce = getCheatEngine();
                    if (ce && typeof ce.setSwitch === "function") {
                        ce.setSwitch(sId, !isOn);
                        swBtn.classList.toggle("fnh-btn-toggle-on", !isOn);
                        swBtn.textContent = !isOn ? "ON" : "OFF";
                        showToast("Switch #" + sId + ": " + (!isOn ? "ON" : "OFF"));
                    }
                    return;
                }

                const varBtn = target.closest ? target.closest(".fnh-prof-var-btn") : null;
                if (varBtn) {
                    const vId = parseInt(varBtn.getAttribute("data-vid"), 10);
                    const inputId = varBtn.getAttribute("data-input-id");
                    const inputEl = document.getElementById(inputId);
                    if (inputEl) {
                        const val = parseInt(inputEl.value, 10) || 0;
                        const ce = getCheatEngine();
                        if (ce && typeof ce.setVariable === "function") {
                            ce.setVariable(vId, val);
                            showToast("Variable #" + vId + " set to " + val);
                        }
                    }
                    return;
                }

                const scrBtn = target.closest ? target.closest(".fnh-prof-script-btn") : null;
                if (scrBtn) {
                    const sec = parseInt(scrBtn.getAttribute("data-sec"), 10);
                    const idx = parseInt(scrBtn.getAttribute("data-idx"), 10);
                    if (globalObj.GAME_CHEATS && globalObj.GAME_CHEATS.sections && globalObj.GAME_CHEATS.sections[sec]) {
                        const item = globalObj.GAME_CHEATS.sections[sec].items[idx];
                        const ce = getCheatEngine();
                        if (ce && typeof ce.evalScript === "function" && item.script) {
                            ce.evalScript(item.script);
                            showToast("Run: " + item.name);
                        }
                    }
                    return;
                }
            });
        }

        // Tab Navigation Listeners
        const tabsContainer = document.getElementById("fnh-cheat-tabs");
        if (tabsContainer) {
            tabsContainer.addEventListener("click", function (e) {
                const target = e.target;
                const tabEl = target.closest ? target.closest(".fnh-cheat-tab") : null;
                if (tabEl) {
                    const tabKey = tabEl.getAttribute("data-tab");
                    if (tabKey) switchTab(tabKey);
                }
            });
        }

        // Variables Tab Listeners
        const varSearch = document.getElementById("fnh-var-search");
        if (varSearch) {
            varSearch.addEventListener("input", function () {
                renderVariables(this.value);
            });
        }
        const varRefresh = document.getElementById("fnh-var-refresh");
        if (varRefresh) {
            varRefresh.addEventListener("click", function () {
                callDumpState();
                getCheatState();
                renderVariables();
                showToast("Variables refreshed");
            });
        }
        const varList = document.getElementById("fnh-var-list");
        if (varList) {
            varList.addEventListener("click", function (e) {
                const target = e.target;
                const btn = target.closest ? target.closest(".fnh-var-set-btn") : (target.classList && target.classList.contains("fnh-var-set-btn") ? target : null);
                if (btn) {
                    const id = btn.getAttribute("data-id");
                    if (id) applyVariableSet(id);
                }
            });
        }

        // Switches Tab Listeners
        const switchSearch = document.getElementById("fnh-switch-search");
        if (switchSearch) {
            switchSearch.addEventListener("input", function () {
                renderSwitches(this.value);
            });
        }
        const switchRefresh = document.getElementById("fnh-switch-refresh");
        if (switchRefresh) {
            switchRefresh.addEventListener("click", function () {
                callDumpState();
                getCheatState();
                renderSwitches();
                showToast("Switches refreshed");
            });
        }
        const switchList = document.getElementById("fnh-switch-list");
        if (switchList) {
            switchList.addEventListener("click", function (e) {
                const target = e.target;
                const btn = target.closest ? target.closest(".fnh-switch-toggle-btn") : (target.classList && target.classList.contains("fnh-switch-toggle-btn") ? target : null);
                if (btn) {
                    const id = btn.getAttribute("data-id");
                    if (id) toggleSwitch(id);
                }
            });
        }

        // Spawner Listeners
        const btnMaxGold = document.getElementById("fnh-btn-max-gold");
        if (btnMaxGold) btnMaxGold.onclick = maxGold;

        const btnRecoverAll = document.getElementById("fnh-btn-recover-all");
        if (btnRecoverAll) btnRecoverAll.onclick = recoverAll;

        const btnSpawnItem = document.getElementById("fnh-btn-spawn-item");
        if (btnSpawnItem) btnSpawnItem.onclick = function () { spawnItem(); };

        const tabSpawner = document.getElementById("fnh-tab-spawner");
        if (tabSpawner) {
            tabSpawner.addEventListener("click", function (e) {
                const target = e.target;
                const quickBtn = target.closest ? target.closest(".fnh-btn-quick-count") : null;
                if (quickBtn) {
                    const add = Number(quickBtn.getAttribute("data-add")) || 1;
                    const countInput = document.getElementById("fnh-spawner-count");
                    if (countInput) {
                        countInput.value = (Number(countInput.value) || 0) + add;
                    }
                }
            });
        }

        // Console Listeners
        const btnRunScript = document.getElementById("fnh-btn-run-script");
        if (btnRunScript) btnRunScript.onclick = runScript;

        const btnSaveSnippet = document.getElementById("fnh-btn-save-snippet");
        if (btnSaveSnippet) btnSaveSnippet.onclick = handleSaveSnippet;

        const btnLoadSnippet = document.getElementById("fnh-btn-load-snippet");
        if (btnLoadSnippet) btnLoadSnippet.onclick = handleLoadSnippet;

        const btnDeleteSnippet = document.getElementById("fnh-btn-delete-snippet");
        if (btnDeleteSnippet) btnDeleteSnippet.onclick = handleDeleteSnippet;

        const snippetSelect = document.getElementById("fnh-snippet-select");
        if (snippetSelect) {
            snippetSelect.addEventListener("change", function () {
                handleLoadSnippet();
            });
        }

        // Initial renders & tab activation
        switchTab(activeTab || "variables");
        refreshSnippetSelect();

        // Draggable Header
        const header = document.getElementById("fnh-cheat-header");
        let isDragging = false;
        let startX, startY, initLeft, initTop;

        function startDrag(clientX, clientY) {
            isDragging = true;
            startX = clientX;
            startY = clientY;
            const rect = modal.getBoundingClientRect ? modal.getBoundingClientRect() : { left: 0, top: 0 };
            initLeft = rect.left;
            initTop = rect.top;
            modal.style.transform = "none";
            modal.style.left = initLeft + "px";
            modal.style.top = initTop + "px";
        }

        function moveDrag(clientX, clientY) {
            if (!isDragging) return;
            const dx = clientX - startX;
            const dy = clientY - startY;
            let nX = initLeft + dx;
            let nY = initTop + dy;
            const winW = globalObj.innerWidth || 800;
            const winH = globalObj.innerHeight || 600;
            const modalW = modal.offsetWidth || 300;
            const modalH = modal.offsetHeight || 300;
            nX = Math.max(10, Math.min(winW - modalW - 10, nX));
            nY = Math.max(10, Math.min(winH - modalH - 10, nY));
            modal.style.left = nX + "px";
            modal.style.top = nY + "px";
        }

        if (header) {
            header.style.touchAction = "none";

            const isIgnoredTarget = (target) => {
                return target.id === "fnh-cheat-close-btn" ||
                       target.id === "btn-cheat-theme-toggle" ||
                       target.id === "btn-cheat-mode-toggle" ||
                       (target.closest && (target.closest("#fnh-cheat-close-btn") || target.closest("#btn-cheat-theme-toggle") || target.closest("#btn-cheat-mode-toggle")));
            };

            header.addEventListener("pointerdown", (e) => {
                if (isIgnoredTarget(e.target)) return;
                try {
                    if (header.setPointerCapture) {
                        header.setPointerCapture(e.pointerId);
                    }
                } catch (err) {}
                startDrag(e.clientX, e.clientY);
            });

            header.addEventListener("pointermove", (e) => {
                if (isDragging) {
                    moveDrag(e.clientX, e.clientY);
                }
            });

            const onPointerUp = (e) => {
                if (isDragging) {
                    try {
                        if (header.releasePointerCapture) {
                            header.releasePointerCapture(e.pointerId);
                        }
                    } catch (err) {}
                    isDragging = false;
                }
            };
            header.addEventListener("pointerup", onPointerUp);
            header.addEventListener("pointercancel", onPointerUp);

            header.addEventListener("mousedown", (e) => {
                if (globalObj.PointerEvent) return;
                if (isIgnoredTarget(e.target)) return;
                e.preventDefault();
                startDrag(e.clientX, e.clientY);
            });
            header.addEventListener("touchstart", (e) => {
                if (globalObj.PointerEvent) return;
                if (isIgnoredTarget(e.target)) return;
                if (e.touches && e.touches.length === 1) {
                    e.preventDefault();
                    startDrag(e.touches[0].clientX, e.touches[0].clientY);
                }
            }, { passive: false });
        }

        document.addEventListener("mousemove", (e) => {
            if (!globalObj.PointerEvent && isDragging) {
                e.preventDefault();
                moveDrag(e.clientX, e.clientY);
            }
        });
        document.addEventListener("touchmove", (e) => {
            if (!globalObj.PointerEvent && isDragging && e.touches && e.touches.length === 1) {
                e.preventDefault();
                moveDrag(e.touches[0].clientX, e.touches[0].clientY);
            }
        }, { passive: false });

        const endDrag = () => { isDragging = false; };
        document.addEventListener("mouseup", endDrag);
        document.addEventListener("touchend", endDrag);

        document.addEventListener("keydown", (e) => {
            if (e.key === "`" || e.key === "~" || e.code === "Backquote" || e.code === "F10") {
                toggleCheatMenu();
            }
        });
    }

    const CheatMenu = {
        open: openCheatMenu,
        close: closeCheatMenu,
        toggle: toggleCheatMenu,
        setTab: switchTab,
        initDOM: initCheatDOM,
        renderVariables: renderVariables,
        renderSwitches: renderSwitches,
        saveSnippet: saveSnippet,
        getSavedSnippets: getSavedSnippets,
        deleteSnippet: deleteSnippet,
        runScript: runScript,
        spawnItem: spawnItem,
        maxGold: maxGold,
        recoverAll: recoverAll,
        applyVariableSet: applyVariableSet,
        toggleSwitch: toggleSwitch,
        theme: toggleTheme,
        toggleMode: toggleCheatMode,
        setMode: setCheatMode,
        renderProfile: renderCheatMenuProfile,
        // Legacy helpers for backwards compatibility
        godMode: function () {
            godModeActive = !godModeActive;
            showToast(godModeActive ? "God Mode: ACTIVE" : "God Mode: OFF");
        },
        noclip: function () {
            noclipActive = !noclipActive;
            if (typeof globalObj.$gamePlayer !== "undefined" && globalObj.$gamePlayer.setThrough) {
                globalObj.$gamePlayer.setThrough(noclipActive);
            }
            showToast(noclipActive ? "Noclip: ACTIVE" : "Noclip: OFF");
        }
    };

    globalObj.CheatMenu = CheatMenu;
    globalObj.FNHCheat = CheatMenu;

    function startup() {
        hookGameLoop();
        initCheatDOM();
    }

    if (typeof document !== "undefined") {
        if (document.readyState === "complete" || document.readyState === "interactive") {
            setTimeout(startup, 250);
        } else {
            document.addEventListener("DOMContentLoaded", () => setTimeout(startup, 250));
        }
        if (typeof globalObj.addEventListener === "function") {
            globalObj.addEventListener("load", () => {
                setTimeout(startup, 800);
            });
        }
    }

    if (typeof module !== "undefined" && module.exports) {
        module.exports = CheatMenu;
    }
})();
