const assert = require("assert");
const fs = require("fs");
const path = require("path");
const vm = require("vm");

const shimPath = path.resolve(__dirname, "../../main/assets/web_shims/runtimepack_web_shims.js");
const shimCode = fs.readFileSync(shimPath, "utf8");

class MockAudioBufferSourceNode {
    constructor(ctx) {
        this.context = ctx;
        this.buffer = null;
        this.loop = false;
        this.playbackRate = { value: 1.0 };
        this._started = false;
        this._stopped = false;
        this._onended = null;
    }
    start(when, offset, duration) {
        if (this._started) throw new Error("InvalidStateError: already started");
        this._started = true;
    }
    stop(when) {
        if (!this._started) {
            throw new Error("InvalidStateError: cannot call stop without calling start first");
        }
        if (this._stopped) throw new Error("InvalidStateError: already stopped");
        this._stopped = true;
    }
    connect(dest) {
        return dest;
    }
    disconnect() {}
}

const originalNativeSetterCalls = [];
const originalOnendedSetter = function(fn) {
    originalNativeSetterCalls.push(fn);
    this._onended = fn;
};
const originalOnendedGetter = function() {
    return this._onended;
};

Object.defineProperty(MockAudioBufferSourceNode.prototype, "onended", {
    get: originalOnendedGetter,
    set: originalOnendedSetter,
    configurable: true,
    enumerable: true
});

class MockAudioContext {
    constructor() {
        this.currentTime = 0;
        this.destination = {};
    }
    createBufferSource() {
        return new MockAudioBufferSourceNode(this);
    }
    createGain() {
        return {
            gain: { value: 1.0 },
            connect: function() {},
            disconnect: function() {},
            numberOfOutputs: 1
        };
    }
    close() {}
}

function createAudioSandbox() {
    const sandbox = {
        console,
        setTimeout,
        clearTimeout,
        setInterval,
        clearInterval,
        Uint8Array,
        ArrayBuffer,
        AudioContext: MockAudioContext,
        AudioBufferSourceNode: MockAudioBufferSourceNode
    };
    sandbox.window = sandbox;
    sandbox.global = sandbox;
    sandbox.self = sandbox;
    vm.createContext(sandbox);
    vm.runInContext(shimCode, sandbox);
    return sandbox;
}

const testCtx = createAudioSandbox();
const audioCtx = new testCtx.AudioContext();
const initialSource = audioCtx.createBufferSource();

assert.throws(() => {
    initialSource.stop(0);
}, /cannot call stop without calling start/);

assert(typeof testCtx.window.__runtimepackPollVnmAudioCompat === "function", "audio compat hook must be exposed");
testCtx.window.__runtimepackPollVnmAudioCompat();

assert.doesNotThrow(() => {
    initialSource.stop(0);
}, "Stop before start must be safely guarded");

const startedSource = audioCtx.createBufferSource();
startedSource.start(0);
startedSource.stop(0);
assert.strictEqual(startedSource._stopped, true);

const failingStartSource = audioCtx.createBufferSource();
const origNativeStart = MockAudioBufferSourceNode.prototype.start;
MockAudioBufferSourceNode.prototype.start = function() {
    if (this._failStart) throw new Error("Hardware audio start failure");
    return origNativeStart.apply(this, arguments);
};
testCtx.window.__runtimepackPollVnmAudioCompat();

failingStartSource._failStart = true;
assert.throws(() => {
    failingStartSource.start(0);
}, /Hardware audio start failure/);
assert.strictEqual(failingStartSource.__rp_started, undefined);

assert.doesNotThrow(() => {
    failingStartSource.stop(0);
});

MockAudioBufferSourceNode.prototype.start = origNativeStart;

function AudioBuffer(context) {
    this.context = context;
    this.decoded = true;
    this.decodedBuffer = { duration: 5.0 };
    this.source = null;
    this.gainNode = context.createGain();
    this.playing = false;
    this.disposed = false;
    this.loop = true;
}
AudioBuffer.audioBuffers = [];
AudioBuffer.prototype.play = function() {
    this.playing = true;
    this.source = this.context.createBufferSource();
    this.source.buffer = this.decodedBuffer;
    this.source.loop = this.loop;
    const self = this;
    this.source.onended = function() {
        if (self.playing = false, null != self.onEnd) self.onEnd(self);
        if (self.loop) return self.play();
    };
    this.source.start(0);
};
AudioBuffer.prototype.stop = function() {
    if (this.playing && this.source) {
        this.source.stop(0);
        this.playing = false;
    }
};
AudioBuffer.prototype.dispose = function() {
    if (!this.disposed) {
        this.stop();
        this.disposed = true;
        this.source = null;
    }
};

