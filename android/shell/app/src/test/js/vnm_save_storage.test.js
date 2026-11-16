const assert = require("assert");
const fs = require("fs");
const path = require("path");
const vm = require("vm");

const shimPath = path.resolve(__dirname, "../../main/assets/web_shims/runtimepack_web_shims.js");
const shimCode = fs.readFileSync(shimPath, "utf8");

class MockNativeSaveRepository {
    constructor() {
        this.stores = new Map();
        this.allowedNamespaces = new Set(["mz", "mv", "vnm"]);
        this.simulateReadErrorSlots = new Set();
    }
    isValidSlot(slot) {
        if (typeof slot !== "string") return false;
        if (!/^[a-zA-Z0-9_.-]{1,128}$/.test(slot)) return false;
        if (slot.startsWith(".") || slot.includes("..")) return false;
        return true;
    }
    isValidNamespace(ns) {
        if (typeof ns !== "string") return false;
        if (!/^[a-zA-Z0-9_.-]{1,64}$/.test(ns)) return false;
        if (ns.startsWith(".") || ns.includes("..")) return false;
        return true;
    }
    registerNamespace(ns) {
        const n = String(ns).toLowerCase();
        if (!this.isValidNamespace(n)) return false;
        this.allowedNamespaces.add(n);
        return true;
    }
    saveSlot(ns, slot, payload) {
        const n = String(ns).toLowerCase();
        if (!this.isValidNamespace(n) || !this.isValidSlot(slot)) return false;
        if (!this.allowedNamespaces.has(n)) return false;
        if (!this.stores.has(n)) this.stores.set(n, new Map());
        this.stores.get(n).set(String(slot), String(payload));
        return true;
    }
    loadSlot(ns, slot) {
        const n = String(ns).toLowerCase();
        if (!this.isValidNamespace(n) || !this.isValidSlot(slot)) return null;
        if (!this.stores.has(n)) return null;
        if (this.simulateReadErrorSlots.has(String(slot))) return null;
        const val = this.stores.get(n).get(String(slot));
        return val === undefined ? null : val;
    }
    slotExists(ns, slot) {
        const n = String(ns).toLowerCase();
        if (!this.isValidNamespace(n) || !this.isValidSlot(slot)) return false;
        if (!this.stores.has(n)) return false;
        return this.stores.get(n).has(String(slot));
    }
    deleteSlot(ns, slot) {
        const n = String(ns).toLowerCase();
        if (!this.isValidNamespace(n) || !this.isValidSlot(slot)) return false;
        if (!this.allowedNamespaces.has(n)) return false;
        if (!this.stores.has(n)) return false;
        return this.stores.get(n).delete(String(slot));
    }
    listSlots(ns) {
        const n = String(ns).toLowerCase();
        if (!this.isValidNamespace(n) || !this.allowedNamespaces.has(n)) return "[]";
        if (!this.stores.has(n)) return "[]";
        const keys = Array.from(this.stores.get(n).keys()).sort();
        return JSON.stringify(keys);
    }
}

const sharedRepository = new MockNativeSaveRepository();

