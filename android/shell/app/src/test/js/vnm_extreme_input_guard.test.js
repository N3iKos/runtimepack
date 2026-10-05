const assert = require("assert");
const fs = require("fs");
const path = require("path");
const vm = require("vm");

const guardShimPath = path.resolve(__dirname, "../../main/assets/web_shims/runtimepack_vnm_input_guard.js");
const guardShimCode = fs.readFileSync(guardShimPath, "utf8");

function createMockEventManager() {
    const handlers = {};
    return {
        handlers: handlers,
        on: function(eventName, handler, data, owner, priority) {
            priority = priority || 0;
            if (!handlers[eventName]) {
                handlers[eventName] = [];
            }
            if (!handlers[eventName][priority]) {
                handlers[eventName][priority] = [];
            }
            const record = {
                eventName: eventName,
                handler: handler,
                data: data,
                owner: owner,
                priority: priority
            };
            handlers[eventName][priority].push(record);
            return record;
        },
        offByOwner: function(eventName, owner) {
            if (!handlers[eventName]) return;
            for (let p = 0; p < handlers[eventName].length; p++) {
                if (!handlers[eventName][p]) continue;
                handlers[eventName][p] = handlers[eventName][p].filter(h => h.owner !== owner);
            }
        },
        emit: function(eventName, eventObj) {
            const list = handlers[eventName];
            if (!list) return;
            for (let p = 0; p < list.length; p++) {
                if (!list[p]) continue;
                for (let i = 0; i < list[p].length; i++) {
                    const h = list[p][i];
                    h.handler(eventObj);
                    if (eventObj && eventObj.breakChain) {
                        return;
                    }
                }
            }
        }
    };
}

function createAuthenticRendererClass(sandbox) {
    function Component_MessageTextRenderer() {
        this.object = {
            findComponentByName: function(name) { return null; },
            animator: {
                disappear: function(anim, easing, dur, cb) {
                    if (cb && typeof cb === "function") cb();
                }
            },
            events: {
                emit: function(name, ctx) {}
            }
        };
        this.isWaiting = false;
        this.waitCounter = 0;
        this.waitForKey = false;
        this.drawImmediately = false;
        this.isRunning = true;
        this.line = 0;
        this.lines = ["Line 1", "Line 2"];
        this.continueCallCount = 0;
    }

    Component_MessageTextRenderer.prototype.setupEventHandlers = function() {
        sandbox.gs.GlobalEventManager.offByOwner("mouseUp", this.object);
        sandbox.gs.GlobalEventManager.offByOwner("keyUp", this.object);
        sandbox.gs.GlobalEventManager.on("mouseUp", ((function(_this) {
            return function(e) {
                if (_this.object.findComponentByName("animation") || (sandbox.GameManager.settings.autoMessage.enabled && !sandbox.GameManager.settings.autoMessage.stopOnAction)) {
                    return;
                }
                if (sandbox.Input.Mouse.buttons[sandbox.Input.Mouse.LEFT] !== 2) {
                    return;
                }
                if (_this.isWaiting && !(_this.waitCounter > 0 || _this.waitForKey)) {
                    e.breakChain = true;
                    _this["continue"]();
                } else {
                    e.breakChain = _this.isRunning;
                    _this.drawImmediately = !_this.waitForKey;
                    _this.waitCounter = 0;
                    _this.waitForKey = false;
                    _this.isWaiting = false;
                }
                if (_this.waitForKey) {
                    if (sandbox.Input.Mouse.buttons[sandbox.Input.Mouse.LEFT] === 2) {
                        e.breakChain = true;
                        sandbox.Input.clear();
                        _this.waitForKey = false;
                        return _this.isWaiting = false;
                    }
                }
            };
        })(this)), null, this.object);

        return sandbox.gs.GlobalEventManager.on("keyUp", ((function(_this) {
            return function(e) {
                if (sandbox.Input.keys[sandbox.Input.C] && (!_this.isWaiting || (_this.waitCounter > 0 || _this.waitForKey))) {
                    _this.drawImmediately = !_this.waitForKey;
                    _this.waitCounter = 0;
                    _this.waitForKey = false;
                    _this.isWaiting = false;
                }
                if (_this.isWaiting && !_this.waitForKey && !_this.waitCounter && sandbox.Input.keys[sandbox.Input.C]) {
                    _this["continue"]();
                }
                if (_this.waitForKey) {
                    if (sandbox.Input.keys[sandbox.Input.C]) {
                        sandbox.Input.clear();
                        _this.waitForKey = false;
                        return _this.isWaiting = false;
                    }
                }
            };
        })(this)), null, this.object);
    };

    Component_MessageTextRenderer.prototype["continue"] = function() {
        this.continueCallCount++;
        this.isWaiting = false;
        if (this.line >= this.lines.length) {
            this.isRunning = false;
            if (this.object.events) this.object.events.emit("messageFinish", this);
        } else {
            if (this.object.events) this.object.events.emit("messageBatch", this);
            this.line++;
        }
    };

    return Component_MessageTextRenderer;
}

