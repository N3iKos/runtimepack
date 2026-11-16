const assert = require("assert");
const fs = require("fs");
const path = require("path");
const vm = require("vm");

console.log("Running Mobile Web Shims Unit Tests...");

const shimsPath = path.resolve(__dirname, "../../main/assets/web_shims/runtimepack_web_shims.js");

assert(fs.existsSync(shimsPath), `Shims file must exist at ${shimsPath}`);
const shimsCode = fs.readFileSync(shimsPath, "utf-8");

// Production trailer source of truth. The core-completion trailer lives in Kotlin as a
// `+`-joined string literal chain, so it is parsed here instead of being hand-copied into
// the test (a hand-copied fence can drift from what the interceptor actually serves).
const interceptorKotlinPath = path.resolve(__dirname, "../../main/java/io/runtimepack/host/VfsResourceInterceptor.kt");

function extractCompletionTrailer(sourceText) {
    const source = typeof sourceText === "string" ? sourceText : fs.readFileSync(interceptorKotlinPath, "utf-8");
    const marker = "val SCRIPT_COMPLETION_TRAILER: String =";
    const start = source.indexOf(marker);
    assert(start >= 0, `Kotlin SCRIPT_COMPLETION_TRAILER declaration must exist in ${interceptorKotlinPath}`);
    const afterMarker = source.slice(start + marker.length);
    // Bounded: the declaration ends at the next top-level Kotlin declaration/annotation/brace.
    const endMatch = afterMarker.search(/\n\s*(?:private|public|internal|val|var|fun|@|\})/);
    const block = endMatch >= 0 ? afterMarker.slice(0, endMatch) : afterMarker;
    const literals = [];
    const literalRegex = /"((?:[^"\\]|\\.)*)"/g;
    let match;
    while ((match = literalRegex.exec(block)) !== null) {
        literals.push(match[1]);
    }
    assert(literals.length > 0, "Kotlin trailer block must contain at least one string literal");
    const trailer = literals.join("").replace(/\\(.)/g, (_, ch) => (ch === "n" ? "\n" : ch === "t" ? "\t" : ch));
    assert(trailer.includes("applyPlatformIdentity"), "Extracted trailer must reference applyPlatformIdentity");
    assert(trailer.includes("Utils"), "Extracted trailer must reference Utils");
    assert(trailer.includes("Object.defineProperty"),
        "Extracted trailer must contain the inline defineProperty fallback");
    assert(trailer.includes("__runtimepack_pendingIdentity"),
        "Extracted trailer must contain the pending identity handoff record");
    return trailer;
}

const trailerCode = extractCompletionTrailer();

// Extractor guard self-check: a trailer that loses the inline fallback or the pending handoff
// must fail inside the extractor (loud harness error), not only as a downstream behaviour
// assertion. Mutations are applied in memory; the real file is untouched.
{
    const realSource = fs.readFileSync(interceptorKotlinPath, "utf-8");
    const mutations = [
        ["Object.defineProperty", "ObjectDefinePropertyRemoved"],
        ["__runtimepack_pendingIdentity", "__rpPendingHandoffRemoved"]
    ];
    for (const [needle, replacement] of mutations) {
        const mutated = realSource.split(needle).join(replacement);
        assert.notStrictEqual(mutated, realSource, `Extractor guard self-check must be able to remove ${needle}`);
        assert.throws(() => extractCompletionTrailer(mutated),
            /Extracted trailer must contain/,
            `Extractor must reject a trailer missing ${needle}`);
    }
}

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
    const sb = createSandbox();
    sb.RuntimePackNative = {
        fileExists: function(path) { return path === "audio/bgm/Theme.ogg" || path === "gameenv.iz"; },
        fileSize: function(path) { return path === "audio/bgm/Theme.ogg" ? 5242880 : 100; }
    };
    vm.runInContext(shimsCode, sb);

    const fs = vm.runInContext("require('fs')", sb);
    assert.strictEqual(fs.existsSync("audio/bgm/Theme.ogg"), true);
    assert.strictEqual(fs.existsSync("audio/bgm/Missing.ogg"), false);
    assert.strictEqual(fs.existsSync("gameenv.iz"), false, "gameenv.iz must always return false to allow plaintext gameenv.json fallback");

    const stat = fs.statSync("audio/bgm/Theme.ogg");
    assert.strictEqual(stat.isFile(), true);
    assert.strictEqual(stat.isDirectory(), false);
    assert.strictEqual(stat.size, 5242880);
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

// ============================================================
// Load Failure Diagnostics (fail-loud RT-LOAD-FAIL)
// ============================================================
{
    const sb = createSandbox();
    vm.runInContext(shimsCode, sb);
    // Capture console.error
    const errors = [];
    vm.runInContext("this.__capturedErrors = [];", sb);
    sb.console.error = (...args) => { errors.push(args.map(String).join(" ")); };
    vm.runInContext(`
        this.__origFailureCalled = false;
        window.Scene_Load = function() {};
        window.Scene_Load.prototype.onLoadFailure = function() {
            window.__origFailureCalled = true;
        };
        if (typeof window.__runtimepackPollLoadDiagnostics === "function") {
            window.__runtimepackPollLoadDiagnostics();
        }
    `, sb);
    const result = vm.runInContext(`
        var s = new window.Scene_Load();
        s.onLoadFailure();
        JSON.stringify({ orig: window.__origFailureCalled });
    `, sb);
    assert.strictEqual(JSON.parse(result).orig, true, "original onLoadFailure must still run");
    assert(errors.some(e => e.indexOf("[RuntimePack] RT-LOAD-FAIL") !== -1),
        "onLoadFailure must log [RuntimePack] RT-LOAD-FAIL via console.error");
    assert(vm.runInContext("typeof window.__runtimepackShowLoadError === 'function'", sb) === true,
        "fail-loud display helper must be exposed");

    // Test DataManager.loadGame rejection logging and re-throw
    vm.runInContext(`
        window.DataManager = {
            loadGame: function(id) {
                return Promise.reject(new Error("corrupt save data"));
            }
        };
        if (typeof window.__runtimepackPollLoadDiagnostics === "function") {
            window.__runtimepackPollLoadDiagnostics();
        }
    `, sb);
    const dmPromise = vm.runInContext("window.DataManager.loadGame(1)", sb);
    let dmCaught = false;
    dmPromise.catch((err) => {
        dmCaught = true;
        assert.strictEqual(err.message, "corrupt save data", "original error must be preserved on rethrow");
    }).then(() => {
        assert.strictEqual(dmCaught, true, "DataManager.loadGame rejection must be rethrown");
        assert(errors.some(e => e.indexOf("[RuntimePack] RT-LOAD-FAIL loadGame rejection: corrupt save data") !== -1),
            "DataManager.loadGame rejection must log [RuntimePack] RT-LOAD-FAIL loadGame rejection");
    });

    // Test __runtimepackShowLoadError with document overlay
    const overlaySb = createSandbox();
    overlaySb.console.error = () => {};
    let appendedChild = null;
    let removedChild = null;
    overlaySb.document = {
        createElement: (tag) => ({
            tagName: tag,
            style: {},
            textContent: "",
            parentNode: {
                removeChild: (el) => { removedChild = el; }
            }
        }),
        body: {
            appendChild: (el) => { appendedChild = el; }
        }
    };
    vm.runInContext(shimsCode, overlaySb);
    overlaySb.window.__runtimepackShowLoadError("Overlay Test Error");
    assert(appendedChild, "Overlay element must be appended to document.body");
    assert.strictEqual(appendedChild.textContent, "Overlay Test Error");
    assert(appendedChild.style.cssText.includes("position:fixed"), "Overlay must have fixed position style");

    console.log("Load Failure Diagnostics tests PASSED");
}

// ============================================================
// [CAP-WEB-012] Global Error & Unhandled Rejection Logger
// ============================================================
{
    const sb = createSandbox();
    const eventListeners = {};
    sb.window.addEventListener = function(event, handler) {
        eventListeners[event] = handler;
    };
    const errorsCaptured = [];
    sb.console.error = function(...args) {
        errorsCaptured.push(args.map(String).join(" "));
    };

    vm.runInContext(shimsCode, sb);

    assert(typeof eventListeners.error === "function", "window 'error' listener must be registered");
    assert(typeof eventListeners.unhandledrejection === "function", "window 'unhandledrejection' listener must be registered");

    // 1. Trigger global script error
    eventListeners.error({
        message: "Uncaught ReferenceError: foo is not defined",
        filename: "js/plugins/TestPlugin.js",
        lineno: 42,
        colno: 15,
        error: { stack: "ReferenceError: foo is not defined\n at js/plugins/TestPlugin.js:42:15" }
    });

    assert(errorsCaptured.some(e => e.includes("[RuntimePack Global Error]") && e.includes("Uncaught ReferenceError")),
        "Error listener must log [RuntimePack Global Error] with message and details");

    // 2. Trigger unhandled rejection
    eventListeners.unhandledrejection({
        reason: new Error("Network save timed out")
    });

    assert(errorsCaptured.some(e => e.includes("[RuntimePack Unhandled Rejection]") && e.includes("Network save timed out")),
        "Unhandledrejection listener must log [RuntimePack Unhandled Rejection] with reason");

    // 3. Native bridge postDiagnosticError call
    const bridgeErrors = [];
    sb.RuntimePackWeb = {
        postDiagnosticError: function(json) {
            bridgeErrors.push(JSON.parse(json));
            return true;
        }
    };
    eventListeners.error({
        message: "Uncaught SyntaxError: Unexpected token",
        filename: "js/main.js",
        lineno: 10,
        colno: 2,
        error: { stack: "SyntaxError: Unexpected token\n at js/main.js:10:2" }
    });
    assert(bridgeErrors.length === 1, "postDiagnosticError must be called on bridge");
    assert(bridgeErrors[0].message === "Uncaught SyntaxError: Unexpected token", "Error message matches");
    assert(bridgeErrors[0].line === 10, "Line matches");
    assert(bridgeErrors[0].kind === "error", "Kind matches");

    console.log("Global Error & Rejection Logger tests PASSED");
}

// ============================================================
// [CAP-WEB-011] AudioManager Web Audio format enforcement
// ============================================================
{
    const sb = createSandbox();
    sb.AudioManager = {
        audioFileExt: function() { return ".m4a"; }
    };
    sb.Utils = {
        canPlayOgg: function() { return true; }
    };
    sb.WebAudio = {
        canPlayOgg: function() { return true; }
    };

    vm.runInContext(shimsCode, sb);

    assert(sb.AudioManager.audioFileExt.__patched, "AudioManager.audioFileExt should be marked as __patched");
    assert.strictEqual(sb.AudioManager.audioFileExt(), ".ogg", "AudioManager.audioFileExt must return '.ogg' when canPlayOgg is true");

    // Fallback when canPlayOgg returns false
    sb.Utils.canPlayOgg = function() { return false; };
    sb.WebAudio.canPlayOgg = function() { return false; };
    assert.strictEqual(sb.AudioManager.audioFileExt(), ".ogg", "AudioManager.audioFileExt must still enforce '.ogg' as default mobile format");

    console.log("AudioManager audioFileExt tests PASSED");
}

// ============================================================
// [CAP-WEB-013] SceneManager caught exception diagnostics
// ============================================================
{
    const sb = createSandbox();
    let origHandlerInvoked = false;
    let forwardedError = null;
    const errorsCaptured = [];
    sb.console.error = function(...args) {
        errorsCaptured.push(args.map(String).join(" "));
    };

    sb.SceneManager = {
        catchException: function(e) {
            origHandlerInvoked = true;
            forwardedError = e;
            return "orig_handled";
        }
    };

    vm.runInContext(shimsCode, sb);

    assert(sb.SceneManager.catchException.__patched, "SceneManager.catchException must be marked as __patched");

    const testErr = new Error("Scene render failure");
    testErr.stack = "Error: Scene render failure\n at Scene_Map.update";
    const retVal = sb.SceneManager.catchException(testErr);

    assert.strictEqual(origHandlerInvoked, true, "Original SceneManager.catchException must be called");
    assert.strictEqual(forwardedError, testErr, "Original error must be passed to original handler");
    assert.strictEqual(retVal, "orig_handled", "Return value from original catchException must be preserved");
    assert(errorsCaptured.some(e => e.includes("[RuntimePack Caught Error]") && e.includes("Scene render failure")),
        "catchException wrapper must log [RuntimePack Caught Error] with stack");

    console.log("SceneManager.catchException tests PASSED");
}

// ============================================================
// [CAP-WEB-010] Graphics._clearUpperCanvas safety
// ============================================================
{
    const sb = createSandbox();
    let origClearInvoked = false;

    sb.Graphics = {
        _upperCanvas: null,
        _clearUpperCanvas: function() {
            origClearInvoked = true;
            return this._upperCanvas.getContext("2d");
        }
    };

    vm.runInContext(shimsCode, sb);

    assert(sb.Graphics._clearUpperCanvas.__patched, "Graphics._clearUpperCanvas must be marked as __patched");

    // Calling with _upperCanvas == null must safely no-op without TypeError
    assert.doesNotThrow(() => {
        sb.Graphics._clearUpperCanvas();
    }, "Graphics._clearUpperCanvas must not throw when _upperCanvas is null");
    assert.strictEqual(origClearInvoked, false, "Original _clearUpperCanvas must not be called when _upperCanvas is null");

    // Calling with valid _upperCanvas must invoke original method
    sb.Graphics._upperCanvas = {
        getContext: function() { return "ctx2d"; }
    };
    const res = sb.Graphics._clearUpperCanvas();
    assert.strictEqual(origClearInvoked, true, "Original _clearUpperCanvas must be called when _upperCanvas is defined");
    assert.strictEqual(res, "ctx2d", "Original return value must be preserved");

    console.log("Graphics._clearUpperCanvas safety tests PASSED");
}

// ============================================================
// [CAP-WEB-001] fs.readdirSync MapInfos fallback (no native bridge)
// ============================================================
{
    const sb = createSandbox();
    // Simulate XMLHttpRequest returning MapInfos.json
    sb.XMLHttpRequest = function() {
        this._url = null;
        this.status = 0;
        this.responseText = "";
        this.open = function(method, url, async) {
            this._url = url;
        };
        this.send = function() {
            if (this._url && this._url.includes("data/MapInfos.json")) {
                this.status = 200;
                this.responseText = JSON.stringify([
                    null,
                    { id: 1, name: "Intro Map" },
                    { id: 5, name: "Castle" },
                    { id: 12, name: "Dungeon" }
                ]);
            } else {
                this.status = 404;
                this.responseText = "";
            }
        };
    };

    vm.runInContext(shimsCode, sb);

    const fsMod = vm.runInContext("require('fs')", sb);
    const dataFiles = fsMod.readdirSync("data");

    assert(Array.isArray(dataFiles), "readdirSync fallback must return an array");
    assert(dataFiles.includes("System.json"), "Fallback list must include System.json");
    assert(dataFiles.includes("Actors.json"), "Fallback list must include Actors.json");
    assert(dataFiles.includes("MapInfos.json"), "Fallback list must include MapInfos.json");
    assert(dataFiles.includes("Map001.json"), "Fallback list must include Map001.json from MapInfos");
    assert(dataFiles.includes("Map005.json"), "Fallback list must include Map005.json from MapInfos");
    assert(dataFiles.includes("Map012.json"), "Fallback list must include Map012.json from MapInfos");

    console.log("readdirSync MapInfos fallback tests PASSED");
}

