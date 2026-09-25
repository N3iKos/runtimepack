/**
 * RuntimePack Asset Audit - Web Loader Instrumentation
 * Intercepts RPG Maker MV/MZ and TyranoScript asset loading failures
 * and forwards structured audit events to native AssetAuditService.
 */
(function(root) {
    if (root.__runtimepack_asset_audit_installed) return;
    root.__runtimepack_asset_audit_installed = true;

    var events = [];
    var seqId = 0;

    var RuntimePackAssetAudit = {
        events: events,
        reset: function() {
            events.length = 0;
            seqId = 0;
        },
        recordEvent: function(ev) {
            seqId++;
            var eventObj = {
                sequence_id: ev.sequence_id || seqId,
                timestamp_ns: ev.timestamp_ns || (Date.now() * 1000000),
                stage: ev.stage || "FILE_EXISTS",
                asset_path: ev.asset_path || "",
                outcome: ev.outcome || "FAIL",
                fallback_action: ev.fallback_action || null,
                error_message: ev.error_message || null,
                source_location: ev.source_location || null
            };
            events.push(eventObj);

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
    }

    function installGraphicsHooks(ctx) {
        if (ctx.Graphics && typeof ctx.Graphics.playVideo === "function") {
            if (!ctx.Graphics.playVideo.__rp_audit_hooked) {
                var origPlayVideo = ctx.Graphics.playVideo;
                ctx.Graphics.playVideo = function(src) {
                    try {
                        return origPlayVideo.apply(this, arguments);
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
        }
    }

    root.RuntimePackAssetAudit = RuntimePackAssetAudit;

    // Auto-install on available globals immediately
    RuntimePackAssetAudit.installHooks(root);

    if (typeof setInterval !== "undefined") {
        var hookTimer = setInterval(function() {
            RuntimePackAssetAudit.installHooks(root);
            if (root.ImageManager && root.AudioManager && root.Graphics) {
                clearInterval(hookTimer);
            }
        }, 50);
    }

    if (typeof module !== "undefined" && module.exports) {
        module.exports = RuntimePackAssetAudit;
    }
})(typeof window !== "undefined" ? window : (typeof globalThis !== "undefined" ? globalThis : this));