function AudioBufferReference(audioBuffer, context) {
    this.context = context;
    this.audioBuffer = audioBuffer;
    this.loop = true;
    this.playing = false;
    this.source = null;
    this.gainNode = context.createGain();
}
AudioBufferReference.audioBufferRefs = [];
AudioBufferReference.prototype.play = function() {
    if (!this.audioBuffer.error) {
        this.playing = true;
        const e = this.source.playbackRate.value;
        const t = this.source.buffer;
        this.source = this.context.createBufferSource();
        this.source.loop = this.loop;
        this.source.buffer = t;
        const self = this;
        this.source.onended = function() {
            if (self.playing = false, null != self.onEnd) self.onEnd(self);
            if (self.loop) return self.play();
        };
        this.source.playbackRate.value = e;
        this.source.start(0);
    }
};
AudioBufferReference.prototype.dispose = function() {
    this.source = null;
};

testCtx.window.gs = { AudioBuffer, AudioBufferReference, Audio: { removeAudioBuffer() {} } };
testCtx.window.__runtimepackPollVnmAudioCompat();

const authenticBuf = new AudioBuffer(audioCtx);
authenticBuf.play();
const activeBufSource = authenticBuf.source;
authenticBuf.dispose();
assert.strictEqual(authenticBuf.source, null);
assert.doesNotThrow(() => {
    if (activeBufSource._onended) {
        activeBufSource._onended();
    }
});

const sharedAudioBuffer = {
    error: false,
    decoded: true,
    decodedBuffer: { duration: 3.0 }
};

const ref1 = new AudioBufferReference(sharedAudioBuffer, audioCtx);
ref1.source = audioCtx.createBufferSource();
ref1.source.buffer = sharedAudioBuffer.decodedBuffer;
ref1.play();

let callbackExecuted = false;
ref1.onEnd = function() {
    callbackExecuted = true;
};

const activeSource = ref1.source;
ref1.dispose();

assert.strictEqual(ref1.source, null, "ref1.source must be nulled on disposal");
assert.doesNotThrow(() => {
    if (activeSource._onended) {
        activeSource._onended();
    }
}, "Stale onended callback after disposal must not throw or resurrect loop");

assert.strictEqual(callbackExecuted, false, "Stale callback must not execute onEnd");

const ref2 = new AudioBufferReference(sharedAudioBuffer, audioCtx);
ref2.source = audioCtx.createBufferSource();
ref2.source.buffer = sharedAudioBuffer.decodedBuffer;
ref2.loop = false;
let naturalLoopExecuted = false;
ref2.onEnd = function() {
    naturalLoopExecuted = true;
};
ref2.play();
const ref2Source = ref2.source;
assert(ref2Source._onended, "ref2 onended handler must be registered");
ref2Source._onended();
assert.strictEqual(naturalLoopExecuted, true, "Natural callback before disposal must execute");

const crypto = require("crypto");

function resolveGameRoot() {
    if (process.env.VNM_GAME_ROOT && fs.existsSync(process.env.VNM_GAME_ROOT)) {
        return process.env.VNM_GAME_ROOT;
    }
    const standardCandidates = [
        "D:/Old-Recvery/Project_2/RPGM-P/Raw-Games/Time for Punishment Class ~Taking Lessons for My Boyfriend~",
        path.resolve(process.cwd(), "../Old-Recvery/Project_2/RPGM-P/Raw-Games/Time for Punishment Class ~Taking Lessons for My Boyfriend~")
    ];
    for (const c of standardCandidates) {
        if (fs.existsSync(c)) return c;
    }
    return null;
}

const gameRoot = resolveGameRoot();
if (!gameRoot) {
    throw new Error("NOT_TESTED: VNM_GAME_ROOT not found");
}

const enginePath = path.join(gameRoot, "data/ENGINE.js");
const scriptsPath = path.join(gameRoot, "data/SCRIPTS.js");
if (!fs.existsSync(enginePath) || !fs.existsSync(scriptsPath)) {
    throw new Error("NOT_TESTED: ENGINE.js or SCRIPTS.js not found in gameRoot");
}

const engineCode = fs.readFileSync(enginePath, "utf8");
const scriptsCode = fs.readFileSync(scriptsPath, "utf8");

const engineHash = crypto.createHash("sha256").update(engineCode).digest("hex");
assert.strictEqual(engineHash, "928372395b6e82020d693653468aa1a02e1326a924dcfac7b7c6d0739b107885");