function createEngineStorageSandbox(repo, gameUid) {
    const memoryLocalStorage = new Map();
    const sandbox = {
        console,
        setTimeout,
        clearTimeout,
        setInterval,
        clearInterval,
        Uint8Array,
        ArrayBuffer,
        TextEncoder,
        TextDecoder,
        localStorage: {
            getItem(k) { return memoryLocalStorage.get(k) || null; },
            setItem(k, v) { memoryLocalStorage.set(k, String(v)); },
            removeItem(k) { memoryLocalStorage.delete(k); },
            clear() { memoryLocalStorage.clear(); }
        },
        $PARAMS: {
            uid: gameUid || "TEST-UID-42",
            isOffline: true,
            platform: { isSteamOS: false }
        },
        RuntimePackNative: {
            registerNamespace(ns) { return repo.registerNamespace(ns); },
            saveSlot(ns, slot, payload) { return repo.saveSlot(ns, slot, payload); },
            loadSlot(ns, slot) { return repo.loadSlot(ns, slot); },
            slotExists(ns, slot) { return repo.slotExists(ns, slot); },
            deleteSlot(ns, slot) { return repo.deleteSlot(ns, slot); },
            listSlots(ns) { return repo.listSlots(ns); }
        }
    };
    sandbox.window = sandbox;
    sandbox.global = sandbox;
    sandbox.self = sandbox;
    vm.createContext(sandbox);

    vm.runInContext(shimCode, sandbox);

    const engineStorageCode = `
        function GameStorage() {}
        GameStorage.prototype.exists = function(e) {
            return $PARAMS.isOffline && !$PARAMS.platform.isSteamOS && null != window.nw
                ? require("fs").existsSync("./" + e + ".vndata")
                : null !== localStorage.getItem($PARAMS.uid + "_" + e);
        };
        GameStorage.prototype.remove = function(e) {
            if ($PARAMS.isOffline && !$PARAMS.platform.isSteamOS && null != window.nw) {
                if (require("fs").existsSync("./" + e + ".vndata")) return require("fs").unlinkSync("./" + e + ".vndata");
            } else {
                return localStorage.removeItem($PARAMS.uid + "_" + e);
            }
        };
        GameStorage.prototype.setData = function(e, t) {
            return $PARAMS.isOffline && !$PARAMS.platform.isSteamOS && null != window.nw
                ? require("fs").writeFileSync("./" + e + ".vndata", t)
                : localStorage.setItem($PARAMS.uid + "_" + e, t);
        };
        GameStorage.prototype.getData = function(e) {
            return $PARAMS.isOffline && !$PARAMS.platform.isSteamOS && null != window.nw
                ? require("fs").readFileSync("./" + e + ".vndata", "utf8")
                : localStorage.getItem($PARAMS.uid + "_" + e);
        };
        GameStorage.prototype.setObject = function(e, t) {
            return this.setData(e, JSON.stringify(t));
        };
        GameStorage.prototype.getObject = function(e) {
            var t = this.getData(e);
            if (t == null) return null;
            try { return JSON.parse(t); } catch(err) { console.warn(e + " corrupted."); return null; }
        };
        window.GameStorage = new GameStorage();
        window.gs = { GameStorage: window.GameStorage };
        if (typeof window.__runtimepackPollVnmStorage === "function") {
            window.__runtimepackPollVnmStorage(window.GameStorage);
        }
    `;
    vm.runInContext(engineStorageCode, sandbox);
    return sandbox;
}

const session1 = createEngineStorageSandbox(sharedRepository, "MY-GAME-UID");
assert(session1.GameStorage.__runtimepack_vnm_save_patched, "GameStorage must be patched by shim");

session1.GameStorage.setData("1", "payload_slot_1_data_日本語_★");
session1.GameStorage.setData("2", "payload_slot_2_data");
session1.GameStorage.setData("3", "payload_slot_3_data");
session1.GameStorage.setObject("1_header", { chapter: 1, playTime: 3600, hero: "Alice" });
session1.GameStorage.setObject("settings", { bgmVolume: 80, textSpeed: 2 });
session1.GameStorage.setObject("globalData", { unlockedGallery: [1, 2, 5] });

assert.strictEqual(session1.GameStorage.exists("1"), true);
assert.strictEqual(session1.GameStorage.exists("2"), true);
assert.strictEqual(session1.GameStorage.exists("3"), true);
assert.strictEqual(session1.GameStorage.exists("nonexistent"), false);

const session2 = createEngineStorageSandbox(sharedRepository, "MY-GAME-UID");
assert.strictEqual(session2.GameStorage.getData("1"), "payload_slot_1_data_日本語_★");
assert.strictEqual(session2.GameStorage.getData("2"), "payload_slot_2_data");
assert.strictEqual(session2.GameStorage.getData("3"), "payload_slot_3_data");