function createSandbox(initialGuardMode) {
    const warnings = [];
    const postedSnapshots = [];
    const sandbox = {
        console: {
            log: console.log,
            error: console.error,
            warn: (msg) => { warnings.push(String(msg)); }
        },
        _warnings: warnings,
        _postedSnapshots: postedSnapshots,
        setTimeout,
        clearTimeout,
        setInterval,
        clearInterval,
        Date,
        JSON,
        Math,
        Array,
        Object,
        isFinite,
        RuntimePackWeb: {
            postVnmInputSnapshot: function(json) {
                postedSnapshots.push(json);
                return true;
            },
            getRuntimeBuildInfo: function() {
                return JSON.stringify({ vnm_input_guard: initialGuardMode || "off" });
            }
        },
        gs: {
            GlobalEventManager: createMockEventManager()
        },
        GameManager: {
            settings: {
                autoMessage: {
                    enabled: false,
                    stopOnAction: true
                }
            },
            tempSettings: {
                skip: false,
                messageFading: { duration: 10, animation: null, easing: null }
            }
        },
        SceneManager: {
            scene: {
                choices: [],
                video: null,
                interpreter: {
                    waitingFor: {
                        inputNumber: null,
                        inputText: null
                    }
                }
            }
        },
        Graphics: {
            transitionData: null,
            frameCount: 100,
            isPaused: false,
            isStopped: false,
            frozen: false
        },
        Input: {
            clear: function() {
                this.Mouse.buttons = [0, 0, 0];
            },
            Mouse: {
                LEFT: 1,
                buttons: [0, 2, 0],
                buttonDown: false,
                buttonUp: true
            },
            C: 67,
            keys: []
        }
    };

    if (initialGuardMode) {
        sandbox.__runtimepack_vnm_input_guard_mode = initialGuardMode;
    }

    sandbox.window = sandbox;
    sandbox.global = sandbox;
    sandbox.self = sandbox;

    sandbox.gs.Component_MessageTextRenderer = createAuthenticRendererClass(sandbox);

    vm.createContext(sandbox);
    return sandbox;
}

console.log("Running VNM Extreme Input Guard JS Tests (vnm_extreme_input_guard.test.js)...");

(function testModeOff() {
    const sb = createSandbox("off");
    vm.runInContext(guardShimCode, sb);

    const renderer = new sb.gs.Component_MessageTextRenderer();
    renderer.isWaiting = true;
    renderer.waitCounter = 0;
    renderer.waitForKey = false;
    renderer.setupEventHandlers();

    sb.Graphics.transitionData = { duration: 30, currentAlpha: 100 };

    const eventObj = { breakChain: false };
    sb.gs.GlobalEventManager.emit("mouseUp", eventObj);

    assert.strictEqual(renderer.continueCallCount, 1, "Mode OFF: continue must be called normally");
    assert.strictEqual(eventObj.breakChain, true, "Mode OFF: breakChain set to true");
    console.log("✔ Test 1: Mode OFF behaves exactly as authentic engine without suppression");
})();

