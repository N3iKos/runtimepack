(function() {
    'use strict';

    var root = typeof window !== "undefined" ? window : global;

    var guardMode = root.__runtimepack_vnm_input_guard_mode;
    if (typeof guardMode !== "string") {
        try {
            var bridge = root.RuntimePackWeb || root.RuntimePackNative;
            if (bridge && typeof bridge.getRuntimeBuildInfo === "function") {
                var info = JSON.parse(bridge.getRuntimeBuildInfo());
                if (info && typeof info.vnm_input_guard === "string") {
                    guardMode = info.vnm_input_guard;
                }
            }
        } catch (_) {}
    }

    if (guardMode !== "transition-aware") {
        return;
    }

    var gs = root.gs;
    if (!gs || !gs.Component_MessageTextRenderer || !gs.Component_MessageTextRenderer.prototype) {
        console.warn("[RuntimePack] VNM Input Guard target gs.Component_MessageTextRenderer not found; guard not installed");
        return;
    }

    if (typeof gs.Component_MessageTextRenderer.prototype.setupEventHandlers !== "function") {
        console.warn("[RuntimePack] VNM Input Guard target setupEventHandlers not found; guard not installed");
        return;
    }

    if (gs.Component_MessageTextRenderer.prototype.setupEventHandlers.__vnm_input_guard_wrapped) {
        return;
    }

    var mouseDownCount = 0;
    var mouseUpCount = 0;
    var touchCancelCount = 0;
    var dialogueCallbackCount = 0;
    var blockedDialogueCount = 0;
    var delegatedDialogueCount = 0;
    var unknownStateCount = 0;
    var snapshotSeq = 0;
    var startTime = Date.now();
    var lastFrameCount = null;
    var bridgeWarningEmitted = false;

    function readSafeDataProp(obj, prop) {
        if (!obj || (typeof obj !== "object" && typeof obj !== "function")) {
            return undefined;
        }
        var desc = Object.getOwnPropertyDescriptor(obj, prop);
        if (desc) {
            if ("value" in desc) {
                return desc.value;
            }
            unknownStateCount++;
            return undefined;
        }
        var proto = Object.getPrototypeOf(obj);
        while (proto) {
            desc = Object.getOwnPropertyDescriptor(proto, prop);
            if (desc) {
                if ("value" in desc) {
                    return desc.value;
                }
                unknownStateCount++;
                return undefined;
            }
            proto = Object.getPrototypeOf(proto);
        }
        return undefined;
    }

    function checkShouldBlock(renderer) {
        var Graphics = root.Graphics;
        if (!Graphics || typeof Graphics !== "object") {
            unknownStateCount++;
            return false;
        }

        var transitionData = readSafeDataProp(Graphics, "transitionData");
        if (!transitionData || typeof transitionData !== "object") {
            return false;
        }

        var duration = readSafeDataProp(transitionData, "duration");
        var currentAlpha = readSafeDataProp(transitionData, "currentAlpha");

        if (typeof duration !== "number" || !isFinite(duration) || duration <= 0) {
            return false;
        }
        if (typeof currentAlpha !== "number" || !isFinite(currentAlpha) || currentAlpha >= 255) {
            return false;
        }

        var GameManager = root.GameManager;
        if (GameManager && typeof GameManager === "object") {
            var tempSettings = readSafeDataProp(GameManager, "tempSettings");
            if (tempSettings && typeof tempSettings === "object") {
                var skip = readSafeDataProp(tempSettings, "skip");
                if (skip === true) {
                    return false;
                }
            }
            var settings = readSafeDataProp(GameManager, "settings");
            if (settings && typeof settings === "object") {
                var autoMessage = readSafeDataProp(settings, "autoMessage");
                if (autoMessage && typeof autoMessage === "object") {
                    var autoEnabled = readSafeDataProp(autoMessage, "enabled");
                    if (autoEnabled === true) {
                        return false;
                    }
                }
            }
        }

        var SceneManager = root.SceneManager;
        if (!SceneManager || typeof SceneManager !== "object") {
            unknownStateCount++;
            return false;
        }

        var scene = readSafeDataProp(SceneManager, "scene");
        if (!scene || typeof scene !== "object") {
            unknownStateCount++;
            return false;
        }

        var choices = readSafeDataProp(scene, "choices");
        if (choices !== undefined && choices !== null) {
            if (Array.isArray(choices) && choices.length > 0) {
                return false;
            }
        }

        var video = readSafeDataProp(scene, "video");
        if (video !== undefined && video !== null) {
            return false;
        }

        var interpreter = readSafeDataProp(scene, "interpreter");
        if (interpreter && typeof interpreter === "object") {
            var waitingFor = readSafeDataProp(interpreter, "waitingFor");
            if (waitingFor && typeof waitingFor === "object") {
                var inputNum = readSafeDataProp(waitingFor, "inputNumber");
                if (inputNum) {
                    return false;
                }
                var inputTxt = readSafeDataProp(waitingFor, "inputText");
                if (inputTxt) {
                    return false;
                }
            }
        }

        return true;
    }

    var originalSetup = gs.Component_MessageTextRenderer.prototype.setupEventHandlers;
    var activeSetupStack = [];

    var wrappedSetup = function() {
        var eventManager = gs.GlobalEventManager;
        if (!eventManager || typeof eventManager.on !== "function") {
            return originalSetup.apply(this, arguments);
        }

        var origOn = eventManager.on;
        var currentRenderer = this;
        var expectedOwner = this ? this.object : null;

        activeSetupStack.push(currentRenderer);

        eventManager.on = function(eventName, handler, data, owner, priority) {
            var topRenderer = activeSetupStack[activeSetupStack.length - 1];
            if (eventName === "mouseUp" && topRenderer && expectedOwner && owner === expectedOwner && typeof handler === "function") {
                var guardedHandler = function(e) {
                    dialogueCallbackCount++;
                    var block = false;
                    try {
                        block = checkShouldBlock(topRenderer);
                    } catch (_) {
                        block = false;
                    }
                    if (block) {
                        blockedDialogueCount++;
                        return;
                    }
                    delegatedDialogueCount++;
                    return handler.apply(this, arguments);
                };
                return origOn.call(eventManager, eventName, guardedHandler, data, owner, priority);
            }
            return origOn.call(eventManager, eventName, handler, data, owner, priority);
        };

        try {
            return originalSetup.apply(this, arguments);
        } finally {
            activeSetupStack.pop();
            if (activeSetupStack.length === 0) {
                eventManager.on = origOn;
            }
        }
    };

    wrappedSetup.__vnm_input_guard_wrapped = true;
    gs.Component_MessageTextRenderer.prototype.setupEventHandlers = wrappedSetup;

    if (typeof root.addEventListener === "function") {
        root.addEventListener("mousedown", function() {
            mouseDownCount++;
        }, true);
        root.addEventListener("mouseup", function() {
            mouseUpCount++;
        }, true);
        root.addEventListener("touchcancel", function() {
            touchCancelCount++;
        }, true);
    }

    var sampleInterval = null;
    if (typeof root.setInterval === "function") {
        sampleInterval = root.setInterval(function() {
            var doc = root.document;
            if (doc && doc.hidden === true) {
                return;
            }

            var Graphics = root.Graphics;
            var SceneManager = root.SceneManager;
            var GameManager = root.GameManager;
            var Input = root.Input;

            var currentFrameCount = null;
            if (Graphics && typeof Graphics.frameCount === "number") {
                currentFrameCount = Graphics.frameCount;
            }
            var frameDelta = null;
            if (currentFrameCount !== null) {
                if (lastFrameCount !== null) {
                    frameDelta = currentFrameCount - lastFrameCount;
                } else {
                    frameDelta = 0;
                }
                lastFrameCount = currentFrameCount;
            }

            var tPresent = false;
            var tAlpha = null;
            var tDur = null;
            if (Graphics && Graphics.transitionData && typeof Graphics.transitionData === "object") {
                tPresent = true;
                if (typeof Graphics.transitionData.currentAlpha === "number") {
                    tAlpha = Graphics.transitionData.currentAlpha;
                }
                if (typeof Graphics.transitionData.duration === "number") {
                    tDur = Graphics.transitionData.duration;
                }
            }

            var leftBtn = null;
            var btnDown = null;
            var btnUp = null;
            if (Input && Input.Mouse) {
                if (Array.isArray(Input.Mouse.buttons)) {
                    leftBtn = !!Input.Mouse.buttons[Input.Mouse.LEFT || 1];
                }
                if (typeof Input.Mouse.buttonDown === "boolean") {
                    btnDown = Input.Mouse.buttonDown;
                }
                if (typeof Input.Mouse.buttonUp === "boolean") {
                    btnUp = Input.Mouse.buttonUp;
                }
            }

            var choiceCount = null;
            var videoPresent = null;
            if (SceneManager && SceneManager.scene) {
                if (Array.isArray(SceneManager.scene.choices)) {
                    choiceCount = SceneManager.scene.choices.length;
                }
                videoPresent = SceneManager.scene.video != null;
            }

            var skipActive = null;
            var autoActive = null;
            if (GameManager) {
                if (GameManager.tempSettings && typeof GameManager.tempSettings.skip === "boolean") {
                    skipActive = GameManager.tempSettings.skip;
                }
                if (GameManager.settings && GameManager.settings.autoMessage && typeof GameManager.settings.autoMessage.enabled === "boolean") {
                    autoActive = GameManager.settings.autoMessage.enabled;
                }
            }

            snapshotSeq++;
            var snapshot = {
                schema_version: 1,
                sequence: snapshotSeq,
                elapsed_ms: Date.now() - startTime,
                mode: "transition-aware",
                frame_count: currentFrameCount,
                frame_delta: frameDelta,
                graphics_paused: Graphics && typeof Graphics.isPaused === "boolean" ? Graphics.isPaused : (Graphics && typeof Graphics.paused === "boolean" ? Graphics.paused : null),
                graphics_stopped: Graphics && typeof Graphics.isStopped === "boolean" ? Graphics.isStopped : (Graphics && typeof Graphics.stopped === "boolean" ? Graphics.stopped : null),
                graphics_frozen: Graphics && typeof Graphics.frozen === "boolean" ? Graphics.frozen : null,
                transition_present: tPresent,
                transition_alpha: tAlpha,
                transition_duration: tDur,
                left_button_state: leftBtn,
                button_down: btnDown,
                button_up: btnUp,
                mouse_down_count: mouseDownCount,
                mouse_up_count: mouseUpCount,
                touch_cancel_count: touchCancelCount,
                dialogue_callback_count: dialogueCallbackCount,
                blocked_dialogue_count: blockedDialogueCount,
                delegated_dialogue_count: delegatedDialogueCount,
                choice_count: choiceCount,
                video_present: videoPresent,
                skip_active: skipActive,
                auto_active: autoActive,
                unknown_state_count: unknownStateCount
            };

            var bridge = root.RuntimePackWeb || root.RuntimePackNative;
            if (bridge && typeof bridge.postVnmInputSnapshot === "function") {
                try {
                    bridge.postVnmInputSnapshot(JSON.stringify(snapshot));
                } catch (e) {
                    if (!bridgeWarningEmitted) {
                        bridgeWarningEmitted = true;
                        console.warn("[RuntimePack] Failed to post VNM input snapshot:", e && e.message);
                    }
                }
            }
        }, 1000);
        if (sampleInterval && typeof sampleInterval.unref === "function") {
            try {
                sampleInterval.unref();
            } catch (_) {}
        }
    }

    if (typeof root.addEventListener === "function") {
        root.addEventListener("pagehide", function() {
            if (sampleInterval !== null && typeof root.clearInterval === "function") {
                root.clearInterval(sampleInterval);
            }
        }, { passive: true });
    }

    Object.defineProperty(root, "__runtimepack_vnm_input_guard", {
        value: {
            getCounts: function() {
                return {
                    mouseDownCount: mouseDownCount,
                    mouseUpCount: mouseUpCount,
                    touchCancelCount: touchCancelCount,
                    dialogueCallbackCount: dialogueCallbackCount,
                    blockedDialogueCount: blockedDialogueCount,
                    delegatedDialogueCount: delegatedDialogueCount,
                    unknownStateCount: unknownStateCount
                };
            },
            checkShouldBlock: checkShouldBlock,
            sampleInterval: sampleInterval
        },
        enumerable: false,
        configurable: true,
        writable: true
    });
})();