const header1 = session2.GameStorage.getObject("1_header");
assert.deepStrictEqual(JSON.parse(JSON.stringify(header1)), { chapter: 1, playTime: 3600, hero: "Alice" });

const settings = session2.GameStorage.getObject("settings");
assert.deepStrictEqual(JSON.parse(JSON.stringify(settings)), { bgmVolume: 80, textSpeed: 2 });

const globalData = session2.GameStorage.getObject("globalData");
assert.deepStrictEqual(JSON.parse(JSON.stringify(globalData)), { unlockedGallery: [1, 2, 5] });

assert.strictEqual(session2.GameStorage.getData("nonexistent"), null);
assert.strictEqual(session2.GameStorage.getObject("nonexistent"), null);

session2.GameStorage.remove("2");
assert.strictEqual(session2.GameStorage.exists("2"), false);
assert.strictEqual(session2.GameStorage.getData("2"), null);

const failingRepo = new MockNativeSaveRepository();
failingRepo.saveSlot = function() { return false; };
const failingSession = createEngineStorageSandbox(failingRepo, "MY-GAME-UID");

assert.throws(() => {
    failingSession.GameStorage.setData("slot_fail", "data");
}, /RT-SAVE-FAIL|Native saveSlot failed/i);

assert.strictEqual(failingSession.GameStorage.exists("slot_fail"), false);

session2.localStorage.setItem("vpad_theme_selected", "matte");
session2.localStorage.setItem("OTHER-UID_slot1", "preserve_this");
session2.GameStorage.clear();

assert.strictEqual(session2.localStorage.getItem("vpad_theme_selected"), "matte");
assert.strictEqual(session2.localStorage.getItem("OTHER-UID_slot1"), "preserve_this");
assert.strictEqual(session2.GameStorage.exists("1"), false);

const partialFailRepo = new MockNativeSaveRepository();
const partialFailSession = createEngineStorageSandbox(partialFailRepo, "MY-GAME-UID");
partialFailSession.GameStorage.setData("slot_ok", "ok_data");
partialFailSession.GameStorage.setData("slot_bad", "bad_data");
partialFailRepo.deleteSlot = function(ns, slot) {
    if (slot === "slot_bad") return false;
    return true;
};
assert.throws(() => {
    partialFailSession.GameStorage.clear();
}, /RT-SAVE-FAIL/);

// =========================================================================
// Task 2B: Legacy READ-Only Save Fallback RED/GREEN Tests
// =========================================================================

const vnmUid = "BB45114F1292564E907B88654BEDAB153D70";
const legacyRepo = new MockNativeSaveRepository();

// Setup spies
let spyRegisterNamespaceCalls = [];
let spySaveSlotCalls = [];
let spyDeleteSlotCalls = [];
legacyRepo.registerNamespace = function(ns) {
    spyRegisterNamespaceCalls.push(ns);
    return MockNativeSaveRepository.prototype.registerNamespace.call(this, ns);
};
legacyRepo.saveSlot = function(ns, slot, payload) {
    spySaveSlotCalls.push({ ns, slot, payload });
    return MockNativeSaveRepository.prototype.saveSlot.call(this, ns, slot, payload);
};
legacyRepo.deleteSlot = function(ns, slot) {
    spyDeleteSlotCalls.push({ ns, slot });
    return MockNativeSaveRepository.prototype.deleteSlot.call(this, ns, slot);
};

// Seed legacy data in namespace "vnm" using Base64 UTF-8 payloads
const legacySaveSlotKey = encodeURIComponent(vnmUid + "_SaveGame_0");
const legacyHeaderSlotKey = encodeURIComponent(vnmUid + "_0_header");
const legacySettingsSlotKey = encodeURIComponent(vnmUid + "_settings");
const legacyGlobalSlotKey = encodeURIComponent(vnmUid + "_globalData");