const audioEndIdx = engineCode.indexOf("window.GS.Audio=new t,gs.Audio=GS.Audio}.call(this)");
const audioStart = engineCode.lastIndexOf("function(){var t;t=function()", audioEndIdx);
const audioEnd = engineCode.indexOf(".call(this)", audioEndIdx) + 11;
const authenticAudioIIFE = engineCode.slice(audioStart, audioEnd);

const refEndIdx = engineCode.indexOf("window.GS.AudioBufferReference=t,gs.AudioBufferReference=t}.call(this)");
const refStart = engineCode.lastIndexOf("function(){var t;t=function()", refEndIdx);
const refEnd = engineCode.indexOf(".call(this)", refEndIdx) + 11;
const authenticRefIIFE = engineCode.slice(refStart, refEnd);

const streamEndIdx = engineCode.indexOf("window.GS.AudioBufferStream=t,window.gs.AudioBufferStream=t}.call(this)");
const streamStart = engineCode.lastIndexOf("function(){var t;t=function()", streamEndIdx);
const streamEnd = engineCode.indexOf(".call(this)", streamEndIdx) + 11;
const authenticStreamIIFE = engineCode.slice(streamStart, streamEnd);

const stopIdx = scriptsCode.indexOf("AudioManager.prototype.stopAllSounds = function()");
const stopEnd = scriptsCode.indexOf("return results;\n  };", stopIdx) + 20;
const authenticStopAllSounds = scriptsCode.slice(stopIdx, stopEnd);

class MockWebAudio {
    constructor() {
        this.paused = true;
        this.currentTime = 0;
        this.volume = 1.0;
        this.src = "";
        this.playbackRate = 1.0;
    }
    addEventListener() {}
    removeEventListener() {}
    play() { this.paused = false; }
    pause() { this.paused = true; }
    load() {}
}

function createAuthenticSandbox(applyShim) {
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
        AudioContext: MockAudioContext,
        AudioBufferSourceNode: MockAudioBufferSourceNode,
        Audio: MockWebAudio,
        $PARAMS: {}
    };
    sandbox.window = sandbox;
    sandbox.global = sandbox;
    sandbox.self = sandbox;
    sandbox.gs = {
        ObjectCodec: {
            decodeProperties: function() {},
            encodeProperties: function(i) { return i; }
        },
        Application: {
            getPlatformSpecificAudioFilePath: function(p) { return p; },
            loadData: function() {}
        },
        Platform: { isSafariMobile: false, isCordova: false }
    };
    sandbox.GS = sandbox.gs;
    vm.createContext(sandbox);

    vm.runInContext("Array.prototype.remove = function(item) { var idx = this.indexOf(item); if (idx !== -1) this.splice(idx, 1); return this; };", sandbox);

    if (applyShim) {
        vm.runInContext(shimCode, sandbox);
    }

    vm.runInContext("(" + authenticAudioIIFE + ")", sandbox);
    vm.runInContext("(" + authenticRefIIFE + ")", sandbox);
    vm.runInContext("(" + authenticStreamIIFE + ")", sandbox);

    return sandbox;
}

const rawSandbox = createAuthenticSandbox(false);
const rawIsPlayingDesc = Object.getOwnPropertyDescriptor(rawSandbox.gs.AudioBufferReference.prototype, "isPlaying");
assert.strictEqual(rawIsPlayingDesc.configurable, false);

assert.throws(() => {
    Object.defineProperty(rawSandbox.gs.AudioBufferReference.prototype, "isPlaying", {
        get: function() { return false; }
    });
}, /Cannot redefine property: isPlaying/);

const rawPendingRef = new rawSandbox.gs.AudioBufferReference(null);
assert.strictEqual(rawSandbox.gs.Audio.audioBuffers.includes(rawPendingRef), true);

assert.throws(() => {
    rawSandbox.gs.Audio.pause();
}, /Cannot read properties of null/);

assert.throws(() => {
    rawPendingRef.stop();
}, /Cannot read properties of null/);

rawPendingRef.customData.wasPlaying = true;
assert.throws(() => {
    rawSandbox.gs.Audio.resume();
}, /Cannot read properties of null/);

let rawClearCount = 0;
let rawFocusLostCount = 0;
const rawInput = { clear: () => { rawClearCount++; } };
const rawGraphics = {
    paused: false,
    onFocusLost: () => { rawFocusLostCount++; }
};
rawSandbox.Input = rawInput;
assert.throws(() => {
    rawGraphics.paused = true;
    rawSandbox.gs.Audio.pause();
    rawInput.clear();
    rawGraphics.onFocusLost();
}, /Cannot read properties of null/);
assert.strictEqual(rawGraphics.paused, true);
assert.strictEqual(rawClearCount, 0);
assert.strictEqual(rawFocusLostCount, 0);

