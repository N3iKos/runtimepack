const assert = require("assert");
const fs = require("fs");
const path = require("path");
const vm = require("vm");

console.log("Running Mobile Web Shims Unit Tests...");

const shimsPath = path.resolve(__dirname, "../../main/assets/web_shims/runtimepack_web_shims.js");

assert(fs.existsSync(shimsPath), `Shims file must exist at ${shimsPath}`);
const shimsCode = fs.readFileSync(shimsPath, "utf-8");

function createSandbox(initialStorageManager) {
    const warned = [];
    const sandbox = {
        console: {
            log: console.log,
            warn: (msg) => { warned.push(msg); },
            error: console.error,
            info: console.info
        },
        _warned: warned,
        btoa: (str) => Buffer.from(str, "binary").toString("base64"),
        atob: (b64) => Buffer.from(b64, "base64").toString("binary")
    };
    sandbox.window = sandbox;
    sandbox.global = sandbox;
    sandbox.globalThis = sandbox;

    if (initialStorageManager !== undefined) {
        sandbox.StorageManager = initialStorageManager;
    }

    vm.createContext(sandbox);
    return sandbox;
}

{
    const sb = createSandbox();
    vm.runInContext(shimsCode, sb);

    const pathMod = vm.runInContext("require('path')", sb);
    assert(pathMod, "require('path') must return a module object");
    assert.strictEqual(pathMod.sep, "/", "path.sep must be '/'");
    assert.strictEqual(pathMod.delimiter, ":", "path.delimiter must be ':'");

    const joined = vm.runInContext("require('path').join('www', 'data', 'System.json')", sb);
    assert.strictEqual(joined, "www/data/System.json", "require('path').join must return 'www/data/System.json'");

    assert.strictEqual(pathMod.join("a", "b", "c"), "a/b/c");
    assert.strictEqual(pathMod.join("a/", "/b/", "/c"), "a/b/c");
    assert.strictEqual(pathMod.join("a", "..", "b"), "b");

    assert.strictEqual(pathMod.normalize("foo/bar/../baz"), "foo/baz");
    assert.strictEqual(pathMod.normalize("foo/./bar///baz"), "foo/bar/baz");

    assert.strictEqual(pathMod.dirname("www/data/System.json"), "www/data");
    assert.strictEqual(pathMod.dirname("System.json"), ".");
    assert.strictEqual(pathMod.dirname("/System.json"), "/");

    assert.strictEqual(pathMod.basename("www/data/System.json"), "System.json");
    assert.strictEqual(pathMod.basename("www/data/System.json", ".json"), "System");

    assert.strictEqual(pathMod.extname("www/data/System.json"), ".json");
    assert.strictEqual(pathMod.extname("System"), "");

    assert.strictEqual(pathMod.resolve("foo", "bar"), "/foo/bar");
}

{
    const sb = createSandbox();
    vm.runInContext(shimsCode, sb);

    const nwGui = vm.runInContext("require('nw.gui')", sb);
    const nwMod = vm.runInContext("require('nw')", sb);
    const globalNw = vm.runInContext("nw", sb);

    assert(nwGui, "require('nw.gui') must return an object");
    assert(nwMod, "require('nw') must return an object");
    assert(globalNw, "global nw must be defined");
    assert.strictEqual(globalNw, nwGui, "global nw must reference nw.gui module");

    const win = vm.runInContext("nw.Window.get()", sb);
    assert(win, "nw.Window.get() must return window mock");
    assert.strictEqual(win.isFullScreen, false, "win.isFullScreen should default to false");

    assert.doesNotThrow(() => {
        win.enterFullscreen();
        win.leaveFullscreen();
        win.on("close", () => {});
        win.showDevTools();
        win.show();
        win.hide();
        win.focus();
        win.blur();
    }, "nw.Window methods must not throw");

    assert(nwGui.App, "nw.App must exist");
    assert(typeof nwGui.App.manifest === "object", "nw.App.manifest must be an object");
}

{
    const sb = createSandbox();
    vm.runInContext(shimsCode, sb);

    const fsMod = vm.runInContext("require('fs')", sb);
    assert(fsMod, "require('fs') must return module");
    assert.strictEqual(fsMod.existsSync("test.txt"), false, "fs.existsSync must return false");
    assert.strictEqual(fsMod.readFileSync("test.txt"), "", "fs.readFileSync must return empty string");

    assert.doesNotThrow(() => {
        fsMod.mkdirSync("save");
        fsMod.writeFileSync("save/file.rpgsave", "data");
    }, "fs mutating methods must safely no-op");

    const processObj = vm.runInContext("process", sb);
    assert(processObj, "process global must be defined");
    assert.strictEqual(processObj.mainModule.filename, "index.html");

    const bufferObj = vm.runInContext("Buffer", sb);
    assert(bufferObj, "Buffer global must be defined");
    assert.strictEqual(bufferObj.from("test").toString(), "test");

    const cryptoMod = vm.runInContext("require('crypto')", sb);
    assert(cryptoMod, "require('crypto') must return module");
    assert.doesNotThrow(() => {
        cryptoMod.createCipheriv("aes-256-cbc", bufferObj.from("key"), bufferObj.from("iv"));
        cryptoMod.createDecipheriv("aes-256-cbc", bufferObj.from("key"), bufferObj.from("iv"));
    });
}