const rawLegacySaveObj = { chapter: 5, scene: "Ch5_Scene2", gold: 12000, heroine: "静香 (Shizuka)" };
const rawLegacyHeaderObj = { chapter: 5, title: "Taking Lessons", timestamp: 1728144000 };
const rawLegacySettingsObj = { bgmVolume: 70, voiceVolume: 90, lang: "jp" };
const rawLegacyGlobalObj = { cgUnlocked: [1, 2, 3, 4, 5], endingA: true };

const b64SavePayload = Buffer.from(JSON.stringify(rawLegacySaveObj), "utf8").toString("base64");
const b64HeaderPayload = Buffer.from(JSON.stringify(rawLegacyHeaderObj), "utf8").toString("base64");
const b64SettingsPayload = Buffer.from(JSON.stringify(rawLegacySettingsObj), "utf8").toString("base64");
const b64GlobalPayload = Buffer.from(JSON.stringify(rawLegacyGlobalObj), "utf8").toString("base64");

legacyRepo.saveSlot("vnm", legacySaveSlotKey, b64SavePayload);
legacyRepo.saveSlot("vnm", legacyHeaderSlotKey, b64HeaderPayload);
legacyRepo.saveSlot("vnm", legacySettingsSlotKey, b64SettingsPayload);
legacyRepo.saveSlot("vnm", legacyGlobalSlotKey, b64GlobalPayload);

// Seed foreign UID and unprefixed slots in "vnm" to verify isolation
const foreignUidSlotKey = encodeURIComponent("DIFFERENT_UID_999_SaveGame_0");
const unprefixedSlotKey = encodeURIComponent("SaveGame_0");
legacyRepo.saveSlot("vnm", foreignUidSlotKey, Buffer.from("{\"chapter\":99}", "utf8").toString("base64"));
legacyRepo.saveSlot("vnm", unprefixedSlotKey, Buffer.from("{\"chapter\":0}", "utf8").toString("base64"));

// Seed a malformed Base64 slot in "vnm"
const malformedSlotKey = encodeURIComponent(vnmUid + "_SaveGame_bad");
legacyRepo.saveSlot("vnm", malformedSlotKey, "NOT_VALID_BASE64_!@#$%^&*()");

// Clear spies before session initialization
spyRegisterNamespaceCalls = [];
spySaveSlotCalls = [];
spyDeleteSlotCalls = [];

const legacySession = createEngineStorageSandbox(legacyRepo, vnmUid);

// RED Test 1: Reading absent current slot must fall back to legacy "vnm" Base64 record and decode UTF-8
const loadedSaveStr = legacySession.GameStorage.getData("SaveGame_0");
assert.strictEqual(loadedSaveStr, JSON.stringify(rawLegacySaveObj), "Legacy save string must decode exact UTF-8");

const loadedSaveObj = legacySession.GameStorage.getObject("SaveGame_0");
assert.deepStrictEqual(JSON.parse(JSON.stringify(loadedSaveObj)), rawLegacySaveObj, "Legacy save object must parse all fields");
assert.strictEqual(legacySession.GameStorage.exists("SaveGame_0"), true, "exists must agree with getData for legacy record");

// Test legacy headers, settings, globalData with multilingual CJK
const loadedHeaderObj = legacySession.GameStorage.getObject("0_header");
assert.deepStrictEqual(JSON.parse(JSON.stringify(loadedHeaderObj)), rawLegacyHeaderObj);
assert.strictEqual(legacySession.GameStorage.exists("0_header"), true);

const loadedSettingsObj = legacySession.GameStorage.getObject("settings");
assert.deepStrictEqual(JSON.parse(JSON.stringify(loadedSettingsObj)), rawLegacySettingsObj);

const loadedGlobalObj = legacySession.GameStorage.getObject("globalData");
assert.deepStrictEqual(JSON.parse(JSON.stringify(loadedGlobalObj)), rawLegacyGlobalObj);