// ============================================================
// [CAP-WEB-014] TyranoBuilder extended hooks (desktop flags & storage & jump)
// ============================================================
{
    const sb = createSandbox();
    let webStorageSaved = null;
    let webStorageRequested = null;
    let nextOrderInvoked = false;

    sb.$ = {
        isNWJS: function() { return true; },
        isElectron: function() { return true; },
        isNeedClickAudio: function() { return true; },
        setStorage: function(key, val, type) { return "disk_saved"; },
        getStorage: function(key, type) { return "disk_loaded"; },
        setStorageWeb: function(key, val) {
            webStorageSaved = { key: key, val: val };
            return true;
        },
        getStorageWeb: function(key) {
            webStorageRequested = key;
            return "web_data_for_" + key;
        }
    };

    sb.TYRANO = {
        kag: {
            tmp: {},
            stat: { current_scenario: "scene1.ks" },
            cache_scenario: {
                "scene1.ks": {
                    map_label: { "real_label": { index: 0, label_name: "real_label" } }
                }
            },
            ftag: {
                startTag: function(tag_name, pm) {
                    return "orig_tag_executed";
                },
                nextOrder: function() {
                    nextOrderInvoked = true;
                    return "order_advanced";
                }
            }
        }
    };

    vm.runInContext(shimsCode, sb);

    // 1. Verify desktop flags forced to false
    assert.strictEqual(sb.$.isNWJS(), false, "$.isNWJS must return false");
    assert.strictEqual(sb.$.isElectron(), false, "$.isElectron must return false");
    assert.strictEqual(sb.$.isNeedClickAudio(), false, "$.isNeedClickAudio must return false");

    // 2. Verify storage redirected to web storage when type === 'file'
    sb.$.setStorage("slot_1", "payload_abc", "file");
    assert.deepStrictEqual(webStorageSaved, { key: "slot_1", val: "payload_abc" },
        "setStorage with type 'file' must redirect to setStorageWeb");

    const loadedVal = sb.$.getStorage("slot_1", "file");
    assert.strictEqual(loadedVal, "web_data_for_slot_1",
        "getStorage with type 'file' must redirect to getStorageWeb");

    // 3. Verify placeholder jump target safety
    assert.strictEqual(sb.TYRANO.kag.tmp.ready_audio, true, "ready_audio must be true");

    sb.TYRANO.kag.ftag.startTag("jump", { target: "*未定" });
    assert.strictEqual(nextOrderInvoked, true, "Placeholder jump '*未定' must safely advance via nextOrder");

    nextOrderInvoked = false;
    sb.TYRANO.kag.ftag.startTag("jump", { target: "*todo" });
    assert.strictEqual(nextOrderInvoked, true, "Placeholder jump '*todo' must safely advance via nextOrder");

    // Real jump label should call origStartTag
    const realJump = sb.TYRANO.kag.ftag.startTag("jump", { target: "*real_label" });
    assert.strictEqual(realJump, "orig_tag_executed", "Existing target label must invoke original startTag");

    console.log("TyranoBuilder extended hooks tests PASSED");
}

// ============================================================
// [CAP-WEB-016] Late-binding polling and lifecycle coordinator
// ============================================================
{
    const sb = createSandbox();
    let capturedIntervalFn = null;
    let capturedTimerId = 99;
    let timerCleared = false;

    sb.setInterval = function(fn, ms) {
        capturedIntervalFn = fn;
        return capturedTimerId;
    };
    sb.clearInterval = function(id) {
        if (id === capturedTimerId) timerCleared = true;
    };

    vm.runInContext(shimsCode, sb);

    assert(typeof capturedIntervalFn === "function", "pollPatches interval should be registered");

    // Late-inject unpatched game managers
    sb.StorageManager = { isLocalMode: function() { return true; } };
    sb.AudioManager = { audioFileExt: function() { return ".m4a"; } };
    sb.Graphics = { _clearUpperCanvas: function() {} };
    sb.SceneManager = { catchException: function() {} };

    // Trigger polling step
    capturedIntervalFn();

    // Verify all late-injected managers were patched by the coordinator
    assert.strictEqual(sb.StorageManager.isLocalMode(), false, "StorageManager must be patched by polling coordinator");
    assert.strictEqual(sb.AudioManager.audioFileExt(), ".ogg", "AudioManager must be patched by polling coordinator");
    assert(sb.Graphics._clearUpperCanvas.__patched, "Graphics must be patched by polling coordinator");
    assert(sb.SceneManager.catchException.__patched, "SceneManager must be patched by polling coordinator");

    console.log("Late-binding polling coordinator tests PASSED");
}

// ============================================================
// [CAP-WEB-029] Platform Identity Virtualization (Utils.isNwjs false guard)
// ============================================================
{
    const sb = createSandbox();
    vm.runInContext(shimsCode, sb);

    // 1. Precondition: Unpatched Utils in mock Node environment returns true
    vm.runInContext(`
        function Utils() { throw new Error("static class"); }
        Utils.RPGMAKER_NAME = "MZ";
        Utils.canPlayOgg = function() { return true; };
        Utils.isNwjs = function() {
            return typeof require === "function" && typeof process === "object";
        };
        window.Utils = Utils;
    `, sb);

    assert.strictEqual(sb.Utils.isNwjs(), true, "Precondition: Utils.isNwjs() returns true prior to fence");

    // 2. Synchronous VFS Core Trailer evaluates in same script context
    vm.runInContext(trailerCode, sb);

    assert.strictEqual(sb.Utils.isNwjs(), false, "Utils.isNwjs() must return false after platform identity virtualization");
    assert.strictEqual(sb.Utils.RPGMAKER_NAME, "MZ", "Unrelated property RPGMAKER_NAME must be preserved");
    assert.strictEqual(sb.Utils.canPlayOgg(), true, "Unrelated method canPlayOgg must be preserved");

    // 3. Setter absorbs subsequent engine or plugin reassignment
    vm.runInContext(`
        Utils.isNwjs = function() { return true; };
    `, sb);
    assert.strictEqual(sb.Utils.isNwjs(), false, "Subsequent reassignment must be absorbed by setter");

    // 4. Strict-mode reassignment does not throw
    assert.doesNotThrow(() => {
        vm.runInContext(`
            "use strict";
            Utils.isNwjs = function() { return "desktop"; };
        `, sb);
    }, "Strict mode reassignment must not throw TypeError");
    assert.strictEqual(sb.Utils.isNwjs(), false, "Strict mode reassignment must remain false");

    // 5. Immediate downstream plugin guard: os.tmpdir() not called
    let osTmpdirCalled = false;
    sb.require = function(mod) {
        if (mod === "os") {
            return {
                tmpdir: () => {
                    osTmpdirCalled = true;
                    return "/tmp";
                }
            };
        }
        return {};
    };

    vm.runInContext(`
        // Simulates shiroin_multilingualSupport.js detectLaunchEnvironment
        (function() {
            if (!Utils.isNwjs || !Utils.isNwjs()) {
                // Mobile/browser branch: early return
                return;
            }
            var os = require("os");
            os.tmpdir();
        })();
    `, sb);
    assert.strictEqual(osTmpdirCalled, false, "Desktop plugin os.tmpdir() branch must not be executed when Utils.isNwjs() is false");

    // 6. Test with plain object shape (MV-style or custom tool)
    const sbObj = createSandbox();
    vm.runInContext(shimsCode, sbObj);
    sbObj.Utils = {
        isNwjs: function() { return true; },
        name: "MV_Obj"
    };
    sbObj.RuntimePack.applyPlatformIdentity(sbObj.Utils);
    assert.strictEqual(sbObj.Utils.isNwjs(), false, "Plain object target must be accepted");
    assert.strictEqual(sbObj.Utils.name, "MV_Obj", "Plain object properties preserved");

    // 7. Idempotence: calling applyPlatformIdentity multiple times produces identical behavior
    sbObj.RuntimePack.applyPlatformIdentity(sbObj.Utils);
    assert.strictEqual(sbObj.Utils.isNwjs(), false, "Re-application must be idempotent");

    // 8. Missing capability / hook leaves original object untouched
    const sbClean = createSandbox();
    vm.runInContext(shimsCode, sbClean);
    sbClean.Utils = {
        isNwjs: function() { return "untouched"; }
    };
    assert.strictEqual(sbClean.RuntimePack.platformIdentityActive, false, "platformIdentityActive must be false initially");
    assert.strictEqual(sbClean.Utils.isNwjs(), "untouched", "Third-party Utils untouched when platform identity hook is not active");

    // 9. StorageManager.isLocalMode returns false and delegates to native bridge for MV and MZ
    const sbSave = createSandbox();
    const recordedSlots = {};
    sbSave.RuntimePackNative = {
        saveSlot: (ns, slot, data) => { recordedSlots[ns + ":" + slot] = data; return true; },
        loadSlot: (ns, slot) => recordedSlots[ns + ":" + slot] || null,
        slotExists: (ns, slot) => (ns + ":" + slot) in recordedSlots,
        deleteSlot: (ns, slot) => { delete recordedSlots[ns + ":" + slot]; return true; },
        listSlots: (ns) => JSON.stringify(Object.keys(recordedSlots).filter(k => k.startsWith(ns + ":")).map(k => k.split(":")[1]))
    };
    vm.runInContext(shimsCode, sbSave);

    // MZ StorageManager
    vm.runInContext(`
        window.StorageManager = {
            isLocalMode: function() { return Utils.isNwjs(); },
            saveToForage: function() {},
            loadFromForage: function() {},
            updateForageKeys: function() {},
            forageExists: function() {}
        };
    `, sbSave);
    sbSave.RuntimePack.applyPlatformIdentity(sbSave.Utils || {});
    assert.strictEqual(sbSave.StorageManager.isLocalMode(), false, "StorageManager.isLocalMode must be false");

    sbSave.StorageManager.saveToForage("save1", "mz_data_payload").then(() => {
        assert(recordedSlots["mz:save1"], "Native save bridge must receive MZ save payload");
        return sbSave.StorageManager.loadFromForage("save1");
    }).then((loaded) => {
        assert.strictEqual(loaded, "mz_data_payload", "Loaded MZ payload must match saved data");
    });

    console.log("Platform Identity Virtualization (CAP-WEB-029) tests PASSED");

    // [CAP-WEB-030] Bounded External URL Service Tests
    const sbUrl = vm.createContext({
        console: { log: () => {}, warn: () => {}, error: () => {} },
        window: {},
        setTimeout: (fn) => fn(),
        clearTimeout: () => {}
    });
    sbUrl.window = sbUrl;
    vm.runInContext(shimsCode, sbUrl);

    assert(sbUrl.RuntimePack, "RuntimePack namespace must exist");
    assert.strictEqual(typeof sbUrl.RuntimePack.openExternalUrl, "function", "RuntimePack.openExternalUrl must be a function");
    assert.strictEqual(typeof sbUrl.window.RuntimePack.openExternalUrl, "function", "window.RuntimePack.openExternalUrl must be exposed");

    // 1. Without native bridge, returns false without throwing
    let warnLogged = false;
    sbUrl.console.warn = (msg) => {
        if (msg && msg.includes("CAP-WEB-030")) warnLogged = true;
    };
    const resNoBridge = sbUrl.RuntimePack.openExternalUrl("https://example.com");
    assert.strictEqual(resNoBridge, false, "openExternalUrl without native bridge must return false");
    assert.strictEqual(warnLogged, true, "openExternalUrl without bridge must log warning");

    // 2. With mock native bridge accepting URL
    let dispatchedUrl = null;
    sbUrl.RuntimePackNative = {
        openExternalUrl: (url) => {
            dispatchedUrl = url;
            return true;
        }
    };
    const resAccepted = sbUrl.RuntimePack.openExternalUrl("https://patreon.com/test");
    assert.strictEqual(resAccepted, true, "openExternalUrl must return true when native bridge accepts");
    assert.strictEqual(dispatchedUrl, "https://patreon.com/test", "Native bridge must receive exact URL");

    // 3. With mock native bridge rejecting URL
    let rejectWarnLogged = false;
    sbUrl.console.warn = (msg) => {
        if (msg && msg.includes("CAP-WEB-030")) rejectWarnLogged = true;
    };
    sbUrl.RuntimePackNative.openExternalUrl = () => false;
    const resRejected = sbUrl.RuntimePack.openExternalUrl("http://user:pass@evil.com");
    assert.strictEqual(resRejected, false, "openExternalUrl must return false when native bridge rejects");
    assert.strictEqual(rejectWarnLogged, true, "openExternalUrl must log warning when native bridge rejects");

    // 4. Verify no fake cordova global or OS module injected
    assert.strictEqual(typeof sbUrl.cordova, "undefined", "No bare cordova global must be injected");
    assert.strictEqual(typeof sbUrl.window.cordova, "undefined", "window.cordova must remain undefined");

    // 5. Verify CAP-WEB-029 APIs preserved
    assert.strictEqual(typeof sbUrl.RuntimePack.applyPlatformIdentity, "function", "applyPlatformIdentity must remain present");

    // 6. [MED5] Verify platformIdentityActive preservation across shims script re-entry
    assert.strictEqual(sbUrl.RuntimePack.platformIdentityActive, false, "Initial platformIdentityActive is false");
    sbUrl.RuntimePack.applyPlatformIdentity(sbUrl.Utils || {});
    assert.strictEqual(sbUrl.RuntimePack.platformIdentityActive, true, "platformIdentityActive is true after apply");
    // Re-evaluate shimsCode in same context (simulates page navigation / re-injection)
    vm.runInContext(shimsCode, sbUrl);
    assert.strictEqual(sbUrl.RuntimePack.platformIdentityActive, true, "platformIdentityActive must remain true after shims re-entry");
    assert.strictEqual(typeof sbUrl.RuntimePack.openExternalUrl, "function", "openExternalUrl preserved across re-entry");

    console.log("Bounded External URL Service (CAP-WEB-030) tests PASSED");
}