const patchedSandbox = createAuthenticSandbox(true);
assert(typeof patchedSandbox.window.__runtimepackPollVnmAudioCompat === "function");
patchedSandbox.window.__runtimepackPollVnmAudioCompat();

const patchedProtoDesc = Object.getOwnPropertyDescriptor(patchedSandbox.gs.AudioBufferReference.prototype, "isPlaying");
assert.strictEqual(patchedProtoDesc.configurable, false);

const patchedPendingRef = new patchedSandbox.gs.AudioBufferReference(null);
assert.strictEqual(patchedSandbox.gs.Audio.audioBuffers.includes(patchedPendingRef), true);
assert.strictEqual(patchedPendingRef.audioBuffer, null);
assert.strictEqual(patchedPendingRef.isPlaying, false);

const ownIsPlaying = Object.getOwnPropertyDescriptor(patchedPendingRef, "isPlaying");
assert(ownIsPlaying, "must have own isPlaying descriptor");
assert.strictEqual(ownIsPlaying.enumerable, false, "own isPlaying must be non-enumerable");

assert.doesNotThrow(() => {
    patchedSandbox.gs.Audio.pause();
});
assert.strictEqual(patchedPendingRef.customData.wasPlaying, false);

assert.doesNotThrow(() => {
    patchedPendingRef.stop();
});

patchedSandbox.AudioManager = function() {};
patchedSandbox.AudioManager.prototype.soundReferences = {
    bgm: [patchedPendingRef]
};
vm.runInContext(authenticStopAllSounds, patchedSandbox);
const audioMgrInstance = new patchedSandbox.AudioManager();
assert.doesNotThrow(() => {
    audioMgrInstance.stopAllSounds();
});

patchedPendingRef.customData.wasPlaying = true;
assert.doesNotThrow(() => {
    patchedSandbox.gs.Audio.resume();
});

let patchedClearCount = 0;
let patchedFocusLostCount = 0;
let patchedFocusReceiveCount = 0;
const patchedGraphics = {
    paused: false,
    onFocusLost: () => { patchedFocusLostCount++; },
    onFocusReceive: () => { patchedFocusReceiveCount++; }
};
patchedSandbox.Input = { clear: () => { patchedClearCount++; } };

patchedGraphics.paused = true;
patchedSandbox.gs.Audio.pause();
patchedSandbox.Input.clear();
patchedGraphics.onFocusLost();
assert.strictEqual(patchedGraphics.paused, true);
assert.strictEqual(patchedClearCount, 1);
assert.strictEqual(patchedFocusLostCount, 1);

patchedGraphics.paused = false;
patchedSandbox.gs.Audio.resume();
patchedSandbox.Input.clear();
patchedGraphics.onFocusReceive();
assert.strictEqual(patchedGraphics.paused, false);
assert.strictEqual(patchedClearCount, 2);
assert.strictEqual(patchedFocusReceiveCount, 1);

assert.strictEqual(patchedPendingRef.volume, 100);
assert.strictEqual(patchedPendingRef.playbackRate, undefined);
patchedPendingRef.volume = 75;
assert.strictEqual(patchedPendingRef.volume, 75);
patchedPendingRef.playbackRate = 1.25;
assert.strictEqual(patchedPendingRef.playbackRate, 1.25);

const bundleBefore = patchedSandbox.gs.AudioBufferReference.toDataBundle(patchedPendingRef, {});
assert.strictEqual(bundleBefore.audioBuffer, null);
assert.strictEqual(bundleBefore.volume, 75);
assert.strictEqual(bundleBefore.playbackRate, 1.25);
assert.strictEqual(bundleBefore.isPlaying, false);
assert.strictEqual(bundleBefore.__rp_pending_sidecar, undefined);
for (const key of Object.keys(patchedPendingRef)) {
    assert(!key.startsWith("__rp_"), "No enumerable shim state: " + key);
}

class MockEmitterAudioBuffer {
    constructor() {
        this.decoded = false;
        this.error = false;
        this.listeners = {};
        this.decodedBuffer = { duration: 4.0 };
    }
    once(evt, data, fn) {
        this.listeners[evt] = { data, fn };
    }
    emit(evt) {
        if (this.listeners[evt]) {
            const { data, fn } = this.listeners[evt];
            delete this.listeners[evt];
            fn({ data });
        }
    }
}