// RED Test 2: Current valid record shadows legacy record
const currentSaveObj = { chapter: 6, gold: 50000, hero: "Updated Current Progress" };
legacySession.GameStorage.setData("SaveGame_0", JSON.stringify(currentSaveObj));
assert.strictEqual(legacySession.GameStorage.getData("SaveGame_0"), JSON.stringify(currentSaveObj), "Current record must shadow legacy record");
assert.deepStrictEqual(JSON.parse(JSON.stringify(legacySession.GameStorage.getObject("SaveGame_0"))), currentSaveObj);

// RED Test 3: Current empty/corrupt record dominates; NO fallback to old progress
legacySession.GameStorage.setData("SaveGame_0", "");
assert.strictEqual(legacySession.GameStorage.getData("SaveGame_0"), "", "Empty current record must NOT fall back to legacy");
assert.strictEqual(legacySession.GameStorage.exists("SaveGame_0"), true, "Empty current record still exists");
assert.strictEqual(legacySession.GameStorage.getObject("SaveGame_0"), null, "Empty record returns null object without fallback");

legacySession.GameStorage.setData("SaveGame_0", "{malformed json content...");
assert.strictEqual(legacySession.GameStorage.getData("SaveGame_0"), "{malformed json content...", "Corrupt current record returned as-is");
assert.strictEqual(legacySession.GameStorage.exists("SaveGame_0"), true);
assert.strictEqual(legacySession.GameStorage.getObject("SaveGame_0"), null, "Corrupt current record must NOT fall back to legacy");

// RED Test 4: Absent both returns null and false
assert.strictEqual(legacySession.GameStorage.getData("SaveGame_999"), null);
assert.strictEqual(legacySession.GameStorage.getObject("SaveGame_999"), null);
assert.strictEqual(legacySession.GameStorage.exists("SaveGame_999"), false);

// RED Test 5: Foreign UID in legacy is ignored
assert.strictEqual(legacySession.GameStorage.exists("SaveGame_foreign"), false);
assert.strictEqual(legacySession.GameStorage.getData("SaveGame_foreign"), null);

// RED Test 6: Unprefixed candidate slot is NOT auto-read when UID is known
assert.strictEqual(legacySession.GameStorage.getData("SaveGame_unprefixed"), null);
assert.strictEqual(legacySession.GameStorage.exists("SaveGame_unprefixed"), false);

// RED Test 7: Malformed legacy Base64 returns null and logs error
assert.strictEqual(legacySession.GameStorage.getData("SaveGame_bad"), null);
assert.strictEqual(legacySession.GameStorage.getObject("SaveGame_bad"), null);
assert.strictEqual(legacySession.GameStorage.exists("SaveGame_bad"), false);

// RED Test 8: Legacy byte-for-byte unchanged across reads
assert.strictEqual(legacyRepo.loadSlot("vnm", legacyHeaderSlotKey), b64HeaderPayload, "Legacy record in repository must remain byte-for-byte unchanged");

// RED Test 9: Zero writes, zero deletes, zero registerNamespace during legacy read
const readsOnlyRepo = new MockNativeSaveRepository();
readsOnlyRepo.saveSlot("vnm", legacyHeaderSlotKey, b64HeaderPayload);
let readSpyRegisters = [];
let readSpySaves = [];
let readSpyDeletes = [];
readsOnlyRepo.registerNamespace = (ns) => { readSpyRegisters.push(ns); return true; };
readsOnlyRepo.saveSlot = (ns, s, p) => { readSpySaves.push({ ns, s, p }); return true; };
readsOnlyRepo.deleteSlot = (ns, s) => { readSpyDeletes.push({ ns, s }); return true; };

const readOnlySession = createEngineStorageSandbox(readsOnlyRepo, vnmUid);
readOnlySession.localStorage.setItem = () => { throw new Error("localStorage.setItem called during read!"); };