// ============================================================
// [CAP-WEB-029] Core-first ordering handoff (shim served after engine core)
// Real trailer text comes from VfsResourceInterceptor.kt; real shim from assets.
// ============================================================
{
    const CORE_UTILS_SNIPPET = `
        function Utils() { throw new Error("static class"); }
        Utils.RPGMAKER_NAME = "MV";
        Utils.isNwjs = function() {
            return typeof require === "function" && typeof process === "object";
        };
        window.Utils = Utils;
    `;

    function createCoreSandbox() {
        const errors = [];
        const warns = [];
        const intervals = [];
        const sandbox = {
            console: {
                log: () => {},
                warn: (msg) => { warns.push(String(msg)); },
                error: (...args) => { errors.push(args.map(String).join(" ")); },
                info: () => {}
            },
            _errors: errors,
            _warns: warns,
            _intervals: intervals,
            require: () => ({}),
            process: { platform: "linux", versions: {} },
            setInterval: (fn) => { intervals.push(fn); return intervals.length; },
            clearInterval: () => {}
        };
        sandbox.window = sandbox;
        sandbox.global = sandbox;
        sandbox.globalThis = sandbox;
        vm.createContext(sandbox);
        return sandbox;
    }

    function web029Errors(sb) {
        return sb._errors.filter((msg) => msg.includes("WEB-029"));
    }

    // 1. Core-first: trailer must pin identity synchronously even when RuntimePack is absent,
    //    and retain the Utils handoff for the shim that is parsed later.
    {
        const sb = createCoreSandbox();
        vm.runInContext(CORE_UTILS_SNIPPET, sb);
        assert.strictEqual(sb.Utils.isNwjs(), true, "Precondition: canonical Utils.isNwjs() is true with require/process present");

        vm.runInContext(trailerCode, sb);

        assert.strictEqual(sb.Utils.isNwjs(), false, "Trailer must pin Utils.isNwjs() false before the shim has executed");
        const descriptor = vm.runInContext('Object.getOwnPropertyDescriptor(Utils, "isNwjs")', sb);
        assert.strictEqual(descriptor.configurable, true, "Trailer fallback must install a configurable accessor");
        assert.strictEqual(typeof descriptor.get, "function", "Trailer fallback must use a getter, not a plain assignment");
        assert.notStrictEqual(typeof sb.__runtimepack_pendingIdentity, "undefined", "Trailer must retain the pending Utils handoff");
        assert.strictEqual(sb.Utils.isNwjs.__rp_patched, true, "Trailer inline pin must use the canonical patched false-fn marker");
        assert.strictEqual(sb.__runtimepack_pendingIdentity, sb.Utils, "Pending handoff must reference the Utils object it pinned");
        assert.strictEqual(typeof sb.RuntimePack, "undefined", "Trailer must not create the RuntimePack namespace");
        assert.strictEqual(web029Errors(sb).length, 0, "Deferred handoff must not be reported as a WEB-029 error");
        assert(sb._warns.some((msg) => msg.includes("WEB-029") && msg.toLowerCase().includes("pending")),
            "Deferred handoff must be reported as a pending warning");
        assert.strictEqual(sb.Utils.RPGMAKER_NAME, "MV", "Unrelated Utils properties must be preserved");
    }

    // 2. Intermediate custom script between core and shim must already observe the false identity.
    {
        const sb = createCoreSandbox();
        vm.runInContext(CORE_UTILS_SNIPPET, sb);
        vm.runInContext(trailerCode, sb);

        let requireCalls = 0;
        sb.require = () => { requireCalls++; return {}; };
        vm.runInContext(`
            (function() {
                if (typeof Utils !== "undefined" && Utils.isNwjs && Utils.isNwjs()) {
                    window.__rp_desktop_branch = true;
                    require("nw.gui");
                }
            })();
        `, sb);

        assert.strictEqual(sb.__rp_desktop_branch, undefined, "Desktop branch must not run between core completion and shim execution");
        assert.strictEqual(requireCalls, 0, "No desktop require() may happen between core completion and shim execution");
    }

    // 3. Shim-first then core (canonical HTML order): applier is used directly, nothing pending.
    {
        const sb = createCoreSandbox();
        vm.runInContext(shimsCode, sb);
        assert.strictEqual(sb.RuntimePack.platformIdentityActive, false, "Identity must not be active before Utils exists");

        vm.runInContext(CORE_UTILS_SNIPPET, sb);
        vm.runInContext(trailerCode, sb);

        assert.strictEqual(sb.Utils.isNwjs(), false, "Shim-first order must pin Utils.isNwjs() false");
        assert.strictEqual(sb.RuntimePack.platformIdentityActive, true, "Shim-first order must mark identity active");
        assert.strictEqual(typeof sb.__runtimepack_pendingIdentity, "undefined", "Shim-first order must not retain a pending handoff");
        assert.strictEqual(web029Errors(sb).length, 0, "Shim-first order must not log WEB-029 errors");
    }

    // 4. Core-first: the shim consumes the pending handoff, activates identity, clears pending,
    //    and the touch latch is applied (not latched-without-patch).
    {
        const sb = createCoreSandbox();
        vm.runInContext(CORE_UTILS_SNIPPET, sb);
        vm.runInContext(trailerCode, sb);
        assert.notStrictEqual(typeof sb.__runtimepack_pendingIdentity, "undefined", "Precondition: pending handoff retained");

        sb.Window_Selectable = {
            prototype: {
                isOkEnabled: function() { return true; }
            }
        };

        vm.runInContext(shimsCode, sb);

        assert.strictEqual(sb.RuntimePack.platformIdentityActive, true, "Shim must activate identity from the pending handoff");
        assert.strictEqual(sb.Utils.isNwjs(), false, "Shim must keep Utils.isNwjs() false after consuming the handoff");
        assert.strictEqual(sb.Utils.isNwjs.__rp_patched, true, "Shim applier must be the canonical patched function");
        assert.strictEqual(typeof sb.__runtimepack_pendingIdentity, "undefined", "Pending handoff must be cleared after a successful pin");
        assert.strictEqual(web029Errors(sb).length, 0, "Successful handoff must not log WEB-029 errors");
        assert.strictEqual(sb.Window_Selectable.prototype.__runtimepack_touch_ok_patched, true, "Touch latch must be marked");
        assert.strictEqual(typeof sb.Window_Selectable.prototype.isTouchOkEnabled, "function",
            "Touch latch must not be marked without applying the mobile patch (latch-without-patch)");
    }

    // 5. Core-first with Utils replaced before the shim: consume the latest root.Utils, not the stale object.
    {
        const sb = createCoreSandbox();
        vm.runInContext(CORE_UTILS_SNIPPET, sb);
        vm.runInContext(trailerCode, sb);
        const staleUtils = sb.Utils;

        vm.runInContext(`
            window.Utils = {
                RPGMAKER_NAME: "MZ",
                isNwjs: function() { return true; }
            };
        `, sb);
        const latestUtils = sb.Utils;
        assert.notStrictEqual(latestUtils, staleUtils, "Precondition: root.Utils was replaced");

        vm.runInContext(shimsCode, sb);

        assert.strictEqual(latestUtils.isNwjs(), false, "Shim must pin the latest root.Utils object");
        assert.strictEqual(latestUtils.isNwjs.__rp_patched, true, "Latest Utils must receive the canonical patched function");
        assert.strictEqual(sb.RuntimePack.platformIdentityActive, true, "Identity must be active after consuming the handoff");
        assert.strictEqual(typeof sb.__runtimepack_pendingIdentity, "undefined", "Pending handoff must be cleared after pinning latest Utils");
    }

    // 6. Shim re-entry with an outstanding pending handoff consumes it before the early return.
    {
        const sb = createCoreSandbox();
        vm.runInContext(shimsCode, sb);
        assert.strictEqual(sb.__runtimepack_shims_installed, true, "Precondition: shim marked installed");

        vm.runInContext(CORE_UTILS_SNIPPET, sb);
        sb.__runtimepack_pendingIdentity = sb.Utils;
        assert.strictEqual(sb.RuntimePack.platformIdentityActive, false, "Precondition: identity not yet active");

        assert.doesNotThrow(() => vm.runInContext(shimsCode, sb), "Shim re-entry must not throw");
        assert.strictEqual(sb.RuntimePack.platformIdentityActive, true, "Re-entry must consume the pending handoff");
        assert.strictEqual(sb.Utils.isNwjs(), false, "Re-entry must pin Utils.isNwjs() false");
        assert.strictEqual(typeof sb.__runtimepack_pendingIdentity, "undefined", "Re-entry must clear the consumed handoff");

        assert.doesNotThrow(() => vm.runInContext(shimsCode, sb), "Second re-entry must remain idempotent");
        assert.strictEqual(sb.RuntimePack.platformIdentityActive, true, "Second re-entry must keep identity active");
    }

    // 7. Missing Utils keeps the explicit diagnostic and stores no handoff.
    {
        const sb = createCoreSandbox();
        vm.runInContext(trailerCode, sb);

        assert.strictEqual(typeof sb.__runtimepack_pendingIdentity, "undefined", "Missing Utils must not create a pending handoff");
        assert.strictEqual(typeof sb.RuntimePack, "undefined", "Missing Utils must not create the RuntimePack namespace");
        assert(sb._errors.some((msg) => msg.includes("WEB-029") && msg.includes("Utils is undefined")),
            "Missing Utils must log the explicit WEB-029 diagnostic");
    }

    // 8. Trailer fallback absorbs later canonical assignment, including strict mode.
    {
        const sb = createCoreSandbox();
        vm.runInContext(CORE_UTILS_SNIPPET, sb);
        vm.runInContext(trailerCode, sb);

        vm.runInContext('Utils.isNwjs = function() { return true; };', sb);
        assert.strictEqual(sb.Utils.isNwjs(), false, "Fallback accessor must absorb a plain reassignment");
        assert.doesNotThrow(() => vm.runInContext('"use strict"; Utils.isNwjs = function() { return "desktop"; };', sb),
            "Strict-mode reassignment must not throw");
        assert.strictEqual(sb.Utils.isNwjs(), false, "Strict-mode reassignment must remain false");

        vm.runInContext(shimsCode, sb);
        assert.strictEqual(sb.Utils.isNwjs(), false, "Shim handoff must preserve the absorbed false identity");
        assert.strictEqual(typeof sb.__runtimepack_pendingIdentity, "undefined", "Pending handoff must be cleared after shim pin");
    }

    // 9. Negative: a non-configurable Utils cannot be pinned, so the handoff is retained and reported.
    {
        const sb = createCoreSandbox();
        vm.runInContext(CORE_UTILS_SNIPPET, sb);
        vm.runInContext(`
            Object.defineProperty(Utils, "isNwjs", {
                value: function() { return true; },
                configurable: false,
                writable: false,
                enumerable: true
            });
        `, sb);

        vm.runInContext(trailerCode, sb);

        assert.strictEqual(sb.Utils.isNwjs(), true, "Non-configurable Utils cannot be pinned by the trailer");
        assert(sb._errors.some((msg) => msg.includes("WEB-029") && msg.includes("Failed to pin")),
            "Failed trailer pin must be reported");
        assert.strictEqual(sb.__runtimepack_pendingIdentity, sb.Utils, "Failed trailer pin must retain the handoff for retry");

        vm.runInContext(shimsCode, sb);

        assert.strictEqual(sb.RuntimePack.platformIdentityActive, false, "Failed shim pin must not report identity active");
        assert.strictEqual(sb.Utils.isNwjs(), true, "Non-configurable Utils stays unpinned");
        assert.strictEqual(sb.__runtimepack_pendingIdentity, sb.Utils, "Failed shim pin must retain the handoff");
        assert(sb._errors.some((msg) => msg.includes("WEB-029") && msg.includes("Failed to pin")),
            "Failed shim pin must be reported");
    }

    // 10. Failed pin keeps the Layer-4 retry alive: a later poll-style re-apply against a
    //     now-pinnable Utils must succeed and clear the retained pending handoff.
    {
        const sb = createCoreSandbox();
        vm.runInContext(CORE_UTILS_SNIPPET, sb);
        vm.runInContext(`
            Object.defineProperty(Utils, "isNwjs", {
                value: function() { return true; },
                configurable: false,
                writable: false,
                enumerable: true
            });
        `, sb);

        vm.runInContext(trailerCode, sb);
        vm.runInContext(shimsCode, sb);

        assert.strictEqual(sb.RuntimePack.platformIdentityActive, false, "Precondition: failed pin leaves identity inactive");
        assert.strictEqual(sb.__runtimepack_pendingIdentity, sb.Utils, "Precondition: failed pin retains the handoff");
        assert(sb._intervals.length > 0, "Shim must install the late-binding poll for retry");

        // Engine replaces Utils with a pinnable object (fresh, configurable property).
        vm.runInContext(`
            window.Utils = { RPGMAKER_NAME: "MZ", isNwjs: function() { return true; } };
        `, sb);
        const retryUtils = sb.Utils;

        // Poll-style re-apply, invoked directly instead of waiting on wall-clock timers.
        sb._intervals.forEach((fn) => fn());

        assert.strictEqual(retryUtils.isNwjs(), false, "Poll retry must pin the now-pinnable Utils");
        assert.strictEqual(retryUtils.isNwjs.__rp_patched, true, "Poll retry must use the canonical patched false-fn");
        assert.strictEqual(sb.RuntimePack.platformIdentityActive, true, "Poll retry must mark identity active after a verified pin");
        assert.strictEqual(typeof sb.__runtimepack_pendingIdentity, "undefined", "Poll retry must clear the handoff after success");
    }

    // 11. A throwing pending assignment must not abort the trailer before pin verification.
    {
        const sb = createCoreSandbox();
        vm.runInContext(CORE_UTILS_SNIPPET, sb);
        vm.runInContext(`
            Object.defineProperty(window, "__runtimepack_pendingIdentity", {
                configurable: true,
                set: function() { throw new Error("global assignment denied"); }
            });
        `, sb);

        assert.doesNotThrow(() => vm.runInContext(trailerCode, sb),
            "Trailer must not propagate a throwing pending assignment");
        assert.strictEqual(sb.Utils.isNwjs(), false, "Trailer must still pin when the pending assignment throws");
        assert(sb._warns.some((msg) => msg.includes("WEB-029") && msg.toLowerCase().includes("pending")),
            "Trailer must still report the provisional handoff when the pending assignment throws");
    }

    console.log("Core-first ordering handoff (CAP-WEB-029) tests PASSED");
}

