# Engine Support Matrix v1

## 1. Scope matrix

| Engine | Engine generation | Runtime family | v1 status | Reference basis |
|---|---|---|---|---|
| RPG Maker XP | RGSS1 | RGSS native | Required | S01, S03, S04, S06 |
| RPG Maker VX | RGSS2 | RGSS native | Required | S01, S03, S04, S06 |
| RPG Maker VX Ace | RGSS3 | RGSS native | Required | S01, S03, S04, S06 |
| RPG Maker MV | JavaScript/HTML5 | Web | Required | S01, S13 |
| RPG Maker MZ | JavaScript/HTML5 | Web | Required | S01, S14 |
| Ren'Py | Python/Ren'Py | Ren'Py | Required | S01, S07, S08 |
| TyranoBuilder/TyranoScript | HTML/JavaScript/KAG | Web/Tyrano adapter | Technical target, legal gate | S01, S10, S11, S12 |

## 2. Explicitly excluded from v1

| Engine | Reason |
|---|---|
| RPG Maker 2000/2003 | Not in current JoiPlay primary support list and would require a separate runtime family |
| RPG Maker Unite | Different technology/runtime; not in current JoiPlay primary list |
| Flash | Deferred |
| Construct | Deferred |
| Godot | Deferred |
| NScripter | Deferred |

## 3. Compatibility categories per family

### RGSS

Expected easy path:

```text
standard RGSS scripts
standard graphics/audio
no native DLL
limited/no unsupported Win32API
```

Risk path:

```text
Win32API
Windows DLL
custom native Ruby extension
unusual codecs
OS-path assumptions
```

mkxp-z itself describes compatibility as broad but not universal and notes that demanding projects may require porting [S06]. Therefore the project must never represent RGSS compatibility as universal.

### MV/MZ

Expected easy path:

```text
browser-compatible project
standard engine APIs
JS plugins using browser APIs
local game assets
```

Risk path:

```text
NW.js-specific API
Node filesystem/process assumptions
native .node modules
Steam/Discord native integration
Windows DLL dependencies
```

JoiPlay's public description also calls out uncommon Node.js functions/classes as a source of incompatibility [S01].

### Ren'Py

Expected easy path:

```text
standard Ren'Py
pure Python supported by target runtime
standard image/audio/video stack
```

Risk path:

```text
compiled Python modules
desktop-only subprocess behavior
OS-specific Python packages
runtime-generation mismatch
```

Versioning is important. JoiPlay has historically provided multiple Ren'Py plugins and created a new one for a breaking Ren'Py update [S18].

### Tyrano

Expected technical easy path:

```text
standard browser-capable Tyrano project
index.html + tyrano/ + data/
pure JS plugins compatible with Android web environment
```

Risk path:

```text
desktop packaging-only APIs
Node/NW integration
third-party native plugin
engine-file redistribution restrictions
```

The last item is not a runtime issue; it is a release/legal gate [S10].
