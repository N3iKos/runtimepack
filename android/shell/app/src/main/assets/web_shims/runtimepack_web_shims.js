(function(root) {
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

    if (root.__runtimepack_shims_installed) {
        if (typeof root.StorageManager !== "undefined") {
            patchStorageManager(root.StorageManager);
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
            manifest: {}
        }
    };

    var fsShim = {
        existsSync: function() { return false; },
        readFileSync: function() { return ""; },
        mkdirSync: function() {},
        writeFileSync: function() {},
        statSync: function() {
            return {
                isFile: function() { return false; },
                isDirectory: function() { return false; }
            };
        },
        readdirSync: function() { return []; },
        unlinkSync: function() {},
        readFile: function(file, opt, cb) {
            var callback = typeof opt === "function" ? opt : cb;
            if (callback) callback(null, "");
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
    root.Greenworks = greenworksShim;
    root.greenworks = greenworksShim;

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

    if (typeof module !== "undefined" && module.exports) {
        module.exports = {
            require: root.require,
            nw: root.nw,
            Greenworks: root.Greenworks,
            greenworks: root.greenworks,
            StorageManager: root.StorageManager
        };
    }
})(typeof window !== "undefined" ? window : (typeof globalThis !== "undefined" ? globalThis : this));
