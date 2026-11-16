(function () {
    "use strict";

    const globalScope = typeof window !== "undefined" ? window : (typeof global !== "undefined" ? global : this);

    const STORAGE_KEY = "vpad_theme_selected";
    const THEME_SHEETS = ["tokens.css", "components.css", "layout.css", "effects.css"];

    const DEFAULT_THEMES = [
        { id: "matte", name: "Minimalist Matte", version: "1.0.0", dir: "themes/matte" },
        { id: "liquid_glass", name: "Liquid Glass", version: "1.0.0", dir: "themes/liquid_glass" },
        { id: "retro_osd", name: "Retro OSD", version: "1.0.0", dir: "themes/retro_osd" }
    ];

    const ALIASES = {
        "clean": "liquid_glass",
        "retro": "retro_osd"
    };

    let availableThemes = DEFAULT_THEMES.slice();
    let defaultThemeId = "matte";
    let currentThemeId = "matte";

    function normalizeThemeId(id) {
        if (!id || typeof id !== "string") return "";
        return ALIASES[id] || id;
    }

    function findTheme(id) {
        const norm = normalizeThemeId(id);
        return availableThemes.find(t => t.id === norm || t.id === id) || null;
    }

    function loadManifest(manifest) {
        if (!manifest) return false;
        try {
            const data = typeof manifest === "string" ? JSON.parse(manifest) : manifest;
            const themesList = data.themes;
            if (Array.isArray(themesList) && themesList.length > 0) {
                availableThemes = themesList.map(t => ({
                    id: t.id,
                    name: t.name,
                    version: t.version || "1.0.0",
                    dir: t.dir || ("themes/" + t.id)
                }));
                if (data.defaultThemeId) {
                    defaultThemeId = data.defaultThemeId;
                }
                return true;
            }
        } catch (_) {}
        return false;
    }

    function getHeadElement() {
        if (typeof document === "undefined") return null;
        return document.head ||
            (document.getElementsByTagName && document.getElementsByTagName("head")[0]) ||
            document.body;
    }

    function unmountThemeSheets() {
        const head = getHeadElement();
        if (!head) return;
        if (typeof head.querySelectorAll === "function") {
            const existing = head.querySelectorAll("link[data-theme-sheet]");
            for (let i = existing.length - 1; i >= 0; i--) {
                const link = existing[i];
                if (link.parentNode) {
                    link.parentNode.removeChild(link);
                } else if (link.parentElement) {
                    link.parentElement.removeChild(link);
                }
            }
        }
    }

    function mountThemeSheets(theme) {
        const head = getHeadElement();
        if (!head || typeof document.createElement !== "function") return;
        let baseDir = theme.dir || ("themes/" + theme.id);
        baseDir = baseDir.replace(/\/+$/, "");

        THEME_SHEETS.forEach(sheet => {
            const link = document.createElement("link");
            link.rel = "stylesheet";
            link.setAttribute("data-theme-sheet", theme.id);
            link.href = baseDir + "/" + sheet;
            head.appendChild(link);
        });
    }

    function updateBodyAttributes(themeId) {
        if (typeof document === "undefined" || !document.body) return;

        if (typeof document.body.setAttribute === "function") {
            document.body.setAttribute("data-theme", themeId);
        }
        if (document.documentElement && typeof document.documentElement.setAttribute === "function") {
            document.documentElement.setAttribute("data-theme", themeId);
        }

        if (document.body.classList) {
            availableThemes.forEach(t => {
                document.body.classList.remove("vpad-theme-" + t.id);
            });
            document.body.classList.remove("vpad-theme-clean", "vpad-theme-matte", "vpad-theme-retro", "vpad-theme-liquid_glass", "vpad-theme-retro_osd");

            document.body.classList.add("vpad-theme-" + themeId);

            if (themeId === "liquid_glass" || themeId === "clean") {
                document.body.classList.add("vpad-theme-clean");
            }
            const isRetro = themeId === "retro_osd" || themeId === "retro";
            document.body.classList.toggle("vpad-theme-retro", isRetro);
        }
    }

    function dispatchThemeChange(theme) {
        const detail = { themeId: theme.id, theme: theme };
        if (typeof window !== "undefined" && window.dispatchEvent) {
            try {
                const evt = (typeof CustomEvent === "function")
                    ? new CustomEvent("vpad:themechange", { detail })
                    : { type: "vpad:themechange", detail };
                window.dispatchEvent(evt);
            } catch (_) {
                window.dispatchEvent({ type: "vpad:themechange", detail });
            }
        }
        if (typeof document !== "undefined" && document.dispatchEvent && (!window || window !== document)) {
            try {
                const evt = (typeof CustomEvent === "function")
                    ? new CustomEvent("vpad:themechange", { detail })
                    : { type: "vpad:themechange", detail };
                document.dispatchEvent(evt);
            } catch (_) {
                document.dispatchEvent({ type: "vpad:themechange", detail });
            }
        }
    }

    function persistTheme(themeId) {
        try {
            if (typeof localStorage !== "undefined" && typeof localStorage.setItem === "function") {
                localStorage.setItem(STORAGE_KEY, themeId);
            }
        } catch (_) {}
    }

    function applyTheme(themeId) {
        const theme = findTheme(themeId);
        if (!theme) {
            return false;
        }

        unmountThemeSheets();
        mountThemeSheets(theme);
        updateBodyAttributes(theme.id);
        currentThemeId = theme.id;
        dispatchThemeChange(theme);
        persistTheme(theme.id);
        return true;
    }

    function getAvailableThemes() {
        return availableThemes.map(t => ({
            id: t.id,
            name: t.name,
            version: t.version
        }));
    }

    function getCurrentTheme() {
        return currentThemeId;
    }

    function tryFetchManifest() {
        if (typeof fetch === "function") {
            try {
                fetch("themes_manifest.json")
                    .then(res => {
                        if (res.ok) return res.json();
                        throw new Error("Failed to load themes_manifest.json: " + res.status);
                    })
                    .then(manifest => {
                        if (loadManifest(manifest)) {
                            const cur = findTheme(currentThemeId);
                            if (cur) {
                                unmountThemeSheets();
                                mountThemeSheets(cur);
                            }
                        }
                    })
                    .catch(() => {});
            } catch (_) {}
        }
    }

    function init() {
        if (typeof globalScope.__RUNTIME_PACK_THEMES_MANIFEST__ !== "undefined") {
            loadManifest(globalScope.__RUNTIME_PACK_THEMES_MANIFEST__);
        } else {
            tryFetchManifest();
        }

        let saved = null;
        try {
            if (typeof localStorage !== "undefined" && typeof localStorage.getItem === "function") {
                saved = localStorage.getItem(STORAGE_KEY);
            }
        } catch (_) {}

        const initial = saved || defaultThemeId || "matte";
        if (typeof document !== "undefined" && document.body) {
            applyTheme(initial);
        } else if (typeof document !== "undefined" && typeof document.addEventListener === "function") {
            document.addEventListener("DOMContentLoaded", () => {
                applyTheme(initial);
            });
        }
    }

    const RuntimePackTheme = {
        STORAGE_KEY,
        applyTheme,
        getAvailableThemes,
        getCurrentTheme,
        loadManifest,
        init
    };

    if (globalScope) {
        globalScope.RuntimePackTheme = RuntimePackTheme;
    }

    init();

    if (typeof module !== "undefined" && module.exports) {
        module.exports = RuntimePackTheme;
    }
})();