{
    // [CAP-WEB-031] Oversized atlas sprite rendering (v2 frame-fit canvas texture).
    //
    // Revised contract (Task 3 Fix Brief top revision):
    // 1. Original Bitmap._baseTexture MUST NOT be rebound to a partial tile (preserves identity,
    //    full dimensions, and unpatched consumers such as TilingSprite / full-frame Sprite).
    // 2. Plain Sprite whose source exceeds gl.MAX_TEXTURE_SIZE but requested frame <= limit gets
    //    an owned GPU-fit frame-sized canvas texture.
    // 3. Copies ALL in-bounds pixels even if the rectangle crosses a former 4096 boundary.
    // 4. Frames larger than the limit stay on the engine path and emit a structured UNSUPPORTED
    //    diagnostic (NO silent clipping, NO fake pass).
    // 5. TilingSprite remains untouched on the original baseTexture; no introduced empty quads.
    // 6. Real PIXI frame-fit validation and destroyed-resource semantics modeled faithfully.
    // 7. Bounded allocation per Sprite, live invalidation on resize/mutation, and clean release.
    console.log("Oversized atlas frame rendering (CAP-WEB-031 v2) tests...");

    function frameBox(frame) {
        return { x: frame.x, y: frame.y, width: frame.width, height: frame.height };
    }

    // Locate a shipped PIXI bundle for the real-semantics probe. No machine-specific default:
    // the path must be supplied through the environment, otherwise the probe is reported as
    // NOT_TESTED instead of silently passing on a weaker fake.
    function resolvePixiPath() {
        const candidates = [];
        if (process.env.RUNTIMEPACK_PIXI_PATH) {
            candidates.push(process.env.RUNTIMEPACK_PIXI_PATH);
        }
        if (process.env.RUNTIMEPACK_GAME_ROOT) {
            candidates.push(path.join(process.env.RUNTIMEPACK_GAME_ROOT, "js", "libs", "pixi.js"));
        }
        return candidates.find((candidate) => {
            try {
                return fs.statSync(candidate).isFile();
            } catch (_) {
                return false;
            }
        }) || null;
    }

    // Minimal DOM sufficient to evaluate the real PIXI 5.x UMD bundle headlessly. Canvases record
    // drawImage calls so tests can assert the exact copied source rectangle.
    function loadRealPixi(pixiPath) {
        const source = fs.readFileSync(pixiPath, "utf-8");
        function makeContext(canvas) {
            const store = {};
            return new Proxy(store, {
                get(target, key) {
                    if (key in target) {
                        return target[key];
                    }
                    const fn = function() {
                        if (key === "drawImage") {
                            if (canvas.__failDraw) {
                                throw new Error("drawImage denied");
                            }
                            canvas.__draws.push({
                                source: arguments[0], sx: arguments[1], sy: arguments[2],
                                sw: arguments[3], sh: arguments[4], dx: arguments[5],
                                dy: arguments[6], dw: arguments[7], dh: arguments[8],
                                argc: arguments.length
                            });
                        }
                        return undefined;
                    };
                    target[key] = fn;
                    return fn;
                },
                set(target, key, value) {
                    target[key] = value;
                    return true;
                }
            });
        }
        class TestCanvas {
            constructor(width, height) {
                this.width = width || 1;
                this.height = height || 1;
                this.style = {};
                this.__draws = [];
                this.__failDraw = false;
            }
            getContext() { return makeContext(this); }
            addEventListener() {}
            removeEventListener() {}
            toDataURL() { return "data:,"; }
        }
        class TestImage {
            constructor() { this.width = 0; this.height = 0; }
        }
        const realm = {
            console: { log() {}, warn() {}, error() {}, info() {} },
            navigator: { userAgent: "runtimepack-test" },
            performance: { now: () => 0 },
            setTimeout, clearTimeout, Math, Date, JSON, Object, Array, String, Number, Boolean,
            Error, Map, Set, WeakMap, Promise, Float32Array, Uint16Array, Uint32Array, Uint8Array,
            Int32Array, ArrayBuffer, devicePixelRatio: 1, screen: { width: 1, height: 1 },
            addEventListener() {}, removeEventListener() {}, requestAnimationFrame: () => 0,
            location: { href: "http://runtimepack.test/" }
        };
        realm.HTMLCanvasElement = TestCanvas;
        realm.HTMLImageElement = TestImage;
        realm.Image = class extends TestImage {};
        realm.HTMLVideoElement = class {};
        realm.HTMLIFrameElement = class {};
        realm.ImageBitmap = class {};
        realm.OffscreenCanvas = class extends TestCanvas {};
        realm.VideoFrame = class {};
        realm.document = {
            createElement(tag) {
                if (tag === "canvas") {
                    return new TestCanvas(1, 1);
                }
                return { tagName: String(tag).toUpperCase(), style: {}, appendChild() {}, setAttribute() {}, addEventListener() {} };
            },
            createElementNS() { return {}; },
            addEventListener() {}, removeEventListener() {},
            documentElement: { style: {} },
            head: { appendChild() {} },
            body: { appendChild() {} }
        };
        realm.window = realm;
        realm.self = realm;
        realm.globalThis = realm;
        vm.createContext(realm);
        vm.runInContext(source, realm);
        return {
            PIXI: realm.PIXI,
            document: realm.document,
            HTMLCanvasElement: TestCanvas,
            HTMLImageElement: TestImage,
            version: realm.PIXI && realm.PIXI.VERSION
        };
    }

    function createV2AtlasSandbox(options) {
        const opts = options || {};
        const errors = [];
        const warns = [];
        const intervals = [];
        const createdBaseTextures = [];
        let canvasCount = 0;
        const failAtCanvas = typeof opts.failCanvasAt === "number" ? opts.failCanvasAt : -1;

        function FakeContext(canvas) {
            this.canvas = canvas;
        }
        FakeContext.prototype.drawImage = function(source, sx, sy, sw, sh, dx, dy, dw, dh) {
            this.canvas.draws.push({ source, sx, sy, sw, sh, dx, dy, dw, dh });
        };
        FakeContext.prototype.clearRect = function(x, y, w, h) {
            this.canvas.clears.push({ x, y, w, h });
        };

        function FakeCanvas(w, h) {
            this.width = w || 0;
            this.height = h || 0;
            this.draws = [];
            this.clears = [];
            this.contextRequests = [];
            this._ctx = new FakeContext(this);
        }
        FakeCanvas.prototype.getContext = function(type) {
            this.contextRequests.push(type);
            if (type !== "2d") return null;
            return this._ctx;
        };

        function FakeRectangle(x, y, width, height) {
            this.x = x || 0;
            this.y = y || 0;
            this.width = width || 0;
            this.height = height || 0;
        }
        FakeRectangle.prototype.clone = function() {
            return new FakeRectangle(this.x, this.y, this.width, this.height);
        };

        // Real PIXI 5.3.12 semantics:
        // - Frame setter enforces x + width <= baseTexture.width and y + height <= baseTexture.height.
        // - destroy() sets destroyed=true, resource=null, clears caches.
        // - update() increments dirtyId.
        function FakeBaseTexture(source) {
            this.source = source || null;
            this.width = source && source.width ? source.width : 0;
            this.height = source && source.height ? source.height : 0;
            this.scaleMode = null;
            this.mipmap = true;
            this.valid = true;
            this.dirtyId = 0;
            this.destroyed = false;
            this.resource = source ? { type: "canvas", internal: true } : null;
            this.destroyCount = 0;
            createdBaseTextures.push(this);
        }
        FakeBaseTexture.prototype.update = function() {
            this.dirtyId++;
        };
        FakeBaseTexture.prototype.destroy = function() {
            this.destroyCount++;
            if (this.destroyCount > 1) {
                throw new Error("BaseTexture destroyed twice");
            }
            this.destroyed = true;
            this.resource = null;
            this.valid = false;
        };

        function FakeTexture(baseTexture, frame) {
            this.baseTexture = baseTexture || null;
            this._frame = frame || new FakeRectangle();
            this.valid = Boolean(frame && frame.width && frame.height);
            this._updateID = 0;
        }
        Object.defineProperty(FakeTexture.prototype, "frame", {
            get: function() { return this._frame; },
            set: function(frame) {
                this._frame = frame;
                if (!frame) return;
                const bt = this.baseTexture;
                if (bt) {
                    if (bt.destroyed) {
                        throw new Error("PIXI: cannot set frame on destroyed baseTexture");
                    }
                    const xFit = frame.x + frame.width <= bt.width;
                    const yFit = frame.y + frame.height <= bt.height;
                    if (!xFit || !yFit) {
                        throw new Error(
                            "Texture Error: frame does not fit inside the base Texture dimensions: " +
                            "X: " + frame.x + " + " + frame.width + " = " + (frame.x + frame.width) + " > " + bt.width +
                            " or Y: " + frame.y + " + " + frame.height + " = " + (frame.y + frame.height) + " > " + bt.height
                        );
                    }
                }
                this.valid = Boolean(frame.width && frame.height);
            },
            configurable: true
        });

        function FakePixiSprite(texture) {
            this.texture = texture;
            this.pivot = { x: 0, y: 0 };
            this.children = [];
        }

        // Real MZ Bitmap shape
        function FakeBitmap(width, height) {
            this._canvas = null;
            this._context = null;
            this._baseTexture = null;
            this._image = null;
            this._smooth = false;
            this._loadListeners = [];
            this._paintOpacity = 255;
            if (width > 0 && height > 0) {
                this._createCanvas(width, height);
            }
        }
        FakeBitmap.prototype._createCanvas = function(width, height) {
            this._canvas = new FakeCanvas(width, height);
            this._context = this._canvas.getContext("2d");
            this._createBaseTexture(this._canvas);
        };
        FakeBitmap.prototype._ensureCanvas = function() {
            if (!this._canvas) {
                if (this._image) {
                    this._createCanvas(this._image.width, this._image.height);
                    this._context.drawImage(this._image, 0, 0, this._image.width, this._image.height,
                        0, 0, this._image.width, this._image.height);
                } else {
                    this._createCanvas(0, 0);
                }
            }
        };
        Object.defineProperty(FakeBitmap.prototype, "canvas", {
            get: function() { this._ensureCanvas(); return this._canvas; },
            configurable: true
        });
        Object.defineProperty(FakeBitmap.prototype, "context", {
            get: function() { this._ensureCanvas(); return this._context; },
            configurable: true
        });
        Object.defineProperty(FakeBitmap.prototype, "width", {
            get: function() { const img = this._canvas || this._image; return img ? img.width : 0; },
            configurable: true
        });
        Object.defineProperty(FakeBitmap.prototype, "height", {
            get: function() { const img = this._canvas || this._image; return img ? img.height : 0; },
            configurable: true
        });
        Object.defineProperty(FakeBitmap.prototype, "baseTexture", {
            get: function() { return this._baseTexture; },
            configurable: true
        });
        Object.defineProperty(FakeBitmap.prototype, "smooth", {
            get: function() { return this._smooth; },
            set: function(val) {
                if (this._smooth !== val) {
                    this._smooth = val;
                    this._updateScaleMode();
                }
            },
            configurable: true
        });
        FakeBitmap.prototype._createBaseTexture = function(source) {
            this._baseTexture = new FakeBaseTexture(source);
            this._baseTexture.mipmap = false;
            this._baseTexture.width = source.width;
            this._baseTexture.height = source.height;
            this._updateScaleMode();
        };
        FakeBitmap.prototype._updateScaleMode = function() {
            if (this._baseTexture) {
                this._baseTexture.scaleMode = this._smooth ? 1 : 0;
            }
        };
        FakeBitmap.prototype._onLoad = function() {
            this._createBaseTexture(this._image);
        };
        FakeBitmap.prototype.resize = function(w, h) {
            w = Math.max(w || 0, 1);
            h = Math.max(h || 0, 1);
            this.canvas.width = w;
            this.canvas.height = h;
            this.baseTexture.width = w;
            this.baseTexture.height = h;
        };
        FakeBitmap.prototype.destroy = function() {
            if (this._baseTexture) {
                this._baseTexture.destroy();
                this._baseTexture = null;
            }
            if (this._canvas) {
                this._canvas.width = 0;
                this._canvas.height = 0;
                this._canvas = null;
            }
        };

        // Real MZ Sprite shape
        function FakeSprite(bitmap) {
            this._bitmap = bitmap || null;
            this._frame = new FakeRectangle();
            this._refreshFrame = true;
            this.texture = new FakeTexture(new FakeBaseTexture(new FakeCanvas(1, 1)), new FakeRectangle());
            this.pivot = { x: 0, y: 0 };
        }
        Object.defineProperty(FakeSprite.prototype, "bitmap", {
            get: function() { return this._bitmap; },
            set: function(val) {
                if (this._bitmap !== val) {
                    this._bitmap = val;
                    this._refreshFrame = true;
                    this._refresh();
                }
            },
            configurable: true
        });
        FakeSprite.prototype.setFrame = function(x, y, width, height) {
            this._refreshFrame = false;
            const frame = this._frame;
            if (x !== frame.x || y !== frame.y || width !== frame.width || height !== frame.height) {
                frame.x = x;
                frame.y = y;
                frame.width = width;
                frame.height = height;
                this._refresh();
            }
        };
        FakeSprite.prototype.destroy = function() {
            this.destroyed = true;
        };
        FakeSprite.prototype._refresh = function() {
            const texture = this.texture;
            const frameX = Math.floor(this._frame.x);
            const frameY = Math.floor(this._frame.y);
            const frameW = Math.floor(this._frame.width);
            const frameH = Math.floor(this._frame.height);
            const baseTexture = this._bitmap ? this._bitmap.baseTexture : null;
            const baseTextureW = baseTexture ? baseTexture.width : 0;
            const baseTextureH = baseTexture ? baseTexture.height : 0;
            const realX = Math.min(Math.max(frameX, 0), baseTextureW);
            const realY = Math.min(Math.max(frameY, 0), baseTextureH);
            const realW = Math.min(Math.max(frameW - realX + frameX, 0), baseTextureW - realX);
            const realH = Math.min(Math.max(frameH - realY + frameY, 0), baseTextureH - realY);
            const frame = new FakeRectangle(realX, realY, realW, realH);
            if (texture) {
                this.pivot.x = frameX - realX;
                this.pivot.y = frameY - realY;
                if (this._tintTexture) {
                    // MV tint path (rpg_core.js:4200-4205): engine crops into its own canvas.
                    texture.baseTexture = this._tintTexture;
                    texture.frame = new FakeRectangle(0, 0, realW, realH);
                } else if (baseTexture) {
                    texture.baseTexture = baseTexture;
                    try {
                        texture.frame = frame;
                    } catch (e) {
                        texture.frame = new FakeRectangle();
                    }
                }
                texture._updateID++;
            }
        };

        // Real MZ TilingSprite shape: assigns frame against bitmap.baseTexture directly.
        function FakeTilingSprite(bitmap) {
            this._bitmap = bitmap || null;
            this._frame = new FakeRectangle();
            this.texture = new FakeTexture(new FakeBaseTexture(new FakeCanvas(1, 1)), new FakeRectangle());
        }
        FakeTilingSprite.prototype._refresh = function() {
            const texture = this.texture;
            const frame = this._frame.clone();
            if (frame.width === 0 && frame.height === 0 && this._bitmap) {
                frame.width = this._bitmap.width;
                frame.height = this._bitmap.height;
            }
            if (texture) {
                if (texture.baseTexture) {
                    try {
                        texture.frame = frame;
                    } catch (e) {
                        texture.frame = new FakeRectangle();
                    }
                }
                texture._updateID++;
            }
        };

        // Minimal PIXI Container/Scene shape: parent links are what the scene purge walks.
        function FakeContainer(name) {
            this.name = name || "";
            this.children = [];
            this.parent = null;
        }
        FakeContainer.prototype.addChild = function(child) {
            this.children.push(child);
            child.parent = this;
            return child;
        };

        const sandbox = {
            console: {
                log: function() {},
                warn: function(msg) { warns.push(String(msg)); },
                error: function(msg) { errors.push(String(msg)); },
                info: function() {}
            },
            btoa: (str) => Buffer.from(str, "binary").toString("base64"),
            atob: (b64) => Buffer.from(b64, "base64").toString("binary"),
            document: {
                createElement: function(tag) {
                    if (tag === "canvas") {
                        canvasCount++;
                        if (failAtCanvas > 0 && canvasCount === failAtCanvas) {
                            throw new Error("canvas allocation denied");
                        }
                        return new FakeCanvas();
                    }
                    return {};
                }
            },
            setInterval: function(fn) { intervals.push(fn); return intervals.length; },
            clearInterval: function() {},
            _errors: errors,
            _warns: warns,
            _intervals: intervals
        };
        sandbox.window = sandbox;
        sandbox.global = sandbox;
        sandbox.globalThis = sandbox;

        if (opts.withEngineClasses !== false) {
            sandbox.Bitmap = FakeBitmap;
            sandbox.Sprite = FakeSprite;
            sandbox.TilingSprite = FakeTilingSprite;
        }
        if (opts.withPixi !== false) {
            sandbox.PIXI = {
                BaseTexture: FakeBaseTexture,
                Texture: FakeTexture,
                Rectangle: FakeRectangle,
                Sprite: FakePixiSprite,
                SCALE_MODES: { LINEAR: 1, NEAREST: 0 },
                MIPMAP_MODES: { OFF: 0 }
            };
        }
        if (opts.withRenderer !== false) {
            const maxTextureSize = typeof opts.maxTextureSize === "number" ? opts.maxTextureSize : 4096;
            sandbox.Graphics = {
                app: {
                    renderer: {
                        gl: {
                            MAX_TEXTURE_SIZE: 0x0d33,
                            getParameter: function(param) {
                                return param === 0x0d33 ? maxTextureSize : 0;
                            }
                        }
                    }
                }
            };
        }

        vm.createContext(sandbox);
        return {
            sandbox,
            FakeBitmap,
            FakeSprite,
            FakeTilingSprite,
            FakeContainer,
            FakeCanvas,
            errors,
            warns,
            intervals,
            createdBaseTextures
        };
    }

    // 1. Fast path: fitting asset is byte-identical and allocates no owned frame texture.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const image = { width: 512, height: 512 };
        const bitmap = new env.FakeBitmap();
        bitmap._image = image;
        bitmap._onLoad();

        const sprite = new env.FakeSprite(bitmap);
        const baseTexturesBefore = env.createdBaseTextures.length;
        sprite.setFrame(0, 0, 512, 512);

        assert.strictEqual(sprite.texture.baseTexture, bitmap.baseTexture,
            "Fitting asset must keep the engine baseTexture directly on the sprite");
        assert.deepStrictEqual(frameBox(sprite.texture.frame), { x: 0, y: 0, width: 512, height: 512 },
            "Fitting asset frame must stay unchanged");
        assert.strictEqual(env.createdBaseTextures.length, baseTexturesBefore,
            "Fast path must allocate no extra BaseTexture");
        assert.strictEqual(sprite.__rpFrameTexture, undefined,
            "Fast path sprite must have no owned frame texture attached");
    }

    // 2. M1 verification: original Bitmap._baseTexture identity and dimensions MUST NOT change.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const image = { width: 512, height: 4128 };
        const bitmap = new env.FakeBitmap();
        bitmap._image = image;
        bitmap._onLoad();

        const origBaseTexture = bitmap.baseTexture;
        assert(origBaseTexture, "Engine must produce the initial baseTexture");
        assert.strictEqual(origBaseTexture.width, 512, "BaseTexture.width must reflect the full source width");
        assert.strictEqual(origBaseTexture.height, 4128, "BaseTexture.height must reflect the full source height (4128)");
        assert.strictEqual(bitmap.width, 512, "Bitmap.width must be 512");
        assert.strictEqual(bitmap.height, 4128, "Bitmap.height must be 4128");

        // Plain sprite renders a frame: original baseTexture must retain its identity and dimensions.
        const sprite = new env.FakeSprite(bitmap);
        sprite.setFrame(0, 3104, 32, 32);

        assert.strictEqual(bitmap.baseTexture, origBaseTexture,
            "M1 regression guard: Bitmap._baseTexture must NOT be replaced with a partial tile");
        assert.strictEqual(bitmap.baseTexture.width, 512, "BaseTexture width must remain 512");
        assert.strictEqual(bitmap.baseTexture.height, 4128, "BaseTexture height must remain 4128");

        // TilingSprite referencing the same bitmap must NOT be broken by primary substitution.
        const tiling = new env.FakeTilingSprite(bitmap);
        tiling.texture.baseTexture = bitmap.baseTexture;
        tiling._refresh();
        assert.strictEqual(tiling.texture.baseTexture, origBaseTexture,
            "TilingSprite must reference the original full baseTexture");
        assert.strictEqual(tiling.texture.frame.width, 512,
            "TilingSprite must keep full 512 width without throwing or collapsing to empty quad");
        assert.strictEqual(tiling.texture.frame.height, 4128,
            "TilingSprite must keep full 4128 height without throwing");
    }

    // 3. Haven enemy cursor (icon 1556, y=3104) renders via GPU-fit frame canvas.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const image = { width: 512, height: 4128 };
        const bitmap = new env.FakeBitmap();
        bitmap._image = image;
        bitmap._onLoad();

        const sprite = new env.FakeSprite(bitmap);
        sprite.setFrame(0, 3104, 32, 32);

        assert.notStrictEqual(sprite.texture.baseTexture, bitmap.baseTexture,
            "Oversized asset sprite must use an owned GPU-fit base texture, not the 4128-tall upload-failing one");
        assert(sprite.texture.baseTexture.width <= 4096, "Owned baseTexture width must fit MAX_TEXTURE_SIZE");
        assert(sprite.texture.baseTexture.height <= 4096, "Owned baseTexture height must fit MAX_TEXTURE_SIZE");
        assert.strictEqual(sprite.texture.baseTexture.width, 32, "Owned baseTexture width must equal requested frame width");
        assert.strictEqual(sprite.texture.baseTexture.height, 32, "Owned baseTexture height must equal requested frame height");
        assert.deepStrictEqual(frameBox(sprite.texture.frame), { x: 0, y: 0, width: 32, height: 32 },
            "Sprite frame on the owned texture must be local (0, 0, 32, 32)");

        const ownedCanvas = sprite.texture.baseTexture.source;
        assert(ownedCanvas, "Owned baseTexture must be backed by a canvas");
        assert.strictEqual(ownedCanvas.draws.length, 1, "Must copy source pixels once");
        assert.deepStrictEqual(ownedCanvas.draws[0], {
            source: image, sx: 0, sy: 3104, sw: 32, sh: 32, dx: 0, dy: 0, dw: 32, dh: 32
        }, "Owned canvas must copy exact row 97 (y=3104) pixels from the decoded image");

        // Repeated refreshes must reuse the owned texture without reallocating.
        const ownedBt = sprite.texture.baseTexture;
        const countBefore = env.createdBaseTextures.length;
        sprite._refresh();
        sprite._refresh();
        sprite.setFrame(0, 3104, 32, 32);
        assert.strictEqual(sprite.texture.baseTexture, ownedBt, "Repeated refresh must reuse the owned texture");
        assert.strictEqual(env.createdBaseTextures.length, countBefore, "Repeated refresh must allocate no new BaseTexture");
    }

    // 4. Overflow row (y=4096) renders all 32 rows cleanly.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const image = { width: 512, height: 4128 };
        const bitmap = new env.FakeBitmap();
        bitmap._image = image;
        bitmap._onLoad();

        const sprite = new env.FakeSprite(bitmap);
        sprite.setFrame(0, 4096, 32, 32);

        assert.strictEqual(sprite.texture.baseTexture.width, 32);
        assert.strictEqual(sprite.texture.baseTexture.height, 32);
        assert.deepStrictEqual(frameBox(sprite.texture.frame), { x: 0, y: 0, width: 32, height: 32 });
        const ownedCanvas = sprite.texture.baseTexture.source;
        assert.deepStrictEqual(ownedCanvas.draws[0], {
            source: image, sx: 0, sy: 4096, sw: 32, sh: 32, dx: 0, dy: 0, dw: 32, dh: 32
        }, "Overflow row must copy pixels starting at y=4096 with full height 32");
    }

    // 5. Cross-boundary frame (y=4080, height=32 straddles 4096): MUST NOT be clipped to 16.
    //    Renders all 32 rows, copying pixels from both sides of the former 4096 tile boundary.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const image = { width: 512, height: 4128 };
        const bitmap = new env.FakeBitmap();
        bitmap._image = image;
        bitmap._onLoad();

        const sprite = new env.FakeSprite(bitmap);
        sprite.setFrame(0, 4080, 32, 32);

        assert.strictEqual(sprite.texture.baseTexture.height, 32,
            "Anti-clipping requirement: frame spanning boundary MUST retain full 32px height");
        assert.deepStrictEqual(frameBox(sprite.texture.frame), { x: 0, y: 0, width: 32, height: 32 },
            "Frame on owned texture must have full height 32");
        const ownedCanvas = sprite.texture.baseTexture.source;
        assert.deepStrictEqual(ownedCanvas.draws[0], {
            source: image, sx: 0, sy: 4080, sw: 32, sh: 32, dx: 0, dy: 0, dw: 32, dh: 32
        }, "Owned canvas must copy all 32 rows across the boundary in a single operation");
    }

    // 6. M2 verification: bitmap canvas materialization MUST NOT destroy live sprite textures.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const image = { width: 512, height: 4128 };
        const bitmap = new env.FakeBitmap();
        bitmap._image = image;
        bitmap._onLoad();

        const sprite = new env.FakeSprite(bitmap);
        sprite.setFrame(0, 3104, 32, 32);
        const liveOwnedBt = sprite.texture.baseTexture;
        assert.strictEqual(liveOwnedBt.destroyed, false, "Precondition: owned baseTexture is alive");

        // Engine materializes CPU canvas lazily (e.g. first blt or drawText)
        const cpuCanvas = bitmap.canvas;
        assert(cpuCanvas, "CPU canvas materialized");

        // The sprite's owned texture must STILL be alive and valid.
        assert.strictEqual(liveOwnedBt.destroyed, false,
            "M2 regression guard: lazy canvas materialization must NOT destroy live sprite textures");
        assert.strictEqual(liveOwnedBt.valid, true, "Live texture must remain valid");
        assert.strictEqual(sprite.texture.baseTexture, liveOwnedBt, "Sprite must still hold its live texture");
        assert.doesNotThrow(() => {
            sprite.texture.frame = new env.sandbox.PIXI.Rectangle(0, 0, 32, 32);
        }, "Frame assignment must not throw against a destroyed baseTexture");
    }

    // 7. Bitmap resize/mutation invalidates sprite cache; new frame copies from mutated source.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const image = { width: 512, height: 4128 };
        const bitmap = new env.FakeBitmap();
        bitmap._image = image;
        bitmap._onLoad();

        const sprite = new env.FakeSprite(bitmap);
        sprite.setFrame(0, 3104, 32, 32);
        const firstBt = sprite.texture.baseTexture;

        // Bitmap resize changes backing dims
        bitmap.resize(512, 5000);
        sprite._refresh();

        assert.strictEqual(firstBt.destroyed, true, "Old owned texture must be destroyed on resize invalidation");
        assert.strictEqual(sprite.texture.baseTexture.destroyed, false, "New owned texture must be alive");
        assert.notStrictEqual(sprite.texture.baseTexture, firstBt, "Must produce a fresh owned texture after resize");
    }

    // 8. Bitmap swap on sprite releases old owned texture; swapping back produces fresh one.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const imageA = { width: 512, height: 4128 };
        const bitmapA = new env.FakeBitmap();
        bitmapA._image = imageA;
        bitmapA._onLoad();

        const imageB = { width: 512, height: 512 }; // normal fitting
        const bitmapB = new env.FakeBitmap();
        bitmapB._image = imageB;
        bitmapB._onLoad();

        const sprite = new env.FakeSprite(bitmapA);
        sprite.setFrame(0, 3104, 32, 32);
        const ownedBtA = sprite.texture.baseTexture;

        // Swap to fitting bitmap
        sprite.bitmap = bitmapB;
        sprite.setFrame(0, 0, 32, 32);
        assert.strictEqual(sprite.texture.baseTexture, bitmapB.baseTexture,
            "Swapping to normal bitmap must use normal baseTexture");
        assert.strictEqual(ownedBtA.destroyed, true, "Owned texture from bitmapA must be released on swap");

        // Swap back to oversized bitmap
        sprite.bitmap = bitmapA;
        sprite.setFrame(0, 3104, 32, 32);
        assert.strictEqual(sprite.texture.baseTexture.destroyed, false, "Fresh owned texture must be alive");
        assert.notStrictEqual(sprite.texture.baseTexture, ownedBtA, "Must create fresh owned texture, not reuse destroyed one");
    }

    // 9. Sprite destruction releases owned texture cleanly.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 512, height: 4128 };
        bitmap._onLoad();

        const sprite = new env.FakeSprite(bitmap);
        sprite.setFrame(0, 3104, 32, 32);
        const ownedBt = sprite.texture.baseTexture;

        sprite.destroy();
        assert.strictEqual(ownedBt.destroyed, true, "Sprite.destroy() must release its owned frame texture");
    }

    // 10. Frame larger than MAX_TEXTURE_SIZE is unsupported: logs UNSUPPORTED, falls back to engine.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 512, height: 4128 };
        bitmap._onLoad();

        const sprite = new env.FakeSprite(bitmap);
        // Request a 4128-tall frame on an oversized source
        sprite.setFrame(0, 0, 512, 4128);

        assert.strictEqual(sprite.texture.baseTexture, bitmap.baseTexture,
            "Frame exceeding GL limit must NOT be faked or silently clipped; stays on engine baseTexture");
        assert(env.errors.some((msg) => msg.includes("CAP-WEB-031") && msg.includes("frame-exceeds-limit")),
            "Must emit structured frame-exceeds-limit diagnostic");
    }

    // 11. Smooth flag propagation to owned frame texture.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 512, height: 4128 };
        bitmap.smooth = true;
        bitmap._onLoad();

        const sprite = new env.FakeSprite(bitmap);
        sprite.setFrame(0, 3104, 32, 32);
        assert.strictEqual(sprite.texture.baseTexture.scaleMode, 1, "Owned texture must inherit smooth=true (LINEAR=1)");

        bitmap.smooth = false;
        sprite._refresh();
        assert.strictEqual(sprite.texture.baseTexture.scaleMode, 0, "Owned texture must inherit smooth=false (NEAREST=0)");
    }

    // 12. Renderer limit unready at startup, then becomes ready on next refresh.
    {
        const env = createV2AtlasSandbox({ withRenderer: false });
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 512, height: 4128 };
        bitmap._onLoad();

        const sprite = new env.FakeSprite(bitmap);
        sprite.setFrame(0, 3104, 32, 32);
        assert.strictEqual(sprite.texture.baseTexture, bitmap.baseTexture,
            "Unready limit must leave sprite on engine baseTexture without throwing");

        // Renderer initializes later
        env.sandbox.Graphics = {
            app: {
                renderer: {
                    gl: {
                        MAX_TEXTURE_SIZE: 0x0d33,
                        getParameter: () => 4096
                    }
                }
            }
        };

        // Next refresh/setFrame must pick up the newly available limit.
        sprite._refresh();
        assert.notStrictEqual(sprite.texture.baseTexture, bitmap.baseTexture,
            "Once renderer limit is ready, subsequent refresh must attach owned frame texture");
        assert.strictEqual(sprite.texture.baseTexture.width, 32);
        assert.strictEqual(sprite.texture.baseTexture.height, 32);
    }

    // 13. Canvas allocation failure cleans up gracefully and stays on engine behavior.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096, failCanvasAt: 1 });
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 512, height: 4128 };
        bitmap._onLoad();

        const sprite = new env.FakeSprite(bitmap);
        sprite.setFrame(0, 3104, 32, 32);

        assert.strictEqual(sprite.texture.baseTexture, bitmap.baseTexture,
            "On canvas allocation failure, sprite must stay on engine baseTexture");
        assert(env.errors.some((msg) => msg.includes("CAP-WEB-031") && msg.includes("frame-build-failed")),
            "Must log structured frame-build-failed error");
    }

    // 14. Inert when PIXI is absent.
    {
        const env = createV2AtlasSandbox({ withPixi: false });
        vm.runInContext(shimsCode, env.sandbox);

        assert.strictEqual(env.FakeSprite.prototype._refresh.__rp_oversized_frame, undefined,
            "Sprite._refresh must NOT be patched when PIXI is absent");
        assert.deepStrictEqual(env.errors, [], "Missing PIXI must produce no errors");
    }

    // 15. MV colour-tint path (rpg_core.js:4200-4205) stays engine-owned: no shim texture.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 512, height: 4128 };
        bitmap._onLoad();

        const sprite = new env.FakeSprite(bitmap);
        sprite.setFrame(0, 3104, 32, 32);
        // Simulate MV tint: engine builds a frame-sized canvas and assigns it as the texture base.
        const tintCanvas = new env.FakeCanvas(32, 32);
        const tintBaseTexture = new env.sandbox.PIXI.BaseTexture(tintCanvas);
        tintBaseTexture.width = 32;
        tintBaseTexture.height = 32;
        sprite._tintTexture = tintBaseTexture;
        sprite.texture.baseTexture = tintBaseTexture;
        sprite.texture.frame = new env.sandbox.PIXI.Rectangle(0, 0, 32, 32);

        sprite._refresh();

        assert.strictEqual(sprite.__rpFrameTexture, undefined,
            "MV tint path must keep the engine's own frame-sized canvas, not a shim copy");
        assert.strictEqual(sprite.texture.baseTexture, tintBaseTexture,
            "MV tint baseTexture must be preserved exactly as the engine assigned it");
        assert.strictEqual(tintBaseTexture.destroyed, false, "Engine tint texture must not be destroyed by the shim");
    }

    // 16. MV accessor shape: _baseTexture is a getter over __baseTexture (no setter). The shim must
    //     only read it, never assign, so a getter-only property cannot be silently shadowed.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        const PIXI_ = env.sandbox.PIXI;

        function MvAccessorBitmap() {
            this.__baseTexture = null;
            this._image = null;
            this._smooth = false;
        }
        Object.defineProperty(MvAccessorBitmap.prototype, "_baseTexture", {
            configurable: true,
            get: function() { return this.__baseTexture; }
        });
        Object.defineProperty(MvAccessorBitmap.prototype, "baseTexture", {
            configurable: true,
            get: function() { return this.__baseTexture; }
        });
        Object.defineProperty(MvAccessorBitmap.prototype, "width", {
            configurable: true,
            get: function() { return this._image ? this._image.width : 0; }
        });
        Object.defineProperty(MvAccessorBitmap.prototype, "height", {
            configurable: true,
            get: function() { return this._image ? this._image.height : 0; }
        });
        MvAccessorBitmap.prototype._createBaseTexture = function(source) {
            this.__baseTexture = new PIXI_.BaseTexture(source);
            this.__baseTexture.width = source.width;
            this.__baseTexture.height = source.height;
        };

        env.sandbox.Bitmap = MvAccessorBitmap;
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new MvAccessorBitmap();
        bitmap._image = { width: 512, height: 4128 };
        bitmap._createBaseTexture(bitmap._image);
        const originalBaseTexture = bitmap.__baseTexture;

        const sprite = new env.FakeSprite(bitmap);
        assert.doesNotThrow(() => sprite.setFrame(0, 3104, 32, 32),
            "A getter-only MV _baseTexture must not break frame resolution");
        assert.strictEqual(bitmap.__baseTexture, originalBaseTexture,
            "Shim must never assign through the MV _baseTexture accessor");
        assert.strictEqual(bitmap.baseTexture.width, 512, "MV baseTexture width must stay 512");
        assert.strictEqual(bitmap.baseTexture.height, 4128, "MV baseTexture height must stay 4128");
        assert.notStrictEqual(sprite.texture.baseTexture, originalBaseTexture,
            "Sprite must still receive its own GPU-fit texture");
        assert.strictEqual(sprite.texture.baseTexture.width, 32);
    }

    // 17. Shipped PIXI 5.3.12 integration: exercises real BaseTexture.destroy() (sets resource=null,
    //     destroyed=true), real Texture.frame fit enforcement (throws on mismatch), real update(),
    //     and real sprite quad creation without mock assumptions. Gated on RUNTIMEPACK_PIXI_PATH /
    //     RUNTIMEPACK_GAME_ROOT so the suite runs cleanly anywhere while proving real engine fidelity
    //     whenever a genuine bundle is provided.
    {
        const realPixiPath = resolvePixiPath();
        if (!realPixiPath) {
            console.log("    [NOT_TESTED] Genuine PIXI bundle not found via RUNTIMEPACK_PIXI_PATH/RUNTIMEPACK_GAME_ROOT; skipping real PIXI 5.x integration");
        } else {
            const loaded = loadRealPixi(realPixiPath);
            assert(loaded.PIXI, "Genuine PIXI must export PIXI namespace");
            assert(loaded.PIXI.BaseTexture, "Genuine PIXI must export BaseTexture");
            assert(loaded.PIXI.Texture, "Genuine PIXI must export Texture");
            assert(loaded.PIXI.Rectangle, "Genuine PIXI must export Rectangle");
            assert(loaded.PIXI.Sprite, "Genuine PIXI must export Sprite");

            const env = createV2AtlasSandbox({
                maxTextureSize: 4096,
                withPixi: false,
                withEngineClasses: false
            });
            env.sandbox.PIXI = loaded.PIXI;
            env.sandbox.document = loaded.document;

            // Wire real PIXI classes into the MZ-style Bitmap and Sprite prototypes
            function RealPixiBitmap() {
                this._canvas = null;
                this._context = null;
                this._baseTexture = null;
                this._image = null;
                this._smooth = false;
            }
            RealPixiBitmap.prototype._createBaseTexture = function(source) {
                this._baseTexture = new loaded.PIXI.BaseTexture(source);
                this._baseTexture.mipmap = false;
                this._baseTexture.width = source.width;
                this._baseTexture.height = source.height;
            };
            RealPixiBitmap.prototype._onLoad = function() {
                this._createBaseTexture(this._image);
            };
            Object.defineProperty(RealPixiBitmap.prototype, "baseTexture", {
                configurable: true,
                get: function() { return this._baseTexture; }
            });
            Object.defineProperty(RealPixiBitmap.prototype, "width", {
                configurable: true,
                get: function() { return this._image ? this._image.width : 0; }
            });
            Object.defineProperty(RealPixiBitmap.prototype, "height", {
                configurable: true,
                get: function() { return this._image ? this._image.height : 0; }
            });

            function RealPixiSprite(bitmap) {
                this._bitmap = bitmap || null;
                this._frame = new loaded.PIXI.Rectangle();
                const emptyBt = new loaded.PIXI.BaseTexture();
                emptyBt.setSize(1, 1);
                this.texture = new loaded.PIXI.Texture(emptyBt, new loaded.PIXI.Rectangle());
                this.pivot = { x: 0, y: 0 };
            }
            RealPixiSprite.prototype.setFrame = function(x, y, w, h) {
                this._frame.x = x;
                this._frame.y = y;
                this._frame.width = w;
                this._frame.height = h;
                this._refresh();
            };
            RealPixiSprite.prototype.destroy = function() {
                this.destroyed = true;
            };
            RealPixiSprite.prototype._refresh = function() {
                const texture = this.texture;
                const fx = Math.floor(this._frame.x);
                const fy = Math.floor(this._frame.y);
                const fw = Math.floor(this._frame.width);
                const fh = Math.floor(this._frame.height);
                const base = this._bitmap ? this._bitmap.baseTexture : null;
                const bw = base ? base.width : 0;
                const bh = base ? base.height : 0;
                const rx = Math.min(Math.max(fx, 0), bw);
                const ry = Math.min(Math.max(fy, 0), bh);
                const rw = Math.min(Math.max(fw - rx + fx, 0), bw - rx);
                const rh = Math.min(Math.max(fh - ry + fy, 0), bh - ry);
                this.pivot.x = fx - rx;
                this.pivot.y = fy - ry;
                if (texture && base) {
                    texture.baseTexture = base;
                    try {
                        texture.frame = new loaded.PIXI.Rectangle(rx, ry, rw, rh);
                    } catch (_) {
                        texture.frame = new loaded.PIXI.Rectangle();
                    }
                    texture._updateID++;
                }
            };

            env.sandbox.Bitmap = RealPixiBitmap;
            env.sandbox.Sprite = RealPixiSprite;
            vm.runInContext(shimsCode, env.sandbox);

            const image = new loaded.HTMLImageElement();
            image.width = 512;
            image.height = 4128;
            const bitmap = new RealPixiBitmap();
            bitmap._image = image;
            bitmap._onLoad();

            // M1 guard under real PIXI: original BaseTexture is alive, retains 512x4128, valid=true
            const origBt = bitmap.baseTexture;
            assert.strictEqual(origBt.width, 512);
            assert.strictEqual(origBt.height, 4128);
            assert.strictEqual(origBt.destroyed, false);

            const sprite = new RealPixiSprite(bitmap);
            sprite.setFrame(0, 3104, 32, 32);

            // Verified against real PIXI internals:
            // 1. sprite.texture.baseTexture is the owned frame texture, NOT origBt
            assert.notStrictEqual(sprite.texture.baseTexture, origBt);
            assert.strictEqual(bitmap.baseTexture, origBt, "Bitmap baseTexture identity preserved under real PIXI");
            assert.strictEqual(sprite.texture.baseTexture.width, 32);
            assert.strictEqual(sprite.texture.baseTexture.height, 32);
            assert.strictEqual(sprite.texture.baseTexture.valid, true);
            assert.strictEqual(sprite.texture.baseTexture.destroyed, false);
            assert.strictEqual(sprite.texture.valid, true, "Real PIXI Texture must be marked valid");
            assert.strictEqual(sprite.texture.frame.width, 32);
            assert.strictEqual(sprite.texture.frame.height, 32);

            // 2. Real PIXI Texture.frame setter rejects out-of-fit frames (throws). Our frame MUST fit.
            assert.doesNotThrow(() => {
                sprite.texture.frame = new loaded.PIXI.Rectangle(0, 0, 32, 32);
            }, "Real PIXI Texture.frame assignment must fit inside owned BaseTexture dimensions");

            // 3. Real cross-boundary frame under genuine PIXI
            sprite.setFrame(0, 4080, 32, 32);
            assert.strictEqual(sprite.texture.baseTexture.height, 32, "Cross-boundary frame must retain full 32px height under real PIXI");
            assert.strictEqual(sprite.texture.frame.height, 32);

            // 4. Real destruction releases owned BaseTexture: real PIXI sets destroyed=true and resource=null
            const ownedBt = sprite.texture.baseTexture;
            sprite.destroy();
            assert.strictEqual(ownedBt.destroyed, true, "Real PIXI BaseTexture.destroy() must set destroyed=true");
            assert.strictEqual(ownedBt.resource, null, "Real PIXI BaseTexture.destroy() must clear resource");

            // 5. Queue/drain path under real PIXI: byte-budget refusal, bounded retry, and real
            //    Texture.frame fit enforcement on the granted texture (review L5.4).
            {
                const bigImage = new loaded.HTMLImageElement();
                bigImage.width = 4096;
                bigImage.height = 8192;
                const bigBitmap = new RealPixiBitmap();
                bigBitmap._image = bigImage;
                bigBitmap._onLoad();
                assert.strictEqual(bigBitmap.baseTexture.height, 8192);

                const holder = new RealPixiSprite(bigBitmap);
                holder.setFrame(0, 0, 4096, 1536); // 24 MiB owned
                assert.strictEqual(holder.texture.baseTexture.width, 4096);

                const refusedSprite = new RealPixiSprite(bigBitmap);
                refusedSprite.setFrame(0, 2048, 4096, 1536); // 24 MiB -> byte-cap refusal
                assert.strictEqual(refusedSprite.texture.baseTexture, bigBitmap.baseTexture,
                    "Real PIXI: byte-cap refusal must keep the engine base texture");
                assert.strictEqual(env.sandbox.RuntimePack.oversizedFrameStats().pending, 1,
                    "Real PIXI: refused sprite must be queued for retry");

                holder.setFrame(0, 0, 32, 32); // release 24 MiB -> drains the queue

                const statsAfterDrain = env.sandbox.RuntimePack.oversizedFrameStats();
                assert.strictEqual(statsAfterDrain.pending, 0, "Real PIXI: release must drain the queue");
                assert.notStrictEqual(refusedSprite.texture.baseTexture, bigBitmap.baseTexture,
                    "Real PIXI: drained sprite must own a GPU-fit texture");
                assert.strictEqual(refusedSprite.texture.baseTexture.width, 4096);
                assert.strictEqual(refusedSprite.texture.baseTexture.height, 1536);
                assert.strictEqual(refusedSprite.texture.baseTexture.destroyed, false);
                assert.strictEqual(refusedSprite.texture.valid, true,
                    "Real PIXI: granted Texture must be valid after the drain assignment");
                assert.doesNotThrow(() => {
                    refusedSprite.texture.frame = new loaded.PIXI.Rectangle(0, 0, 4096, 1536);
                }, "Real PIXI: granted frame must fit the owned BaseTexture");

                // Destroying a QUEUED sprite outside the shim hook must not leak an owned texture:
                // PIXI 5.3.12 has no DisplayObject.destroyed, and a destroyed/reclaimed sprite loses
                // its texture, which is the signal the drain consults.
                const orphan = new RealPixiSprite(bigBitmap);
                orphan.setFrame(0, 0, 4096, 1536); // 24 MiB with ~24 MiB already owned -> refused
                assert.strictEqual(orphan.texture.baseTexture, bigBitmap.baseTexture);
                assert.strictEqual(env.sandbox.RuntimePack.oversizedFrameStats().pending, 1,
                    "Real PIXI: second refusal must be queued");
                orphan.texture = null; // destroyed/reclaimed without the shim's dequeue

                refusedSprite.destroy(); // releases 24 MiB and triggers a drain

                const statsAfterOrphan = env.sandbox.RuntimePack.oversizedFrameStats();
                assert.strictEqual(statsAfterOrphan.pending, 0,
                    "Real PIXI: drain must dequeue a destroyed queued sprite");
                assert.strictEqual(statsAfterOrphan.owned, 1,
                    "Real PIXI: destroyed queued sprite must not be granted a texture");
                assert.strictEqual(statsAfterOrphan.bytes, 32 * 32 * 4,
                    "Real PIXI: destroyed queued sprite must not consume byte accounting");
                assert.strictEqual(orphan.__rpFrameTexture, undefined);
            }

            console.log(`    Genuine PIXI ${loaded.version} integration tests PASSED`);
        }
    }

    // 18. Count ceiling regression: more than the former OVERSIZED_FRAME_MAX_TEXTURES=32 distinct
    //     32x32 oversized-frame consumers must all receive owned textures. On device the 33rd
    //     requester was refused at owned=32 bytes=131072 of a 33554432 byte budget, leaving a
    //     sprite on the 512x4128 base texture and rendering it opaque black. 40 distinct 32x32
    //     frames (40 * 4096 = 163840 bytes) are far inside the byte budget, so none may be refused.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 512, height: 4128 };
        bitmap._onLoad();

        const sprites = [];
        for (let i = 0; i < 40; i++) {
            const sprite = new env.FakeSprite(bitmap);
            sprite.setFrame(0, i * 32, 32, 32);
            sprites.push(sprite);
        }

        sprites.forEach((sprite, i) => {
            assert.notStrictEqual(sprite.texture.baseTexture, bitmap.baseTexture,
                `Distinct 32x32 consumer ${i} must receive an owned GPU-fit texture above the former 32 ceiling`);
            assert.strictEqual(sprite.texture.baseTexture.width, 32,
                `Consumer ${i} owned texture must be 32 wide`);
            assert.strictEqual(sprite.texture.baseTexture.height, 32,
                `Consumer ${i} owned texture must be 32 tall`);
        });

        const stats = env.sandbox.RuntimePack.oversizedFrameStats();
        assert.strictEqual(stats.owned, 40, "All 40 owned frame textures must be accounted for");
        assert.strictEqual(stats.bytes, 40 * 32 * 32 * 4, "Byte accounting must be 40 * 4096 bytes");
        assert.strictEqual(stats.pending, 0, "No consumer may be left queued when memory allows the copy");
        assert(!env.errors.some((msg) => msg.includes("CAP-WEB-031") && msg.includes("frame-cache-full")),
            "No frame-cache-full refusal may occur inside the byte budget for 32x32 consumers");
    }

    // 19. Byte budget is the real bound: a refusal caused by bytes must say so, stay on the engine
    //     path (no fake success) and allocate nothing. Frame 4096x4096 = 67108864 bytes exceeds the
    //     33554432 byte budget on its own while fitting MAX_TEXTURE_SIZE.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 4096, height: 8192 };
        bitmap._onLoad();

        const sprite = new env.FakeSprite(bitmap);
        sprite.setFrame(0, 0, 4096, 4096);

        assert.strictEqual(sprite.texture.baseTexture, bitmap.baseTexture,
            "Byte-cap refusal must leave the engine path untouched (no fake success)");
        assert(env.errors.some((msg) => msg.includes("CAP-WEB-031") && msg.includes("frame-cache-full") &&
            msg.includes("\"reason\":\"bytes\"")),
            "Byte-cap refusal must be labelled reason=bytes in the structured diagnostic");
        assert(!env.errors.some((msg) => msg.includes("\"reason\":\"count\"")),
            "A byte-cap refusal must not be reported as a count-cap refusal");

        const stats = env.sandbox.RuntimePack.oversizedFrameStats();
        assert.strictEqual(stats.owned, 0, "Refused allocation must not count as owned");
        assert.strictEqual(stats.bytes, 0, "Refused allocation must not consume byte budget");
    }

    // 20. Count-cap refusal is labelled distinctly and the sprite is retried deterministically once
    //     a slot is released. The ceiling is pinned to the documented policy value so a regression to
    //     a smaller integer (e.g. 32/64/1024) cannot pass by merely "moving" the ceiling.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const maxTextures = env.sandbox.RuntimePack.oversizedFrameStats().maxTextures;
        assert.strictEqual(maxTextures, 2048,
            "Documented count ceiling must be 2048 (backstop; the 32 MiB byte budget is primary)");

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 512, height: 4128 };
        bitmap._onLoad();

        const holders = [];
        for (let i = 0; i < maxTextures; i++) {
            const holder = new env.FakeSprite(bitmap);
            holder.setFrame(0, (i % 128) * 32, 32, 32);
            holders.push(holder);
        }
        assert.strictEqual(env.sandbox.RuntimePack.oversizedFrameStats().owned, maxTextures,
            "Precondition: the count ceiling must be reachable by icon-sized consumers");

        const refused = new env.FakeSprite(bitmap);
        refused.setFrame(0, 3104, 32, 32);

        assert.strictEqual(refused.texture.baseTexture, bitmap.baseTexture,
            "Count-cap refusal must leave the engine path untouched (honest failure, no truncation)");
        assert(env.errors.some((msg) => msg.includes("CAP-WEB-031") && msg.includes("frame-cache-full") &&
            msg.includes("\"reason\":\"count\"")),
            "Count-cap refusal must be labelled reason=count");
        assert.strictEqual(env.sandbox.RuntimePack.oversizedFrameStats().pending, 1,
            "Refused sprite must be queued for a bounded retry");

        holders[0].destroy();

        const statsAfter = env.sandbox.RuntimePack.oversizedFrameStats();
        assert.strictEqual(statsAfter.pending, 0, "Releasing a slot must drain the retry queue");
        assert.strictEqual(statsAfter.owned, maxTextures, "The retried sprite must take the released slot");
        assert.notStrictEqual(refused.texture.baseTexture, bitmap.baseTexture,
            "Queued sprite must receive an owned texture after capacity is released");
        assert.strictEqual(refused.texture.baseTexture.width, 32);
        assert.strictEqual(refused.texture.baseTexture.height, 32);
    }

    // 21. Release by frame change drains the queue: a sprite refused under the byte budget must be
    //     retried when the holder shrinks its frame (the shim releases the old owned copy).
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 4096, height: 8192 };
        bitmap._onLoad();

        const holder = new env.FakeSprite(bitmap);
        holder.setFrame(0, 0, 4096, 1536); // 25165824 bytes (24 MiB)
        assert.strictEqual(env.sandbox.RuntimePack.oversizedFrameStats().bytes, 4096 * 1536 * 4);

        const refused = new env.FakeSprite(bitmap);
        refused.setFrame(0, 2048, 4096, 1536); // 24 MiB again: 48 MiB > 32 MiB budget
        assert.strictEqual(refused.texture.baseTexture, bitmap.baseTexture,
            "Second 24 MiB frame must be refused by the byte budget");
        assert(env.errors.some((msg) => msg.includes("frame-cache-full") && msg.includes("\"reason\":\"bytes\"")));
        assert.strictEqual(env.sandbox.RuntimePack.oversizedFrameStats().pending, 1);

        // Frame change on the holder releases its 24 MiB owned copy.
        holder.setFrame(0, 0, 32, 32);

        const stats = env.sandbox.RuntimePack.oversizedFrameStats();
        assert.strictEqual(stats.pending, 0, "Frame-change release must drain the pending retry");
        assert.strictEqual(stats.owned, 2, "Holder (32x32) plus retried frame must both be owned");
        assert.notStrictEqual(refused.texture.baseTexture, bitmap.baseTexture,
            "Retried sprite must hold an owned GPU-fit texture after the release");
        assert.strictEqual(refused.texture.baseTexture.width, 4096);
        assert.strictEqual(refused.texture.baseTexture.height, 1536);
    }

    // 22. Repeated refresh of a refused sprite must not re-allocate, must not grow the queue and must
    //     not spam diagnostics; the single drain after the release builds exactly one texture.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 4096, height: 8192 };
        bitmap._onLoad();

        const holder = new env.FakeSprite(bitmap);
        holder.setFrame(0, 0, 4096, 1536);

        const refused = new env.FakeSprite(bitmap);
        refused.setFrame(0, 2048, 4096, 1536);

        const texturesBefore = env.createdBaseTextures.length;
        for (let i = 0; i < 5; i++) {
            refused._refresh();
        }

        assert.strictEqual(env.createdBaseTextures.length, texturesBefore,
            "Refused sprite refreshes must allocate no BaseTexture");
        assert.strictEqual(env.sandbox.RuntimePack.oversizedFrameStats().pending, 1,
            "Repeated refusal must not duplicate the queue entry");
        assert.strictEqual(
            env.errors.filter((msg) => msg.includes("frame-cache-full")).length, 1,
            "Repeated refusal must reuse the deduplicated diagnostic");

        holder.destroy();

        assert.strictEqual(env.createdBaseTextures.length, texturesBefore + 1,
            "The single drain after release must build exactly one texture");
        assert.strictEqual(env.sandbox.RuntimePack.oversizedFrameStats().pending, 0);
        assert.strictEqual(refused.texture.baseTexture.destroyed, false,
            "Drained sprite must hold a live owned texture");
    }

    // 23. A QUEUED sprite that is destroyed before any release must not keep a slot: the destroy hook
    //     dequeues it, allocates nothing, and leaves the counters untouched (review M2 regression
    //     guard for the strong references held by the retry queue).
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 4096, height: 8192 };
        bitmap._onLoad();

        const holder = new env.FakeSprite(bitmap);
        holder.setFrame(0, 0, 4096, 1536); // 24 MiB owned

        const queued = new env.FakeSprite(bitmap);
        queued.setFrame(0, 2048, 4096, 1536); // refused, queued
        assert.strictEqual(env.sandbox.RuntimePack.oversizedFrameStats().pending, 1,
            "Precondition: refused sprite is queued");

        const texturesBefore = env.createdBaseTextures.length;
        const statsBefore = env.sandbox.RuntimePack.oversizedFrameStats();
        queued.destroy();

        const statsAfter = env.sandbox.RuntimePack.oversizedFrameStats();
        assert.strictEqual(statsAfter.pending, 0, "Destroying a queued sprite must empty the queue");
        assert.strictEqual(statsAfter.owned, statsBefore.owned,
            "Destroying a queued sprite must not change the owned count");
        assert.strictEqual(statsAfter.bytes, statsBefore.bytes,
            "Destroying a queued sprite must not change the byte accounting");
        assert.strictEqual(env.createdBaseTextures.length, texturesBefore,
            "Destroying a queued sprite must allocate nothing");
    }

    // 24. A queued sprite destroyed outside the patched hook (plugin/pooled sprite) must not be granted a
    //     texture by a later drain: the drain skips and dequeues it, so _rpOversizedOwned/_rpOversizedBytes
    //     cannot drift upward and no GL texture is leaked. PIXI 5.3.12 exposes the destroy flag as the
    //     protected `_destroyed` (pixi.js:7544) and Sprite.destroy() drops the texture, so both are used
    //     as the real signals; `destroyed` is only a BaseTexture flag and is never set on a DisplayObject.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 4096, height: 8192 };
        bitmap._onLoad();

        const holder = new env.FakeSprite(bitmap);
        holder.setFrame(0, 0, 4096, 1536);

        const orphan = new env.FakeSprite(bitmap);
        orphan.setFrame(0, 2048, 4096, 1536); // refused -> queued
        assert.strictEqual(env.sandbox.RuntimePack.oversizedFrameStats().pending, 1);

        // Simulate a destroy that bypasses the shim patch (e.g. the plugin captured the original
        // destroy before the shim installed). PIXI 5.3.12 marks the object with `_destroyed`.
        orphan._destroyed = true;
        assert.strictEqual(orphan.__rpOversizedPending, true,
            "Precondition: the sprite is still queued with the shim unaware of the destroy");

        const texturesBefore = env.createdBaseTextures.length;
        const statsBefore = env.sandbox.RuntimePack.oversizedFrameStats();

        // Any release triggers a drain; the drain must refuse to allocate for the dead sprite.
        holder.setFrame(0, 0, 32, 32);

        const statsAfter = env.sandbox.RuntimePack.oversizedFrameStats();
        assert.strictEqual(statsAfter.pending, 0, "Drain must dequeue a destroyed sprite");
        assert.strictEqual(statsAfter.owned, 1, "Only the holder's new 32x32 texture may be owned");
        assert.strictEqual(statsAfter.bytes, 32 * 32 * 4,
            "Byte accounting must not include a texture for the destroyed sprite");
        assert.strictEqual(env.createdBaseTextures.length, texturesBefore + 1,
            "Drain must allocate only the holder's replacement texture");
        assert.strictEqual(orphan.__rpFrameTexture, undefined,
            "Destroyed sprite must not be granted an owned texture");
    }

    // 25. Assign-failure path must restore the previous entry and release the newly built one so the
    //     accounting stays exact and the previously owned texture stays reachable (review L1).
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 512, height: 4128 };
        bitmap._onLoad();

        const sprite = new env.FakeSprite(bitmap);
        sprite.setFrame(0, 3104, 32, 32);
        const previousEntry = sprite.__rpFrameTexture;
        const previousBaseTexture = sprite.texture.baseTexture;
        assert(previousEntry, "Precondition: sprite owns a frame texture");

        // Reject only the shim's re-assignment: the engine refresh still writes the engine base
        // texture, but any attempt to point the texture at an owned copy throws (frozen/instrumented
        // Texture, as a plugin could do).
        const engineBaseTexture = bitmap.baseTexture;
        Object.defineProperty(sprite.texture, "baseTexture", {
            configurable: true,
            get() { return engineBaseTexture; },
            set(value) {
                if (value !== engineBaseTexture) {
                    throw new Error("frozen texture: baseTexture reassignment denied");
                }
            }
        });

        const texturesBefore = env.createdBaseTextures.length;
        sprite.setFrame(0, 3008, 32, 32);

        const stats = env.sandbox.RuntimePack.oversizedFrameStats();
        assert.strictEqual(sprite.__rpFrameTexture, previousEntry,
            "Assign failure must restore the previous ownership record");
        assert.strictEqual(previousBaseTexture.destroyed, false,
            "The previously owned texture must stay alive (still reachable via the sprite)");
        assert.strictEqual(stats.owned, 1, "Assign failure must not change the owned count");
        assert.strictEqual(stats.bytes, 32 * 32 * 4, "Assign failure must not change the byte accounting");
        assert.strictEqual(env.createdBaseTextures.length, texturesBefore + 1,
            "Only the discarded new texture may have been allocated");
        assert(env.errors.some((msg) => msg.includes("CAP-WEB-031") && msg.includes("frame-assign-failed")),
            "Assign failure must stay reported (no silent success)");
    }

    // 26. Queue bound: the retry queue is capped (OVERSIZED_FRAME_MAX_PENDING = 64). Beyond the bound
    //     refusals are dropped from retry but never granted textures; one release drains exactly one
    //     waiting sprite and the rest stay queued.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const maxTextures = env.sandbox.RuntimePack.oversizedFrameStats().maxTextures;
        assert.strictEqual(maxTextures, 2048, "Precondition: documented count ceiling");

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 512, height: 4128 };
        bitmap._onLoad();

        const holders = [];
        for (let i = 0; i < maxTextures; i++) {
            const holder = new env.FakeSprite(bitmap);
            holder.setFrame(0, (i % 128) * 32, 32, 32);
            holders.push(holder);
        }

        const refused = [];
        for (let i = 0; i < 70; i++) {
            const sprite = new env.FakeSprite(bitmap);
            sprite.setFrame(0, 3104, 32, 32);
            refused.push(sprite);
        }

        assert.strictEqual(env.sandbox.RuntimePack.oversizedFrameStats().pending, 64,
            "Retry queue must be capped at OVERSIZED_FRAME_MAX_PENDING = 64");
        refused.forEach((sprite, i) => {
            assert.strictEqual(sprite.__rpFrameTexture, undefined,
                `Refused sprite ${i} must not own a texture`);
            assert.strictEqual(sprite.texture.baseTexture, bitmap.baseTexture,
                `Refused sprite ${i} must stay on the engine path (no fake success)`);
        });
        assert.strictEqual(env.sandbox.RuntimePack.oversizedFrameStats().owned, maxTextures,
            "Refusals beyond the queue bound must not allocate");

        const cappedStats = env.sandbox.RuntimePack.oversizedFrameStats();
        assert.strictEqual(cappedStats.registered, maxTextures,
            "Registry must equal the owned entries at the ceiling");
        assert(cappedStats.registered <= cappedStats.maxTextures,
            "Registry must never exceed the texture ceiling");

        holders[0].destroy();

        const afterOneRelease = env.sandbox.RuntimePack.oversizedFrameStats();
        assert.strictEqual(afterOneRelease.owned, maxTextures, "One release must serve one queued sprite");
        assert.strictEqual(afterOneRelease.pending, 63, "One release must drain exactly one queued sprite");
        assert.notStrictEqual(refused[0].texture.baseTexture, bitmap.baseTexture,
            "The first queued sprite must be served by the drain");
        assert.strictEqual(refused[1].texture.baseTexture, bitmap.baseTexture,
            "The second queued sprite must still wait for capacity");
    }

    // 27. Registry of sprites holding owned frame textures: it tracks exactly the owned entries, and it
    //     is bounded by the texture ceiling (a strong-ref registry is allowed only because it can never
    //     exceed OVERSIZED_FRAME_MAX_TEXTURES). Reads `RuntimePack.oversizedFrameStats().registered`.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 512, height: 4128 };
        bitmap._onLoad();

        const sprites = [];
        for (let i = 0; i < 40; i++) {
            const sprite = new env.FakeSprite(bitmap);
            sprite.setFrame(0, i * 32, 32, 32);
            sprites.push(sprite);
        }

        let stats = env.sandbox.RuntimePack.oversizedFrameStats();
        assert.strictEqual(stats.maxTextures, 2048, "Documented count ceiling stays 2048");
        assert.strictEqual(stats.owned, 40);
        assert.strictEqual(stats.registered, 40, "Registry must track every sprite holding an owned texture");
        assert(stats.registered <= stats.maxTextures, "Registry must be bounded by the texture ceiling");

        for (let i = 0; i < 10; i++) {
            sprites[i].destroy();
        }

        stats = env.sandbox.RuntimePack.oversizedFrameStats();
        assert.strictEqual(stats.owned, 30, "Releases must decrement the owned count");
        assert.strictEqual(stats.registered, 30, "Registry must drop released sprites");
        assert.strictEqual(stats.registered, stats.owned, "Registry size must equal owned entries");
    }

    // 28. Scene-termination purge: only sprites descending from the terminating scene lose their owned
    //     texture. Current-scene sprites and sprites with no determinable ancestry are untouched.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        const sm = {
            _scene: null,
            _nextScene: null,
            onSceneTerminate: function() { this.__called = (this.__called || 0) + 1; }
        };
        env.sandbox.SceneManager = sm;
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 512, height: 4128 };
        bitmap._onLoad();

        const battle = new env.FakeContainer("Scene_Battle");
        const battleLayer = new env.FakeContainer("WindowLayer");
        battle.addChild(battleLayer);
        const map = new env.FakeContainer("Scene_Map");

        const battleA = new env.FakeSprite(bitmap);
        battleLayer.addChild(battleA);
        battleA.setFrame(0, 3104, 32, 32);

        const battleB = new env.FakeSprite(bitmap);
        battle.addChild(battleB);
        battleB.setFrame(0, 3008, 32, 32);

        const mapSprite = new env.FakeSprite(bitmap);
        map.addChild(mapSprite);
        mapSprite.setFrame(0, 2944, 32, 32);

        const detached = new env.FakeSprite(bitmap);
        detached.setFrame(0, 2880, 32, 32); // no parent: ancestry cannot be determined

        const battleABase = battleA.texture.baseTexture;
        const battleBBase = battleB.texture.baseTexture;
        const mapBase = mapSprite.texture.baseTexture;
        const detachedBase = detached.texture.baseTexture;
        assert.strictEqual(env.sandbox.RuntimePack.oversizedFrameStats().owned, 4);
        assert.strictEqual(env.sandbox.RuntimePack.oversizedFrameStats().registered, 4);

        sm._scene = battle; // changeScene: terminate() then onSceneTerminate(), _scene not yet replaced
        sm.onSceneTerminate.call(sm);

        assert.strictEqual(battleA.__rpFrameTexture, undefined, "Nested battle sprite must be released");
        assert.strictEqual(battleB.__rpFrameTexture, undefined, "Direct battle sprite must be released");
        assert.strictEqual(battleABase.destroyed, true);
        assert.strictEqual(battleBBase.destroyed, true);
        assert.strictEqual(battleA.texture.baseTexture, bitmap.baseTexture,
            "Released sprite must fall back to the engine base texture, never a destroyed one");
        assert.strictEqual(mapSprite.__rpFrameTexture !== undefined, true,
            "Current-scene (map) sprite must keep its owned texture");
        assert.strictEqual(mapBase.destroyed, false, "Current-scene texture must stay alive");
        assert.strictEqual(detached.__rpFrameTexture !== undefined, true,
            "Sprite without determinable ancestry must not be released");

        const stats = env.sandbox.RuntimePack.oversizedFrameStats();
        assert.strictEqual(stats.owned, 2, "Only the two battle sprites may be released");
        assert.strictEqual(stats.bytes, 2 * 32 * 32 * 4, "Byte accounting must match the two survivors");
        assert.strictEqual(stats.registered, 2, "Registry must keep the two survivors");
        assert.strictEqual(sm.__called, 1, "The engine's own onSceneTerminate must still run");
        assert(env.errors.some((msg) => msg.includes("CAP-WEB-031") && msg.includes("scene-purge") &&
            msg.includes("\"released\":2")),
            "Scene purge must emit one structured diagnostic with the released count");
        assert.strictEqual(detachedBase.destroyed, false);
    }

    // 29. Idempotent hook and no double release: a repeated termination for the same scene and repeated
    //     installer runs must not release anything twice (FakeBaseTexture.destroy throws on a second
    //     destroy, so a double release would surface as a thrown error).
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        const sm = {
            _scene: null,
            _nextScene: null,
            onSceneTerminate: function() {}
        };
        env.sandbox.SceneManager = sm;
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 512, height: 4128 };
        bitmap._onLoad();

        const scene = new env.FakeContainer("Scene_Battle");
        const sprite = new env.FakeSprite(bitmap);
        scene.addChild(sprite);
        sprite.setFrame(0, 3104, 32, 32);

        const hookAfterInstall = sm.onSceneTerminate;
        assert.strictEqual(hookAfterInstall.__rp_oversized_scene_purge, true,
            "The terminate hook must be marked as installed");
        env.sandbox.RuntimePack.installOversizedSpriteFrames();
        env.sandbox.RuntimePack.installOversizedSpriteFrames();
        assert.strictEqual(sm.onSceneTerminate, hookAfterInstall,
            "Repeated installer runs must not wrap the terminate hook again");

        sm._scene = scene;
        sm.onSceneTerminate.call(sm);
        const afterFirst = env.sandbox.RuntimePack.oversizedFrameStats();
        assert.strictEqual(afterFirst.owned, 0);
        assert.strictEqual(afterFirst.registered, 0);

        assert.doesNotThrow(() => sm.onSceneTerminate.call(sm),
            "Second termination must not re-destroy an already released texture");
        const afterSecond = env.sandbox.RuntimePack.oversizedFrameStats();
        assert.strictEqual(afterSecond.owned, 0, "Second termination must not change the counter");
        assert.strictEqual(afterSecond.bytes, 0, "Second termination must not change the byte total");
    }

    // 30. Battle -> map -> battle cycle: the owned count must return to the pre-battle level instead of
    //     accumulating (~90 per battle on device because the engine never destroys the retired scene).
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        const sm = {
            _scene: null,
            _nextScene: null,
            onSceneTerminate: function() {}
        };
        env.sandbox.SceneManager = sm;
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 512, height: 4128 };
        bitmap._onLoad();

        const baseline = 5;
        const battleIcons = 90;
        const map = new env.FakeContainer("Scene_Map");
        for (let i = 0; i < baseline; i++) {
            const sprite = new env.FakeSprite(bitmap);
            map.addChild(sprite);
            sprite.setFrame(0, (i % 32) * 32, 32, 32);
        }
        sm._scene = map;

        const preBattle = env.sandbox.RuntimePack.oversizedFrameStats();
        assert.strictEqual(preBattle.owned, baseline);
        const baselineBytes = preBattle.bytes;

        for (let cycle = 0; cycle < 3; cycle++) {
            const battle = new env.FakeContainer("Scene_Battle");
            const layer = new env.FakeContainer("Spriteset");
            battle.addChild(layer);
            for (let i = 0; i < battleIcons; i++) {
                const sprite = new env.FakeSprite(bitmap);
                layer.addChild(sprite);
                sprite.setFrame(0, (i % 128) * 32, 32, 32);
            }
            sm._scene = battle;
            const duringBattle = env.sandbox.RuntimePack.oversizedFrameStats();
            assert.strictEqual(duringBattle.owned, baseline + battleIcons,
                `Cycle ${cycle}: battle sprites must be owned while the scene is live`);

            sm.onSceneTerminate.call(sm); // engine: terminate() -> onSceneTerminate()
            sm._scene = map;              // engine: _scene = _nextScene

            const afterBattle = env.sandbox.RuntimePack.oversizedFrameStats();
            assert.strictEqual(afterBattle.owned, baseline,
                `Cycle ${cycle}: owned must return to the pre-battle level, not accumulate`);
            assert.strictEqual(afterBattle.bytes, baselineBytes,
                `Cycle ${cycle}: bytes must return to the pre-battle level`);
            assert.strictEqual(afterBattle.registered, baseline,
                `Cycle ${cycle}: registry must return to the pre-battle level`);
        }
    }

    // 31. Dead retry-queue slot: a queued sprite whose texture is gone (real signal: PIXI 5.3.12 has no
    //     DisplayObject.destroyed; the resolve bail on `!texture` and the drain guard are the real
    //     signals) is dequeued without allocating and without drifting the counters. Both paths are
    //     covered: a direct refresh (resolve bail) and a drain triggered by another release.
    {
        const env = createV2AtlasSandbox({ maxTextureSize: 4096 });
        vm.runInContext(shimsCode, env.sandbox);

        const bitmap = new env.FakeBitmap();
        bitmap._image = { width: 4096, height: 8192 };
        bitmap._onLoad();

        const holder = new env.FakeSprite(bitmap);
        holder.setFrame(0, 0, 4096, 1536); // 24 MiB

        // A. Direct refresh of a queued sprite whose texture is gone.
        const dead = new env.FakeSprite(bitmap);
        dead.setFrame(0, 2048, 4096, 1536); // refused -> queued
        assert.strictEqual(env.sandbox.RuntimePack.oversizedFrameStats().pending, 1);
        dead.texture = null;
        const texturesBeforeRefresh = env.createdBaseTextures.length;
        dead._refresh();
        assert.strictEqual(env.sandbox.RuntimePack.oversizedFrameStats().pending, 0,
            "Resolve must dequeue a sprite whose texture is gone");
        assert.strictEqual(env.createdBaseTextures.length, texturesBeforeRefresh,
            "Resolve of a dead queue slot must allocate nothing");
        assert.strictEqual(dead.__rpFrameTexture, undefined);

        // B. A drain triggered by another release must reclaim the dead slot.
        const dead2 = new env.FakeSprite(bitmap);
        dead2.setFrame(0, 1024, 4096, 1536); // refused -> queued
        assert.strictEqual(env.sandbox.RuntimePack.oversizedFrameStats().pending, 1);
        dead2.texture = null;
        const texturesBeforeDrain = env.createdBaseTextures.length;
        holder.destroy();

        const stats = env.sandbox.RuntimePack.oversizedFrameStats();
        assert.strictEqual(stats.pending, 0, "Drain must reclaim a queue slot whose texture is gone");
        assert.strictEqual(stats.owned, 0, "Dead queue slot must not be granted a texture");
        assert.strictEqual(stats.bytes, 0, "Dead queue slot must not consume byte accounting");
        assert.strictEqual(env.createdBaseTextures.length, texturesBeforeDrain,
            "Reclaiming a dead queue slot must allocate nothing");
    }

    console.log("Oversized atlas frame rendering (CAP-WEB-031 v2) tests PASSED");
}

console.log("All Mobile Web Shims tests passed successfully!");