const fakeBuf = new MockEmitterAudioBuffer();
const restoredRef = patchedSandbox.gs.AudioBufferReference.fromDataBundle({
    audioBuffer: fakeBuf,
    customData: { restored: true },
    useAnalyser: false,
    volume: 85,
    playbackRate: 1.0,
    isPlaying: false
}, {});
assert.strictEqual(restoredRef.audioBuffer, null);
assert.strictEqual(patchedSandbox.gs.Audio.audioBuffers.includes(restoredRef), true);

restoredRef.volume = 85;
restoredRef.playbackRate = 1.0;
assert.strictEqual(restoredRef.volume, 85);
assert.strictEqual(restoredRef.playbackRate, 1.0);

fakeBuf.decoded = true;
fakeBuf.emit("finishDecode");
assert.strictEqual(restoredRef.audioBuffer, fakeBuf);
assert.strictEqual(restoredRef.volume, 85);
assert.strictEqual(restoredRef.playbackRate, 1.0);

const sharedBuf = new MockEmitterAudioBuffer();
sharedBuf.decoded = true;
const refA = new patchedSandbox.gs.AudioBufferReference(sharedBuf);
const refB = new patchedSandbox.gs.AudioBufferReference(sharedBuf);
refA.initialize();
refB.initialize();
const sourceA = refA.source;
assert(sourceA, "refA source created");
assert(refB.source, "refB source created");
let refAOnEndCalled = false;
refA.onEnd = () => { refAOnEndCalled = true; };
refA.dispose();
assert.strictEqual(refA.source, null);
assert.strictEqual(refA.__rp_disposed, true);
assert.strictEqual(refB.__rp_disposed, undefined);
assert.notStrictEqual(refB.source, null);
assert.strictEqual(sharedBuf.decodedBuffer !== null, true);

if (sourceA._onended) {
    sourceA._onended();
}
assert.strictEqual(refAOnEndCalled, false);

refA.initialize();
assert.strictEqual(refA.source, null);

const stream = new patchedSandbox.gs.AudioBufferStream("music.ogg", "en");
assert.strictEqual(stream.isPlaying, false);
stream.dispose();
assert.strictEqual(stream.isPlaying, false);
assert.doesNotThrow(() => {
    stream.stop();
});

const retroSandbox = createAuthenticSandbox(true);
const preExistingRef = new retroSandbox.gs.AudioBufferReference(null);
assert.strictEqual(retroSandbox.gs.Audio.audioBuffers.includes(preExistingRef), true);
retroSandbox.window.__runtimepackPollVnmAudioCompat();
assert.strictEqual(preExistingRef.isPlaying, false);
assert.doesNotThrow(() => {
    retroSandbox.gs.Audio.pause();
});

const failingRef = new patchedSandbox.gs.AudioBufferReference({
    error: false,
    decoded: true,
    decodedBuffer: { duration: 1.0 }
});
failingRef.initialize();
failingRef.source.stop = function() { throw new Error("Hardware native failure"); };
failingRef.playing = true;
assert.throws(() => {
    failingRef.stop();
}, /Hardware native failure/);

const warnings = patchedSandbox._warnings;
const unconfigurableRef = {
    audioBuffer: null,
    playing: false,
    initialize: function() {}
};
Object.defineProperty(unconfigurableRef, "isPlaying", {
    get: function() { return false; },
    configurable: false
});
Object.defineProperty(unconfigurableRef, "volume", {
    get: function() { return 100; },
    configurable: false
});
Object.defineProperty(unconfigurableRef, "playbackRate", {
    get: function() { return 1.0; },
    configurable: false
});
patchedSandbox.gs.Audio.addAudioBuffer(unconfigurableRef);
assert(warnings.some(w => w.includes("Unsupported own non-configurable isPlaying descriptor on reference")));
assert(warnings.some(w => w.includes("Unsupported own non-configurable volume descriptor on reference")));
assert(warnings.some(w => w.includes("Unsupported own non-configurable playbackRate descriptor on reference")));

const unconfigurableStream = {
    audioStream: {},
    fadeInVolume: 0
};
Object.defineProperty(unconfigurableStream, "isPlaying", {
    get: function() { return false; },
    configurable: false
});
patchedSandbox.gs.Audio.addAudioBuffer(unconfigurableStream);
assert(warnings.some(w => w.includes("Unsupported own non-configurable isPlaying descriptor on stream")));

