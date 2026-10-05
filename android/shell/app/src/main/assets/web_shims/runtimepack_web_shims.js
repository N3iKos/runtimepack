(function(root) {
    if (typeof root.window !== "undefined" && typeof root.window.addEventListener === "function") {
        root.window.addEventListener("error", function(e) {
            try {
                var errObj = {
                    message: (e && e.message) ? e.message : String(e),
                    file: (e && e.filename) ? e.filename : "",
                    line: (e && typeof e.lineno === "number") ? e.lineno : 0,
                    column: (e && typeof e.colno === "number") ? e.colno : 0,
                    stack: (e && e.error && e.error.stack) ? e.error.stack : "",
                    kind: "error",
                    source: "GAME"
                };
                var jsonStr = JSON.stringify(errObj);
                if (root.RuntimePackWeb && typeof root.RuntimePackWeb.postDiagnosticError === "function") {
                    root.RuntimePackWeb.postDiagnosticError(jsonStr);
                } else if (root.RuntimePackNative && typeof root.RuntimePackNative.postDiagnosticError === "function") {
                    root.RuntimePackNative.postDiagnosticError(jsonStr);
                } else if (root.RuntimePackVPad && typeof root.RuntimePackVPad.postDiagnosticError === "function") {
                    root.RuntimePackVPad.postDiagnosticError(jsonStr);
                }
            } catch (_) {}
            if (root.console && typeof root.console.error === "function") {
                root.console.error("[RuntimePack Global Error]", e.message, "at", e.filename, ":", e.lineno, ":", e.colno, e.error && e.error.stack ? e.error.stack : "");
            }
        });
        root.window.addEventListener("unhandledrejection", function(e) {
            try {
                var reason = e ? e.reason : null;
                var msg = reason ? (reason.message || String(reason)) : "Unhandled Promise Rejection";
                var stack = reason && reason.stack ? reason.stack : "";
                var errObj = {
                    message: msg,
                    stack: stack,
                    kind: "unhandledrejection",
                    source: "GAME"
                };
                var jsonStr = JSON.stringify(errObj);
                if (root.RuntimePackWeb && typeof root.RuntimePackWeb.postDiagnosticError === "function") {
                    root.RuntimePackWeb.postDiagnosticError(jsonStr);
                } else if (root.RuntimePackNative && typeof root.RuntimePackNative.postDiagnosticError === "function") {
                    root.RuntimePackNative.postDiagnosticError(jsonStr);
                } else if (root.RuntimePackVPad && typeof root.RuntimePackVPad.postDiagnosticError === "function") {
                    root.RuntimePackVPad.postDiagnosticError(jsonStr);
                }
            } catch (_) {}
            if (root.console && typeof root.console.error === "function") {
                root.console.error("[RuntimePack Unhandled Rejection]", e.reason);
            }
        });
    }

    // [CAP-WEB-008] StorageManager.isLocalMode virtualization
    function patchStorageManager(sm) {
        if (!sm) return;
        try {
            Object.defineProperty(sm, "isLocalMode", {
                configurable: true,
                enumerable: true,
                get: function() {
                    return function() { return false; };
                },
                set: function() {}
            });
        } catch (e) {
            sm.isLocalMode = function() { return false; };
        }

        if (typeof sm.saveToForage === "function" || typeof sm.loadFromForage === "function") {
            patchMzStorage(sm);
        }
        if (typeof sm.saveToWebStorage === "function" || typeof sm.loadFromWebStorage === "function") {
            patchMvStorage(sm);
        }
    }

    // [CAP-WEB-002] RPG Maker MZ universal storage adapter
    function patchMzStorage(sm) {
        if (!sm || sm.__runtimepack_mz_save_patched) return;
        sm.__runtimepack_mz_save_patched = true;

        sm._forageKeys = sm._forageKeys || [];
        sm._forageKeysUpdated = false;

        var encodeBase64 = function(str) {
            try {
                if (typeof root.btoa === "function") return root.btoa(str);
                if (typeof window !== "undefined" && typeof window.btoa === "function") return window.btoa(str);
                if (typeof Buffer !== "undefined") return Buffer.from(str, "binary").toString("base64");
            } catch (e) {}
            return str;
        };

        var decodeBase64 = function(b64) {
            try {
                if (typeof root.atob === "function") return root.atob(b64);
                if (typeof window !== "undefined" && typeof window.atob === "function") return window.atob(b64);
                if (typeof Buffer !== "undefined") return Buffer.from(b64, "base64").toString("binary");
            } catch (e) {}
            return b64;
        };

        sm.saveToForage = function(saveName, zip) {
            return new Promise(function(resolve, reject) {
                var nativeBridge = (typeof root.window !== "undefined" && root.window.RuntimePackNative)
                    || root.RuntimePackNative || (typeof window !== "undefined" && window.RuntimePackNative);
                if (nativeBridge && typeof nativeBridge.saveSlot === "function") {
                    var b64Payload = encodeBase64(zip);
                    var ok = nativeBridge.saveSlot("mz", String(saveName), b64Payload);
                    if (ok) {
                        if (!sm._forageKeys.includes(saveName)) {
                            sm._forageKeys.push(saveName);
                        }
                        resolve(0);
                    } else {
                        reject(new Error("[RuntimePack] Native save failed for slot: " + saveName));
                    }
                } else {
                    reject(new Error("[RuntimePack] Native save bridge unavailable"));
                }
            });
        };

        sm.loadFromForage = function(saveName) {
            return new Promise(function(resolve, reject) {
                var nativeBridge = (typeof root.window !== "undefined" && root.window.RuntimePackNative)
                    || root.RuntimePackNative || (typeof window !== "undefined" && window.RuntimePackNative);
                if (nativeBridge && typeof nativeBridge.loadSlot === "function") {
                    var rawB64 = nativeBridge.loadSlot("mz", String(saveName));
                    if (rawB64 !== null && rawB64 !== undefined) {
                        var decodedZip = decodeBase64(rawB64);
                        resolve(decodedZip);
                    } else {
                        resolve(null);
                    }
                } else {
                    resolve(null);
                }
            });
        };

        sm.forageExists = function(saveName) {
            var nativeBridge = (typeof root.window !== "undefined" && root.window.RuntimePackNative)
                || root.RuntimePackNative || (typeof window !== "undefined" && window.RuntimePackNative);
            if (sm._forageKeys && sm._forageKeys.includes(saveName)) {
                return true;
            }
            if (nativeBridge && typeof nativeBridge.slotExists === "function") {
                return nativeBridge.slotExists("mz", String(saveName));
            }
            return false;
        };

        sm.removeForage = function(saveName) {
            return new Promise(function(resolve) {
                var nativeBridge = (typeof root.window !== "undefined" && root.window.RuntimePackNative)
                    || root.RuntimePackNative || (typeof window !== "undefined" && window.RuntimePackNative);
                if (nativeBridge && typeof nativeBridge.deleteSlot === "function") {
                    nativeBridge.deleteSlot("mz", String(saveName));
                }
                var idx = sm._forageKeys.indexOf(saveName);
                if (idx !== -1) {
                    sm._forageKeys.splice(idx, 1);
                }
                resolve(0);
            });
        };

        sm.updateForageKeys = function() {
            sm._forageKeysUpdated = false;
            return new Promise(function(resolve) {
                var nativeBridge = (typeof root.window !== "undefined" && root.window.RuntimePackNative)
                    || root.RuntimePackNative || (typeof window !== "undefined" && window.RuntimePackNative);
                if (nativeBridge && typeof nativeBridge.listSlots === "function") {
                    try {
                        var listRaw = nativeBridge.listSlots("mz");
                        var parsed = JSON.parse(listRaw);
                        if (Array.isArray(parsed)) {
                            sm._forageKeys = parsed;
                        }
                    } catch (e) {
                        sm._forageKeys = [];
                    }
                }
                sm._forageKeysUpdated = true;
                resolve(0);
            });
        };

        sm.forageKeysUpdated = function() {
            return sm._forageKeysUpdated;
        };

        sm.updateForageKeys();
    }

    // [CAP-WEB-003] RPG Maker MV universal webstorage adapter
    function patchMvStorage(sm) {
        if (!sm || sm.__runtimepack_mv_save_patched) return;
        sm.__runtimepack_mv_save_patched = true;

        var getLZString = function() {
            return (typeof root.LZString !== "undefined" && root.LZString)
                || (typeof window !== "undefined" && window.LZString)
                || (typeof LZString !== "undefined" ? LZString : null);
        };

        var getNativeBridge = function() {
            return (typeof root.window !== "undefined" && root.window.RuntimePackNative)
                || root.RuntimePackNative
                || (typeof window !== "undefined" && window.RuntimePackNative);
        };

        var mapMvSlot = function(savefileId) {
            if (typeof savefileId === "string") {
                if (savefileId === "config" || savefileId === "global" || /^file\d+$/.test(savefileId)) {
                    return savefileId;
                }
            }
            if (savefileId < 0) return "config";
            if (savefileId === 0) return "global";
            return "file" + savefileId;
        };

        sm.saveToWebStorage = function(savefileId, json) {
            var nativeBridge = getNativeBridge();
            if (nativeBridge && typeof nativeBridge.saveSlot === "function") {
                var lz = getLZString();
                var data = (lz && typeof lz.compressToBase64 === "function") ? lz.compressToBase64(json) : json;
                var slot = mapMvSlot(savefileId);
                return nativeBridge.saveSlot("mv", slot, data);
            }
            return false;
        };

        sm.loadFromWebStorage = function(savefileId) {
            var nativeBridge = getNativeBridge();
            if (nativeBridge && typeof nativeBridge.loadSlot === "function") {
                var slot = mapMvSlot(savefileId);
                var raw = nativeBridge.loadSlot("mv", slot);
                if (raw !== null && raw !== undefined) {
                    var lz = getLZString();
                    return (lz && typeof lz.decompressFromBase64 === "function") ? lz.decompressFromBase64(raw) : raw;
                }
            }
            return null;
        };

        sm.webStorageExists = function(savefileId) {
            var nativeBridge = getNativeBridge();
            if (nativeBridge && typeof nativeBridge.slotExists === "function") {
                var slot = mapMvSlot(savefileId);
                return nativeBridge.slotExists("mv", slot);
            }
            return false;
        };

        sm.removeWebStorage = function(savefileId) {
            var nativeBridge = getNativeBridge();
            if (nativeBridge && typeof nativeBridge.deleteSlot === "function") {
                var slot = mapMvSlot(savefileId);
                return nativeBridge.deleteSlot("mv", slot);
            }
            return false;
        };

        sm.backup = function(savefileId) {
            var nativeBridge = getNativeBridge();
            if (nativeBridge && typeof nativeBridge.loadSlot === "function" && typeof nativeBridge.saveSlot === "function") {
                var slot = mapMvSlot(savefileId);
                var data = nativeBridge.loadSlot("mv", slot);
                if (data !== null && data !== undefined) {
                    return nativeBridge.saveSlot("mv", slot + ".bak", data);
                }
            }
            return false;
        };

        sm.backupExists = function(savefileId) {
            var nativeBridge = getNativeBridge();
            if (nativeBridge && typeof nativeBridge.slotExists === "function") {
                var slot = mapMvSlot(savefileId);
                return nativeBridge.slotExists("mv", slot + ".bak");
            }
            return false;
        };
        sm.webStorageBackupExists = sm.backupExists;

        sm.cleanBackup = function(savefileId) {
            var nativeBridge = getNativeBridge();
            if (nativeBridge && typeof nativeBridge.deleteSlot === "function") {
                var slot = mapMvSlot(savefileId);
                return nativeBridge.deleteSlot("mv", slot + ".bak");
            }
            return false;
        };

        sm.restoreBackup = function(savefileId) {
            var nativeBridge = getNativeBridge();
            if (nativeBridge && typeof nativeBridge.loadSlot === "function" && typeof nativeBridge.saveSlot === "function") {
                var slot = mapMvSlot(savefileId);
                var data = nativeBridge.loadSlot("mv", slot + ".bak");
                if (data !== null && data !== undefined) {
                    var ok = nativeBridge.saveSlot("mv", slot, data);
                    if (ok) {
                        sm.cleanBackup(savefileId);
                    }
                    return ok;
                }
            }
            return false;
        };

        sm.loadFromWebStorageBackup = function(savefileId) {
            var nativeBridge = getNativeBridge();
            if (nativeBridge && typeof nativeBridge.loadSlot === "function") {
                var slot = mapMvSlot(savefileId);
                var raw = nativeBridge.loadSlot("mv", slot + ".bak");
                if (raw !== null && raw !== undefined) {
                    var lz = getLZString();
                    return (lz && typeof lz.decompressFromBase64 === "function") ? lz.decompressFromBase64(raw) : raw;
                }
            }
            return null;
        };
    }

    // [CAP-WEB-015] In-game cheat, state dump, and diagnostic command bridge
    function executeCheatCmd(cmd) {
        if (!cmd || typeof cmd !== "object") return;
        var action = cmd.action || cmd.type;
        var rpw = root.RuntimePackWeb || (typeof window !== "undefined" ? window.RuntimePackWeb : null);

        switch (action) {
            case "set_variable":
                if (root.$gameVariables && typeof root.$gameVariables.setValue === "function") {
                    root.$gameVariables.setValue(cmd.id, cmd.value);
                }
                break;
            case "set_switch":
                if (root.$gameSwitches && typeof root.$gameSwitches.setValue === "function") {
                    root.$gameSwitches.setValue(cmd.id, Boolean(cmd.value));
                }
                break;
            case "gain_gold":
                if (root.$gameParty && typeof root.$gameParty.gainGold === "function") {
                    root.$gameParty.gainGold(cmd.amount);
                }
                break;
            case "recover_all":
                if (root.$gameParty) {
                    var members = null;
                    if (typeof root.$gameParty.members === "function") {
                        members = root.$gameParty.members();
                    } else if (Array.isArray(root.$gameParty.members)) {
                        members = root.$gameParty.members;
                    }
                    if (Array.isArray(members)) {
                        members.forEach(function(m) {
                            if (m && typeof m.recoverAll === "function") {
                                m.recoverAll();
                            }
                        });
                    }
                }
                break;
            case "gain_item":
                if (root.$gameParty && typeof root.$gameParty.gainItem === "function") {
                    var dataItems = root.$dataItems || (typeof window !== "undefined" ? window.$dataItems : null);
                    var dataWeapons = root.$dataWeapons || (typeof window !== "undefined" ? window.$dataWeapons : null);
                    var dataArmors = root.$dataArmors || (typeof window !== "undefined" ? window.$dataArmors : null);
                    var item = null;
                    if (cmd.type === "weapon" && dataWeapons) {
                        item = dataWeapons[cmd.id];
                    } else if (cmd.type === "armor" && dataArmors) {
                        item = dataArmors[cmd.id];
                    } else if (cmd.type === "item" && dataItems) {
                        item = dataItems[cmd.id];
                    }
                    if (!item) {
                        item = (dataItems && dataItems[cmd.id]) ||
                               (dataWeapons && dataWeapons[cmd.id]) ||
                               (dataArmors && dataArmors[cmd.id]) || null;
                    }
                    if (item) {
                        var amount = cmd.amount !== undefined ? cmd.amount : 1;
                        root.$gameParty.gainItem(item, amount);
                    }
                }
                break;
            case "eval":
                try {
                    var res = (0, eval)(cmd.code);
                    var resStr = res !== undefined ? String(res) : "";
                    if (rpw && typeof rpw.setEvalResult === "function") {
                        rpw.setEvalResult(resStr);
                    }
                } catch (e) {
                    var errStr = "[CHT-002] " + (e && e.message ? e.message : String(e));
                    if (rpw && typeof rpw.setEvalResult === "function") {
                        rpw.setEvalResult(errStr);
                    }
                }
                break;
            case "dump_state":
                var gold = (root.$gameParty && typeof root.$gameParty.gold === "function")
                    ? root.$gameParty.gold()
                    : 0;
                var varsData = (root.$gameVariables && root.$gameVariables._data !== undefined)
                    ? root.$gameVariables._data
                    : {};
                var switchesData = (root.$gameSwitches && root.$gameSwitches._data !== undefined)
                    ? root.$gameSwitches._data
                    : {};
                var payload = {
                    gold: gold,
                    variables: varsData,
                    switches: switchesData
                };
                if (rpw && typeof rpw.setCheatState === "function") {
                    rpw.setCheatState(JSON.stringify(payload));
                }
                break;
            case "teleport_map":
                var tMapId = Number(cmd.map_id !== undefined ? cmd.map_id : (cmd.mapId !== undefined ? cmd.mapId : 1));
                var tX = Number(cmd.x !== undefined ? cmd.x : 0);
                var tY = Number(cmd.y !== undefined ? cmd.y : 0);
                if (root.$gamePlayer && typeof root.$gamePlayer.reserveTransfer === "function") {
                    root.$gamePlayer.reserveTransfer(tMapId, tX, tY, 2, 0);
                }
                if (root.SceneManager && root.Scene_Map && typeof root.SceneManager.goto === "function") {
                    if (!root.SceneManager._scene || !(root.SceneManager._scene instanceof root.Scene_Map)) {
                        root.SceneManager.goto(root.Scene_Map);
                    }
                }
                var tpMsg = "Teleported to Map " + tMapId + " (" + tX + ", " + tY + ")";
                if (rpw && typeof rpw.setEvalResult === "function") {
                    rpw.setEvalResult(tpMsg);
                }
                break;
            case "export_bug_report":
                var sName = "Unknown";
                if (root.SceneManager && root.SceneManager._scene && root.SceneManager._scene.constructor) {
                    sName = root.SceneManager._scene.constructor.name || "Scene_Base";
                }
                var mId = (root.$gameMap && typeof root.$gameMap.mapId === "function") ? root.$gameMap.mapId() : null;
                var pX = null;
                var pY = null;
                if (root.$gamePlayer) {
                    if (typeof root.$gamePlayer.x === "function") {
                        pX = root.$gamePlayer.x();
                    } else if (root.$gamePlayer.x !== undefined) {
                        pX = root.$gamePlayer.x;
                    } else if (root.$gamePlayer._x !== undefined) {
                        pX = root.$gamePlayer._x;
                    }
                    if (typeof root.$gamePlayer.y === "function") {
                        pY = root.$gamePlayer.y();
                    } else if (root.$gamePlayer.y !== undefined) {
                        pY = root.$gamePlayer.y;
                    } else if (root.$gamePlayer._y !== undefined) {
                        pY = root.$gamePlayer._y;
                    }
                }
                var pGold = (root.$gameParty && typeof root.$gameParty.gold === "function") ? root.$gameParty.gold() : 0;
                var bugReport = {
                    timestamp: new Date().toISOString(),
                    scene: sName,
                    map_id: mId,
                    player: { x: pX, y: pY },
                    gold: pGold,
                    engine: (root.Utils && root.Utils.RPGMAKER_NAME) ? root.Utils.RPGMAKER_NAME : "Web Engine"
                };
                var bugJson = JSON.stringify(bugReport, null, 2);
                root.__runtimepack_bug_report = bugReport;
                try {
                    if (typeof root.localStorage !== "undefined") {
                        root.localStorage.setItem("runtimepack_bug_report", bugJson);
                    }
                } catch (e) {}
                if (rpw && typeof rpw.setEvalResult === "function") {
                    rpw.setEvalResult(bugJson);
                }
                break;
            case "run_stress_test":
                var auditReport = {
                    timestamp: new Date().toISOString(),
                    engine: (root.Utils && root.Utils.RPGMAKER_NAME) ? root.Utils.RPGMAKER_NAME : "Web Engine",
                    stats: {
                        total_maps_scanned: 0,
                        total_assets_checked: 0,
                        exact_count: 0,
                        case_mismatch_count: 0,
                        missing_count: 0,
                        broken_transfer_count: 0
                    },
                    broken_transfers: [],
                    case_mismatches: [],
                    missing_assets: [],
                    scanned_maps: []
                };

                var checkedAssets = {};
                var checkAsset = function(assetPath, exts, type) {
                    if (!assetPath) return;
                    if (checkedAssets[assetPath]) return;
                    auditReport.stats.total_assets_checked++;
                    var found = false;
                    var testExts = exts || [""];
                    for (var i = 0; i < testExts.length; i++) {
                        var candidate = assetPath + testExts[i];
                        if (syncExists(candidate)) {
                            found = true;
                            checkedAssets[assetPath] = "EXACT";
                            auditReport.stats.exact_count++;
                            break;
                        }
                    }
                    if (!found) {
                        checkedAssets[assetPath] = "MISSING";
                        auditReport.stats.missing_count++;
                        auditReport.missing_assets.push({ referenced: assetPath, type: type || "asset" });
                    }
                };

                if (root.$dataMap && root.$dataMap.events) {
                    auditReport.stats.total_maps_scanned++;
                    var events = root.$dataMap.events;
                    var mapId = (root.$gameMap && typeof root.$gameMap.mapId === "function") ? root.$gameMap.mapId() : 1;
                    auditReport.scanned_maps.push({ map_id: mapId });
                    for (var eKey in events) {
                        var ev = events[eKey];
                        if (!ev || !ev.pages) continue;
                        for (var p = 0; p < ev.pages.length; p++) {
                            var list = ev.pages[p].list || [];
                            for (var c = 0; c < list.length; c++) {
                                var cmdItem = list[c];
                                if (!cmdItem) continue;
                                if (cmdItem.code === 201) { // Transfer
                                    var targetMap = cmdItem.parameters[1];
                                    if (targetMap && !syncExists("data/Map" + String(targetMap).padStart(3, "0") + ".json")) {
                                        auditReport.broken_transfers.push({ from_map: mapId, target_map_id: targetMap });
                                        auditReport.stats.broken_transfer_count++;
                                    }
                                } else if (cmdItem.code === 231) { // Picture
                                    var pic = cmdItem.parameters[1];
                                    if (pic) checkAsset("img/pictures/" + pic, [".png", ".rpgmvp", ".png_"], "Picture");
                                } else if (cmdItem.code === 241) { // BGM
                                    var bgm = cmdItem.parameters[0];
                                    var bgmName = (bgm && bgm.name) ? bgm.name : (typeof bgm === "string" ? bgm : null);
                                    if (bgmName) checkAsset("audio/bgm/" + bgmName, [".ogg", ".m4a", ".rpgmvo"], "BGM");
                                } else if (cmdItem.code === 245) { // BGS
                                    var bgs = cmdItem.parameters[0];
                                    var bgsName = (bgs && bgs.name) ? bgs.name : (typeof bgs === "string" ? bgs : null);
                                    if (bgsName) checkAsset("audio/bgs/" + bgsName, [".ogg", ".m4a", ".rpgmvo"], "BGS");
                                } else if (cmdItem.code === 250) { // SE
                                    var se = cmdItem.parameters[0];
                                    var seName = (se && se.name) ? se.name : (typeof se === "string" ? se : null);
                                    if (seName) checkAsset("audio/se/" + seName, [".ogg", ".m4a", ".rpgmvo"], "SE");
                                } else if (cmdItem.code === 322) { // Change Actor Graphic
                                    var charName = cmdItem.parameters[1];
                                    var faceName = cmdItem.parameters[3];
                                    if (charName) checkAsset("img/characters/" + charName, [".png", ".rpgmvp"], "Character");
                                    if (faceName) checkAsset("img/faces/" + faceName, [".png", ".rpgmvp"], "Face");
                                }
                            }
                        }
                    }
                }

                if (root.$dataActors && Array.isArray(root.$dataActors)) {
                    for (var a = 0; a < root.$dataActors.length; a++) {
                        var act = root.$dataActors[a];
                        if (!act) continue;
                        if (act.characterName) checkAsset("img/characters/" + act.characterName, [".png", ".rpgmvp"], "Character");
                        if (act.faceName) checkAsset("img/faces/" + act.faceName, [".png", ".rpgmvp"], "Face");
                        if (act.battlerName) checkAsset("img/sv_actors/" + act.battlerName, [".png", ".rpgmvp"], "Battler");
                    }
                }
                if (root.$dataEnemies && Array.isArray(root.$dataEnemies)) {
                    for (var en = 0; en < root.$dataEnemies.length; en++) {
                        var enemy = root.$dataEnemies[en];
                        if (!enemy) continue;
                        if (enemy.battlerName) checkAsset("img/enemies/" + enemy.battlerName, [".png", ".rpgmvp"], "Battler");
                    }
                }

                var auditJson = JSON.stringify(auditReport, null, 2);
                root.__runtimepack_audit_report = auditReport;
                try {
                    if (typeof root.localStorage !== "undefined") {
                        root.localStorage.setItem("runtimepack_audit_report", auditJson);
                    }
                } catch (e) {}
                var auditSummary = "Stress test completed: " + auditReport.stats.total_maps_scanned + " maps, " + auditReport.stats.missing_count + " missing assets.";
                if (rpw && typeof rpw.setEvalResult === "function") {
                    rpw.setEvalResult(auditSummary);
                }
                break;
        }
    }

    if (root.__runtimepack_shims_installed) {
        if (typeof root.StorageManager !== "undefined") {
            patchStorageManager(root.StorageManager);
        }
        if (typeof root.Graphics !== "undefined") {
            patchGraphics(root.Graphics);
        }
        root.executeCheatCmd = executeCheatCmd;
        if (typeof root.window !== "undefined") {
            root.window.executeCheatCmd = executeCheatCmd;
        }
        return;
    }
    root.__runtimepack_shims_installed = true;

    // [CAP-WEB-009] Desktop shell, Node.js built-ins, and Steamworks mocks
    function normalizePath(p) {
        if (typeof p !== "string") return "";
        var isAbs = p.charCodeAt(0) === 47;
        var segments = p.split("/");
        var res = [];
        for (var i = 0; i < segments.length; i++) {
            var seg = segments[i];
            if (!seg || seg === ".") continue;
            if (seg === "..") {
                if (res.length > 0 && res[res.length - 1] !== "..") {
                    res.pop();
                } else if (!isAbs) {
                    res.push("..");
                }
            } else {
                res.push(seg);
            }
        }
        var joined = res.join("/");
        if (isAbs) {
            return "/" + joined;
        }
        return joined.length > 0 ? joined : (isAbs ? "/" : ".");
    }

    var pathShim = {
        sep: "/",
        delimiter: ":",
        join: function() {
            var paths = [];
            for (var i = 0; i < arguments.length; i++) {
                var arg = arguments[i];
                if (typeof arg === "string" && arg.length > 0) {
                    paths.push(arg);
                }
            }
            if (paths.length === 0) return ".";
            var raw = paths.join("/").replace(/\/+/g, "/");
            return normalizePath(raw);
        },
        resolve: function() {
            var resolvedPath = "";
            var resolvedAbsolute = false;
            for (var i = arguments.length - 1; i >= -1 && !resolvedAbsolute; i--) {
                var p = (i >= 0) ? arguments[i] : "/";
                if (typeof p !== "string" || p.length === 0) continue;
                resolvedPath = p + "/" + resolvedPath;
                resolvedAbsolute = p.charCodeAt(0) === 47;
            }
            return normalizePath(resolvedPath);
        },
        normalize: function(p) {
            return normalizePath(p);
        },
        dirname: function(p) {
            if (typeof p !== "string" || p.length === 0) return ".";
            var norm = p.replace(/\/+$/, "");
            var idx = norm.lastIndexOf("/");
            if (idx === -1) return ".";
            if (idx === 0) return "/";
            return norm.substring(0, idx);
        },
        basename: function(p, ext) {
            if (typeof p !== "string" || p.length === 0) return "";
            var norm = p.replace(/\/+$/, "");
            var idx = norm.lastIndexOf("/");
            var base = idx >= 0 ? norm.substring(idx + 1) : norm;
            if (ext && typeof ext === "string" && base.endsWith(ext)) {
                base = base.substring(0, base.length - ext.length);
            }
            return base;
        },
        extname: function(p) {
            if (typeof p !== "string" || p.length === 0) return "";
            var base = p.split("/").pop().replace(/\/+$/, "");
            var idx = base.lastIndexOf(".");
            if (idx <= 0) return "";
            return base.substring(idx);
        }
    };

    var windowListeners = {};
    var windowState = {
        isFullscreen: false,
        isFullScreen: false,
        resizable: true
    };
    var nwWindowInstance = {
        get isFullscreen() {
            return windowState.isFullscreen;
        },
        set isFullscreen(v) {
            windowState.isFullscreen = !!v;
            windowState.isFullScreen = !!v;
        },
        get isFullScreen() {
            return windowState.isFullscreen;
        },
        set isFullScreen(v) {
            windowState.isFullscreen = !!v;
            windowState.isFullScreen = !!v;
        },
        setResizable: function(r) {
            windowState.resizable = !!r;
        },
        enterFullscreen: function() {
            windowState.isFullscreen = true;
            windowState.isFullScreen = true;
        },
        leaveFullscreen: function() {
            windowState.isFullscreen = false;
            windowState.isFullScreen = false;
        },
        on: function(event, handler) {
            if (!event || typeof handler !== "function") return;
            if (!windowListeners[event]) windowListeners[event] = [];
            windowListeners[event].push(handler);
        },
        removeListener: function(event, handler) {
            if (!event || !windowListeners[event]) return;
            windowListeners[event] = windowListeners[event].filter(function(h) { return h !== handler; });
        },
        removeAllListeners: function(event) {
            if (event) {
                delete windowListeners[event];
            } else {
                windowListeners = {};
            }
        },
        emit: function(event) {
            var args = Array.prototype.slice.call(arguments, 1);
            if (windowListeners[event]) {
                windowListeners[event].slice().forEach(function(h) {
                    try {
                        h.apply(nwWindowInstance, args);
                    } catch (e) {
                        console.error("[RuntimePack nw.Window] listener error:", e);
                    }
                });
            }
        },
        showDevTools: function() {},
        show: function() {},
        hide: function() {},
        focus: function() {},
        blur: function() {}
    };

    var nwShim = {
        Window: {
            get: function() {
                return nwWindowInstance;
            }
        },
        App: {
            argv: [],
            dataPath: "",
            manifest: {},
            quit: function() {},
            closeAllWindows: function() {}
        }
    };

    if (typeof root.process === "undefined") {
        root.process = {
            mainModule: {
                filename: "index.html"
            },
            cwd: function() { return ""; },
            platform: "android",
            env: { HOME: "" },
            execPath: "",
            __dirname: ""
        };
    } else {
        root.process.execPath = root.process.execPath || "";
        root.process.__dirname = root.process.__dirname || "";
        root.process.env = root.process.env || { HOME: "" };
        if (typeof root.process.env.HOME === "undefined") {
            root.process.env.HOME = "";
        }
    }
    if (typeof root.window !== "undefined" && typeof root.window.process === "undefined") {
        root.window.process = root.process;
    }

    if (typeof root.Buffer === "undefined") {
        function Buffer(arg, byteOffset, length) {
            if (!(this instanceof Buffer)) {
                return new Buffer(arg, byteOffset, length);
            }
            if (typeof arg === "number") {
                var size = Math.max(0, arg | 0);
                var u8 = new Uint8Array(size);
                Object.setPrototypeOf(u8, Buffer.prototype);
                return u8;
            }
            return Buffer.from(arg, byteOffset, length);
        }

        Object.setPrototypeOf(Buffer.prototype, Uint8Array.prototype);
        Object.setPrototypeOf(Buffer, Uint8Array);

        Buffer.prototype._isBuffer = true;

        Buffer.isBuffer = function(b) {
            return !!(b && (b instanceof Buffer || b._isBuffer === true));
        };

        var normalizeEncoding = function(enc) {
            var e = String(enc || "utf8").toLowerCase().replace(/[^a-z0-9]/g, "");
            if (e === "utf8") return "utf8";
            if (e === "base64") return "base64";
            if (e === "hex") return "hex";
            if (e === "binary" || e === "latin1") return "binary";
            if (e === "ascii") return "ascii";
            throw new TypeError("Unknown encoding: " + enc);
        };

        var utf8Encode = function(str) {
            if (typeof TextEncoder !== "undefined") {
                return new TextEncoder().encode(str);
            }
            var utf8 = [];
            for (var i = 0; i < str.length; i++) {
                var charcode = str.charCodeAt(i);
                if (charcode < 0x80) utf8.push(charcode);
                else if (charcode < 0x800) {
                    utf8.push(0xc0 | (charcode >> 6),
                              0x80 | (charcode & 0x3f));
                } else if (charcode < 0xd800 || charcode >= 0xe000) {
                    utf8.push(0xe0 | (charcode >> 12),
                              0x80 | ((charcode >> 6) & 0x3f),
                              0x80 | (charcode & 0x3f));
                } else {
                    i++;
                    charcode = 0x10000 + (((charcode & 0x3ff) << 10) | (str.charCodeAt(i) & 0x3ff));
                    utf8.push(0xf0 | (charcode >> 18),
                              0x80 | ((charcode >> 12) & 0x3f),
                              0x80 | ((charcode >> 6) & 0x3f),
                              0x80 | (charcode & 0x3f));
                }
            }
            return new Uint8Array(utf8);
        };

        var utf8Decode = function(bytes, start, end) {
            var s = start || 0;
            var e = (end === undefined) ? bytes.length : end;
            var sub = bytes.subarray(s, e);
            if (typeof TextDecoder !== "undefined") {
                return new TextDecoder("utf-8").decode(sub);
            }
            var out = "";
            var i = 0;
            while (i < sub.length) {
                var c = sub[i++];
                if (c < 0x80) {
                    out += String.fromCharCode(c);
                } else if (c > 0xbf && c < 0xe0) {
                    var c2 = sub[i++];
                    out += String.fromCharCode(((c & 0x1f) << 6) | (c2 & 0x3f));
                } else if (c >= 0xe0 && c < 0xf0) {
                    var c2 = sub[i++];
                    var c3 = sub[i++];
                    out += String.fromCharCode(((c & 0x0f) << 12) | ((c2 & 0x3f) << 6) | (c3 & 0x3f));
                } else {
                    var c2 = sub[i++];
                    var c3 = sub[i++];
                    var c4 = sub[i++];
                    var codepoint = (((c & 0x07) << 18) | ((c2 & 0x3f) << 12) | ((c3 & 0x3f) << 6) | (c4 & 0x3f)) - 0x10000;
                    out += String.fromCharCode(0xd800 + (codepoint >> 10), 0xdc00 + (codepoint & 0x3ff));
                }
            }
            return out;
        };

        var base64Chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        var base64Lookup = new Uint8Array(256);
        for (var bIdx = 0; bIdx < base64Chars.length; bIdx++) {
            base64Lookup[base64Chars.charCodeAt(bIdx)] = bIdx;
        }

        var base64Encode = function(bytes, start, end) {
            var s = start || 0;
            var e = (end === undefined) ? bytes.length : end;
            var sub = bytes.subarray(s, e);
            var btoaFn = (typeof btoa === "function") ? btoa : ((typeof root.btoa === "function") ? root.btoa : null);
            if (btoaFn) {
                var bin = "";
                for (var i = 0; i < sub.length; i++) {
                    bin += String.fromCharCode(sub[i]);
                }
                return btoaFn(bin);
            }
            var len = sub.length;
            var extraBytes = len % 3;
            var output = "";
            var i = 0;
            var temp, length = len - extraBytes;
            for (i = 0; i < length; i += 3) {
                temp = (sub[i] << 16) + (sub[i + 1] << 8) + sub[i + 2];
                output += (base64Chars[(temp >> 18) & 63] +
                           base64Chars[(temp >> 12) & 63] +
                           base64Chars[(temp >> 6) & 63] +
                           base64Chars[temp & 63]);
            }
            if (extraBytes === 1) {
                temp = sub[len - 1];
                output += (base64Chars[temp >> 2] +
                           base64Chars[(temp << 4) & 63] +
                           "==");
            } else if (extraBytes === 2) {
                temp = (sub[len - 2] << 8) + sub[len - 1];
                output += (base64Chars[temp >> 10] +
                           base64Chars[(temp >> 4) & 63] +
                           base64Chars[(temp << 2) & 63] +
                           "=");
            }
            return output;
        };

        var base64Decode = function(str) {
            var atobFn = (typeof atob === "function") ? atob : ((typeof root.atob === "function") ? root.atob : null);
            if (atobFn) {
                var bin = atobFn(str);
                var bytes = new Uint8Array(bin.length);
                for (var i = 0; i < bin.length; i++) {
                    bytes[i] = bin.charCodeAt(i);
                }
                return bytes;
            }
            var clean = String(str || "").trim();
            var len = clean.length;
            var placeHolders = (clean.charAt(len - 1) === "=") ? ((clean.charAt(len - 2) === "=") ? 2 : 1) : 0;
            var bytes = new Uint8Array((len * 3 / 4) - placeHolders);
            var l = (placeHolders > 0) ? len - 4 : len;
            var cur = 0;
            for (var i = 0; i < l; i += 4) {
                var c1 = base64Lookup[clean.charCodeAt(i)];
                var c2 = base64Lookup[clean.charCodeAt(i + 1)];
                var c3 = base64Lookup[clean.charCodeAt(i + 2)];
                var c4 = base64Lookup[clean.charCodeAt(i + 3)];
                bytes[cur++] = (c1 << 2) | (c2 >> 4);
                bytes[cur++] = ((c2 & 15) << 4) | (c3 >> 2);
                bytes[cur++] = ((c3 & 3) << 6) | c4;
            }
            if (placeHolders === 1) {
                var c1 = base64Lookup[clean.charCodeAt(i)];
                var c2 = base64Lookup[clean.charCodeAt(i + 1)];
                var c3 = base64Lookup[clean.charCodeAt(i + 2)];
                bytes[cur++] = (c1 << 2) | (c2 >> 4);
                bytes[cur++] = ((c2 & 15) << 4) | (c3 >> 2);
            } else if (placeHolders === 2) {
                var c1 = base64Lookup[clean.charCodeAt(i)];
                var c2 = base64Lookup[clean.charCodeAt(i + 1)];
                bytes[cur++] = (c1 << 2) | (c2 >> 4);
            }
            return bytes;
        };

        var hexEncode = function(bytes, start, end) {
            var s = start || 0;
            var e = (end === undefined) ? bytes.length : end;
            var out = "";
            for (var i = s; i < e; i++) {
                var hex = bytes[i].toString(16);
                out += (hex.length === 1 ? "0" + hex : hex);
            }
            return out;
        };

        var hexDecode = function(str) {
            var clean = String(str || "").replace(/[^0-9a-fA-F]/g, "");
            var len = clean.length & ~1;
            var bytes = new Uint8Array(len / 2);
            for (var i = 0; i < len; i += 2) {
                bytes[i / 2] = parseInt(clean.substring(i, i + 2), 16);
            }
            return bytes;
        };

        Buffer.from = function(data, byteOffset, length) {
            if (typeof data === "string") {
                var encType = normalizeEncoding(byteOffset);
                var raw;
                if (encType === "utf8") raw = utf8Encode(data);
                else if (encType === "base64") raw = base64Decode(data);
                else if (encType === "hex") raw = hexDecode(data);
                else if (encType === "binary" || encType === "ascii") {
                    raw = new Uint8Array(data.length);
                    for (var i = 0; i < data.length; i++) {
                        raw[i] = data.charCodeAt(i) & 0xff;
                    }
                }
                var b = new Uint8Array(raw.buffer, raw.byteOffset, raw.byteLength);
                Object.setPrototypeOf(b, Buffer.prototype);
                return b;
            }
            if (data instanceof ArrayBuffer) {
                var offset = (byteOffset === undefined) ? 0 : (byteOffset | 0);
                var len = (length === undefined) ? (data.byteLength - offset) : (length | 0);
                var b = new Uint8Array(data, offset, len);
                Object.setPrototypeOf(b, Buffer.prototype);
                return b;
            }
            if (ArrayBuffer.isView(data) || (data && typeof data.byteLength === "number" && data.buffer)) {
                var copy = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
                var b = new Uint8Array(copy);
                Object.setPrototypeOf(b, Buffer.prototype);
                return b;
            }
            if (Array.isArray(data)) {
                var b = new Uint8Array(data);
                Object.setPrototypeOf(b, Buffer.prototype);
                return b;
            }
            if (typeof data === "number") {
                throw new TypeError("The first argument must not be a number");
            }
            var b = new Uint8Array(0);
            Object.setPrototypeOf(b, Buffer.prototype);
            return b;
        };

        Buffer.alloc = function(size, fill, enc) {
            var b = new Buffer(size);
            if (fill !== undefined) {
                b.fill(fill, enc);
            }
            return b;
        };

        Buffer.concat = function(list, totalLength) {
            if (!Array.isArray(list)) {
                throw new TypeError("list argument must be an Array");
            }
            var len = 0;
            if (totalLength === undefined) {
                for (var i = 0; i < list.length; i++) {
                    len += (list[i] ? list[i].length : 0);
                }
            } else {
                len = totalLength | 0;
            }
            var result = Buffer.alloc(len);
            var pos = 0;
            for (var i = 0; i < list.length; i++) {
                var item = list[i];
                if (!item) continue;
                var copyLen = Math.min(item.length, len - pos);
                if (copyLen <= 0) break;
                result.set(item.subarray(0, copyLen), pos);
                pos += copyLen;
            }
            return result;
        };

        Buffer.prototype.toString = function(enc, start, end) {
            var encType = normalizeEncoding(enc);
            if (encType === "utf8") return utf8Decode(this, start, end);
            if (encType === "base64") return base64Encode(this, start, end);
            if (encType === "hex") return hexEncode(this, start, end);
            var s = start || 0;
            var e = (end === undefined) ? this.length : end;
            var out = "";
            for (var i = s; i < e; i++) {
                out += String.fromCharCode(this[i]);
            }
            return out;
        };

        Buffer.prototype.slice = function(start, end) {
            var len = this.length;
            var s = (start === undefined) ? 0 : (start < 0 ? Math.max(0, len + start) : Math.min(len, start));
            var e = (end === undefined) ? len : (end < 0 ? Math.max(0, len + end) : Math.min(len, end));
            if (e < s) e = s;
            var b = new Uint8Array(this.buffer, this.byteOffset + s, e - s);
            Object.setPrototypeOf(b, Buffer.prototype);
            return b;
        };

        Buffer.prototype.fill = function(val, enc) {
            var fillByte = 0;
            if (typeof val === "number") {
                fillByte = val & 0xff;
            } else if (typeof val === "string") {
                fillByte = val.charCodeAt(0) & 0xff;
            }
            Uint8Array.prototype.fill.call(this, fillByte);
            return this;
        };

        root.Buffer = Buffer;
        if (typeof root.window !== "undefined") {
            root.window.Buffer = Buffer;
        }
    }

    var cryptoShim = {
        createCipheriv: function() {
            return {
                update: function() { return (root.Buffer && root.Buffer.alloc) ? root.Buffer.alloc(0) : []; },
                final: function() { return (root.Buffer && root.Buffer.alloc) ? root.Buffer.alloc(0) : []; }
            };
        },
        createDecipheriv: function() {
            return {
                update: function() { return (root.Buffer && root.Buffer.alloc) ? root.Buffer.alloc(0) : []; },
                final: function() { return (root.Buffer && root.Buffer.alloc) ? root.Buffer.alloc(0) : []; }
            };
        }
    };

    function syncFetch(relPath) {
        if (!relPath) return "";
        if (typeof XMLHttpRequest === "undefined") return "";
        try {
            var cleanPath = String(relPath).replace(/\\/g, '/');
            cleanPath = cleanPath.replace(/^https?:\/\/[^\/]+\//, '');
            cleanPath = cleanPath.replace(/^\/+/, '');
            while (cleanPath.startsWith("./")) {
                cleanPath = cleanPath.substring(2);
            }
            var url = "https://game.local/" + cleanPath;
            var xhr = new XMLHttpRequest();
            xhr.open("GET", url, false);
            xhr.send(null);
            if (xhr.status === 200 || (xhr.status === 0 && xhr.responseText && xhr.responseText.length > 0)) {
                return xhr.responseText;
            }
        } catch (e) {
            if (root.console && typeof root.console.warn === "function") {
                root.console.warn("[RuntimePack fsShim] syncFetch error for " + relPath, e);
            }
        }
        return "";
    }

    function syncExists(p) {
        if (!p) return false;
        var cleanPath = String(p).replace(/\\/g, "/").replace(/^\.?\//, "");
        if (cleanPath.endsWith("gameenv.iz")) {
            return false;
        }
        var nativeBridge = (typeof root.window !== "undefined" && root.window.RuntimePackNative) || root.RuntimePackNative || (typeof window !== "undefined" && window.RuntimePackNative);
        if (nativeBridge && typeof nativeBridge.fileExists === "function") {
            try {
                return nativeBridge.fileExists(cleanPath);
            } catch (e) {}
        }
        if (typeof XMLHttpRequest === "undefined") return false;
        try {
            var xhrPath = String(p).replace(/\\/g, '/');
            xhrPath = xhrPath.replace(/^https?:\/\/[^\/]+\//, '');
            xhrPath = xhrPath.replace(/^\/+/, '');
            while (xhrPath.startsWith("./")) {
                xhrPath = xhrPath.substring(2);
            }
            if (xhrPath.endsWith("gameenv.iz")) {
                return false;
            }
            var url = "https://game.local/" + xhrPath;
            var xhr = new XMLHttpRequest();
            xhr.open("GET", url, false);
            xhr.send(null);
            return (xhr.status === 200) || (xhr.status === 0 && xhr.responseText && xhr.responseText.length > 0);
        } catch (e) {
            return false;
        }
    }

    var fsShim = {
        existsSync: function(p) {
            return syncExists(p);
        },
        readFileSync: function(p, opt) {
            return syncFetch(p);
        },
        mkdirSync: function() {},
        writeFileSync: function() {},
        statSync: function(p) {
            var cleanPath = String(p || "").replace(/\\/g, "/").replace(/^\.?\//, "");
            var exists = syncExists(cleanPath);
            var size = 0;
            var nativeBridge = (typeof root.window !== "undefined" && root.window.RuntimePackNative) || root.RuntimePackNative || (typeof window !== "undefined" && window.RuntimePackNative);
            if (exists && nativeBridge && typeof nativeBridge.fileSize === "function") {
                try {
                    size = nativeBridge.fileSize(cleanPath);
                } catch (e) {}
            } else if (exists) {
                size = (syncFetch(p) || "").length;
            }
            return {
                isFile: function() { return exists; },
                isDirectory: function() { return false; },
                size: size
            };
        },
        // [CAP-WEB-001] fs.readdirSync native bridge and MapInfos fallback
        readdirSync: function(dirPath) {
            var clean = String(dirPath || "").replace(/\\/g, "/").replace(/^\.?\//, "").replace(/\/$/, "");
            var nativeBridge = (typeof root.window !== "undefined" && root.window.RuntimePackNative) || root.RuntimePackNative || (typeof window !== "undefined" && window.RuntimePackNative);
            if (nativeBridge && typeof nativeBridge.readdir === "function") {
                try {
                    var res = nativeBridge.readdir(clean);
                    var parsed = JSON.parse(res);
                    if (Array.isArray(parsed) && parsed.length > 0) {
                        return parsed;
                    }
                } catch (e) {}
            }
            if (clean === "data" || clean === "") {
                var mapInfosStr = syncFetch("data/MapInfos.json");
                if (mapInfosStr) {
                    var list = ["Actors.json", "Classes.json", "CommonEvents.json", "Enemies.json", "Items.json", "MapInfos.json", "Skills.json", "States.json", "System.json", "Tilesets.json", "Troops.json", "Weapons.json", "Armors.json", "Animations.json"];
                    try {
                        var infos = JSON.parse(mapInfosStr);
                        if (Array.isArray(infos)) {
                            infos.forEach(function(m) {
                                if (m && typeof m.id === "number") {
                                    list.push("Map" + String(m.id).padStart(3, "0") + ".json");
                                }
                            });
                        }
                    } catch (e) {}
                    return list;
                }
            }
            return [];
        },
        unlinkSync: function() {},
        readFile: function(file, opt, cb) {
            var callback = typeof opt === "function" ? opt : cb;
            var data = syncFetch(file);
            if (callback) callback(null, data);
        },
        writeFile: function(file, data, opt, cb) {
            var callback = typeof opt === "function" ? opt : cb;
            if (callback) callback(null);
        }
    };

    var greenworksShim = {
        init: function() { return true; },
        isSteamRunning: function() { return false; },
        getScreenLanguage: function() { return "en"; },
        activateAchievement: function() {},
        getNumberOfPlayers: function(cb) {
            if (typeof cb === "function") cb(1);
        }
    };

    var modules = {
        "path": pathShim,
        "nw.gui": nwShim,
        "nw": nwShim,
        "fs": fsShim,
        "crypto": cryptoShim,
        "greenworks": greenworksShim,
        "Greenworks": greenworksShim
    };

    root.require = function(mod) {
        if (Object.prototype.hasOwnProperty.call(modules, mod)) {
            return modules[mod];
        }
        if (typeof mod === "string" && (mod.indexOf("jquery") !== -1 || mod.indexOf("jQuery") !== -1)) {
            var jq = (typeof root.window !== "undefined" && (root.window.jQuery || root.window.$))
                || root.jQuery || root.$;
            if (jq) return jq;
        }
        if (root.console && typeof root.console.warn === "function") {
            root.console.warn("[RuntimePack] Unhandled require module:", mod);
        }
        return {};
    };

    // [CAP-WEB-014] TyranoBuilder engine compatibility hooks
    function setupTyranoHooks() {
        var w = (typeof root.window !== "undefined") ? root.window : root;
        var checkAndPatch = function() {
            var jq = (w && w.$) || (w && w.jQuery);
            if (jq) {
                if (jq.isNWJS && !jq.isNWJS.__rp_patched) {
                    var fnNw = function() { return false; };
                    fnNw.__rp_patched = true;
                    jq.isNWJS = fnNw;
                }
                if (jq.isElectron && !jq.isElectron.__rp_patched) {
                    var fnEl = function() { return false; };
                    fnEl.__rp_patched = true;
                    jq.isElectron = fnEl;
                }
                if (jq.isNeedClickAudio && !jq.isNeedClickAudio.__rp_patched) {
                    var fnNoClick = function() { return false; };
                    fnNoClick.__rp_patched = true;
                    jq.isNeedClickAudio = fnNoClick;
                }
                if (jq.setStorage && !jq.setStorage.__rp_patched) {
                    var origSet = jq.setStorage;
                    var patchedSet = function(key, val, type) {
                        if (type === "file") {
                            if (typeof jq.setStorageWeb === "function") {
                                return jq.setStorageWeb(key, val);
                            }
                        }
                        return origSet.apply(this, arguments);
                    };
                    patchedSet.__rp_patched = true;
                    jq.setStorage = patchedSet;
                }
                if (jq.getStorage && !jq.getStorage.__rp_patched) {
                    var origGet = jq.getStorage;
                    var patchedGet = function(key, type) {
                        if (type === "file") {
                            if (typeof jq.getStorageWeb === "function") {
                                return jq.getStorageWeb(key);
                            }
                        }
                        return origGet.apply(this, arguments);
                    };
                    patchedGet.__rp_patched = true;
                    jq.getStorage = patchedGet;
                }
            }
            var kag = (w && w.TYRANO && w.TYRANO.kag) || (root && root.TYRANO && root.TYRANO.kag);
            if (kag) {
                kag.tmp = kag.tmp || {};
                kag.tmp.ready_audio = true;

                if (kag.loadScenario && !kag.loadScenario.__rp_patched) {
                    var origLoadScenario = kag.loadScenario;
                    var patchedLoadScenario = function(file_name, call_back) {
                        var baseName = "";
                        if (typeof file_name === "string") {
                            baseName = file_name.split("/").pop().split("\\").pop().toLowerCase();
                        }
                        var isOptionalStub = (
                            baseName === "cg.ks" ||
                            baseName === "replay.ks" ||
                            baseName === "extra.ks" ||
                            baseName === "gallery.ks" ||
                            baseName === "memory.ks" ||
                            baseName === "cg_view.ks" ||
                            baseName === "scene.ks"
                        );

                        var cleanPath = file_name;
                        if (cleanPath && typeof cleanPath === "string" && !cleanPath.startsWith("data/")) {
                            cleanPath = "data/scenario/" + cleanPath;
                        }
                        var exists = syncExists(cleanPath) || syncExists("data/scenario/" + baseName) || syncExists(file_name);

                        if (!exists && (isOptionalStub || !syncExists(cleanPath))) {
                            if (root.console && typeof root.console.warn === "function") {
                                root.console.warn("[RuntimePack] Using safety stub for optional/missing scenario: " + file_name);
                            }
                            var stubText = "*start\n[cm]\n[clearfix]\n[back]\n[return]\n[jump storage=\"title.ks\"]\n";
                            var parsed = null;
                            if (kag.parser && typeof kag.parser.parseScenario === "function") {
                                parsed = kag.parser.parseScenario(stubText);
                            } else {
                                parsed = {
                                    array_s: [
                                        { pm: {}, val: "start", name: "*start" },
                                        { pm: {}, val: "", name: "cm" },
                                        { pm: {}, val: "", name: "clearfix" },
                                        { pm: {}, val: "", name: "back" },
                                        { pm: {}, val: {}, name: "return" },
                                        { pm: { storage: "title.ks" }, val: "", name: "jump" }
                                    ],
                                    map_label: { "start": { index: 0, label_name: "start" } }
                                };
                            }
                            kag.cache_scenario = kag.cache_scenario || {};
                            kag.cache_scenario[file_name] = parsed;
                            if (typeof call_back === "function") {
                                return call_back(parsed);
                            }
                            return;
                        }

                        try {
                            return origLoadScenario.call(this, file_name, function(parsed) {
                                if (typeof call_back === "function") {
                                    call_back(parsed);
                                }
                            });
                        } catch (err) {
                            if (root.console && typeof root.console.error === "function") {
                                root.console.error("[RuntimePack] Failed to load scenario: " + file_name, err);
                            }
                            var stubText = "*start\n[cm]\n[clearfix]\n[back]\n[return]\n[jump storage=\"title.ks\"]\n";
                            var fallbackParsed = (kag.parser && typeof kag.parser.parseScenario === "function")
                                ? kag.parser.parseScenario(stubText)
                                : {
                                    array_s: [
                                        { pm: {}, val: "start", name: "*start" },
                                        { pm: {}, val: "", name: "cm" },
                                        { pm: {}, val: "", name: "clearfix" },
                                        { pm: {}, val: "", name: "back" },
                                        { pm: {}, val: {}, name: "return" },
                                        { pm: { storage: "title.ks" }, val: "", name: "jump" }
                                    ],
                                    map_label: { "start": { index: 0, label_name: "start" } }
                                };
                            kag.cache_scenario = kag.cache_scenario || {};
                            kag.cache_scenario[file_name] = fallbackParsed;
                            if (typeof call_back === "function") {
                                return call_back(fallbackParsed);
                            }
                        }
                    };
                    patchedLoadScenario.__rp_patched = true;
                    kag.loadScenario = patchedLoadScenario;
                }

                if (kag.ftag && typeof kag.ftag.startTag === "function" && !kag.ftag.startTag.__rp_patched) {
                    var origStartTag = kag.ftag.startTag;
                    var patchedStartTag = function(tag_name, pm) {
                        if (tag_name === "jump" && pm && pm.target) {
                            var targetClean = (typeof pm.target === "string") ? pm.target.replace(/^\*/, "").trim().toLowerCase() : "";
                            var isPlaceholder = (
                                targetClean === "未定" ||
                                targetClean === "tbd" ||
                                targetClean === "todo" ||
                                targetClean === "coming_soon" ||
                                targetClean === "temp" ||
                                targetClean === "dummy" ||
                                targetClean === "dammy" ||
                                targetClean === "none"
                            );
                            var currentStorage = (kag.stat && kag.stat.current_scenario) || "";
                            var targetStorage = pm.storage || currentStorage;
                            var scenarioData = kag.cache_scenario && kag.cache_scenario[targetStorage];
                            var labelExists = false;
                            if (scenarioData && scenarioData.map_label) {
                                var rawKey = pm.target.replace(/^\*/, "");
                                labelExists = !!(scenarioData.map_label[rawKey] || scenarioData.map_label[pm.target]);
                            }
                            if (!labelExists && isPlaceholder) {
                                if (root.console && typeof root.console.warn === "function") {
                                    root.console.warn("[RuntimePack] Safe handling for missing placeholder jump target: " + pm.target);
                                }
                                if (typeof kag.ftag.nextOrder === "function") {
                                    return kag.ftag.nextOrder();
                                }
                                return;
                            }
                        }
                        return origStartTag.apply(this, arguments);
                    };
                    patchedStartTag.__rp_patched = true;
                    kag.ftag.startTag = patchedStartTag;
                }
            }
        };

        checkAndPatch();
        try {
            if (typeof window !== "undefined" && typeof window.setInterval === "function") {
                var timer = window.setInterval(checkAndPatch, 50);
                if (typeof window.setTimeout === "function" && typeof window.clearInterval === "function") {
                    window.setTimeout(function() { window.clearInterval(timer); }, 10000);
                }
            } else if (typeof setInterval === "function") {
                var timer = setInterval(checkAndPatch, 50);
                if (typeof setTimeout === "function" && typeof clearInterval === "function") {
                    setTimeout(function() { clearInterval(timer); }, 10000);
                }
            }
        } catch (_) {}
        if (typeof w.addEventListener === "function") {
            w.addEventListener("DOMContentLoaded", checkAndPatch);
            w.addEventListener("load", checkAndPatch);
        }
    }

    setupTyranoHooks();

    root.nw = nwShim;
    root.Greenworks = greenworksShim; // Preserve native direct eval semantics for Yanfly formulas.
    root.greenworks = greenworksShim;
    root.executeCheatCmd = executeCheatCmd;
    if (typeof root.window !== "undefined") {
        root.window.executeCheatCmd = executeCheatCmd;
    }

    if (typeof root.StorageManager !== "undefined") {
        patchStorageManager(root.StorageManager);
    }
    try {
        var _storageManager = root.StorageManager;
        Object.defineProperty(root, "StorageManager", {
            configurable: true,
            enumerable: true,
            get: function() {
                return _storageManager;
            },
            set: function(val) {
                _storageManager = val;
                patchStorageManager(_storageManager);
            }
        });
    } catch (e) {}

    if (typeof root.Graphics !== "undefined") {
        patchGraphics(root.Graphics);
    }
    try {
        var _graphics = root.Graphics;
        Object.defineProperty(root, "Graphics", {
            configurable: true,
            enumerable: true,
            get: function() {
                return _graphics;
            },
            set: function(val) {
                _graphics = val;
                patchGraphics(_graphics);
            }
        });
    } catch (e) {}

    // [CAP-WEB-011] AudioManager Web Audio format enforcement
    function patchAudioManager(am) {
        if (!am) return;
        if (!am.audioFileExt || !am.audioFileExt.__patched) {
            am.audioFileExt = function() {
                if (root.Utils && typeof root.Utils.canPlayOgg === "function" && root.Utils.canPlayOgg()) {
                    return '.ogg';
                }
                if (root.WebAudio && typeof root.WebAudio.canPlayOgg === "function" && root.WebAudio.canPlayOgg()) {
                    return '.ogg';
                }
                return '.ogg';
            };
            am.audioFileExt.__patched = true;
        }
        if (root.RuntimePackAssetAudit && typeof root.RuntimePackAssetAudit.installHooks === "function") {
            root.RuntimePackAssetAudit.installHooks(root);
        }
    }

    // [CAP-WEB-010] Graphics canvas clearing and viewport stretch fit
    function patchGraphics(g) {
        if (!g) return;
        if (g._clearUpperCanvas && !g._clearUpperCanvas.__patched) {
            var origClear = g._clearUpperCanvas;
            g._clearUpperCanvas = function() {
                if (!this._upperCanvas) return;
                return origClear.apply(this, arguments);
            };
            g._clearUpperCanvas.__patched = true;
        }

        var fullHeightFn = function() {
            return (typeof window !== "undefined" && window.innerHeight) ?
                window.innerHeight : (document.documentElement ? document.documentElement.clientHeight : 600);
        };
        fullHeightFn.__rp_fit_patched = true;

        if (typeof g._stretchHeight === "function") {
            if (!g._stretchHeight.__rp_fit_patched) {
                g._stretchHeight = fullHeightFn;
            }
        } else {
            try {
                Object.defineProperty(g, "_stretchHeight", {
                    configurable: true,
                    enumerable: true,
                    get: function() {
                        return fullHeightFn;
                    },
                    set: function(val) {
                        // Retain 100% viewport height
                    }
                });
            } catch (e) {}
        }

        var fullWidthFn = function() {
            return (typeof window !== "undefined" && window.innerWidth) ?
                window.innerWidth : (document.documentElement ? document.documentElement.clientWidth : 800);
        };
        fullWidthFn.__rp_fit_patched = true;

        if (typeof g._stretchWidth === "function") {
            if (!g._stretchWidth.__rp_fit_patched) {
                g._stretchWidth = fullWidthFn;
            }
        } else {
            try {
                Object.defineProperty(g, "_stretchWidth", {
                    configurable: true,
                    enumerable: true,
                    get: function() {
                        return fullWidthFn;
                    },
                    set: function(val) {
                        // Retain 100% viewport width
                    }
                });
            } catch (e) {}
        }
    }

    root.patchGraphics = patchGraphics;
    if (typeof root.window !== "undefined") {
        root.window.patchGraphics = patchGraphics;
    }

    // [CAP-WEB-013] SceneManager caught exception diagnostics
    function patchSceneManager(sm) {
        if (!sm || !sm.catchException || sm.catchException.__patched) return;
        var origCatch = sm.catchException;
        sm.catchException = function(e) {
            if (root.console && typeof root.console.error === "function") {
                root.console.error("[RuntimePack Caught Error]", e && e.stack ? e.stack : e);
            }
            return origCatch.apply(this, arguments);
        };
        sm.catchException.__patched = true;
    }

    // [CAP-WEB-004] Touch input jitter tolerance and single-tap mobile OK
    function patchEngineInput() {
        var Input_ = root.Input;
        var TouchInput_ = root.TouchInput;

        // -------------------------------------------------------------
        // A. Touch jitter tolerance: 10px terlalu sensitif di layar DPI tinggi
        // -------------------------------------------------------------
        if (TouchInput_ && TouchInput_.moveThreshold === 10) {
            TouchInput_.moveThreshold = 24;
        }

        // -------------------------------------------------------------
        // B. Single-tap OK pada seluruh window select (mode web/mobile)
        // -------------------------------------------------------------
        if (root.Window_SavefileList && root.Window_SavefileList.prototype &&
            !root.Window_SavefileList.prototype.__runtimepack_touch_ok_patched) {
            root.Window_SavefileList.prototype.isTouchOkEnabled = function() {
                return this.isOkEnabled();
            };
            root.Window_SavefileList.prototype.__runtimepack_touch_ok_patched = true;
        }

        if (root.Window_Selectable && root.Window_Selectable.prototype &&
            !root.Window_Selectable.prototype.__runtimepack_touch_ok_patched) {
            var isNwjs = !!(root.Utils && typeof root.Utils.isNwjs === "function" && root.Utils.isNwjs());
            if (!isNwjs) {
                root.Window_Selectable.prototype.isTouchOkEnabled = function() {
                    return this.isOkEnabled();
                };
            }
            root.Window_Selectable.prototype.__runtimepack_touch_ok_patched = true;
        }

        // -------------------------------------------------------------
        // C. Input.codeMapper fallback: event.code W3C saat keyCode === 0
        //    (Chromium modern membuang keyCode dari KeyboardEventInit)
        // -------------------------------------------------------------
        // [CAP-WEB-005] W3C Keyboard codeMapper and virtual gamepad input bridge
        if (Input_ && !Input_.__runtimepack_codemapper_patched) {
            Input_.codeMapper = {
                "Enter": "ok", "NumpadEnter": "ok", "Space": "ok",
                "KeyZ": "ok", "KeyJ": "ok",
                "Escape": "escape", "KeyX": "escape", "KeyK": "escape", "Numpad0": "escape",
                "ShiftLeft": "shift", "ShiftRight": "shift",
                "ArrowUp": "up", "KeyW": "up",
                "ArrowDown": "down", "KeyS": "down",
                "ArrowLeft": "left", "KeyA": "left",
                "ArrowRight": "right", "KeyD": "right",
                "PageUp": "pageup", "KeyQ": "pageup",
                "PageDown": "pagedown", "KeyE": "pagedown",
                "F9": "debug"
            };
            var origOnKeyDown = Input_._onKeyDown;
            if (typeof origOnKeyDown === "function") {
                Input_._onKeyDown = function(event) {
                    var keyCode = event ? event.keyCode : 0;
                    var code = event ? event.code : "";
                    var handled = false;
                    if (!keyCode && code && this.codeMapper && this.codeMapper[code]) {
                        var buttonName = this.codeMapper[code];
                        this._currentState[buttonName] = true;
                        this._latestButton = buttonName;
                        this._pressedTime = 0;
                        this._date = Date.now();
                        handled = true;
                    }
                    if (!handled) {
                        return origOnKeyDown.call(this, event);
                    }
                };
            }
            var origOnKeyUp = Input_._onKeyUp;
            if (typeof origOnKeyUp === "function") {
                Input_._onKeyUp = function(event) {
                    var keyCode = event.keyCode;
                    var code = event.code;
                    var handled = false;
                    if (!keyCode && code && this.codeMapper && this.codeMapper[code]) {
                        this._currentState[this.codeMapper[code]] = false;
                        handled = true;
                    }
                    if (!handled) {
                        return origOnKeyUp.call(this, event);
                    }
                };
            }
            Input_.__runtimepack_codemapper_patched = true;
        }

        // -------------------------------------------------------------
        // D. API global untuk virtual gamepad Android (bypass DOM event)
        // -------------------------------------------------------------
        if (!root.RuntimePackInput) {
            root.RuntimePackInput = {
                press: function(buttonName) {
                    var I = root.Input;
                    if (I && I._currentState && buttonName) {
                        I._currentState[buttonName] = true;
                        I._latestButton = buttonName;
                        I._pressedTime = 0;
                        I._date = Date.now();
                    }
                },
                release: function(buttonName) {
                    var I = root.Input;
                    if (I && I._currentState && buttonName) {
                        I._currentState[buttonName] = false;
                    }
                },
                trigger: function(buttonName) {
                    this.press(buttonName);
                    var I = root.Input;
                    if (I && I._currentState) {
                        I._currentState[buttonName] = false;
                        setTimeout(function() {}, 0);
                    }
                },
                isAvailable: function() {
                    return !!(root.Input && root.Input._currentState);
                }
            };
            if (typeof root.window !== "undefined") {
                root.window.RuntimePackInput = root.RuntimePackInput;
            }
        }
    }

    // [CAP-WEB-007] Fail-loud load failure diagnostics
    function __runtimepackShowLoadError(msg) {
        try {
            console.error("[RuntimePack] RT-LOAD-FAIL " + msg);
            if (typeof document !== "undefined" && document && document.body && typeof document.createElement === "function") {
                var overlay = document.createElement("div");
                overlay.style.cssText = "position:fixed; left:0; right:0; top:40%; background:rgba(150,0,0,0.85); color:#fff; font:bold 14px sans-serif; text-align:center; padding:12px; z-index:99999;";
                overlay.textContent = String(msg);
                document.body.appendChild(overlay);
                if (typeof setTimeout !== "undefined") {
                    setTimeout(function() {
                        try {
                            if (overlay && overlay.parentNode) {
                                overlay.parentNode.removeChild(overlay);
                            }
                        } catch (e) {}
                    }, 8000);
                }
            }
        } catch (e) {}
    }
    root.__runtimepackShowLoadError = __runtimepackShowLoadError;
    if (typeof root.window !== "undefined") {
        root.window.__runtimepackShowLoadError = __runtimepackShowLoadError;
    }

    function patchLoadDiagnostics() {
        try {
            var sLoad = root.Scene_Load || (typeof window !== "undefined" && window.Scene_Load);
            if (sLoad && sLoad.prototype && !sLoad.prototype.__runtimepack_load_diag_patched) {
                var origOnLoadFailure = sLoad.prototype.onLoadFailure;
                sLoad.prototype.onLoadFailure = function() {
                    try {
                        __runtimepackShowLoadError("Load game gagal \u2014 cek logcat [GameConsole] untuk detail (RT-LOAD-FAIL)");
                    } catch (e) {}
                    if (typeof origOnLoadFailure === "function") {
                        return origOnLoadFailure.apply(this, arguments);
                    }
                };
                sLoad.prototype.__runtimepack_load_diag_patched = true;
                root.__runtimepack_load_diag_patched = true;
            }

            var dataMgr = root.DataManager || (typeof window !== "undefined" && window.DataManager);
            if (dataMgr && typeof dataMgr.loadGame === "function" && !dataMgr.__runtimepack_loadgame_diag_patched) {
                var origLoadGame = dataMgr.loadGame;
                dataMgr.loadGame = function(savefileId) {
                    var p = origLoadGame.apply(this, arguments);
                    if (p && typeof p.then === "function" && typeof p.catch === "function") {
                        return p.catch(function(err) {
                            try {
                                console.error("[RuntimePack] RT-LOAD-FAIL loadGame rejection:", err && err.message ? err.message : err);
                            } catch (e) {}
                            throw err;
                        });
                    }
                    return p;
                };
                dataMgr.__runtimepack_loadgame_diag_patched = true;
            }
        } catch (e) {}
    }

    var _compatDummyWindow = null;
    // [CAP-WEB-006] HalfMove plugin Sprite_Button escape character delegation
    function patchHalfMoveCompat() {
        if (root.__runtimepack_halfmove_compat_patched) return;
        var Sprite_Button = root.Sprite_Button || (typeof window !== "undefined" && window.Sprite_Button);
        if (!Sprite_Button || !Sprite_Button.prototype) return;

        root.__runtimepack_halfmove_compat_patched = true;

        Sprite_Button.prototype.convertEscapeCharacters = function(text) {
            try {
                var scene = (root.SceneManager && root.SceneManager._scene) ||
                            (typeof window !== "undefined" && window.SceneManager && window.SceneManager._scene);
                if (scene) {
                    if (scene._helpWindow && typeof scene._helpWindow.convertEscapeCharacters === "function") {
                        return scene._helpWindow.convertEscapeCharacters(text);
                    }
                    if (scene._windowLayer && Array.isArray(scene._windowLayer.children)) {
                        for (var i = 0; i < scene._windowLayer.children.length; i++) {
                            var child = scene._windowLayer.children[i];
                            if (child && child !== this && typeof child.convertEscapeCharacters === "function") {
                                return child.convertEscapeCharacters(text);
                            }
                        }
                    }
                }
                var Window_Base = root.Window_Base || (typeof window !== "undefined" && window.Window_Base);
                if (Window_Base && Window_Base.prototype && typeof Window_Base.prototype.convertEscapeCharacters === "function") {
                    if (!_compatDummyWindow) {
                        if (typeof root.Rectangle === "function") {
                            try {
                                _compatDummyWindow = new Window_Base(new root.Rectangle(0, 0, 0, 0));
                            } catch (e) {}
                        }
                        if (!_compatDummyWindow) {
                            _compatDummyWindow = Object.create(Window_Base.prototype);
                        }
                    }
                    if (_compatDummyWindow && typeof _compatDummyWindow.convertEscapeCharacters === "function") {
                        return _compatDummyWindow.convertEscapeCharacters(text);
                    }
                    return Window_Base.prototype.convertEscapeCharacters.call(this, text);
                }
            } catch (err) {
                if (root.console && typeof root.console.warn === "function") {
                    root.console.warn("[RuntimePack] Sprite_Button.convertEscapeCharacters delegation error:", err);
                }
            }
            return text;
        };
    }

    function patchVnmAudio() {
        var AudioNodeProto = (typeof AudioBufferSourceNode !== "undefined" && AudioBufferSourceNode.prototype)
            || (typeof root.AudioBufferSourceNode !== "undefined" && root.AudioBufferSourceNode.prototype)
            || (typeof window !== "undefined" && window.AudioBufferSourceNode && window.AudioBufferSourceNode.prototype);
        if (AudioNodeProto) {
            var origStart = AudioNodeProto.start;
            if (origStart && !origStart.__runtimepack_started_guarded) {
                AudioNodeProto.start = function() {
                    var res = origStart.apply(this, arguments);
                    this.__rp_started = true;
                    return res;
                };
                AudioNodeProto.start.__runtimepack_started_guarded = true;
            }
            var origStop = AudioNodeProto.stop;
            if (origStop && !origStop.__runtimepack_stop_guarded) {
                AudioNodeProto.stop = function(when) {
                    if (!this.__rp_started) {
                        return;
                    }
                    return origStop.apply(this, arguments);
                };
                AudioNodeProto.stop.__runtimepack_stop_guarded = true;
            }
            var desc = Object.getOwnPropertyDescriptor(AudioNodeProto, "onended");
            if (!desc && typeof AudioScheduledSourceNode !== "undefined") {
                desc = Object.getOwnPropertyDescriptor(AudioScheduledSourceNode.prototype, "onended");
            }
            if (desc && desc.set && !desc.set.__runtimepack_onended_wrapped) {
                var origSetter = desc.set;
                var origGetter = desc.get;
                var wrappedSetter = function(fn) {
                    if (typeof fn !== "function") {
                        return origSetter.call(this, fn);
                    }
                    var node = this;
                    var wrapped = function(event) {
                        if (node.__rp_invalidated || (node.__rp_owner && node.__rp_owner.__rp_disposed)) {
                            return;
                        }
                        return fn.apply(this, arguments);
                    };
                    return origSetter.call(this, wrapped);
                };
                wrappedSetter.__runtimepack_onended_wrapped = true;
                Object.defineProperty(AudioNodeProto, "onended", {
                    get: origGetter,
                    set: wrappedSetter,
                    configurable: true,
                    enumerable: true
                });
            }
        }
        var gsObj = (typeof window !== "undefined" && window.gs) || root.gs;
        var GSObj = (typeof window !== "undefined" && window.GS) || root.GS;
        if (gsObj || GSObj) {
            var vnmPendingSidecar = typeof WeakMap !== "undefined" ? new WeakMap() : null;
            var patchedInstances = typeof WeakSet !== "undefined" ? new WeakSet() : null;
            var getPendingSidecar = function(ref) {
                if (vnmPendingSidecar) {
                    var s = vnmPendingSidecar.get(ref);
                    if (!s) {
                        s = {};
                        vnmPendingSidecar.set(ref, s);
                    }
                    return s;
                }
                if (!ref.__rp_pending_sidecar) {
                    Object.defineProperty(ref, "__rp_pending_sidecar", {
                        value: {},
                        writable: true,
                        configurable: true,
                        enumerable: false
                    });
                }
                return ref.__rp_pending_sidecar;
            };

            var warnedAudioKeys = {};
            var logAudioDiag = function(key, msg) {
                if (!warnedAudioKeys[key]) {
                    warnedAudioKeys[key] = true;
                    console.warn("[CAP-WEB-022] " + msg);
                }
            };

            var RefProto = (gsObj && gsObj.AudioBufferReference && gsObj.AudioBufferReference.prototype)
                || (GSObj && GSObj.AudioBufferReference && GSObj.AudioBufferReference.prototype);

            var origRefIsPlayingDesc = RefProto && Object.getOwnPropertyDescriptor(RefProto, "isPlaying");
            var origRefIsPlayingGetter = origRefIsPlayingDesc && origRefIsPlayingDesc.get;

            var origRefVolumeDesc = RefProto && Object.getOwnPropertyDescriptor(RefProto, "volume");
            var origRefVolumeGetter = origRefVolumeDesc && origRefVolumeDesc.get;
            var origRefVolumeSetter = origRefVolumeDesc && origRefVolumeDesc.set;

            var origRefPlaybackRateDesc = RefProto && Object.getOwnPropertyDescriptor(RefProto, "playbackRate");
            var origRefPlaybackRateGetter = origRefPlaybackRateDesc && origRefPlaybackRateDesc.get;
            var origRefPlaybackRateSetter = origRefPlaybackRateDesc && origRefPlaybackRateDesc.set;

            var StreamProto = (gsObj && gsObj.AudioBufferStream && gsObj.AudioBufferStream.prototype)
                || (GSObj && GSObj.AudioBufferStream && GSObj.AudioBufferStream.prototype);

            var origStreamIsPlayingDesc = StreamProto && Object.getOwnPropertyDescriptor(StreamProto, "isPlaying");
            var origStreamIsPlayingGetter = origStreamIsPlayingDesc && origStreamIsPlayingDesc.get;

            var isAudioBufferReference = function(obj) {
                if (!obj || typeof obj !== "object") return false;
                var RefClass = (gsObj && gsObj.AudioBufferReference) || (GSObj && GSObj.AudioBufferReference);
                if (RefClass && obj instanceof RefClass) return true;
                return typeof obj.initialize === "function" && ("audioBuffer" in obj) && ("playing" in obj);
            };

            var isAudioBufferStream = function(obj) {
                if (!obj || typeof obj !== "object") return false;
                var StreamClass = (gsObj && gsObj.AudioBufferStream) || (GSObj && GSObj.AudioBufferStream);
                if (StreamClass && obj instanceof StreamClass) return true;
                return ("audioStream" in obj) && ("fadeInVolume" in obj);
            };

            var patchAudioInstance = function(inst) {
                if (!inst || typeof inst !== "object") return;
                if (patchedInstances) {
                    if (patchedInstances.has(inst)) return;
                    patchedInstances.add(inst);
                } else {
                    if (inst.__rp_instance_patched) return;
                    Object.defineProperty(inst, "__rp_instance_patched", {
                        value: true,
                        enumerable: false,
                        configurable: true,
                        writable: true
                    });
                }

                if (isAudioBufferReference(inst)) {
                    var ownIsPlaying = Object.getOwnPropertyDescriptor(inst, "isPlaying");
                    if (!ownIsPlaying || ownIsPlaying.configurable) {
                        Object.defineProperty(inst, "isPlaying", {
                            get: function() {
                                if (this.__rp_disposed) return false;
                                if (!this.audioBuffer) return false;
                                if (this.audioBuffer.error) return false;
                                return origRefIsPlayingGetter
                                    ? origRefIsPlayingGetter.call(this)
                                    : (null != this.context ? Boolean(this.playing) : (this.source ? !this.source.paused : false));
                            },
                            enumerable: false,
                            configurable: true
                        });
                    } else {
                        logAudioDiag("own_is_playing_unsupported", "Unsupported own non-configurable isPlaying descriptor on reference");
                    }

                    var ownVolume = Object.getOwnPropertyDescriptor(inst, "volume");
                    if (!ownVolume || ownVolume.configurable) {
                        Object.defineProperty(inst, "volume", {
                            get: function() {
                                if (this.__rp_disposed) return 100;
                                if (!this.audioBuffer) {
                                    var sc = getPendingSidecar(this);
                                    return sc.queuedVolume !== undefined ? sc.queuedVolume : 100;
                                }
                                return origRefVolumeGetter
                                    ? origRefVolumeGetter.call(this)
                                    : (this.audioBuffer.error ? 100 : (null != this.context ? (this.gainNode && this.gainNode.gain ? this.gainNode.gain.value : 100) : (this.source ? this.source.volume : 100)));
                            },
                            set: function(v) {
                                if (this.__rp_disposed) return;
                                if (!this.audioBuffer) {
                                    getPendingSidecar(this).queuedVolume = v;
                                    return;
                                }
                                if (origRefVolumeSetter) {
                                    origRefVolumeSetter.call(this, v);
                                } else if (!this.audioBuffer.error) {
                                    if (null != this.context && this.gainNode && this.gainNode.gain) {
                                        this.gainNode.gain.value = v;
                                    } else if (this.source) {
                                        this.source.volume = v;
                                    }
                                }
                            },
                            enumerable: false,
                            configurable: true
                        });
                    } else {
                        logAudioDiag("own_volume_unsupported", "Unsupported own non-configurable volume descriptor on reference");
                    }

                    var ownPlaybackRate = Object.getOwnPropertyDescriptor(inst, "playbackRate");
                    if (!ownPlaybackRate || ownPlaybackRate.configurable) {
                        Object.defineProperty(inst, "playbackRate", {
                            get: function() {
                                if (this.__rp_disposed) return undefined;
                                if (!this.audioBuffer) {
                                    var sc = getPendingSidecar(this);
                                    return sc.queuedPlaybackRate !== undefined ? sc.queuedPlaybackRate : undefined;
                                }
                                return origRefPlaybackRateGetter
                                    ? origRefPlaybackRateGetter.call(this)
                                    : (this.audioBuffer.error ? undefined : (this.context ? ((this.source && this.source.playbackRate ? this.source.playbackRate.value : undefined) || 100) : (this.source ? this.source.playbackRate : 100)));
                            },
                            set: function(r) {
                                if (this.__rp_disposed) return;
                                if (!this.audioBuffer) {
                                    getPendingSidecar(this).queuedPlaybackRate = r;
                                    return;
                                }
                                if (origRefPlaybackRateSetter) {
                                    origRefPlaybackRateSetter.call(this, r);
                                } else if (!this.audioBuffer.error && this.source) {
                                    if (this.context && this.source.playbackRate) {
                                        this.source.playbackRate.value = r;
                                    } else {
                                        this.source.playbackRate = r;
                                    }
                                }
                            },
                            enumerable: false,
                            configurable: true
                        });
                    } else {
                        logAudioDiag("own_playback_rate_unsupported", "Unsupported own non-configurable playbackRate descriptor on reference");
                    }
                } else if (isAudioBufferStream(inst)) {
                    var ownStreamPlaying = Object.getOwnPropertyDescriptor(inst, "isPlaying");
                    if (!ownStreamPlaying || ownStreamPlaying.configurable) {
                        Object.defineProperty(inst, "isPlaying", {
                            get: function() {
                                if (this.__rp_disposed || this.disposed || !this.audioStream) return false;
                                return origStreamIsPlayingGetter ? origStreamIsPlayingGetter.call(this) : !this.audioStream.paused;
                            },
                            enumerable: false,
                            configurable: true
                        });
                    } else {
                        logAudioDiag("own_stream_is_playing_unsupported", "Unsupported own non-configurable isPlaying descriptor on stream");
                    }
                }
            };

            var audioManager = (gsObj && gsObj.Audio) || (GSObj && GSObj.Audio);
            var AudioProto = (audioManager && Object.getPrototypeOf(audioManager))
                || (gsObj && gsObj.Audio && gsObj.Audio.constructor && gsObj.Audio.constructor.prototype);

            if (AudioProto && AudioProto.addAudioBuffer && !AudioProto.addAudioBuffer.__runtimepack_add_wrapped) {
                var origProtoAddAudioBuffer = AudioProto.addAudioBuffer;
                AudioProto.addAudioBuffer = function(buf) {
                    patchAudioInstance(buf);
                    return origProtoAddAudioBuffer.apply(this, arguments);
                };
                AudioProto.addAudioBuffer.__runtimepack_add_wrapped = true;
            }

            if (audioManager && audioManager.addAudioBuffer && !audioManager.addAudioBuffer.__runtimepack_add_wrapped) {
                var origInstAddAudioBuffer = audioManager.addAudioBuffer;
                audioManager.addAudioBuffer = function(buf) {
                    patchAudioInstance(buf);
                    return origInstAddAudioBuffer.apply(this, arguments);
                };
                audioManager.addAudioBuffer.__runtimepack_add_wrapped = true;
            }

            if (audioManager && Array.isArray(audioManager.audioBuffers)) {
                for (var bi = 0; bi < audioManager.audioBuffers.length; bi++) {
                    patchAudioInstance(audioManager.audioBuffers[bi]);
                }
            }

            if (RefProto) {
                if (RefProto.initialize && !RefProto.initialize.__runtimepack_init_wrapped) {
                    var origRefInitialize = RefProto.initialize;
                    RefProto.initialize = function() {
                        if (this.__rp_disposed) {
                            return;
                        }
                        if (!this.audioBuffer) {
                            logAudioDiag("ref_init_null_buf", "AudioBufferReference.initialize called with null audioBuffer");
                            return;
                        }
                        if (this.audioBuffer.error) {
                            return;
                        }
                        var res = origRefInitialize.apply(this, arguments);
                        if (this.source) {
                            Object.defineProperty(this.source, "__rp_owner", {
                                value: this,
                                enumerable: false,
                                configurable: true,
                                writable: true
                            });
                        }
                        var sc = getPendingSidecar(this);
                        if (sc.queuedVolume !== undefined) {
                            var qv = sc.queuedVolume;
                            delete sc.queuedVolume;
                            this.volume = qv;
                        }
                        if (sc.queuedPlaybackRate !== undefined) {
                            var qr = sc.queuedPlaybackRate;
                            delete sc.queuedPlaybackRate;
                            this.playbackRate = qr;
                        }
                        return res;
                    };
                    RefProto.initialize.__runtimepack_init_wrapped = true;
                }

                if (RefProto.stop && !RefProto.stop.__runtimepack_stop_wrapped) {
                    var origRefStop = RefProto.stop;
                    RefProto.stop = function() {
                        if (this.__rp_disposed || !this.audioBuffer || this.audioBuffer.error || !this.source) {
                            if (!this.audioBuffer) {
                                logAudioDiag("ref_stop_null_buf", "AudioBufferReference.stop safely ignored on pending null buffer");
                            }
                            return;
                        }
                        return origRefStop.apply(this, arguments);
                    };
                    RefProto.stop.__runtimepack_stop_wrapped = true;
                }

                if (RefProto.play && !RefProto.play.__runtimepack_play_wrapped) {
                    var origRefPlay = RefProto.play;
                    RefProto.play = function() {
                        if (this.__rp_disposed || !this.audioBuffer || this.audioBuffer.error || (this.context && !this.source)) {
                            if (!this.audioBuffer) {
                                logAudioDiag("ref_play_null_buf", "AudioBufferReference.play safely ignored on pending null buffer");
                            }
                            return;
                        }
                        var res = origRefPlay.apply(this, arguments);
                        if (this.source) {
                            Object.defineProperty(this.source, "__rp_owner", {
                                value: this,
                                enumerable: false,
                                configurable: true,
                                writable: true
                            });
                        }
                        return res;
                    };
                    RefProto.play.__runtimepack_play_wrapped = true;
                }

                if (RefProto.resume && !RefProto.resume.__runtimepack_resume_wrapped) {
                    var origRefResume = RefProto.resume;
                    RefProto.resume = function() {
                        if (this.__rp_disposed || !this.audioBuffer || this.audioBuffer.error || (this.context && !this.source)) {
                            if (!this.audioBuffer) {
                                logAudioDiag("ref_resume_null_buf", "AudioBufferReference.resume safely ignored on pending null buffer");
                            }
                            return;
                        }
                        var res = origRefResume.apply(this, arguments);
                        if (this.source) {
                            Object.defineProperty(this.source, "__rp_owner", {
                                value: this,
                                enumerable: false,
                                configurable: true,
                                writable: true
                            });
                        }
                        return res;
                    };
                    RefProto.resume.__runtimepack_resume_wrapped = true;
                }

                if (RefProto.dispose && !RefProto.dispose.__runtimepack_dispose_wrapped) {
                    var origRefDispose = RefProto.dispose;
                    RefProto.dispose = function() {
                        if (this.source) {
                            Object.defineProperty(this.source, "__rp_invalidated", {
                                value: true,
                                enumerable: false,
                                configurable: true,
                                writable: true
                            });
                        }
                        Object.defineProperty(this, "__rp_disposed", {
                            value: true,
                            enumerable: false,
                            configurable: true,
                            writable: true
                        });
                        return origRefDispose.apply(this, arguments);
                    };
                    RefProto.dispose.__runtimepack_dispose_wrapped = true;
                }
            }

            if (StreamProto) {
                if (StreamProto.dispose && !StreamProto.dispose.__runtimepack_stream_dispose_wrapped) {
                    var origStreamDispose = StreamProto.dispose;
                    StreamProto.dispose = function() {
                        Object.defineProperty(this, "__rp_disposed", {
                            value: true,
                            enumerable: false,
                            configurable: true,
                            writable: true
                        });
                        return origStreamDispose.apply(this, arguments);
                    };
                    StreamProto.dispose.__runtimepack_stream_dispose_wrapped = true;
                }

                if (StreamProto.stop && !StreamProto.stop.__runtimepack_stream_stop_wrapped) {
                    var origStreamStop = StreamProto.stop;
                    StreamProto.stop = function() {
                        if (this.__rp_disposed || this.disposed || !this.audioStream) {
                            return;
                        }
                        return origStreamStop.apply(this, arguments);
                    };
                    StreamProto.stop.__runtimepack_stream_stop_wrapped = true;
                }
            }

            var BufProto = (gsObj && gsObj.AudioBuffer && gsObj.AudioBuffer.prototype)
                || (GSObj && GSObj.AudioBuffer && GSObj.AudioBuffer.prototype);

            if (BufProto) {
                if (BufProto.dispose && !BufProto.dispose.__runtimepack_dispose_wrapped) {
                    var origBufDispose = BufProto.dispose;
                    BufProto.dispose = function() {
                        if (this.source) {
                            Object.defineProperty(this.source, "__rp_invalidated", {
                                value: true,
                                enumerable: false,
                                configurable: true,
                                writable: true
                            });
                        }
                        Object.defineProperty(this, "__rp_disposed", {
                            value: true,
                            enumerable: false,
                            configurable: true,
                            writable: true
                        });
                        return origBufDispose.apply(this, arguments);
                    };
                    BufProto.dispose.__runtimepack_dispose_wrapped = true;
                }

                if (BufProto.play && !BufProto.play.__runtimepack_play_wrapped) {
                    var origBufPlay = BufProto.play;
                    BufProto.play = function() {
                        var res = origBufPlay.apply(this, arguments);
                        if (this.source) {
                            Object.defineProperty(this.source, "__rp_owner", {
                                value: this,
                                enumerable: false,
                                configurable: true,
                                writable: true
                            });
                        }
                        return res;
                    };
                    BufProto.play.__runtimepack_play_wrapped = true;
                }
            }

            var sanitizeSoundPoolInPlace = function(arr) {
                if (!Array.isArray(arr)) return 0;
                var removed = 0;
                for (var i = arr.length - 1; i >= 0; i--) {
                    if (arr[i] === null || arr[i] === undefined) {
                        arr.splice(i, 1);
                        removed++;
                    }
                }
                return removed;
            };

            var warnedPoolKeys = {};
            var logPoolDiag = function(key, msg) {
                if (!warnedPoolKeys[key]) {
                    warnedPoolKeys[key] = true;
                    if (typeof console !== "undefined" && typeof console.warn === "function") {
                        console.warn("[CAP-WEB-025] " + msg);
                    }
                    var bridge = (typeof root.window !== "undefined" && root.window.RuntimePackNative)
                        || root.RuntimePackNative
                        || (typeof window !== "undefined" && window.RuntimePackNative);
                    if (bridge && typeof bridge.postDiagnosticError === "function") {
                        try {
                            bridge.postDiagnosticError("WEB-025", msg);
                        } catch (e) {}
                    }
                }
            };

            var sanitizeSoundNamePool = function(target, name) {
                if (!target || !target.soundReferences) return;
                var soundKey = (name && typeof name === "object" && name.name != null) ? name.name : name;
                if (soundKey && Array.isArray(target.soundReferences[soundKey])) {
                    var count = sanitizeSoundPoolInPlace(target.soundReferences[soundKey]);
                    if (count > 0) {
                        logPoolDiag("sanitized_pool_" + soundKey, "Sanitized " + count + " null/hole reference(s) from pool: " + soundKey);
                    }
                }
            };

            var sanitizeAllSoundPools = function(target) {
                if (!target || !target.soundReferences) return;
                var total = 0;
                for (var k in target.soundReferences) {
                    if (Object.prototype.hasOwnProperty.call(target.soundReferences, k)) {
                        total += sanitizeSoundPoolInPlace(target.soundReferences[k]);
                    }
                }
                if (total > 0) {
                    logPoolDiag("sanitized_all_pools", "Sanitized " + total + " null/hole reference(s) across sound pools");
                }
            };

            var wrapAudioManagerTarget = function(target) {
                if (!target) return;

                if (typeof target.playSound === "function" && !target.playSound.__runtimepack_pool_safe) {
                    var origPlaySound = target.playSound;
                    target.playSound = function(name, volume, rate, musicEffect, voice, loopSound) {
                        sanitizeSoundNamePool(this, name);
                        return origPlaySound.apply(this, arguments);
                    };
                    target.playSound.__runtimepack_pool_safe = true;
                }

                if (typeof target.stopSound === "function" && !target.stopSound.__runtimepack_pool_safe) {
                    var origStopSound = target.stopSound;
                    target.stopSound = function(name) {
                        sanitizeSoundNamePool(this, name);
                        return origStopSound.apply(this, arguments);
                    };
                    target.stopSound.__runtimepack_pool_safe = true;
                }

                if (typeof target.stopAllVoices === "function" && !target.stopAllVoices.__runtimepack_pool_safe) {
                    var origStopAllVoices = target.stopAllVoices;
                    target.stopAllVoices = function() {
                        sanitizeAllSoundPools(this);
                        return origStopAllVoices.apply(this, arguments);
                    };
                    target.stopAllVoices.__runtimepack_pool_safe = true;
                }

                if (typeof target.stopAllSounds === "function" && !target.stopAllSounds.__runtimepack_pool_safe) {
                    var origStopAllSounds = target.stopAllSounds;
                    target.stopAllSounds = function() {
                        sanitizeAllSoundPools(this);
                        return origStopAllSounds.apply(this, arguments);
                    };
                    target.stopAllSounds.__runtimepack_pool_safe = true;
                }
            };

            var amProto = (gsObj && gsObj.AudioManager && gsObj.AudioManager.prototype)
                || (GSObj && GSObj.AudioManager && GSObj.AudioManager.prototype);
            if (amProto) {
                wrapAudioManagerTarget(amProto);
            }
            var winAm = (typeof window !== "undefined" && window.AudioManager) || (typeof root !== "undefined" && root.AudioManager);
            if (winAm) {
                wrapAudioManagerTarget(Object.getPrototypeOf(winAm));
                wrapAudioManagerTarget(winAm);
            }
        }
    }

    function patchVnmStorage(storage) {
        if (!storage || storage.__runtimepack_vnm_save_patched) return;
        storage.__runtimepack_vnm_save_patched = true;

        var getNamespace = function() {
            if (typeof root.$PARAMS !== "undefined" && root.$PARAMS && root.$PARAMS.uid) {
                return String(root.$PARAMS.uid);
            }
            if (typeof window !== "undefined" && window.$PARAMS && window.$PARAMS.uid) {
                return String(window.$PARAMS.uid);
            }
            return "vnm";
        };

        var getBridge = function() {
            return (typeof root.window !== "undefined" && root.window.RuntimePackNative)
                || root.RuntimePackNative
                || (typeof window !== "undefined" && window.RuntimePackNative);
        };

        var decodeStrictBase64Utf8 = function(b64) {
            if (typeof b64 !== "string" || !b64) return null;
            var trimmed = b64.trim();
            if (trimmed.length === 0 || trimmed.length % 4 !== 0) return null;
            if (!/^[A-Za-z0-9+/]+={0,2}$/.test(trimmed)) return null;
            try {
                if (typeof Buffer !== "undefined") {
                    var buf = Buffer.from(trimmed, "base64");
                    if (typeof TextDecoder !== "undefined") {
                        var decoder = new TextDecoder("utf-8", { fatal: true });
                        return decoder.decode(buf);
                    }
                    return buf.toString("utf8");
                }
                var atobFn = (typeof atob === "function") ? atob : (typeof window !== "undefined" && typeof window.atob === "function" ? window.atob : null);
                if (atobFn) {
                    var binary = atobFn(trimmed);
                    var bytes = new Uint8Array(binary.length);
                    for (var i = 0; i < binary.length; i++) {
                        bytes[i] = binary.charCodeAt(i);
                    }
                    if (typeof TextDecoder !== "undefined") {
                        var decoder2 = new TextDecoder("utf-8", { fatal: true });
                        return decoder2.decode(bytes);
                    }
                    return decodeURIComponent(escape(binary));
                }
            } catch (e) {
                return null;
            }
            return null;
        };

        storage.exists = function(slotKey) {
            var rawKey = String(slotKey);
            var bridge = getBridge();
            if (bridge && typeof bridge.slotExists === "function") {
                var ns = getNamespace();
                if (bridge.slotExists(ns, rawKey)) {
                    return true;
                }
                if (ns && ns !== "vnm") {
                    var legacySlot = encodeURIComponent(ns + "_" + rawKey);
                    if (bridge.slotExists("vnm", legacySlot)) {
                        var rawLegacy = bridge.loadSlot("vnm", legacySlot);
                        if (rawLegacy !== null && rawLegacy !== undefined) {
                            return decodeStrictBase64Utf8(String(rawLegacy)) !== null;
                        }
                    }
                }
                return false;
            }
            if (bridge && typeof bridge.loadSlot === "function") {
                return storage.getData(slotKey) !== null;
            }
            return null !== localStorage.getItem(getNamespace() + "_" + rawKey);
        };

        storage.remove = function(slotKey) {
            var bridge = getBridge();
            if (bridge && typeof bridge.deleteSlot === "function") {
                var ns = getNamespace();
                if (typeof bridge.registerNamespace === "function") {
                    bridge.registerNamespace(ns);
                }
                bridge.deleteSlot(ns, String(slotKey));
            }
            localStorage.removeItem(getNamespace() + "_" + slotKey);
        };

        storage.setData = function(slotKey, data) {
            var strVal = String(data === undefined || data === null ? "" : data);
            var bridge = getBridge();
            if (bridge && typeof bridge.saveSlot === "function") {
                var ns = getNamespace();
                if (typeof bridge.registerNamespace === "function") {
                    bridge.registerNamespace(ns);
                }
                var ok = bridge.saveSlot(ns, String(slotKey), strVal);
                if (!ok) {
                    throw new Error("[RuntimePack] RT-SAVE-FAIL: native saveSlot failed for slot: " + slotKey);
                }
                return ok;
            }
            localStorage.setItem(getNamespace() + "_" + slotKey, strVal);
            return true;
        };

        storage.getData = function(slotKey) {
            var rawKey = String(slotKey);
            var bridge = getBridge();
            if (bridge && typeof bridge.loadSlot === "function") {
                var ns = getNamespace();
                var hasCurrent = false;
                if (typeof bridge.slotExists === "function") {
                    hasCurrent = !!bridge.slotExists(ns, rawKey);
                }
                if (hasCurrent) {
                    var currentVal = bridge.loadSlot(ns, rawKey);
                    if (currentVal === null || currentVal === undefined) {
                        throw new Error("[RuntimePack] RT-LOAD-FAIL: native slotExists was true but loadSlot returned null for slot: " + rawKey);
                    }
                    return String(currentVal);
                }
                if (!hasCurrent && typeof bridge.slotExists !== "function") {
                    var directVal = bridge.loadSlot(ns, rawKey);
                    if (directVal !== null && directVal !== undefined) {
                        return String(directVal);
                    }
                }
                if (ns && ns !== "vnm") {
                    var legacySlot = encodeURIComponent(ns + "_" + rawKey);
                    var hasLegacy = false;
                    if (typeof bridge.slotExists === "function") {
                        hasLegacy = !!bridge.slotExists("vnm", legacySlot);
                    }
                    if (hasLegacy || typeof bridge.slotExists !== "function") {
                        var rawLegacy = bridge.loadSlot("vnm", legacySlot);
                        if (rawLegacy !== null && rawLegacy !== undefined) {
                            var decoded = decodeStrictBase64Utf8(String(rawLegacy));
                            if (decoded === null) {
                                if (typeof console !== "undefined" && typeof console.warn === "function") {
                                    console.warn("[CAP-WEB-026] Corrupt legacy Base64 UTF-8 save record for slot: " + rawKey);
                                }
                                if (typeof bridge.postDiagnosticError === "function") {
                                    try {
                                        bridge.postDiagnosticError("WEB-026", "Corrupt legacy Base64 UTF-8 save record for slot: " + rawKey);
                                    } catch (e) {}
                                }
                                return null;
                            }
                            return decoded;
                        }
                    }
                }
                return null;
            }
            return localStorage.getItem(getNamespace() + "_" + rawKey);
        };

        storage.setObject = function(slotKey, obj) {
            var str = JSON.stringify(obj);
            return storage.setData(slotKey, str);
        };

        storage.getObject = function(slotKey) {
            var str = storage.getData(slotKey);
            if (str === null || str === undefined || str === "") return null;
            try {
                return JSON.parse(str);
            } catch (e) {
                console.warn(slotKey + " corrupted.");
                console.warn(e);
                return null;
            }
        };

        storage.clear = function() {
            var ns = getNamespace();
            var prefix = ns + "_";
            var bridge = getBridge();
            if (bridge && typeof bridge.listSlots === "function" && typeof bridge.deleteSlot === "function") {
                var listJson = bridge.listSlots(ns);
                if (typeof listJson !== "string") {
                    throw new Error("[RuntimePack] RT-SAVE-FAIL: listSlots returned invalid response for namespace: " + ns);
                }
                var list = JSON.parse(listJson);
                if (!Array.isArray(list)) {
                    throw new Error("[RuntimePack] RT-SAVE-FAIL: listSlots did not return array for namespace: " + ns);
                }
                for (var i = 0; i < list.length; i++) {
                    var ok = bridge.deleteSlot(ns, list[i]);
                    if (!ok) {
                        throw new Error("[RuntimePack] RT-SAVE-FAIL: failed to delete slot: " + list[i]);
                    }
                    if (typeof localStorage !== "undefined" && localStorage && typeof localStorage.removeItem === "function") {
                        localStorage.removeItem(prefix + list[i]);
                    }
                }
            }
            if (typeof localStorage !== "undefined" && localStorage) {
                if (typeof localStorage.length === "number" && typeof localStorage.key === "function" && typeof localStorage.removeItem === "function") {
                    var toRemove = [];
                    for (var k = 0; k < localStorage.length; k++) {
                        var kName = localStorage.key(k);
                        if (kName && kName.indexOf(prefix) === 0) {
                            toRemove.push(kName);
                        }
                    }
                    for (var r = 0; r < toRemove.length; r++) {
                        localStorage.removeItem(toRemove[r]);
                    }
                } else if (typeof localStorage.removeItem === "function") {
                    for (var prop in localStorage) {
                        if (Object.prototype.hasOwnProperty.call(localStorage, prop) && prop.indexOf(prefix) === 0) {
                            localStorage.removeItem(prop);
                        }
                    }
                }
            }
        };
    }

    root.__runtimepackPollEngineInput = patchEngineInput;
    if (typeof root.window !== "undefined") {
        root.window.__runtimepackPollEngineInput = patchEngineInput;
    }
    root.__runtimepackPollLoadDiagnostics = patchLoadDiagnostics;
    if (typeof root.window !== "undefined") {
        root.window.__runtimepackPollLoadDiagnostics = patchLoadDiagnostics;
    }
    root.__runtimepackPollHalfMoveCompat = patchHalfMoveCompat;
    if (typeof root.window !== "undefined") {
        root.window.__runtimepackPollHalfMoveCompat = patchHalfMoveCompat;
    }
    function patchVnmMessageCompat() {
        var vnObj = (typeof window !== "undefined" && window.vn) || root.vn;
        if (!vnObj || !vnObj.Component_MessageBehavior || !vnObj.Component_MessageBehavior.prototype) {
            return;
        }
        var proto = vnObj.Component_MessageBehavior.prototype;
        if (typeof proto.voiceDuration !== "function" || proto.voiceDuration.__runtimepack_voice_patched) {
            return;
        }
        var origVoiceDuration = proto.voiceDuration;
        var warnedDegraded = false;
        var warnedNonFinite = false;

        var wrappedVoiceDuration = function() {
            if (this.tempSettings && this.tempSettings.skip) {
                return 1;
            }
            var isVoiceGateActive = (this.voice != null) && this.settings && this.settings.autoMessage && (this.settings.autoMessage.waitForVoice || this.settings.timeMessageToVoice);
            if (isVoiceGateActive) {
                if (!this.voice.source || !this.voice.source.buffer || !this.voice.source.playbackRate) {
                    if (!warnedDegraded) {
                        warnedDegraded = true;
                        if (typeof console !== "undefined" && console.warn) {
                            console.warn("[CAP-WEB-020] voice source or buffer absent during voice duration calculation, returning degraded timing 0");
                        }
                    }
                    return 0;
                }
            }
            var result = origVoiceDuration.apply(this, arguments);
            if (typeof result === "number" && !isFinite(result)) {
                if (!warnedNonFinite) {
                    warnedNonFinite = true;
                    if (typeof console !== "undefined" && console.warn) {
                        console.warn("[CAP-WEB-020] non-finite voice duration result mapped to 0");
                    }
                }
                return 0;
            }
            return result;
        };

        wrappedVoiceDuration.__runtimepack_voice_patched = true;
        proto.voiceDuration = wrappedVoiceDuration;
    }

    root.__runtimepackPollVnmStorage = patchVnmStorage;
    if (typeof root.window !== "undefined") {
        root.window.__runtimepackPollVnmStorage = patchVnmStorage;
    }
    root.__runtimepackPollVnmAudioCompat = patchVnmAudio;
    if (typeof root.window !== "undefined") {
        root.window.__runtimepackPollVnmAudioCompat = patchVnmAudio;
    }
    root.__runtimepackPollVnmMessageCompat = patchVnmMessageCompat;
    if (typeof root.window !== "undefined") {
        root.window.__runtimepackPollVnmMessageCompat = patchVnmMessageCompat;
    }

    function installVnmTouchCancelSafety() {
        var targetWindow = typeof window !== "undefined" ? window : (typeof root.window !== "undefined" ? root.window : null);
        if (!targetWindow || targetWindow.__runtimepack_vnm_touchcancel_installed) {
            return;
        }
        if (typeof targetWindow.addEventListener !== "function") {
            return;
        }
        targetWindow.__runtimepack_vnm_touchcancel_installed = true;

        targetWindow.addEventListener("touchcancel", function(e) {
            var inputObj = targetWindow.Input || (targetWindow.gs && targetWindow.gs.Input) || (typeof root !== "undefined" && root.Input);
            var mouse = inputObj && inputObj.Mouse;
            if (mouse) {
                if (typeof mouse.clear === "function") {
                    mouse.clear();
                }
                mouse.buttonDown = false;
                mouse.buttonUp = false;
                if (mouse.buttons) {
                    for (var i = 0; i < mouse.buttons.length; i++) {
                        mouse.buttons[i] = 0;
                    }
                }
            }
        }, true);
    }

    root.__runtimepackInstallVnmTouchCancelSafety = installVnmTouchCancelSafety;
    if (typeof root.window !== "undefined") {
        root.window.__runtimepackInstallVnmTouchCancelSafety = installVnmTouchCancelSafety;
    }

    // [CAP-WEB-016] Late-binding polling and lifecycle coordinator
    function pollPatches() {
        if (root.StorageManager) {
            patchStorageManager(root.StorageManager);
        }
        if (root.AudioManager) {
            patchAudioManager(root.AudioManager);
        }
        if (root.Graphics) {
            patchGraphics(root.Graphics);
        }
        if (root.SceneManager) {
            patchSceneManager(root.SceneManager);
        }
        if (root.Input || root.TouchInput || root.Window_Selectable) {
            patchEngineInput();
        }
        if (root.RuntimePackAssetAudit && typeof root.RuntimePackAssetAudit.installHooks === "function") {
            root.RuntimePackAssetAudit.installHooks(root);
        }
        if (root.Scene_Load || (typeof window !== "undefined" && window.Scene_Load) || root.DataManager || (typeof window !== "undefined" && window.DataManager)) {
            patchLoadDiagnostics();
        }
        if (root.Sprite_Button || (typeof window !== "undefined" && window.Sprite_Button)) {
            patchHalfMoveCompat();
        }
        var gsStorage = (typeof window !== "undefined" && (window.GameStorage || (window.gs && window.gs.GameStorage))) || root.GameStorage || (root.gs && root.gs.GameStorage);
        if (gsStorage) {
            patchVnmStorage(gsStorage);
        }
        var gsGlobal = (typeof window !== "undefined" && (window.gs || window.GS)) || root.gs || root.GS;
        if (gsGlobal && (gsGlobal.Audio || gsGlobal.AudioBuffer || gsGlobal.AudioBufferReference || gsGlobal.AudioBufferStream)) {
            patchVnmAudio();
        }
        var vnObj = (typeof window !== "undefined" && window.vn) || root.vn;
        if (vnObj && vnObj.Component_MessageBehavior) {
            patchVnmMessageCompat();
        }
        var isVnmDetected = Boolean(vnObj || gsGlobal || gsStorage || (typeof window !== "undefined" && window.Input && window.Input.Mouse));
        if (isVnmDetected) {
            installVnmTouchCancelSafety();
        }
    }

    pollPatches();
    if (typeof setInterval !== "undefined") {
        var pollTicks = 0;
        var patchTimer = setInterval(function() {
            pollPatches();
            pollTicks++;
            if (root.StorageManager && root.AudioManager && root.Graphics && root.SceneManager &&
                root.Input && root.TouchInput && root.Scene_Load && root.Window_SavefileList && pollTicks > 300) {
                clearInterval(patchTimer);
            } else if (pollTicks > 1000) {
                clearInterval(patchTimer);
            }
        }, 10);
    }

    if (typeof module !== "undefined" && module.exports) {
        module.exports = {
            require: root.require,
            nw: root.nw,
            Greenworks: root.Greenworks,
            greenworks: root.greenworks,
            StorageManager: root.StorageManager,
            Graphics: root.Graphics,
            patchGraphics: patchGraphics,
            executeCheatCmd: executeCheatCmd,
            installVnmTouchCancelSafety: installVnmTouchCancelSafety
        };
    }
})(typeof window !== "undefined" ? window : (typeof globalThis !== "undefined" ? globalThis : this));