{
    const sb = createSandbox();
    vm.runInContext(shimsCode, sb);

    const greenworksInit = vm.runInContext("Greenworks.init()", sb);
    assert.strictEqual(greenworksInit, true, "Greenworks.init() must return true");

    const lowerInit = vm.runInContext("greenworks.init()", sb);
    assert.strictEqual(lowerInit, true, "greenworks.init() must return true");

    assert.strictEqual(vm.runInContext("Greenworks.isSteamRunning()", sb), false);
    assert.strictEqual(vm.runInContext("Greenworks.getScreenLanguage()", sb), "en");

    assert.doesNotThrow(() => {
        vm.runInContext("Greenworks.activateAchievement('ACH_TEST')", sb);
    });

    let playersCount = 0;
    vm.runInContext("Greenworks.getNumberOfPlayers(function(c) { playersCount = c; })", sb);
    assert.strictEqual(sb.playersCount, 1, "Greenworks.getNumberOfPlayers must invoke callback with 1");
}

{
    const initialSM = {
        isLocalMode: function() { return true; }
    };
    const sb = createSandbox(initialSM);
    assert.strictEqual(sb.StorageManager.isLocalMode(), true, "Initially localMode is true");

    vm.runInContext(shimsCode, sb);

    const isLocal = vm.runInContext("StorageManager.isLocalMode()", sb);
    assert.strictEqual(isLocal, false, "StorageManager.isLocalMode() must return false after shim");
}

{
    const sb = createSandbox();
    vm.runInContext(shimsCode, sb);

    vm.runInContext(`
        window.StorageManager = function() {};
        window.StorageManager.isLocalMode = function() { return true; };
    `, sb);

    const isLocal = vm.runInContext("StorageManager.isLocalMode()", sb);
    assert.strictEqual(isLocal, false, "Late-assigned StorageManager.isLocalMode() must be patched to false");
}

{
    const sb = createSandbox();
    const originalEval = sb.eval;
    vm.runInContext(shimsCode, sb);
    assert.strictEqual(sb.__runtimepack_shims_installed, true, "__runtimepack_shims_installed must be true");
    assert.strictEqual(sb.eval, originalEval, "Global eval must remain untouched so plugin direct eval keeps lexical scope");

    assert.doesNotThrow(() => {
        vm.runInContext(shimsCode, sb);
    }, "Re-running shim code must not throw");
    assert.strictEqual(sb.__runtimepack_shims_installed, true);
}

{
    const sb = createSandbox();
    vm.runInContext(shimsCode, sb);

    const unknown = vm.runInContext("require('some_unsupported_native_addon')", sb);
    assert.strictEqual(typeof unknown, "object");
    assert.strictEqual(Object.keys(unknown).length, 0, "Unhandled require must return safe empty object");
    assert(sb._warned.length > 0, "Warning should be logged for unhandled module");
}

