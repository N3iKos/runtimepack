# Technical Specification v1

## 1. Normative architecture

RuntimePack consists of a host-side build system and an Android-side runtime host.

```text
HOST
┌──────────────────────────────────────────────────────────┐
│ CLI                                                      │
│  -> Ingest                                               │
│  -> Static Detector                                      │
│  -> Compatibility Analyzer                               │
│  -> Runtime Resolver                                     │
│  -> Content Packer                                       │
│  -> Android Project Generator                            │
│  -> Gradle/Signing Driver                                │
└───────────────────────────┬──────────────────────────────┘
                            │
                            v
ANDROID OUTPUT
┌──────────────────────────────────────────────────────────┐
│ BootstrapActivity                                        │
│ RuntimeHost                                              │
│ Content/VFS                                              │
│ Save/Input/Lifecycle/Diagnostics                         │
│ Selected backend                                         │
│  - RGSS                                                  │
│  - Web (MV/MZ)                                           │
│  - Ren'Py                                                │
│  - Tyrano adapter over Web runtime                       │
└──────────────────────────────────────────────────────────┘
```

### TECH-CORE-001

The host core MUST be engine-agnostic. Engine-specific logic belongs in detectors, analyzers, adapters, or backends.

### TECH-CORE-002

The generated APK MUST contain exactly one `BuildManifest` describing the engine, runtime, content format, permissions, ABI set, and tool versions.

### TECH-CORE-003

`inspect` MUST NOT execute game scripts, load native libraries from the game, launch NW.js, launch Python game code, or invoke an RPG Maker executable.

### TECH-CORE-004

The input directory MUST be treated as immutable unless an explicit `--in-place` mode is added in a future specification.

## 2. Recommended implementation language split

This is a recommendation, not a hard license constraint.

```text
Host CLI/core            Rust
Android app shell        Kotlin
RGSS runtime             C/C++ + Ruby runtime
Web backend              Kotlin/Android WebView + JS bridge
Ren'Py backend           upstream Ren'Py/Python Android runtime
Schemas                  JSON Schema
Build orchestration      Rust -> Gradle subprocess
```

Rationale:

- host work is heavy on filesystem, hashing, archive handling, diagnostics, and deterministic processing;
- Android lifecycle/storage/UI integration is native Kotlin territory;
- RGSS reference runtimes are C/C++/Ruby lineage;
- MV/MZ and Tyrano are web-technology engines.

## 3. Core domain types

### 3.1 EngineId

```rust
enum EngineId {
    RpgMakerXp,
    RpgMakerVx,
    RpgMakerVxAce,
    RpgMakerMv,
    RpgMakerMz,
    RenPy,
    Tyrano,
    Unknown,
}
```

No generic `RpgMaker` value is allowed after detection completes.

### 3.2 RuntimeFamily

```rust
enum RuntimeFamily {
    Rgss,
    WebRpgMaker,
    RenPy,
    TyranoWeb,
}
```

`TyranoWeb` may internally reuse WebRuntime but remains a separate adapter so engine-specific compatibility and licensing remain isolated.

### 3.3 DetectionConfidence

```text
EXACT
HIGH
MEDIUM
LOW
NONE
AMBIGUOUS
```

Only EXACT/HIGH may auto-build by default.

MEDIUM requires `--accept-detection`.
LOW/NONE fail.
AMBIGUOUS always fails until explicitly resolved with `--engine`.

### 3.4 CompatibilityStatus

```text
COMPATIBLE
COMPATIBLE_WITH_SHIMS
DEGRADED
UNSUPPORTED_DEPENDENCY
UNKNOWN
BLOCKED
```

`BLOCKED` covers policy/license gates such as Tyrano redistribution.

## 4. Input model

`GameSource` is an abstraction over a directory tree.

v1 MUST support:

- unpacked directory;
- optionally ZIP as convenience input after secure extraction.

v1 MUST NOT automatically execute self-extracting EXEs to unpack games.

Future archive adapters must use explicit format parsers.

### TECH-INPUT-001

Symlinks escaping the source root MUST be rejected during ingestion.

### TECH-INPUT-002

Paths MUST be normalized into a platform-neutral logical path using `/` separators while preserving original path spelling metadata for case-sensitive diagnostics.

### TECH-INPUT-003

Source hashing SHOULD use SHA-256 for build provenance.

## 5. Detection pipeline

```text
GameSource
 -> enumerate static markers
 -> run all Detector plugins
 -> score evidence
 -> detect conflicts
 -> EngineDescriptor
```

A detector cannot "win" simply because it runs first.

Each evidence item has:

```json
{
  "kind": "file_exists",
  "path": "Data/Scripts.rvdata2",
  "weight": 100,
  "source": "DET-RGSS-003"
}
```

The detailed rules are in `ENGINE_DETECTION.md`.

## 6. Runtime resolution

Runtime resolution receives:

```text
EngineDescriptor
CompatibilityReport
RuntimeRegistry
ProjectPolicy
```

and emits:

```text
RuntimePlan
```

`RuntimePlan` includes:

- runtime package ID;
- exact version;
- supported ABI;
- required Android modules;
- required compatibility shims;
- permissions;
- build-time transformations;
- content access mode;
- license bundle.

### TECH-RUNTIME-001

Resolver MUST NOT choose an unpinned "latest" runtime.

### TECH-RUNTIME-002

If a runtime does not declare a required capability, resolution fails unless an explicit compatibility shim provides it.

## 7. Runtime registry

Runtime metadata is data-driven.

Example:

```toml
id = "rgss-android"
version = "0.1.0"
runtime_api = 1
family = "rgss"

engines = ["rpgmaker-xp", "rpgmaker-vx", "rpgmaker-vxace"]
abis = ["arm64-v8a"]

[android]
min_sdk = 26

[capabilities]
input_keyboard = true
input_gamepad = true
filesystem_vfs = true
win32_shim = true
```

`min_sdk = 26` above is an example project decision, not copied from JoiPlay. JoiPlay's older public Android MKXP build used minSdk 21 and ABI splits including ARMv7, ARM64 and x86_64 [S05]; RuntimePack v1 may intentionally choose a narrower modern baseline.

## 8. Runtime API

The Android host exposes a small engine-neutral lifecycle.

```text
create(context)
mountContent(contentProvider)
configure(runtimeConfig)
attachSurface(surface)
start(entryPoint)

onPause()
onResume()
onLowMemory()
onInput(event)
shutdown()
```

Native backends expose equivalent JNI entry points.

The host MUST own Android lifecycle. The engine backend MUST NOT directly own arbitrary Android Activities.

## 9. RPG Maker RGSS backend

### 9.1 Scope

- XP / RGSS1
- VX / RGSS2
- VX Ace / RGSS3

JoiPlay's public Android MKXP fork is a mobile MKXP port modified for JoiPlay [S04]. Its launcher extends SDLActivity and uses JoiPlay's common/gamepad modules [S04/S05]. mkxp-z is an open-source player for XP/VX/VX Ace and documents that highly demanding RGSS projects may still require porting work [S06].

### TECH-RGSS-001

The backend MUST run game data/scripts through a compatible RGSS runtime. It MUST NOT execute the Windows `Game.exe`.

### TECH-RGSS-002

RGSS profiles MUST be explicit:

```text
XP     -> RGSS1
VX     -> RGSS2
VX Ace -> RGSS3
```

### TECH-RGSS-003

The backend MUST expose filesystem access through RuntimePack VFS where practical. Direct arbitrary host filesystem access is prohibited.

### TECH-RGSS-004

Win32 API calls are handled by a capability-based shim registry. Unknown calls MUST produce a diagnostic, not silently succeed.

### TECH-RGSS-005

Native Windows DLL loading is unsupported in v1 unless an individual DLL has a dedicated Android replacement adapter.

## 10. RPG Maker MV/MZ web backend

Official RPG Maker documentation confirms MV games use HTML5 browser technologies including WebGL/2D Canvas, WebAudio and JavaScript and describes Android packaging as a hybrid-app/WebView approach [S13]. MZ officially supports web deployment and lists Android-capable modern browsers/WebGL in its web requirements [S14].

### TECH-WEB-001

MV/MZ run inside an Android web runtime.

### TECH-WEB-002

Detector/adapter distinguishes MV and MZ. They are not a single generic "HTML game" once detected.

### TECH-WEB-003

Runtime MUST serve packaged resources through a controlled local-origin/content-provider mechanism, not unrestricted `file://` access.

### TECH-WEB-004

Node/NW.js compatibility is allowlist-based. Native `.node` modules are unsupported by default.

### TECH-WEB-005

The scanner MUST detect at minimum:

- `require(...)` patterns;
- `process`;
- NW.js API usage where statically observable;
- `.node` files;
- platform DLL/SO dependencies;
- external HTTP(S) resources;
- custom plugins.

### TECH-WEB-006

Save behavior MUST be redirected to an app-scoped persistence layer. A stable per-game origin MUST be used so local web storage does not collide between games.

## 11. Ren'Py backend

Official Ren'Py documentation states RAPT creates Android packages and that Android builds run as ordinary Android applications [S07]. Official CLI documentation exposes an Android build command and APK/AAB output behavior [S08].

JoiPlay's public release history demonstrates that different Ren'Py generations can require separate runtime plugins when breaking changes occur; therefore RuntimePack treats Ren'Py as a versioned runtime family, not one timeless backend [S18].

### TECH-RENPY-001

Ren'Py backend resolution MUST be version-aware.

### TECH-RENPY-002

If the exact runtime generation cannot be established, status is `RUNTIME_VERSION_UNRESOLVED`.

### TECH-RENPY-003

v1 SHOULD integrate with upstream Ren'Py Android packaging/runtime components rather than recreate Ren'Py.

### TECH-RENPY-004

Compiled/native Python extensions not available for target Android ABI MUST be reported before build where detectable.

