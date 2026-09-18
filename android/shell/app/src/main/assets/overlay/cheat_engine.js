// ============================================================================
// Feature Module: cheat_engine.js
// Profile-Driven In-Game Cheat Engine for RPG Maker MV/MZ
// ============================================================================

(function () {
    "use strict";

    const globalObj = typeof window !== "undefined" ? window : (typeof global !== "undefined" ? global : this);

    const cheatEngine = {
        profile: globalObj.__gameProfile || null,

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

                // Tipe A: Actor EXP (seperti Satiety di F&H2)
                if (mech.type === "actor_exp") {
                    if (typeof $gameParty !== "undefined" && $gameParty.allMembers) {
                        const targetVal = mech.max_val || 100;
                        $gameParty.allMembers().forEach(function (actor) {
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

                // Tipe B: Switches + Common Event (seperti Limb Recovery di F&H2)
                if (mech.type === "switches_and_common_event") {
                    if (Array.isArray(mech.switches_to_disable) && typeof $gameSwitches !== "undefined") {
                        mech.switches_to_disable.forEach(function (swId) {
                            $gameSwitches.setValue(swId, false);
                        });
                    }
                    if (mech.refresh_common_event && typeof $gameTemp !== "undefined" && $gameTemp.reserveCommonEvent) {
                        $gameTemp.reserveCommonEvent(mech.refresh_common_event);
                    }
                    if (typeof $gamePlayer !== "undefined" && $gamePlayer.refresh) {
                        $gamePlayer.refresh();
                    }
                    return { success: true, message: "Restored limbs and refreshed character." };
                }

                // Tipe C: Scene Push (Save Anywhere)
                if (mech.type === "scene_push") {
                    if (typeof SceneManager !== "undefined") {
                        const targetScene = globalObj[mech.scene];
                        if (targetScene && SceneManager.push) {
                            SceneManager.push(targetScene);
                            return { success: true, message: `Opened scene: ${mech.scene}` };
                        }
                    }
                }

                // Tipe D: Gain Gold
                if (mech.type === "gain_gold") {
                    if (typeof $gameParty !== "undefined" && $gameParty.gainGold) {
                        $gameParty.gainGold(mech.amount || 999999);
                        return { success: true, message: `Added gold: ${mech.amount || 999999}` };
                    }
                }
            }

            // Fallback generic safe actions (bila profil tidak memiliki definisi khusus)
            if (actionKey === "heal_all") {
                if (typeof $gameParty !== "undefined" && $gameParty.allMembers) {
                    $gameParty.allMembers().forEach(function (actor) {
                        if (actor && actor.recoverAll) actor.recoverAll();
                    });
                    return { success: true, message: "Party fully recovered." };
                }
            }

            if (actionKey === "save_anywhere") {
                if (typeof SceneManager !== "undefined" && typeof Scene_Save !== "undefined") {
                    SceneManager.push(Scene_Save);
                    return { success: true, message: "Save menu opened." };
                }
            }

            return { success: false, error: `Unhandled or unknown action: ${actionKey}` };
        }
    };

    globalObj.__cheatEngine = cheatEngine;
    console.log("[RPG-Suite] cheat_engine.js initialized.");
})();
