# Haven: Secret of Caledria — render findings and verification

Date: 27 October 2026
Status: **BLACK RECTANGLES RESOLVED AND VERIFIED ON DEVICE. One residual capacity risk documented (not fixed).**

## Scope
Finish the black rectangle remaining below the enemy name after the white battle cursor became visible; document root causes, fixes, verification, rejected approaches and limits. Agent used read-only ADB/DevTools observations; the user performed all gameplay and visual acceptance.

## Final artifacts

| Item | Value |
|---|---|
| Installed / accepted candidate | `g08-Haven-v7.0.1-beta-windows-20261027-cache-fix-v2.apk` |
| Size | 279,059,897 bytes |
| SHA-256 | `757EC645075DB05E1E1DB5AFC8E993BFCE7B81C757052A0A3C69F4035159C9EF` |
| Package / version | `io.runtimepack.linux20261022.g08`, 1.0.0 (vc 1), label `Haven v7.0.1 Beta Linux` |
| Certificate SHA-256 | `52af1a306d054e52508edb3de4ed4e544d6c8b632e9b7ea49404fab95d45e74d` (matches previous installs) |
| Embedded shim | `runtimepack_web_shims.js`, 164,664 B, `e0f0cfbd…e03127`, byte-identical to the reviewed tree |
| Game payload | GPACK 1,688 records, byte-identical to the original delivery minus `debug.log`, no personal PC saves |

Superseded and explicitly marked do-not-install: `…-20261027-cache-fix.apk` (`6189df50…`), which embedded the interim 1,024-texture revision.

Source baseline: `runtime-v2`, branch `fix/linux-batch-compat-20261022`, HEAD `1c6683c415e9e4a2ef53c56fb3658ca1b34c692d` plus reviewed uncommitted changes. Detailed evidence:
`D:\02_Development\Raw-Games\Sample\port_apk\_reports\haven-render-20261026\`.

## Defect 1 — oversized atlas (battle cursor)
Device context reported `MAX_TEXTURE_SIZE = 4096`; `img/system/IconSet.png_` is 512x4128, above that limit. Packed and source bytes matched and in-memory decryption produced a valid 512x4128 ARGB PNG, so this was never asset corruption. PIXI logged `texImage2D: width or height out of range`.

Fix: per-sprite GPU-fit frame texture built from a 1:1 CPU copy of the exact requested source rectangle, leaving the original `Bitmap`, its `baseTexture`, and `TilingSprite` behaviour untouched. Frames larger than the limit stay on the engine path with a structured `frame-exceeds-limit` event rather than silent clipping.

Result: the white hand cursor now renders.

## Defect 2 — frame-texture cache exhaustion (remaining rectangle)
Measured log:

```
CAP-WEB-031 {"event":"frame-cache-full","owned":32,"bytes":131072,"requestedBytes":4096,"maxTextures":32,"maxBytes":33554432}
```

Scene scan at the rectangle's coordinates found three `Sprite` objects, 32x32, `frame 0:0:32:32`, `bt=512x4128` (the oversized atlas), `own=N`. Cause: the count ceiling of 32 owned textures was consumed by 26 invisible `Sprite_WeaknessIcon` copies plus 6 `Sprite_BattleSelectCursor` copies; the 33rd consumer — the `NUUN_GaugeValueEX` label icon, a plain `Sprite` holding the IconSet bitmap (`NUUN_GaugeValueEX.js:1297-1308`) — was refused and kept the oversized base texture, which cannot upload, so it sampled black.

The byte budget was only 0.4% used (131,072 of 33,554,432), so an arbitrary texture count, not memory, was the binding limit.

Fix: memory is the primary bound (32 MiB unchanged); the count ceiling became a documented backstop at 2,048 (2,048 x 4,096 B = 8 MiB, 25% of budget, 62x the measured 33 requesters). Refusals are now structured (`reason: bytes | count`), capacity refusals are queued for a bounded retry that is drained deterministically from the release path, and an assign-failure now restores the previous entry instead of leaking it.

## Verification (device evidence, not build success)

- Installed `base.apk` SHA-256 equals the candidate; package flags include `DEBUGGABLE`; `versionCode` unchanged so it installed as an update.
- Save files `config`, `file1`, `global` retained identical hashes across the update.
- New process: **no `frame-cache-full` event at all**; the only remaining CAP-WEB-031 event is the honest `frame-exceeds-limit` for a whole-atlas request.
- `RuntimePack.oversizedFrameStats()` in `Scene_Battle` reports `owned: 90`, `bytes: 368,640`, `maxTextures: 2048` — previously pinned at 32.
- Scene scan for sprites bound to the raw 512x4128 base texture returns **zero hits**; three such sprites existed at the rectangle before the fix.
- User confirmed visually that the remaining black rectangle is gone.

## Rejected approaches
- Atlas tile grid with the primary `baseTexture` rebound to tile 0, and clipping of frames crossing a tile boundary: rejected before deployment for pixel loss and for changing full-frame/TilingSprite semantics.
- Cropping or downscaling the shipped atlas: rejected — it removes content and breaks dynamic icon indices.
- Treating the CPU-copy state-icon path or a dark icon as the cause: disproved (rendered icons 1556, 1669, 1555, 2043 and 0 directly from the atlas; all are normal graphics).
- Interpreting an empty `_glTextures` map as proof that upload never succeeded: retracted, because PIXI garbage collection can clear those records.

## Residual risks and limits
1. **Owned-texture count grows across scene changes (not fixed).** MZ `SceneManager.changeScene` calls only `terminate()` and never destroys the outgoing scene's sprites (`rmmz_managers.js:2125-2131`), so `__rpFrameTexture` entries are not released and the counter drifts. Observed ~90 per battle, so the 2,048 ceiling would be reached after roughly 20 battles in one process, at which point the refusal path returns. The correct fix is a shared per-frame texture cache (many sprites reuse a handful of icon frames) rather than per-sprite ownership.
2. A destroyed-but-still-queued sprite keeps a dead retry slot: PIXI 5.3.12 does not expose `DisplayObject.destroyed`, so the drain guard is inert. The harmful grant is still prevented because PIXI nulls `_texture` on destroy, but the queue entry is not reclaimed.
3. Whole-atlas frames (512x4128 requested as one quad) and oversized `TilingSprite` remain unsupported and are logged, not silently clipped.
4. Only `IconSet` was measured; other atlases were not exercised, though the cache scan showed no other bitmap above the limit in this game.
5. Verification covers one device (MuMu, Adreno-class virtual GPU), this game, and the scenes the user played. It is not a claim about all devices or all RPG Maker titles.
6. Android host unit tests were not re-run for this change (JS-only change); the earlier host run was 252/258 with six Ren'Py real-symlink failures caused by Windows privileges, reported not hidden.

## Follow-ups (not in this task)
Shared per-frame texture cache to remove the counter drift; reclaim dead retry slots via `!sprite.texture`; Diagnostic Suite truthfulness (completion versus verified rendering); the no-op/synthetic API stubs in the web shim; the RGSS unconditionally-enabled WebView debugging debt.