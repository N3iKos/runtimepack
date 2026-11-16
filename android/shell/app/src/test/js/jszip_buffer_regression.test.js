const assert = require("assert");
const fs = require("fs");
const path = require("path");
const vm = require("vm");

const shimPath = path.resolve(__dirname, "../../main/assets/web_shims/runtimepack_web_shims.js");
const shimCode = fs.readFileSync(shimPath, "utf8");

function resolveJsZipPath() {
    if (process.env.VNM_JSZIP_PATH && fs.existsSync(process.env.VNM_JSZIP_PATH)) {
        return process.env.VNM_JSZIP_PATH;
    }
    if (process.env.VNM_GAME_ROOT) {
        const candidate = path.join(process.env.VNM_GAME_ROOT, "data/lib/jszip.min.js");
        if (fs.existsSync(candidate)) return candidate;
    }
    const standardCandidates = [
        "D:/Old-Recvery/Project_2/RPGM-P/Raw-Games/Time for Punishment Class ~Taking Lessons for My Boyfriend~/data/lib/jszip.min.js",
        path.resolve(process.cwd(), "../Old-Recvery/Project_2/RPGM-P/Raw-Games/Time for Punishment Class ~Taking Lessons for My Boyfriend~/data/lib/jszip.min.js")
    ];
    for (const c of standardCandidates) {
        if (fs.existsSync(c)) return c;
    }
    return null;
}

const jszipPath = resolveJsZipPath();

function createSandbox() {
    const sandbox = {
        console,
        setTimeout,
        clearTimeout,
        setInterval,
        clearInterval,
        Uint8Array,
        ArrayBuffer,
        DataView,
        TextEncoder,
        TextDecoder
    };
    sandbox.window = sandbox;
    sandbox.global = sandbox;
    sandbox.self = sandbox;
    vm.createContext(sandbox);
    vm.runInContext(shimCode, sandbox);
    return sandbox;
}

const ctx = createSandbox();
const Buffer = ctx.Buffer;

assert(typeof Buffer === "function", "Buffer must be a callable constructor");
assert(typeof Buffer.isBuffer === "function", "Buffer.isBuffer must be a function");

const b1 = new Buffer(8);
assert.strictEqual(b1.length, 8);
assert(Buffer.isBuffer(b1), "Buffer.isBuffer(new Buffer(8)) must be true");

const b2 = Buffer("hello world", "utf8");
assert(Buffer.isBuffer(b2), "Buffer.isBuffer(Buffer(string)) must be true");
assert.strictEqual(b2.toString("utf8"), "hello world");

const bUtf8 = Buffer.from("Hello 世界 🌍", "utf8");
assert.strictEqual(bUtf8.toString("utf8"), "Hello 世界 🌍");

const bB64 = Buffer.from("SGVsbG8gV29ybGQ=", "base64");
assert.strictEqual(bB64.toString("utf8"), "Hello World");
assert.strictEqual(bB64.toString("base64"), "SGVsbG8gV29ybGQ=");

const bHex = Buffer.from("48656c6c6f", "hex");
assert.strictEqual(bHex.toString("utf8"), "Hello");
assert.strictEqual(bHex.toString("hex"), "48656c6c6f");

assert.throws(() => {
    Buffer.from("test", "unsupported_encoding_xyz");
}, /Unknown encoding|unsupported/i);

const bAlloc = Buffer.alloc(4, 0x41);
assert.strictEqual(bAlloc.toString("utf8"), "AAAA");

const bSlice = b2.slice(0, 5);
assert.strictEqual(bSlice.toString("utf8"), "hello");
assert(Buffer.isBuffer(bSlice));

const bConcat = Buffer.concat([Buffer.from("foo"), Buffer.from("bar")]);
assert.strictEqual(bConcat.toString("utf8"), "foobar");

if (jszipPath) {
    const jszipCode = fs.readFileSync(jszipPath, "utf8");
    vm.runInContext(jszipCode, ctx);
    assert(typeof ctx.JSZip === "function", "JSZip must be loaded");

    const zip = new ctx.JSZip();
    zip.file("nested/test.txt", "Actual JSZip nodebuffer payload \u2603 \u4E16\u754C");
    const generated = zip.generate({ type: "nodebuffer" });

    assert(Buffer.isBuffer(generated), "Generated output must satisfy Buffer.isBuffer");
    assert(generated.length > 0, "Generated zip buffer must not be empty");

    const zip2 = new ctx.JSZip();
    zip2.load(generated);
    const extracted = zip2.file("nested/test.txt").asText();
    assert.strictEqual(extracted, "Actual JSZip nodebuffer payload \u2603 \u4E16\u754C");
} else {
    console.warn("[NOT_TESTED] Genuine JSZip bundle not found via VNM_JSZIP_PATH/VNM_GAME_ROOT; skipping genuine library roundtrip test");
}
console.log("jszip_buffer_regression tests PASSED");
