const assert = require("assert");
const path = require("path");

console.log("Running Cheat Engine Primitives & Bridge Tests...");

const cheatEnginePath = path.resolve(__dirname, "../../main/assets/overlay/cheat_engine.js");
const CheatEngine = require(cheatEnginePath);

assert(CheatEngine, "CheatEngine module must be exported");

// ============================================================================
// PART 1: NATIVE RGSS MODE TESTS
// ============================================================================
console.log("\n--- Part 1: Native RGSS Mode Tests ---");

const postedCmds = [];
let mockCheatState = JSON.stringify({
    gold: 9999,
    variables: { "1": 100, "15": 42 },
    switches: { "7": true }
});
let mockEvalResult = "42";

const mockVPad = {
    postCheatCmd: function (jsonStr) {
        const parsed = JSON.parse(jsonStr);
        postedCmds.push(parsed);
        return true;
    },
    getCheatState: function () {
        return mockCheatState;
    },
    getEvalResult: function () {
        const res = mockEvalResult;
        mockEvalResult = "";
        return res;
    }
};

global.RuntimePackVPad = mockVPad;
if (typeof window !== "undefined") {
    window.RuntimePackVPad = mockVPad;
}

CheatEngine.setMode("native");
assert.strictEqual(CheatEngine.isNative(), true, "CheatEngine should be in Native mode");

// 1. setVariable
postedCmds.length = 0;
const setVarRes = CheatEngine.setVariable(15, 42);
assert.strictEqual(setVarRes, true);
assert.strictEqual(postedCmds.length, 1);
assert.deepStrictEqual(postedCmds[0], { action: "set_variable", id: 15, value: 42 });
console.log("✔ Native setVariable(15, 42) posts valid JSON command");

// 2. setSwitch
postedCmds.length = 0;
CheatEngine.setSwitch(7, true);
assert.strictEqual(postedCmds.length, 1);
assert.deepStrictEqual(postedCmds[0], { action: "set_switch", id: 7, value: true });

CheatEngine.setSwitch(7, 0);
assert.strictEqual(postedCmds.length, 2);
assert.deepStrictEqual(postedCmds[1], { action: "set_switch", id: 7, value: false });
console.log("✔ Native setSwitch(7, true/false) posts valid JSON command");

// 3. gainGold
postedCmds.length = 0;
CheatEngine.gainGold(5000);
assert.strictEqual(postedCmds.length, 1);
assert.deepStrictEqual(postedCmds[0], { action: "gain_gold", amount: 5000 });
console.log("✔ Native gainGold(5000) posts valid JSON command");

// 4. recoverAll
postedCmds.length = 0;
CheatEngine.recoverAll();
assert.strictEqual(postedCmds.length, 1);
assert.deepStrictEqual(postedCmds[0], { action: "recover_all" });
console.log("✔ Native recoverAll() posts valid JSON command");

// 5. gainItem (item, weapon, armor)
postedCmds.length = 0;
CheatEngine.gainItem("item", 1, 5);
CheatEngine.gainItem("weapon", 10, 1);
CheatEngine.gainItem("armor", 20, 2);
assert.strictEqual(postedCmds.length, 3);
assert.deepStrictEqual(postedCmds[0], { action: "gain_item", type: "item", id: 1, amount: 5 });
assert.deepStrictEqual(postedCmds[1], { action: "gain_item", type: "weapon", id: 10, amount: 1 });
assert.deepStrictEqual(postedCmds[2], { action: "gain_item", type: "armor", id: 20, amount: 2 });
console.log("✔ Native gainItem() posts valid type, id, amount JSON commands");

// 6. requestStateDump & getLoadedState
postedCmds.length = 0;
CheatEngine.requestStateDump();
assert.strictEqual(postedCmds.length, 1);
assert.deepStrictEqual(postedCmds[0], { action: "dump_state" });

const loadedState = CheatEngine.getLoadedState();
assert.strictEqual(loadedState.gold, 9999);
assert.strictEqual(loadedState.variables["15"], 42);
assert.strictEqual(loadedState.switches["7"], true);
console.log("✔ Native requestStateDump() & getLoadedState() work correctly");

// 7. evalScript
postedCmds.length = 0;
mockEvalResult = "123";
let evalCallbackCalled = false;
CheatEngine.evalScript("$foo = 123", function (res) {
    evalCallbackCalled = true;
    assert.strictEqual(res, "123");
});
assert.strictEqual(postedCmds.length, 1);
assert.deepStrictEqual(postedCmds[0], { action: "eval", code: "$foo = 123" });
assert.strictEqual(evalCallbackCalled, true);
console.log("✔ Native evalScript() posts eval command and receives result");