{
    const sb = createSandbox();
    vm.runInContext(shimsCode, sb);

    assert(typeof sb.window.executeCheatCmd === "function", "window.executeCheatCmd must be a function");

    // Mock $gameVariables
    let setVarCalled = null;
    sb.$gameVariables = {
        _data: [0, 10, 20],
        setValue: function(id, val) {
            setVarCalled = { id: id, val: val };
        }
    };

    // Mock $gameSwitches
    let setSwitchCalled = null;
    sb.$gameSwitches = {
        _data: [false, true, false],
        setValue: function(id, val) {
            setSwitchCalled = { id: id, val: val };
        }
    };

    // Mock $gameParty
    let gainGoldCalled = null;
    let recoveredActors = 0;
    let gainItemCalled = null;
    sb.$gameParty = {
        gold: function() { return 5000; },
        gainGold: function(amount) { gainGoldCalled = amount; },
        members: function() {
            return [
                { recoverAll: function() { recoveredActors++; } },
                { recoverAll: function() { recoveredActors++; } }
            ];
        },
        gainItem: function(item, amount) { gainItemCalled = { item: item, amount: amount }; }
    };

    // Mock $dataItems, $dataWeapons, $dataArmors
    sb.$dataItems = { 1: { id: 1, name: "Potion" } };
    sb.$dataWeapons = { 2: { id: 2, name: "Iron Sword" } };
    sb.$dataArmors = { 3: { id: 3, name: "Leather Shield" } };

    // Mock RuntimePackWeb
    let recordedCheatState = null;
    let recordedEvalResult = null;
    sb.RuntimePackWeb = {
        setCheatState: function(stateJson) { recordedCheatState = stateJson; },
        setEvalResult: function(resStr) { recordedEvalResult = resStr; }
    };

    // Test set_variable
    sb.window.executeCheatCmd({ action: "set_variable", id: 1, value: 99 });
    assert.deepStrictEqual(setVarCalled, { id: 1, val: 99 }, "executeCheatCmd set_variable should call $gameVariables.setValue");

    // Test set_switch
    sb.window.executeCheatCmd({ action: "set_switch", id: 2, value: 1 });
    assert.deepStrictEqual(setSwitchCalled, { id: 2, val: true }, "executeCheatCmd set_switch should call $gameSwitches.setValue with boolean");

    // Test gain_gold
    sb.window.executeCheatCmd({ action: "gain_gold", amount: 1500 });
    assert.strictEqual(gainGoldCalled, 1500, "executeCheatCmd gain_gold should call $gameParty.gainGold");

    // Test recover_all
    sb.window.executeCheatCmd({ action: "recover_all" });
    assert.strictEqual(recoveredActors, 2, "executeCheatCmd recover_all should call recoverAll on all party members");

    // Test gain_item (item)
    sb.window.executeCheatCmd({ action: "gain_item", id: 1, amount: 5 });
    assert.deepStrictEqual(gainItemCalled, { item: { id: 1, name: "Potion" }, amount: 5 }, "executeCheatCmd gain_item should find $dataItems and call $gameParty.gainItem");

    // Test gain_item (weapon fallback)
    sb.window.executeCheatCmd({ action: "gain_item", id: 2, amount: 1 });
    assert.deepStrictEqual(gainItemCalled, { item: { id: 2, name: "Iron Sword" }, amount: 1 }, "executeCheatCmd gain_item should fallback to $dataWeapons");

    // Test gain_item (armor fallback)
    sb.window.executeCheatCmd({ action: "gain_item", id: 3, amount: 2 });
    assert.deepStrictEqual(gainItemCalled, { item: { id: 3, name: "Leather Shield" }, amount: 2 }, "executeCheatCmd gain_item should fallback to $dataArmors");

    // Test eval
    sb.window.executeCheatCmd({ action: "eval", code: "5 * 6" });
    assert.strictEqual(recordedEvalResult, "30", "executeCheatCmd eval should evaluate code and report to RuntimePackWeb.setEvalResult");

    // Test eval error handling
    sb.window.executeCheatCmd({ action: "eval", code: "throw new Error('test eval error')" });
    assert(recordedEvalResult && recordedEvalResult.startsWith("[CHT-002]"), "executeCheatCmd eval error should report [CHT-002]");

    // Test teleport_map
    let teleportCalled = null;
    sb.$gamePlayer = {
        reserveTransfer: function(mapId, x, y, d, fade) {
            teleportCalled = { mapId, x, y, d, fade };
        },
        x: 10,
        y: 20
    };
    sb.SceneManager = {
        _scene: { constructor: { name: "Scene_Map" } },
        goto: function() {}
    };
    sb.window.executeCheatCmd({ action: "teleport_map", map_id: 7, x: 12, y: 15 });
    assert.deepStrictEqual(teleportCalled, { mapId: 7, x: 12, y: 15, d: 2, fade: 0 }, "executeCheatCmd teleport_map should call reserveTransfer");
    assert(recordedEvalResult && recordedEvalResult.includes("Teleported to Map 7"), "teleport_map should report status to RuntimePackWeb");

    // Test export_bug_report
    sb.$gameMap = { mapId: function() { return 7; } };
    sb.window.executeCheatCmd({ action: "export_bug_report" });
    assert(sb.__runtimepack_bug_report, "export_bug_report should set __runtimepack_bug_report");
    assert.strictEqual(sb.__runtimepack_bug_report.scene, "Scene_Map");
    assert.strictEqual(sb.__runtimepack_bug_report.map_id, 7);
    assert.strictEqual(sb.__runtimepack_bug_report.player.x, 10);
    assert.strictEqual(sb.__runtimepack_bug_report.player.y, 20);

    // Test run_stress_test
    sb.$dataMap = {
        events: {
            1: {
                pages: [
                    {
                        list: [
                            { code: 201, parameters: [0, 999, 0, 0] }, // Broken transfer
                            { code: 231, parameters: [1, "test_pic"] },
                            { code: 241, parameters: [{ name: "test_bgm" }] }
                        ]
                    }
                ]
            }
        }
    };
    sb.window.executeCheatCmd({ action: "run_stress_test" });
    assert(sb.__runtimepack_audit_report, "run_stress_test should generate __runtimepack_audit_report");
    assert.strictEqual(sb.__runtimepack_audit_report.stats.total_maps_scanned, 1);
    assert(sb.__runtimepack_audit_report.broken_transfers.length > 0, "Broken transfer to Map 999 should be detected");
    assert(recordedEvalResult && recordedEvalResult.includes("Stress test completed"), "Stress test should report summary to RuntimePackWeb");

    // Test graceful handling when engine objects are missing
    const emptySb = createSandbox();
    vm.runInContext(shimsCode, emptySb);
    assert.doesNotThrow(() => {
        emptySb.window.executeCheatCmd(null);
        emptySb.window.executeCheatCmd({});
        emptySb.window.executeCheatCmd({ action: "set_variable", id: 1, value: 10 });
        emptySb.window.executeCheatCmd({ action: "set_switch", id: 1, value: true });
        emptySb.window.executeCheatCmd({ action: "gain_gold", amount: 50 });
        emptySb.window.executeCheatCmd({ action: "recover_all" });
        emptySb.window.executeCheatCmd({ action: "gain_item", id: 1, amount: 1 });
        emptySb.window.executeCheatCmd({ action: "dump_state" });
        emptySb.window.executeCheatCmd({ action: "teleport_map", map_id: 1, x: 0, y: 0 });
        emptySb.window.executeCheatCmd({ action: "export_bug_report" });
        emptySb.window.executeCheatCmd({ action: "run_stress_test" });
    }, "executeCheatCmd must safely no-op when game globals are undefined");
}

