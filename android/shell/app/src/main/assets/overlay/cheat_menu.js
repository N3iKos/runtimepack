/**
 * In-Game Cheat Overlay Suite
 * Clean, Compact, Professional & Neutral (Zero AI Slop)
 * Supports 2 Themes: Clean Slate & Retro OSD
 * Author: SHIRAI & Venora Community
 */

(function () {
    "use strict";

    // State Variables
    let godModeActive = false;
    let noclipActive = false;
    let freezeEnemiesActive = false;
    let currentSpeed = 4;
    let activeTab = "tab-char";
    let isMenuOpen = false;

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
    if (typeof window !== "undefined" && window.TouchInput) {
        hookTouchInput(window.TouchInput);
    }

    // Hook Scene_Base.update untuk persistent God Mode & Speed
    function hookGameLoop() {
        if (typeof Scene_Base !== "undefined" && !Scene_Base.prototype._cheatHooked) {
            Scene_Base.prototype._cheatHooked = true;
            const origUpdate = Scene_Base.prototype.update;
            Scene_Base.prototype.update = function () {
                origUpdate.call(this);

                if (godModeActive && typeof $gameParty !== "undefined" && $gameParty.allMembers) {
                    try {
                        const members = $gameParty.allMembers();
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

                if (noclipActive && typeof $gamePlayer !== "undefined") {
                    if (!$gamePlayer.isThrough()) {
                        $gamePlayer.setThrough(true);
                    }
                }
            };
        }

        if (typeof Game_Action !== "undefined" && !Game_Action.prototype._cheatDamageHooked) {
            Game_Action.prototype._cheatDamageHooked = true;
            const origExecuteDamage = Game_Action.prototype.executeDamage;
            Game_Action.prototype.executeDamage = function (target, value) {
                if (godModeActive && target && target.isActor && target.isActor()) {
                    value = 0;
                }
                origExecuteDamage.call(this, target, value);
            };
        }

        if (typeof Game_Player !== "undefined" && !Game_Player.prototype._cheatThroughHooked) {
            Game_Player.prototype._cheatThroughHooked = true;
            const origIsThrough = Game_Player.prototype.isThrough;
            Game_Player.prototype.isThrough = function () {
                if (noclipActive) return true;
                return origIsThrough.call(this);
            };
        }

        if (typeof Game_Event !== "undefined" && !Game_Event.prototype._cheatChaseHooked) {
            Game_Event.prototype._cheatChaseHooked = true;
            const origCanSee = Game_Event.prototype.canSeePlayer;
            if (origCanSee) {
                Game_Event.prototype.canSeePlayer = function () {
                    if (freezeEnemiesActive) return false;
                    return origCanSee.call(this);
                };
            }
        }
    }

    // CSS Styling
    function injectStyles() {
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
                max-width: 500px;
                max-height: 84vh;
                background: #141416;
                border: 1px solid #27272a;
                box-shadow: 0 16px 36px rgba(0, 0, 0, 0.8);
                border-radius: 10px;
                color: #f4f4f5;
                font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
                display: none;
                flex-direction: column;
                z-index: 100010;
                overflow: hidden;
                user-select: none;
                -webkit-user-select: none;
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
                scrollbar-width: none;
                flex-shrink: 0;
            }
            #fnh-cheat-tabs::-webkit-scrollbar { display: none; }
            .fnh-cheat-tab {
                flex: 1;
                min-width: 78px;
                padding: 9px 4px;
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
                width: 13px; height: 13px;
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
                gap: 6px;
            }

            /* Compact Clean Row */
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
            }
            .fnh-cheat-icon svg {
                width: 15px; height: 15px;
                stroke: currentColor;
                fill: none;
                stroke-width: 2;
                stroke-linecap: round;
                stroke-linejoin: round;
                display: block;
                color: #a1a1aa;
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
                min-width: 78px;
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
            }
            .fnh-btn-toggle-on {
                background: #15803d;
                color: #dcfce7;
                border-color: #22c55e;
            }

            /* Quick Grid Items */
            .fnh-cheat-grid {
                display: grid;
                grid-template-columns: repeat(2, 1fr);
                gap: 6px;
            }
            .fnh-grid-btn {
                background: #18181b;
                border: 1px solid #27272a;
                border-radius: 6px;
                padding: 8px 6px;
                color: #e4e4e7;
                font-size: 11px;
                font-weight: 600;
                cursor: pointer;
                text-align: center;
                display: flex; align-items: center; justify-content: center; gap: 6px;
            }
            .fnh-grid-btn:active {
                background: #27272a;
                border-color: #3f3f46;
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
            body.vpad-theme-retro .fnh-cheat-row,
            body.vpad-theme-retro .fnh-grid-btn {
                background: #0e0e12 !important;
                border-color: #333 !important;
                border-radius: 2px !important;
            }
            body.vpad-theme-retro .fnh-cheat-btn {
                border-radius: 2px !important;
                background: #16161c !important;
                border-color: #444 !important;
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

    // Tab 1: Character
    function fullHealAndRestoreLimbs() {
        if (typeof $gameParty === "undefined" || !$gameParty.allMembers) {
            showToast("Game not active!");
            return;
        }
        const limbSwitches = [
            36, 37, 38, 39, 168, 169, 170, 171, 248, 249, 250, 251,
            252, 253, 254, 255, 256, 257, 258, 259, 261, 262, 263, 264,
            270, 271, 272, 381, 382, 383, 384, 385, 386, 387, 388,
            390, 391, 392, 393, 968, 969, 970, 971, 1824, 1825, 1826, 1827,
            1832, 1833, 1834
        ];
        limbSwitches.forEach(sId => {
            if ($gameSwitches) $gameSwitches.setValue(sId, false);
        });
        if ($gameSwitches) {
            for (let s = 1327; s <= 1393; s++) $gameSwitches.setValue(s, false);
        }
        if ($gameVariables) $gameVariables.setValue(138, 0);

        const badStates = [
            1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 14, 15, 16, 17, 19, 22, 23, 24, 30, 31, 34,
            39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 55, 56, 61, 63, 65, 66, 67, 68, 69, 70, 71,
            72, 75, 79, 80, 81, 83, 84, 85, 86, 87, 88, 89, 90, 93, 94, 95, 96, 97, 98, 99
        ];
        const members = $gameParty.allMembers();
        members.forEach(actor => {
            badStates.forEach(stId => actor.removeState(stId));
            actor.setHp(actor.mhp);
            actor.setMp(actor.mmp);
            if (actor._exp) {
                for (let c in actor._exp) actor._exp[c] = 0;
            }
        });

        if (typeof $gamePlayer !== "undefined") $gamePlayer.refresh();
        if (typeof $gameMap !== "undefined") $gameMap.requestRefresh(28);
        if (typeof $gameTemp !== "undefined" && $gameTemp.reserveCommonEvent) {
            $gameTemp.reserveCommonEvent(166);
        }
        showToast("Limbs restored & HP/Mind 100%!");
    }

    function toggleGodMode() {
        godModeActive = !godModeActive;
        const btn = document.getElementById("btn-cheat-godmode");
        if (btn) {
            btn.textContent = godModeActive ? "[ ON ]" : "[ OFF ]";
            btn.className = "fnh-cheat-btn " + (godModeActive ? "fnh-btn-toggle-on" : "fnh-btn-toggle-off");
        }
        showToast(godModeActive ? "God Mode: ACTIVE" : "God Mode: OFF");
    }

    function resetHunger() {
        if (typeof $gameParty === "undefined") return;
        if (typeof $gameVariables !== "undefined") $gameVariables.setValue(138, 0);
        $gameParty.allMembers().forEach(actor => {
            [39, 40, 41, 42, 43].forEach(st => actor.removeState(st));
            if (actor._exp) {
                for (let c in actor._exp) actor._exp[c] = 0;
            }
        });
        showToast("Hunger reset to 0!");
    }

    function maxMind() {
        if (typeof $gameParty === "undefined") return;
        $gameParty.allMembers().forEach(actor => {
            actor.setMp(actor.mmp);
            [44, 45, 46, 47, 48, 15, 16, 34].forEach(st => actor.removeState(st));
        });
        showToast("Sanity / Mind fully restored!");
    }

    function reviveAllMembers() {
        if (typeof $gameParty === "undefined") return;
        $gameParty.allMembers().forEach(actor => {
            actor.removeState(1);
            actor.setHp(actor.mhp);
        });
        showToast("All party members revived!");
    }

    // Tab 2: Items
    function addEssentialSurvivalKit() {
        if (typeof $gameParty === "undefined" || typeof $dataItems === "undefined") return;
        const itemsToAdd = [
            { id: 20, count: 99 },
            { id: 5,  count: 99 },
            { id: 7,  count: 99 },
            { id: 40, count: 50 },
            { id: 59, count: 999 },
            { id: 201, count: 99 },
            { id: 61, count: 99 },
            { id: 34, count: 99 },
            { id: 28, count: 99 },
            { id: 72, count: 99 },
            { id: 8,  count: 99 },
            { id: 126, count: 20 },
            { id: 127, count: 20 },
            { id: 128, count: 20 },
            { id: 129, count: 20 }
        ];
        itemsToAdd.forEach(entry => {
            const item = $dataItems[entry.id];
            if (item) $gameParty.gainItem(item, entry.count);
        });
        showToast("Survival Kit (+99 Essentials) added!");
    }

    function addSingleItem(id, count, label) {
        if (typeof $gameParty === "undefined" || typeof $dataItems === "undefined") return;
        const item = $dataItems[id];
        if (item) {
            $gameParty.gainItem(item, count);
            showToast("+" + count + " " + (item.name || label) + " added!");
        }
    }

    function addAllItems() {
        if (typeof $gameParty === "undefined" || typeof $dataItems === "undefined") return;
        let count = 0;
        for (let i = 1; i < $dataItems.length; i++) {
            const item = $dataItems[i];
            if (item && item.name && item.name.trim().length > 0) {
                $gameParty.gainItem(item, 10);
                count++;
            }
        }
        showToast("+" + count + " item types (10x each) added!");
    }

    function addAllWeaponsAndArmors() {
        if (typeof $gameParty === "undefined") return;
        let wCount = 0, aCount = 0;
        if (typeof $dataWeapons !== "undefined") {
            for (let i = 1; i < $dataWeapons.length; i++) {
                const w = $dataWeapons[i];
                if (w && w.name && w.name.trim().length > 0) {
                    $gameParty.gainItem(w, 1);
                    wCount++;
                }
            }
        }
        if (typeof $dataArmors !== "undefined") {
            for (let j = 1; j < $dataArmors.length; j++) {
                const a = $dataArmors[j];
                if (a && a.name && a.name.trim().length > 0) {
                    $gameParty.gainItem(a, 1);
                    aCount++;
                }
            }
        }
        showToast("+" + wCount + " Weapons & +" + aCount + " Armors added!");
    }

    // Tab 3: Explore
    function toggleNoclip() {
        noclipActive = !noclipActive;
        if (typeof $gamePlayer !== "undefined") {
            $gamePlayer.setThrough(noclipActive);
        }
        const btn = document.getElementById("btn-cheat-noclip");
        if (btn) {
            btn.textContent = noclipActive ? "[ ON ]" : "[ OFF ]";
            btn.className = "fnh-cheat-btn " + (noclipActive ? "fnh-btn-toggle-on" : "fnh-btn-toggle-off");
        }
        showToast(noclipActive ? "Noclip: ACTIVE" : "Noclip: OFF");
    }

    function cycleMoveSpeed() {
        if (typeof $gamePlayer === "undefined") return;
        if (currentSpeed === 4) currentSpeed = 5;
        else if (currentSpeed === 5) currentSpeed = 6;
        else currentSpeed = 4;

        $gamePlayer.setMoveSpeed(currentSpeed);
        const btn = document.getElementById("btn-cheat-speed");
        const label = currentSpeed === 4 ? "Normal (4)" : currentSpeed === 5 ? "Fast (5)" : "Turbo (6)";
        if (btn) btn.textContent = label;
        showToast("Movement Speed: " + label);
    }

    function toggleFreezeEnemies() {
        freezeEnemiesActive = !freezeEnemiesActive;
        const btn = document.getElementById("btn-cheat-freeze");
        if (btn) {
            btn.textContent = freezeEnemiesActive ? "[ ON ]" : "[ OFF ]";
            btn.className = "fnh-cheat-btn " + (freezeEnemiesActive ? "fnh-btn-toggle-on" : "fnh-btn-toggle-off");
        }
        showToast(freezeEnemiesActive ? "Stealth Mode: ACTIVE" : "Stealth Mode: OFF");
    }

    // Tab 4: Battle
    function instaKillBattleEnemies() {
        if (typeof $gameParty === "undefined" || !$gameParty.inBattle()) {
            showToast("Only available inside battle!");
            return;
        }
        if (typeof $gameTroop !== "undefined") {
            $gameTroop.members().forEach(enemy => {
                if (enemy && enemy.isAlive()) {
                    enemy.setHp(0);
                    enemy.performCollapse();
                }
            });
            if (typeof BattleManager !== "undefined") {
                BattleManager.checkBattleEnd();
            }
            showToast("All enemies defeated!");
        }
    }

    function instantWinBattle() {
        if (typeof $gameParty === "undefined" || !$gameParty.inBattle()) {
            showToast("Only available inside battle!");
            return;
        }
        if (typeof BattleManager !== "undefined") {
            BattleManager.processVictory();
            showToast("Battle won!");
            closeCheatMenu();
        }
    }

    function instantEscapeBattle() {
        if (typeof $gameParty === "undefined" || !$gameParty.inBattle()) {
            showToast("Only available inside battle!");
            return;
        }
        if (typeof BattleManager !== "undefined") {
            BattleManager.processAbort();
            showToast("Escaped successfully!");
            closeCheatMenu();
        }
    }

    // Tab 5: System
    function openSaveMenu() {
        if (typeof SceneManager === "undefined") return;
        if (typeof $gameParty === "undefined" || (!$gameParty.inBattle() && (typeof $gamePlayer === "undefined" || !$gamePlayer.canMove))) {
            showToast("Start or load game before saving!");
            return;
        }
        if (typeof $gameSystem !== "undefined") {
            $gameSystem.enableSave();
        }
        closeCheatMenu();
        SceneManager.push(Scene_Save);
    }

    function openLoadMenu() {
        if (typeof SceneManager === "undefined") return;
        closeCheatMenu();
        SceneManager.push(Scene_Load);
    }

    function openDebugMenu() {
        if (typeof SceneManager === "undefined") return;
        closeCheatMenu();
        SceneManager.push(Scene_Debug);
    }

    function toggleTheme() {
        const isRetro = document.body.classList.contains("vpad-theme-retro");
        const nextTheme = isRetro ? "clean" : "retro";
        if (typeof window.applyVpadTheme === "function") {
            window.applyVpadTheme(nextTheme);
        } else {
            document.body.classList.toggle("vpad-theme-retro", nextTheme === "retro");
        }
        const btn = document.getElementById("btn-cheat-theme-toggle");
        if (btn) btn.textContent = nextTheme === "retro" ? "Retro OSD" : "Clean Slate";
        showToast("UI Theme: " + (nextTheme === "retro" ? "Retro OSD (Orange)" : "Clean Slate"));
    }

    // SVG Helper Icons (Monochrome, Inline, 100% Offline)
    const ICONS = {
        heart: `<svg viewBox="0 0 24 24"><path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"></path></svg>`,
        shield: `<svg viewBox="0 0 24 24"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"></path></svg>`,
        flame: `<svg viewBox="0 0 24 24"><path d="M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z"></path></svg>`,
        brain: `<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="9"></circle><path d="M12 3v18"></path><path d="M3 12h18"></path></svg>`,
        zap: `<svg viewBox="0 0 24 24"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"></polygon></svg>`,
        bag: `<svg viewBox="0 0 24 24"><path d="M6 2L3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4z"></path><line x1="3" y1="6" x2="21" y2="6"></line><path d="M16 10a4 4 0 0 1-8 0"></path></svg>`,
        coin: `<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="9"></circle><path d="M14.8 9A2 2 0 0 0 13 8h-2a2 2 0 0 0 0 4h2a2 2 0 0 1 0 4h-2a2 2 0 0 1-1.8-1"></path><path d="M12 6v2m0 8v2"></path></svg>`,
        sword: `<svg viewBox="0 0 24 24"><polyline points="14.5 17.5 3 6 3 3 6 3 17.5 14.5"></polyline><line x1="13" y1="19" x2="19" y2="13"></line><line x1="16" y1="16" x2="20" y2="20"></line><line x1="19" y1="21" x2="21" y2="19"></line></svg>`,
        compass: `<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="10"></circle><polygon points="16.24 7.76 14.12 14.12 7.76 16.24 9.88 9.88 16.24 7.76"></polygon></svg>`,
        save: `<svg viewBox="0 0 24 24"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"></path><polyline points="17 21 17 13 7 13 7 21"></polyline><polyline points="7 3 7 8 15 8"></polyline></svg>`,
        folder: `<svg viewBox="0 0 24 24"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>`,
        tool: `<svg viewBox="0 0 24 24"><path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"></path></svg>`
    };

    // Render Compact Rows
    function renderTabContent() {
        const body = document.getElementById("fnh-cheat-body");
        if (!body) return;
        body.innerHTML = "";

        if (activeTab === "tab-char") {
            body.innerHTML = `
                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.heart}</span>
                        <span>Restore Limbs & Health</span>
                    </div>
                    <button class="fnh-cheat-btn fnh-btn-action" onclick="window.FNHCheat.heal()">Restore</button>
                </div>

                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.shield}</span>
                        <span>God Mode (Invulnerable)</span>
                    </div>
                    <button id="btn-cheat-godmode" class="fnh-cheat-btn ${godModeActive ? 'fnh-btn-toggle-on' : 'fnh-btn-toggle-off'}" onclick="window.FNHCheat.godMode()">
                        ${godModeActive ? '[ ON ]' : '[ OFF ]'}
                    </button>
                </div>

                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.flame}</span>
                        <span>Reset Starvation</span>
                    </div>
                    <button class="fnh-cheat-btn" onclick="window.FNHCheat.hunger()">Reset</button>
                </div>

                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.brain}</span>
                        <span>Sanity / Mind 100%</span>
                    </div>
                    <button class="fnh-cheat-btn" onclick="window.FNHCheat.mind()">Max</button>
                </div>

                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.zap}</span>
                        <span>Revive Fallen Party</span>
                    </div>
                    <button class="fnh-cheat-btn" onclick="window.FNHCheat.revive()">Revive</button>
                </div>
            `;
        } else if (activeTab === "tab-item") {
            body.innerHTML = `
                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.bag}</span>
                        <span>Survival Kit (+99 All Essentials)</span>
                    </div>
                    <button class="fnh-cheat-btn fnh-btn-action" onclick="window.FNHCheat.kit()">Claim</button>
                </div>

                <div class="fnh-cheat-grid">
                    <button class="fnh-grid-btn" onclick="window.FNHCheat.add(59, 1000, 'Silver coins')">🪙 +1,000 Silver</button>
                    <button class="fnh-grid-btn" onclick="window.FNHCheat.add(201, 50, 'Lucky coins')">🍀 +50 Lucky Coins</button>
                    <button class="fnh-grid-btn" onclick="window.FNHCheat.add(40, 20, 'Book of enlightenment')">📖 +20 Save Books</button>
                    <button class="fnh-grid-btn" onclick="window.FNHCheat.add(20, 50, 'Skeleton key')">🔑 +50 Small Keys</button>
                    <button class="fnh-grid-btn" onclick="window.FNHCheat.add(5, 50, 'Torch'); window.FNHCheat.add(7, 50, 'Tinderbox');">🔥 +50 Torches</button>
                    <button class="fnh-grid-btn" onclick="window.FNHCheat.add(61, 50, 'Green vial'); window.FNHCheat.add(34, 50, 'Blood vial'); window.FNHCheat.add(28, 50, 'Blue vial');">🧪 +50 Potions</button>
                </div>

                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.coin}</span>
                        <span>Unlock All Items (x10)</span>
                    </div>
                    <button class="fnh-cheat-btn" onclick="window.FNHCheat.allItems()">Give All</button>
                </div>

                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.sword}</span>
                        <span>Unlock Weapons & Armors (x1)</span>
                    </div>
                    <button class="fnh-cheat-btn" onclick="window.FNHCheat.allEquip()">Give All</button>
                </div>
            `;
        } else if (activeTab === "tab-explore") {
            body.innerHTML = `
                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.compass}</span>
                        <span>Noclip (Walk Through Walls)</span>
                    </div>
                    <button id="btn-cheat-noclip" class="fnh-cheat-btn ${noclipActive ? 'fnh-btn-toggle-on' : 'fnh-btn-toggle-off'}" onclick="window.FNHCheat.noclip()">
                        ${noclipActive ? '[ ON ]' : '[ OFF ]'}
                    </button>
                </div>

                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.zap}</span>
                        <span>Movement Speed</span>
                    </div>
                    <button id="btn-cheat-speed" class="fnh-cheat-btn" onclick="window.FNHCheat.speed()">
                        ${currentSpeed === 4 ? 'Normal (4)' : (currentSpeed === 5 ? 'Fast (5)' : 'Turbo (6)')}
                    </button>
                </div>

                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.shield}</span>
                        <span>Stealth (Ignore Enemies)</span>
                    </div>
                    <button id="btn-cheat-freeze" class="fnh-cheat-btn ${freezeEnemiesActive ? 'fnh-btn-toggle-on' : 'fnh-btn-toggle-off'}" onclick="window.FNHCheat.freeze()">
                        ${freezeEnemiesActive ? '[ ON ]' : '[ OFF ]'}
                    </button>
                </div>
            `;
        } else if (activeTab === "tab-battle") {
            body.innerHTML = `
                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.sword}</span>
                        <span>Instant Kill Enemies</span>
                    </div>
                    <button class="fnh-cheat-btn fnh-btn-action" style="background:#dc2626; border-color:#ef4444;" onclick="window.FNHCheat.kill()">Execute</button>
                </div>

                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.shield}</span>
                        <span>Instant Victory</span>
                    </div>
                    <button class="fnh-cheat-btn" onclick="window.FNHCheat.win()">Win</button>
                </div>

                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.zap}</span>
                        <span>Instant Flee (100% Escape)</span>
                    </div>
                    <button class="fnh-cheat-btn" onclick="window.FNHCheat.escape()">Escape</button>
                </div>
            `;
        } else if (activeTab === "tab-sys") {
            const isRetro = document.body.classList.contains("vpad-theme-retro");
            body.innerHTML = `
                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.save}</span>
                        <span>Save Game Anywhere</span>
                    </div>
                    <button class="fnh-cheat-btn fnh-btn-action" onclick="window.FNHCheat.save()">Save</button>
                </div>

                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.folder}</span>
                        <span>Load Save File</span>
                    </div>
                    <button class="fnh-cheat-btn" onclick="window.FNHCheat.load()">Load</button>
                </div>

                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.tool}</span>
                        <span>Engine Debug Panel (F9)</span>
                    </div>
                    <button class="fnh-cheat-btn" onclick="window.FNHCheat.debug()">Debug</button>
                </div>

                <div class="fnh-cheat-row">
                    <div class="fnh-cheat-left">
                        <span class="fnh-cheat-icon">${ICONS.tool}</span>
                        <span>UI Theme</span>
                    </div>
                    <button id="btn-cheat-theme-toggle" class="fnh-cheat-btn" onclick="window.FNHCheat.theme()">
                        ${isRetro ? 'Retro OSD' : 'Clean Slate'}
                    </button>
                </div>
            `;
        }
    }

    function switchTab(tabId) {
        const modal = document.getElementById("fnh-cheat-overlay");
        if (!modal) return;
        const tabs = modal.querySelectorAll(".fnh-cheat-tab");
        tabs.forEach(t => {
            t.classList.toggle("active", t.getAttribute("data-tab") === tabId);
        });
        activeTab = tabId;
        renderTabContent();
    }

    function toggleCheatMenu() {
        if (isMenuOpen) closeCheatMenu();
        else openCheatMenu();
    }

    function openCheatMenu() {
        const overlay = document.getElementById("fnh-cheat-overlay");
        if (!overlay) return;
        renderTabContent();
        overlay.style.display = "flex";
        isMenuOpen = true;

        const cheatBtn = document.getElementById("vpad-btn-cheat");
        if (cheatBtn) cheatBtn.classList.add("active");
    }

    function closeCheatMenu() {
        const overlay = document.getElementById("fnh-cheat-overlay");
        if (!overlay) return;
        overlay.style.display = "none";
        isMenuOpen = false;

        const cheatBtn = document.getElementById("vpad-btn-cheat");
        if (cheatBtn) cheatBtn.classList.remove("active");
    }

    function initCheatDOM() {
        injectStyles();

        let topBar = document.getElementById("vpad-topbar");
        if (!topBar) {
            topBar = document.createElement("div");
            topBar.id = "vpad-topbar";
            document.body.appendChild(topBar);
        }

        let cheatBtn = document.getElementById("vpad-btn-cheat");
        if (!cheatBtn) {
            cheatBtn = document.createElement("div");
            cheatBtn.id = "vpad-btn-cheat";
            cheatBtn.className = "vpad-top-btn cheat-interactive";
            cheatBtn.title = "Cheat Menu";
            cheatBtn.innerHTML = `<svg viewBox="0 0 24 24"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>`;
            cheatBtn.setAttribute("onclick", "window.FNHCheat.toggle()");
            topBar.appendChild(cheatBtn);
        } else {
            cheatBtn.setAttribute("onclick", "window.FNHCheat.toggle()");
        }

        if (document.getElementById("fnh-cheat-overlay")) return;

        const modal = document.createElement("div");
        modal.id = "fnh-cheat-overlay";
        modal.className = "cheat-interactive";
        modal.innerHTML = `
            <div id="fnh-cheat-header">
                <div class="fnh-cheat-title">
                    <svg viewBox="0 0 24 24"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
                    <span>CHEAT MENU</span>
                </div>
                <div class="fnh-cheat-close" id="fnh-cheat-close-btn" title="Close" onclick="window.FNHCheat.close()">✕</div>
            </div>
            <div id="fnh-cheat-tabs">
                <div class="fnh-cheat-tab active" data-tab="tab-char" onclick="window.FNHCheat.setTab('tab-char')">
                    ${ICONS.heart} <span>Character</span>
                </div>
                <div class="fnh-cheat-tab" data-tab="tab-item" onclick="window.FNHCheat.setTab('tab-item')">
                    ${ICONS.bag} <span>Items</span>
                </div>
                <div class="fnh-cheat-tab" data-tab="tab-explore" onclick="window.FNHCheat.setTab('tab-explore')">
                    ${ICONS.compass} <span>Explore</span>
                </div>
                <div class="fnh-cheat-tab" data-tab="tab-battle" onclick="window.FNHCheat.setTab('tab-battle')">
                    ${ICONS.sword} <span>Battle</span>
                </div>
                <div class="fnh-cheat-tab" data-tab="tab-sys" onclick="window.FNHCheat.setTab('tab-sys')">
                    ${ICONS.save} <span>System</span>
                </div>
            </div>
            <div id="fnh-cheat-body"></div>
        `;
        document.body.appendChild(modal);

        // Draggable Header
        const header = document.getElementById("fnh-cheat-header");
        let isDragging = false;
        let startX, startY, initLeft, initTop;

        function startDrag(clientX, clientY) {
            isDragging = true;
            startX = clientX;
            startY = clientY;
            const rect = modal.getBoundingClientRect();
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
            nX = Math.max(10, Math.min(window.innerWidth - modal.offsetWidth - 10, nX));
            nY = Math.max(10, Math.min(window.innerHeight - modal.offsetHeight - 10, nY));
            modal.style.left = nX + "px";
            modal.style.top = nY + "px";
        }

        header.addEventListener("mousedown", (e) => {
            if (e.target.id === "fnh-cheat-close-btn") return;
            startDrag(e.clientX, e.clientY);
        });
        header.addEventListener("touchstart", (e) => {
            if (e.target.id === "fnh-cheat-close-btn") return;
            if (e.touches.length === 1) {
                startDrag(e.touches[0].clientX, e.touches[0].clientY);
            }
        }, { passive: true });

        document.addEventListener("mousemove", (e) => {
            if (isDragging) moveDrag(e.clientX, e.clientY);
        });
        document.addEventListener("touchmove", (e) => {
            if (isDragging && e.touches.length === 1) {
                moveDrag(e.touches[0].clientX, e.touches[0].clientY);
            }
        }, { passive: true });

        const endDrag = () => { isDragging = false; };
        document.addEventListener("mouseup", endDrag);
        document.addEventListener("touchend", endDrag);

        document.addEventListener("keydown", (e) => {
            if (e.key === "`" || e.key === "~" || e.code === "Backquote" || e.code === "F10") {
                toggleCheatMenu();
            }
        });
    }

    window.FNHCheat = {
        open: openCheatMenu,
        close: closeCheatMenu,
        toggle: toggleCheatMenu,
        setTab: switchTab,
        heal: fullHealAndRestoreLimbs,
        godMode: toggleGodMode,
        hunger: resetHunger,
        mind: maxMind,
        revive: reviveAllMembers,
        kit: addEssentialSurvivalKit,
        add: addSingleItem,
        allItems: addAllItems,
        allEquip: addAllWeaponsAndArmors,
        noclip: toggleNoclip,
        speed: cycleMoveSpeed,
        freeze: toggleFreezeEnemies,
        kill: instaKillBattleEnemies,
        win: instantWinBattle,
        escape: instantEscapeBattle,
        save: openSaveMenu,
        load: openLoadMenu,
        debug: openDebugMenu,
        theme: toggleTheme
    };

    function startup() {
        hookGameLoop();
        initCheatDOM();
    }

    if (document.readyState === "complete" || document.readyState === "interactive") {
        setTimeout(startup, 250);
    } else {
        document.addEventListener("DOMContentLoaded", () => setTimeout(startup, 250));
    }
    window.addEventListener("load", () => {
        setTimeout(startup, 800);
    });
})();
