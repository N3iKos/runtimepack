# Coffin (g05) — spike outcome

- Date: 2026-10-25
- Status: **BLOCKED**, not by a runtime capability gap.
- Evidence: `G05-BOOTSTRAP-SPIKE-20261025.md`
- Repository change: none. This was a read-only decode.

## What was done

Decoded the eight base64 fragments, inflated the payload, and inventoried what the loader needs.

- 58,432 base64 chars → 43,822 bytes → **151,734 bytes** inflated, valid UTF-8.
- The payload is the game's custom NW.js wrapper (`App`, `Utils`, `Steam`, `Crypto`, `DataManager`), not an asset decoder.

## What was found

Boot is gated:

```js
if (App.requiresSteam() && !Steam.init()) return;
if (!Crypto.hashMatchDRM(0x6ee0ff1a)) return;
```

`BUILD_FLAGS = 0x181` sets `BF_VENDOR_STEAM`. `Steam.init()` returns `false` when `Utils.isNwjs()` is false, and the runtime shim forces exactly that. The game returns before initialising: black screen.

`zlib` is therefore necessary but provably insufficient. Making this build boot would mean making `Steam.init()` succeed without Steam — DRM circumvention, which is out of scope and will not be implemented.

The DRM hash check is legitimate: `XORTARGET` decodes to `Copyrights - Coffin of Andy and Leyley.txt`, a shipped file.

## Capability list a DRM-free build would need

`zlib.inflateSync`, binary `fs.readFileSync`, `fs.readdirSync`/`statSync`/`accessSync`/`mkdirSync`/`unlinkSync`, `crypto.createHash('sha256'/'md5')`, `os.networkInterfaces`, `nw.gui` window control. `Buffer` already exists.

## Decision needed

1. Supply the DRM-free (itch.io) build of Coffin, or
2. Record Coffin as unsupported for the Steam build.

No code work starts before that choice.

## Ledger note

Coffin joins OneShot as a case where the blocker is the game's own desktop/vendor design, not a RuntimePack capability. The distinction matters: OneShot needs desktop-OS interaction, Coffin needs a Steam client. Neither is a missing shim.