const gsOnlySandbox = {
    console,
    setTimeout,
    clearTimeout,
    setInterval,
    clearInterval,
    Uint8Array,
    ArrayBuffer,
    AudioContext: MockAudioContext,
    AudioBufferSourceNode: MockAudioBufferSourceNode,
    Audio: MockWebAudio,
    $PARAMS: {}
};
gsOnlySandbox.window = gsOnlySandbox;
gsOnlySandbox.global = gsOnlySandbox;
gsOnlySandbox.self = gsOnlySandbox;
gsOnlySandbox.GS = {
    ObjectCodec: {
        decodeProperties: function() {},
        encodeProperties: function(i) { return i; }
    },
    Application: {
        getPlatformSpecificAudioFilePath: function(p) { return p; },
        loadData: function() {}
    },
    Platform: { isSafariMobile: false, isCordova: false }
};
gsOnlySandbox.gs = gsOnlySandbox.GS;
vm.createContext(gsOnlySandbox);
vm.runInContext("Array.prototype.remove = function(item) { var idx = this.indexOf(item); if (idx !== -1) this.splice(idx, 1); return this; };", gsOnlySandbox);
vm.runInContext(shimCode, gsOnlySandbox);
vm.runInContext("(" + authenticAudioIIFE + ")", gsOnlySandbox);
vm.runInContext("(" + authenticRefIIFE + ")", gsOnlySandbox);
vm.runInContext("(" + authenticStreamIIFE + ")", gsOnlySandbox);
gsOnlySandbox.gs = gsOnlySandbox.GS;
gsOnlySandbox.window.gs = gsOnlySandbox.GS;
gsOnlySandbox.window.GS = gsOnlySandbox.GS;
assert.strictEqual(typeof gsOnlySandbox.GS.Audio, "object");
gsOnlySandbox.window.__runtimepackPollVnmAudioCompat();
const gsOnlyPendingRef = new gsOnlySandbox.GS.AudioBufferReference(null);
assert.strictEqual(gsOnlyPendingRef.isPlaying, false);
assert.doesNotThrow(() => {
    gsOnlySandbox.GS.Audio.pause();
});

// =========================================================================
// Task 2A: Authentic VNM AudioManager Pool Null Safety & Lifecycle Tests
// =========================================================================
const amStartIdx = scriptsCode.indexOf("AudioManager = (function()");
const amEndIdx = scriptsCode.indexOf("gs.AudioManager = AudioManager;", amStartIdx) + "gs.AudioManager = AudioManager;".length;
const authenticAudioManagerCode = scriptsCode.slice(amStartIdx, amEndIdx);

function createManagerSandbox(applyShim) {
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
        AudioContext: MockAudioContext,
        AudioBufferSourceNode: MockAudioBufferSourceNode,
        Audio: MockWebAudio,
        $PARAMS: { uid: "BB45114F1292564E907B88654BEDAB153D70" },
        GameManager: {
            settings: { soundEnabled: true, voiceEnabled: true },
            defaults: {
                audio: { voiceVolume: 100, voicePlaybackRate: 100 }
            }
        },
        ResourceManager: {
            getAudioBuffer: function(p) { return null; }
        }
    };
    sandbox.window = sandbox;
    sandbox.global = sandbox;
    sandbox.self = sandbox;
    sandbox.gs = {
        ObjectCodec: {
            decodeProperties: function() {},
            encodeProperties: function(i) { return i; }
        },
        Application: {
            getPlatformSpecificAudioFilePath: function(p) { return p; },
            loadData: function() {}
        },
        Platform: { isSafariMobile: false, isCordova: false }
    };
    sandbox.GS = sandbox.gs;
    vm.createContext(sandbox);

    vm.runInContext("Array.prototype.remove = function(item) { var idx = this.indexOf(item); if (idx !== -1) this.splice(idx, 1); return this; };", sandbox);

    if (applyShim) {
        vm.runInContext(shimCode, sandbox);
    }

    vm.runInContext("(" + authenticAudioIIFE + ")", sandbox);
    vm.runInContext("(" + authenticRefIIFE + ")", sandbox);
    vm.runInContext("(" + authenticStreamIIFE + ")", sandbox);
    vm.runInContext("var AudioManager;\n" + authenticAudioManagerCode, sandbox);

    return sandbox;
}

// 1. RED VERIFICATION on Raw Authentic Manager:
const rawAmSandbox = createManagerSandbox(false);
const rawAm = rawAmSandbox.window.AudioManager;
assert(rawAm, "raw AudioManager must exist");