{
    // Web Asset Audit Instrumentation Tests (Image, Audio, Video loaders)
    const auditShimsPath = path.resolve(__dirname, "../../main/assets/web_shims/runtimepack_asset_audit.js");
    assert(fs.existsSync(auditShimsPath), `Audit shims file must exist at ${auditShimsPath}`);
    const auditCode = fs.readFileSync(auditShimsPath, "utf-8");

    const sb = createSandbox();
    const postedEvents = [];
    sb.RuntimePackWeb = {
        postAuditEvent: function(eventJson) {
            const ev = typeof eventJson === "string" ? JSON.parse(eventJson) : eventJson;
            postedEvents.push(ev);
            return true;
        }
    };

    // Load main shims and audit shims into sandbox
    vm.runInContext(shimsCode, sb);
    vm.runInContext(auditCode, sb);

    // Mock ImageManager
    sb.ImageManager = {
        loadBitmap: function(folder, filename, hue, smooth) {
            const bmp = {
                _url: folder + filename + ".png",
                _isLoading: true,
                _errorListeners: [],
                addLoadListener: function(fn) {},
                addErrorListener: function(fn) {
                    this._errorListeners.push(fn);
                },
                triggerError: function() {
                    this._errorListeners.forEach(fn => fn(this));
                }
            };
            return bmp;
        }
    };

    // Mock AudioManager
    sb.AudioManager = {
        playBgm: function(bgm, pos) {
            throw new Error("Failed to decode audio stream: " + (bgm ? bgm.name : "null"));
        },
        playSe: function(se) {
            throw new Error("Audio file not found: " + (se ? se.name : "null"));
        }
    };

    // Mock Graphics
    sb.Graphics = {
        playVideo: function(src) {
            if (this._videoErrorCb) {
                this._videoErrorCb(new Error("Video format unsupported: " + src));
            }
        },
        _videoErrorCb: null
    };

    // Install asset audit hooks in sandbox
    vm.runInContext("if (window.RuntimePackAssetAudit) { window.RuntimePackAssetAudit.installHooks(window); }", sb);

    // 1. Test ImageManager.loadBitmap error interception
    const bmp = sb.ImageManager.loadBitmap("img/characters/", "MissingHero", 0, true);
    assert(bmp, "loadBitmap should return a bitmap instance");
    bmp.triggerError();

    const imgEvent = postedEvents.find(e => e.asset_path && e.asset_path.includes("MissingHero"));
    assert(imgEvent, "Error in loadBitmap should emit an audit event to RuntimePackWeb");
    assert.strictEqual(imgEvent.stage, "FILE_EXISTS");
    assert.strictEqual(imgEvent.outcome, "FAIL");
    assert.strictEqual(imgEvent.fallback_action, "PLACEHOLDER_SUBSTITUTED");
    assert.strictEqual(imgEvent.source_location, "ImageManager.loadBitmap");

    // 2. Test AudioManager.playBgm failure suppression and audit emission
    assert.doesNotThrow(() => {
        sb.AudioManager.playBgm({ name: "MissingTheme", volume: 90, pitch: 100, pan: 0 });
    }, "playBgm error should be safely caught");

    const bgmEvent = postedEvents.find(e => e.asset_path && e.asset_path.includes("MissingTheme"));
    assert(bgmEvent, "playBgm failure should emit an audit event");
    assert.strictEqual(bgmEvent.outcome, "FAIL");
    assert.strictEqual(bgmEvent.fallback_action, "AUDIO_LOAD_SUPPRESSED");
    assert.strictEqual(bgmEvent.source_location, "AudioManager.playBgm");

    // 3. Test AudioManager.playSe failure suppression and audit emission
    assert.doesNotThrow(() => {
        sb.AudioManager.playSe({ name: "MissingSound", volume: 80, pitch: 100, pan: 0 });
    }, "playSe error should be safely caught");

    const seEvent = postedEvents.find(e => e.asset_path && e.asset_path.includes("MissingSound"));
    assert(seEvent, "playSe failure should emit an audit event");
    assert.strictEqual(seEvent.outcome, "FAIL");
    assert.strictEqual(seEvent.fallback_action, "AUDIO_LOAD_SUPPRESSED");
    assert.strictEqual(seEvent.source_location, "AudioManager.playSe");

    // 4. Test Graphics.playVideo error interception
    assert.doesNotThrow(() => {
        sb.Graphics.playVideo("movies/Opening.webm");
    });

    // 5. Test Asynchronous WebAudio._onError and _onXhrLoad
    sb.WebAudio = function() {
        this._url = "audio/bgm/AsyncTheme.ogg";
    };
    sb.WebAudio.prototype._onError = function() {};
    sb.WebAudio.prototype._onXhrLoad = function(xhr) {};

    sb.Html5Audio = {
        _url: "audio/se/AsyncHtml5.ogg",
        _onError: function() {}
    };

    // Re-install hooks for newly defined audio/video objects
    vm.runInContext("window.RuntimePackAssetAudit.installHooks(window);", sb);

    const waInstance = new sb.WebAudio();
    waInstance._onError();
    const waErrorEvent = postedEvents.find(e => e.asset_path && e.asset_path.includes("AsyncTheme.ogg"));
    assert(waErrorEvent, "WebAudio._onError should emit an audit event");
    assert.strictEqual(waErrorEvent.fallback_action, "AUDIO_LOAD_SUPPRESSED");
    assert.strictEqual(waErrorEvent.outcome, "FAIL");

    waInstance._url = "audio/bgm/Http404.ogg";
    waInstance._onXhrLoad({ status: 404 });
    const wa404Event = postedEvents.find(e => e.asset_path && e.asset_path.includes("Http404.ogg"));
    assert(wa404Event, "WebAudio._onXhrLoad HTTP 404 should emit an audit event");
    assert.strictEqual(wa404Event.fallback_action, "AUDIO_LOAD_SUPPRESSED");

    // 6. Test Asynchronous Html5Audio._onError
    sb.Html5Audio._onError();
    const html5Event = postedEvents.find(e => e.asset_path && e.asset_path.includes("AsyncHtml5.ogg"));
    assert(html5Event, "Html5Audio._onError should emit an audit event");
    assert.strictEqual(html5Event.fallback_action, "AUDIO_LOAD_SUPPRESSED");

    // 7. Test Asynchronous Graphics._onVideoError
    sb.Graphics._onVideoError = function() {};
    sb.Graphics._video = { src: "movies/AsyncIntro.webm" };
    vm.runInContext("window.RuntimePackAssetAudit.installHooks(window);", sb);

    sb.Graphics._onVideoError();
    const videoEvent = postedEvents.find(e => e.asset_path && e.asset_path.includes("AsyncIntro.webm"));
    assert(videoEvent, "Graphics._onVideoError should emit a VIDEO_PLAY_SUPPRESSED audit event");
    assert.strictEqual(videoEvent.fallback_action, "VIDEO_PLAY_SUPPRESSED");
    assert.strictEqual(videoEvent.outcome, "FAIL");
}