(function testActiveTransitionSuppression() {
    const sb = createSandbox("transition-aware");
    vm.runInContext(guardShimCode, sb);

    const renderer = new sb.gs.Component_MessageTextRenderer();
    renderer.isWaiting = true;
    renderer.waitCounter = 5;
    renderer.waitForKey = true;
    renderer.drawImmediately = false;
    renderer.setupEventHandlers();

    sb.Graphics.transitionData = { duration: 30, currentAlpha: 120 };

    const eventObj = { breakChain: false };
    sb.gs.GlobalEventManager.emit("mouseUp", eventObj);

    assert.strictEqual(renderer.continueCallCount, 0, "Active transition: continue must NOT be called");
    assert.strictEqual(renderer.isWaiting, true, "Active transition: isWaiting not mutated");
    assert.strictEqual(renderer.waitCounter, 5, "Active transition: waitCounter preserved");
    assert.strictEqual(renderer.waitForKey, true, "Active transition: waitForKey preserved");
    assert.strictEqual(renderer.drawImmediately, false, "Active transition: drawImmediately not set");
    assert.strictEqual(eventObj.breakChain, false, "Active transition: e.breakChain not mutated");
    assert.strictEqual(sb.Input.Mouse.buttons[sb.Input.Mouse.LEFT], 2, "Input buttons not cleared");

    const counts = sb.__runtimepack_vnm_input_guard.getCounts();
    assert.strictEqual(counts.blockedDialogueCount, 1, "blockedDialogueCount must be 1");
    assert.strictEqual(counts.delegatedDialogueCount, 0, "delegatedDialogueCount must be 0");
    console.log("✔ Test 2: Active valid transition suppresses dialogue advance and preserves all state");
})();

(function testTransitionClearedDelegation() {
    const sb = createSandbox("transition-aware");
    vm.runInContext(guardShimCode, sb);

    const renderer = new sb.gs.Component_MessageTextRenderer();
    renderer.isWaiting = true;
    renderer.waitCounter = 0;
    renderer.waitForKey = false;
    renderer.setupEventHandlers();

    sb.Graphics.transitionData = { duration: 30, currentAlpha: 50 };
    sb.gs.GlobalEventManager.emit("mouseUp", { breakChain: false });
    assert.strictEqual(renderer.continueCallCount, 0);

    sb.Graphics.transitionData.currentAlpha = 180;
    sb.gs.GlobalEventManager.emit("mouseUp", { breakChain: false });
    assert.strictEqual(renderer.continueCallCount, 0);

    sb.Graphics.transitionData = null;

    const eventObj = { breakChain: false };
    sb.gs.GlobalEventManager.emit("mouseUp", eventObj);

    assert.strictEqual(renderer.continueCallCount, 1, "Transition cleared: continue called exactly once");
    assert.strictEqual(eventObj.breakChain, true, "Transition cleared: breakChain set to true");

    const counts = sb.__runtimepack_vnm_input_guard.getCounts();
    assert.strictEqual(counts.blockedDialogueCount, 2, "Blocked 2 taps during transition");
    assert.strictEqual(counts.delegatedDialogueCount, 1, "Delegated 1 tap after transition cleared");
    console.log("✔ Test 3: Transition cleared delegates next tap once without click replay");
})();

(function testPartialTextTapOutsideTransition() {
    const sb = createSandbox("transition-aware");
    vm.runInContext(guardShimCode, sb);

    const renderer = new sb.gs.Component_MessageTextRenderer();
    renderer.isWaiting = false;
    renderer.waitCounter = 0;
    renderer.waitForKey = false;
    renderer.drawImmediately = false;
    renderer.setupEventHandlers();

    sb.Graphics.transitionData = null;

    const eventObj = { breakChain: false };
    sb.gs.GlobalEventManager.emit("mouseUp", eventObj);

    assert.strictEqual(renderer.drawImmediately, true, "Partial-text tap: sets drawImmediately = true");
    assert.strictEqual(renderer.continueCallCount, 0, "Partial-text tap: continue not called yet");
    assert.strictEqual(eventObj.breakChain, true, "Partial-text tap: breakChain set to isRunning");
    console.log("✔ Test 4: Partial-text tap outside transition sets drawImmediately as original");
})();

