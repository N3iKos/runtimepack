(function(root) {
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
    }

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
        }
    }

    if (root.__runtimepack_shims_installed) {
        if (typeof root.StorageManager !== "undefined") {
            patchStorageManager(root.StorageManager);
        }
        root.executeCheatCmd = executeCheatCmd;
        if (typeof root.window !== "undefined") {
            root.window.executeCheatCmd = executeCheatCmd;
        }
        return;
    }
    root.__runtimepack_shims_installed = true;

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
            env: {}
        };
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

    function syncExists(relPath) {
        if (!relPath) return false;
        if (typeof XMLHttpRequest === "undefined") return false;
        try {
            var cleanPath = String(relPath).replace(/\\/g, '/');
            cleanPath = cleanPath.replace(/^https?:\/\/[^\/]+\//, '');
            cleanPath = cleanPath.replace(/^\/+/, '');
            while (cleanPath.startsWith("./")) {
                cleanPath = cleanPath.substring(2);
            }
            if (cleanPath.endsWith("gameenv.iz")) {
                return false;
            }
            var url = "https://game.local/" + cleanPath;
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
            var exists = syncExists(p);
            return {
                isFile: function() { return exists; },
                isDirectory: function() { return false; },
                size: exists ? (syncFetch(p) || "").length : 0
            };
        },
        readdirSync: function() { return []; },
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
        if (root.console && typeof root.console.warn === "function") {
            root.console.warn("[RuntimePack] Unhandled require module:", mod);
        }
        return {};
    };

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

    function patchAudioManager(am) {
        if (!am || (am.audioFileExt && am.audioFileExt.__patched)) return;
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

    function patchGraphics(g) {
        if (!g || !g._clearUpperCanvas || g._clearUpperCanvas.__patched) return;
        var origClear = g._clearUpperCanvas;
        g._clearUpperCanvas = function() {
            if (!this._upperCanvas) return;
            return origClear.apply(this, arguments);
        };
        g._clearUpperCanvas.__patched = true;
    }

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
    }

    pollPatches();
    if (typeof setInterval !== "undefined") {
        var patchTimer = setInterval(function() {
            pollPatches();
            if (root.StorageManager && root.AudioManager && root.Graphics && root.SceneManager) {
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
            executeCheatCmd: executeCheatCmd
        };
    }
})(typeof window !== "undefined" ? window : (typeof globalThis !== "undefined" ? globalThis : this));
