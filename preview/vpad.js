// ============================================================================
// Direct In-Game Controller Layout Editor (Winlator / JoiPlay Style Overlay)
// for RPG Maker MV & MZ Android Ports.
//
// Modernized iOS-Style Spring Physics & 3-State Floating Toolbar Architecture
// Themes: Sleek Cyber Glass, Minimalist Matte Tactile, Retro OSD
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

    // Hook TouchInput RPG Maker agar tidak memblokir event touch pada UI overlay editor
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
                event.target.closest(".cheat-interactive") ||
                event.target.closest("#rp-diag-modal") ||
                event.target.closest("#vpad-btn-diag") ||
                event.target.closest(".diag-interactive")
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

    // Notify native host whether overlay text field currently holds focus
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
        if (typeof window !== "undefined" && typeof window.addEventListener === "function") {
            window.addEventListener("blur", () => notify(false));
        }
        document.addEventListener("visibilitychange", () => {
            if (document.hidden) notify(false);
        });
    }
    setupOverlayTextFocusBridge();

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
    // 2. The 5 Presets Templates
    // ------------------------------------------------------------------------
    const TEMPLATE_PRESET_1 = [
        { id: "dpad", type: "dpad", name: "D-Pad 8-Way", cx: 0.11, cy: 0.75, scale: 1.0, opacity: 0.75, width: 146, height: 146, visible: true, bindings: { up: 38, down: 40, left: 37, right: 39 } },
        { id: "btn_ok", type: "button", name: "A (OK / Z)", cx: 0.89, cy: 0.84, scale: 1.0, opacity: 0.75, width: 66, height: 66, keyCode: 90, label: "A", style: "green", visible: true },
        { id: "btn_cancel", type: "button", name: "B (Cancel / X)", cx: 0.82, cy: 0.92, scale: 1.0, opacity: 0.75, width: 60, height: 60, keyCode: 88, label: "B", style: "red", visible: true },
        { id: "btn_dash", type: "button", name: "DASH (Shift)", cx: 0.82, cy: 0.76, scale: 1.0, opacity: 0.75, width: 54, height: 54, keyCode: 16, label: "DASH", style: "yellow", visible: true },
        { id: "btn_menu", type: "button", name: "MENU (Escape)", cx: 0.89, cy: 0.69, scale: 1.0, opacity: 0.75, width: 52, height: 52, keyCode: 27, label: "MENU", style: "default", visible: true },
        { id: "btn_slot_l", type: "button", name: "L (Q / Slot Left)", cx: 0.41, cy: 0.045, scale: 1.0, opacity: 0.75, width: 51, height: 32, keyCode: 81, label: "◀ Q", style: "util", visible: true },
        { id: "btn_skip", type: "button", name: "SKIP (Ctrl)", cx: 0.50, cy: 0.045, scale: 1.0, opacity: 0.75, width: 51, height: 32, keyCode: 17, label: "SKIP", style: "util", visible: true },
        { id: "btn_slot_r", type: "button", name: "R (E / Slot Right)", cx: 0.59, cy: 0.045, scale: 1.0, opacity: 0.75, width: 51, height: 32, keyCode: 69, label: "E ▶", style: "util", visible: true }
    ];

    const TEMPLATE_PRESET_2 = [
        { id: "dpad", type: "dpad", name: "D-Pad 8-Way", cx: 0.10, cy: 0.77, scale: 0.82, opacity: 0.55, width: 140, height: 140, visible: true, bindings: { up: 38, down: 40, left: 37, right: 39 } },
        { id: "btn_ok", type: "button", name: "A (OK / Z)", cx: 0.90, cy: 0.86, scale: 0.85, opacity: 0.55, width: 66, height: 66, keyCode: 90, label: "A", style: "green", visible: true },
        { id: "btn_cancel", type: "button", name: "B (Cancel / X)", cx: 0.84, cy: 0.93, scale: 0.85, opacity: 0.55, width: 60, height: 60, keyCode: 88, label: "B", style: "red", visible: true },
        { id: "btn_dash", type: "button", name: "DASH (Shift)", cx: 0.84, cy: 0.79, scale: 0.85, opacity: 0.55, width: 54, height: 54, keyCode: 16, label: "DASH", style: "yellow", visible: true },
        { id: "btn_menu", type: "button", name: "MENU (Escape)", cx: 0.90, cy: 0.72, scale: 0.85, opacity: 0.55, width: 52, height: 52, keyCode: 27, label: "MENU", style: "default", visible: true },
        { id: "btn_slot_l", type: "button", name: "L (Q)", cx: 0.41, cy: 0.035, scale: 0.85, opacity: 0.55, width: 51, height: 32, keyCode: 81, label: "◀ Q", style: "util", visible: true },
        { id: "btn_skip", type: "button", name: "SKIP (Ctrl)", cx: 0.50, cy: 0.035, scale: 0.85, opacity: 0.55, width: 51, height: 32, keyCode: 17, label: "SKIP", style: "util", visible: true },
        { id: "btn_slot_r", type: "button", name: "R (E)", cx: 0.59, cy: 0.035, scale: 0.85, opacity: 0.55, width: 51, height: 32, keyCode: 69, label: "E ▶", style: "util", visible: true }
    ];

    const TEMPLATE_PRESET_3 = [
        { id: "dpad", type: "dpad", name: "D-Pad 8-Way", cx: 0.13, cy: 0.72, scale: 1.15, opacity: 0.80, width: 154, height: 154, visible: true, bindings: { up: 38, down: 40, left: 37, right: 39 } },
        { id: "btn_ok", type: "button", name: "A (OK / Z)", cx: 0.92, cy: 0.82, scale: 1.25, opacity: 0.80, width: 66, height: 66, keyCode: 90, label: "A", style: "green", visible: true },
        { id: "btn_cancel", type: "button", name: "B (Cancel / X)", cx: 0.83, cy: 0.91, scale: 1.25, opacity: 0.80, width: 60, height: 60, keyCode: 88, label: "B", style: "red", visible: true },
        { id: "btn_dash", type: "button", name: "DASH (Shift)", cx: 0.83, cy: 0.72, scale: 1.20, opacity: 0.80, width: 54, height: 54, keyCode: 16, label: "DASH", style: "yellow", visible: true },
        { id: "btn_menu", type: "button", name: "MENU (Escape)", cx: 0.92, cy: 0.63, scale: 1.20, opacity: 0.80, width: 52, height: 52, keyCode: 27, label: "MENU", style: "default", visible: true },
        { id: "btn_slot_l", type: "button", name: "L (Q)", cx: 0.42, cy: 0.05, scale: 1.20, opacity: 0.80, width: 54, height: 34, keyCode: 81, label: "◀ Q", style: "util", visible: true },
        { id: "btn_skip", type: "button", name: "SKIP (Ctrl)", cx: 0.50, cy: 0.05, scale: 1.20, opacity: 0.80, width: 54, height: 34, keyCode: 17, label: "SKIP", style: "util", visible: true },
        { id: "btn_slot_r", type: "button", name: "R (E)", cx: 0.58, cy: 0.05, scale: 1.20, opacity: 0.80, width: 54, height: 34, keyCode: 69, label: "E ▶", style: "util", visible: true }
    ];

    const TEMPLATE_PRESET_4 = [
        { id: "dpad", type: "dpad", name: "D-Pad 8-Way", cx: 0.11, cy: 0.75, scale: 1.0, opacity: 0.65, width: 146, height: 146, visible: true, bindings: { up: 38, down: 40, left: 37, right: 39 } },
        { id: "btn_ok", type: "button", name: "A (OK)", cx: 0.94, cy: 0.85, scale: 1.05, opacity: 0.70, width: 66, height: 66, keyCode: 90, label: "A", style: "green", visible: true },
        { id: "btn_cancel", type: "button", name: "B (Cancel)", cx: 0.86, cy: 0.92, scale: 1.0, opacity: 0.70, width: 60, height: 60, keyCode: 88, label: "B", style: "red", visible: true }
    ];

    const TEMPLATE_PRESET_5 = [
        { id: "joystick", type: "joystick", name: "Analog Joystick", cx: 0.11, cy: 0.75, scale: 1.0, opacity: 0.75, width: 146, height: 146, visible: true, bindings: { up: 38, down: 40, left: 37, right: 39 } },
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
        matte: { id: "matte", name: "Minimalist Matte" },
        clean: { id: "clean", name: "Liquid Glass" },
        retro: { id: "retro", name: "Retro OSD" },
        liquid_glass: { id: "liquid_glass", name: "Liquid Glass" },
        retro_osd: { id: "retro_osd", name: "Retro OSD" }
    };

    // ------------------------------------------------------------------------
    // 3. Persistent State Management
    // ------------------------------------------------------------------------
    const STORAGE_KEY = "vpad_controller_profiles_v11";

    let state = {
        activePresetId: "preset1",
        activeThemeId: "matte",
        touchRipple: true,
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
                    state.activeThemeId = (parsed.activeThemeId && THEMES[parsed.activeThemeId]) ? parsed.activeThemeId : "matte";
                    state.touchRipple = (typeof parsed.touchRipple === "boolean") ? parsed.touchRipple : true;
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

    // 3-State Floating Toolbar Cycle:
    // State 1: Default / Idle (Single Gamepad icon, submenu collapsed)
    // State 2: Expanded (Menu opens showing Cheat, Link, Pad, Gear, Diag; Trigger shows ^)
    // State 3: Standby (Submenu collapses; Trigger shows Eye)
    // State 3 -> State 1: Clicking Eye hides all gamepad controls; Trigger returns to Gamepad icon.
    let toolbarCycleState = 1;

    let dragState = {
        type: null,
        targetId: null,
        startX: 0, startY: 0,
        initLeft: 0, initTop: 0
    };

    const modalRegistry = (typeof window !== "undefined" && window.RuntimePackModalRegistry instanceof Set)
        ? window.RuntimePackModalRegistry
        : new Set();

    function normalizePanelId(id) {
        if (!id) return "";
        if (id === "diagnostic") return "diag";
        if (id === "custom-control") return "custom_control";
        return id;
    }

    function syncOverlayModalState() {
        const reg = (typeof window !== "undefined" && window.RuntimePackModalRegistry instanceof Set)
            ? window.RuntimePackModalRegistry
            : modalRegistry;
        if (typeof window !== "undefined" && window.RuntimePackVPad && window.RuntimePackVPad.setOverlayModalOpen) {
            window.RuntimePackVPad.setOverlayModalOpen(reg.size > 0);
        }
    }

    function setModalState(id, isOpen) {
        if (!id) return;
        const normId = normalizePanelId(id);
        const reg = (typeof window !== "undefined" && window.RuntimePackModalRegistry instanceof Set)
            ? window.RuntimePackModalRegistry
            : modalRegistry;
        const previousSize = reg.size;
        if (isOpen) {
            reg.add(normId);
        } else {
            reg.delete(normId);
            if (normId !== id) reg.delete(id);
            if (normId === "diag") reg.delete("diagnostic");
            if (normId === "custom_control") reg.delete("custom-control");
        }
        if (reg.size !== previousSize) {
            syncOverlayModalState();
        }
    }

    function registerModal(id, isOpen) {
        setModalState(id, isOpen !== false);
    }

    if (typeof window !== "undefined") {
        window.RuntimePackModalRegistry = modalRegistry;
        window.setModalState = setModalState;
        window.registerModal = registerModal;
        window.RuntimePackOverlayPanels = {
            open: (id) => setModalState(id, true),
            close: (id) => setModalState(id, false),
            size: () => {
                const reg = (window.RuntimePackModalRegistry instanceof Set) ? window.RuntimePackModalRegistry : modalRegistry;
                return reg.size;
            }
        };
    }

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

    let collapseGuardTimestamp = 0;

    function bindTap(el, callback) {
        if (!el) return;
        let activePointerId = null;
        let startX = 0, startY = 0;
        let isPressed = false;
        let hasPointerMoved = false;
        let lastHandledTime = 0;

        const trigger = (e) => {
            if (Date.now() < collapseGuardTimestamp) return;
            e.stopPropagation();
            if (e.preventDefault && e.cancelable) e.preventDefault();
            callback(e);
        };

        if (typeof window !== "undefined" && window.PointerEvent) {
            el.addEventListener("pointerdown", (e) => {
                if (Date.now() < collapseGuardTimestamp) {
                    e.stopPropagation();
                    if (e.preventDefault && e.cancelable) e.preventDefault();
                    return;
                }
                activePointerId = e.pointerId;
                startX = e.clientX;
                startY = e.clientY;
                isPressed = true;
                hasPointerMoved = false;
                el.classList.add("vpad-btn-pressed");
                try {
                    if (el.setPointerCapture) el.setPointerCapture(e.pointerId);
                } catch (_) {}
                e.stopPropagation();
                if (e.preventDefault && e.cancelable) e.preventDefault();
            });

            el.addEventListener("pointermove", (e) => {
                if (!isPressed || e.pointerId !== activePointerId) return;
                const dx = Math.abs(e.clientX - startX);
                const dy = Math.abs(e.clientY - startY);
                if (dx > 10 || dy > 10) {
                    hasPointerMoved = true;
                    isPressed = false;
                    el.classList.remove("vpad-btn-pressed");
                }
            });

            const onPointerEnd = (e) => {
                if (e.pointerId !== activePointerId) return;
                try {
                    if (el.releasePointerCapture) el.releasePointerCapture(e.pointerId);
                } catch (_) {}
                const wasValid = isPressed && !hasPointerMoved;
                isPressed = false;
                activePointerId = null;
                el.classList.remove("vpad-btn-pressed");

                if (wasValid) {
                    const dx = Math.abs(e.clientX - startX);
                    const dy = Math.abs(e.clientY - startY);
                    if (dx <= 10 && dy <= 10) {
                        lastHandledTime = Date.now();
                        trigger(e);
                    }
                }
            };

            el.addEventListener("pointerup", onPointerEnd);
            el.addEventListener("pointercancel", (e) => {
                if (e.pointerId === activePointerId) {
                    try {
                        if (el.releasePointerCapture) el.releasePointerCapture(e.pointerId);
                    } catch (_) {}
                    isPressed = false;
                    activePointerId = null;
                    el.classList.remove("vpad-btn-pressed");
                }
            });

            el.addEventListener("click", (e) => {
                if (Date.now() - lastHandledTime < 500) {
                    e.stopPropagation();
                    if (e.preventDefault && e.cancelable) e.preventDefault();
                }
            });
            el.addEventListener("click", (e) => {
                e.stopPropagation();
                if (e.preventDefault && e.cancelable) e.preventDefault();
                isPressed = false;
                activePointerId = null;
                el.classList.remove("vpad-btn-pressed");
                if (hasPointerMoved) {
                    hasPointerMoved = false;
                    return;
                }
                if (Date.now() - lastHandledTime < 500) {
                    return;
                }
                lastHandledTime = Date.now();
                trigger(e);
            });
        } else if (typeof window !== "undefined" && "ontouchstart" in window) {
            let touchStartX = 0, touchStartY = 0, touchMoved = false;
            el.addEventListener("touchstart", (e) => {
                if (Date.now() < collapseGuardTimestamp) {
                    e.stopPropagation();
                    return;
                }
                if (e.touches && e.touches[0]) {
                    touchStartX = e.touches[0].clientX;
                    touchStartY = e.touches[0].clientY;
                }
                touchMoved = false;
                el.classList.add("vpad-btn-pressed");
                e.stopPropagation();
            }, { passive: false });

            el.addEventListener("touchmove", (e) => {
                if (e.touches && e.touches[0]) {
                    const dx = Math.abs(e.touches[0].clientX - touchStartX);
                    const dy = Math.abs(e.touches[0].clientY - touchStartY);
                    if (dx > 10 || dy > 10) {
                        touchMoved = true;
                        el.classList.remove("vpad-btn-pressed");
                    }
                }
            }, { passive: true });

            el.addEventListener("touchend", (e) => {
                el.classList.remove("vpad-btn-pressed");
                if (!touchMoved) {
                    lastHandledTime = Date.now();
                    trigger(e);
                }
            }, { passive: false });

            el.addEventListener("click", (e) => {
                e.stopPropagation();
                if (e.preventDefault && e.cancelable) e.preventDefault();
                if (Date.now() - lastHandledTime < 500) return;
                lastHandledTime = Date.now();
                trigger(e);
            });
        } else {
            el.addEventListener("click", trigger);
        }
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
    // 5. CSS Link & Fallback Injection
    // ------------------------------------------------------------------------
    function ensureStylesheet() {
        if (typeof document === "undefined" || !document.head) return;
        const hasCss = document.querySelector('link[href*="vpad.css"]');
        if (!hasCss) {
            const link = document.createElement("link");
            link.rel = "stylesheet";
            link.href = "vpad.css";
            document.head.appendChild(link);
        }
    }

    // ------------------------------------------------------------------------
    // 6. SVG Icons for 3-State Machine
    // ------------------------------------------------------------------------
    const SVG_ICONS = {
        gamepad: `<svg viewBox="0 0 24 24"><rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 12h4m-2-2v4m7-2h.01m3 0h.01"/></svg>`,
        chevronUp: `<svg viewBox="0 0 24 24"><polyline points="18 15 12 9 6 15"/></svg>`,
        eye: `<svg viewBox="0 0 24 24"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>`,
        eyeOff: `<svg viewBox="0 0 24 24"><path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"/><line x1="1" y1="1" x2="23" y2="23"/></svg>`,
        cheat: `<svg viewBox="0 0 24 24"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>`,
        link: `<svg viewBox="0 0 24 24"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>`,
        gear: `<svg viewBox="0 0 24 24"><path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/></svg>`,
        ripple: `<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="2.5"/><circle cx="12" cy="12" r="6"/><circle cx="12" cy="12" r="9.5"/></svg>`,
        diag: `<svg viewBox="0 0 24 24"><path d="m8 2 1.88 1.88"/><path d="M14.12 3.88 16 2"/><path d="M9 7.13v-1a3.003 3.003 0 1 1 6 0v1"/><path d="M12 20c-3.3 0-6-2.7-6-6v-3a4 4 0 0 1 4-4h4a4 4 0 0 1 4 4v3c0 3.3-2.7 6-6 6"/><path d="M12 20v-9"/><path d="M6.53 9C4.6 8.8 3 7.1 3 5"/><path d="M6 13H2"/><path d="M3 21c0-2.1 1.7-3.9 3.8-4"/><path d="M20.97 5c0 2.1-1.6 3.8-3.5 4"/><path d="M22 13h-4"/><path d="M17.2 17c2.1.1 3.8 1.9 3.8 4"/></svg>`
    };

    // /* Watermark Top-Right */
    // ------------------------------------------------------------------------
    // 7. DOM Creation & Structure
    // ------------------------------------------------------------------------
    function createOverlay() {
        if (typeof document === "undefined" || !document.body) return;
        if (document.getElementById("vpad-root")) return;
        ensureStylesheet();

        const root = document.createElement("div");
        root.id = "vpad-root";

        root.innerHTML = `
            <div id="fnh-watermark"></div>

            <!-- Top Left Floating Toolbar with iOS Origin-Based Spring -->
            <div id="vpad-topbar">
                <!-- Main Toggle Trigger Button -->
                <div id="vpad-main-trigger-btn" title="Toggle Controls Menu">
                    ${SVG_ICONS.gamepad}
                </div>

                <!-- Sub-Menu: 1. Gear/Settings, 2. Cheat, 3. Link, 4. Eye/Visibility, 5. Diag, 6. Ripple -->
                <div id="vpad-sub-menu">
                    <div id="vpad-btn-gear" class="vpad-top-btn" title="Layout Editor & Settings">
                        ${SVG_ICONS.gear}
                    </div>
                    <div id="vpad-btn-cheat" class="vpad-top-btn cheat-interactive" title="Cheat Menu">
                        ${SVG_ICONS.cheat}
                    </div>
                    <div id="vpad-btn-link" class="vpad-top-btn" title="Veldra Works Community">
                        ${SVG_ICONS.link}
                    </div>
                    <div id="vpad-btn-visibility" class="vpad-top-btn" title="Hide All Controls">
                        ${SVG_ICONS.eye}
                    </div>
                    <div id="vpad-btn-diag" class="vpad-top-btn diag-interactive" title="Diagnostic Suite">
                        ${SVG_ICONS.diag}
                    </div>
                    <div id="vpad-btn-ripple" class="vpad-top-btn" title="Touchscreen Ripple (On/Off)">
                        ${SVG_ICONS.ripple}
                    </div>
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
                    <span id="vpad-bar-theme-name">Theme: Minimalist Matte</span> ▼
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
                <div class="vpad-popover-item" data-id="matte">1. Minimalist Matte (Default)</div>
                <div class="vpad-popover-item" data-id="clean">2. Liquid Glass</div>
                <div class="vpad-popover-item" data-id="retro">3. Retro OSD</div>
            </div>

            <!-- Touchscreen Visual Ripple Layer & Motion Trail Canvas -->
            <div id="vpad-touch-ripple-layer">
                <canvas id="vpad-trail-canvas"></canvas>
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
                            <span id="vpad-prop-key-name" class="vpad-text-accent" style="font-weight:bold;">A (Z)</span>
                        </div>
                        <button id="vpad-prop-btn-changekey" class="vpad-prop-btn" style="width:100%;">Change Key / Binding</button>
                    </div>

                    <!-- Switch Type (For D-Pad / Joystick) -->
                    <div class="vpad-prop-row" id="vpad-prop-switch-type-row" style="display:none;">
                        <button id="vpad-prop-btn-switch-type" class="vpad-prop-btn" style="width:100%;">Switch to Analog Joystick</button>
                    </div>

                    <!-- Joystick Mode: Fixed vs Semi-Floating -->
                    <div class="vpad-prop-row" id="vpad-prop-joystick-mode-row" style="display:none;">
                        <div class="vpad-prop-label">
                            <span>Joystick:</span>
                            <span id="vpad-prop-joystick-mode-label" class="vpad-text-accent" style="font-weight:bold;">Semi-Floating</span>
                        </div>
                        <button id="vpad-prop-btn-joystick-mode" class="vpad-prop-btn" style="width:100%;">Switch to Fixed</button>
                    </div>

                    <!-- Actions: Duplicate & Delete -->
                    <div class="vpad-prop-actions">
                        <button id="vpad-prop-btn-dup" class="vpad-prop-btn">Duplicate</button>
                        <button id="vpad-prop-btn-del" class="vpad-prop-btn danger">Delete</button>
                    </div>
                </div>
            </div>

            <!-- Custom Control Modal Dialog -->
            <div id="vpad-key-modal">
                <div class="vpad-modal-box">
                    <div class="vpad-modal-header">
                        <span id="vpad-modal-title">Custom Control Setup</span>
                        <span id="vpad-modal-close" style="cursor:pointer; font-size:16px;">✕</span>
                    </div>

                    <div class="vpad-modal-body">
                        <!-- Control Type Selector -->
                        <div id="vpad-modal-type-container" style="margin-top:4px;">
                            <span class="vpad-label-muted">Control Type:</span>
                            <div class="vpad-segmented-group" id="vpad-modal-type-group">
                                <button type="button" class="vpad-segmented-btn active" data-value="button">Button</button>
                                <button type="button" class="vpad-segmented-btn" data-value="joystick">Joystick</button>
                                <button type="button" class="vpad-segmented-btn" data-value="dpad">D-Pad</button>
                            </div>
                        </div>

                        <!-- Button Specific Fields -->
                        <div id="vpad-modal-button-fields">
                            <div style="margin-top:8px;">
                                <span class="vpad-label-muted">Label (max 12 characters):</span>
                                <input type="text" id="vpad-modal-input-label" class="vpad-input-field" maxlength="12">
                            </div>

                            <div style="margin-top:8px;">
                                <span class="vpad-label-muted">Select Keyboard Key (scroll to browse):</span>
                                <div class="vpad-modal-scroll-area" id="vpad-modal-scroll-area">
                                    <div class="vpad-picker-grid" id="vpad-modal-key-grid"></div>
                                </div>
                            </div>
                        </div>
                    </div>

                    <div class="vpad-modal-footer">
                        <button id="vpad-modal-btn-cancel" class="vpad-bar-btn" style="flex:1; justify-content:center;">Cancel</button>
                        <button id="vpad-modal-btn-apply" class="vpad-bar-btn vpad-bar-done" style="flex:1; justify-content:center;">Apply</button>
                    </div>
                </div>
            </div>
        `;

        document.body.appendChild(root);

        // Hoist floating editor panels directly into body so their stacking context is independent of gameplay buttons
        ["vpad-editor-bar", "vpad-preset-popover", "vpad-theme-popover", "vpad-prop-panel", "vpad-key-modal"].forEach(id => {
            const el = document.getElementById(id);
            if (el) document.body.appendChild(el);
        });

        setupOverlayLogic();
        applyTheme(state.activeThemeId || "matte");
    }

    // ------------------------------------------------------------------------
    // 8. Touch Sliders Component
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
            if (!fillEl || !knobEl) return;
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

        const onEnd = () => {
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
    // 9. Controller Render & Interaction Logic
    // ------------------------------------------------------------------------
    let scaleSliderCtrl = null;
    let opacSliderCtrl = null;

    function syncThemeUI(themeId) {
        const norm = (themeId === "clean" ? "liquid_glass" : (themeId === "retro" ? "retro_osd" : themeId));
        state.activeThemeId = norm;
        let displayName = norm;
        if (typeof window !== "undefined" && window.RuntimePackTheme && typeof window.RuntimePackTheme.getAvailableThemes === "function") {
            const list = window.RuntimePackTheme.getAvailableThemes();
            const match = list.find(t => t.id === norm || t.id === themeId);
            if (match) displayName = match.name;
        } else if (THEMES[norm]) {
            displayName = THEMES[norm].name;
        } else if (THEMES[themeId]) {
            displayName = THEMES[themeId].name;
        }

        const themeNameEl = document.getElementById("vpad-bar-theme-name");
        if (themeNameEl) {
            themeNameEl.textContent = "Theme: " + displayName;
        }

        const popover = document.getElementById("vpad-theme-popover");
        if (popover) {
            popover.querySelectorAll(".vpad-popover-item").forEach(el => {
                const id = el.getAttribute("data-id");
                const active = id === norm || id === themeId ||
                    (norm === "liquid_glass" && id === "clean") ||
                    (norm === "retro_osd" && id === "retro");
                el.classList.toggle("active", active);
            });
        }

        if (typeof window !== "undefined" && typeof window.applyCheatTheme === "function") {
            window.applyCheatTheme(norm);
        }
        saveState();
    }

    if (typeof window !== "undefined") {
        window.addEventListener("vpad:themechange", (e) => {
            const id = (e.detail && e.detail.themeId) || (window.RuntimePackTheme && window.RuntimePackTheme.getCurrentTheme());
            if (id) syncThemeUI(id);
        });
    }

    function applyTheme(themeId) {
        if (typeof window !== "undefined" && window.RuntimePackTheme && typeof window.RuntimePackTheme.applyTheme === "function") {
            const ok = window.RuntimePackTheme.applyTheme(themeId);
            if (ok) return;
        }

        const valid = THEMES[themeId] ? themeId : "matte";
        state.activeThemeId = valid;
        if (typeof document !== "undefined" && document.body) {
            document.body.classList.remove("vpad-theme-clean", "vpad-theme-matte", "vpad-theme-retro", "vpad-theme-liquid_glass", "vpad-theme-retro_osd");
            document.body.classList.add("vpad-theme-" + valid);
            if (valid === "liquid_glass" || valid === "clean") {
                document.body.classList.add("vpad-theme-clean");
            }
            document.body.classList.toggle("vpad-theme-retro", valid === "retro" || valid === "retro_osd");

            const themeNameEl = document.getElementById("vpad-bar-theme-name");
            if (themeNameEl) themeNameEl.textContent = "Theme: " + THEMES[valid].name;

            document.querySelectorAll("#vpad-theme-popover .vpad-popover-item").forEach(el => {
                const id = el.getAttribute("data-id");
                el.classList.toggle("active", id === valid || (valid === "liquid_glass" && id === "clean") || (valid === "retro_osd" && id === "retro"));
            });
        }

        if (typeof window !== "undefined" && typeof window.applyCheatTheme === "function") {
            window.applyCheatTheme(valid);
        }
        saveState();
    }
    if (typeof window !== "undefined") {
        window.applyVpadTheme = applyTheme;
    }

    function shouldIgnoreTouchForFeedback(target) {
        if (!target) return false;
        // If target is the full-screen empty controls container or overlay background, DO NOT ignore!
        if (target.id === "vpad-controls" || target.id === "vpad-root" || target.id === "vpad-touch-ripple-layer" || target.id === "vpad-trail-canvas" || target.tagName === "BODY" || target.tagName === "HTML") {
            return false;
        }
        if (typeof target.closest === "function") {
            return !!(
                target.closest(".vpad-control-item") ||
                target.closest(".vpad-btn") ||
                target.closest(".vpad-top-btn") ||
                target.closest("#vpad-main-trigger-btn") ||
                target.closest(".vpad-dpad-container") ||
                target.closest(".vpad-joystick-container") ||
                target.closest("#vpad-topbar") ||
                target.closest("#vpad-editor-bar") ||
                target.closest("#vpad-prop-panel") ||
                target.closest("#vpad-key-modal") ||
                target.closest(".vpad-modal-box") ||
                target.closest("#fnh-cheat-overlay") ||
                target.closest("#rp-diag-modal") ||
                target.closest(".vpad-popover")
            );
        }
        let cur = target;
        while (cur) {
            if (cur.id === "vpad-controls" || cur.id === "vpad-root" || cur.id === "vpad-touch-ripple-layer" || cur.id === "vpad-trail-canvas") return false;
            if (typeof cur.matches === "function" && (
                cur.matches(".vpad-control-item") ||
                cur.matches(".vpad-btn") ||
                cur.matches(".vpad-top-btn") ||
                cur.matches("#vpad-main-trigger-btn") ||
                cur.matches(".vpad-dpad-container") ||
                cur.matches(".vpad-joystick-container") ||
                cur.matches("#vpad-topbar") ||
                cur.matches("#vpad-editor-bar") ||
                cur.matches("#vpad-prop-panel") ||
                cur.matches("#vpad-key-modal") ||
                cur.matches(".vpad-modal-box") ||
                cur.matches("#fnh-cheat-overlay") ||
                cur.matches("#rp-diag-modal") ||
                cur.matches(".vpad-popover")
            )) {
                return true;
            }
            cur = cur.parentElement;
        }
        return false;
    }

    // ------------------------------------------------------------------------
    // HTML5 Canvas Swipe Motion Trail & Visual Ripple Indicator
    // ------------------------------------------------------------------------
    const MAX_TRAIL_AGE = 220; // ms
    const TRAIL_SAMPLE_DIST_SQ = 16; // >= 4px distance threshold (4 * 4)
    const trailStrokes = new Map(); // pointerId -> Array<{ x, y, time, width }>
    let trailAnimationFrameId = null;
    const activeFeedbackPointers = new Map();

    function getTrailCanvas() {
        let canvas = document.getElementById("vpad-trail-canvas");
        if (!canvas) {
            const layer = document.getElementById("vpad-touch-ripple-layer");
            if (layer) {
                canvas = document.createElement("canvas");
                canvas.id = "vpad-trail-canvas";
                layer.appendChild(canvas);
            }
        }
        return canvas;
    }

    function resizeTrailCanvas() {
        const canvas = getTrailCanvas();
        if (!canvas) return;
        const dpr = (typeof window !== "undefined" && window.devicePixelRatio) || 1;
        const w = (typeof window !== "undefined" && window.innerWidth) || 800;
        const h = (typeof window !== "undefined" && window.innerHeight) || 600;
        const bufW = Math.round(w * dpr);
        const bufH = Math.round(h * dpr);

        if (canvas.width !== bufW || canvas.height !== bufH) {
            canvas.width = bufW;
            canvas.height = bufH;
        }
        if (canvas.style) {
            canvas.style.width = w + "px";
            canvas.style.height = h + "px";
        }

        if (typeof canvas.getContext === "function") {
            const ctx = canvas.getContext("2d");
            if (ctx && typeof ctx.setTransform === "function") {
                ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
            }
        }
    }

    function renderTrailLoop() {
        const canvas = getTrailCanvas();
        if (!canvas || typeof canvas.getContext !== "function") {
            trailAnimationFrameId = null;
            return;
        }
        const ctx = canvas.getContext("2d");
        if (!ctx) {
            trailAnimationFrameId = null;
            return;
        }

        const now = (typeof performance !== "undefined" && performance.now) ? performance.now() : Date.now();
        let totalActivePoints = 0;

        if (typeof ctx.save === "function") {
            ctx.save();
            if (typeof ctx.setTransform === "function") {
                ctx.setTransform(1, 0, 0, 1, 0, 0);
            }
            ctx.clearRect(0, 0, canvas.width, canvas.height);
            ctx.restore();
        } else if (typeof ctx.clearRect === "function") {
            ctx.clearRect(0, 0, canvas.width, canvas.height);
        }

        for (const [pid, points] of trailStrokes.entries()) {
            while (points.length > 0 && (now - points[0].time) > MAX_TRAIL_AGE) {
                points.shift();
            }

            if (points.length === 0) {
                if (!activeFeedbackPointers.has(pid) || !activeFeedbackPointers.get(pid).active) {
                    trailStrokes.delete(pid);
                }
                continue;
            }

            totalActivePoints += points.length;

            if (points.length === 1) {
                const p0 = points[0];
                const age = now - p0.time;
                const progress = Math.min(1, Math.max(0, age / MAX_TRAIL_AGE));
                const alpha = Math.max(0, (1 - progress) * 0.85);
                const radius = Math.max(1, (1 - progress) * (p0.width / 2));
                ctx.beginPath();
                ctx.arc(p0.x, p0.y, radius, 0, Math.PI * 2);
                ctx.fillStyle = `rgba(255, 255, 255, ${alpha.toFixed(3)})`;
                ctx.fill();
            } else {
                for (let i = 0; i < points.length - 1; i++) {
                    const p0 = points[i];
                    const p1 = points[i + 1];
                    const mid = {
                        x: (p0.x + p1.x) / 2,
                        y: (p0.y + p1.y) / 2
                    };
                    const age = now - p1.time;
                    const progress = Math.min(1, Math.max(0, age / MAX_TRAIL_AGE));
                    const alpha = Math.max(0, (1 - progress) * 0.85);
                    const segWidth = Math.max(1, (1 - progress) * p1.width);

                    const startX = (i === 0) ? p0.x : ((points[i - 1].x + p0.x) / 2);
                    const startY = (i === 0) ? p0.y : ((points[i - 1].y + p0.y) / 2);

                    ctx.beginPath();
                    ctx.moveTo(startX, startY);
                    ctx.quadraticCurveTo(p0.x, p0.y, mid.x, mid.y);
                    ctx.lineWidth = segWidth;
                    ctx.strokeStyle = `rgba(255, 255, 255, ${alpha.toFixed(3)})`;
                    ctx.lineCap = "round";
                    ctx.lineJoin = "round";
                    ctx.stroke();

                    if (i === points.length - 2) {
                        ctx.beginPath();
                        ctx.moveTo(mid.x, mid.y);
                        ctx.lineTo(p1.x, p1.y);
                        ctx.lineWidth = segWidth;
                        ctx.strokeStyle = `rgba(255, 255, 255, ${alpha.toFixed(3)})`;
                        ctx.lineCap = "round";
                        ctx.lineJoin = "round";
                        ctx.stroke();
                    }
                }
            }
        }

        if (totalActivePoints > 0) {
            trailAnimationFrameId = (typeof requestAnimationFrame === "function") ?
                requestAnimationFrame(renderTrailLoop) : null;
        } else {
            if (typeof ctx.save === "function") {
                ctx.save();
                if (typeof ctx.setTransform === "function") {
                    ctx.setTransform(1, 0, 0, 1, 0, 0);
                }
                ctx.clearRect(0, 0, canvas.width, canvas.height);
                ctx.restore();
            } else if (typeof ctx.clearRect === "function") {
                ctx.clearRect(0, 0, canvas.width, canvas.height);
            }
            trailAnimationFrameId = null;
        }
    }

    function startTrailLoop() {
        if (trailAnimationFrameId) return;
        if (typeof requestAnimationFrame === "function") {
            trailAnimationFrameId = requestAnimationFrame(renderTrailLoop);
        }
    }

    function addTrailPoint(pid, x, y) {
        const now = (typeof performance !== "undefined" && performance.now) ? performance.now() : Date.now();
        let points = trailStrokes.get(pid);
        if (!points) {
            points = [];
            trailStrokes.set(pid, points);
        }
        points.push({ x, y, time: now, width: 8 });
        startTrailLoop();
    }

    function clearTrailCanvas() {
        trailStrokes.clear();
        activeFeedbackPointers.clear();
        const canvas = document.getElementById("vpad-trail-canvas");
        if (canvas && typeof canvas.getContext === "function") {
            const ctx = canvas.getContext("2d");
            if (ctx) {
                if (typeof ctx.save === "function") {
                    ctx.save();
                    if (typeof ctx.setTransform === "function") {
                        ctx.setTransform(1, 0, 0, 1, 0, 0);
                    }
                    ctx.clearRect(0, 0, canvas.width, canvas.height);
                    ctx.restore();
                } else if (typeof ctx.clearRect === "function") {
                    ctx.clearRect(0, 0, canvas.width, canvas.height);
                }
            }
        }
        if (trailAnimationFrameId && typeof cancelAnimationFrame === "function") {
            cancelAnimationFrame(trailAnimationFrameId);
        }
        trailAnimationFrameId = null;
    }

    function spawnTouchRipple(clientX, clientY) {
        if (!state.touchRipple) return;
        const layer = document.getElementById("vpad-touch-ripple-layer");
        if (!layer) return;
        const ripple = document.createElement("div");
        ripple.className = "vpad-touch-ripple";
        ripple.style.left = clientX + "px";
        ripple.style.top = clientY + "px";
        layer.appendChild(ripple);
        setTimeout(() => {
            if (ripple.parentNode) ripple.parentNode.removeChild(ripple);
        }, 380);
    }

    function setupOverlayLogic() {
        const mainTriggerBtn = document.getElementById("vpad-main-trigger-btn");
        const subMenu = document.getElementById("vpad-sub-menu");
        const padToggleBtn = document.getElementById("vpad-btn-pad-toggle");
        const gearBtn = document.getElementById("vpad-btn-gear");
        const cheatBtn = document.getElementById("vpad-btn-cheat");
        const linkBtn = document.getElementById("vpad-btn-link");
        const diagBtn = document.getElementById("vpad-btn-diag");
        const rippleBtn = document.getElementById("vpad-btn-ripple");

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
        const propBtnJoystickMode = document.getElementById("vpad-prop-btn-joystick-mode");
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

        // Setup instant tactile press feedback on toolbar buttons
        document.querySelectorAll(".vpad-top-btn, #vpad-main-trigger-btn").forEach(btn => {
            const press = () => btn.classList.add("vpad-btn-pressed");
            const unpress = () => btn.classList.remove("vpad-btn-pressed");
            btn.addEventListener("pointerdown", press);
            btn.addEventListener("pointerup", unpress);
            btn.addEventListener("pointercancel", unpress);
            btn.addEventListener("pointerleave", unpress);
            btn.addEventListener("touchstart", press, { passive: true });
            btn.addEventListener("touchend", unpress, { passive: true });
            btn.addEventListener("touchcancel", unpress, { passive: true });
        });

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

        // 2. Hide / Show Gamepad Controls Layer
        function applyGamepadHidden() {
            const controlsEl = document.getElementById("vpad-controls");
            if (controlsEl) {
                controlsEl.classList.toggle("vpad-hidden", state.hidden);
            }
            if (typeof window !== "undefined" && window.RuntimePackVPad && window.RuntimePackVPad.setGamepadHidden) {
                window.RuntimePackVPad.setGamepadHidden(state.hidden);
            }
            reportControlHitboxes();
        }
        applyGamepadHidden();

        // 3. Floating Toolbar Menu & Hide All / Show All Toggle Logic
        let isMenuOpen = false;

        function renderMenuState() {
            if (!mainTriggerBtn) return;
            if (isMenuOpen) {
                mainTriggerBtn.innerHTML = SVG_ICONS.chevronUp;
                mainTriggerBtn.title = "Collapse Controls Menu";
                mainTriggerBtn.classList.add("active");
                if (subMenu) subMenu.classList.add("vpad-menu-open");
            } else {
                mainTriggerBtn.innerHTML = SVG_ICONS.gamepad;
                mainTriggerBtn.title = "Open Controls Menu";
                mainTriggerBtn.classList.remove("active");
                if (subMenu) subMenu.classList.remove("vpad-menu-open");
            }
        }

        bindTap(mainTriggerBtn, () => {
            if (isEditMode) return;
            isMenuOpen = !isMenuOpen;
            renderMenuState();
        });
        renderMenuState();

        // Eye Button: Hide All / Show All Controls Toggle
        const visibilityBtn = document.getElementById("vpad-btn-visibility");
        function updateVisibilityBtnUI() {
            if (!visibilityBtn) return;
            if (state.hidden) {
                visibilityBtn.innerHTML = SVG_ICONS.eyeOff;
                visibilityBtn.title = "Show All Controls";
                visibilityBtn.classList.add("active");
            } else {
                visibilityBtn.innerHTML = SVG_ICONS.eye;
                visibilityBtn.title = "Hide All Controls";
                visibilityBtn.classList.remove("active");
            }
        }

        if (visibilityBtn) {
            bindTap(visibilityBtn, () => {
                state.hidden = !state.hidden;
                applyGamepadHidden();
                saveState();
                updateVisibilityBtnUI();
            });
            updateVisibilityBtnUI();
        }

        // Ripple Button: Touchscreen Visual Ripple On/Off Toggle
        function updateRippleBtnUI() {
            if (!rippleBtn) return;
            if (state.touchRipple) {
                rippleBtn.classList.add("active");
                rippleBtn.title = "Touchscreen Ripple: ON (Tap to Disable)";
            } else {
                rippleBtn.classList.remove("active");
                rippleBtn.title = "Touchscreen Ripple: OFF (Tap to Enable)";
            }
        }

        if (rippleBtn) {
            bindTap(rippleBtn, () => {
                state.touchRipple = !state.touchRipple;
                if (!state.touchRipple && typeof clearTrailCanvas === "function") {
                    clearTrailCanvas();
                }
                saveState();
                updateRippleBtnUI();
            });
            updateRippleBtnUI();
        }

        // Layout Editor Gear Button
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

        // Community Linktree
        window.openLinktree = openLinktree;
        if (linkBtn) {
            bindTap(linkBtn, window.openLinktree);
        }

        // Cheat Menu
        if (cheatBtn) {
            bindTap(cheatBtn, () => {
                if (typeof window !== "undefined" && window.CheatMenu && typeof window.CheatMenu.toggle === "function") {
                    window.CheatMenu.toggle();
                }
                const cheatOverlay = document.getElementById("fnh-cheat-overlay");
                if (cheatOverlay && cheatOverlay.style.display !== "none") {
                    if (typeof window !== "undefined" && typeof window.bringPanelToFront === "function") {
                        window.bringPanelToFront(cheatOverlay);
                    } else {
                        bringPanelToFront(cheatOverlay);
                    }
                }
            });
        }

        if (diagBtn) {
            bindTap(diagBtn, () => {
                if (typeof window !== "undefined" && window.DiagnosticSuite && typeof window.DiagnosticSuite.toggle === "function") {
                    window.DiagnosticSuite.toggle();
                }
                const diagModal = document.getElementById("rp-diag-modal");
                if (diagModal && diagModal.style.display !== "none") {
                    if (typeof window !== "undefined" && typeof window.bringPanelToFront === "function") {
                        window.bringPanelToFront(diagModal);
                    } else {
                        bringPanelToFront(diagModal);
                    }
                }
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
                bringPanelToFront(presetPopover);
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

        function updateThemePopoverUI() {
            if (!themePopover) return;
            const themes = (typeof window !== "undefined" && window.RuntimePackTheme && typeof window.RuntimePackTheme.getAvailableThemes === "function")
                ? window.RuntimePackTheme.getAvailableThemes()
                : [
                    { id: "matte", name: "Minimalist Matte (Default)" },
                    { id: "clean", name: "Liquid Glass" },
                    { id: "retro", name: "Retro OSD" }
                ];
            themePopover.innerHTML = "";
            const currentId = (typeof window !== "undefined" && window.RuntimePackTheme && typeof window.RuntimePackTheme.getCurrentTheme === "function")
                ? window.RuntimePackTheme.getCurrentTheme()
                : state.activeThemeId;
            themes.forEach((thm, idx) => {
                const item = document.createElement("div");
                item.className = "vpad-popover-item";
                item.setAttribute("data-id", thm.id);
                item.textContent = (idx + 1) + ". " + thm.name;
                const active = thm.id === currentId || (currentId === "liquid_glass" && thm.id === "clean") || (currentId === "retro_osd" && thm.id === "retro");
                if (active) item.classList.add("active");
                bindTap(item, () => {
                    if (typeof window !== "undefined" && window.RuntimePackTheme && typeof window.RuntimePackTheme.applyTheme === "function") {
                        window.RuntimePackTheme.applyTheme(thm.id);
                    } else {
                        applyTheme(thm.id);
                    }
                    themePopover.classList.remove("show");
                });
                themePopover.appendChild(item);
            });
        }

        bindTap(themeBtn, () => {
            presetPopover.classList.remove("show");
            const isShown = themePopover.classList.contains("show");
            if (isShown) {
                themePopover.classList.remove("show");
            } else {
                updateThemePopoverUI();
                const rect = themeBtn.getBoundingClientRect();
                themePopover.style.left = Math.max(10, rect.left) + "px";
                themePopover.style.top = (rect.bottom + 6) + "px";
                bringPanelToFront(themePopover);
                themePopover.classList.add("show");
                popoverJustOpened = true;
                setTimeout(() => { popoverJustOpened = false; }, 250);
            }
        });

        updateThemePopoverUI();
        const initialThemeId = (typeof window !== "undefined" && window.RuntimePackTheme && typeof window.RuntimePackTheme.getCurrentTheme === "function")
            ? window.RuntimePackTheme.getCurrentTheme()
            : state.activeThemeId;
        const initialThemeNameEl = document.getElementById("vpad-bar-theme-name");
        if (initialThemeNameEl) {
            let label = initialThemeId;
            if (typeof window !== "undefined" && window.RuntimePackTheme && typeof window.RuntimePackTheme.getAvailableThemes === "function") {
                const tObj = window.RuntimePackTheme.getAvailableThemes().find(t => t.id === initialThemeId);
                if (tObj) label = tObj.name;
            } else if (THEMES[initialThemeId]) {
                label = THEMES[initialThemeId].name;
            }
            initialThemeNameEl.textContent = "Theme: " + label;
        }

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

        keyModal.addEventListener("pointerdown", (e) => {
            bringPanelToFront(keyModal);
            e.stopPropagation();
        });
        keyModal.addEventListener("mousedown", () => {
            bringPanelToFront(keyModal);
        });
        keyModal.addEventListener("pointerup", (e) => { e.stopPropagation(); });

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
            bringPanelToFront(keyModal);
            keyModal.classList.add("show");
            setModalState("custom_control", true);
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
            bringPanelToFront(keyModal);
            keyModal.classList.add("show");
            setModalState("custom_control", true);
        });

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

        if (propBtnJoystickMode) {
            bindTap(propBtnJoystickMode, () => {
                const ctrl = getSelectedControl();
                if (!ctrl || ctrl.type !== "joystick") return;
                ctrl.floating = (ctrl.floating === false) ? true : false;
                saveState();
                updatePropertyPanel(ctrl);
            });
        }

        const closeKeyModal = () => {
            keyModal.classList.remove("show");
            setModalState("custom_control", false);
        };
        bindTap(modalClose, closeKeyModal);
        bindTap(modalCancel, closeKeyModal);

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
            closeKeyModal();
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

        // 11. Draggable Property Panel Header
        propPanel.addEventListener("pointerdown", (e) => {
            bringPanelToFront(propPanel);
            e.stopPropagation();
        });
        propPanel.addEventListener("mousedown", () => {
            bringPanelToFront(propPanel);
        });
        propPanel.addEventListener("pointerup", (e) => e.stopPropagation());
        setupDraggableHeader(propPanel, propHeader, (x, y) => {
            state.panelPosition = { x, y };
            saveState();
        });

        // 12. Draggable Editor Bar
        editorBar.addEventListener("pointerdown", (e) => {
            bringPanelToFront(editorBar);
            e.stopPropagation();
        });
        editorBar.addEventListener("mousedown", () => {
            bringPanelToFront(editorBar);
        });
        editorBar.addEventListener("pointerup", (e) => e.stopPropagation());
        setupDraggableHeader(editorBar, barDrag, (x, y) => {
            state.toolbarPosition = { x, y };
            saveState();
        });

        // 13. Collapse / Expand Toolbar
        bindTap(collapseBtn, () => {
            state.toolbarCollapsed = !state.toolbarCollapsed;
            applyToolbarCollapsedUI(true);
            saveState();
        });

        function applyToolbarCollapsedUI(fromUserInteraction) {
            const isCol = state.toolbarCollapsed;
            doneBtn.style.display = isCol ? "none" : "flex";
            presetBtn.style.display = isCol ? "none" : "flex";
            themeBtn.style.display = isCol ? "none" : "flex";
            addBtn.style.display = isCol ? "none" : "flex";
            resetBtn.style.display = isCol ? "none" : "flex";
            collapseBtn.textContent = isCol ? "≡" : "─";
            barDrag.textContent = isCol ? "⋮⋮ EDIT" : "⋮⋮";

            if (fromUserInteraction) {
                collapseGuardTimestamp = Date.now() + 150;
                const buttonsToGuard = [doneBtn, presetBtn, themeBtn, addBtn, resetBtn, collapseBtn];
                buttonsToGuard.forEach(b => {
                    if (b) b.style.pointerEvents = "none";
                });
                setTimeout(() => {
                    buttonsToGuard.forEach(b => {
                        if (b) b.style.pointerEvents = "";
                    });
                }, 150);
            }
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

        // --------------------------------------------------------------------
        // HTML5 Canvas Swipe Motion Trail & Visual Ripple Indicator Listeners
        // --------------------------------------------------------------------
        window.addEventListener("pointerdown", (e) => {
            if (isEditMode || !state.touchRipple) return;
            if (shouldIgnoreTouchForFeedback(e.target)) return;
            spawnTouchRipple(e.clientX, e.clientY);
            const pid = e.pointerId !== undefined ? e.pointerId : 1;
            const cx = typeof e.clientX === "number" ? e.clientX : 0;
            const cy = typeof e.clientY === "number" ? e.clientY : 0;
            activeFeedbackPointers.set(pid, { lastX: cx, lastY: cy, active: true });
            addTrailPoint(pid, cx, cy);
        }, { passive: true, capture: true });

        window.addEventListener("pointermove", (e) => {
            if (isEditMode || !state.touchRipple) return;
            const pid = e.pointerId !== undefined ? e.pointerId : 1;
            const tracked = activeFeedbackPointers.get(pid);
            if (!tracked || !tracked.active) return;

            const cx = typeof e.clientX === "number" ? e.clientX : tracked.lastX;
            const cy = typeof e.clientY === "number" ? e.clientY : tracked.lastY;
            const dx = cx - tracked.lastX;
            const dy = cy - tracked.lastY;
            if (dx * dx + dy * dy >= TRAIL_SAMPLE_DIST_SQ) { // >= 4px distance threshold
                addTrailPoint(pid, cx, cy);
                tracked.lastX = cx;
                tracked.lastY = cy;
            }
        }, { passive: true, capture: true });

        function resetFeedbackPointer(e) {
            const pid = e && e.pointerId !== undefined ? e.pointerId : 1;
            const tracked = activeFeedbackPointers.get(pid);
            if (tracked) {
                tracked.active = false;
            }
            activeFeedbackPointers.delete(pid);
        }

        window.addEventListener("pointerup", resetFeedbackPointer, { passive: true, capture: true });
        window.addEventListener("pointercancel", resetFeedbackPointer, { passive: true, capture: true });
        window.addEventListener("blur", () => {
            activeFeedbackPointers.clear();
        }, { passive: true });

        window.addEventListener("resize", () => {
            resizeTrailCanvas();
            applyToolbarPosition();
            applyPanelPosition();
            renderControls();
        });

        resizeTrailCanvas();
        renderControls();
        setTimeout(reportControlHitboxes, 250);
        setTimeout(reportControlHitboxes, 1000);
    }

    function enterEditMode() {
        if (typeof clearTrailCanvas === "function") {
            clearTrailCanvas();
        }
        isEditMode = true;
        document.body.classList.add("vpad-edit-mode");
        const editorBar = document.getElementById("vpad-editor-bar");
        if (editorBar) {
            editorBar.classList.add("show");
            bringPanelToFront(editorBar);
        }
        document.getElementById("vpad-btn-gear").classList.add("active");
        setModalState("settings", true);
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
        setModalState("custom_control", false);
        setModalState("settings", false);
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
        bringPanelToFront(panel);
        setModalState("inspector", true);
    }

    function hidePropertyPanel() {
        const panel = document.getElementById("vpad-prop-panel");
        if (panel) panel.classList.remove("show");
        setModalState("inspector", false);
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
        const joystickModeRow = document.getElementById("vpad-prop-joystick-mode-row");
        const joystickModeLabel = document.getElementById("vpad-prop-joystick-mode-label");
        const btnJoystickMode = document.getElementById("vpad-prop-btn-joystick-mode");

        if (ctrl.type === "button") {
            bindingRow.style.display = "block";
            switchRow.style.display = "none";
            if (joystickModeRow) joystickModeRow.style.display = "none";
            const keyLabel = KEY_NAMES[ctrl.keyCode] || ("Key " + ctrl.keyCode);
            document.getElementById("vpad-prop-key-name").textContent = `${ctrl.label || "BTN"} (${keyLabel})`;
        } else if (ctrl.type === "dpad") {
            bindingRow.style.display = "none";
            switchRow.style.display = "block";
            switchBtn.textContent = "Switch to Analog Joystick";
            if (joystickModeRow) joystickModeRow.style.display = "none";
        } else if (ctrl.type === "joystick") {
            bindingRow.style.display = "none";
            switchRow.style.display = "block";
            switchBtn.textContent = "Switch to D-Pad 8-Way";
            if (joystickModeRow) {
                joystickModeRow.style.display = "block";
                const isFloating = ctrl.floating !== false;
                if (joystickModeLabel) {
                    joystickModeLabel.textContent = isFloating ? "Semi-Floating" : "Fixed";
                    if (joystickModeLabel.classList) {
                        joystickModeLabel.classList.toggle("vpad-text-accent", isFloating);
                        joystickModeLabel.classList.toggle("vpad-text-amber", !isFloating);
                    }
                    joystickModeLabel.style.color = "";
                }
                if (btnJoystickMode) {
                    btnJoystickMode.textContent = isFloating ? "Switch to Fixed" : "Switch to Semi-Floating";
                }
            }
        } else {
            bindingRow.style.display = "none";
            switchRow.style.display = "none";
            if (joystickModeRow) joystickModeRow.style.display = "none";
        }
    }

    let topZIndexCounter = 10;

    function bringControlToFront(id) {
        const ctrlList = currentProfile().controls;
        const idx = ctrlList.findIndex(c => c.id === id);
        if (idx >= 0) {
            const [item] = ctrlList.splice(idx, 1);
            ctrlList.push(item);
            saveState();
        }

        const el = document.querySelector(`[data-id="${id}"]`);
        if (el) {
            topZIndexCounter++;
            el.style.zIndex = topZIndexCounter;
            if (el.parentNode) {
                el.parentNode.appendChild(el);
            }
        }
    }

    // ------------------------------------------------------------------------
    // Universal Panel Bring-To-Front (Z-Index Elevation on Touch/Click)
    // ------------------------------------------------------------------------
    let topPanelZIndex = 100010;
    let lastBroughtElement = null;
    let lastBroughtTime = 0;

    function bringPanelToFront(panelOrId) {
        if (!panelOrId) return null;
        let el = panelOrId;
        if (typeof panelOrId === "string" && typeof document !== "undefined") {
            const cleanId = panelOrId.replace(/^#/, "");
            el = document.getElementById(cleanId) || (document.querySelector ? document.querySelector(panelOrId) : null);
        }
        if (!el || !el.style) return null;
        const isPopover = (el.classList && el.classList.contains("vpad-popover")) ||
                          (el.id === "vpad-preset-popover" || el.id === "vpad-theme-popover");
        const now = (typeof Date !== "undefined" && Date.now) ? Date.now() : 0;
        if (!isPopover && el === lastBroughtElement && (now - lastBroughtTime < 100)) {
            return el;
        }
        const inc = isPopover ? 10 : 2;
        const globalScope = typeof window !== "undefined" ? window : (typeof global !== "undefined" ? global : null);
        const currentTop = (globalScope && typeof globalScope.__topPanelZIndex === "number")
            ? globalScope.__topPanelZIndex
            : topPanelZIndex;
        topPanelZIndex = Math.max(topPanelZIndex, currentTop) + inc;
        el.style.zIndex = topPanelZIndex;
        if (globalScope) {
            globalScope.__topPanelZIndex = topPanelZIndex;
        }
        lastBroughtElement = el;
        lastBroughtTime = now;
        return el;
    }

    function elevatePanel(panelOrId) {
        return bringPanelToFront(panelOrId);
    }

    if (typeof window !== "undefined") {
        window.bringPanelToFront = bringPanelToFront;
        window.elevatePanel = bringPanelToFront;
    }

    if (typeof document !== "undefined") {
        let lastHandledElevationTime = 0;
        let lastHandledPanel = null;

        const handlePanelElevation = (e) => {
            const panel = e.target && e.target.closest && e.target.closest("#rp-diag-modal, #fnh-cheat-overlay, #vpad-editor-bar, #vpad-prop-panel, #vpad-key-modal");
            if (panel) {
                const now = (typeof Date !== "undefined" && Date.now) ? Date.now() : 0;
                if (panel === lastHandledPanel && (now - lastHandledElevationTime < 100)) {
                    return;
                }
                lastHandledPanel = panel;
                lastHandledElevationTime = now;
                bringPanelToFront(panel);
            }
        };
        document.addEventListener("pointerdown", handlePanelElevation, true);
        document.addEventListener("touchstart", handlePanelElevation, { passive: true, capture: true });
        document.addEventListener("mousedown", handlePanelElevation, true);
    }

    function selectControl(id) {
        inspectorCooldown = Date.now() + 250;
        selectedControlId = id;
        bringControlToFront(id);
        document.querySelectorAll(".vpad-control-item").forEach(el => {
            el.classList.toggle("vpad-selected", el.getAttribute("data-id") === id);
        });
        const ctrl = getSelectedControl();
        if (ctrl) showPropertyPanel(ctrl);
    }

    // ------------------------------------------------------------------------
    // 10. Independent Drag Handlers
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

        if (typeof window !== "undefined" && window.PointerEvent) {
            handleEl.addEventListener("pointerdown", (e) => {
                e.stopPropagation();
                if (e.cancelable) e.preventDefault();
                try {
                    if (handleEl.setPointerCapture) handleEl.setPointerCapture(e.pointerId);
                } catch (_) {}
                start(e.clientX, e.clientY);
            });
            handleEl.addEventListener("pointermove", (e) => {
                if (isDragging) {
                    e.stopPropagation();
                    if (e.cancelable) e.preventDefault();
                    move(e.clientX, e.clientY);
                }
            });
            const onPointerUp = (e) => {
                if (isDragging) {
                    try {
                        if (handleEl.releasePointerCapture) handleEl.releasePointerCapture(e.pointerId);
                    } catch (_) {}
                    e.stopPropagation();
                    end();
                }
            };
            handleEl.addEventListener("pointerup", onPointerUp);
            handleEl.addEventListener("pointercancel", onPointerUp);
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
            if (typeof window !== "undefined" && window.PointerEvent) return;
            e.stopPropagation();
            start(e.clientX, e.clientY);
        });
        window.addEventListener("mousemove", (e) => {
            if (isDragging && (!window.PointerEvent)) move(e.clientX, e.clientY);
        });
        window.addEventListener("mouseup", () => {
            if (!window.PointerEvent) end();
        });
    }

    // ------------------------------------------------------------------------
    // 11. Control DOM Rendering
    // ------------------------------------------------------------------------
    let controlListenerCleanups = [];

    function addControlWindowListener(type, handler, options) {
        window.addEventListener(type, handler, options);
        controlListenerCleanups.push(() => window.removeEventListener(type, handler));
    }

    function renderControls() {
        const container = document.getElementById("vpad-controls");
        if (!container) return;
        controlListenerCleanups.forEach(cleanup => cleanup());
        controlListenerCleanups = [];
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

        reportControlHitboxes();
    }

    function reportControlHitboxes() {
        if (typeof window === "undefined" || !window.RuntimePackVPad ||
            typeof window.RuntimePackVPad.updateControlHitboxes !== "function") return;
        try {
            const rects = [];
            const dpr = (typeof window !== "undefined" && window.devicePixelRatio) || 1;
            const controlsEl = document.getElementById("vpad-controls");
            if (controlsEl && !controlsEl.classList.contains("vpad-hidden")) {
                controlsEl.querySelectorAll(".vpad-control-item").forEach(el => {
                    const r = el.getBoundingClientRect();
                    if (r.width <= 0 || r.height <= 0) return;
                    rects.push({
                        left: r.left * dpr,
                        top: r.top * dpr,
                        right: r.right * dpr,
                        bottom: r.bottom * dpr
                    });
                });
            }
            window.RuntimePackVPad.updateControlHitboxes(JSON.stringify(rects));
        } catch (ignored) {
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
        const baseScale = ctrl.scale || 1.0;
        el.style.setProperty("--vpad-scale", baseScale);
        el.style.transform = `scale(${baseScale})`;
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
    // 12. Direct Manipulation: Drag (1 Finger) & Pinch-to-Scale (2 Fingers)
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

        addControlWindowListener("touchmove", (e) => {
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
                reportControlHitboxes();
            }
        };
        addControlWindowListener("touchend", handleTouchEnd);
        addControlWindowListener("touchcancel", handleTouchEnd);

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

        addControlWindowListener("mousemove", (e) => {
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

        addControlWindowListener("mouseup", () => {
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
                reportControlHitboxes();
            }
        });
    }

    // ------------------------------------------------------------------------
    // 13. Gameplay D-Pad, Joystick & Button Handlers
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

        addControlWindowListener("touchmove", (e) => {
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
        addControlWindowListener("touchend", endTouch);
        addControlWindowListener("touchcancel", endTouch);

        // Mouse Support
        let isMouseDown = false;
        dpad.addEventListener("mousedown", (e) => {
            if (isEditMode) return;
            e.stopPropagation();
            isMouseDown = true;
            updateDirection(e.clientX, e.clientY);
        });
        addControlWindowListener("mousemove", (e) => {
            if (isEditMode || !isMouseDown) return;
            updateDirection(e.clientX, e.clientY);
        });
        addControlWindowListener("mouseup", () => {
            if (isEditMode || !isMouseDown) return;
            isMouseDown = false;
            clearArrows();
        });
    }

    function setupJoystickTouch(jsContainer, ctrl) {
        let activeTouchId = null;
        const currentPressedKeys = new Set();
        const stick = jsContainer.querySelector(".vpad-joystick-stick");
        let restCenterX = 0;
        let restCenterY = 0;
        let isMouseDown = false;

        function updateJoystick(clientX, clientY) {
            const dx = clientX - restCenterX;
            const dy = clientY - restCenterY;
            const distance = Math.hypot(dx, dy);

            const baseScale = ctrl.scale || 1.0;
            const containerRadius = (ctrl.width / 2) * baseScale;
            const maxRadius = containerRadius * 0.65;
            const angleRad = Math.atan2(dy, dx);

            const isFloating = ctrl.floating !== false;

            let baseX = 0, baseY = 0;
            if (isFloating && distance > maxRadius) {
                // Semi-floating: outer ring elastically follows thumb at the boundary
                const excess = distance - maxRadius;
                const elasticShift = Math.min(excess * 0.42, maxRadius * 0.45);
                baseX = Math.cos(angleRad) * elasticShift;
                baseY = Math.sin(angleRad) * elasticShift;
            }

            if (baseX !== 0 || baseY !== 0) {
                jsContainer.style.transform = `scale(${baseScale}) translate(${baseX}px, ${baseY}px)`;
            } else {
                jsContainer.style.transform = `scale(${baseScale})`;
            }

            const stickDist = Math.min(distance, maxRadius);
            const stickX = Math.cos(angleRad) * stickDist;
            const stickY = Math.sin(angleRad) * stickDist;

            stick.style.transform = `translate(calc(-50% + ${stickX}px), calc(-50% + ${stickY}px))`;
            jsContainer.classList.add("active");

            const deadzone = containerRadius * 0.15;
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
            const baseScale = ctrl.scale || 1.0;
            jsContainer.style.transition = "transform 0.28s cubic-bezier(0.34, 1.56, 0.64, 1)";
            jsContainer.style.transform = `scale(${baseScale}) translate(0px, 0px)`;
            stick.style.transition = "transform 0.28s cubic-bezier(0.34, 1.56, 0.64, 1)";
            stick.style.transform = "translate(-50%, -50%)";
            jsContainer.classList.remove("active");
            clearKeys();
            setTimeout(() => {
                jsContainer.style.transition = "";
                stick.style.transition = "";
            }, 300);
        }

        function startJoystick(clientX, clientY) {
            const rect = jsContainer.getBoundingClientRect();
            restCenterX = rect.left + rect.width / 2;
            restCenterY = rect.top + rect.height / 2;
            jsContainer.style.transition = "none";
            stick.style.transition = "none";
            updateJoystick(clientX, clientY);
        }

        jsContainer.addEventListener("touchstart", (e) => {
            if (isEditMode) return;
            e.stopPropagation();
            e.preventDefault();
            const touch = e.changedTouches[0];
            activeTouchId = touch.identifier;
            startJoystick(touch.clientX, touch.clientY);
        }, { passive: false });

        addControlWindowListener("touchmove", (e) => {
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
        addControlWindowListener("touchend", endTouch);
        addControlWindowListener("touchcancel", endTouch);

        // Mouse Support
        jsContainer.addEventListener("mousedown", (e) => {
            if (isEditMode) return;
            e.stopPropagation();
            isMouseDown = true;
            startJoystick(e.clientX, e.clientY);
        });
        addControlWindowListener("mousemove", (e) => {
            if (isEditMode || !isMouseDown) return;
            updateJoystick(e.clientX, e.clientY);
        });
        addControlWindowListener("mouseup", () => {
            if (isEditMode || !isMouseDown) return;
            isMouseDown = false;
            resetStick();
        });
    }

    if (typeof window !== "undefined") {
        window.pressKey = pressKey;
        window.releaseKey = releaseKey;
        window.openLinktree = openLinktree;
        window.vpadSwitchPreset = switchPreset;
        window.shouldIgnoreTouchForFeedback = shouldIgnoreTouchForFeedback;
        window.getTrailCanvas = getTrailCanvas;
        window.resizeTrailCanvas = resizeTrailCanvas;
        window.renderTrailLoop = renderTrailLoop;
        window.clearTrailCanvas = clearTrailCanvas;
        window.addTrailPoint = addTrailPoint;
        window.trailStrokes = trailStrokes;
    }

    if (typeof module !== "undefined" && module.exports) {
        module.exports = {
            pressKey,
            releaseKey,
            openLinktree,
            setupDirectManipulation,
            setupDpadTouch,
            shouldIgnoreTouchForFeedback,
            getTrailCanvas,
            resizeTrailCanvas,
            renderTrailLoop,
            clearTrailCanvas,
            addTrailPoint,
            trailStrokes
        };
    }

    // ------------------------------------------------------------------------
    // 14. Auto-Start
    // ------------------------------------------------------------------------
    if (typeof document !== "undefined") {
        if (document.readyState === "complete" || document.readyState === "interactive") {
            createOverlay();
        } else {
            document.addEventListener("DOMContentLoaded", createOverlay);
        }
    }
})();
