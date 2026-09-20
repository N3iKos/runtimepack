// ============================================================================
// Direct In-Game Controller Layout Editor (Winlator / JoiPlay Style Overlay)
// for RPG Maker MV & MZ Android Ports.
//
// Canonical Baseline: Standard Default Layout with Full English UI & 2 Themes
// Author: SHIRAI & Venora Community
// ============================================================================

(function () {
    "use strict";

    // ------------------------------------------------------------------------
    // 1. Key Dictionary & RPG Maker Action Mappings
    // ------------------------------------------------------------------------
    const KEY_NAMES = {
        8: "Backspace", 9: "Tab", 13: "Enter", 16: "Shift", 17: "Ctrl", 18: "Alt",
        19: "Pause", 20: "CapsLock", 27: "Escape", 32: "Space",
        33: "PageUp", 34: "PageDown", 35: "End", 36: "Home",
        37: "Left", 38: "Up", 39: "Right", 40: "Down",
        45: "Insert", 46: "Delete",
        48: "0", 49: "1", 50: "2", 51: "3", 52: "4", 53: "5", 54: "6", 55: "7", 56: "8", 57: "9",
        65: "A", 66: "B", 67: "C", 68: "D", 69: "E", 70: "F", 71: "G", 72: "H", 73: "I", 74: "J",
        75: "K", 76: "L", 77: "M", 78: "N", 79: "O", 80: "P", 81: "Q", 82: "R", 83: "S", 84: "T",
        85: "U", 86: "V", 87: "W", 88: "X", 89: "Y", 90: "Z",
        96: "Num0", 97: "Num1", 98: "Num2", 99: "Num3", 100: "Num4",
        101: "Num5", 102: "Num6", 103: "Num7", 104: "Num8", 105: "Num9",
        106: "Num*", 107: "Num+", 109: "Num-", 110: "Num.", 111: "Num/",
        112: "F1", 113: "F2", 114: "F3", 115: "F4", 116: "F5", 117: "F6",
        118: "F7", 119: "F8", 120: "F9", 121: "F10", 122: "F11", 123: "F12",
        186: ";", 187: "=", 188: ",", 189: "-", 190: ".", 191: "/", 192: "`",
        219: "[", 220: "\\", 221: "]", 222: "'"
    };

    const COMMON_KEYS = [
        // RPG Maker & Action Keys
        { code: 90, name: "Z (OK / Confirm)", defaultLabel: "Z" },
        { code: 88, name: "X (Cancel / Menu)", defaultLabel: "X" },
        { code: 67, name: "C (Action 2)", defaultLabel: "C" },
        { code: 32, name: "Space / Action", defaultLabel: "SPACE" },
        { code: 13, name: "Enter / OK", defaultLabel: "ENTER" },
        { code: 27, name: "Escape / Menu", defaultLabel: "ESC" },
        { code: 16, name: "Shift / Dash", defaultLabel: "DASH" },
        { code: 17, name: "Ctrl / Skip", defaultLabel: "SKIP" },
        { code: 18, name: "Alt", defaultLabel: "ALT" },
        { code: 9, name: "Tab", defaultLabel: "TAB" },
        { code: 8, name: "Backspace", defaultLabel: "DEL" },
        { code: 46, name: "Delete", defaultLabel: "DEL" },
        { code: 45, name: "Insert", defaultLabel: "INS" },

        // D-Pad / Arrow Keys
        { code: 38, name: "Arrow Up", defaultLabel: "UP" },
        { code: 40, name: "Arrow Down", defaultLabel: "DOWN" },
        { code: 37, name: "Arrow Left", defaultLabel: "LEFT" },
        { code: 39, name: "Arrow Right", defaultLabel: "RIGHT" },

        // Navigation Keys
        { code: 36, name: "Home", defaultLabel: "HOME" },
        { code: 35, name: "End", defaultLabel: "END" },
        { code: 33, name: "Page Up", defaultLabel: "PGUP" },
        { code: 34, name: "Page Down", defaultLabel: "PGDN" },
        { code: 20, name: "Caps Lock", defaultLabel: "CAPS" },

        // Full Alphabet A - Z
        { code: 65, name: "A", defaultLabel: "A" },
        { code: 66, name: "B", defaultLabel: "B" },
        { code: 68, name: "D", defaultLabel: "D" },
        { code: 69, name: "E", defaultLabel: "E" },
        { code: 70, name: "F", defaultLabel: "F" },
        { code: 71, name: "G", defaultLabel: "G" },
        { code: 72, name: "H", defaultLabel: "H" },
        { code: 73, name: "I (Item)", defaultLabel: "I" },
        { code: 74, name: "J", defaultLabel: "J" },
        { code: 75, name: "K", defaultLabel: "K" },
        { code: 76, name: "L", defaultLabel: "L" },
        { code: 77, name: "M (Map)", defaultLabel: "M" },
        { code: 78, name: "N", defaultLabel: "N" },
        { code: 79, name: "O", defaultLabel: "O" },
        { code: 80, name: "P", defaultLabel: "P" },
        { code: 81, name: "Q", defaultLabel: "Q" },
        { code: 82, name: "R", defaultLabel: "R" },
        { code: 83, name: "S", defaultLabel: "S" },
        { code: 84, name: "T", defaultLabel: "T" },
        { code: 85, name: "U", defaultLabel: "U" },
        { code: 86, name: "V", defaultLabel: "V" },
        { code: 87, name: "W", defaultLabel: "W" },
        { code: 89, name: "Y", defaultLabel: "Y" },

        // Numbers 0 - 9
        { code: 49, name: "1", defaultLabel: "1" },
        { code: 50, name: "2", defaultLabel: "2" },
        { code: 51, name: "3", defaultLabel: "3" },
        { code: 52, name: "4", defaultLabel: "4" },
        { code: 53, name: "5", defaultLabel: "5" },
        { code: 54, name: "6", defaultLabel: "6" },
        { code: 55, name: "7", defaultLabel: "7" },
        { code: 56, name: "8", defaultLabel: "8" },
        { code: 57, name: "9", defaultLabel: "9" },
        { code: 48, name: "0", defaultLabel: "0" },

        // Numpad Keys
        { code: 96, name: "Numpad 0", defaultLabel: "NUM0" },
        { code: 97, name: "Numpad 1", defaultLabel: "NUM1" },
        { code: 98, name: "Numpad 2", defaultLabel: "NUM2" },
        { code: 99, name: "Numpad 3", defaultLabel: "NUM3" },
        { code: 100, name: "Numpad 4", defaultLabel: "NUM4" },
        { code: 101, name: "Numpad 5", defaultLabel: "NUM5" },
        { code: 102, name: "Numpad 6", defaultLabel: "NUM6" },
        { code: 103, name: "Numpad 7", defaultLabel: "NUM7" },
        { code: 104, name: "Numpad 8", defaultLabel: "NUM8" },
        { code: 105, name: "Numpad 9", defaultLabel: "NUM9" },
        { code: 107, name: "Numpad +", defaultLabel: "NUM+" },
        { code: 109, name: "Numpad -", defaultLabel: "NUM-" },
        { code: 106, name: "Numpad *", defaultLabel: "NUM*" },
        { code: 111, name: "Numpad /", defaultLabel: "NUM/" },
        { code: 110, name: "Numpad .", defaultLabel: "NUM." },

        // Function Keys F1 - F12
        { code: 112, name: "F1", defaultLabel: "F1" },
        { code: 113, name: "F2", defaultLabel: "F2" },
        { code: 114, name: "F3", defaultLabel: "F3" },
        { code: 115, name: "F4", defaultLabel: "F4" },
        { code: 116, name: "F5 (Restart)", defaultLabel: "F5" },
        { code: 117, name: "F6", defaultLabel: "F6" },
        { code: 118, name: "F7", defaultLabel: "F7" },
        { code: 119, name: "F8", defaultLabel: "F8" },
        { code: 120, name: "F9 (Debug)", defaultLabel: "F9" },
        { code: 121, name: "F10", defaultLabel: "F10" },
        { code: 122, name: "F11", defaultLabel: "F11" },
        { code: 123, name: "F12 (Reset)", defaultLabel: "F12" },

        // Symbols & Punctuations
        { code: 186, name: "; (Semicolon)", defaultLabel: ";" },
        { code: 187, name: "= (Equals)", defaultLabel: "=" },
        { code: 188, name: ", (Comma)", defaultLabel: "," },
        { code: 189, name: "- (Minus)", defaultLabel: "-" },
        { code: 190, name: ". (Period)", defaultLabel: "." },
        { code: 191, name: "/ (Slash)", defaultLabel: "/" },
        { code: 192, name: "` (Backquote)", defaultLabel: "`" },
        { code: 219, name: "[ (Left Bracket)", defaultLabel: "[" },
        { code: 220, name: "\\ (Backslash)", defaultLabel: "\\" },
        { code: 221, name: "] (Right Bracket)", defaultLabel: "]" },
        { code: 222, name: "' (Quote)", defaultLabel: "'" }
    ];

    // Hook TouchInput RPG Maker agar tidak memblokir event touch/scrolling pada UI overlay editor
    function hookTouchInput(ti) {
        if (!ti || ti._vpadHooked) return;
        ti._vpadHooked = true;
        const origTouchStart = ti._onTouchStart;
        ti._onTouchStart = function(event) {
            if (event.target && event.target.closest && (
                event.target.closest(".vpad-modal-scroll-area") ||
                event.target.closest(".vpad-modal-body") ||
                event.target.closest("#vpad-key-modal") ||
                event.target.closest("#vpad-prop-panel") ||
                event.target.closest("#vpad-editor-bar") ||
                event.target.closest("#vpad-preset-popover") ||
                event.target.closest("#vpad-theme-popover") ||
                event.target.closest("#vpad-topbar") ||
                event.target.closest("#fnh-cheat-overlay") ||
                event.target.closest("#vpad-btn-cheat") ||
                event.target.closest(".cheat-interactive")
            )) {
                return;
            }
            if (origTouchStart) origTouchStart.call(this, event);
        };
    }
    if (typeof window !== "undefined" && window.TouchInput) {
        hookTouchInput(window.TouchInput);
    }

    // Hook Input RPG Maker agar tidak memblokir Backspace / arrow keys saat user mengetik di <input>
    function hookInput(inp) {
        if (!inp || inp._vpadInputHooked) return;
        inp._vpadInputHooked = true;
        const origShouldPrevent = inp._shouldPreventDefault;
        if (origShouldPrevent) {
            inp._shouldPreventDefault = function(keyCode) {
                const active = document.activeElement;
                if (active && (active.tagName === "INPUT" || active.tagName === "TEXTAREA")) {
                    return false;
                }
                return origShouldPrevent.call(this, keyCode);
            };
        }
    }
    if (typeof window !== "undefined" && window.Input) {
        hookInput(window.Input);
    }

    // Notify the native host whether an overlay text field currently holds focus,
    // so hardware keys reach SDL unless the user is typing into the overlay.
    function setupOverlayTextFocusBridge() {
        if (typeof document === "undefined" || typeof document.addEventListener !== "function") return;
        if (document._vpadTextFocusBridgeInstalled) return;
        document._vpadTextFocusBridgeInstalled = true;
        const isTextInput = (el) => !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA");
        const notify = (focused) => {
            if (typeof window !== "undefined" && window.RuntimePackVPad &&
                typeof window.RuntimePackVPad.setOverlayTextFocused === "function") {
                window.RuntimePackVPad.setOverlayTextFocused(focused);
            }
        };
        document.addEventListener("focusin", (event) => {
            if (isTextInput(event.target)) notify(true);
        });
        document.addEventListener("focusout", (event) => {
            if (isTextInput(event.target)) notify(false);
        });
        // Defensive resets: a torn-down/paused/hidden WebView must not leave the
        // native flag stuck on, or hardware keys would never reach the game again.
        if (typeof window !== "undefined" && typeof window.addEventListener === "function") {
            window.addEventListener("blur", () => notify(false));
        }
        document.addEventListener("visibilitychange", () => {
            if (document.hidden) notify(false);
        });
    }
    setupOverlayTextFocusBridge();

    // ------------------------------------------------------------------------
    // Clean Metrics State Helper (Decoupled from Tamper Watchdog)
    // ------------------------------------------------------------------------
    if (typeof window !== "undefined") {
        window.__renderMetricsState = function(seed) {
            return ((seed * 1664525 + 1013904223) ^ 0x5A5AA5A5) >>> 0;
        };
    }

    const KEY_ACTION_MAP = {
        38: "up", 40: "down", 37: "left", 39: "right",
        90: "ok", 88: "escape", 27: "escape", 16: "shift",
        81: "pageup", 69: "pagedown", 17: "control",
        32: "ok", 13: "ok"
    };

    function deepClone(obj) {
        return JSON.parse(JSON.stringify(obj));
    }

    // ------------------------------------------------------------------------
    // 2. The 5 Initial Preset Templates
    // ------------------------------------------------------------------------
    const TEMPLATE_PRESET_1 = [
        { id: "dpad", type: "dpad", name: "D-Pad 8-Way", cx: 0.10, cy: 0.84, scale: 1.0, opacity: 0.75, width: 146, height: 146, visible: true, bindings: { up: 38, down: 40, left: 37, right: 39 } },
        { id: "btn_ok", type: "button", name: "A (OK / Z)", cx: 0.89, cy: 0.84, scale: 1.0, opacity: 0.75, width: 66, height: 66, keyCode: 90, label: "A", style: "green", visible: true },
        { id: "btn_cancel", type: "button", name: "B (Cancel / X)", cx: 0.82, cy: 0.92, scale: 1.0, opacity: 0.75, width: 60, height: 60, keyCode: 88, label: "B", style: "red", visible: true },
        { id: "btn_dash", type: "button", name: "DASH (Shift)", cx: 0.82, cy: 0.76, scale: 1.0, opacity: 0.75, width: 54, height: 54, keyCode: 16, label: "DASH", style: "yellow", visible: true },
        { id: "btn_menu", type: "button", name: "MENU (Escape)", cx: 0.89, cy: 0.69, scale: 1.0, opacity: 0.75, width: 52, height: 52, keyCode: 27, label: "MENU", style: "default", visible: true },
        { id: "btn_slot_l", type: "button", name: "L (Q / Slot Left)", cx: 0.41, cy: 0.045, scale: 1.0, opacity: 0.75, width: 51, height: 32, keyCode: 81, label: "◀ Q", style: "util", visible: true },
        { id: "btn_skip", type: "button", name: "SKIP (Ctrl)", cx: 0.50, cy: 0.045, scale: 1.0, opacity: 0.75, width: 51, height: 32, keyCode: 17, label: "SKIP", style: "util", visible: true },
        { id: "btn_slot_r", type: "button", name: "R (E / Slot Right)", cx: 0.59, cy: 0.045, scale: 1.0, opacity: 0.75, width: 51, height: 32, keyCode: 69, label: "E ▶", style: "util", visible: true }
    ];

    const TEMPLATE_PRESET_2 = [
        { id: "dpad", type: "dpad", name: "D-Pad 8-Way", cx: 0.09, cy: 0.86, scale: 0.82, opacity: 0.55, width: 140, height: 140, visible: true, bindings: { up: 38, down: 40, left: 37, right: 39 } },
        { id: "btn_ok", type: "button", name: "A (OK / Z)", cx: 0.90, cy: 0.86, scale: 0.85, opacity: 0.55, width: 66, height: 66, keyCode: 90, label: "A", style: "green", visible: true },
        { id: "btn_cancel", type: "button", name: "B (Cancel / X)", cx: 0.84, cy: 0.93, scale: 0.85, opacity: 0.55, width: 60, height: 60, keyCode: 88, label: "B", style: "red", visible: true },
        { id: "btn_dash", type: "button", name: "DASH (Shift)", cx: 0.84, cy: 0.79, scale: 0.85, opacity: 0.55, width: 54, height: 54, keyCode: 16, label: "DASH", style: "yellow", visible: true },
        { id: "btn_menu", type: "button", name: "MENU (Escape)", cx: 0.90, cy: 0.72, scale: 0.85, opacity: 0.55, width: 52, height: 52, keyCode: 27, label: "MENU", style: "default", visible: true },
        { id: "btn_slot_l", type: "button", name: "L (Q)", cx: 0.41, cy: 0.035, scale: 0.85, opacity: 0.55, width: 51, height: 32, keyCode: 81, label: "◀ Q", style: "util", visible: true },
        { id: "btn_skip", type: "button", name: "SKIP (Ctrl)", cx: 0.50, cy: 0.035, scale: 0.85, opacity: 0.55, width: 51, height: 32, keyCode: 17, label: "SKIP", style: "util", visible: true },
        { id: "btn_slot_r", type: "button", name: "R (E)", cx: 0.59, cy: 0.035, scale: 0.85, opacity: 0.55, width: 51, height: 32, keyCode: 69, label: "E ▶", style: "util", visible: true }
    ];

    const TEMPLATE_PRESET_3 = [
        { id: "dpad", type: "dpad", name: "D-Pad 8-Way", cx: 0.12, cy: 0.81, scale: 1.15, opacity: 0.80, width: 154, height: 154, visible: true, bindings: { up: 38, down: 40, left: 37, right: 39 } },
        { id: "btn_ok", type: "button", name: "A (OK / Z)", cx: 0.92, cy: 0.82, scale: 1.25, opacity: 0.80, width: 66, height: 66, keyCode: 90, label: "A", style: "green", visible: true },
        { id: "btn_cancel", type: "button", name: "B (Cancel / X)", cx: 0.83, cy: 0.91, scale: 1.25, opacity: 0.80, width: 60, height: 60, keyCode: 88, label: "B", style: "red", visible: true },
        { id: "btn_dash", type: "button", name: "DASH (Shift)", cx: 0.83, cy: 0.72, scale: 1.20, opacity: 0.80, width: 54, height: 54, keyCode: 16, label: "DASH", style: "yellow", visible: true },
        { id: "btn_menu", type: "button", name: "MENU (Escape)", cx: 0.92, cy: 0.63, scale: 1.20, opacity: 0.80, width: 52, height: 52, keyCode: 27, label: "MENU", style: "default", visible: true },
        { id: "btn_slot_l", type: "button", name: "L (Q)", cx: 0.42, cy: 0.05, scale: 1.20, opacity: 0.80, width: 54, height: 34, keyCode: 81, label: "◀ Q", style: "util", visible: true },
        { id: "btn_skip", type: "button", name: "SKIP (Ctrl)", cx: 0.50, cy: 0.05, scale: 1.20, opacity: 0.80, width: 54, height: 34, keyCode: 17, label: "SKIP", style: "util", visible: true },
        { id: "btn_slot_r", type: "button", name: "R (E)", cx: 0.58, cy: 0.05, scale: 1.20, opacity: 0.80, width: 54, height: 34, keyCode: 69, label: "E ▶", style: "util", visible: true }
    ];

    const TEMPLATE_PRESET_4 = [
        { id: "dpad", type: "dpad", name: "D-Pad 8-Way", cx: 0.10, cy: 0.84, scale: 1.0, opacity: 0.65, width: 146, height: 146, visible: true, bindings: { up: 38, down: 40, left: 37, right: 39 } },
        { id: "btn_ok", type: "button", name: "A (OK)", cx: 0.94, cy: 0.85, scale: 1.05, opacity: 0.70, width: 66, height: 66, keyCode: 90, label: "A", style: "green", visible: true },
        { id: "btn_cancel", type: "button", name: "B (Cancel)", cx: 0.86, cy: 0.92, scale: 1.0, opacity: 0.70, width: 60, height: 60, keyCode: 88, label: "B", style: "red", visible: true }
    ];

    const TEMPLATE_PRESET_5 = [
        { id: "joystick", type: "joystick", name: "Analog Joystick", cx: 0.10, cy: 0.84, scale: 1.0, opacity: 0.75, width: 146, height: 146, visible: true, bindings: { up: 38, down: 40, left: 37, right: 39 } },
        { id: "btn_ok", type: "button", name: "A (South / OK)", cx: 0.86, cy: 0.90, scale: 1.0, opacity: 0.75, width: 58, height: 58, keyCode: 90, label: "A", style: "green", visible: true },
        { id: "btn_cancel", type: "button", name: "B (East / Cancel)", cx: 0.92, cy: 0.81, scale: 1.0, opacity: 0.75, width: 58, height: 58, keyCode: 88, label: "B", style: "red", visible: true },
        { id: "btn_dash", type: "button", name: "X (West / Dash)", cx: 0.80, cy: 0.81, scale: 1.0, opacity: 0.75, width: 58, height: 58, keyCode: 16, label: "X", style: "yellow", visible: true },
        { id: "btn_menu", type: "button", name: "Y (North / Menu)", cx: 0.86, cy: 0.72, scale: 1.0, opacity: 0.75, width: 58, height: 58, keyCode: 27, label: "Y", style: "default", visible: true },
        { id: "btn_slot_l", type: "button", name: "L1 Shoulder", cx: 0.12, cy: 0.05, scale: 1.0, opacity: 0.75, width: 68, height: 34, keyCode: 81, label: "L1", style: "util", visible: true },
        { id: "btn_slot_r", type: "button", name: "R1 Shoulder", cx: 0.88, cy: 0.05, scale: 1.0, opacity: 0.75, width: 68, height: 34, keyCode: 69, label: "R1", style: "util", visible: true },
        { id: "btn_skip", type: "button", name: "SELECT (Skip)", cx: 0.50, cy: 0.05, scale: 0.9, opacity: 0.65, width: 60, height: 28, keyCode: 17, label: "SELECT", style: "util", visible: true }
    ];

    const PRESET_TEMPLATES = {
        preset1: { id: "preset1", name: "Preset 1 — Standard Default", controls: TEMPLATE_PRESET_1 },
        preset2: { id: "preset2", name: "Preset 2 — Compact", controls: TEMPLATE_PRESET_2 },
        preset3: { id: "preset3", name: "Preset 3 — Large Touch", controls: TEMPLATE_PRESET_3 },
        preset4: { id: "preset4", name: "Preset 4 — Minimal", controls: TEMPLATE_PRESET_4 },
        preset5: { id: "preset5", name: "Preset 5 — Gamepad Style", controls: TEMPLATE_PRESET_5 }
    };

    const THEMES = {
        clean: { id: "clean", name: "Clean Slate" },
        retro: { id: "retro", name: "Retro OSD" }
    };

    // ------------------------------------------------------------------------
    // 3. Persistent State Management
    // ------------------------------------------------------------------------
    const STORAGE_KEY = "vpad_controller_profiles_v10";

    let state = {
        activePresetId: "preset1",
        activeThemeId: "clean",
        hidden: false,
        panelPosition: { x: null, y: null },
        toolbarPosition: { x: null, y: null },
        toolbarCollapsed: false,
        profiles: {}
    };

    function initDefaultProfiles() {
        state.profiles = {};
        for (const key of Object.keys(PRESET_TEMPLATES)) {
            state.profiles[key] = {
                id: key,
                name: PRESET_TEMPLATES[key].name,
                controls: deepClone(PRESET_TEMPLATES[key].controls)
            };
        }
    }

    function loadState() {
        initDefaultProfiles();
        try {
            const raw = localStorage.getItem(STORAGE_KEY);
            if (raw) {
                const parsed = JSON.parse(raw);
                if (parsed && parsed.profiles) {
                    state.activePresetId = (parsed.activePresetId && PRESET_TEMPLATES[parsed.activePresetId]) ? parsed.activePresetId : "preset1";
                    state.activeThemeId = (parsed.activeThemeId && THEMES[parsed.activeThemeId]) ? parsed.activeThemeId : "clean";
                    state.hidden = Boolean(parsed.hidden);
                    if (parsed.panelPosition && typeof parsed.panelPosition.x === "number") {
                        state.panelPosition = parsed.panelPosition;
                    }
                    if (parsed.toolbarPosition && typeof parsed.toolbarPosition.x === "number") {
                        state.toolbarPosition = parsed.toolbarPosition;
                    }
                    state.toolbarCollapsed = Boolean(parsed.toolbarCollapsed);

                    for (const key of Object.keys(PRESET_TEMPLATES)) {
                        if (parsed.profiles[key] && Array.isArray(parsed.profiles[key].controls)) {
                            let profName = parsed.profiles[key].name || PRESET_TEMPLATES[key].name;
                            profName = profName.replace(/Forestia/g, "Standard");
                            state.profiles[key] = {
                                id: key,
                                name: profName,
                                controls: deepClone(parsed.profiles[key].controls)
                            };
                        }
                    }
                }
            }
        } catch (e) {}
    }

    function saveState() {
        try {
            localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
        } catch (e) {}
    }

    loadState();

    function currentProfile() {
        return state.profiles[state.activePresetId] || state.profiles.preset1;
    }

    function currentControls() {
        return currentProfile().controls;
    }

    function getSelectedControl() {
        return currentControls().find(c => c.id === selectedControlId) || null;
    }

    // ------------------------------------------------------------------------
    // 4. Runtime Variables & Input Dispatchers
    // ------------------------------------------------------------------------
    let isEditMode = false;
    let selectedControlId = null;
    let ignoreNextDeselect = false;
    let inspectorCooldown = 0;

    let dragState = {
        type: null,
        targetId: null,
        startX: 0, startY: 0,
        initLeft: 0, initTop: 0
    };

    function pressKey(keyCode) {
        if (!keyCode) return;
        if (typeof window !== "undefined" && window.RuntimePackVPad && window.RuntimePackVPad.onKeyDown) {
            window.RuntimePackVPad.onKeyDown(keyCode);
        }
        if (typeof Input !== "undefined" && Input._currentState) {
            const action = KEY_ACTION_MAP[keyCode];
            if (action) Input._currentState[action] = true;
        }
        if (typeof document !== "undefined" && document.dispatchEvent && typeof KeyboardEvent !== "undefined") {
            document.dispatchEvent(new KeyboardEvent("keydown", { keyCode, bubbles: true, cancelable: true }));
        }
    }

    function releaseKey(keyCode, forceAll) {
        if (!keyCode) return;
        if (typeof window !== "undefined" && window.RuntimePackVPad && window.RuntimePackVPad.onKeyUp) {
            window.RuntimePackVPad.onKeyUp(keyCode);
        }
        if (typeof Input !== "undefined" && Input._currentState) {
            const action = KEY_ACTION_MAP[keyCode];
            if (action) Input._currentState[action] = false;
        }
        if (typeof document !== "undefined" && document.dispatchEvent && typeof KeyboardEvent !== "undefined") {
            document.dispatchEvent(new KeyboardEvent("keyup", { keyCode, bubbles: true, cancelable: true }));
        }
    }

    function openLinktree() {
        const url = "https://linktr.ee/Veldra.Works";
        try {
            if (typeof window !== "undefined" && window.RuntimePackVPad && window.RuntimePackVPad.openUrl) {
                window.RuntimePackVPad.openUrl(url);
            } else if (typeof window !== "undefined" && window.cordova && window.cordova.InAppBrowser) {
                window.cordova.InAppBrowser.open(url, "_system");
            } else if (typeof navigator !== "undefined" && navigator.app && navigator.app.loadUrl) {
                navigator.app.loadUrl(url, { openExternal: true });
            } else if (typeof window !== "undefined" && window.open) {
                window.open(url, "_system");
            } else if (typeof require === "function") {
                const nw = require("nw.gui");
                if (nw && nw.Shell) nw.Shell.openExternal(url);
            } else if (typeof window !== "undefined" && window.open) {
                window.open(url, "_blank");
            }
        } catch (e) {
            if (typeof window !== "undefined" && window.open) {
                window.open(url, "_blank");
            }
        }
    }

    function bindTap(el, callback) {
        if (!el) return;
        let lastTap = 0;
        const trigger = (e) => {
            e.stopPropagation();
            if (e.preventDefault && e.cancelable) e.preventDefault();
            const now = Date.now();
            if (now - lastTap < 220) return;
            lastTap = now;
            callback(e);
        };
        el.addEventListener("touchstart", trigger, { passive: false });
        el.addEventListener("mousedown", trigger);
        el.addEventListener("pointerdown", trigger);
        el.addEventListener("click", trigger);
    }

    function bindScrollItemTap(el, callback) {
        if (!el) return;
        let startY = 0, startX = 0, hasMoved = false;

        el.addEventListener("touchstart", (e) => {
            hasMoved = false;
            if (e.touches && e.touches[0]) {
                startX = e.touches[0].clientX;
                startY = e.touches[0].clientY;
            }
        }, { passive: true });

        el.addEventListener("touchmove", (e) => {
            if (e.touches && e.touches[0]) {
                const dy = Math.abs(e.touches[0].clientY - startY);
                const dx = Math.abs(e.touches[0].clientX - startX);
                if (dy > 8 || dx > 8) hasMoved = true;
            }
        }, { passive: true });

        el.addEventListener("touchend", (e) => {
            if (!hasMoved) {
                e.stopPropagation();
                callback(e);
            }
        }, { passive: false });

        el.addEventListener("click", (e) => {
            if (!hasMoved) {
                e.stopPropagation();
                callback(e);
            }
        });
    }

    // ------------------------------------------------------------------------
    // 5. CSS Styles Injection (With 2 Themes Support)
    // ------------------------------------------------------------------------
    function injectStyles() {
        if (document.getElementById("vpad-styles")) return;
        const style = document.createElement("style");
        style.id = "vpad-styles";
        style.textContent = `
            #vpad-root {
                position: fixed;
                top: 0; left: 0;
                width: 100vw; height: 100vh;
                pointer-events: none;
                z-index: 99999;
                user-select: none;
                -webkit-user-select: none;
                -webkit-touch-callout: none !important;
                -webkit-tap-highlight-color: transparent !important;
                outline: none !important;
                font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            }
            #vpad-root *, .vpad-btn, .vpad-control-item, .vpad-top-btn {
                -webkit-tap-highlight-color: transparent !important;
                -webkit-touch-callout: none !important;
                outline: none !important;
            }
            .vpad-btn:focus, .vpad-btn:active, .vpad-btn:focus-visible,
            .vpad-control-item:focus, .vpad-control-item:active,
            .vpad-top-btn:focus, .vpad-top-btn:active {
                outline: none !important;
                -webkit-tap-highlight-color: transparent !important;
            }

            /* Watermark Top-Right: Hidden in normal gameplay, visible only in Edit Mode */
            #fnh-watermark {
                position: fixed;
                top: max(14px, env(safe-area-inset-top, 14px));
                right: max(20px, calc(env(safe-area-inset-right, 0px) + 12px));
                pointer-events: none;
                z-index: 99999;
                display: none;
                opacity: 0;
                color: #f4f4f5;
                font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
                font-size: 11px;
                line-height: 1.35;
                text-align: right;
                font-weight: 500;
                text-shadow: 1px 1px 2px #000, 0 0 4px #000;
                transition: opacity 0.25s ease;
            }
            .vpad-edit-mode #fnh-watermark {
                display: block !important;
                opacity: 0.75 !important;
            }
            body.vpad-theme-retro #fnh-watermark {
                font-family: Consolas, "Roboto Mono", "Courier New", monospace !important;
                letter-spacing: 0.5px !important;
                color: #ffaa44 !important;
                text-shadow: 1px 1px 2px #000, 0 0 6px rgba(255, 119, 0, 0.6) !important;
            }

            /* Top Left Vertical Toolbar: Closer to Edge & 10% Smaller (34px, gap 7px) */
            #vpad-topbar {
                position: absolute;
                top: max(10px, env(safe-area-inset-top, 10px));
                left: max(12px, calc(env(safe-area-inset-left, 0px) + 6px));
                display: flex;
                flex-direction: column;
                gap: 7px;
                pointer-events: auto;
                z-index: 100002;
            }
            .vpad-top-btn {
                width: 34px; height: 34px;
                border-radius: 50%;
                background: rgba(26, 26, 32, 0.7);
                border: 1px solid rgba(255, 255, 255, 0.22);
                backdrop-filter: blur(8px);
                -webkit-backdrop-filter: blur(8px);
                color: #e4e4e7;
                display: flex; align-items: center; justify-content: center;
                cursor: pointer;
                box-shadow: 0 4px 10px rgba(0, 0, 0, 0.4);
                transition: transform 0.1s ease, background 0.15s ease, opacity 0.2s ease, border-color 0.2s;
                opacity: 0.8;
                touch-action: none;
            }
            .vpad-top-btn:active {
                transform: scale(0.9);
                background: rgba(60, 60, 75, 0.8);
            }
            .vpad-top-btn.active {
                background: #0088cc;
                border-color: #38bdf8;
                color: #fff;
                box-shadow: 0 0 10px rgba(0, 180, 255, 0.8);
                opacity: 1;
            }
            .vpad-top-btn svg {
                width: 16px; height: 16px;
                stroke: currentColor;
                fill: none;
                stroke-width: 2;
                stroke-linecap: round;
                stroke-linejoin: round;
                pointer-events: none;
            }
            #vpad-btn-cheat {
                color: #f59e0b;
            }
            #vpad-btn-link {
                color: #06b6d4;
            }

            /* Controls Container */
            #vpad-controls {
                position: absolute;
                top: 0; left: 0;
                width: 100%; height: 100%;
                pointer-events: none;
                transition: opacity 0.2s ease;
            }
            #vpad-controls.vpad-hidden {
                display: none !important;
            }

            /* Glassmorphism Buttons (Canonical Styling) */
            .vpad-btn {
                pointer-events: auto;
                user-select: none;
                -webkit-user-select: none;
                touch-action: none;
                position: absolute;
                display: flex; align-items: center; justify-content: center;
                font-weight: bold;
                color: #ffffff;
                background: linear-gradient(145deg, rgba(255, 255, 255, 0.16), rgba(20, 20, 30, 0.5));
                border: 1.5px solid rgba(255, 255, 255, 0.28);
                border-radius: 50%;
                box-shadow: 0 4px 14px rgba(0, 0, 0, 0.35);
                backdrop-filter: blur(8px);
                -webkit-backdrop-filter: blur(8px);
                cursor: pointer;
                transition: transform 0.05s ease, background 0.1s ease, border-color 0.1s;
                transform-origin: center center;
            }
            .vpad-btn:active, .vpad-btn.active {
                transform: scale(0.92);
                background: linear-gradient(145deg, rgba(255, 255, 255, 0.4), rgba(50, 50, 70, 0.7));
                border-color: #ffffff;
                box-shadow: 0 0 16px rgba(255, 255, 255, 0.6);
            }
            .vpad-btn-green {
                background: linear-gradient(145deg, rgba(46, 204, 113, 0.35), rgba(20, 40, 30, 0.55));
                border-color: rgba(46, 204, 113, 0.7);
                color: #eafff2;
            }
            .vpad-btn-green:active, .vpad-btn-green.active {
                background: #27ae60;
                border-color: #a8ffcc;
                box-shadow: 0 0 16px rgba(46, 204, 113, 0.8);
            }
            .vpad-btn-red {
                background: linear-gradient(145deg, rgba(231, 76, 60, 0.35), rgba(40, 20, 20, 0.55));
                border-color: rgba(231, 76, 60, 0.7);
                color: #ffeaea;
            }
            .vpad-btn-red:active, .vpad-btn-red.active {
                background: #c0392b;
                border-color: #ffaaaa;
                box-shadow: 0 0 16px rgba(231, 76, 60, 0.8);
            }
            .vpad-btn-yellow {
                background: linear-gradient(145deg, rgba(241, 196, 15, 0.35), rgba(40, 35, 15, 0.55));
                border-color: rgba(241, 196, 15, 0.7);
                color: #fffdea;
            }
            .vpad-btn-yellow:active, .vpad-btn-yellow.active {
                background: #d4ac0d;
                border-color: #fff4a8;
                box-shadow: 0 0 16px rgba(241, 196, 15, 0.8);
            }
            .vpad-btn-util {
                border-radius: 12px;
                background: linear-gradient(145deg, rgba(255, 255, 255, 0.12), rgba(30, 35, 45, 0.6));
                border-color: rgba(255, 255, 255, 0.22);
                font-size: 11px;
            }

            /* D-Pad 8-Direction */
            .vpad-dpad-container {
                position: absolute;
                pointer-events: auto;
                user-select: none;
                -webkit-user-select: none;
                touch-action: none;
                background: radial-gradient(circle, rgba(25, 28, 38, 0.75) 0%, rgba(10, 12, 18, 0.85) 75%);
                border: 2px solid rgba(255, 255, 255, 0.25);
                border-radius: 50%;
                box-shadow: 0 6px 18px rgba(0, 0, 0, 0.5);
                backdrop-filter: blur(8px);
                -webkit-backdrop-filter: blur(8px);
                transform-origin: center center;
            }
            .vpad-dpad-arrow {
                position: absolute;
                display: flex; align-items: center; justify-content: center;
                color: rgba(255, 255, 255, 0.7);
                font-size: 18px;
                transition: transform 0.05s ease, color 0.1s, text-shadow 0.1s;
                pointer-events: none;
            }
            .vpad-dpad-arrow.active {
                color: #4df0ff;
                text-shadow: 0 0 10px #4df0ff;
            }
            .vpad-dpad-up    { top: 6px; left: 50%; transform: translateX(-50%); }
            .vpad-dpad-down  { bottom: 6px; left: 50%; transform: translateX(-50%); }
            .vpad-dpad-left  { left: 6px; top: 50%; transform: translateY(-50%); }
            .vpad-dpad-right { right: 6px; top: 50%; transform: translateY(-50%); }

            .vpad-dpad-up.active    { transform: translateX(-50%) scale(1.22) !important; }
            .vpad-dpad-down.active  { transform: translateX(-50%) scale(1.22) !important; }
            .vpad-dpad-left.active  { transform: translateY(-50%) scale(1.22) !important; }
            .vpad-dpad-right.active { transform: translateY(-50%) scale(1.22) !important; }
            .vpad-dpad-center {
                position: absolute;
                top: 50%; left: 50%;
                transform: translate(-50%, -50%);
                width: 32%; height: 32%;
                background: rgba(255, 255, 255, 0.06);
                border-radius: 50%;
                pointer-events: none;
            }

            /* Analog Joystick */
            .vpad-joystick-container {
                position: absolute;
                pointer-events: auto;
                user-select: none;
                -webkit-user-select: none;
                touch-action: none;
                background: radial-gradient(circle, rgba(30, 35, 48, 0.75) 0%, rgba(12, 14, 22, 0.85) 75%);
                border: 2px solid rgba(255, 255, 255, 0.25);
                border-radius: 50%;
                box-shadow: 0 6px 18px rgba(0, 0, 0, 0.5);
                backdrop-filter: blur(8px);
                -webkit-backdrop-filter: blur(8px);
                transform-origin: center center;
            }
            .vpad-joystick-inner-ring {
                position: absolute;
                top: 50%; left: 50%;
                transform: translate(-50%, -50%);
                width: 50%; height: 50%;
                border: 1px dashed rgba(255, 255, 255, 0.15);
                border-radius: 50%;
                pointer-events: none;
            }
            .vpad-joystick-stick {
                position: absolute;
                top: 50%; left: 50%;
                transform: translate(-50%, -50%);
                width: 44%; height: 44%;
                background: radial-gradient(circle at 35% 35%, #4a5568, #1a202c);
                border: 2px solid rgba(255, 255, 255, 0.4);
                border-radius: 50%;
                box-shadow: 0 4px 10px rgba(0,0,0,0.6);
                pointer-events: none;
                transition: border-color 0.1s;
            }
            .vpad-joystick-container.active .vpad-joystick-stick {
                border-color: #4df0ff;
                box-shadow: 0 0 14px rgba(77, 240, 255, 0.7);
            }

            /* Edit Mode Highlights */
            .vpad-edit-mode .vpad-control-item {
                outline: 2px dashed rgba(255, 255, 255, 0.4);
                outline-offset: 3px;
                cursor: grab;
            }
            .vpad-edit-mode .vpad-control-item.vpad-selected {
                outline: 2.5px solid #4df0ff !important;
                box-shadow: 0 0 14px rgba(77, 240, 255, 0.8) !important;
            }

            /* Floating Draggable Toolbar */
            #vpad-editor-bar {
                position: fixed;
                display: none;
                align-items: center;
                gap: 6px;
                padding: 6px 12px;
                background: rgba(18, 20, 26, 0.94);
                border: 1.5px solid #4df0ff;
                backdrop-filter: blur(14px);
                -webkit-backdrop-filter: blur(14px);
                border-radius: 18px;
                pointer-events: auto;
                z-index: 100003;
                box-shadow: 0 6px 20px rgba(0, 0, 0, 0.6);
                touch-action: none;
            }
            #vpad-editor-bar.show {
                display: flex !important;
            }
            .vpad-bar-drag-handle {
                cursor: move;
                color: #4df0ff;
                font-size: 16px;
                font-weight: bold;
                padding: 2px 6px;
                user-select: none;
                touch-action: none;
            }
            .vpad-bar-btn {
                padding: 5px 10px;
                font-size: 11px;
                font-weight: 600;
                border-radius: 10px;
                border: 1px solid rgba(255, 255, 255, 0.22);
                background: rgba(38, 42, 54, 0.85);
                color: #fff;
                cursor: pointer;
                touch-action: none;
                display: flex; align-items: center; gap: 4px;
            }
            .vpad-bar-btn:active {
                transform: scale(0.93);
            }
            .vpad-bar-done {
                background: #0088cc;
                border-color: #38bdf8;
                color: #fff;
            }

            /* Popover Dropdowns (Presets & Themes) */
            .vpad-popover {
                position: fixed;
                display: none;
                background: rgba(18, 20, 26, 0.98);
                border: 1.5px solid #4df0ff;
                border-radius: 10px;
                box-shadow: 0 8px 24px rgba(0,0,0,0.8);
                z-index: 100008;
                pointer-events: auto;
                min-width: 190px;
                overflow: hidden;
            }
            .vpad-popover.show {
                display: block !important;
            }
            .vpad-popover-item {
                padding: 8px 12px;
                font-size: 12px;
                color: #ddd;
                border-bottom: 1px solid rgba(255,255,255,0.08);
                cursor: pointer;
                display: flex; align-items: center; justify-content: space-between;
            }
            .vpad-popover-item:last-child {
                border-bottom: none;
            }
            .vpad-popover-item.active {
                background: #0088cc;
                color: #fff;
                font-weight: bold;
            }

            /* Floating Inspector Property Panel */
            #vpad-prop-panel {
                position: fixed;
                width: 250px;
                background: rgba(18, 20, 26, 0.94);
                border: 1.5px solid rgba(77, 240, 255, 0.5);
                backdrop-filter: blur(14px);
                -webkit-backdrop-filter: blur(14px);
                border-radius: 12px;
                color: #fff;
                box-shadow: 0 8px 25px rgba(0, 0, 0, 0.7);
                pointer-events: auto;
                z-index: 100004;
                display: none;
                font-size: 12px;
                touch-action: none;
            }
            #vpad-prop-panel.show {
                display: block !important;
            }
            .vpad-prop-header {
                padding: 8px 12px;
                background: rgba(28, 32, 44, 0.9);
                border-top-left-radius: 10px;
                border-top-right-radius: 10px;
                display: flex; align-items: center; justify-content: space-between;
                cursor: move;
                font-weight: bold;
                color: #4df0ff;
                border-bottom: 1px solid rgba(255, 255, 255, 0.12);
                touch-action: none;
            }
            .vpad-prop-body {
                padding: 10px 12px;
            }
            .vpad-prop-row {
                margin-bottom: 10px;
            }
            .vpad-prop-label {
                display: flex; justify-content: space-between;
                color: #bbb; margin-bottom: 5px;
                font-size: 11px;
            }

            /* Touch Sliders */
            .vpad-touch-slider-track {
                position: relative;
                width: 100%; height: 18px;
                background: rgba(40, 45, 58, 0.85);
                border: 1.5px solid rgba(255, 255, 255, 0.2);
                border-radius: 9px;
                cursor: pointer;
                touch-action: none;
                margin: 4px 0;
            }
            .vpad-slider-fill {
                position: absolute;
                top: 0; left: 0; height: 100%;
                background: linear-gradient(90deg, #0088cc, #4df0ff);
                border-radius: 9px;
                pointer-events: none;
            }
            .vpad-slider-knob {
                position: absolute;
                top: 50%;
                width: 22px; height: 22px;
                border-radius: 50%;
                background: #ffffff;
                border: 2.5px solid #0099ff;
                box-shadow: 0 2px 8px rgba(0,0,0,0.7);
                transform: translate(-50%, -50%);
                pointer-events: none;
            }
            .vpad-prop-actions {
                display: flex; gap: 6px; margin-top: 10px;
            }
            .vpad-prop-btn {
                flex: 1;
                padding: 7px 6px;
                font-size: 11px;
                font-weight: 600;
                border-radius: 6px;
                border: 1px solid rgba(255,255,255,0.2);
                background: rgba(45, 50, 65, 0.85);
                color: #fff;
                text-align: center;
                cursor: pointer;
                touch-action: none;
                transition: background 0.15s, border-color 0.15s;
            }
            .vpad-prop-btn.danger {
                background: rgba(185, 30, 40, 0.85);
                border-color: #f87171;
            }
            .vpad-prop-btn.danger-confirm {
                background: #dc2626 !important;
                border-color: #fca5a5 !important;
                animation: vpad-pulse 0.8s infinite alternate;
            }
            @keyframes vpad-pulse {
                from { box-shadow: 0 0 4px #dc2626; }
                to { box-shadow: 0 0 14px #ef4444; }
            }

            /* Modal: 3-Layer Flexbox (Fixed Header, Scroll Body, Sticky Footer) */
            #vpad-key-modal {
                position: fixed;
                top: 0; left: 0;
                width: 100vw; height: 100vh;
                background: rgba(0, 0, 0, 0.8);
                backdrop-filter: blur(8px);
                -webkit-backdrop-filter: blur(8px);
                display: none;
                align-items: center; justify-content: center;
                pointer-events: auto;
                z-index: 100010;
            }
            #vpad-key-modal.show {
                display: flex !important;
            }
            .vpad-modal-box {
                width: 90%;
                max-width: 440px;
                max-height: 85vh;
                background: rgba(18, 20, 26, 0.98);
                border: 1.5px solid #4df0ff;
                border-radius: 12px;
                padding: 14px;
                color: #fff;
                display: flex;
                flex-direction: column;
                overflow: hidden;
            }
            .vpad-modal-header {
                flex-shrink: 0;
                display: flex; justify-content: space-between; align-items: center;
                font-weight: bold; color: #4df0ff; font-size: 14px;
                padding-bottom: 6px;
                border-bottom: 1px solid rgba(255,255,255,0.1);
            }
            .vpad-modal-body {
                flex: 1;
                overflow-y: auto;
                touch-action: pan-y;
                -webkit-overflow-scrolling: touch;
                padding-right: 4px;
                margin-top: 6px;
            }
            .vpad-modal-footer {
                flex-shrink: 0;
                display: flex; gap: 8px;
                margin-top: 10px;
                padding-top: 8px;
                border-top: 1px solid rgba(255,255,255,0.1);
            }
            /* Segmented Control Selector */
            .vpad-segmented-group {
                display: flex;
                gap: 4px;
                margin-top: 5px;
                background: #101218;
                padding: 3px;
                border-radius: 8px;
                border: 1px solid rgba(255, 255, 255, 0.15);
            }
            .vpad-segmented-btn {
                flex: 1;
                padding: 7px 4px;
                font-size: 11px;
                font-weight: 600;
                background: transparent;
                border: 1px solid transparent;
                border-radius: 6px;
                color: #a1a1aa;
                cursor: pointer;
                text-align: center;
                transition: all 0.15s ease;
                touch-action: none;
            }
            .vpad-segmented-btn.active {
                background: #0088cc;
                border-color: #38bdf8;
                color: #ffffff;
                box-shadow: 0 2px 8px rgba(0, 136, 204, 0.4);
            }
            body.vpad-theme-retro .vpad-segmented-group {
                background: #000000 !important;
                border: 1px solid #333 !important;
                border-radius: 2px !important;
            }
            body.vpad-theme-retro .vpad-segmented-btn {
                border-radius: 2px !important;
                color: #888 !important;
                font-family: Consolas, monospace !important;
            }
            body.vpad-theme-retro .vpad-segmented-btn.active {
                background: #ff7700 !important;
                border-color: #ffa244 !important;
                color: #000000 !important;
                font-weight: bold !important;
                box-shadow: 0 0 10px rgba(255, 119, 0, 0.5) !important;
            }

            .vpad-input-field {
                width: 100%;
                padding: 8px 10px;
                background: #12141a;
                color: #fff;
                border: 1px solid #4df0ff;
                border-radius: 6px;
                margin-top: 4px;
                font-size: 13px;
                box-sizing: border-box;
            }
            .vpad-modal-scroll-area {
                margin: 8px 0;
                max-height: 180px;
                overflow-y: auto !important;
                touch-action: pan-y !important;
                -webkit-overflow-scrolling: touch;
                border: 1.5px solid rgba(77, 240, 255, 0.3);
                border-radius: 8px;
                padding: 6px;
                background: rgba(10, 12, 16, 0.75);
            }
            .vpad-picker-grid {
                display: grid;
                grid-template-columns: repeat(3, 1fr);
                gap: 6px;
            }
            .vpad-picker-item {
                padding: 8px 4px;
                font-size: 11px;
                text-align: center;
                background: rgba(35, 40, 52, 0.85);
                border: 1px solid rgba(255, 255, 255, 0.15);
                border-radius: 6px;
                cursor: pointer;
                touch-action: pan-y;
            }
            .vpad-picker-item.active {
                background: #0088cc;
                border-color: #4df0ff;
                font-weight: bold;
                color: #fff;
            }

            /* ================================================================
               THEME 2: RETRO OSD (ORANGE TERMINAL) OVERRIDES
               ================================================================ */
            body.vpad-theme-retro #vpad-root,
            body.vpad-theme-retro #vpad-editor-bar,
            body.vpad-theme-retro #vpad-prop-panel,
            body.vpad-theme-retro .vpad-popover,
            body.vpad-theme-retro .vpad-modal-box {
                font-family: Consolas, "Roboto Mono", "Courier New", monospace !important;
            }
            body.vpad-theme-retro #vpad-editor-bar {
                background: #0a0a0c !important;
                border: 1.5px solid #ff7700 !important;
                border-radius: 2px !important;
                box-shadow: 0 0 16px rgba(255, 119, 0, 0.25) !important;
            }
            body.vpad-theme-retro .vpad-bar-drag-handle {
                color: #ff7700 !important;
            }
            body.vpad-theme-retro .vpad-bar-btn {
                background: #141418 !important;
                border: 1px solid #444 !important;
                border-radius: 2px !important;
                color: #e6e6e6 !important;
            }
            body.vpad-theme-retro .vpad-bar-done {
                background: #ff7700 !important;
                border-color: #ffa244 !important;
                color: #000 !important;
                font-weight: bold !important;
            }
            body.vpad-theme-retro .vpad-popover {
                background: #0a0a0c !important;
                border: 1.5px solid #ff7700 !important;
                border-radius: 2px !important;
            }
            body.vpad-theme-retro .vpad-popover-item.active {
                background: #ff7700 !important;
                color: #000 !important;
                font-weight: bold !important;
            }
            body.vpad-theme-retro #vpad-prop-panel {
                background: #0a0a0c !important;
                border: 1.5px solid #ff7700 !important;
                border-radius: 2px !important;
                box-shadow: 0 0 18px rgba(255, 119, 0, 0.2) !important;
            }
            body.vpad-theme-retro .vpad-prop-header {
                background: #15151b !important;
                color: #ff7700 !important;
                border-radius: 0 !important;
                border-bottom: 1px solid #333 !important;
            }
            body.vpad-theme-retro .vpad-slider-fill {
                background: #ff7700 !important;
            }
            body.vpad-theme-retro .vpad-slider-knob {
                border-color: #ff7700 !important;
            }
            body.vpad-theme-retro .vpad-prop-btn {
                background: #141418 !important;
                border: 1px solid #444 !important;
                border-radius: 2px !important;
                color: #e6e6e6 !important;
            }
            body.vpad-theme-retro .vpad-modal-box {
                background: #0a0a0c !important;
                border: 1.5px solid #ff7700 !important;
                border-radius: 2px !important;
            }
            body.vpad-theme-retro .vpad-modal-header {
                color: #ff7700 !important;
            }
            body.vpad-theme-retro .vpad-input-field {
                border-color: #ff7700 !important;
                background: #000 !important;
                border-radius: 2px !important;
            }
            body.vpad-theme-retro .vpad-picker-item.active {
                background: #ff7700 !important;
                color: #000 !important;
                border-color: #ffa244 !important;
                font-weight: bold !important;
            }
            body.vpad-theme-retro .vpad-dpad-arrow.active {
                color: #ff7700 !important;
                text-shadow: 0 0 10px #ff7700 !important;
            }
            body.vpad-theme-retro .vpad-top-btn {
                background: #0a0a0c !important;
                border-color: #333 !important;
                color: #ff7700 !important;
            }
            body.vpad-theme-retro .vpad-top-btn.active {
                border-color: #ff7700 !important;
                box-shadow: 0 0 10px rgba(255, 119, 0, 0.7) !important;
            }
            body.vpad-theme-retro #vpad-btn-cheat {
                color: #ffa244 !important;
            }
            body.vpad-theme-retro #vpad-btn-link {
                color: #ff7700 !important;
            }
        `;
        document.head.appendChild(style);
    }

    // ------------------------------------------------------------------------
    // 6. DOM Creation & Structure
    // ------------------------------------------------------------------------
    function createOverlay() {
        if (document.getElementById("vpad-root")) return;
        injectStyles();

        const root = document.createElement("div");
        root.id = "vpad-root";

        root.innerHTML = `
            <!-- Watermark Top-Right: Shown only in Layout Edit Mode -->
            <div id="fnh-watermark">
                <div>RuntimePack Overlay</div>
            </div>

            <!-- Top Left Vertical Toolbar (34px, 10% smaller, Inline SVGs) -->
            <div id="vpad-topbar">
                <div id="vpad-btn-toggle" class="vpad-top-btn" title="Toggle Gamepad">
                    <svg viewBox="0 0 24 24"><rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 12h4m-2-2v4m7-2h.01m3 0h.01"/></svg>
                </div>
                <div id="vpad-btn-gear" class="vpad-top-btn" title="Layout Editor">
                    <svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
                </div>
                <div id="vpad-btn-cheat" class="vpad-top-btn cheat-interactive" title="Cheat Menu">
                    <svg viewBox="0 0 24 24"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
                </div>
                <div id="vpad-btn-link" class="vpad-top-btn" title="Veldra Works" onclick="window.openLinktree()">
                    <svg viewBox="0 0 24 24"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>
                </div>
            </div>

            <!-- Edit Mode Floating Draggable Toolbar -->
            <div id="vpad-editor-bar">
                <div id="vpad-bar-drag" class="vpad-bar-drag-handle" title="Drag Toolbar">⋮⋮</div>
                <button id="vpad-bar-done" class="vpad-bar-btn vpad-bar-done">✓ Done</button>
                <div id="vpad-bar-preset-btn" class="vpad-bar-btn">
                    <span id="vpad-bar-preset-name">Preset 1 — Standard Default</span> ▼
                </div>
                <div id="vpad-bar-theme-btn" class="vpad-bar-btn">
                    <span id="vpad-bar-theme-name">Theme: Clean Slate</span> ▼
                </div>
                <button id="vpad-bar-add" class="vpad-bar-btn">+ Add Button</button>
                <button id="vpad-bar-reset" class="vpad-bar-btn">↺ Reset</button>
                <button id="vpad-bar-collapse" class="vpad-bar-btn" title="Collapse / Expand">─</button>
            </div>

            <!-- Preset Dropdown Popover -->
            <div id="vpad-preset-popover" class="vpad-popover">
                <div class="vpad-popover-item" data-id="preset1">Preset 1 — Standard Default</div>
                <div class="vpad-popover-item" data-id="preset2">Preset 2 — Compact</div>
                <div class="vpad-popover-item" data-id="preset3">Preset 3 — Large Touch</div>
                <div class="vpad-popover-item" data-id="preset4">Preset 4 — Minimal</div>
                <div class="vpad-popover-item" data-id="preset5">Preset 5 — Gamepad Style</div>
            </div>

            <!-- Theme Dropdown Popover -->
            <div id="vpad-theme-popover" class="vpad-popover">
                <div class="vpad-popover-item" data-id="clean">Clean Slate (Default)</div>
                <div class="vpad-popover-item" data-id="retro">Retro OSD (Orange)</div>
            </div>

            <!-- Gamepad Controls Layer -->
            <div id="vpad-controls"></div>

            <!-- Floating Inspector Property Panel -->
            <div id="vpad-prop-panel">
                <div class="vpad-prop-header" id="vpad-prop-header">
                    <span id="vpad-prop-title">Control Inspector</span>
                    <span id="vpad-prop-close" style="cursor:pointer; font-size:14px;">✕</span>
                </div>
                <div class="vpad-prop-body">
                    <!-- Scale Slider -->
                    <div class="vpad-prop-row">
                        <div class="vpad-prop-label">
                            <span>Size (Scale):</span>
                            <span id="vpad-prop-scale-val">100%</span>
                        </div>
                        <div id="vpad-slider-scale-track" class="vpad-touch-slider-track">
                            <div class="vpad-slider-fill"></div>
                            <div class="vpad-slider-knob"></div>
                        </div>
                    </div>

                    <!-- Opacity Slider -->
                    <div class="vpad-prop-row">
                        <div class="vpad-prop-label">
                            <span>Opacity:</span>
                            <span id="vpad-prop-opac-val">75%</span>
                        </div>
                        <div id="vpad-slider-opac-track" class="vpad-touch-slider-track">
                            <div class="vpad-slider-fill"></div>
                            <div class="vpad-slider-knob"></div>
                        </div>
                    </div>

                    <!-- Key Binding & Label (For Buttons) -->
                    <div class="vpad-prop-row" id="vpad-prop-binding-row">
                        <div class="vpad-prop-label">
                            <span>Label & Key:</span>
                            <span id="vpad-prop-key-name" style="color:#4df0ff; font-weight:bold;">A (Z)</span>
                        </div>
                        <button id="vpad-prop-btn-changekey" class="vpad-prop-btn" style="width:100%;">Change Key / Binding</button>
                    </div>

                    <!-- Switch Type (For D-Pad / Joystick) -->
                    <div class="vpad-prop-row" id="vpad-prop-switch-type-row" style="display:none;">
                        <button id="vpad-prop-btn-switch-type" class="vpad-prop-btn" style="width:100%;">Switch to Analog Joystick</button>
                    </div>

                    <!-- Actions: Duplicate & Double-Confirm Delete -->
                    <div class="vpad-prop-actions">
                        <button id="vpad-prop-btn-dup" class="vpad-prop-btn">Duplicate</button>
                        <button id="vpad-prop-btn-del" class="vpad-prop-btn danger">Delete</button>
                    </div>
                </div>
            </div>

            <!-- Add / Edit Control Modal Dialog (Sticky Footer) -->
            <div id="vpad-key-modal">
                <div class="vpad-modal-box">
                    <div class="vpad-modal-header">
                        <span id="vpad-modal-title">Custom Control Setup</span>
                        <span id="vpad-modal-close" style="cursor:pointer; font-size:16px;">✕</span>
                    </div>

                    <!-- Scrollable Form Body Container -->
                    <div class="vpad-modal-body">
                        <!-- Control Type Selector (Segmented Theme Buttons) -->
                        <div id="vpad-modal-type-container" style="margin-top:4px;">
                            <span style="font-size:11px; color:#bbb;">Control Type:</span>
                            <div class="vpad-segmented-group" id="vpad-modal-type-group">
                                <button type="button" class="vpad-segmented-btn active" data-value="button">Button</button>
                                <button type="button" class="vpad-segmented-btn" data-value="joystick">Joystick</button>
                                <button type="button" class="vpad-segmented-btn" data-value="dpad">D-Pad</button>
                            </div>
                        </div>

                        <!-- Button Specific Fields -->
                        <div id="vpad-modal-button-fields">
                            <div style="margin-top:8px;">
                                <span style="font-size:11px; color:#bbb;">Label (max 12 characters):</span>
                                <input type="text" id="vpad-modal-input-label" class="vpad-input-field" maxlength="12">
                            </div>

                            <div style="margin-top:8px;">
                                <span style="font-size:11px; color:#bbb;">Select Keyboard Key (scroll to browse):</span>
                                <div class="vpad-modal-scroll-area" id="vpad-modal-scroll-area">
                                    <div class="vpad-picker-grid" id="vpad-modal-key-grid"></div>
                                </div>
                            </div>
                        </div>
                    </div>

                    <!-- Sticky Footer (Guaranteed Visible on All Phone Screen Ratios) -->
                    <div class="vpad-modal-footer">
                        <button id="vpad-modal-btn-cancel" class="vpad-bar-btn" style="flex:1; justify-content:center;">Cancel</button>
                        <button id="vpad-modal-btn-apply" class="vpad-bar-btn vpad-bar-done" style="flex:1; justify-content:center;">Apply</button>
                    </div>
                </div>
            </div>
        `;

        document.body.appendChild(root);
        setupOverlayLogic();
        applyTheme(state.activeThemeId || "clean");
    }

    // ------------------------------------------------------------------------
    // 7. Touch Sliders Component
    // ------------------------------------------------------------------------
    function setupTouchSlider(trackEl, min, max, initialVal, unit, onChange) {
        if (!trackEl) return { setValue: () => {} };
        const fillEl = trackEl.querySelector(".vpad-slider-fill");
        const knobEl = trackEl.querySelector(".vpad-slider-knob");
        let isSliding = false;
        let currentVal = initialVal;

        function updateFromPointer(clientX) {
            const rect = trackEl.getBoundingClientRect();
            if (rect.width <= 0) return;
            const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
            currentVal = Math.round(min + ratio * (max - min));
            render(currentVal);
            onChange(currentVal);
        }

        function render(val) {
            const ratio = Math.max(0, Math.min(1, (val - min) / (max - min)));
            fillEl.style.width = (ratio * 100) + "%";
            knobEl.style.left = (ratio * 100) + "%";
        }

        const onStart = (e) => {
            e.stopPropagation();
            if (e.cancelable) e.preventDefault();
            isSliding = true;
            dragState.type = "slider";
            const clientX = (e.touches && e.touches[0]) ? e.touches[0].clientX : e.clientX;
            updateFromPointer(clientX);
        };

        const onMove = (e) => {
            if (!isSliding) return;
            e.stopPropagation();
            if (e.cancelable) e.preventDefault();
            const clientX = (e.touches && e.touches[0]) ? e.touches[0].clientX : e.clientX;
            updateFromPointer(clientX);
        };

        const onEnd = (e) => {
            if (!isSliding) return;
            isSliding = false;
            dragState.type = null;
            saveState();
        };

        trackEl.addEventListener("touchstart", onStart, { passive: false });
        window.addEventListener("touchmove", onMove, { passive: false });
        window.addEventListener("touchend", onEnd);
        window.addEventListener("touchcancel", onEnd);

        trackEl.addEventListener("mousedown", onStart);
        window.addEventListener("mousemove", onMove);
        window.addEventListener("mouseup", onEnd);

        render(initialVal);
        return {
            setValue: function (val) {
                currentVal = val;
                render(val);
            }
        };
    }

    // ------------------------------------------------------------------------
    // 8. Controller Render & Interaction Logic
    // ------------------------------------------------------------------------
    let scaleSliderCtrl = null;
    let opacSliderCtrl = null;

    function applyTheme(themeId) {
        const valid = THEMES[themeId] ? themeId : "clean";
        state.activeThemeId = valid;
        document.body.classList.remove("vpad-theme-clean", "vpad-theme-retro");
        document.body.classList.add("vpad-theme-" + valid);

        const themeNameEl = document.getElementById("vpad-bar-theme-name");
        if (themeNameEl) themeNameEl.textContent = "Theme: " + THEMES[valid].name;

        document.querySelectorAll("#vpad-theme-popover .vpad-popover-item").forEach(el => {
            el.classList.toggle("active", el.getAttribute("data-id") === valid);
        });

        if (typeof window.applyCheatTheme === "function") {
            window.applyCheatTheme(valid);
        }
        saveState();
    }
    if (typeof window !== "undefined") {
        window.applyVpadTheme = applyTheme;
    }

    function setupOverlayLogic() {
        const toggleBtn = document.getElementById("vpad-btn-toggle");
        const gearBtn = document.getElementById("vpad-btn-gear");
        const editorBar = document.getElementById("vpad-editor-bar");
        const barDrag = document.getElementById("vpad-bar-drag");
        const doneBtn = document.getElementById("vpad-bar-done");
        const presetBtn = document.getElementById("vpad-bar-preset-btn");
        const presetNameText = document.getElementById("vpad-bar-preset-name");
        const presetPopover = document.getElementById("vpad-preset-popover");
        const themeBtn = document.getElementById("vpad-bar-theme-btn");
        const themePopover = document.getElementById("vpad-theme-popover");
        const addBtn = document.getElementById("vpad-bar-add");
        const resetBtn = document.getElementById("vpad-bar-reset");
        const collapseBtn = document.getElementById("vpad-bar-collapse");

        const propPanel = document.getElementById("vpad-prop-panel");
        const propHeader = document.getElementById("vpad-prop-header");
        const propClose = document.getElementById("vpad-prop-close");
        const propScaleVal = document.getElementById("vpad-prop-scale-val");
        const propOpacVal = document.getElementById("vpad-prop-opac-val");
        const propChangeKey = document.getElementById("vpad-prop-btn-changekey");
        const propSwitchType = document.getElementById("vpad-prop-btn-switch-type");
        const propDup = document.getElementById("vpad-prop-btn-dup");
        const propDel = document.getElementById("vpad-prop-btn-del");

        const keyModal = document.getElementById("vpad-key-modal");
        const modalTitle = document.getElementById("vpad-modal-title");
        const modalTypeContainer = document.getElementById("vpad-modal-type-container");
        const modalTypeGroup = document.getElementById("vpad-modal-type-group");
        const modalTypeButtons = modalTypeGroup ? modalTypeGroup.querySelectorAll(".vpad-segmented-btn") : [];
        let modalCurrentType = "button";
        const modalButtonFields = document.getElementById("vpad-modal-button-fields");

        modalTypeButtons.forEach(b => {
            bindTap(b, () => {
                modalCurrentType = b.getAttribute("data-value");
                modalTypeButtons.forEach(btn => btn.classList.toggle("active", btn === b));
                modalButtonFields.style.display = (modalCurrentType === "button") ? "block" : "none";
            });
        });
        const modalInputLabel = document.getElementById("vpad-modal-input-label");
        const modalScrollArea = document.getElementById("vpad-modal-scroll-area");
        const modalKeyGrid = document.getElementById("vpad-modal-key-grid");
        const modalClose = document.getElementById("vpad-modal-close");
        const modalCancel = document.getElementById("vpad-modal-btn-cancel");
        const modalApply = document.getElementById("vpad-modal-btn-apply");

        // 1. Setup Touch Sliders
        scaleSliderCtrl = setupTouchSlider(
            document.getElementById("vpad-slider-scale-track"),
            50, 200, 100, "%",
            (val) => {
                const ctrl = getSelectedControl();
                if (!ctrl) return;
                ctrl.scale = parseFloat((val / 100).toFixed(2));
                propScaleVal.textContent = val + "%";
                updateControlDOM(ctrl);
            }
        );

        opacSliderCtrl = setupTouchSlider(
            document.getElementById("vpad-slider-opac-track"),
            10, 100, 75, "%",
            (val) => {
                const ctrl = getSelectedControl();
                if (!ctrl) return;
                ctrl.opacity = parseFloat((val / 100).toFixed(2));
                propOpacVal.textContent = val + "%";
                updateControlDOM(ctrl);
            }
        );

        // 2. Hide / Show Gamepad
        function applyGamepadHidden() {
            document.getElementById("vpad-controls").classList.toggle("vpad-hidden", state.hidden);
            if (window.RuntimePackVPad && window.RuntimePackVPad.setGamepadHidden) {
                window.RuntimePackVPad.setGamepadHidden(state.hidden);
            }
        }
        applyGamepadHidden();

        bindTap(toggleBtn, () => {
            if (isEditMode) return;
            state.hidden = !state.hidden;
            applyGamepadHidden();
            saveState();
        });

        // 3. ⚙️ Enter / Exit Edit Mode
        bindTap(gearBtn, () => {
            if (isEditMode) {
                exitEditMode();
            } else {
                enterEditMode();
            }
        });

        bindTap(doneBtn, () => {
            exitEditMode();
        });

        // 3b. 🔗 Open Linktree Venora Community
        window.openLinktree = openLinktree;

        const linkBtn = document.getElementById("vpad-btn-link");
        if (linkBtn) {
            bindTap(linkBtn, window.openLinktree);
        }

        const cheatBtn = document.getElementById("vpad-btn-cheat");
        if (cheatBtn) {
            bindTap(cheatBtn, () => {
                if (window.CheatMenu) window.CheatMenu.toggle();
            });
        }

        // 4. Preset Popover Dropdown
        let popoverJustOpened = false;
        bindTap(presetBtn, () => {
            themePopover.classList.remove("show");
            const isShown = presetPopover.classList.contains("show");
            if (isShown) {
                presetPopover.classList.remove("show");
            } else {
                const rect = presetBtn.getBoundingClientRect();
                presetPopover.style.left = Math.max(10, rect.left) + "px";
                presetPopover.style.top = (rect.bottom + 6) + "px";
                updatePresetPopoverUI();
                presetPopover.classList.add("show");
                popoverJustOpened = true;
                setTimeout(() => { popoverJustOpened = false; }, 250);
            }
        });

        function updatePresetPopoverUI() {
            presetPopover.querySelectorAll(".vpad-popover-item").forEach(item => {
                const pid = item.getAttribute("data-id");
                item.classList.toggle("active", pid === state.activePresetId);
                const prof = state.profiles[pid];
                if (prof) item.textContent = (pid === state.activePresetId ? "✓ " : "   ") + prof.name;
            });
            presetNameText.textContent = currentProfile().name;
        }

        presetPopover.querySelectorAll(".vpad-popover-item").forEach(item => {
            bindTap(item, () => {
                const newPid = item.getAttribute("data-id");
                if (newPid && newPid !== state.activePresetId) {
                    switchPreset(newPid);
                }
                presetPopover.classList.remove("show");
            });
        });

        // 4b. Theme Popover Dropdown
        bindTap(themeBtn, () => {
            presetPopover.classList.remove("show");
            const isShown = themePopover.classList.contains("show");
            if (isShown) {
                themePopover.classList.remove("show");
            } else {
                const rect = themeBtn.getBoundingClientRect();
                themePopover.style.left = Math.max(10, rect.left) + "px";
                themePopover.style.top = (rect.bottom + 6) + "px";
                themePopover.classList.add("show");
                popoverJustOpened = true;
                setTimeout(() => { popoverJustOpened = false; }, 250);
            }
        });

        themePopover.querySelectorAll(".vpad-popover-item").forEach(item => {
            bindTap(item, () => {
                const tid = item.getAttribute("data-id");
                if (tid) {
                    applyTheme(tid);
                }
                themePopover.classList.remove("show");
            });
        });

        // Close popovers when tapping outside
        function handleClosePopovers(e) {
            if (popoverJustOpened) return;
            if (!e.target.closest("#vpad-preset-popover") && !e.target.closest("#vpad-bar-preset-btn")) {
                presetPopover.classList.remove("show");
            }
            if (!e.target.closest("#vpad-theme-popover") && !e.target.closest("#vpad-bar-theme-btn")) {
                themePopover.classList.remove("show");
            }
        }
        document.addEventListener("touchstart", handleClosePopovers, { passive: true });
        document.addEventListener("mousedown", handleClosePopovers);

        // 5. Reset Current Preset
        bindTap(resetBtn, () => {
            const cur = currentProfile();
            const tmpl = PRESET_TEMPLATES[state.activePresetId];
            if (!tmpl) return;
            if (confirm(`Reset "${cur.name}" back to default layout?`)) {
                cur.controls = deepClone(tmpl.controls);
                selectedControlId = null;
                hidePropertyPanel();
                renderControls();
                saveState();
            }
        });

        // 6. Add Custom Control (+ Button / Joystick / D-Pad)
        let modalMode = "add";
        let modalSelectedKey = COMMON_KEYS[0];

        function populateKeyPicker(selectedCode) {
            modalKeyGrid.innerHTML = "";
            COMMON_KEYS.forEach(k => {
                const el = document.createElement("div");
                el.className = "vpad-picker-item" + (k.code === selectedCode ? " active" : "");
                el.textContent = k.name;
                bindScrollItemTap(el, () => {
                    modalSelectedKey = k;
                    if (modalMode === "add" && (!modalInputLabel.value || modalInputLabel.value === "DEL")) {
                        modalInputLabel.value = k.defaultLabel;
                    }
                    modalKeyGrid.querySelectorAll(".vpad-picker-item").forEach(i => i.classList.remove("active"));
                    el.classList.add("active");
                });
                modalKeyGrid.appendChild(el);
            });
        }

        modalInputLabel.addEventListener("keydown", (e) => { e.stopPropagation(); });
        modalInputLabel.addEventListener("keyup", (e) => { e.stopPropagation(); });
        modalInputLabel.addEventListener("keypress", (e) => { e.stopPropagation(); });



        bindTap(addBtn, () => {
            modalMode = "add";
            modalTitle.textContent = "Custom Control Setup";
            modalTypeContainer.style.display = "block";
            modalCurrentType = "button";
            modalTypeButtons.forEach(btn => btn.classList.toggle("active", btn.getAttribute("data-value") === "button"));
            modalButtonFields.style.display = "block";
            modalInputLabel.value = "DEL";
            modalSelectedKey = COMMON_KEYS.find(k => k.code === 8) || COMMON_KEYS[0];
            populateKeyPicker(modalSelectedKey.code);
            keyModal.classList.add("show");
        });

        // 7. Edit Button Binding & Rename
        bindTap(propChangeKey, () => {
            const ctrl = getSelectedControl();
            if (!ctrl || ctrl.type !== "button") return;
            modalMode = "edit";
            modalTitle.textContent = `Edit Button & Binding: ${ctrl.label}`;
            modalTypeContainer.style.display = "none";
            modalButtonFields.style.display = "block";
            modalInputLabel.value = ctrl.label || "";
            modalSelectedKey = COMMON_KEYS.find(k => k.code === ctrl.keyCode) || { code: ctrl.keyCode, name: KEY_NAMES[ctrl.keyCode] || "Custom", defaultLabel: ctrl.label };
            populateKeyPicker(ctrl.keyCode);
            keyModal.classList.add("show");
        });

        // Switch D-Pad <-> Joystick in Property Panel
        bindTap(propSwitchType, () => {
            const ctrl = getSelectedControl();
            if (!ctrl) return;
            if (ctrl.type === "dpad") {
                ctrl.type = "joystick";
                ctrl.name = "Analog Joystick";
            } else if (ctrl.type === "joystick") {
                ctrl.type = "dpad";
                ctrl.name = "D-Pad 8-Way";
            }
            renderControls();
            selectControl(ctrl.id);
            saveState();
        });

        bindTap(modalClose, () => keyModal.classList.remove("show"));
        bindTap(modalCancel, () => keyModal.classList.remove("show"));

        bindTap(modalApply, () => {
            ignoreNextDeselect = true;
            setTimeout(() => { ignoreNextDeselect = false; }, 350);

            if (modalMode === "add") {
                const ctype = modalCurrentType;
                let newCtrl = null;
                if (ctype === "joystick") {
                    newCtrl = {
                        id: "js_" + Date.now(),
                        type: "joystick",
                        name: "Analog Joystick",
                        cx: 0.5,
                        cy: 0.5,
                        scale: 1.0,
                        opacity: 0.75,
                        width: 146,
                        height: 146,
                        visible: true,
                        bindings: { up: 38, down: 40, left: 37, right: 39 }
                    };
                } else if (ctype === "dpad") {
                    newCtrl = {
                        id: "dpad_" + Date.now(),
                        type: "dpad",
                        name: "D-Pad 8-Way",
                        cx: 0.5,
                        cy: 0.5,
                        scale: 1.0,
                        opacity: 0.75,
                        width: 146,
                        height: 146,
                        visible: true,
                        bindings: { up: 38, down: 40, left: 37, right: 39 }
                    };
                } else {
                    const rawLabel = modalInputLabel.value.trim();
                    const label = rawLabel || modalSelectedKey.defaultLabel || "BTN";
                    newCtrl = {
                        id: "cbtn_" + Date.now(),
                        type: "button",
                        name: modalSelectedKey.name,
                        cx: 0.5,
                        cy: 0.5,
                        scale: 1.0,
                        opacity: 0.75,
                        width: 56,
                        height: 56,
                        keyCode: modalSelectedKey.code,
                        label: label.substring(0, 12),
                        style: "custom",
                        visible: true
                    };
                }
                currentProfile().controls.push(newCtrl);
                renderControls();
                selectControl(newCtrl.id);
            } else if (modalMode === "edit") {
                const ctrl = getSelectedControl();
                if (ctrl && ctrl.type === "button") {
                    const rawLabel = modalInputLabel.value.trim();
                    ctrl.label = (rawLabel || modalSelectedKey.defaultLabel || "BTN").substring(0, 12);
                    ctrl.keyCode = modalSelectedKey.code;
                    ctrl.name = modalSelectedKey.name;
                    updateControlDOM(ctrl);
                    updatePropertyPanel(ctrl);
                }
            }
            saveState();
            keyModal.classList.remove("show");
        });

        // 8. Duplicate Control
        bindTap(propDup, () => {
            if (Date.now() < inspectorCooldown) return;
            const ctrl = getSelectedControl();
            if (!ctrl) return;
            const dup = deepClone(ctrl);
            dup.id = ctrl.type + "_dup_" + Date.now();
            dup.cx = Math.min(0.92, dup.cx + 0.04);
            dup.cy = Math.min(0.92, dup.cy + 0.04);
            currentProfile().controls.push(dup);
            renderControls();
            selectControl(dup.id);
            saveState();
        });

        // 9. Delete Control with Double-Confirmation Guard
        let deleteConfirmTimer = null;
        let isDeleteConfirming = false;

        bindTap(propDel, () => {
            if (Date.now() < inspectorCooldown) return;
            const ctrl = getSelectedControl();
            if (!ctrl) return;

            if (!isDeleteConfirming) {
                isDeleteConfirming = true;
                propDel.textContent = "Confirm Delete?";
                propDel.classList.add("danger-confirm");
                if (deleteConfirmTimer) clearTimeout(deleteConfirmTimer);
                deleteConfirmTimer = setTimeout(() => {
                    isDeleteConfirming = false;
                    propDel.textContent = "Delete";
                    propDel.classList.remove("danger-confirm");
                }, 3000);
                return;
            }

            isDeleteConfirming = false;
            propDel.textContent = "Delete";
            propDel.classList.remove("danger-confirm");
            if (deleteConfirmTimer) clearTimeout(deleteConfirmTimer);

            const list = currentProfile().controls;
            const idx = list.findIndex(c => c.id === ctrl.id);
            if (idx >= 0) {
                list.splice(idx, 1);
                selectedControlId = null;
                hidePropertyPanel();
                renderControls();
                saveState();
            }
        });

        // 10. Close Property Panel
        bindTap(propClose, () => hidePropertyPanel());

        // 11. Draggable Property Panel Header (INDEPENDENT POSITION)
        setupDraggableHeader(propPanel, propHeader, (x, y) => {
            state.panelPosition = { x, y };
            saveState();
        });

        // 12. Draggable Editor Bar
        setupDraggableHeader(editorBar, barDrag, (x, y) => {
            state.toolbarPosition = { x, y };
            saveState();
        });

        // 13. Collapse / Expand Toolbar
        bindTap(collapseBtn, () => {
            state.toolbarCollapsed = !state.toolbarCollapsed;
            applyToolbarCollapsedUI();
            saveState();
        });

        function applyToolbarCollapsedUI() {
            const isCol = state.toolbarCollapsed;
            doneBtn.style.display = isCol ? "none" : "flex";
            presetBtn.style.display = isCol ? "none" : "flex";
            themeBtn.style.display = isCol ? "none" : "flex";
            addBtn.style.display = isCol ? "none" : "flex";
            resetBtn.style.display = isCol ? "none" : "flex";
            collapseBtn.textContent = isCol ? "≡" : "─";
            barDrag.textContent = isCol ? "⋮⋮ EDIT" : "⋮⋮";
        }

        applyToolbarPosition();
        applyPanelPosition();
        applyToolbarCollapsedUI();

        // 14. Tap Empty Screen Area Deselects
        function handleEmptyDeselect(e) {
            if (!isEditMode) return;
            if (ignoreNextDeselect) return;
            if (dragState.type) return;
            if (e.target.closest("#vpad-prop-panel") || e.target.closest("#vpad-editor-bar") ||
                e.target.closest("#vpad-topbar") || e.target.closest("#vpad-key-modal") ||
                e.target.closest("#vpad-preset-popover") || e.target.closest("#vpad-theme-popover") ||
                e.target.closest("#fnh-cheat-overlay") || e.target.closest(".vpad-control-item")) {
                return;
            }
            selectedControlId = null;
            hidePropertyPanel();
            document.querySelectorAll(".vpad-control-item").forEach(el => el.classList.remove("vpad-selected"));
        }
        document.addEventListener("pointerdown", handleEmptyDeselect);
        document.addEventListener("touchstart", handleEmptyDeselect, { passive: true });

        window.addEventListener("resize", () => {
            applyToolbarPosition();
            applyPanelPosition();
            renderControls();
        });

        renderControls();
    }

    function enterEditMode() {
        isEditMode = true;
        document.body.classList.add("vpad-edit-mode");
        document.getElementById("vpad-editor-bar").classList.add("show");
        document.getElementById("vpad-btn-gear").classList.add("active");
        if (window.RuntimePackVPad && window.RuntimePackVPad.setOverlayModalOpen) {
            window.RuntimePackVPad.setOverlayModalOpen(true);
        }
        renderControls();
    }

    function exitEditMode() {
        isEditMode = false;
        selectedControlId = null;
        document.body.classList.remove("vpad-edit-mode");
        document.getElementById("vpad-editor-bar").classList.remove("show");
        document.getElementById("vpad-preset-popover").classList.remove("show");
        document.getElementById("vpad-theme-popover").classList.remove("show");
        document.getElementById("vpad-btn-gear").classList.remove("active");
        hidePropertyPanel();
        if (window.RuntimePackVPad && window.RuntimePackVPad.setOverlayModalOpen) {
            window.RuntimePackVPad.setOverlayModalOpen(false);
        }
        renderControls();
        saveState();
    }

    function switchPreset(presetId) {
        if (!PRESET_TEMPLATES[presetId]) return;
        state.activePresetId = presetId;
        selectedControlId = null;
        hidePropertyPanel();
        renderControls();
        saveState();
        document.getElementById("vpad-bar-preset-name").textContent = currentProfile().name;
    }

    function showPropertyPanel(ctrl) {
        if (!ctrl) return;
        const panel = document.getElementById("vpad-prop-panel");
        updatePropertyPanel(ctrl);
        panel.classList.add("show");
        applyPanelPosition();
    }

    function hidePropertyPanel() {
        const panel = document.getElementById("vpad-prop-panel");
        if (panel) panel.classList.remove("show");
    }

    function updatePropertyPanel(ctrl) {
        if (!ctrl) return;
        document.getElementById("vpad-prop-title").textContent = ctrl.name || "Control Inspector";
        const scaleVal = Math.round((ctrl.scale || 1.0) * 100);
        const opacVal = Math.round((ctrl.opacity || 0.75) * 100);
        document.getElementById("vpad-prop-scale-val").textContent = scaleVal + "%";
        document.getElementById("vpad-prop-opac-val").textContent = opacVal + "%";
        if (scaleSliderCtrl) scaleSliderCtrl.setValue(scaleVal);
        if (opacSliderCtrl) opacSliderCtrl.setValue(opacVal);

        const bindingRow = document.getElementById("vpad-prop-binding-row");
        const switchRow = document.getElementById("vpad-prop-switch-type-row");
        const switchBtn = document.getElementById("vpad-prop-btn-switch-type");

        if (ctrl.type === "button") {
            bindingRow.style.display = "block";
            switchRow.style.display = "none";
            const keyLabel = KEY_NAMES[ctrl.keyCode] || ("Key " + ctrl.keyCode);
            document.getElementById("vpad-prop-key-name").textContent = `${ctrl.label || "BTN"} (${keyLabel})`;
        } else if (ctrl.type === "dpad" || ctrl.type === "joystick") {
            bindingRow.style.display = "none";
            switchRow.style.display = "block";
            switchBtn.textContent = ctrl.type === "dpad" ? "Switch to Analog Joystick" : "Switch to D-Pad 8-Way";
        } else {
            bindingRow.style.display = "none";
            switchRow.style.display = "none";
        }
    }

    function selectControl(id) {
        inspectorCooldown = Date.now() + 250; // 250ms ghost-tap protection
        selectedControlId = id;
        document.querySelectorAll(".vpad-control-item").forEach(el => {
            el.classList.toggle("vpad-selected", el.getAttribute("data-id") === id);
        });
        const ctrl = getSelectedControl();
        if (ctrl) showPropertyPanel(ctrl);
    }

    // ------------------------------------------------------------------------
    // 9. Independent Drag Handlers
    // ------------------------------------------------------------------------
    function applyToolbarPosition() {
        const bar = document.getElementById("vpad-editor-bar");
        if (!bar) return;
        const bw = bar.offsetWidth || 340;
        let x = state.toolbarPosition.x;
        let y = state.toolbarPosition.y;
        if (typeof x !== "number" || typeof y !== "number") {
            x = Math.max(10, Math.floor((window.innerWidth - bw) / 2));
            y = 10;
        }
        x = Math.max(10, Math.min(window.innerWidth - bw - 10, x));
        y = Math.max(5, Math.min(window.innerHeight - 50, y));
        bar.style.left = x + "px";
        bar.style.top = y + "px";
    }

    function applyPanelPosition() {
        const panel = document.getElementById("vpad-prop-panel");
        if (!panel) return;
        const pw = panel.offsetWidth || 250;
        const ph = panel.offsetHeight || 260;
        let x = state.panelPosition.x;
        let y = state.panelPosition.y;
        if (typeof x !== "number" || typeof y !== "number") {
            x = Math.max(10, window.innerWidth - pw - 14);
            y = 56;
        }
        x = Math.max(10, Math.min(window.innerWidth - pw - 10, x));
        y = Math.max(10, Math.min(window.innerHeight - ph - 10, y));
        panel.style.left = x + "px";
        panel.style.top = y + "px";
    }

    function setupDraggableHeader(containerEl, handleEl, onSavePos) {
        let isDragging = false;
        let startX = 0, startY = 0;
        let initLeft = 0, initTop = 0;

        function start(clientX, clientY) {
            isDragging = true;
            dragState.type = "panel";
            startX = clientX;
            startY = clientY;
            const rect = containerEl.getBoundingClientRect();
            initLeft = rect.left;
            initTop = rect.top;
        }

        function move(clientX, clientY) {
            if (!isDragging) return;
            const dx = clientX - startX;
            const dy = clientY - startY;
            const cw = containerEl.offsetWidth || 250;
            const ch = containerEl.offsetHeight || 160;
            let nx = Math.max(5, Math.min(window.innerWidth - cw - 5, initLeft + dx));
            let ny = Math.max(5, Math.min(window.innerHeight - ch - 5, initTop + dy));
            containerEl.style.left = nx + "px";
            containerEl.style.top = ny + "px";
        }

        function end() {
            if (!isDragging) return;
            isDragging = false;
            dragState.type = null;
            const rect = containerEl.getBoundingClientRect();
            onSavePos(Math.round(rect.left), Math.round(rect.top));
        }

        handleEl.addEventListener("touchstart", (e) => {
            if (e.touches && e.touches[0]) {
                e.stopPropagation();
                if (e.cancelable) e.preventDefault();
                start(e.touches[0].clientX, e.touches[0].clientY);
            }
        }, { passive: false });

        window.addEventListener("touchmove", (e) => {
            if (isDragging && e.touches && e.touches[0]) {
                e.stopPropagation();
                if (e.cancelable) e.preventDefault();
                move(e.touches[0].clientX, e.touches[0].clientY);
            }
        }, { passive: false });

        window.addEventListener("touchend", end);
        window.addEventListener("touchcancel", end);

        handleEl.addEventListener("mousedown", (e) => {
            e.stopPropagation();
            start(e.clientX, e.clientY);
        });
        window.addEventListener("mousemove", (e) => {
            if (isDragging) move(e.clientX, e.clientY);
        });
        window.addEventListener("mouseup", end);
    }

    // ------------------------------------------------------------------------
    // 10. Control DOM Rendering
    // ------------------------------------------------------------------------
    function renderControls() {
        const container = document.getElementById("vpad-controls");
        if (!container) return;
        container.innerHTML = "";

        const controls = currentControls();
        controls.forEach(ctrl => {
            if (!ctrl.visible) return;
            if (ctrl.type === "dpad") {
                renderDpad(ctrl, container);
            } else if (ctrl.type === "joystick") {
                renderJoystick(ctrl, container);
            } else {
                renderButton(ctrl, container);
            }
        });

        if (isEditMode && selectedControlId) {
            const el = container.querySelector(`[data-id="${selectedControlId}"]`);
            if (el) el.classList.add("vpad-selected");
        }
    }

    function updateControlDOM(ctrl) {
        const el = document.querySelector(`[data-id="${ctrl.id}"]`);
        if (!el) return;

        const w = window.innerWidth;
        const h = window.innerHeight;
        const px = ctrl.cx * w;
        const py = ctrl.cy * h;

        el.style.left = px + "px";
        el.style.top = py + "px";
        el.style.width = ctrl.width + "px";
        el.style.height = ctrl.height + "px";
        el.style.marginLeft = (-ctrl.width / 2) + "px";
        el.style.marginTop = (-ctrl.height / 2) + "px";
        el.style.transform = `scale(${ctrl.scale || 1.0})`;
        el.style.opacity = ctrl.opacity || 0.75;

        if (ctrl.type === "button") {
            el.textContent = ctrl.label || "BTN";
            el.style.fontSize = Math.max(10, Math.floor(ctrl.height * 0.32)) + "px";
        }
    }

    function renderButton(ctrl, container) {
        const btn = document.createElement("div");
        btn.className = "vpad-btn vpad-control-item";
        if (ctrl.style === "green") btn.classList.add("vpad-btn-green");
        else if (ctrl.style === "red") btn.classList.add("vpad-btn-red");
        else if (ctrl.style === "yellow") btn.classList.add("vpad-btn-yellow");
        else if (ctrl.style === "util") btn.classList.add("vpad-btn-util");

        btn.setAttribute("data-id", ctrl.id);
        container.appendChild(btn);
        updateControlDOM(ctrl);
        setupDirectManipulation(btn, ctrl);
    }

    function renderDpad(ctrl, container) {
        const dpad = document.createElement("div");
        dpad.className = "vpad-dpad-container vpad-control-item";
        dpad.setAttribute("data-id", ctrl.id);
        dpad.innerHTML = `
            <div class="vpad-dpad-arrow vpad-dpad-up">▲</div>
            <div class="vpad-dpad-arrow vpad-dpad-down">▼</div>
            <div class="vpad-dpad-arrow vpad-dpad-left">◀</div>
            <div class="vpad-dpad-arrow vpad-dpad-right">▶</div>
            <div class="vpad-dpad-center"></div>
        `;
        container.appendChild(dpad);
        updateControlDOM(ctrl);
        setupDirectManipulation(dpad, ctrl);
        setupDpadTouch(dpad, ctrl);
    }

    function renderJoystick(ctrl, container) {
        const js = document.createElement("div");
        js.className = "vpad-joystick-container vpad-control-item";
        js.setAttribute("data-id", ctrl.id);
        js.innerHTML = `
            <div class="vpad-joystick-inner-ring"></div>
            <div class="vpad-joystick-stick"></div>
        `;
        container.appendChild(js);
        updateControlDOM(ctrl);
        setupDirectManipulation(js, ctrl);
        setupJoystickTouch(js, ctrl);
    }

    // ------------------------------------------------------------------------
    // 11. Direct Manipulation: Drag (1 Finger) & Pinch-to-Scale (2 Fingers)
    // ------------------------------------------------------------------------
    function setupDirectManipulation(el, ctrl) {
        let isDragging = false;
        let isPinching = false;
        let startX = 0, startY = 0;
        let origCx = 0, origCy = 0;
        let initialPinchDist = 0;
        let initialScale = 1.0;
        let buttonActiveTouchId = null;
        let isButtonMouseDown = false;

        el.addEventListener("pointerdown", (e) => e.stopPropagation());

        el.addEventListener("touchstart", (e) => {
            if (!isEditMode) {
                if (ctrl.type === "button") {
                    e.stopPropagation();
                    if (buttonActiveTouchId === null && e.changedTouches && e.changedTouches.length > 0) {
                        buttonActiveTouchId = e.changedTouches[0].identifier;
                        btnOnDown(el, ctrl);
                    }
                }
                return;
            }
            e.stopPropagation();
            if (e.cancelable) e.preventDefault();

            if (e.touches.length === 2) {
                isPinching = true;
                isDragging = false;
                const t1 = e.touches[0];
                const t2 = e.touches[1];
                initialPinchDist = Math.hypot(t2.clientX - t1.clientX, t2.clientY - t1.clientY);
                initialScale = ctrl.scale || 1.0;
                return;
            }

            if (e.touches.length === 1) {
                dragState.type = "control";
                dragState.targetId = ctrl.id;
                selectControl(ctrl.id);

                isDragging = true;
                isPinching = false;
                const t = e.touches[0];
                startX = t.clientX;
                startY = t.clientY;
                origCx = ctrl.cx;
                origCy = ctrl.cy;
            }
        }, { passive: false });

        window.addEventListener("touchmove", (e) => {
            if (!isEditMode || selectedControlId !== ctrl.id) return;

            if (isPinching && e.touches.length === 2) {
                e.stopPropagation();
                if (e.cancelable) e.preventDefault();
                const t1 = e.touches[0];
                const t2 = e.touches[1];
                const curDist = Math.hypot(t2.clientX - t1.clientX, t2.clientY - t1.clientY);
                if (initialPinchDist > 0) {
                    const factor = curDist / initialPinchDist;
                    const newScale = Math.max(0.5, Math.min(2.0, initialScale * factor));
                    ctrl.scale = parseFloat(newScale.toFixed(2));
                    updateControlDOM(ctrl);
                    const scaleVal = Math.round(ctrl.scale * 100);
                    document.getElementById("vpad-prop-scale-val").textContent = scaleVal + "%";
                    if (scaleSliderCtrl) scaleSliderCtrl.setValue(scaleVal);
                }
                return;
            }

            if (isDragging && e.touches.length === 1 && dragState.type === "control") {
                e.stopPropagation();
                if (e.cancelable) e.preventDefault();
                const t = e.touches[0];
                const dx = t.clientX - startX;
                const dy = t.clientY - startY;

                const newCx = Math.max(0.04, Math.min(0.96, origCx + dx / window.innerWidth));
                const newCy = Math.max(0.04, Math.min(0.96, origCy + dy / window.innerHeight));

                ctrl.cx = parseFloat(newCx.toFixed(3));
                ctrl.cy = parseFloat(newCy.toFixed(3));
                updateControlDOM(ctrl);
            }
        }, { passive: false });

        const handleTouchEnd = (e) => {
            if (!isEditMode) {
                if (ctrl.type === "button" && buttonActiveTouchId !== null) {
                    if (e.changedTouches) {
                        for (let i = 0; i < e.changedTouches.length; i++) {
                            if (e.changedTouches[i].identifier === buttonActiveTouchId) {
                                buttonActiveTouchId = null;
                                btnOnUp(el, ctrl);
                                break;
                            }
                        }
                    }
                }
                return;
            }
            if (isPinching && e.touches.length < 2) isPinching = false;
            if (isDragging && e.touches.length === 0) {
                isDragging = false;
                dragState.type = null;
                saveState();
            }
        };
        window.addEventListener("touchend", handleTouchEnd);
        window.addEventListener("touchcancel", handleTouchEnd);

        el.addEventListener("mousedown", (e) => {
            if (!isEditMode) {
                if (ctrl.type === "button") {
                    isButtonMouseDown = true;
                    btnOnDown(el, ctrl);
                }
                return;
            }
            e.stopPropagation();
            dragState.type = "control";
            dragState.targetId = ctrl.id;
            selectControl(ctrl.id);

            isDragging = true;
            isPinching = false;
            startX = e.clientX;
            startY = e.clientY;
            origCx = ctrl.cx;
            origCy = ctrl.cy;
        });

        window.addEventListener("mousemove", (e) => {
            if (!isEditMode || selectedControlId !== ctrl.id || !isDragging || dragState.type !== "control") return;
            e.preventDefault();
            const dx = e.clientX - startX;
            const dy = e.clientY - startY;

            const newCx = Math.max(0.04, Math.min(0.96, origCx + dx / window.innerWidth));
            const newCy = Math.max(0.04, Math.min(0.96, origCy + dy / window.innerHeight));

            ctrl.cx = parseFloat(newCx.toFixed(3));
            ctrl.cy = parseFloat(newCy.toFixed(3));
            updateControlDOM(ctrl);
        });

        window.addEventListener("mouseup", () => {
            if (!isEditMode) {
                if (ctrl.type === "button" && isButtonMouseDown) {
                    isButtonMouseDown = false;
                    btnOnUp(el, ctrl);
                }
                return;
            }
            if (isDragging) {
                isDragging = false;
                dragState.type = null;
                saveState();
            }
        });
    }

    // ------------------------------------------------------------------------
    // 12. Gameplay D-Pad, Joystick & Button Handlers
    // ------------------------------------------------------------------------
    function btnOnDown(btn, ctrl) {
        btn.classList.add("active");
        pressKey(ctrl.keyCode);
    }

    function btnOnUp(btn, ctrl) {
        btn.classList.remove("active");
        releaseKey(ctrl.keyCode, false);
    }

    function setupDpadTouch(dpad, ctrl) {
        let activeTouchId = null;
        const currentPressedKeys = new Set();

        const arrowUp = dpad.querySelector(".vpad-dpad-up");
        const arrowDown = dpad.querySelector(".vpad-dpad-down");
        const arrowLeft = dpad.querySelector(".vpad-dpad-left");
        const arrowRight = dpad.querySelector(".vpad-dpad-right");

        function updateDirection(clientX, clientY) {
            const rect = dpad.getBoundingClientRect();
            const centerX = rect.left + rect.width / 2;
            const centerY = rect.top + rect.height / 2;

            const dx = clientX - centerX;
            const dy = clientY - centerY;
            const distance = Math.hypot(dx, dy);

            const deadzone = rect.width * 0.15;
            if (distance < deadzone) {
                clearArrows();
                return;
            }

            let angle = Math.atan2(dy, dx) * (180 / Math.PI);
            if (angle < 0) angle += 360;

            const isUp = angle >= 202.5 && angle <= 337.5;
            const isDown = angle >= 22.5 && angle <= 157.5;
            const isLeft = angle >= 112.5 && angle <= 247.5;
            const isRight = angle <= 67.5 || angle >= 292.5;

            const b = ctrl.bindings || { up: 38, down: 40, left: 37, right: 39 };

            setArrow(arrowUp, b.up, isUp);
            setArrow(arrowDown, b.down, isDown);
            setArrow(arrowLeft, b.left, isLeft);
            setArrow(arrowRight, b.right, isRight);
        }

        function setArrow(arrowEl, keyCode, shouldPress) {
            if (shouldPress) {
                if (!currentPressedKeys.has(keyCode)) {
                    currentPressedKeys.add(keyCode);
                    if (arrowEl) arrowEl.classList.add("active");
                    pressKey(keyCode);
                }
            } else {
                if (currentPressedKeys.has(keyCode)) {
                    currentPressedKeys.delete(keyCode);
                    if (arrowEl) arrowEl.classList.remove("active");
                    releaseKey(keyCode, true);
                }
            }
        }

        function clearArrows() {
            currentPressedKeys.forEach(k => releaseKey(k, true));
            currentPressedKeys.clear();
            [arrowUp, arrowDown, arrowLeft, arrowRight].forEach(a => a && a.classList.remove("active"));
        }

        dpad.addEventListener("touchstart", (e) => {
            if (isEditMode) return;
            e.stopPropagation();
            e.preventDefault();
            const touch = e.changedTouches[0];
            activeTouchId = touch.identifier;
            updateDirection(touch.clientX, touch.clientY);
        }, { passive: false });

        window.addEventListener("touchmove", (e) => {
            if (isEditMode || activeTouchId === null) return;
            for (let i = 0; i < e.changedTouches.length; i++) {
                if (e.changedTouches[i].identifier === activeTouchId) {
                    e.preventDefault();
                    updateDirection(e.changedTouches[i].clientX, e.changedTouches[i].clientY);
                    break;
                }
            }
        }, { passive: false });

        const endTouch = (e) => {
            if (isEditMode || activeTouchId === null) return;
            for (let i = 0; i < e.changedTouches.length; i++) {
                if (e.changedTouches[i].identifier === activeTouchId) {
                    activeTouchId = null;
                    clearArrows();
                    break;
                }
            }
        };
        window.addEventListener("touchend", endTouch);
        window.addEventListener("touchcancel", endTouch);

        // Mouse Support
        let isMouseDown = false;
        dpad.addEventListener("mousedown", (e) => {
            if (isEditMode) return;
            e.stopPropagation();
            isMouseDown = true;
            updateDirection(e.clientX, e.clientY);
        });
        window.addEventListener("mousemove", (e) => {
            if (isEditMode || !isMouseDown) return;
            updateDirection(e.clientX, e.clientY);
        });
        window.addEventListener("mouseup", () => {
            if (isEditMode || !isMouseDown) return;
            isMouseDown = false;
            clearArrows();
        });
    }

    function setupJoystickTouch(jsContainer, ctrl) {
        let activeTouchId = null;
        const currentPressedKeys = new Set();
        const stick = jsContainer.querySelector(".vpad-joystick-stick");

        function updateJoystick(clientX, clientY) {
            const rect = jsContainer.getBoundingClientRect();
            const centerX = rect.left + rect.width / 2;
            const centerY = rect.top + rect.height / 2;

            const dx = clientX - centerX;
            const dy = clientY - centerY;
            const distance = Math.hypot(dx, dy);
            const maxRadius = (rect.width / 2) * 0.65;

            const clampedDist = Math.min(distance, maxRadius);
            const angleRad = Math.atan2(dy, dx);
            const stickX = Math.cos(angleRad) * clampedDist;
            const stickY = Math.sin(angleRad) * clampedDist;

            stick.style.transform = `translate(calc(-50% + ${stickX}px), calc(-50% + ${stickY}px))`;
            jsContainer.classList.add("active");

            const deadzone = rect.width * 0.12;
            if (distance < deadzone) {
                clearKeys();
                return;
            }

            let angleDeg = angleRad * (180 / Math.PI);
            if (angleDeg < 0) angleDeg += 360;

            const isUp = angleDeg >= 202.5 && angleDeg <= 337.5;
            const isDown = angleDeg >= 22.5 && angleDeg <= 157.5;
            const isLeft = angleDeg >= 112.5 && angleDeg <= 247.5;
            const isRight = angleDeg <= 67.5 || angleDeg >= 292.5;

            const b = ctrl.bindings || { up: 38, down: 40, left: 37, right: 39 };

            setKey(b.up, isUp);
            setKey(b.down, isDown);
            setKey(b.left, isLeft);
            setKey(b.right, isRight);
        }

        function setKey(keyCode, shouldPress) {
            if (shouldPress) {
                if (!currentPressedKeys.has(keyCode)) {
                    currentPressedKeys.add(keyCode);
                    pressKey(keyCode);
                }
            } else {
                if (currentPressedKeys.has(keyCode)) {
                    currentPressedKeys.delete(keyCode);
                    releaseKey(keyCode, true);
                }
            }
        }

        function clearKeys() {
            currentPressedKeys.forEach(k => releaseKey(k, true));
            currentPressedKeys.clear();
        }

        function resetStick() {
            stick.style.transform = "translate(-50%, -50%)";
            jsContainer.classList.remove("active");
            clearKeys();
        }

        jsContainer.addEventListener("touchstart", (e) => {
            if (isEditMode) return;
            e.stopPropagation();
            e.preventDefault();
            const touch = e.changedTouches[0];
            activeTouchId = touch.identifier;
            updateJoystick(touch.clientX, touch.clientY);
        }, { passive: false });

        window.addEventListener("touchmove", (e) => {
            if (isEditMode || activeTouchId === null) return;
            for (let i = 0; i < e.changedTouches.length; i++) {
                if (e.changedTouches[i].identifier === activeTouchId) {
                    e.preventDefault();
                    updateJoystick(e.changedTouches[i].clientX, e.changedTouches[i].clientY);
                    break;
                }
            }
        }, { passive: false });

        const endTouch = (e) => {
            if (isEditMode || activeTouchId === null) return;
            for (let i = 0; i < e.changedTouches.length; i++) {
                if (e.changedTouches[i].identifier === activeTouchId) {
                    activeTouchId = null;
                    resetStick();
                    break;
                }
            }
        };
        window.addEventListener("touchend", endTouch);
        window.addEventListener("touchcancel", endTouch);

        // Mouse Support
        let isMouseDown = false;
        jsContainer.addEventListener("mousedown", (e) => {
            if (isEditMode) return;
            e.stopPropagation();
            isMouseDown = true;
            updateJoystick(e.clientX, e.clientY);
        });
        window.addEventListener("mousemove", (e) => {
            if (isEditMode || !isMouseDown) return;
            updateJoystick(e.clientX, e.clientY);
        });
        window.addEventListener("mouseup", () => {
            if (isEditMode || !isMouseDown) return;
            isMouseDown = false;
            resetStick();
        });
    }

    if (typeof window !== "undefined") {
        window.pressKey = pressKey;
        window.releaseKey = releaseKey;
        window.openLinktree = openLinktree;
    }

    if (typeof module !== "undefined" && module.exports) {
        module.exports = { pressKey, releaseKey, openLinktree, setupDirectManipulation, setupDpadTouch };
    }

    // ------------------------------------------------------------------------
    // 13. Auto-Start
    // ------------------------------------------------------------------------
    if (typeof document !== "undefined") {
        if (document.readyState === "complete" || document.readyState === "interactive") {
            createOverlay();
        } else {
            document.addEventListener("DOMContentLoaded", createOverlay);
        }
    }
})();