// RED 1: playSound / playVoice throws exact TypeError when soundReferences contains null
const rawVoicePool = [null, { isPlaying: false, play: () => {}, volume: 1, playbackRate: 1, loop: false, voice: true }];
rawAm.soundReferences["voice_ch5_01"] = rawVoicePool;
assert.throws(() => {
    rawAm.playVoice("voice_ch5_01");
}, /Cannot read properties of null \(reading 'isPlaying'\)/);

// RED 2: stopSound throws TypeError when pool contains null
rawAm.soundReferences["voice_ch5_02"] = [null, { stop: () => {} }];
assert.throws(() => {
    rawAm.stopSound("voice_ch5_02");
}, /Cannot read properties of null \(reading 'stop'\)/);

// RED 3: stopAllVoices throws TypeError when pool contains null
rawAm.soundReferences["voice_ch5_03"] = [null, { voice: true, stop: () => {} }];
assert.throws(() => {
    rawAm.stopAllVoices();
}, /Cannot read properties of null \(reading 'voice'\)/);

// 2. GREEN VERIFICATION on Patched Manager:
const patchedAmSandbox = createManagerSandbox(true);
assert(typeof patchedAmSandbox.window.__runtimepackPollVnmAudioCompat === "function");
patchedAmSandbox.window.__runtimepackPollVnmAudioCompat();

const patchedAm = patchedAmSandbox.window.AudioManager;
assert(patchedAm, "patched AudioManager must exist");

// Task 2A.1: Restored soundReferences with [null, validInactiveRef] -> sanitized in-place, preserves array identity
let playCountInactive = 0;
const validInactiveRef = {
    isPlaying: false,
    volume: 1,
    playbackRate: 1,
    loop: false,
    voice: false,
    play: function() { playCountInactive++; },
    stop: function() {}
};
const poolCh5 = [null, validInactiveRef];
patchedAm.soundReferences["voice_ch5_01"] = poolCh5;

assert.doesNotThrow(() => {
    patchedAm.playVoice("voice_ch5_01", 90, 110);
});
assert.strictEqual(patchedAm.soundReferences["voice_ch5_01"], poolCh5, "Array identity must be preserved");
assert.strictEqual(poolCh5.length, 1, "null entry must be removed in-place");
assert.strictEqual(poolCh5[0], validInactiveRef, "Valid inactive reference preserved");
assert.strictEqual(playCountInactive, 1, "Inactive reference must be played exactly once");
assert.strictEqual(validInactiveRef.volume, 0.9, "Volume must be set to 90/100");
assert.strictEqual(validInactiveRef.playbackRate, 1.1, "PlaybackRate must be set to 110/100");
assert.strictEqual(validInactiveRef.voice, true, "voice flag must be set");

// Task 2A.2: Holes and undefined elements and all-null array -> creates new reference through original method
let createdReference = null;
const mockAudioBuffer = {
    loaded: true,
    decoded: true,
    decodedBuffer: { duration: 2.0 },
    error: false
};
patchedAmSandbox.ResourceManager.getAudioBuffer = function(p) {
    if (p === "Audio/Sounds/voice_ch5_02") return mockAudioBuffer;
    return null;
};
const sparsePool = [undefined, null];
sparsePool.length = 3; // hole at index 2
patchedAm.soundReferences["voice_ch5_02"] = sparsePool;

assert.doesNotThrow(() => {
    createdReference = patchedAm.playSound("voice_ch5_02", 80, 100);
});
assert.strictEqual(patchedAm.soundReferences["voice_ch5_02"], sparsePool, "Array identity must be preserved");
assert.strictEqual(sparsePool.length, 1, "Holes and nulls pruned, newly created ref pushed");
assert.strictEqual(sparsePool[0], createdReference, "Created reference in pool");
assert(createdReference instanceof patchedAmSandbox.GS.AudioBufferReference, "New reference instantiated through authentic constructor");

// Task 2A.3: Valid playing references skipped, inactive reused
let playCountActive = 0;
let playCountInactive2 = 0;
const playingRef = {
    isPlaying: true,
    play: function() { playCountActive++; },
    stop: function() {}
};
const inactiveRef = {
    isPlaying: false,
    play: function() { playCountInactive2++; },
    stop: function() {}
};
const mixedPool = [null, playingRef, undefined, inactiveRef];
patchedAm.soundReferences["sound_effect_01"] = mixedPool;

