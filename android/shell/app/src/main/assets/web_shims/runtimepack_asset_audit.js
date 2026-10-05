/**
 * RuntimePack Asset Audit - Web Loader Instrumentation
 * Intercepts RPG Maker MV/MZ, TyranoScript, and HTML5 asset loading failures
 * (sync and async Image, WebAudio, Html5Audio, Video)
 * and forwards structured audit events to native AssetAuditService.
 */
(function(root) {
    if (root.__runtimepack_asset_audit_installed) return;
    root.__runtimepack_asset_audit_installed = true;

    var events = [];
    var seqId = 0;
    var MAX_EVENTS = 1000;
    var recentEventsMap = {};

    var RuntimePackAssetAudit = {
        events: events,
        reset: function() {
            events.length = 0;
            seqId = 0;
            recentEventsMap = {};
        },
        recordEvent: function(ev) {
            // Deduplicate identical failure events within 200ms
            var dedupKey = (ev.asset_path || "") + "|" + (ev.stage || "") + "|" + (ev.fallback_action || "");
            var now = Date.now();
            if (recentEventsMap[dedupKey] && (now - recentEventsMap[dedupKey]) < 200) {
                return null;
            }
            recentEventsMap[dedupKey] = now;

            seqId++;
            var eventObj = {
                sequence_id: ev.sequence_id || seqId,
                timestamp_ns: ev.timestamp_ns || (now * 1000000),
                stage: ev.stage || "FILE_EXISTS",
                asset_path: ev.asset_path || "",
                outcome: ev.outcome || "FAIL",
                fallback_action: ev.fallback_action || null,
                error_message: ev.error_message || null,
                source_location: ev.source_location || null
            };
            events.push(eventObj);
            if (events.length > MAX_EVENTS) {
                events.shift();
            }

            var jsonStr = JSON.stringify(eventObj);
            try {
                if (root.RuntimePackWeb && typeof root.RuntimePackWeb.postAuditEvent === "function") {
                    root.RuntimePackWeb.postAuditEvent(jsonStr);
                } else if (root.RuntimePackVPad && typeof root.RuntimePackVPad.postAuditEvent === "function") {
                    root.RuntimePackVPad.postAuditEvent(jsonStr);
                }
            } catch (e) {
                if (root.console && typeof root.console.error === "function") {
                    root.console.error("[RuntimePack AssetAudit] Failed to post event to native bridge:", e);
                }
            }
            return eventObj;
        },
        installHooks: function(target) {
            var ctx = target || root;
            installImageHooks(ctx);
            installAudioHooks(ctx);
            installGraphicsHooks(ctx);
        }
    };

    function installImageHooks(ctx) {
        if (ctx.ImageManager && typeof ctx.ImageManager.loadBitmap === "function") {
            if (!ctx.ImageManager.loadBitmap.__rp_audit_hooked) {
                var origLoadBitmap = ctx.ImageManager.loadBitmap;
                ctx.ImageManager.loadBitmap = function(folder, filename, hue, smooth) {
                    var bmp = origLoadBitmap.apply(this, arguments);
                    if (bmp && typeof bmp.addErrorListener === "function" && !bmp.__rp_audit_hooked) {
                        bmp.__rp_audit_hooked = true;
                        var targetPath = (folder || "") + (filename || "");
                        bmp.addErrorListener(function(errBmp) {
                            var assetUrl = (errBmp && errBmp._url) ? errBmp._url : (bmp._url || targetPath);
                            RuntimePackAssetAudit.recordEvent({
                                stage: "FILE_EXISTS",
                                asset_path: assetUrl,
                                outcome: "FAIL",
                                fallback_action: "PLACEHOLDER_SUBSTITUTED",
                                error_message: "Image bitmap failed to load: " + assetUrl,
                                source_location: "ImageManager.loadBitmap"
                            });
                        });
                    }
                    return bmp;
                };
                ctx.ImageManager.loadBitmap.__rp_audit_hooked = true;
            }
        }
    }

    function installAudioHooks(ctx) {
        // 1. Synchronous AudioManager calls
        if (ctx.AudioManager) {
            var audioMethods = ["playBgm", "playBgs", "playMe", "playSe"];
            audioMethods.forEach(function(m) {
                if (typeof ctx.AudioManager[m] === "function" && !ctx.AudioManager[m].__rp_audit_hooked) {
                    var orig = ctx.AudioManager[m];
                    ctx.AudioManager[m] = function(audioObj) {
                        try {
                            return orig.apply(this, arguments);
                        } catch (err) {
                            var audioPath = (audioObj && (audioObj.name || audioObj.src)) || String(audioObj || "");
                            RuntimePackAssetAudit.recordEvent({
                                stage: "FILE_EXISTS",
                                asset_path: audioPath,
                                outcome: "FAIL",
                                fallback_action: "AUDIO_LOAD_SUPPRESSED",
                                error_message: String(err && err.message ? err.message : err),
                                source_location: "AudioManager." + m
                            });
                            return null;
                        }
                    };
                    ctx.AudioManager[m].__rp_audit_hooked = true;
                }
            });
        }

        // 2. Asynchronous WebAudio error hooks
        if (ctx.WebAudio) {
            var webAudioProto = ctx.WebAudio.prototype || ctx.WebAudio;
            if (typeof webAudioProto._onError === "function" && !webAudioProto._onError.__rp_audit_hooked) {
                var origWebAudioOnError = webAudioProto._onError;
                webAudioProto._onError = function() {
                    var assetUrl = this._url || "";
                    RuntimePackAssetAudit.recordEvent({
                        stage: "FILE_EXISTS",
                        asset_path: assetUrl,
                        outcome: "FAIL",
                        fallback_action: "AUDIO_LOAD_SUPPRESSED",
                        error_message: "WebAudio async error: " + assetUrl,
                        source_location: "WebAudio._onError"
                    });
                    return origWebAudioOnError.apply(this, arguments);
                };
                webAudioProto._onError.__rp_audit_hooked = true;
            }
            if (typeof webAudioProto._onXhrLoad === "function" && !webAudioProto._onXhrLoad.__rp_audit_hooked) {
                var origOnXhrLoad = webAudioProto._onXhrLoad;
                webAudioProto._onXhrLoad = function(xhr) {
                    if (xhr && xhr.status >= 400) {
                        var assetUrl = this._url || "";
                        RuntimePackAssetAudit.recordEvent({
                            stage: "FILE_EXISTS",
                            asset_path: assetUrl,
                            outcome: "FAIL",
                            fallback_action: "AUDIO_LOAD_SUPPRESSED",
                            error_message: "WebAudio HTTP " + xhr.status + ": " + assetUrl,
                            source_location: "WebAudio._onXhrLoad"
                        });
                    }
                    return origOnXhrLoad.apply(this, arguments);
                };
                webAudioProto._onXhrLoad.__rp_audit_hooked = true;
            }
        }

        // 3. Asynchronous Html5Audio error hooks
        if (ctx.Html5Audio) {
            var html5AudioTarget = ctx.Html5Audio.prototype || ctx.Html5Audio;
            if (typeof html5AudioTarget._onError === "function" && !html5AudioTarget._onError.__rp_audit_hooked) {
                var origHtml5OnError = html5AudioTarget._onError;
                html5AudioTarget._onError = function() {
                    var assetUrl = this._url || (this._audioElement && (this._audioElement.src || this._audioElement.currentSrc)) || "";
                    RuntimePackAssetAudit.recordEvent({
                        stage: "FILE_EXISTS",
                        asset_path: assetUrl,
                        outcome: "FAIL",
                        fallback_action: "AUDIO_LOAD_SUPPRESSED",
                        error_message: "Html5Audio async error: " + assetUrl,
                        source_location: "Html5Audio._onError"
                    });
                    return origHtml5OnError.apply(this, arguments);
                };
                html5AudioTarget._onError.__rp_audit_hooked = true;
            }
        }

        // 4. Global HTMLAudioElement error listener interception
        if (ctx.HTMLAudioElement && ctx.HTMLAudioElement.prototype && typeof ctx.HTMLAudioElement.prototype.addEventListener === "function") {
            if (!ctx.HTMLAudioElement.prototype.addEventListener.__rp_audit_hooked) {
                var origAudioAddEventListener = ctx.HTMLAudioElement.prototype.addEventListener;
                ctx.HTMLAudioElement.prototype.addEventListener = function(type, listener, options) {
                    if (type === "error" && typeof listener === "function") {
                        var self = this;
                        var wrappedListener = function(event) {
                            var assetUrl = self.src || self.currentSrc || "";
                            if (assetUrl) {
                                RuntimePackAssetAudit.recordEvent({
                                    stage: "FILE_EXISTS",
                                    asset_path: assetUrl,
                                    outcome: "FAIL",
                                    fallback_action: "AUDIO_LOAD_SUPPRESSED",
                                    error_message: "HTMLAudioElement async error event for: " + assetUrl,
                                    source_location: "HTMLAudioElement.onerror"
                                });
                            }
                            return listener.apply(this, arguments);
                        };
                        return origAudioAddEventListener.call(this, type, wrappedListener, options);
                    }
                    return origAudioAddEventListener.apply(this, arguments);
                };
                ctx.HTMLAudioElement.prototype.addEventListener.__rp_audit_hooked = true;
            }
        }
    }

    function installGraphicsHooks(ctx) {
        if (ctx.Graphics && typeof ctx.Graphics.playVideo === "function") {
            if (!ctx.Graphics.playVideo.__rp_audit_hooked) {
                var origPlayVideo = ctx.Graphics.playVideo;
                ctx.Graphics.playVideo = function(src) {
                    try {
                        var res = origPlayVideo.apply(this, arguments);
                        if (ctx.Graphics._video && !ctx.Graphics._video.__rp_audit_hooked) {
                            ctx.Graphics._video.__rp_audit_hooked = true;
                            if (typeof ctx.Graphics._video.addEventListener === "function") {
                                ctx.Graphics._video.addEventListener("error", function() {
                                    RuntimePackAssetAudit.recordEvent({
                                        stage: "FILE_EXISTS",
                                        asset_path: String(src || ""),
                                        outcome: "FAIL",
                                        fallback_action: "VIDEO_PLAY_SUPPRESSED",
                                        error_message: "HTMLVideoElement error event for: " + src,
                                        source_location: "Graphics.playVideo"
                                    });
                                });
                            }
                        }
                        return res;
                    } catch (err) {
                        RuntimePackAssetAudit.recordEvent({
                            stage: "FILE_EXISTS",
                            asset_path: String(src || ""),
                            outcome: "FAIL",
                            fallback_action: "VIDEO_PLAY_SUPPRESSED",
                            error_message: String(err && err.message ? err.message : err),
                            source_location: "Graphics.playVideo"
                        });
                        return null;
                    }
                };
                ctx.Graphics.playVideo.__rp_audit_hooked = true;
            }

            if (typeof ctx.Graphics._onVideoError === "function" && !ctx.Graphics._onVideoError.__rp_audit_hooked) {
                var origOnVideoError = ctx.Graphics._onVideoError;
                ctx.Graphics._onVideoError = function() {
                    var assetUrl = (ctx.Graphics._video && (ctx.Graphics._video.src || ctx.Graphics._video.currentSrc)) || "";
                    RuntimePackAssetAudit.recordEvent({
                        stage: "FILE_EXISTS",
                        asset_path: assetUrl,
                        outcome: "FAIL",
                        fallback_action: "VIDEO_PLAY_SUPPRESSED",
                        error_message: "Graphics._onVideoError triggered for: " + assetUrl,
                        source_location: "Graphics._onVideoError"
                    });
                    return origOnVideoError.apply(this, arguments);
                };
                ctx.Graphics._onVideoError.__rp_audit_hooked = true;
            }
        }
    }

    root.RuntimePackAssetAudit = RuntimePackAssetAudit;

    // Auto-install on available globals immediately
    RuntimePackAssetAudit.installHooks(root);

    if (typeof setInterval !== "undefined") {
        var hookAttempts = 0;
        var hookTimer = setInterval(function() {
            hookAttempts++;
            RuntimePackAssetAudit.installHooks(root);
            if ((root.ImageManager && root.AudioManager && root.Graphics) || hookAttempts >= 100) {
                clearInterval(hookTimer);
            }
        }, 50);
    }

    if (typeof module !== "undefined" && module.exports) {
        module.exports = RuntimePackAssetAudit;
    }
})(typeof window !== "undefined" ? window : (typeof globalThis !== "undefined" ? globalThis : this));