// ============================================================================
// PART 2: WEB MV/MZ MODE TESTS
// ============================================================================
console.log("\n--- Part 2: Web MV/MZ Mode Tests ---");

CheatEngine.setMode("web");
assert.strictEqual(CheatEngine.isNative(), false, "CheatEngine should be in Web mode");

// Setup Web MV/MZ Mocks
const mockVariables = {
    _data: {},
    setValue: function (id, val) { this._data[id] = val; },
    value: function (id) { return this._data[id] || 0; }
};

const mockSwitches = {
    _data: {},
    setValue: function (id, val) { this._data[id] = val; },
    value: function (id) { return this._data[id] || false; }
};

const mockPartyMembers = [
    { hp: 10, mhp: 100, mp: 0, mmp: 50, recoverAll: function () { this.hp = this.mhp; this.mp = this.mmp; } },
    { hp: 1, mhp: 80, mp: 0, mmp: 40, recoverAll: function () { this.hp = this.mhp; this.mp = this.mmp; } }
];

const mockParty = {
    _gold: 100,
    _items: {},
    gainGold: function (amount) { this._gold += amount; },
    gold: function () { return this._gold; },
    allMembers: function () { return mockPartyMembers; },
    gainItem: function (item, amount) {
        if (!item) return;
        this._items[item.id] = (this._items[item.id] || 0) + amount;
    }
};

global.$gameVariables = mockVariables;
global.$gameSwitches = mockSwitches;
global.$gameParty = mockParty;
global.$dataItems = { 1: { id: 1, name: "Potion" } };
global.$dataWeapons = { 5: { id: 5, name: "Iron Sword" } };
global.$dataArmors = { 8: { id: 8, name: "Iron Shield" } };

// 1. setVariable in Web mode
CheatEngine.setVariable(3, 777);
assert.strictEqual(mockVariables.value(3), 777);
console.log("✔ Web setVariable(3, 777) calls $gameVariables.setValue");

// 2. setSwitch in Web mode
CheatEngine.setSwitch(9, true);
assert.strictEqual(mockSwitches.value(9), true);
CheatEngine.setSwitch(9, false);
assert.strictEqual(mockSwitches.value(9), false);
console.log("✔ Web setSwitch(9, true/false) calls $gameSwitches.setValue");

// 3. gainGold in Web mode
CheatEngine.gainGold(300);
assert.strictEqual(mockParty.gold(), 400);
console.log("✔ Web gainGold(300) calls $gameParty.gainGold");

// 4. recoverAll in Web mode
CheatEngine.recoverAll();
assert.strictEqual(mockPartyMembers[0].hp, 100);
assert.strictEqual(mockPartyMembers[0].mp, 50);
assert.strictEqual(mockPartyMembers[1].hp, 80);
assert.strictEqual(mockPartyMembers[1].mp, 40);
console.log("✔ Web recoverAll() calls recoverAll on all members");

// 5. gainItem in Web mode
CheatEngine.gainItem("item", 1, 10);
assert.strictEqual(mockParty._items[1], 10);

CheatEngine.gainItem("weapon", 5, 2);
assert.strictEqual(mockParty._items[5], 2);

CheatEngine.gainItem("armor", 8, 1);
assert.strictEqual(mockParty._items[8], 1);
console.log("✔ Web gainItem() gains item, weapon, and armor");

// 6. requestStateDump & getLoadedState in Web mode
CheatEngine.requestStateDump();
const webLoaded = CheatEngine.getLoadedState();
assert.strictEqual(webLoaded.gold, 400);
assert.strictEqual(webLoaded.variables["3"], 777);
console.log("✔ Web requestStateDump() & getLoadedState() construct accurate state snapshot");

// 7. evalScript in Web mode
let webEvalCallback = null;
const evalRes = CheatEngine.evalScript("10 + 25", function (val) {
    webEvalCallback = val;
});
assert.strictEqual(evalRes, 35);
assert.strictEqual(webEvalCallback, "35");
console.log("✔ Web evalScript('10 + 25') executes and passes result to callback");

// ============================================================================
// PART 3: BACKWARD COMPATIBILITY & PROFILE CHECKS
// ============================================================================
console.log("\n--- Part 3: Backward Compatibility & Profile ---");

CheatEngine.profile = {
    forbidden_features: ["forbidden_action"]
};
const blockedRes = CheatEngine.executeAction("forbidden_action");
assert.strictEqual(blockedRes.success, false);
assert(blockedRes.error.includes("forbidden"));
console.log("✔ Forbidden features in profile are blocked");

const healRes = CheatEngine.executeAction("heal_all");
assert.strictEqual(healRes.success, true);
console.log("✔ Fallback executeAction('heal_all') routes to recoverAll");

console.log("\n==================================================");
console.log("All CheatEngine primitive tests passed successfully!");
console.log("==================================================");