(function testFailOpenCases() {
    const sb = createSandbox("transition-aware");
    vm.runInContext(guardShimCode, sb);

    const renderer = new sb.gs.Component_MessageTextRenderer();
    renderer.isWaiting = true;
    renderer.waitCounter = 0;
    renderer.waitForKey = false;
    renderer.setupEventHandlers();

    sb.Graphics.transitionData = { duration: 30, currentAlpha: 100 };
    sb.SceneManager.scene.choices = [{ id: 1, text: "Option A" }];
    sb.gs.GlobalEventManager.emit("mouseUp", { breakChain: false });
    assert.strictEqual(renderer.continueCallCount, 1, "5A: choices present fails open");
    sb.SceneManager.scene.choices = [];

    renderer.isWaiting = true;
    sb.SceneManager.scene.video = { isPlaying: true };
    sb.gs.GlobalEventManager.emit("mouseUp", { breakChain: false });
    assert.strictEqual(renderer.continueCallCount, 2, "5B: video present fails open");
    sb.SceneManager.scene.video = null;

    renderer.isWaiting = true;
    sb.GameManager.tempSettings.skip = true;
    sb.gs.GlobalEventManager.emit("mouseUp", { breakChain: false });
    assert.strictEqual(renderer.continueCallCount, 3, "5C: skip active fails open");
    sb.GameManager.tempSettings.skip = false;

    renderer.isWaiting = true;
    sb.GameManager.settings.autoMessage.enabled = true;
    sb.gs.GlobalEventManager.emit("mouseUp", { breakChain: false });
    assert.strictEqual(renderer.continueCallCount, 4, "5D: auto active fails open");
    sb.GameManager.settings.autoMessage.enabled = false;

    renderer.isWaiting = true;
    sb.SceneManager.scene.interpreter.waitingFor.inputNumber = { variable: 1 };
    sb.gs.GlobalEventManager.emit("mouseUp", { breakChain: false });
    assert.strictEqual(renderer.continueCallCount, 5, "5E: inputNumber active fails open");
    sb.SceneManager.scene.interpreter.waitingFor.inputNumber = null;

    renderer.isWaiting = true;
    sb.SceneManager.scene.interpreter.waitingFor.inputText = { variable: 2 };
    sb.gs.GlobalEventManager.emit("mouseUp", { breakChain: false });
    assert.strictEqual(renderer.continueCallCount, 6, "5F: inputText active fails open");
    sb.SceneManager.scene.interpreter.waitingFor.inputText = null;

    console.log("✔ Test 5: Interactive contexts (choices, video, skip, prompts) safely fail open");
})();

(function testDescriptorSafetyNoGettersCalled() {
    const sb = createSandbox("transition-aware");
    vm.runInContext(guardShimCode, sb);

    let getterCalled = 0;
    const transitionDataWithSpy = {
        duration: 30,
        currentAlpha: 50
    };
    Object.defineProperty(transitionDataWithSpy, "trapGetter", {
        get: function() {
            getterCalled++;
            return "trap";
        },
        enumerable: true
    });

    sb.Graphics.transitionData = transitionDataWithSpy;

    const renderer = new sb.gs.Component_MessageTextRenderer();
    renderer.isWaiting = true;
    renderer.setupEventHandlers();

    sb.gs.GlobalEventManager.emit("mouseUp", { breakChain: false });
    assert.strictEqual(getterCalled, 0, "No getters with side effects must be invoked");
    console.log("✔ Test 6: Property inspection is descriptor-safe and invokes 0 getter traps");
})();

(function testKeyUpAndInstantSkipUntouched() {
    const sb = createSandbox("transition-aware");
    vm.runInContext(guardShimCode, sb);

    const renderer = new sb.gs.Component_MessageTextRenderer();
    renderer.isWaiting = true;
    renderer.waitCounter = 0;
    renderer.waitForKey = false;
    renderer.setupEventHandlers();

    sb.Graphics.transitionData = { duration: 30, currentAlpha: 100 };

    sb.Input.keys[sb.Input.C] = true;
    sb.gs.GlobalEventManager.emit("keyUp", { breakChain: false });

    assert.strictEqual(renderer.continueCallCount, 1, "KeyUp advance remains untouched");

    renderer.continue();
    assert.strictEqual(renderer.continueCallCount, 2, "Direct continue() remains untouched");
    console.log("✔ Test 7: KeyUp and direct continue()/instantSkip execution remain completely untouched");
})();