assert.doesNotThrow(() => {
    const val = readOnlySession.GameStorage.getData("0_header");
    assert.strictEqual(val, JSON.stringify(rawLegacyHeaderObj));
    const obj = readOnlySession.GameStorage.getObject("0_header");
    assert.deepStrictEqual(JSON.parse(JSON.stringify(obj)), rawLegacyHeaderObj);
    const ex = readOnlySession.GameStorage.exists("0_header");
    assert.strictEqual(ex, true);
});

assert.strictEqual(readSpyRegisters.length, 0, "No registerNamespace calls during legacy read fallback");
assert.strictEqual(readSpySaves.length, 0, "No saveSlot calls during legacy read fallback");
assert.strictEqual(readSpyDeletes.length, 0, "No deleteSlot calls during legacy read fallback");

// RED Test 10: Current remove and clear do NOT delete from "vnm" namespace
readsOnlyRepo.saveSlot(vnmUid, "SaveGame_1", "current_data_to_remove");
readOnlySession.GameStorage.remove("SaveGame_1");
assert.strictEqual(readsOnlyRepo.loadSlot("vnm", legacyHeaderSlotKey), b64HeaderPayload, "Legacy vnm records preserved after remove");

readOnlySession.GameStorage.clear();
assert.strictEqual(readsOnlyRepo.loadSlot("vnm", legacyHeaderSlotKey), b64HeaderPayload, "Legacy vnm records preserved after clear");

// Round 1 Test 11: Current presence true but loadSlot null -> explicit RT-LOAD-FAIL, NO fallback to legacy
const errorSimRepo = new MockNativeSaveRepository();
errorSimRepo.registerNamespace(vnmUid);
errorSimRepo.saveSlot(vnmUid, "SaveGame_corrupt_read", "placeholder");
errorSimRepo.saveSlot("vnm", encodeURIComponent(vnmUid + "_SaveGame_corrupt_read"), Buffer.from("{\"chapter\":1}", "utf8").toString("base64"));
errorSimRepo.simulateReadErrorSlots.add("SaveGame_corrupt_read");

const errorSession = createEngineStorageSandbox(errorSimRepo, vnmUid);
assert.strictEqual(errorSimRepo.slotExists(vnmUid, "SaveGame_corrupt_read"), true);
assert.strictEqual(errorSimRepo.loadSlot(vnmUid, "SaveGame_corrupt_read"), null);

assert.throws(() => {
    errorSession.GameStorage.getData("SaveGame_corrupt_read");
}, /RT-LOAD-FAIL/);

// Round 1 Test 12: Process-restart unregistered namespace read allowed, writes/deletes denied
const restartRepo = new MockNativeSaveRepository();
// Simulate previous session having written to disk:
restartRepo.stores.set(vnmUid.toLowerCase(), new Map([
    ["SaveGame_0", "{\"chapter\":5,\"gold\":12000}"]
]));
// Fresh session: allowedNamespaces only has mz, mv, vnm (vnmUid is UNREGISTERED)
assert.strictEqual(restartRepo.allowedNamespaces.has(vnmUid.toLowerCase()), false);

const restartSession = createEngineStorageSandbox(restartRepo, vnmUid);
assert.strictEqual(restartSession.GameStorage.exists("SaveGame_0"), true);
assert.strictEqual(restartSession.GameStorage.getData("SaveGame_0"), "{\"chapter\":5,\"gold\":12000}");
assert.deepStrictEqual(JSON.parse(JSON.stringify(restartSession.GameStorage.getObject("SaveGame_0"))), { chapter: 5, gold: 12000 });

// Unregistered mutation denied without registerNamespace
assert.strictEqual(restartRepo.deleteSlot(vnmUid, "SaveGame_0"), false);
assert.strictEqual(restartRepo.saveSlot(vnmUid, "SaveGame_1", "data"), false);

console.log("vnm_save_storage tests PASSED");