{
    const sb = createSandbox();
    sb.TYRANO = {
        kag: {
            tmp: {},
            cache_scenario: {},
            loadScenario: function(file_name, cb) {
                throw new Error("Failed to load: " + file_name);
            }
        }
    };
    vm.runInContext(shimsCode, sb);

    assert(sb.TYRANO.kag.loadScenario.__rp_patched, "loadScenario should be patched");

    let callbackResult = null;
    sb.TYRANO.kag.loadScenario("cg.ks", function(parsed) {
        callbackResult = parsed;
    });

    assert(callbackResult, "Fallback scenario object should be provided for optional stub cg.ks");
    assert(Array.isArray(callbackResult.array_s), "array_s should exist in fallback parsed scenario");
    assert(callbackResult.array_s.some(tag => tag.name === "return"), "fallback scenario should have return tag");
    assert(callbackResult.array_s.some(tag => tag.name === "jump"), "fallback scenario should have jump tag");
}

{
    // RPG Maker MZ screen fit: Graphics._stretchHeight and _stretchWidth 100% scale
    const sb = createSandbox();
    sb.window.innerHeight = 720;
    sb.window.innerWidth = 1280;
    sb.document = {
        documentElement: {
            clientHeight: 720,
            clientWidth: 1280
        }
    };

    // Simulate RPG Maker MZ Graphics with 0.9 mobile scaling penalty
    sb.Graphics = {
        _width: 816,
        _height: 624,
        _stretchEnabled: true,
        _stretchWidth: function() {
            return sb.document.documentElement.clientWidth;
        },
        _stretchHeight: function() {
            // Unpatched MZ mobile behavior with 0.9 penalty
            const rate = 0.9;
            return sb.document.documentElement.clientHeight * rate;
        },
        _updateRealScale: function() {
            if (this._stretchEnabled && this._width > 0 && this._height > 0) {
                const h = this._stretchWidth() / this._width;
                const v = this._stretchHeight() / this._height;
                this._realScale = Math.min(h, v);
            }
        }
    };

    assert.strictEqual(sb.Graphics._stretchHeight(), 648, "Precondition: unpatched height is 648 (0.9 penalty)");

    vm.runInContext(shimsCode, sb);

    assert(sb.Graphics._stretchHeight.__rp_fit_patched, "Graphics._stretchHeight should have __rp_fit_patched flag");
    assert(sb.Graphics._stretchWidth.__rp_fit_patched, "Graphics._stretchWidth should have __rp_fit_patched flag");

    assert.strictEqual(sb.Graphics._stretchHeight(), 720, "Graphics._stretchHeight must return full 100% viewport height (720), not 648");
    assert.strictEqual(sb.Graphics._stretchWidth(), 1280, "Graphics._stretchWidth must return full 100% viewport width (1280)");

    sb.Graphics._updateRealScale();
    const expectedV = 720 / 624;
    assert.strictEqual(sb.Graphics._realScale, expectedV, "Graphics._realScale should scale to full 100% height");

    // Also verify late-assigned Graphics gets patched
    const sbLate = createSandbox();
    sbLate.window.innerHeight = 1080;
    sbLate.window.innerWidth = 1920;
    vm.runInContext(shimsCode, sbLate);

    sbLate.Graphics = {
        _stretchHeight: function() { return 1080 * 0.9; },
        _stretchWidth: function() { return 1920; }
    };
    // Trigger patch or property setter
    if (typeof sbLate.Graphics._stretchHeight === "function" && !sbLate.Graphics._stretchHeight.__rp_fit_patched) {
        if (typeof sbLate.window.patchGraphics === "function") {
            sbLate.window.patchGraphics(sbLate.Graphics);
        }
    }
    assert.strictEqual(sbLate.Graphics._stretchHeight(), 1080, "Late assigned Graphics._stretchHeight must return 1080");
}

