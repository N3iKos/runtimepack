(function(root) {
    // [CAP-WEB-012] Global unhandled error and rejection listeners
    if (typeof root.window !== "undefined" && typeof root.window.addEventListener === "function") {
        root.window.addEventListener("error", function(e) {
            if (root.console && typeof root.console.error === "function") {
                root.console.error("[RuntimePack Global Error]", e.message, "at", e.filename, ":", e.lineno, ":", e.colno, e.error && e.error.stack ? e.error.stack : "");
            }
        });
        root.window.addEventListener("unhandledrejection", function(e) {
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

    var nwShim = {
        Window: {
            get: function() {
                return {
                    isFullScreen: false,
                    enterFullscreen: function() {},
                    leaveFullscreen: function() {},
                    on: function() {},
                    showDevTools: function() {},
                    show: function() {},
                    hide: function() {},
                    focus: function() {},
                    blur: function() {}
                };
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
        var BufferMock = {
            from: function(data, enc) {
                return {
                    toString: function() { return String(data || ""); }
                };
            },
            concat: function(arr) {
                return {
                    toString: function() { return ""; }
                };
            },
            alloc: function(size) {
                return new Uint8Array(size);
            }
        };
        root.Buffer = BufferMock;
        if (typeof root.window !== "undefined") {
            root.window.Buffer = BufferMock;
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
            executeCheatCmd: executeCheatCmd
        };
    }
})(typeof window !== "undefined" ? window : (typeof globalThis !== "undefined" ? globalThis : this));
