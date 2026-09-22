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
    vm.runInContext(shimsCode, sb);
    assert.strictEqual(sb.__runtimepack_shims_installed, true, "__runtimepack_shims_installed must be true");

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

console.log("All Mobile Web Shims tests passed successfully!");