{
    const sb = createSandbox();
    sb.RuntimePackNative = {
        readdir: function(dir) {
            if (dir === "data" || dir === "data/") {
                return JSON.stringify(["System.json", "Map001.json", "Map002.json"]);
            }
            if (dir === "img/pictures") {
                return JSON.stringify(["cg1.png", "cg2.png"]);
            }
            return "[]";
        }
    };
    vm.runInContext(shimsCode, sb);

    const fs = vm.runInContext("require('fs')", sb);
    const dataFiles = fs.readdirSync("data");
    assert(Array.isArray(dataFiles), "readdirSync must return an array");
    assert.strictEqual(dataFiles.length, 3, "readdirSync('data') must return 3 files");
    assert.strictEqual(dataFiles[0], "System.json");

    const pictures = fs.readdirSync("img/pictures");
    assert.strictEqual(pictures.length, 2);
    assert.strictEqual(pictures[1], "cg2.png");
}

{
    // Test: RPG Maker MV Universal Save Adapter
    const sb = createSandbox();
    const mvSlots = {};

    sb.RuntimePackNative = {
        saveSlot: function(ns, slot, payload) {
            if (ns !== "mv") return false;
            mvSlots[slot] = payload;
            return true;
        },
        loadSlot: function(ns, slot) {
            if (ns !== "mv") return null;
            return mvSlots[slot] !== undefined ? mvSlots[slot] : null;
        },
        slotExists: function(ns, slot) {
            return ns === "mv" && mvSlots[slot] !== undefined;
        },
        deleteSlot: function(ns, slot) {
            if (ns === "mv" && mvSlots[slot] !== undefined) {
                delete mvSlots[slot];
                return true;
            }
            return false;
        },
        listSlots: function(ns) {
            if (ns !== "mv") return "[]";
            return JSON.stringify(Object.keys(mvSlots));
        }
    };

    // Minimal mock of LZString
    sb.LZString = {
        compressToBase64: function(s) { return "lzb64:" + s; },
        decompressFromBase64: function(s) { return s && s.startsWith("lzb64:") ? s.substring(6) : s; }
    };

    // Mock MV StorageManager before shims initialization
    sb.StorageManager = {
        saveToWebStorage: function() {},
        loadFromWebStorage: function() {},
        webStorageExists: function() {},
        removeWebStorage: function() {},
        backup: function() {},
        backupExists: function() {},
        cleanBackup: function() {},
        restoreBackup: function() {},
        loadFromWebStorageBackup: function() {}
    };

    vm.runInContext(shimsCode, sb);

    const sm = sb.StorageManager;

    // 1. Save and Load slot 1
    sm.saveToWebStorage(1, "{\"party\":[\"Actor1\"]}");
    assert.strictEqual(sm.webStorageExists(1), true, "Slot 1 must exist");
    const loadedJson = sm.loadFromWebStorage(1);
    assert.strictEqual(loadedJson, "{\"party\":[\"Actor1\"]}", "Loaded slot 1 JSON must match original");

    // Check underlying mapped slot in native repository
    assert.strictEqual(mvSlots["file1"], "lzb64:{\"party\":[\"Actor1\"]}", "Native slot must be mapped to 'file1'");

    // 2. Backup and restore
    sm.backup(1);
    assert.strictEqual(sm.backupExists(1), true, "Backup for slot 1 must exist");
    assert.strictEqual(mvSlots["file1.bak"], "lzb64:{\"party\":[\"Actor1\"]}", "Backup must be saved as file1.bak");

    // Overwrite slot 1 with corrupt data
    sm.saveToWebStorage(1, "corrupt");
    assert.strictEqual(sm.loadFromWebStorage(1), "corrupt");

    // Restore backup
    sm.restoreBackup(1);
    assert.strictEqual(sm.loadFromWebStorage(1), "{\"party\":[\"Actor1\"]}", "Restored slot 1 must match backup");
    assert.strictEqual(sm.backupExists(1), false, "Backup must be cleaned up after restore");

    // 3. Config and Global slots
    sm.saveToWebStorage(-1, "{\"volume\":80}");
    sm.saveToWebStorage(0, "[null,\"info1\"]");
    assert.strictEqual(mvSlots["config"], "lzb64:{\"volume\":80}");
    assert.strictEqual(mvSlots["global"], "lzb64:[null,\"info1\"]");
    assert.strictEqual(sm.loadFromWebStorage(-1), "{\"volume\":80}");
    assert.strictEqual(sm.loadFromWebStorage(0), "[null,\"info1\"]");
}

