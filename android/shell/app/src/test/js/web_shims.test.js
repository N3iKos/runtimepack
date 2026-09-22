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
        _warned: warned
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
    }, "executeCheatCmd must safely no-op when game globals are undefined");
}

console.log("All Mobile Web Shims tests passed successfully!");