### TECH-RENPY-005

The backend must preserve user saves in app-specific persistent storage.

## 12. Tyrano backend

TyranoScript's official public repository describes the engine as HTML/JavaScript and cross-platform for browser, iOS, Android and desktop [S10]. Official source layout includes `index.html`, `tyrano/`, and `data/`, while `index.html` loads the Tyrano JS/KAG stack [S12].

### TECH-TYRANO-001

Tyrano uses a dedicated adapter over the Web runtime.

### TECH-TYRANO-002

v1 detector looks for a combination of:

```text
index.html
tyrano/tyrano.js
tyrano/plugins/kag/
data/
```

No single marker is sufficient for EXACT confidence.

### TECH-TYRANO-003

Standard browser-compatible projects are the v1 technical target. Desktop-only plugins/APIs may be unsupported.

### TECH-TYRANO-004

TyranoScript and TyranoBuilder projects are fully supported via the Web runtime backend. Games are packaged using the user-provided game content into the Android WebView shell.

## 13. Android storage model

Generated apps use:

```text
APK/assets or asset pack
  -> immutable content

Context.filesDir / equivalent app-specific storage
  -> saves/config/migrations

Context.cacheDir
  -> ephemeral caches only
```

Android documentation states app-specific internal storage is inaccessible to other apps under normal sandboxing and needs no storage permission [S15].

### TECH-AND-001

Normal generated apps MUST NOT request `MANAGE_EXTERNAL_STORAGE`.

### TECH-AND-002

Normal generated apps MUST NOT request legacy broad READ/WRITE external storage solely for game execution.

This intentionally differs from older JoiPlay plugin manifests, which used broad storage permissions because JoiPlay acts as a launcher over arbitrary external game folders [S19]. RuntimePack generated games own their embedded content and do not need that launcher architecture.

## 14. Content format

Supported v1 modes:

```text
plain
packed
encrypted-streaming
```

### `plain`

Files remain individually addressable in generated assets.

### `packed`

Files are stored in `game.pack`, indexed and optionally compressed.

### `encrypted-streaming`

Each content chunk is authenticated and encrypted. Runtime decrypts only requested chunks to memory/ephemeral buffers.

Normative format details are in `CONTENT_VFS_SECURITY.md`.

## 15. Security model

Security goals:

- detect accidental corruption;
- prevent trivial asset extraction;
- avoid persistent full-game plaintext extraction;
- isolate saves;
- preserve signing provenance.

Non-goal:

- absolute secrecy against a rooted/instrumented device.

Android Keystore can hold non-exportable keys and can bind keys to secure hardware where available [S16]. It can be used for installation/device-specific wrapping but cannot protect a purely offline static content key that must be available before first run without some recoverable key material.

## 16. Permissions

Permission generation is capability-driven.

Example:

```text
no network game:
  no INTERNET

game requires remote API:
  INTERNET
```

No permission is emitted simply because another backend once used it.

## 17. Signing

Every installable APK must be signed. Android documentation describes APK signing keys as part of package/update identity [S17].

Build modes:

```text
debug
release-local
release-external-keystore
```

RuntimePack MUST NOT generate a long-term release key silently.

## 18. Diagnostics

Stable error code prefixes:

```text
ING-xxx ingestion
DET-xxx detection
CMP-xxx compatibility
RT-xxx  runtime
WEB-xxx web
RGS-xxx RGSS
RNP-xxx Ren'Py
TYR-xxx Tyrano
CNT-xxx content
AND-xxx Android host
BLD-xxx build
SIG-xxx signing
LIC-xxx licensing
```

Errors must include:

- code;
- human message;
- engine/runtime context;
- source path when relevant;
- remediation category.

## 19. Generated artifacts

Every successful build MUST produce:

```text
<Game>.apk
build-manifest.json
compatibility-report.json
checksums.txt
third-party-notices/
```

AAB builds additionally produce `<Game>.aab`.

## 20. Reproducibility

The build manifest records:

```text
runtimepack version
git commit
runtime package versions
Android Gradle Plugin
Gradle
JDK
Android SDK platform
NDK if used
content hash
project config hash
APK hash
```

Bit-for-bit reproducibility is a stretch goal; dependency reproducibility is mandatory.

## 21. Performance requirements

v1 SHOULD:

- avoid unpacking the whole game at startup;
- lazily open packed resources;
- maintain an in-memory content index;
- support asynchronous reads for Web assets;
- avoid copying multi-gigabyte game content more than necessary during build;
- record peak temporary storage estimate before build.

## 22. Extension model

Adding an engine in a later spec requires:

1. detector;
2. engine descriptor;
3. compatibility scanner;
4. runtime backend/adapter;
5. fixture corpus;
6. license record;
7. capability declarations;
8. ADR if host API changes.

The core MUST NOT be rewritten just to add an engine.

## 23. Conformance

An implementation conforms to Technical Specification v1 if all P0 requirements in PRD, all MUST statements in this document, accepted ADRs, and schemas are satisfied.