assert.doesNotThrow(() => {
    patchedAm.playSound("sound_effect_01");
});
assert.strictEqual(playCountActive, 0, "Playing reference must be skipped");
assert.strictEqual(playCountInactive2, 1, "Inactive reference reused and played once");
assert.strictEqual(mixedPool.length, 2, "Only playing and inactive remain");
assert.strictEqual(mixedPool[0], playingRef);
assert.strictEqual(mixedPool[1], inactiveRef);

// Task 2A.4: Pending reference object with audioBuffer=null remains in pool (NOT pruned as null)
const pendingRef = new patchedAmSandbox.gs.AudioBufferReference(null);
assert.strictEqual(pendingRef.audioBuffer, null);
assert.strictEqual(pendingRef.isPlaying, false);
const pendingPool = [null, pendingRef];
patchedAm.soundReferences["voice_pending"] = pendingPool;

assert.doesNotThrow(() => {
    patchedAm.playSound("voice_pending");
});
assert.strictEqual(pendingPool.length, 1, "Pending ref must NOT be pruned");
assert.strictEqual(pendingPool[0], pendingRef, "Pending reference remains intact in pool");

// Task 2A.5: Genuine errors on malformed non-null object propagate; no blanket catch
const faultyRef = {
    get isPlaying() {
        throw new Error("Native Audio Subsystem Device Disconnected");
    },
    stop: function() {}
};
patchedAm.soundReferences["voice_faulty"] = [faultyRef];
assert.throws(() => {
    patchedAm.playSound("voice_faulty");
}, /Native Audio Subsystem Device Disconnected/);

// Task 2A.6: stopSound and stopAllVoices analogous null-entry paths
let stopCalled1 = false;
let stopCalledVoice = false;
const validStopRef = {
    stop: function() { stopCalled1 = true; }
};
const validVoiceRef = {
    voice: true,
    stop: function() { stopCalledVoice = true; }
};
patchedAm.soundReferences["sound_stop"] = [null, validStopRef];
assert.doesNotThrow(() => {
    patchedAm.stopSound("sound_stop");
});
assert.strictEqual(stopCalled1, true, "stopSound executed on valid ref after null pruning");
assert.strictEqual(patchedAm.soundReferences["sound_stop"].length, 1);

patchedAm.soundReferences["voice_all_stop"] = [null, validVoiceRef];
assert.doesNotThrow(() => {
    patchedAm.stopAllVoices();
});
assert.strictEqual(stopCalledVoice, true, "stopAllVoices executed on valid voice ref after null pruning");
assert.strictEqual(patchedAm.soundReferences["voice_all_stop"].length, 1);

// Task 2A.6b: stopAllSounds existing nullcheck semantics preserved
let stopAllSoundsCalled = false;
const validStopAllRef = {
    stop: function() { stopAllSoundsCalled = true; }
};
patchedAm.soundReferences["sound_all_stop"] = [null, validStopAllRef];
assert.doesNotThrow(() => {
    patchedAm.stopAllSounds();
});
assert.strictEqual(stopAllSoundsCalled, true);
assert.strictEqual(patchedAm.soundReferences["sound_all_stop"].length, 1);

// Task 2A.7: Object argument with folderPath and name normalization
let objectArgPlayCount = 0;
const objArgRef = {
    isPlaying: false,
    play: function() { objectArgPlayCount++; },
    stop: function() {}
};
patchedAm.soundReferences["voice_obj"] = [null, objArgRef];
assert.doesNotThrow(() => {
    patchedAm.playSound({ name: "voice_obj", volume: 95, playbackRate: 105, folderPath: "Audio/Voices" });
});
assert.strictEqual(objectArgPlayCount, 1, "Object argument normalized and delegated cleanly");
assert.strictEqual(objArgRef.volume, 0.95);
assert.strictEqual(objArgRef.playbackRate, 1.05);

const bundleTestRef = new patchedAmSandbox.gs.AudioBufferReference(null);
bundleTestRef.volume = 90;
const bundleResult = patchedAmSandbox.gs.AudioBufferReference.toDataBundle(bundleTestRef, {});
assert.strictEqual(bundleResult.audioBuffer, null);
assert.strictEqual(bundleResult.volume, 90);
for (const key of Object.keys(bundleResult)) {
    assert(!key.startsWith("__rp_"), "No leaked shim keys in data bundle: " + key);
}

assert(patchedAmSandbox._warnings.some(w => w.includes("[CAP-WEB-025]")), "CAP-WEB-025 pool warning logged");
assert(!patchedAmSandbox._warnings.some(w => w.includes("[CAP-WEB-022] Sanitized")), "CAP-WEB-022 not used for pool sanitization");

console.log("vnm_audio_lifecycle tests PASSED");