{
    // Test: RPG Maker MZ Universal Save Adapter
    const sb = createSandbox();
    const storedSlots = {};

    sb.RuntimePackNative = {
        saveSlot: function(ns, slot, payload) {
            if (ns !== "mz") return false;
            storedSlots[slot] = payload;
            return true;
        },
        loadSlot: function(ns, slot) {
            if (ns !== "mz") return null;
            return storedSlots[slot] !== undefined ? storedSlots[slot] : null;
        },
        slotExists: function(ns, slot) {
            return ns === "mz" && storedSlots[slot] !== undefined;
        },
        deleteSlot: function(ns, slot) {
            if (ns === "mz" && storedSlots[slot] !== undefined) {
                delete storedSlots[slot];
                return true;
            }
            return false;
        },
        listSlots: function(ns) {
            if (ns !== "mz") return "[]";
            return JSON.stringify(Object.keys(storedSlots));
        }
    };

    // Mock MZ StorageManager before shims initialization
    sb.StorageManager = {
        _forageKeys: [],
        _forageKeysUpdated: false,
        forageKey: function(name) { return "rmmzsave.test." + name; },
        forageTestKey: function() { return "rmmzsave.test"; },
        saveToForage: function() {},
        loadFromForage: function() {},
        forageExists: function(name) { return this._forageKeys.includes(this.forageKey(name)); },
        removeForage: function() {},
        updateForageKeys: function() {},
        forageKeysUpdated: function() { return this._forageKeysUpdated; }
    };

    vm.runInContext(shimsCode, sb);

    // 1. Verify updateForageKeys queries native listSlots
    storedSlots["global"] = Buffer.from("mz_global_data").toString("base64");
    storedSlots["file1"] = Buffer.from("mz_binary_\x00\x80\xFF_data", "binary").toString("base64");

    const sm = sb.StorageManager;
    sm.updateForageKeys().then(function() {
        assert.strictEqual(sm.forageKeysUpdated(), true, "forageKeysUpdated must be true after update");
        assert.strictEqual(sm.forageExists("global"), true, "forageExists('global') must be true");
        assert.strictEqual(sm.forageExists("file1"), true, "forageExists('file1') must be true");
        assert.strictEqual(sm.forageExists("file2"), false, "forageExists('file2') must be false");

        // 2. Verify loadFromForage decodes Base64 preserving binary bytes
        return sm.loadFromForage("file1");
    }).then(function(rawZip) {
        assert.strictEqual(rawZip, "mz_binary_\x00\x80\xFF_data", "loadFromForage must decode raw binary string exactly");

        // 3. Verify saveToForage encodes binary to Base64 and persists to native
        return sm.saveToForage("file2", "new_save_\x01\x02\xFE");
    }).then(function() {
        assert(storedSlots["file2"], "file2 must be stored in native bridge");
        const decoded = Buffer.from(storedSlots["file2"], "base64").toString("binary");
        assert.strictEqual(decoded, "new_save_\x01\x02\xFE", "Persisted payload must match original binary bytes");
        assert.strictEqual(sm.forageExists("file2"), true, "forageExists must report true for newly saved slot");

        // 4. Verify binary strings with UTF-8 lookalike bytes maintain byte length and equality without being mutated
        const binaryPayload = "mz_save_\xC2\xA0\x00\x80\xFF_bytes";
        return sm.saveToForage("file3", binaryPayload).then(function() {
            return sm.loadFromForage("file3");
        }).then(function(loadedPayload) {
            assert.strictEqual(loadedPayload.length, binaryPayload.length, "Binary payload length must remain identical");
            assert.strictEqual(loadedPayload, binaryPayload, "Binary payload with UTF-8 lookalike bytes must not be mutated");
            return sm.removeForage("file3");
        });
    }).then(function() {
        // 5. Verify removeForage removes from native
        return sm.removeForage("file2");
    }).then(function() {
        assert.strictEqual(storedSlots["file2"], undefined, "file2 must be deleted from native repository");
        assert.strictEqual(sm.forageExists("file2"), false, "forageExists must be false after deletion");
        console.log("All Mobile Web Shims tests passed successfully!");
    }).catch(function(err) {
        console.error("Test failed in MZ storage promise chain:", err);
        process.exit(1);
    });
}

