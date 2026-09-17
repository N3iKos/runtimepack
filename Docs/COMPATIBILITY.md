# Compatibility Analysis and Shim Policy

## 1. Principle

Compatibility is a first-class build artifact.

The system must answer:

```text
what did we detect?
what external/runtime behavior does the game need?
does our selected runtime provide it?
what is unknown?
```

## 2. Compatibility report model

```json
{
  "status": "COMPATIBLE_WITH_SHIMS",
  "requirements": [],
  "provided": [],
  "missing": [],
  "warnings": [],
  "native_dependencies": []
}
```

## 3. RGSS scanner

Static checks:

- parse `Game.ini`;
- inventory `.dll` files;
- identify script archive;
- optionally decode/analyze scripts only when a lawful and implemented parser exists;
- look for Win32API references when script source is available;
- inventory audio/video formats.

Never execute Ruby during scan.

### Native DLL rule

Any game-shipped PE DLL:

```text
-> CMP-210_NATIVE_WINDOWS_DLL
```

unless a compatibility database entry maps it to an approved Android replacement.

### Win32 shim model

```text
Win32 function
 -> capability ID
 -> shim implementation
 -> test fixture
```

Example:

```text
user32.GetKeyboardState
 -> rgss.win32.user32.keyboard
```

No catch-all fake-success behavior.

## 4. MV/MZ scanner

Scan JS source safely as text/AST where feasible.

Flag:

```text
require(
process.
nw.
nw.Window
greenworks
steamworks
*.node
child_process
fs calls outside allowed subset
```

Classification:

```text
browser-standard -> supported
known shim -> supported with shim
native module -> unsupported
unknown dynamic require -> unknown
```

JoiPlay itself publicly warns that uncommon Node.js classes/functions can prevent games from working [S01], so this is a central requirement rather than an edge case.

## 5. Ren'Py scanner

Inventory:

```text
.rpy
.rpyc
.rpa
Python packages
.so/.pyd/.dll
game metadata
Ren'Py runtime/version hints
```

Native package finding:

```text
.pyd/.dll -> Windows native dependency
.so -> inspect ABI/platform if safe
```

Do not assume pure-Python compatibility if imports are dynamically generated and unresolved.

## 6. Tyrano scanner

Inventory:

```text
tyrano/
data/
plugins
index.html
require(
process
studio_api
NW-specific patterns
```

Technical status may be compatible while release status remains BLOCKED due to licensing.

## 7. Compatibility database

Data files, not scattered hardcoded exceptions.

Example:

```yaml
id: rgss-user32-getkeyboardstate
engine:
  - rpgmaker-xp
  - rpgmaker-vx
  - rpgmaker-vxace

detect:
  kind: ruby_symbol
  value: GetKeyboardState

requires:
  - rgss.win32.user32.keyboard

severity_if_missing: unsupported
```

## 8. Game-specific patches

A patch is identified by content fingerprint, not game title alone.

```yaml
match:
  files:
    - path: Data/Scripts.rvdata2
      sha256: ...
```

This prevents patches from applying to a different release that happens to have the same title.

## 9. Status calculation

```text
missing hard requirement -> UNSUPPORTED_DEPENDENCY
unknown high-risk requirement -> UNKNOWN
all requirements provided, shims non-empty -> COMPATIBLE_WITH_SHIMS
all requirements provided, no shims -> COMPATIBLE
known behavior loss -> DEGRADED
license/policy gate -> BLOCKED
```

## 10. No fake scores

Do not output arbitrary `93% compatible`.

If future versions introduce a score, its formula and benchmark must be documented and testable.