(function testRepeatedSetupAndOffByOwner() {
    const sb = createSandbox("transition-aware");
    vm.runInContext(guardShimCode, sb);

    const renderer = new sb.gs.Component_MessageTextRenderer();
    renderer.isWaiting = true;
    renderer.waitCounter = 0;
    renderer.waitForKey = false;

    renderer.setupEventHandlers();
    renderer.setupEventHandlers();
    renderer.setupEventHandlers();

    const mouseUpHandlers = sb.gs.GlobalEventManager.handlers["mouseUp"][0] || [];
    assert.strictEqual(mouseUpHandlers.length, 1, "Only 1 active handler registered after repeated setups");

    sb.Graphics.transitionData = null;
    sb.gs.GlobalEventManager.emit("mouseUp", { breakChain: false });
    assert.strictEqual(renderer.continueCallCount, 1, "Called continue exactly once");
    console.log("✔ Test 8: Repeated setupEventHandlers disposes old listeners cleanly without duplicates");
})();

(function testSetupExceptionRestoresOn() {
    const sb = createSandbox("transition-aware");
    vm.runInContext(guardShimCode, sb);

    const origOn = sb.gs.GlobalEventManager.on;

    const renderer = new sb.gs.Component_MessageTextRenderer();
    const origOff = sb.gs.GlobalEventManager.offByOwner;
    sb.gs.GlobalEventManager.offByOwner = function() {
        throw new Error("SetupCrash");
    };

    let threw = false;
    try {
        renderer.setupEventHandlers();
    } catch (_) {
        threw = true;
    } finally {
        sb.gs.GlobalEventManager.offByOwner = origOff;
    }
    assert.strictEqual(threw, true, "Setup threw error as expected");
    assert.strictEqual(sb.gs.GlobalEventManager.on, origOn, "GlobalEventManager.on must be restored in finally");
    console.log("✔ Test 9: Exception during setupEventHandlers restores GlobalEventManager.on safely");
})();

(function testGameExceptionPropagates() {
    const sb = createSandbox("transition-aware");
    vm.runInContext(guardShimCode, sb);

    const renderer = new sb.gs.Component_MessageTextRenderer();
    renderer.isWaiting = true;
    renderer["continue"] = function() {
        throw new Error("AuthenticGameEngineCrash");
    };
    renderer.setupEventHandlers();

    sb.Graphics.transitionData = null;

    assert.throws(() => {
        sb.gs.GlobalEventManager.emit("mouseUp", { breakChain: false });
    }, /AuthenticGameEngineCrash/, "Original game exception must propagate without being caught or swallowed");
    console.log("✔ Test 10: Original game exception propagates cleanly without being swallowed");
})();

(function testSaveSerializationExclusion() {
    const sb = createSandbox("transition-aware");
    vm.runInContext(guardShimCode, sb);

    const renderer = new sb.gs.Component_MessageTextRenderer();
    renderer.setupEventHandlers();

    const keys = Object.keys(renderer);
    assert.strictEqual(keys.includes("__vnm_input_guard"), false, "No adapter properties on renderer");

    const json = JSON.stringify(renderer);
    assert.strictEqual(json.includes("vnm_input_guard"), false, "Renderer serialized JSON excludes adapter internals");

    const protoKeys = Object.keys(sb.gs.Component_MessageTextRenderer.prototype);
    assert.strictEqual(protoKeys.includes("__vnm_input_guard_wrapped"), false, "Prototype property is non-enumerable");
    console.log("✔ Test 11: Adapter internals are non-enumerable and excluded from JSON serialization");
})();

(function testMultipleRendererInstancesIsolated() {
    const sb = createSandbox("transition-aware");
    vm.runInContext(guardShimCode, sb);

    const r1 = new sb.gs.Component_MessageTextRenderer();
    const r2 = new sb.gs.Component_MessageTextRenderer();

    r1.setupEventHandlers();
    r2.setupEventHandlers();

    const list = sb.gs.GlobalEventManager.handlers["mouseUp"][0];
    assert.strictEqual(list.length, 2);
    assert.strictEqual(list[0].owner, r1.object);
    assert.strictEqual(list[1].owner, r2.object);
    console.log("✔ Test 12: Multiple renderer instances have isolated registrations");
})();

console.log("\n==================================================");
console.log("All 12 VNM Extreme Input Guard JS tests passed successfully!");
console.log("==================================================");