// ============================================================
// Engine Input Patch (Touch Ok + moveThreshold + codeMapper + RuntimePackInput)
// ============================================================
{
    const sb = createSandbox();
    // Mock engine MZ yang dimuat SETELAH shim dieksekusi (late binding)
    vm.runInContext(shimsCode, sb);

    vm.runInContext(`
        window.Input = {
            keyMapper: { 90: "ok", 13: "ok", 27: "escape", 37: "left", 38: "up" },
            _currentState: {},
            _latestButton: null,
            _pressedTime: -1,
            _date: 0,
            _onKeyDown: function(event) {
                var buttonName = this.keyMapper[event.keyCode];
                if (!buttonName && event.code && this.codeMapper) {
                    buttonName = this.codeMapper[event.code];
                }
                if (buttonName) {
                    this._currentState[buttonName] = true;
                }
            },
            _onKeyUp: function(event) {
                var buttonName = this.keyMapper[event.keyCode];
                if (!buttonName && event.code && this.codeMapper) {
                    buttonName = this.codeMapper[event.code];
                }
                if (buttonName) {
                    this._currentState[buttonName] = false;
                }
            }
        };
        window.TouchInput = { moveThreshold: 10 };
        function Window_SelectableMock() { this._doubleTouch = false; }
        Window_SelectableMock.prototype.isOkEnabled = function() { return true; };
        Window_SelectableMock.prototype.isTouchOkEnabled = function() {
            return this.isOkEnabled() && (this._doubleTouch);
        };
        window.Window_Selectable = Window_SelectableMock;
        function Window_SavefileListMock() { this._doubleTouch = false; }
        Window_SavefileListMock.prototype = Object.create(Window_SelectableMock.prototype);
        Window_SavefileListMock.prototype.constructor = Window_SavefileListMock;
        window.Window_SavefileList = Window_SavefileListMock;
        // Trigger pollPatches ulang manual (meniru interval di WebView)
        if (typeof window.__runtimepackPollEngineInput === "function") {
            window.__runtimepackPollEngineInput();
        }
    `, sb);

    // 1. moveThreshold dinaikkan dari 10 -> 24
    const threshold = vm.runInContext("window.TouchInput.moveThreshold", sb);
    assert.strictEqual(threshold, 24, "TouchInput.moveThreshold must be raised to 24");

    // 2. Single-tap OK: isTouchOkEnabled harus true TANPA _doubleTouch
    const touchOk = vm.runInContext(
        "var w = new Window_SavefileList(); w.isTouchOkEnabled()", sb
    );
    assert.strictEqual(touchOk, true,
        "Window_SavefileList.isTouchOkEnabled must return true on single tap (no doubleTouch)");

    // 3. Window_Selectable umum juga single-tap (mode non-NWjs/mobile)
    const genericOk = vm.runInContext(
        "var g = new Window_Selectable(); g.isTouchOkEnabled()", sb
    );
    assert.strictEqual(genericOk, true,
        "Window_Selectable.isTouchOkEnabled must return true in non-NWjs (mobile) environment");

    // 4. Input.codeMapper fallback: event keyCode 0 + code 'KeyZ' -> action 'ok'
    vm.runInContext(`
        var fakeEvent = { keyCode: 0, code: "KeyZ" };
        window.Input._onKeyDown(fakeEvent);
    `, sb);
    const okState = vm.runInContext("window.Input._currentState['ok']", sb);
    assert.strictEqual(okState, true,
        "Input._onKeyDown must map event.code 'KeyZ' to action 'ok' when keyCode is 0");

    vm.runInContext(`
        var fakeUp = { keyCode: 0, code: "KeyZ" };
        window.Input._onKeyUp(fakeUp);
    `, sb);
    const okStateAfter = vm.runInContext("window.Input._currentState['ok']", sb);
    assert.strictEqual(okStateAfter, false,
        "Input._onKeyUp must release action 'ok' via event.code mapping");

    // 5. RuntimePackInput API
    assert(vm.runInContext("typeof window.RuntimePackInput === 'object'", sb),
        "window.RuntimePackInput must be defined");
    vm.runInContext("window.RuntimePackInput.press('escape')", sb);
    assert.strictEqual(vm.runInContext("window.Input._currentState['escape']", sb), true,
        "RuntimePackInput.press('escape') must set Input._currentState.escape = true");
    vm.runInContext("window.RuntimePackInput.release('escape')", sb);
    assert.strictEqual(vm.runInContext("window.Input._currentState['escape']", sb), false,
        "RuntimePackInput.release('escape') must set Input._currentState.escape = false");
    assert.strictEqual(vm.runInContext("window.RuntimePackInput.isAvailable()", sb), true,
        "RuntimePackInput.isAvailable() must return true when Input is present");

    console.log("Engine Input Patch tests PASSED");
}

{
    const sb = createSandbox();
    vm.runInContext(shimsCode, sb);

    vm.runInContext(`
        function Window_BaseMock() {}
        Window_BaseMock.prototype.convertEscapeCharacters = function(text) {
            return text.replace(/\\\\V\\[1\\]/g, "Var1Value");
        };
        window.Window_Base = Window_BaseMock;

        function Sprite_ButtonMock() {}
        window.Sprite_Button = Sprite_ButtonMock;

        if (typeof window.__runtimepackPollHalfMoveCompat === "function") {
            window.__runtimepackPollHalfMoveCompat();
        }
    `, sb);

    assert(vm.runInContext("typeof window.Sprite_Button.prototype.convertEscapeCharacters === 'function'", sb),
        "Sprite_Button.prototype.convertEscapeCharacters must be a function after patch");

    vm.runInContext(`
        var button1 = new window.Sprite_Button();
        var helpWin = new window.Window_Base();
        window.SceneManager = {
            _scene: {
                _helpWindow: helpWin,
                _windowLayer: { children: [button1, helpWin] }
            }
        };
    `, sb);

    const resHelp = vm.runInContext("window.SceneManager._scene._windowLayer.children[0].convertEscapeCharacters('\\\\V[1]')", sb);
    assert.strictEqual(resHelp, "Var1Value", "convertEscapeCharacters on children[0] (Sprite_Button) must delegate to _helpWindow");

    vm.runInContext(`
        var button2 = new window.Sprite_Button();
        var listWin = new window.Window_Base();
        window.SceneManager = {
            _scene: {
                _helpWindow: null,
                _windowLayer: { children: [button2, listWin] }
            }
        };
    `, sb);

    const resLayer = vm.runInContext("window.SceneManager._scene._windowLayer.children[0].convertEscapeCharacters('\\\\V[1]')", sb);
    assert.strictEqual(resLayer, "Var1Value", "convertEscapeCharacters on children[0] must delegate to first Window_Base child when _helpWindow is null");

    vm.runInContext(`
        var button3 = new window.Sprite_Button();
        window.SceneManager = { _scene: null };
    `, sb);

    const resFallback = vm.runInContext("button3.convertEscapeCharacters('\\\\V[1]')", sb);
    assert.strictEqual(resFallback, "Var1Value", "convertEscapeCharacters on Sprite_Button must fall back to Window_Base prototype when scene has no windows");

    vm.runInContext(`
        var pcWin = new window.Window_Base();
        window.SceneManager = {
            _scene: {
                _windowLayer: { children: [pcWin] }
            }
        };
    `, sb);

    const resPc = vm.runInContext("window.SceneManager._scene._windowLayer.children[0].convertEscapeCharacters('\\\\V[1]')", sb);
    assert.strictEqual(resPc, "Var1Value", "convertEscapeCharacters on Window_Base children[0] must function unmodified for PC parity");

    console.log("HalfMove Compat tests PASSED");
}
