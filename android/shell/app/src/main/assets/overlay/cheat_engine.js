// ============================================================================
// Feature Module: cheat_engine.js
// Engine-Agnostic Core Cheat Runner & Primitives for RuntimePack
// Supports Native RGSS (mkxp-z atomic IPC) & Web MV/MZ (direct JS bridge)
// ============================================================================

(function () {
    "use strict";

    const globalObj = typeof window !== "undefined" ? window : (typeof global !== "undefined" ? global : this);

    let lastDumpedState = { gold: 0, variables: {}, switches: {} };

    const CheatEngine = {
        _modeOverride: null, // "native", "web", or null for auto-detect
        profile: globalObj.__gameProfile || null,

        setMode: function (mode) {
            this._modeOverride = mode;
        },

        isNative: function () {
            if (this._modeOverride === "native") return true;
            if (this._modeOverride === "web") return false;
            return typeof globalObj.RuntimePackVPad !== "undefined" &&
                   typeof globalObj.RuntimePackVPad.postCheatCmd === "function";
        },

        postCommand: function (cmd) {
            if (this.isNative()) {
                const jsonStr = typeof cmd === "string" ? cmd : JSON.stringify(cmd);
                return globalObj.RuntimePackVPad.postCheatCmd(jsonStr);
            }
            return false;
        },

        setVariable: function (id, value) {
            const numId = Number(id);
            if (this.isNative()) {
                return this.postCommand({ action: "set_variable", id: numId, value: value });
            }
            // Web MV/MZ mode
            if (typeof globalObj.$gameVariables !== "undefined" && typeof globalObj.$gameVariables.setValue === "function") {
                globalObj.$gameVariables.setValue(numId, value);
                return true;
            }
            return false;
        },

        setSwitch: function (id, value) {
            const numId = Number(id);
            const boolVal = Boolean(value);
            if (this.isNative()) {
                return this.postCommand({ action: "set_switch", id: numId, value: boolVal });
            }
            // Web MV/MZ mode
            if (typeof globalObj.$gameSwitches !== "undefined" && typeof globalObj.$gameSwitches.setValue === "function") {
                globalObj.$gameSwitches.setValue(numId, boolVal);
                return true;
            }
            return false;
        },

        gainGold: function (amount) {
            const numAmt = Number(amount);
            if (this.isNative()) {
                return this.postCommand({ action: "gain_gold", amount: numAmt });
            }
            // Web MV/MZ mode
            if (typeof globalObj.$gameParty !== "undefined" && typeof globalObj.$gameParty.gainGold === "function") {
                globalObj.$gameParty.gainGold(numAmt);
                return true;
            }
            return false;
        },

        recoverAll: function () {
            if (this.isNative()) {
                return this.postCommand({ action: "recover_all" });
            }
            // Web MV/MZ mode
            if (typeof globalObj.$gameParty !== "undefined") {
                const members = typeof globalObj.$gameParty.allMembers === "function"
                    ? globalObj.$gameParty.allMembers()
                    : (typeof globalObj.$gameParty.members === "function" ? globalObj.$gameParty.members() : []);
                if (Array.isArray(members)) {
                    members.forEach(function (actor) {
                        if (actor && typeof actor.recoverAll === "function") {
                            actor.recoverAll();
                        }
                    });
                    return true;
                }
            }
            return false;
        },

        gainItem: function (type, id, amount) {
            const strType = String(type).toLowerCase();
            const numId = Number(id);
            const numAmt = Number(amount);
            if (this.isNative()) {
                return this.postCommand({ action: "gain_item", type: strType, id: numId, amount: numAmt });
            }
            // Web MV/MZ mode
            if (typeof globalObj.$gameParty !== "undefined" && typeof globalObj.$gameParty.gainItem === "function") {
                let db = null;
                if (strType === "weapon" && typeof globalObj.$dataWeapons !== "undefined") {
                    db = globalObj.$dataWeapons;
                } else if (strType === "armor" && typeof globalObj.$dataArmors !== "undefined") {
                    db = globalObj.$dataArmors;
                } else if (typeof globalObj.$dataItems !== "undefined") {
                    db = globalObj.$dataItems;
                }
                const item = db ? db[numId] : null;
                if (item) {
                    globalObj.$gameParty.gainItem(item, numAmt);
                    return true;
                }
            }
            return false;
        },

        evalScript: function (code, callback) {
            const strCode = String(code);
            if (this.isNative()) {
                this.postCommand({ action: "eval", code: strCode });
                if (typeof callback === "function") {
                    // Check if immediate result is available (synchronous mock / already generated)
                    let immediate = "";
                    if (typeof globalObj.RuntimePackVPad !== "undefined" && typeof globalObj.RuntimePackVPad.getEvalResult === "function") {
                        immediate = globalObj.RuntimePackVPad.getEvalResult();
                    }
                    if (immediate) {
                        callback(immediate);
                        return true;
                    }
                    // Poll asynchronously
                    let attempts = 0;
                    const maxAttempts = 30;
                    const interval = setInterval(function () {
                        attempts++;
                        let res = "";
                        if (typeof globalObj.RuntimePackVPad !== "undefined" && typeof globalObj.RuntimePackVPad.getEvalResult === "function") {
                            res = globalObj.RuntimePackVPad.getEvalResult();
                        }
                        if (res || attempts >= maxAttempts) {
                            clearInterval(interval);
                            callback(res || null);
                        }
                    }, 50);
                }
                return true;
            }
            // Web MV/MZ mode
            try {
                const res = (function () { return eval(strCode); })();
                if (typeof callback === "function") {
                    callback(res !== undefined ? String(res) : "");
                }
                return res;
            } catch (err) {
                if (typeof callback === "function") {
                    callback("[CHT-002] " + err.name + ": " + err.message);
                }
                return null;
            }
        },

        requestStateDump: function () {
            if (this.isNative()) {
                return this.postCommand({ action: "dump_state" });
            }
            // Web MV/MZ mode
            const gold = (typeof globalObj.$gameParty !== "undefined" && typeof globalObj.$gameParty.gold === "function")
                ? globalObj.$gameParty.gold()
                : 0;
            const vars = {};
            const switches = {};
            if (typeof globalObj.$gameVariables !== "undefined" && typeof globalObj.$gameVariables.value === "function") {
                for (let i = 1; i <= 300; i++) {
                    const v = globalObj.$gameVariables.value(i);
                    if (v !== 0 && v !== undefined && v !== null) {
                        vars[String(i)] = v;
                    }
                }
            }
            if (typeof globalObj.$gameSwitches !== "undefined" && typeof globalObj.$gameSwitches.value === "function") {
                for (let i = 1; i <= 300; i++) {
                    const s = globalObj.$gameSwitches.value(i);
                    if (s) {
                        switches[String(i)] = true;
                    }
                }
            }
            lastDumpedState = { gold: gold, variables: vars, switches: switches };
            return true;
        },

        getLoadedState: function () {
            if (this.isNative()) {
                if (typeof globalObj.RuntimePackVPad !== "undefined" && typeof globalObj.RuntimePackVPad.getCheatState === "function") {
                    const stateStr = globalObj.RuntimePackVPad.getCheatState();
                    try {
                        lastDumpedState = JSON.parse(stateStr);
                    } catch (e) {
                        // Keep previous or empty
                    }
                }
                return lastDumpedState;
            }
            // Web MV/MZ mode
            if (!lastDumpedState || (Object.keys(lastDumpedState.variables || {}).length === 0 && lastDumpedState.gold === 0)) {
                this.requestStateDump();
            }
            return lastDumpedState;
        },

        // Profile-Driven Action Executor (Backward Compatibility)
        executeAction: function (actionKey) {
            // 1. Cek fitur terlarang
            if (this.profile && Array.isArray(this.profile.forbidden_features)) {
                if (this.profile.forbidden_features.indexOf(actionKey) !== -1) {
                    console.warn(`[CheatEngine] Action '${actionKey}' is forbidden by game profile!`);
                    return { success: false, error: "Action is forbidden by game profile." };
                }
            }

            // 2. Eksekusi aksi berbasis profil
            if (this.profile && this.profile.mechanics && this.profile.mechanics[actionKey]) {
                const mech = this.profile.mechanics[actionKey];

                if (mech.type === "actor_exp") {
                    if (typeof globalObj.$gameParty !== "undefined" && globalObj.$gameParty.allMembers) {
                        const targetVal = mech.max_val || 100;
                        globalObj.$gameParty.allMembers().forEach(function (actor) {
                            if (actor && actor.changeExp) {
                                actor.changeExp(targetVal, false);
                            }
                            if (Array.isArray(mech.states_to_remove) && actor.removeState) {
                                mech.states_to_remove.forEach(function (stId) {
                                    actor.removeState(stId);
                                });
                            }
                        });
                        return { success: true, message: `Set EXP to ${targetVal} for all party members.` };
                    }
                }

                if (mech.type === "switches_and_common_event") {
                    if (Array.isArray(mech.switches_to_disable) && typeof globalObj.$gameSwitches !== "undefined") {
                        mech.switches_to_disable.forEach(function (swId) {
                            globalObj.$gameSwitches.setValue(swId, false);
                        });
                    }
                    if (mech.refresh_common_event && typeof globalObj.$gameTemp !== "undefined" && globalObj.$gameTemp.reserveCommonEvent) {
                        globalObj.$gameTemp.reserveCommonEvent(mech.refresh_common_event);
                    }
                    if (typeof globalObj.$gamePlayer !== "undefined" && globalObj.$gamePlayer.refresh) {
                        globalObj.$gamePlayer.refresh();
                    }
                    return { success: true, message: "Restored limbs and refreshed character." };
                }

                if (mech.type === "scene_push") {
                    if (typeof globalObj.SceneManager !== "undefined") {
                        const targetScene = globalObj[mech.scene];
                        if (targetScene && globalObj.SceneManager.push) {
                            globalObj.SceneManager.push(targetScene);
                            return { success: true, message: `Opened scene: ${mech.scene}` };
                        }
                    }
                }

                if (mech.type === "gain_gold") {
                    this.gainGold(mech.amount || 999999);
                    return { success: true, message: `Added gold: ${mech.amount || 999999}` };
                }
            }

            // Fallback generic safe actions
            if (actionKey === "heal_all") {
                this.recoverAll();
                return { success: true, message: "Party fully recovered." };
            }

            if (actionKey === "save_anywhere") {
                if (typeof globalObj.SceneManager !== "undefined" && typeof globalObj.Scene_Save !== "undefined") {
                    globalObj.SceneManager.push(globalObj.Scene_Save);
                    return { success: true, message: "Save menu opened." };
                }
            }

            return { success: false, error: `Unhandled or unknown action: ${actionKey}` };
        }
    };

    globalObj.CheatEngine = CheatEngine;
    globalObj.__cheatEngine = CheatEngine;

    if (typeof module !== "undefined" && module.exports) {
        module.exports = CheatEngine;
    }

    console.log("[RuntimePack] cheat_engine.js initialized.");
})();
