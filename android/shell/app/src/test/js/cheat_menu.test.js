const assert = require("assert");
const path = require("path");

console.log("Running Cheat Menu Suite Tests (cheat_menu.test.js)...");

// ============================================================================
// 1. SETUP MOCK DOM & BROWSER ENVIRONMENT FOR NODE.JS
// ============================================================================

function createMockEnvironment() {
    const idMap = {};

    function matchesSelector(el, sel) {
        if (!el || !sel) return false;
        sel = sel.trim();

        // Check attribute selector like [attr="val"] or tag[attr="val"]
        const attrMatch = sel.match(/^([a-zA-Z0-9-_*]*)(?:\[([a-zA-Z0-9-_:]+)(?:=(?:"([^"]*)"|'([^']*)'|([^\]]+)))?\])?$/);
        if (attrMatch && (attrMatch[2] !== undefined)) {
            const tag = attrMatch[1];
            const attrName = attrMatch[2];
            const attrVal = attrMatch[3] !== undefined ? attrMatch[3] : 
                           (attrMatch[4] !== undefined ? attrMatch[4] : 
                           (attrMatch[5] !== undefined ? attrMatch[5] : null));
            if (tag && el.tagName.toLowerCase() !== tag.toLowerCase()) return false;
            if (attrVal !== null) {
                return el.getAttribute(attrName) === attrVal;
            }
            return el.getAttribute(attrName) !== null;
        }

        if (sel.startsWith("#")) {
            return el.id === sel.slice(1);
        }
        if (sel.startsWith(".")) {
            const cls = sel.slice(1);
            return el.classList.contains(cls);
        }
        if (sel.includes(".")) {
            const [t, c] = sel.split(".");
            if (t && el.tagName.toLowerCase() !== t.toLowerCase()) return false;
            return el.classList.contains(c);
        }
        return el.tagName.toLowerCase() === sel.toLowerCase();
    }

    function removeSubtreeIds(node, doc) {
        if (node.id) delete doc._idMap[node.id];
        for (let i = 0; i < node.children.length; i++) {
            removeSubtreeIds(node.children[i], doc);
        }
    }

    function parseHTML(html, parentNode, doc) {
        // Clear previous children and IDs
        for (let i = 0; i < parentNode.children.length; i++) {
            removeSubtreeIds(parentNode.children[i], doc);
        }
        parentNode.children = [];
        parentNode._text = "";

        const regex = /<(\/)?([a-zA-Z0-9-]+)([^>]*)>|([^<]+)/g;
        let match;
        const stack = [parentNode];
        const voidTags = new Set(["input", "br", "hr", "img", "meta", "link"]);

        while ((match = regex.exec(html)) !== null) {
            if (match[4]) {
                const text = match[4];
                if (stack.length > 0) {
                    stack[stack.length - 1]._text += text;
                }
            } else if (match[1]) {
                const tagName = match[2].toLowerCase();
                if (stack.length > 1 && stack[stack.length - 1].tagName.toLowerCase() === tagName) {
                    stack.pop();
                }
            } else {
                const tagName = match[2].toLowerCase();
                const attrStr = match[3] || "";
                const el = doc.createElement(tagName);

                const attrRegex = /([a-zA-Z0-9-_:]+)(?:=(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g;
                let aMatch;
                while ((aMatch = attrRegex.exec(attrStr)) !== null) {
                    const name = aMatch[1];
                    const val = aMatch[2] !== undefined ? aMatch[2] : 
                               (aMatch[3] !== undefined ? aMatch[3] : 
                               (aMatch[4] !== undefined ? aMatch[4] : ""));
                    el.setAttribute(name, val);
                }

                const current = stack[stack.length - 1];
                if (current) current.appendChild(el);

                const isSelfClosing = voidTags.has(tagName) || attrStr.trim().endsWith("/");
                if (!isSelfClosing) {
                    stack.push(el);
                }
            }
        }
    }

    function createMockElement(tagName, doc) {
        return {
            tagName: tagName.toUpperCase(),
            id: "",
            className: "",
            _value: "",
            _text: "",
            style: {},
            children: [],
            parentElement: null,
            _listeners: {},
            attributes: {},

            get value() {
                return this._value;
            },
            set value(v) {
                this._value = String(v);
            },

            get textContent() {
                if (this.children.length === 0) return this._text;
                return this._text + this.children.map(c => c.textContent).join("");
            },
            set textContent(v) {
                this._text = String(v);
                this.children = [];
            },

            get innerHTML() {
                return this._innerHTML || "";
            },
            set innerHTML(html) {
                this._innerHTML = html;
                parseHTML(html, this, doc);
            },

            setAttribute(n, v) {
                this.attributes[n] = String(v);
                if (n === "id") {
                    this.id = String(v);
                    doc._idMap[this.id] = this;
                }
                if (n === "class") {
                    this.className = String(v);
                }
                if (n === "value") {
                    this._value = String(v);
                }
                if (n === "style") {
                    String(v).split(";").forEach(rule => {
                        if (!rule.trim()) return;
                        const [prop, valStr] = rule.split(":");
                        if (prop && valStr) {
                            const camel = prop.trim().replace(/-([a-z])/g, (_, g) => g.toUpperCase());
                            this.style[camel] = valStr.trim();
                        }
                    });
                }
            },
            getAttribute(n) {
                return this.attributes[n] !== undefined ? this.attributes[n] : null;
            },
            removeAttribute(n) {
                delete this.attributes[n];
                if (n === "id") delete doc._idMap[this.id];
            },

            appendChild(c) {
                c.parentElement = this;
                this.children.push(c);
                if (c.id) doc._idMap[c.id] = c;
                return c;
            },
            removeChild(c) {
                const idx = this.children.indexOf(c);
                if (idx >= 0) {
                    this.children.splice(idx, 1);
                    c.parentElement = null;
                    if (c.id) delete doc._idMap[c.id];
                }
                return c;
            },

            addEventListener(type, fn) {
                if (!this._listeners[type]) this._listeners[type] = [];
                this._listeners[type].push(fn);
            },
            removeEventListener(type, fn) {
                if (!this._listeners[type]) return;
                this._listeners[type] = this._listeners[type].filter(f => f !== fn);
            },
            dispatchEvent(evt) {
                const type = typeof evt === "string" ? evt : evt.type;
                const eventObj = typeof evt === "string" ? { type: evt, target: this } : evt;
                if (!eventObj.target) eventObj.target = this;

                if (this._listeners[type]) {
                    this._listeners[type].slice().forEach(fn => fn.call(this, eventObj));
                }
                if (typeof this["on" + type] === "function") {
                    this["on" + type].call(this, eventObj);
                }
                // Bubble up
                if (!eventObj.cancelBubble && this.parentElement) {
                    this.parentElement.dispatchEvent(eventObj);
                }
            },
            click() {
                this.dispatchEvent({ type: "click", target: this });
            },

            get classList() {
                const self = this;
                return {
                    add(c) {
                        const tokens = new Set(self.className.split(/\s+/).filter(Boolean));
                        tokens.add(c);
                        self.className = Array.from(tokens).join(" ");
                    },
                    remove(c) {
                        const tokens = new Set(self.className.split(/\s+/).filter(Boolean));
                        tokens.delete(c);
                        self.className = Array.from(tokens).join(" ");
                    },
                    toggle(c, force) {
                        if (force !== undefined) {
                            if (force) this.add(c);
                            else this.remove(c);
                        } else {
                            if (this.contains(c)) this.remove(c);
                            else this.add(c);
                        }
                    },
                    contains(c) {
                        return self.className.split(/\s+/).filter(Boolean).includes(c);
                    }
                };
            },

            querySelectorAll(sel) {
                const results = [];
                function walk(node) {
                    for (let i = 0; i < node.children.length; i++) {
                        const child = node.children[i];
                        if (matchesSelector(child, sel)) results.push(child);
                        walk(child);
                    }
                }
                walk(this);
                return results;
            },
            querySelector(sel) {
                const all = this.querySelectorAll(sel);
                return all.length > 0 ? all[0] : null;
            },
            closest(sel) {
                let curr = this;
                while (curr) {
                    if (matchesSelector(curr, sel)) return curr;
                    curr = curr.parentElement;
                }
                return null;
            }
        };
    }

    function isConnected(node, doc) {
        let curr = node;
        while (curr) {
            if (curr === doc.body || curr === doc.head) return true;
            curr = curr.parentElement;
        }
        return false;
    }

    const mockDoc = {
        _idMap: idMap,
        _listeners: {},
        createElement(tag) {
            return createMockElement(tag, mockDoc);
        },
        getElementById(id) {
            const cached = this._idMap[id];
            if (cached && isConnected(cached, this)) return cached;
            if (cached) delete this._idMap[id];
            function find(n) {
                if (!n) return null;
                if (n.id === id) return n;
                for (let i = 0; i < n.children.length; i++) {
                    const found = find(n.children[i]);
                    if (found) return found;
                }
                return null;
            }
            const found = find(this.body) || find(this.head);
            if (found) this._idMap[id] = found;
            return found;
        },
        addEventListener(t, fn) {
            if (!this._listeners[t]) this._listeners[t] = [];
            this._listeners[t].push(fn);
        },
        removeEventListener(t, fn) {
            if (!this._listeners[t]) return;
            this._listeners[t] = this._listeners[t].filter(f => f !== fn);
        },
        readyState: "complete"
    };

    mockDoc.head = createMockElement("head", mockDoc);
    mockDoc.body = createMockElement("body", mockDoc);

    const mockStorage = {
        _data: {},
        getItem(k) { return this._data[k] !== undefined ? this._data[k] : null; },
        setItem(k, v) { this._data[k] = String(v); },
        removeItem(k) { delete this._data[k]; },
        clear() { this._data = {}; }
    };

    return { doc: mockDoc, localStorage: mockStorage };
}

// Instantiate environment
const env = createMockEnvironment();
global.document = env.doc;
global.window = {
    document: env.doc,
    localStorage: env.localStorage,
    innerWidth: 800,
    innerHeight: 600
};
global.localStorage = env.localStorage;

// Setup Mock CheatEngine
const cheatEngineCalls = {
    setVariable: [],
    setSwitch: [],
    gainGold: [],
    recoverAll: 0,
    gainItem: [],
    evalScript: [],
    dumpState: 0,
    getState: 0
};

let mockGameState = {
    gold: 500,
    variables: { "1": 10, "15": 42 },
    switches: { "7": true }
};

const mockCheatEngine = {
    setVariable(id, val) {
        cheatEngineCalls.setVariable.push({ id, val });
        mockGameState.variables[String(id)] = val;
        return true;
    },
    setSwitch(id, val) {
        cheatEngineCalls.setSwitch.push({ id, val });
        mockGameState.switches[String(id)] = val;
        return true;
    },
    gainGold(amt) {
        cheatEngineCalls.gainGold.push(amt);
        mockGameState.gold += amt;
        return true;
    },
    recoverAll() {
        cheatEngineCalls.recoverAll++;
        return true;
    },
    gainItem(type, id, count) {
        cheatEngineCalls.gainItem.push({ type, id, count });
        return true;
    },
    evalScript(code, cb) {
        cheatEngineCalls.evalScript.push(code);
        if (code.includes("move_speed")) {
            if (typeof cb === "function") cb("6");
            return "6";
        }
        if (typeof cb === "function") cb("Result: " + code);
        return "Result: " + code;
    },
    dumpState() {
        cheatEngineCalls.dumpState++;
        return true;
    },
    getState() {
        cheatEngineCalls.getState++;
        return mockGameState;
    }
};

global.CheatEngine = mockCheatEngine;
global.window.CheatEngine = mockCheatEngine;

// Load cheat_menu.js
const cheatMenuPath = path.resolve(__dirname, "../../main/assets/overlay/cheat_menu.js");
const CheatMenu = require(cheatMenuPath);

assert(CheatMenu, "CheatMenu module must be exported");

// Initialize DOM
CheatMenu.initDOM();

// Verify overlay created
const overlay = document.getElementById("fnh-cheat-overlay");
assert(overlay, "#fnh-cheat-overlay must be created in document");
console.log("✔ CheatMenu DOM initialized successfully");

// ============================================================================
// TEST 1: FOUR CORE TABS PRESENT & TAB SWITCHING
// ============================================================================
console.log("\n--- Test 1: Tab Switching ---");

const tabVariables = document.getElementById("fnh-tab-variables");
const tabSwitches = document.getElementById("fnh-tab-switches");
const tabSpawner = document.getElementById("fnh-tab-spawner");
const tabConsole = document.getElementById("fnh-tab-console");

assert(tabVariables, "Pane #fnh-tab-variables must exist");
assert(tabSwitches, "Pane #fnh-tab-switches must exist");
assert(tabSpawner, "Pane #fnh-tab-spawner must exist");
assert(tabConsole, "Pane #fnh-tab-console must exist");

// Initial state: Variables is active
assert.strictEqual(tabVariables.style.display, "flex", "Variables pane should initially be visible");
assert.strictEqual(tabSwitches.style.display, "none", "Switches pane should initially be hidden");

// Switch to switches
CheatMenu.setTab("switches");
assert.strictEqual(tabVariables.style.display, "none", "Variables pane should now be hidden");
assert.strictEqual(tabSwitches.style.display, "flex", "Switches pane should now be visible");

// Switch to spawner
CheatMenu.setTab("spawner");
assert.strictEqual(tabSwitches.style.display, "none", "Switches pane should be hidden");
assert.strictEqual(tabSpawner.style.display, "flex", "Spawner pane should be visible");

// Switch to console
CheatMenu.setTab("console");
assert.strictEqual(tabSpawner.style.display, "none", "Spawner pane should be hidden");
assert.strictEqual(tabConsole.style.display, "flex", "Console pane should be visible");

// Switch back to variables
CheatMenu.setTab("variables");
assert.strictEqual(tabVariables.style.display, "flex", "Variables pane should be visible again");
console.log("✔ Tab switching correctly toggles active panes");

// ============================================================================
// TEST 2: VARIABLE SEARCH AND SET DISPATCH
// ============================================================================
console.log("\n--- Test 2: Variable Search & Set Dispatch ---");

const varSearchInput = document.getElementById("fnh-var-search");
const varRefreshBtn = document.getElementById("fnh-var-refresh");
assert(varSearchInput, "Search input #fnh-var-search must exist");
assert(varRefreshBtn, "Refresh button #fnh-var-refresh must exist");

// Initial check: variable 15 has value 42
const varCard15 = document.getElementById("fnh-var-card-15");
assert(varCard15, "Card for Variable #15 should exist");
const varLabel15 = document.getElementById("fnh-var-label-15");
assert(varLabel15.textContent.includes("42"), "Variable #15 label should show 42");

// Test Search filtering
CheatMenu.renderVariables("15");
const varList = document.getElementById("fnh-var-list");
assert(document.getElementById("fnh-var-card-15"), "Variable 15 must match search '15'");
assert(!document.getElementById("fnh-var-card-1"), "Variable 1 should be filtered out by search '15'");

// Reset search
CheatMenu.renderVariables("");
assert(document.getElementById("fnh-var-card-1"), "Variable 1 should be restored after clearing search");

// Test Set Dispatch
cheatEngineCalls.setVariable.length = 0;
const varInput15 = document.getElementById("fnh-var-input-15");
assert(varInput15, "Input field for Variable 15 must exist");
varInput15.value = "999";

const varSetBtn15 = document.getElementById("fnh-var-set-15");
assert(varSetBtn15, "Set button for Variable 15 must exist");
varSetBtn15.click();

assert.strictEqual(cheatEngineCalls.setVariable.length, 1, "CheatEngine.setVariable should be called once");
assert.strictEqual(cheatEngineCalls.setVariable[0].id, "15", "Variable ID should be 15");
assert.strictEqual(cheatEngineCalls.setVariable[0].val, 999, "Variable value should be 999");
const updatedVarLabel15 = document.getElementById("fnh-var-label-15");
assert(updatedVarLabel15.textContent.includes("999"), "Card label should update to 999");

// Test Refresh button
cheatEngineCalls.dumpState = 0;
varRefreshBtn.click();
assert(cheatEngineCalls.dumpState > 0, "Refresh button should trigger dumpState query");
console.log("✔ Variable search, set dispatch, and refresh working properly");

// ============================================================================
// TEST 3: SWITCH TOGGLE DISPATCH
// ============================================================================
console.log("\n--- Test 3: Switch Toggle Dispatch ---");

CheatMenu.setTab("switches");
const switchSearchInput = document.getElementById("fnh-switch-search");
const switchRefreshBtn = document.getElementById("fnh-switch-refresh");
assert(switchSearchInput, "#fnh-switch-search must exist");
assert(switchRefreshBtn, "#fnh-switch-refresh must exist");

// Initial check: switch 7 is ON
const switchCard7 = document.getElementById("fnh-switch-card-7");
assert(switchCard7, "Card for Switch #7 must exist");
const switchToggleBtn7 = document.getElementById("fnh-switch-toggle-7");
assert.strictEqual(switchToggleBtn7.textContent.trim(), "ON", "Switch 7 should initially be ON");

// Toggle Switch 7 -> OFF
cheatEngineCalls.setSwitch.length = 0;
switchToggleBtn7.click();
assert.strictEqual(cheatEngineCalls.setSwitch.length, 1, "CheatEngine.setSwitch should be called once");
assert.strictEqual(cheatEngineCalls.setSwitch[0].id, "7", "Switch ID should be 7");
assert.strictEqual(cheatEngineCalls.setSwitch[0].val, false, "Switch value should be toggled to false");
assert.strictEqual(switchToggleBtn7.textContent.trim(), "OFF", "Toggle button text should change to OFF");

// Toggle Switch 7 -> ON
cheatEngineCalls.setSwitch.length = 0;
switchToggleBtn7.click();
assert.strictEqual(cheatEngineCalls.setSwitch[0].val, true, "Switch value should be toggled back to true");
assert.strictEqual(switchToggleBtn7.textContent.trim(), "ON", "Toggle button text should change back to ON");

// Test Switch Search
CheatMenu.renderSwitches("7");
assert(document.getElementById("fnh-switch-card-7"), "Switch 7 must be shown when searched");
assert(!document.getElementById("fnh-switch-card-1"), "Switch 1 should be filtered out");
CheatMenu.renderSwitches("");

console.log("✔ Switch toggle dispatch, UI update, and search working properly");

// ============================================================================
// TEST 4: ITEM SPAWNER & QUICK CHEATS
// ============================================================================
console.log("\n--- Test 4: Item Spawner & Quick Cheats ---");

CheatMenu.setTab("spawner");

// 1. Max Gold
cheatEngineCalls.gainGold.length = 0;
const btnMaxGold = document.getElementById("fnh-btn-max-gold");
assert(btnMaxGold, "Button #fnh-btn-max-gold must exist");
btnMaxGold.click();
assert.strictEqual(cheatEngineCalls.gainGold.length, 1, "gainGold should be called");
assert.strictEqual(cheatEngineCalls.gainGold[0], 999999, "gainGold should add 999999 gold");

// 2. Recover All
cheatEngineCalls.recoverAll = 0;
const btnRecoverAll = document.getElementById("fnh-btn-recover-all");
assert(btnRecoverAll, "Button #fnh-btn-recover-all must exist");
btnRecoverAll.click();
assert.strictEqual(cheatEngineCalls.recoverAll, 1, "recoverAll should be called once");

// 3. Item Spawner
cheatEngineCalls.gainItem.length = 0;
const spawnerCat = document.getElementById("fnh-spawner-category");
const spawnerId = document.getElementById("fnh-spawner-id");
const spawnerCount = document.getElementById("fnh-spawner-count");
const btnSpawnItem = document.getElementById("fnh-btn-spawn-item");

assert(spawnerCat, "#fnh-spawner-category must exist");
assert(spawnerId, "#fnh-spawner-id must exist");
assert(spawnerCount, "#fnh-spawner-count must exist");
assert(btnSpawnItem, "#fnh-btn-spawn-item must exist");

spawnerCat.value = "weapon";
spawnerId.value = "12";
spawnerCount.value = "3";

btnSpawnItem.click();
assert.strictEqual(cheatEngineCalls.gainItem.length, 1, "gainItem should be called");
assert.strictEqual(cheatEngineCalls.gainItem[0].type, "weapon");
assert.strictEqual(cheatEngineCalls.gainItem[0].id, 12);
assert.strictEqual(cheatEngineCalls.gainItem[0].count, 3);

console.log("✔ Spawner quick actions (Max Gold, Recover All) & item spawn dispatch working properly");

// ============================================================================
// TEST 5: SCRIPT CONSOLE EXECUTION & OUTPUT
// ============================================================================
console.log("\n--- Test 5: Script Console Execution & Output ---");

CheatMenu.setTab("console");

const consoleInput = document.getElementById("fnh-console-input");
const consoleOutput = document.getElementById("fnh-console-output");
const btnRunScript = document.getElementById("fnh-btn-run-script");

assert(consoleInput, "Textarea #fnh-console-input must exist");
assert(consoleOutput, "Output #fnh-console-output must exist");
assert(btnRunScript, "Button #fnh-btn-run-script must exist");

// Run script
cheatEngineCalls.evalScript.length = 0;
consoleInput.value = "$game_player.move_speed = 6";
btnRunScript.click();

assert.strictEqual(cheatEngineCalls.evalScript.length, 1, "evalScript should be called once");
assert.strictEqual(cheatEngineCalls.evalScript[0], "$game_player.move_speed = 6");
assert.strictEqual(consoleOutput.textContent, "6", "Terminal output should render return value '6'");

console.log("✔ Console script evaluation and output terminal rendering working properly");

// ============================================================================
// TEST 6: SNIPPET PERSISTENCE IN LOCALSTORAGE
// ============================================================================
console.log("\n--- Test 6: Snippet Persistence in LocalStorage ---");

const snippetName = document.getElementById("fnh-snippet-name");
const btnSaveSnippet = document.getElementById("fnh-btn-save-snippet");
const snippetSelect = document.getElementById("fnh-snippet-select");
const btnLoadSnippet = document.getElementById("fnh-btn-load-snippet");
const btnDeleteSnippet = document.getElementById("fnh-btn-delete-snippet");

assert(snippetName, "#fnh-snippet-name must exist");
assert(btnSaveSnippet, "#fnh-btn-save-snippet must exist");
assert(snippetSelect, "#fnh-snippet-select must exist");
assert(btnLoadSnippet, "#fnh-btn-load-snippet must exist");
assert(btnDeleteSnippet, "#fnh-btn-delete-snippet must exist");

// 1. Save snippet
snippetName.value = "Turbo Speed";
consoleInput.value = "$game_player.move_speed = 8";
btnSaveSnippet.click();

const rawStorage = localStorage.getItem("vpad_user_cheat_snippets");
assert(rawStorage, "localStorage key 'vpad_user_cheat_snippets' must be populated");
const parsedStorage = JSON.parse(rawStorage);
assert.strictEqual(parsedStorage.length, 1);
assert.strictEqual(parsedStorage[0].name, "Turbo Speed");
assert.strictEqual(parsedStorage[0].code, "$game_player.move_speed = 8");

// Verify select dropdown contains saved snippet
const opt = snippetSelect.querySelector("option[value='Turbo Speed']");
assert(opt, "Dropdown should contain an option for 'Turbo Speed'");

// 2. Clear input and load snippet back
consoleInput.value = "";
snippetName.value = "";
snippetSelect.value = "Turbo Speed";
btnLoadSnippet.click();

assert.strictEqual(consoleInput.value, "$game_player.move_speed = 8", "Console input should restore saved code");
assert.strictEqual(snippetName.value, "Turbo Speed", "Snippet name should restore saved name");

// 3. Delete snippet
btnDeleteSnippet.click();
const updatedStorage = JSON.parse(localStorage.getItem("vpad_user_cheat_snippets"));
assert.strictEqual(updatedStorage.length, 0, "Snippet should be removed from localStorage");

console.log("✔ Snippet save, dropdown listing, loading, and deletion in localStorage working properly");

// ============================================================================
// TEST 7: THEME TOGGLE
// ============================================================================
console.log("\n--- Test 7: Theme Toggle ---");

assert.strictEqual(document.body.classList.contains("vpad-theme-retro"), false);
CheatMenu.theme();
assert.strictEqual(document.body.classList.contains("vpad-theme-retro"), true);
CheatMenu.theme();
assert.strictEqual(document.body.classList.contains("vpad-theme-retro"), false);

console.log("✔ Theme toggle correctly toggles .vpad-theme-retro class");

// ============================================================================
// TEST 8: NEUTRALIZED CSS & ZERO INJECTED HARDCODED STYLES
// ============================================================================
console.log("\n--- Test 8: Neutralized CSS & Zero Injected Hardcoded Styles ---");

const fs = require("fs");
const cheatCode = fs.readFileSync(path.resolve(__dirname, "../../main/assets/overlay/cheat_menu.js"), "utf-8");

// 1. Verify injectStyles did not inject #fnh-cheat-styles
const styleEl = document.getElementById("fnh-cheat-styles");
assert.strictEqual(styleEl, null, "injectStyles must be neutralized and NOT inject #fnh-cheat-styles");

// 2. Verify cheat_menu.js contains zero presentation hex colors
const hexRegex = /#[0-9a-fA-F]{3,8}\b/g;
const hexMatches = cheatCode.match(hexRegex);
assert.strictEqual(hexMatches, null, `cheat_menu.js must have zero presentation hex colors, found: ${hexMatches}`);

// 3. Verify mode toggle button uses semantic classes and no inline background
const modeToggleBtn = document.getElementById("btn-cheat-mode-toggle");
assert(modeToggleBtn, "Mode toggle button must exist");
assert.strictEqual(modeToggleBtn.style.background, undefined, "Mode toggle button must not have inline style.background");

console.log("✔ Neutralized styles and zero hardcoded hex verified in Cheat Menu");

console.log("\n==================================================");
console.log("All 8 Cheat Menu test suites passed successfully! (100%)");
console.log("==================================================");
