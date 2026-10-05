const assert = require("assert");
const fs = require("fs");
const path = require("path");
const vm = require("vm");

const shimPath = path.resolve(__dirname, "../../main/assets/web_shims/runtimepack_web_shims.js");
const shimCode = fs.readFileSync(shimPath, "utf8");

function createSandbox() {
    const warnings = [];
    const sandbox = {
        console: {
            log: console.log,
            error: console.error,
            warn: (msg) => { warnings.push(String(msg)); }
        },
        _warnings: warnings,
        setTimeout,
        clearTimeout,
        setInterval,
        clearInterval,
        Uint8Array,
        ArrayBuffer,
        TextEncoder,
        TextDecoder
    };
    sandbox.window = sandbox;
    sandbox.global = sandbox;
    sandbox.self = sandbox;
    vm.createContext(sandbox);
    return sandbox;
}

const ctx = createSandbox();

function setupAuthenticVnmScripts(sandbox) {
    sandbox.vn = {};
    function Component_MessageBehavior() {
        this.voice = null;
        this.settings = {
            autoMessage: {
                waitForVoice: false,
                timeMessageToVoice: false
            }
        };
        this.tempSettings = {
            skip: false
        };
    }
    Component_MessageBehavior.prototype.voiceDuration = function() {
        var duration;
        duration = 0;
        if ((this.voice != null) && (this.settings.autoMessage.waitForVoice || this.settings.timeMessageToVoice)) {
            duration = Math.round((this.voice.source.buffer.duration * (1.0 / this.voice.source.playbackRate.value)) * 1000 / 16.6);
        } else {
            duration = 0;
        }
        if (this.tempSettings.skip) {
            return 1;
        } else {
            return duration;
        }
    };
    sandbox.vn.Component_MessageBehavior = Component_MessageBehavior;
}

vm.runInContext(shimCode, ctx);
assert(typeof ctx.window.Buffer === "function", "BEFOREENGINE: Buffer must be available before engine");
assert(typeof ctx.window.nw === "object", "BEFOREENGINE: nw must be available before engine");

setupAuthenticVnmScripts(ctx);

assert(typeof ctx.window.__runtimepackPollVnmMessageCompat === "function", "AFTERSCRIPTS: __runtimepackPollVnmMessageCompat hook must be exposed");

ctx.window.__runtimepackPollVnmMessageCompat();
assert(ctx.vn.Component_MessageBehavior.prototype.voiceDuration.__runtimepack_voice_patched, "voiceDuration must be patched");

const origWrapped = ctx.vn.Component_MessageBehavior.prototype.voiceDuration;
ctx.window.__runtimepackPollVnmMessageCompat();
assert.strictEqual(ctx.vn.Component_MessageBehavior.prototype.voiceDuration, origWrapped, "patch must be idempotent");

const msg = new ctx.vn.Component_MessageBehavior();
assert.strictEqual(msg.voiceDuration(), 0, "No voice returns 0");

msg.voice = {
    source: {
        buffer: { duration: 2.5 },
        playbackRate: { value: 1.0 }
    }
};
msg.settings.autoMessage.waitForVoice = true;
assert.strictEqual(msg.voiceDuration(), 151, "Valid voice returns exact frame duration 151");

msg.voice.source = null;
msg.tempSettings.skip = true;
assert.strictEqual(msg.voiceDuration(), 1, "tempSettings.skip returns 1 even when voice source is null");

msg.tempSettings.skip = false;
assert.strictEqual(msg.voiceDuration(), 0, "Gate active with null voice.source returns degraded timing 0");
assert(ctx._warnings.some(w => w.includes("CAP-WEB-020")), "Diagnostic warning must be logged for degraded voice timing");

const initialWarnCount = ctx._warnings.length;
msg.voiceDuration();
assert.strictEqual(ctx._warnings.length, initialWarnCount, "Diagnostic warning must be bounded one-shot");

msg.voice = {
    source: {
        buffer: { duration: 2.5 },
        playbackRate: { value: 0 }
    }
};
assert.strictEqual(msg.voiceDuration(), 0, "Non-finite result maps to 0");

const unpatchedSandbox = createSandbox();
setupAuthenticVnmScripts(unpatchedSandbox);
const nonVnmMsg = new unpatchedSandbox.vn.Component_MessageBehavior();
assert.strictEqual(nonVnmMsg.voiceDuration.__runtimepack_voice_patched, undefined, "Non-shimmed scope remains untouched");

const orderedSandbox = createSandbox();
orderedSandbox.RuntimePackNative = {
    saved: {},
    saveSlot(ns, slot, payload) { this.saved[ns + "_" + slot] = payload; return true; },
    loadSlot(ns, slot) { return this.saved[ns + "_" + slot] || null; },
    slotExists(ns, slot) { return (ns + "_" + slot) in this.saved; },
    deleteSlot(ns, slot) { delete this.saved[ns + "_" + slot]; return true; },
    listSlots() { return "[]"; }
};

vm.runInContext(shimCode, orderedSandbox);

const engineCode = `
    window.GameStorage = {
        exists: function() { return false; },
        setData: function(k, v) { this[k] = v; return true; },
        getData: function(k) { return this[k] || null; },
        getObject: function(k) { var d = this.getData(k); return d ? JSON.parse(d) : null; },
        setObject: function(k, v) { return this.setData(k, JSON.stringify(v)); }
    };
    window.gs = { GameStorage: window.GameStorage };
`;
vm.runInContext(engineCode, orderedSandbox);

const postEngineHook = `
    if (window.__runtimepackPollVnmStorage) {
        window.__runtimepackPollVnmStorage(window.GameStorage || (window.gs && window.gs.GameStorage));
    }
`;
vm.runInContext(postEngineHook, orderedSandbox);

assert.strictEqual(orderedSandbox.window.GameStorage.__runtimepack_vnm_save_patched, true);

const scriptsCode = `
    window.$PARAMS = { uid: "STARTUP_GAME_UID" };
    window.GameStorage.setObject("settings", { volume: 75 });
    setupAuthenticVnmScripts(window);
`;
orderedSandbox.setupAuthenticVnmScripts = setupAuthenticVnmScripts;
vm.runInContext(scriptsCode, orderedSandbox);

assert.strictEqual(orderedSandbox.RuntimePackNative.saved["STARTUP_GAME_UID_settings"], JSON.stringify({ volume: 75 }));

const postScriptsHook = `
    if (window.__runtimepackPollVnmMessageCompat) { window.__runtimepackPollVnmMessageCompat(); }
    if (window.__runtimepackPollVnmStorage) { window.__runtimepackPollVnmStorage(window.GameStorage || (window.gs && window.gs.GameStorage)); }
    if (window.__runtimepackPollVnmAudioCompat) { window.__runtimepackPollVnmAudioCompat(); }
`;
vm.runInContext(postScriptsHook, orderedSandbox);

assert.strictEqual(orderedSandbox.vn.Component_MessageBehavior.prototype.voiceDuration.__runtimepack_voice_patched, true);

console.log("vnm_startup_lifecycle tests PASSED");
