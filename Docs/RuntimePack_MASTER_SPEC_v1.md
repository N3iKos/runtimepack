# RuntimePack Specification Pack v1 — Combined

This combined file is generated from the canonical split documents. For agent work, prefer the split files.


---

<!-- BEGIN README.md -->

# RuntimePack Technical Specification v1

> Working name: **RuntimePack**. Replace this name later without changing architecture.
>
> Status: **Specification v1 / implementation blueprint**
>
> Scope: **RPG Maker XP, VX, VX Ace, MV, MZ; Ren'Py; TyranoBuilder/TyranoScript**
>
> Target platform: **Android standalone APK first; AAB as secondary output**
>
> Primary interface: **CLI first; GUI is explicitly deferred**

## Purpose

RuntimePack is a build-time compatibility-runtime packager. It takes an existing supported game, detects its engine, analyzes compatibility, selects an Android runtime backend, packages the game together with that runtime, and emits a standalone Android application.

The end-user experience is intentionally simple:

```text
install Game.apk
    -> tap game icon
    -> game starts directly
```

There is no universal launcher UI in the generated APK. The generated APK contains only the runtime family needed by that game unless an explicit debug/universal build mode is requested.

This repository specification is written for implementation by autonomous/agentic coding systems. It therefore uses evidence IDs, stable requirement IDs, explicit non-goals, schemas, deterministic rules, and "do not infer" constraints.

## Required reading order for an implementation agent

1. `AGENT_GUIDE.md`
2. `PRD.md`
3. `TECHNICAL_SPEC_V1.md`
4. `ARCHITECTURE.md`
5. `ENGINE_SUPPORT_MATRIX.md`
6. `ENGINE_DETECTION.md`
7. `RUNTIME_CONTRACT.md`
8. `BUILD_PIPELINE.md`
9. `ANDROID_RUNTIME.md`
10. `CONTENT_VFS_SECURITY.md`
11. `COMPATIBILITY.md`
12. `TEST_PLAN.md`
13. `LICENSE_COMPLIANCE.md`
14. `IMPLEMENTATION_PLAN.md`
15. `REFERENCES.md`
16. `adr/*`
17. `schemas/*`

## Non-negotiable interpretation rules

- "Supported" means the engine family is in scope and a backend exists. It does **not** mean every game made with that engine will run.
- Do not claim Windows emulation. RuntimePack does not implement a general Windows or x86 emulator in v1.
- RPG Maker 2000/2003 and RPG Maker Unite are **out of scope for v1**.
- Tyrano support is technically designed but is subject to a **license/redistribution gate** before any TyranoScript engine code is redistributed.
- RuntimePack may use open-source upstream runtimes when licensing is compatible, but the project core must not depend on proprietary JoiPlay source.
- JoiPlay is a behavioral/architectural reference and source of public implementation evidence, not a binary dependency.
- Every generated APK must contain a build manifest that records the exact runtime/backend/toolchain versions used.

## Deliverables described by this specification

```text
runtimepack/
├── cli/
├── core/
├── runtimes/
│   ├── rgss/
│   ├── web/
│   ├── renpy/
│   └── tyrano/        # adapter; distribution gated
├── android/
├── compatibility-db/
├── schemas/
├── tests/
└── third_party/
```

The v1 CLI must support at minimum:

```bash
runtimepack doctor
runtimepack inspect <game>
runtimepack explain <game>
runtimepack build <game> --package <id> --name <name>
runtimepack runtime list
runtimepack licenses
```

## Specification authority

If documents disagree, use this precedence:

1. `AGENT_GUIDE.md` safety/grounding rules.
2. `TECHNICAL_SPEC_V1.md` normative requirements.
3. `adr/*` accepted architecture decisions.
4. Engine-specific documents.
5. `PRD.md`.
6. Examples.

External factual claims are indexed in `REFERENCES.md`.


<!-- END README.md -->


---

<!-- BEGIN AGENT_GUIDE.md -->

# Agent Guide

## 1. Objective

This document constrains autonomous implementation agents. Its purpose is to prevent architectural drift, invented engine behavior, unverified compatibility claims, and accidental license violations.

The agent is expected to implement the specification incrementally and to stop at defined gates rather than guessing.

## 2. Evidence classes

Every external fact used in implementation must belong to one of these classes:

| Class | Meaning | Allowed use |
|---|---|---|
| A | Official product/vendor documentation | Normative external fact |
| B | Official/upstream open-source repository | Normative implementation reference |
| C | JoiPlay public repository/documentation | Behavioral/architecture reference |
| D | Community/unofficial material | Diagnostic hint only, never sole basis for a normative decision |

The current evidence registry is `REFERENCES.md`.

## 3. Mandatory anti-hallucination rules

An implementation agent MUST:

1. Preserve exact scope: XP, VX, VX Ace, MV, MZ, Ren'Py, Tyrano.
2. Never add RPG Maker 2000/2003, Wolf RPG, Unity, Godot, Flash, Construct, NScripter, or other engines to v1 unless the specification is revised.
3. Never claim a runtime supports a function merely because a similarly named engine does.
4. Never infer a Ren'Py runtime version from filename alone when version metadata cannot be proven.
5. Never execute untrusted game code during `inspect`, `detect`, or static compatibility scanning.
6. Never silently fall back from one engine family to another.
7. Never silently ignore an unsupported native dependency.
8. Never invent a Windows API shim. A shim requires:
   - a tracked compatibility issue,
   - a test fixture,
   - an implementation,
   - a capability ID,
   - an entry in `compatibility-db`.
9. Never copy code from a third-party repository until `LICENSE_COMPLIANCE.md` marks that dependency as approved.
10. Never redistribute TyranoScript/TyranoBuilder engine files until the `TYRANO-LEGAL` gate is explicitly cleared.
11. Never make build output depend on "latest". All runtime/toolchain versions must be pinned in a lockfile.
12. Never write decrypted game contents to persistent storage when the selected content mode is `encrypted-streaming`, except for documented temporary cases.
13. Never request broad Android storage access merely to simplify implementation.
14. Never mark an unknown result as compatible. Unknown stays `UNKNOWN`.
15. Never use percentages such as "95% compatible" unless a separately defined benchmark corpus and calculation exist.

## 4. Source verification protocol

Before implementing behavior based on external information:

```text
claim
  -> find Sxx reference
  -> verify source class A/B/C
  -> record implementation note
  -> add test
  -> implement
```

If no source exists:

```text
claim
  -> mark OPEN QUESTION
  -> create spike/issue
  -> do not implement guessed behavior
```

## 5. Change protocol

Any change to a normative requirement must:

- preserve the old requirement ID in history;
- add an ADR if architecture changes;
- update impacted schemas;
- update tests;
- update `REFERENCES.md` if the change relies on new external facts.

Do not mutate a requirement in a way that makes existing issue references ambiguous.

## 6. Definition keywords

The words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are normative.

- MUST: required for v1 conformance.
- SHOULD: expected unless a documented reason exists.
- MAY: optional.

## 7. Work unit format

Agentic implementation should operate in bounded work units:

```yaml
task:
  id: RGSS-BOOT-001
  goal: "Boot a minimal VX Ace fixture to first frame"
  inputs:
    - TECH-RGSS-001
    - DET-RGSS-003
  dependencies:
    - AND-HOST-001
  acceptance:
    - "fixture boots on arm64 emulator"
    - "first-frame marker emitted"
    - "no storage permission requested"
  evidence:
    - S04
    - S06
```

Do not assign broad tasks such as "implement RPG Maker support."

## 8. Required implementation artifacts per feature

A completed feature must have:

- code;
- unit/integration tests as appropriate;
- requirement IDs in commit/issue description;
- diagnostic error codes;
- documentation update;
- license/provenance update if dependency changed.

## 9. Unknowns that are intentionally not resolved by v1 spec

These are not permission to guess.

- Exact Android adaptation strategy for current mkxp-z versus a separate Android MKXP lineage.
- Exact Ren'Py game-version extraction for every historical Ren'Py release.
- Legal permission model for bundling TyranoScript in a conversion product.
- Compatibility behavior for arbitrary native Node modules in MV/MZ.
- Compatibility behavior for arbitrary Python binary extensions in Ren'Py.
- DRM resistance against a fully compromised/rooted device.

Each must be handled through a spike or explicit unsupported path.

## 10. Stop conditions

The agent must stop implementation of a specific path and return a structured blocker when:

```text
LICENSE_BLOCKED
SOURCE_UNVERIFIED
AMBIGUOUS_ENGINE
UNSUPPORTED_NATIVE_DEPENDENCY
RUNTIME_VERSION_UNRESOLVED
TOOLCHAIN_MISSING
```

A blocker is preferable to invented behavior.


<!-- END AGENT_GUIDE.md -->


---

<!-- BEGIN PRD.md -->

# Product Requirements Document (PRD) — RuntimePack v1

## 1. Product statement

RuntimePack converts an existing supported desktop/web game distribution into a standalone Android application by bundling the game with a compatible Android runtime.

The product is a **packager and compatibility-runtime platform**, not a source-code transpiler and not a general Windows emulator.

Reference architecture: JoiPlay publicly describes itself as an interpreter/launcher for RPG Maker XP/VX/VX Ace/MV/MZ, Ren'Py, TyranoBuilder and other families, while explicitly stating it does not emulate Windows [S01]. RPGPacker separately demonstrates the build-time model for XP/VX/VX Ace: generate an Android project from a game and build it using Gradle into APK/AAB [S02].

## 2. v1 target users

Primary:

- developers/localization teams that have lawful access to a game distribution and want a standalone Android package;
- technical users automating repeatable Android builds;
- maintainers building controlled game ports with engine-specific compatibility patches.

Secondary:

- future GUI users; GUI is not a v1 deliverable.

## 3. v1 engine scope

### In scope

- RPG Maker XP
- RPG Maker VX
- RPG Maker VX Ace
- RPG Maker MV
- RPG Maker MZ
- Ren'Py
- TyranoBuilder/TyranoScript, **technical integration only until legal gate is cleared**

JoiPlay currently lists all of these engine families among its supported types [S01].

### Out of scope

- RPG Maker 2000/2003
- RPG Maker Unite
- Construct
- Flash/Ruffle
- Godot
- NScripter/ONScripter
- Unity
- Unreal
- arbitrary Windows executables
- full Wine/Windows/x86 emulation

## 4. Product goals

### P0 — Standalone application illusion

The output must behave like a dedicated Android game:

```text
tap icon -> bootstrap -> game
```

No launcher selection screen is shown.

### P0 — Deterministic engine detection

The packager must identify an engine using static evidence. It must not execute the game during detection.

### P0 — Build-time runtime selection

Only the needed runtime family is embedded by default.

### P0 — Repeatable build

A build manifest and lockfile must record runtime and toolchain versions.

### P0 — Diagnostics before build

The tool must surface unsupported dependencies before spending time on Android packaging where feasible.

### P0 — Save isolation

Original game content is read-only; save data is written to app-specific storage.

### P1 — Asset protection

Optional encrypted content container with streaming reads.

### P1 — Compatibility overlay

Compatibility patches and shims live outside original game files.

### P1 — APK and AAB

APK is primary. AAB is supported after APK pipeline is stable.

## 5. Non-goals

RuntimePack v1 does not promise:

- 100% compatibility;
- faithful Windows API behavior;
- arbitrary DLL loading;
- arbitrary Node native module loading;
- arbitrary Python compiled extension support;
- perfect extraction prevention;
- automatic legal rights verification for game content;
- automatic decompilation of protected game formats;
- automated patching based on guesses.

## 6. User journeys

### 6.1 Inspect

```bash
runtimepack inspect ./Game
```

Expected:

```text
Engine          RPG Maker VX Ace
Confidence      EXACT
Runtime family  RGSS
Compatibility   COMPATIBLE_WITH_SHIMS

Evidence
  Game.ini
  Data/Scripts.rvdata2
  Library=System\RGSS301.dll

Warnings
  Win32API call candidates: 2
  Native DLL dependencies: 0
```

### 6.2 Explain

```bash
runtimepack explain ./Game
```

Must explain why the engine/backend was chosen.

### 6.3 Build

```bash
runtimepack build ./Game \
  --package com.example.game \
  --name "Example Game" \
  --abi arm64-v8a \
  --content-mode packed \
  --output ./dist
```

Output:

```text
dist/
├── ExampleGame.apk
├── build-manifest.json
├── compatibility-report.json
├── third-party-notices/
└── checksums.txt
```

### 6.4 Protected build

```bash
runtimepack build ./Game --content-mode encrypted-streaming
```

The game should read assets through RuntimePack VFS without extracting the entire game to a persistent plaintext directory.

## 7. Product requirements

| ID | Requirement | Priority |
|---|---|---|
| PRD-001 | CLI must inspect without executing game code | P0 |
| PRD-002 | CLI must detect all in-scope engine families or return UNKNOWN/AMBIGUOUS | P0 |
| PRD-003 | Build must select exactly one primary runtime family | P0 |
| PRD-004 | Generated APK launches directly into the game | P0 |
| PRD-005 | Runtime and toolchain versions must be pinned | P0 |
| PRD-006 | Save data must be stored separately from immutable game content | P0 |
| PRD-007 | Build failures must have stable error codes | P0 |
| PRD-008 | Compatibility scanner must report native dependencies | P0 |
| PRD-009 | Output must include third-party license notices for bundled components | P0 |
| PRD-010 | Broad storage permissions must not be required for normal generated APK operation | P0 |
| PRD-011 | Content packing must support `plain` and `packed` | P0 |
| PRD-012 | `encrypted-streaming` content mode | P1 |
| PRD-013 | APK output | P0 |
| PRD-014 | AAB output | P1 |
| PRD-015 | User-supplied icon/name/orientation metadata | P1 |
| PRD-016 | Physical controller and touch-input abstraction | P1 |
| PRD-017 | Build report records detection evidence | P0 |
| PRD-018 | Tyrano bundling blocked until legal gate is cleared | P0 |
| PRD-019 | GUI is not required for v1 | P0 |
| PRD-020 | Engine-specific patches must not mutate the source game by default | P0 |

## 8. Success metrics

Do not use broad compatibility percentages in v1.

Measure:

- detector accuracy on maintained fixture corpus;
- build success rate on fixture corpus;
- boot-to-first-frame success;
- save/load success;
- pause/resume success;
- deterministic-build metadata equality;
- crash-free smoke tests on defined Android device/API matrix.

## 9. Product risks

1. RGSS compatibility depends on Windows API/native dependencies.
2. MV/MZ desktop plugins may rely on NW.js/Node APIs not present in Android WebView.
3. Ren'Py versions can contain breaking runtime changes; JoiPlay itself distributes multiple versioned Ren'Py plugins [S08/S09 context in references].
4. Tyrano redistribution terms may block the intended bundling model unless permission is obtained [S10].
5. Licenses of runtime dependencies can impose redistribution/source/notice obligations.
6. Asset encryption is hardening, not absolute secrecy.

## 10. v1 release gate

v1 may be declared complete only when:

- XP/VX/VX Ace minimal fixtures boot;
- MV/MZ minimal fixtures boot;
- selected Ren'Py version fixtures boot;
- Tyrano either boots under a legally approved packaging model or is shipped as disabled/experimental with a clear legal gate;
- detector and reports are deterministic;
- APK signing works;
- save isolation works;
- no normal game requires broad storage permissions;
- license report is emitted.


<!-- END PRD.md -->


---

<!-- BEGIN TECHNICAL_SPEC_V1.md -->

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

### TECH-TYRANO-004 — LEGAL GATE

The public TyranoScript repository includes terms discouraging redistribution of TyranoScript itself and specifically references use as a smartphone conversion tool as requiring consultation with the developer [S10].

Therefore:

```text
Tyrano bundling = BLOCKED
until explicit redistribution/legal approval is documented.
```

An agent MUST NOT bypass this gate by copying upstream engine files into RuntimePack.

A permissible future architecture may package user-supplied Tyrano engine files from the input game without redistributing them as part of RuntimePack itself, but that interpretation still requires legal review and is not asserted by this specification.

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


<!-- END TECHNICAL_SPEC_V1.md -->


---

<!-- BEGIN ARCHITECTURE.md -->

# Architecture

## 1. Architectural style

RuntimePack uses:

- ports-and-adapters on host side;
- backend plugin architecture for engines;
- immutable build planning;
- app-side service boundaries for content, input, saves, diagnostics;
- build-time runtime selection.

## 2. Host component graph

```text
CLI
 |
 v
CommandService
 |
 +--> DoctorService
 +--> InspectService
 |      |
 |      +--> Ingestor
 |      +--> DetectorRegistry
 |      +--> AnalyzerRegistry
 |
 +--> BuildService
        |
        +--> Ingestor
        +--> DetectorRegistry
        +--> AnalyzerRegistry
        +--> RuntimeResolver
        +--> BuildPlanner
        +--> ContentPacker
        +--> AndroidProjectGenerator
        +--> GradleDriver
        +--> Signer
        +--> ArtifactReporter
```

## 3. Pure planning boundary

All decisions should be calculated before build side effects.

```text
Input + Config + RuntimeRegistry
       |
       v
    BuildPlan   <--- serializable
       |
       v
 side-effecting execution
```

`BuildPlan` must be serializable so it can be reviewed/debugged.

## 4. Core interfaces

### Detector

```rust
trait Detector {
    fn id(&self) -> &'static str;
    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate;
}
```

### Analyzer

```rust
trait Analyzer {
    fn supports(&self, engine: EngineId) -> bool;
    fn analyze(
        &self,
        source: &GameSourceIndex,
        engine: &EngineDescriptor
    ) -> CompatibilityReport;
}
```

### RuntimeResolver

```rust
trait RuntimeResolver {
    fn resolve(
        &self,
        engine: &EngineDescriptor,
        report: &CompatibilityReport,
        registry: &RuntimeRegistry,
        policy: &BuildPolicy
    ) -> Result<RuntimePlan, ResolveError>;
}
```

### ContentTransform

```rust
trait ContentTransform {
    fn plan(&self, input: &ContentGraph) -> TransformPlan;
    fn execute(&self, plan: &TransformPlan, out: &Path) -> Result<()>;
}
```

## 5. Android component graph

```text
BootstrapActivity
       |
       v
RuntimeApplication
       |
       +--> BuildManifestLoader
       +--> ContentService
       +--> SaveService
       +--> InputService
       +--> DiagnosticsService
       +--> RuntimeHost
                |
                +--> RgssBackend
                +--> RpgMakerWebBackend
                +--> RenPyBackend
                +--> TyranoWebBackend
```

## 6. Dependency direction

Allowed:

```text
CLI -> core
engine adapters -> core interfaces
Android backends -> Android runtime API
packager -> schemas
```

Forbidden:

```text
core -> concrete RPG Maker class
core -> JoiPlay implementation
RGSS backend -> Ren'Py backend
engine detector -> Android Activity
Android Activity -> host CLI
```

## 7. Repository layout

```text
runtimepack/
├── Cargo.toml
├── cli/
├── core/
│   ├── ingest/
│   ├── detection/
│   ├── compatibility/
│   ├── runtime_registry/
│   ├── build_plan/
│   ├── diagnostics/
│   └── provenance/
├── engines/
│   ├── rgss/
│   ├── rpgmaker_web/
│   ├── renpy/
│   └── tyrano/
├── pack/
│   ├── container/
│   ├── crypto/
│   └── compression/
├── android/
│   ├── shell/
│   ├── runtime-api/
│   ├── content/
│   ├── input/
│   └── saves/
├── runtimes/
│   ├── manifests/
│   └── build-recipes/
├── compatibility-db/
├── schemas/
├── tests/
│   ├── fixtures/
│   ├── integration/
│   └── device/
└── third_party/
```

## 8. Why not a universal generated APK?

A JoiPlay-style launcher has to dynamically host multiple runtime families. RuntimePack resolves the runtime at build time.

Benefits:

- smaller output;
- fewer dependencies and licenses per game;
- smaller attack surface;
- fewer Android permissions;
- simpler startup path;
- clearer compatibility report.

A debug-only universal APK may be introduced for internal testing but is not a product output requirement.

## 9. Compatibility overlay architecture

```text
logical read
  |
  +--> PatchLayer
  |      |
  |      +-- game-specific replacement
  |
  +--> BaseContentLayer
  |
  +--> GeneratedCompatibilityLayer
```

The original source remains untouched.

## 10. Version boundaries

There are four independent versions:

```text
RuntimePack CLI version
Runtime API version
Content container version
Backend/runtime package version
```

They must not be collapsed into one version number.


<!-- END ARCHITECTURE.md -->


---

<!-- BEGIN ENGINE_SUPPORT_MATRIX.md -->

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


<!-- END ENGINE_SUPPORT_MATRIX.md -->


---

<!-- BEGIN ENGINE_DETECTION.md -->

# Engine Detection Specification

## 1. Principle

Detection is static, deterministic, non-executing, and evidence-based.

The detector scans filenames, selected small text/config files, and safe metadata. It does not run executables or game scripts.

## 2. Candidate scoring

Each detector produces:

```json
{
  "engine": "rpgmaker-vxace",
  "confidence": "EXACT",
  "score": 220,
  "evidence": [],
  "conflicts": []
}
```

Suggested thresholds:

```text
>= 200 EXACT
>= 140 HIGH
>= 80  MEDIUM
>= 30  LOW
< 30   NONE
```

Weights are internal policy and may evolve, but evidence rules must stay explicit.

## 3. RPG Maker XP

JoiPlay's public RGA template shows XP with `Game.ini`, library `RGSS104E.dll`, and `Scripts=Data\Scripts.rxdata` [S20].

Evidence:

```text
+120 Data/Scripts.rxdata exists
+80  Game.ini exists and Scripts ends in .rxdata
+30  Game.ini Library contains RGSS1
+10  Game.exe exists
```

Required for automatic HIGH or EXACT:

- `Game.ini`; and
- `.rxdata` script evidence.

Do not classify merely because `Game.exe` exists.

## 4. RPG Maker VX

JoiPlay's template uses `Scripts=Data\Scripts.rvdata` and an RGSS2 library [S21].

```text
+120 Data/Scripts.rvdata
+80  Game.ini Scripts=.rvdata
+30  Game.ini Library contains RGSS2
+10  Game.exe
```

## 5. RPG Maker VX Ace

JoiPlay's template uses `Scripts=Data\Scripts.rvdata2` and `RGSS301.dll` [S22].

```text
+120 Data/Scripts.rvdata2
+80  Game.ini Scripts=.rvdata2
+30  Game.ini Library contains RGSS3
+10  Game.exe
```

## 6. RPG Maker MV

High-confidence markers:

```text
index.html
js/rpg_core.js
js/rpg_managers.js
data/System.json
```

Accept either root layout or a deployed nested `www/` layout, but normalize logical root before final classification.

Official MV documentation confirms its HTML5/JavaScript web execution model [S13].

Conflict rule:

- if `rmmz_core.js` exists alongside `rpg_core.js`, return AMBIGUOUS unless a known deployment migration rule proves one is stale.

## 7. RPG Maker MZ

High-confidence markers:

```text
index.html
js/rmmz_core.js
js/rmmz_managers.js
data/System.json
```

Official MZ documentation confirms web browser deployment [S14].

## 8. Ren'Py

Possible markers:

```text
game/
*.rpy
*.rpyc
*.rpa
renpy/
```

Detection must distinguish:

- source distribution;
- desktop packaged distribution;
- already-Android package (not accepted as source in v1).

Version extraction is a separate phase and may fail even if engine detection succeeds.

Detection result may therefore be:

```text
engine = RenPy
confidence = HIGH
version = UNKNOWN
```

Build then fails with `RNP-110 RUNTIME_VERSION_UNRESOLVED` unless policy resolves it safely.

## 9. Tyrano

Official TyranoScript source layout includes `index.html`, `tyrano/`, `data/`, and KAG engine files [S10/S12]. JoiPlay's RGA template uses `index.html` as Tyrano entrypoint [S23].

Evidence:

```text
+80 index.html
+80 tyrano/tyrano.js
+50 tyrano/plugins/kag/
+40 data/
+30 data/scenario/
```

`index.html` alone is generic HTML and never enough.

## 10. Ambiguous HTML games

If a source contains `index.html` but lacks an MV/MZ/Tyrano signature:

```text
engine = UNKNOWN
reason = DET-150_GENERIC_HTML_OUT_OF_SCOPE
```

Generic HTML is not in v1 scope even though JoiPlay supports it.

## 11. Security limits

Detector must limit:

- maximum bytes read from text files;
- recursive depth;
- file count before warning;
- path length;
- archive expansion ratio if archive input is enabled.

No parser may trust file extensions alone.

## 12. Detection report

Every detection output records:

```json
{
  "engine": "rpgmaker-vxace",
  "confidence": "EXACT",
  "evidence": [
    {"rule":"DET-RGSS-VA-001","path":"Game.ini"},
    {"rule":"DET-RGSS-VA-002","path":"Data/Scripts.rvdata2"}
  ],
  "negative_evidence": [],
  "conflicts": []
}
```

The `explain` command prints this report in human-readable form.


<!-- END ENGINE_DETECTION.md -->


---

<!-- BEGIN RUNTIME_CONTRACT.md -->

# Runtime Contract v1

## 1. Goal

A runtime backend should be replaceable without changing the CLI core.

There are two contracts:

1. Host-side `RuntimePackage` contract.
2. Android-side `RuntimeHost` lifecycle contract.

## 2. Runtime package manifest

Required fields:

```toml
manifest_version = 1
id = "rgss-android"
version = "0.1.0"
runtime_api = 1
family = "rgss"

engines = [
  "rpgmaker-xp",
  "rpgmaker-vx",
  "rpgmaker-vxace"
]

abis = ["arm64-v8a"]

[android]
min_sdk = 26

[artifacts]
android_module = "runtime-rgss"

[licenses]
notice_bundle = "licenses/"
```

## 3. Capabilities

Capabilities are stable strings.

Initial registry:

```text
fs.vfs.read
fs.vfs.seek
fs.save.write

input.keyboard
input.pointer
input.gamepad
input.virtual_gamepad

graphics.surface
audio.standard
video.standard

rgss.win32.user32.keyboard
rgss.win32.kernel32.temp

web.local_origin
web.javascript
web.webgl
web.webaudio
web.node.path
web.node.fs_subset
web.nwjs_subset

renpy.runtime.versioned
renpy.python.pure

network.http
```

A game requirement must match a runtime or shim capability.

## 4. Build-time hooks

```rust
trait RuntimePackage {
    fn manifest(&self) -> &RuntimeManifest;

    fn validate_plan(
        &self,
        game: &GameDescriptor,
        compatibility: &CompatibilityReport
    ) -> Result<()>;

    fn prepare_content(
        &self,
        ctx: &mut ContentPrepareContext
    ) -> Result<()>;

    fn contribute_android(
        &self,
        project: &mut AndroidProject
    ) -> Result<()>;

    fn emit_runtime_config(
        &self,
        plan: &RuntimePlan
    ) -> Result<serde_json::Value>;
}
```

## 5. Android lifecycle contract

Backend state machine:

```text
NEW
 -> CREATED
 -> CONTENT_MOUNTED
 -> CONFIGURED
 -> SURFACE_ATTACHED
 -> RUNNING
 -> PAUSED
 -> RUNNING
 -> STOPPING
 -> STOPPED
```

Invalid transitions must fail with `RT-120_INVALID_STATE`.

## 6. Android host methods

Pseudo-interface:

```kotlin
interface RuntimeBackend {
    fun create(context: RuntimeContext)
    fun mountContent(provider: GameContentProvider)
    fun configure(config: RuntimeConfig)
    fun attachSurface(surface: RuntimeSurface)
    fun start(entryPoint: String)
    fun onInput(event: RuntimeInputEvent)
    fun pause()
    fun resume()
    fun stop()
}
```

## 7. RuntimeContext

Backend receives only scoped services:

```text
ContentService
SaveService
InputService
LogService
ClockService
NetworkService?  # only if permitted
```

Do not pass unrestricted application/global filesystem helpers.

## 8. Backend-specific adapters

### RGSS

Maps RuntimePack services into MKXP/RGSS runtime interfaces.

### RPG Maker Web

Maps content to a stable local origin and maps input/save services into web-compatible behavior.

### Ren'Py

Integrates upstream runtime expectations and maps Android lifecycle/save paths.

### Tyrano

Uses Web backend primitives but has a separate adapter for Tyrano-specific bootstrap and compatibility analysis.

## 9. ABI policy

v1 required ABI:

```text
arm64-v8a
```

Optional test ABI:

```text
x86_64
```

ARMv7 is deferred unless project requirements explicitly require it.

This is a RuntimePack v1 product decision; it is not a statement about JoiPlay. JoiPlay's older public Android MKXP build contains ARMv7, ARM64, and x86_64 splits [S05].

## 10. Backward compatibility

Runtime API v1 is frozen once the first backend is released.

Breaking host-backend changes require `runtime_api = 2`.


<!-- END RUNTIME_CONTRACT.md -->


---

<!-- BEGIN BUILD_PIPELINE.md -->

# Build Pipeline

## 1. Pipeline overview

```text
Source
  |
  v
1 Ingest
  |
2 Index
  |
3 Detect
  |
4 Analyze
  |
5 Resolve Runtime
  |
6 Plan Build
  |
7 Transform/Patch
  |
8 Pack Content
  |
9 Generate Android Project
  |
10 Compile
  |
11 Sign
  |
12 Validate
  |
13 Report
```

## 2. Stage outputs

### Stage 1 — Ingest

Output:

```text
SourceDescriptor
```

Checks:

- path exists;
- supported input type;
- no source mutation;
- symlink containment.

### Stage 2 — Index

Output:

```text
GameSourceIndex
```

Contains path metadata, size, selected hashes, and safe small-file previews.

### Stage 3 — Detect

Output:

```text
EngineDescriptor
```

Failure:

```text
DET-100 UNKNOWN
DET-110 AMBIGUOUS
```

### Stage 4 — Analyze

Output:

```text
CompatibilityReport
```

Static scanners run here.

### Stage 5 — Resolve runtime

Output:

```text
RuntimePlan
```

All runtime dependencies must already be installed/pinned or build stops.

### Stage 6 — Build plan

Output:

```text
build-plan.json
```

This is the final pre-side-effect decision record.

### Stage 7 — Transform

Uses temporary working directory only.

Examples:

- generated compatibility JS shim;
- generated RGSS runtime config;
- normalized metadata;
- overlay patches.

### Stage 8 — Content pack

Output:

```text
game.pack
```

or an asset tree for `plain`.

### Stage 9 — Android project generation

Template + runtime module + generated metadata.

No manual editing is expected.

### Stage 10 — Compile

Gradle is invoked with pinned wrapper/tool versions.

### Stage 11 — Sign

Modes:

```text
debug
keystore:<path>
```

Release build without an explicit key is rejected by default.

### Stage 12 — Validate

Minimum:

- APK exists;
- APK signature verifies;
- required ABI library exists;
- build manifest exists;
- no forbidden permission;
- content hash matches.

Optional device validation:

- install;
- launch;
- wait for runtime health marker.

### Stage 13 — Report

Generate:

```text
build-manifest.json
compatibility-report.json
checksums.txt
third-party-notices/
```

## 3. Build directories

```text
.runtimepack/
├── cache/
├── runtimes/
└── toolchains/

project-work/
├── ingest/
├── transformed/
├── content/
└── android/

dist/
```

Build work must not be placed inside the source game directory.

## 4. Incremental caching

Cache keys include:

```text
source content hash
project config hash
runtime version
container version
transform version
```

Never cache based on path name alone.

## 5. Atomic output

Write output to temporary path and atomically promote when validation succeeds.

Failed builds must not replace a previously good artifact.

## 6. Build cancellation

Long stages must be cancelable at safe boundaries.

Content packer and hashing loops should periodically check cancellation tokens.

## 7. Example CLI flow

```bash
runtimepack build ./Game --config runtimepack.toml
```

Equivalent internal sequence:

```text
inspect
resolve
plan
execute
validate
report
```


<!-- END BUILD_PIPELINE.md -->


---

<!-- BEGIN ANDROID_RUNTIME.md -->

# Android Runtime Host

## 1. Application model

Generated apps are dedicated games, not launchers.

```text
Launcher intent
 -> BootstrapActivity
 -> RuntimeApplication
 -> RuntimeHost
 -> first frame
```

## 2. Bootstrap responsibilities

`BootstrapActivity` MUST:

- load `build-manifest.json`;
- initialize logging;
- initialize content service;
- initialize save service;
- instantiate selected backend;
- attach rendering surface;
- forward lifecycle.

It MUST NOT:

- detect game engine;
- scan source files;
- choose between multiple runtime families;
- show a game library UI.

Those decisions happened at build time.

## 3. Storage

Use app-specific internal storage for:

```text
saves/
runtime-state/
migration/
logs/   # bounded
```

Android states these locations require no storage permission and are isolated from other apps [S15].

Large immutable game content lives in APK assets or a supported asset delivery mechanism.

## 4. Permissions

Default generated manifest:

```text
no storage permission
no all-files permission
no network permission unless required
```

Possible capability-driven permissions:

```text
INTERNET
VIBRATE
POST_NOTIFICATIONS   # only if a future game capability truly needs it
```

## 5. Lifecycle

Backend must handle:

```text
onCreate
onStart
onResume
onPause
onStop
onDestroy
```

Save flush should occur on pause/stop where engine semantics permit.

## 6. Rendering abstraction

```text
RuntimeSurface
├── NativeSurface
└── WebSurface
```

RGSS uses native surface/SDL-like rendering.

MV/MZ/Tyrano use WebView/Web runtime surface.

Ren'Py uses the rendering surface required by the selected upstream Android runtime.

## 7. Web local origin

Do not load the game via broad unrestricted filesystem URLs.

Use a stable local application origin backed by a request handler/content provider. The exact implementation may use AndroidX WebKit facilities or an equivalent controlled mechanism.

Goals:

- same-origin stability;
- VFS integration;
- encrypted content streaming;
- interceptable diagnostics;
- path traversal prevention.

## 8. Web security defaults

Unless a compatibility profile requires otherwise:

- disable arbitrary file access;
- disallow mixed-content relaxation;
- expose the minimum JS bridge;
- reject navigation outside permitted origins unless intentionally opened externally;
- restrict debug WebView features in release builds.

## 9. Save service

Logical interface:

```kotlin
interface SaveService {
    fun openRead(slot: String): InputStream?
    fun openWrite(slot: String): OutputStream
    fun exists(slot: String): Boolean
    fun list(): List<String>
    fun delete(slot: String)
}
```

Web engines may internally use web storage. The adapter must ensure per-game isolation and backup/migration behavior.

## 10. Input

Common event format:

```json
{
  "type": "key_down",
  "code": "ENTER",
  "timestamp_ns": 0
}
```

Sources:

- touch virtual buttons;
- physical keyboard;
- gamepad;
- pointer/touch.

JoiPlay publicly maintains a separate virtual gamepad component for plugins [S24], supporting the design choice that input should be a reusable subsystem rather than engine code.

## 11. Health markers

Debug/test builds expose structured milestones:

```text
HOST_CREATED
CONTENT_MOUNTED
RUNTIME_CREATED
GAME_STARTED
FIRST_FRAME
SAVE_READY
```

Device CI uses these instead of screenshot guessing.


<!-- END ANDROID_RUNTIME.md -->


---

<!-- BEGIN CONTENT_VFS_SECURITY.md -->

# Content, VFS, and Asset Protection

## 1. Design goals

- one logical filesystem API across engines where practical;
- immutable base content;
- writable save layer;
- patch overlay;
- optional packing/compression/encryption;
- deterministic content hashes;
- no full plaintext extraction in protected mode.

## 2. Logical mounts

```text
game:/    immutable base game
patch:/   compatibility/user overlay
save:/    persistent writable data
cache:/   ephemeral runtime cache
```

Runtime adapters translate engine paths into these mounts.

## 3. Read resolution

```text
read logical game path
  -> patch override?
      yes -> patch content
      no  -> base content
```

A patch cannot silently delete a base file unless the overlay format explicitly declares a tombstone.

## 4. GPACK/1 format

Working name: `GPACK/1`.

### Header

```text
magic
format_version
flags
manifest_offset
manifest_length
index_offset
index_length
chunk_table_offset
chunk_count
```

### File index entry

```text
logical_path
size
content_hash
chunk_start
chunk_count
compression
encryption
```

### Chunk entry

```text
offset
stored_length
plain_length
nonce/id
authentication_tag
```

Binary layout is intentionally not finalized in this prose document. Implement it from a dedicated schema/format ADR before production.

## 5. Path rules

Canonical logical paths:

- UTF-8;
- `/` separator;
- no `..`;
- no absolute root;
- no NUL;
- preserved original spelling metadata when needed.

Lookup policy for RGSS case-insensitivity must be backend-controlled; do not globally lowercase all paths because web engines can be case-sensitive.

## 6. Content modes

### plain

Best for debugging.

### packed

Single indexed container, optional compression, no secrecy.

### encrypted-streaming

Per-chunk authenticated encryption.

Recommended primitive family:

```text
AEAD
```

Exact algorithm is chosen in a security ADR. Do not hand-roll cryptography.

## 7. Encryption key hierarchy

Static/offline mode:

```text
content key
  -> wrapped/obfuscated for application
```

This is deterrence only.

Device-bound mode, future:

```text
server/provisioned content key
  -> wrapped by Android Keystore key
```

Android Keystore supports non-exportable keys and may use secure hardware [S16].

## 8. Threat model

Protect against:

- casual APK unzip;
- trivial asset copying;
- accidental corruption;
- naive static resource browsing.

Not guaranteed against:

- rooted device;
- debugger/instrumentation;
- runtime memory extraction;
- patched native code;
- full control of process.

Documentation must call protection "hardening", not unbreakable DRM.

## 9. Integrity

All packed files/chunks must have integrity verification.

The build manifest records the final content container SHA-256.

## 10. Save content

Saves are never stored inside encrypted immutable `game.pack`.

Save encryption, if added, is a separate feature with separate migration semantics.

## 11. Web asset serving

A Web runtime request:

```text
https://game.local/img/a.png
 -> LocalOriginHandler
 -> VFS.open("img/a.png")
 -> decrypt/decompress as needed
 -> stream response
```

No full content unpack is required.

## 12. RGSS integration

Preferred end-state:

```text
RGSS runtime file abstraction
 -> RuntimePack VFS
```

If the selected upstream runtime cannot be cleanly redirected, an adaptation spike must determine the least invasive mount mechanism.

A temporary full extraction strategy is acceptable only in early prototypes and must be explicitly labeled non-production for encrypted mode.


<!-- END CONTENT_VFS_SECURITY.md -->


---

<!-- BEGIN COMPATIBILITY.md -->

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


<!-- END COMPATIBILITY.md -->


---

<!-- BEGIN TEST_PLAN.md -->

# Test Plan v1

## 1. Test philosophy

The unit of quality is not "APK compiled." It is:

```text
correct detection
+ correct runtime resolution
+ boot
+ first frame
+ input
+ audio
+ save/load
+ lifecycle
+ reproducible metadata
```

## 2. Fixture policy

Use:

- project-owned minimal fixtures;
- upstream sample projects whose licenses explicitly allow use;
- synthetic files for detection.

Do not commit commercial games or copyrighted third-party game assets as test fixtures.

## 3. Detector fixtures

Required:

```text
xp_min/
vx_min/
vxace_min/
mv_min/
mz_min/
renpy_supported_version_A/
renpy_supported_version_B/
tyrano_min/
generic_html_negative/
ambiguous_mv_mz/
invalid_rgss_game_ini/
```

## 4. Detector tests

Examples:

```text
DET-XP-001:
  input xp_min
  expect RPG_MAKER_XP / EXACT

DET-HTML-NEG-001:
  index.html only
  expect UNKNOWN

DET-AMB-001:
  both rpg_core.js and rmmz_core.js
  expect AMBIGUOUS
```

## 5. Build unit tests

- project generation snapshots;
- manifest generation;
- permission allowlist;
- path normalization;
- runtime lock resolution;
- signing configuration validation.

## 6. Container tests

- round-trip pack/read;
- random seek;
- corrupted index rejection;
- corrupted chunk rejection;
- path traversal rejection;
- wrong key rejection;
- zero-byte files;
- large files;
- Unicode filenames;
- case-collision behavior.

## 7. Android instrumentation tests

For every runtime family:

```text
launch
FIRST_FRAME marker
input event accepted
pause
resume
save
process kill
relaunch
load
```

## 8. Device/API matrix

Minimum CI categories:

- current Android emulator API;
- one older supported API;
- x86_64 emulator if backend supports it;
- physical ARM64 device in release qualification.

Do not hardcode "current" SDK in tests. Toolchain lock defines tested values.

## 9. RGSS compatibility tests

Synthetic scripts for:

- bitmap;
- sprite;
- audio;
- keyboard;
- save/load;
- supported Win32 shim;
- unsupported Win32 call;
- native DLL detection.

## 10. MV/MZ tests

- canvas/WebGL start;
- WebAudio;
- local asset load;
- JS plugin;
- supported compatibility shim;
- unsupported `.node` module detection;
- save persistence;
- stable local origin.

## 11. Ren'Py tests

At least two pinned runtime generations.

Tests:

- basic dialogue;
- image;
- audio;
- menu;
- save/load;
- rollback if supported by selected runtime;
- unsupported native extension detection.

## 12. Tyrano tests

Before legal gate clearance, testing may use developer-owned/synthetic structures without redistributing upstream engine files in release artifacts.

Tests:

- detection;
- adapter bootstrap using permitted fixture strategy;
- scenario navigation;
- assets;
- save if supported by approved model.

## 13. Performance tests

Record:

- cold startup to runtime start;
- runtime start to first frame;
- peak memory;
- pack read throughput;
- random VFS lookup latency;
- build time;
- temp disk amplification.

No fixed performance SLA is imposed before baseline measurement.

## 14. Regression policy

Every fixed compatibility bug gets a regression fixture if legally possible.

Issue closure requires:

```text
bug
 -> fixture
 -> failing test
 -> fix
 -> passing test
```

## 15. Release acceptance

No P0 release if:

- detector has unresolved false positive on fixture corpus;
- release APK asks for forbidden broad storage permission;
- save is lost across restart;
- unsigned APK emitted as release success;
- license notices missing;
- Tyrano legal gate bypassed.


<!-- END TEST_PLAN.md -->


---

<!-- BEGIN LICENSE_COMPLIANCE.md -->

# License and Redistribution Compliance

> This is engineering compliance guidance, not legal advice. Ambiguous redistribution questions are release blockers pending qualified review or permission.

## 1. Principle

RuntimePack has three separate intellectual-property layers:

```text
A. RuntimePack original code
B. Third-party runtime/dependencies
C. User/game content
```

They must not be conflated.

## 2. JoiPlay

JoiPlay is used as:

- public behavioral reference;
- public architecture reference;
- public repository evidence.

Do not depend on proprietary/non-public JoiPlay core code.

JoiPlay's public `plugin` repository is MIT-licensed [S25], but that does not automatically establish the license of every JoiPlay repository or every submodule. Each dependency must be verified separately.

## 3. RGSS runtime

`mkxp-z` publicly states GPL-2.0+ and notes that a default HTTPS/OpenSSL build combination can create GPLv3 implications [S06].

Engineering consequence:

- do not copy/link/distribute mkxp-z until the intended distribution model has a documented compliance plan;
- record exact build options and dependency licenses;
- generated notices/source-offer requirements must be reviewed for the chosen linkage and distribution architecture.

The JoiPlay `android-mkxp` repository proves Android technical feasibility [S04/S05], but the implementation agent must not assume its entire combined tree has one simple license merely because it is public. Audit repository/submodule licenses before code reuse.

## 4. Ren'Py

Official Ren'Py documentation says most of Ren'Py uses MIT, while portions/dependencies use LGPL and other licenses, and distribution must satisfy those licenses [S09].

Therefore the Ren'Py runtime package must ship an automatically generated third-party notice bundle based on the exact pinned runtime version.

## 5. TyranoScript — hard gate

The official public TyranoScript repository states that it is free/commercially usable for making works, but also lists prohibitions including redistributing TyranoScript itself and treating TyranoScript as a smartphone conversion-tool product, with instruction to consult the developer for such cases [S10].

RuntimePack is explicitly a conversion/packaging tool.

Therefore:

```text
TYRANO-LEGAL = BLOCKED
```

until one of the following is documented:

1. explicit permission/license from the Tyrano rights holder;
2. a reviewed architecture that does not redistribute restricted engine material and is legally acceptable;
3. an updated official license/term clearly permitting the intended model.

Implementation agents may build detector/analyzer abstractions, but MUST NOT ship Tyrano engine binaries/source in RuntimePack distributions while the gate is blocked.

## 6. RPG Maker game runtime/assets

RuntimePack does not grant rights to RPG Maker engine/game assets. A user must supply the input game.

Avoid bundling proprietary RPG Maker editor/runtime assets into RuntimePack unless distribution rights are verified.

The packager should prefer using files already present in the user's game distribution plus independently licensable compatibility runtimes.

## 7. License manifest

Every runtime package has:

```json
{
  "component": "example",
  "version": "1.2.3",
  "source": "https://...",
  "license": "MIT",
  "notice_files": [],
  "source_obligation": "reviewed"
}
```

Build fails with `LIC-120_MISSING_MANIFEST` for a bundled third-party component without license metadata.

## 8. Generated notices

Output:

```text
third-party-notices/
├── NOTICE.txt
├── components.json
└── licenses/
```

Only components actually bundled in the output APK/AAB belong in its notices.

## 9. Provenance

Each third-party source dependency must record:

- repository URL;
- commit/tag;
- cryptographic source archive hash;
- local patches;
- build options;
- license files;
- approval status.

## 10. Forbidden agent behaviors

An agent must not:

- scrape closed binaries and copy implementation;
- decompile proprietary engine/runtime code as an implementation shortcut;
- remove notices to reduce APK size;
- label a repository "MIT" based only on another repo by the same author;
- bypass Tyrano terms because the project is technically feasible.


<!-- END LICENSE_COMPLIANCE.md -->


---

<!-- BEGIN IMPLEMENTATION_PLAN.md -->

# Implementation Plan

## 1. Strategy

Implement vertical slices, not all engine detectors followed by all runtime code.

Each milestone ends in a runnable artifact or a deterministic report.

## Milestone 0 — Repository and evidence baseline

Deliver:

- repository skeleton;
- schemas;
- CLI command parser;
- structured diagnostics;
- source/evidence registry;
- runtime registry loader;
- toolchain lock format.

Acceptance:

```text
runtimepack --version
runtimepack doctor
runtimepack runtime list
```

No engine execution yet.

## Milestone 1 — Static detection and inspect

Implement all v1 detectors first.

Acceptance:

```text
runtimepack inspect fixtures/*
runtimepack explain fixtures/*
```

No Android build.

## Milestone 2 — Android shell

Implement:

- bootstrap;
- build manifest loading;
- content service;
- save service;
- logging;
- input abstraction;
- minimal empty/test backend.

Acceptance:

A generated test APK reaches `FIRST_FRAME`/equivalent test marker without broad storage permissions.

## Milestone 3 — Content pack/VFS

Implement `plain` and `packed`.

Defer encryption until read semantics are stable.

Acceptance:

- Android test backend reads random files from `game.pack`;
- save is writable and isolated;
- path traversal tests pass.

## Milestone 4 — RGSS vertical slice

Order:

1. VX Ace minimal fixture;
2. VX;
3. XP;
4. gamepad;
5. save;
6. compatibility scanner;
7. selected Win32 shims.

Why VX Ace first: easy discriminator and common RGSS3 test target; not because it is inherently more supported.

Critical spike:

```text
SPIKE-RGSS-ANDROID-001
Compare:
A. adapt a legally approved Android MKXP lineage
B. port current mkxp-z components to Android
```

Do not lock architecture before spike results.

## Milestone 5 — MV web backend

Implement:

- controlled local origin;
- resource loading from VFS;
- WebGL/WebAudio;
- save persistence;
- JS plugin inventory;
- NW/Node detection.

Then add MZ profile.

## Milestone 6 — Ren'Py

First pick a narrow pinned pair of Ren'Py versions.

Recommended implementation sequence:

```text
one 7.x generation
one 8.x generation
```

The exact versions must be selected from current upstream documentation and runtime feasibility at implementation time, then pinned.

Do not promise arbitrary historical Ren'Py support in initial v1.

## Milestone 7 — Encryption

Add `encrypted-streaming`.

Acceptance:

- no full plaintext persistent extraction;
- corrupted chunk fails closed;
- wrong key fails;
- performance benchmark recorded.

## Milestone 8 — Tyrano technical adapter

Implement detection and compatibility analysis.

Runtime packaging remains disabled while `TYRANO-LEGAL` is blocked.

If gate clears, integrate browser-compatible Tyrano fixture and enable packaging.

## Milestone 9 — Release hardening

- AAB;
- release signing flow;
- third-party notices;
- provenance lock;
- CI device matrix;
- bounded logs;
- crash diagnostics;
- reproducibility report.

## 2. Agent work packages

Suggested issue hierarchy:

```text
CORE-*
DET-*
AND-*
CNT-*
RGSS-*
WEB-*
RNP-*
TYR-*
LIC-*
TEST-*
```

Each issue should reference exact requirement IDs.

## 3. First 20 implementable tickets

1. CORE-001 create workspace and error taxonomy.
2. CORE-002 load/validate `runtimepack.toml`.
3. CORE-003 build `GameSourceIndex`.
4. DET-001 XP detector.
5. DET-002 VX detector.
6. DET-003 VX Ace detector.
7. DET-004 MV detector.
8. DET-005 MZ detector.
9. DET-006 Ren'Py detector.
10. DET-007 Tyrano detector.
11. DET-008 conflict resolver.
12. CORE-004 compatibility report model.
13. CORE-005 runtime registry parser.
14. AND-001 Android shell template.
15. AND-002 generated manifest + permission allowlist.
16. AND-003 save service.
17. CNT-001 logical path normalization.
18. CNT-002 GPACK prototype.
19. TEST-001 detector fixture corpus.
20. SPIKE-RGSS-ANDROID-001 runtime strategy comparison.

## 4. What not to parallelize initially

Do not independently implement multiple competing versions of:

- content container;
- runtime contract;
- Android lifecycle API;
- detection confidence algorithm.

These are shared foundations and need one accepted design.

Engine-specific scanners can be parallelized after those foundations stabilize.


<!-- END IMPLEMENTATION_PLAN.md -->


---

<!-- BEGIN CONFIG_AND_SCHEMAS.md -->

# Configuration and Schemas

## 1. Project configuration

`runtimepack.toml`:

```toml
schema = 1

[app]
name = "Example Game"
package = "com.example.game"
version_name = "1.0.0"
version_code = 1

[engine]
mode = "auto"

[android]
abis = ["arm64-v8a"]
orientation = "auto"
format = "apk"

[content]
mode = "packed"
compression = "auto"

[compatibility]
policy = "strict"

[signing]
mode = "debug"
```

## 2. Engine override

Allowed only when user intentionally resolves detection:

```toml
[engine]
mode = "forced"
id = "rpgmaker-vxace"
```

Build report must record that detection was overridden.

## 3. Content modes

```text
plain
packed
encrypted-streaming
```

## 4. Compatibility policies

### strict

UNKNOWN or unsupported dependency fails build.

### warn

UNKNOWN may build after explicit CLI opt-in. Hard unsupported native dependency still fails unless separately overridden by a development-only flag.

## 5. Build manifest

See `schemas/build-manifest.schema.json`.

## 6. Runtime manifest

See `schemas/runtime-manifest.schema.json`.

## 7. Schema evolution

Rules:

- required semantic changes increment schema integer;
- parsers reject future unknown major schema versions;
- new optional fields may be added without breaking old readers.


<!-- END CONFIG_AND_SCHEMAS.md -->


---

<!-- BEGIN OPEN_QUESTIONS_AND_SPIKES.md -->

# Open Questions and Required Spikes

## SPIKE-RGSS-ANDROID-001 — RGSS Android runtime base

Question:

Should v1:
A. adapt an Android-MKXP lineage,
B. port/currentize mkxp-z for Android,
C. maintain a narrow custom fork combining pieces?

Required outputs:

- build feasibility;
- ARM64 startup;
- VFS integration points;
- Ruby version mapping;
- SDL/rendering dependencies;
- license matrix;
- maintenance cost;
- Win32 shim extensibility.

Do not decide solely from JoiPlay's old public code.

## SPIKE-RENPY-001 — Supported version set

Question:

Which exact Ren'Py runtime generations should v1 promise?

Need:

- current upstream Android build feasibility;
- historical game compatibility;
- package sizes;
- license bundle;
- static version-detection reliability.

Output must pin exact versions.

## SPIKE-WEB-001 — Web origin/VFS bridge

Compare:

- AndroidX WebView local asset loader;
- custom WebViewClient interception;
- localhost embedded HTTP server.

Criteria:

- same-origin;
- streaming encrypted content;
- MIME handling;
- range requests/video;
- service worker behavior;
- security;
- startup cost.

## SPIKE-MVNODE-001 — Node/NW compatibility minimum

Build a corpus of common API use:

```text
path
fs
process
nw.Window
require dynamic/static
```

Define the smallest subset worth implementing.

Do not emulate Node wholesale.

## SPIKE-TYRANO-LEGAL-001

Status: BLOCKING FOR DISTRIBUTION.

Obtain:
- explicit permission or clarified license;
- allowed redistribution model;
- whether user-supplied engine files may be repackaged by a conversion tool;
- notice/attribution conditions.

Until resolved, release build refuses Tyrano with `LIC-310_TYRANO_REDISTRIBUTION_BLOCKED`.

## SPIKE-CONTENT-001 — Container binary format

Finalize:
- binary layout;
- index encoding;
- compression;
- AEAD choice;
- random access;
- very large file behavior;
- forward compatibility.

Requires ADR before production implementation.

## SPIKE-SAVE-001 — Web save persistence

Verify MV and MZ save implementations under chosen stable local origin and ensure package updates do not alter origin identity.


<!-- END OPEN_QUESTIONS_AND_SPIKES.md -->


---

<!-- BEGIN GLOSSARY.md -->

# Glossary

**Adapter** — engine-specific layer translating a generic RuntimePack service/runtime contract into the behavior expected by an engine.

**Backend** — executable runtime implementation embedded into generated Android application.

**Compatibility layer** — shims/adapters that reproduce a subset of APIs expected by a game.

**Content container** — packed representation of immutable game files, working name `GPACK`.

**Engine detector** — static classifier identifying the game's engine family.

**GameSource** — normalized read-only view of input game files.

**RGSS** — Ruby Game Scripting System used by RPG Maker XP/VX/VX Ace generations.

**Runtime family** — major execution technology: RGSS, web, Ren'Py, Tyrano web adapter.

**Runtime package** — pinned build artifact plus metadata that can be inserted into an Android build.

**Shim** — targeted implementation of a missing external/platform API.

**VFS** — virtual filesystem mediating access to base content, patches and saves.

**Standalone APK** — output Android application dedicated to one game, with no universal game picker.

**Build-time dispatch** — choosing runtime while producing APK, contrasted with JoiPlay's play-time launcher/plugin selection.

**Legal gate** — implementation/distribution is intentionally blocked until redistribution terms are resolved.


<!-- END GLOSSARY.md -->


---

<!-- BEGIN REFERENCES.md -->

# References and Evidence Registry

Access baseline: **2026-09-18**

Use source IDs in implementation comments/design docs. URLs should be re-verified before major release because external documentation can change.

## Class A — Official product/vendor documentation

### S01 — JoiPlay official product page
URL: https://www.joiplay.cyou/index.html

Supports the claims:
- JoiPlay is a game interpreter and launcher.
- Listed families include RPG Maker XP/VX/VX Ace/MV/MZ, Ren'Py, TyranoBuilder, Construct, Flash, HTML.
- JoiPlay explicitly says it does not emulate Windows/another OS.
- Windows APIs and uncommon Node.js APIs can affect compatibility.

### S02 — JoiPlay RPGPacker
URL: https://joiplay.cyou/rpgpacker/

Supports:
- RPGPacker targets RPG Maker XP/VX/VX Ace -> Android.
- It generates a project from user/game files and builds with Gradle.
- It produces APK/AAB and signing artifacts.

### S07 — Ren'Py Android documentation
URL: https://www.renpy.org/doc/html/android.html

Supports:
- RAPT creates Android packages.
- Ren'Py Android packages run as Android apps.
- Android packaging has upstream-supported workflow.
- Ren'Py Android distributions include license obligations.

### S08 — Ren'Py command-line documentation
URL: https://www.renpy.org/doc/html/cli.html

Supports:
- upstream Android build CLI;
- APK/AAB build modes.

### S09 — Ren'Py license documentation
URL: https://www.renpy.org/doc/html/license.html

Supports:
- most Ren'Py under MIT;
- portions/dependencies under LGPL and other licenses;
- redistribution notice/compliance requirement.

### S11 — TyranoBuilder export tutorial
URL: https://tyranobuilder.com/tutorials/all-done-time-to-export

Supports:
- TyranoBuilder exports browser, Windows, macOS, iOS and Android targets.
- Browser export is a web-project model.

### S13 — RPG Maker MV official Android conversion documentation
URL: https://rpgmakerofficial.com/product/MV_Help/page/01_11_05.html

Supports:
- MV uses HTML5/WebGL/Canvas/WebAudio/JavaScript.
- Android packaging is described as a hybrid application using a browser engine/WebView.

### S14 — RPG Maker MZ official output documentation
URL: https://rpgmakerofficial.com/product/MZ_help-en/01_11_03.html

Supports:
- MZ web-browser deployment.
- HTML5/WebGL-capable Android browser environment is part of documented web target requirements.

### S15 — Android app-specific storage
URL: https://developer.android.com/training/data-storage/app-specific

Supports:
- internal app-specific storage isolation;
- no system storage permission needed for app-private files.

### S16 — Android Keystore
URL: https://developer.android.com/privacy-and-security/keystore

Supports:
- non-exportable key material;
- possible secure-hardware binding.

### S17 — Android App Bundle / signing FAQ
URL: https://developer.android.com/guide/app-bundle/faq

Supports:
- APKs need cryptographic signing;
- signing key is part of application update identity.

## Class B — Upstream/open-source runtime repositories

### S06 — mkxp-z
URL: https://github.com/mkxp-z/mkxp-z

Supports:
- open-source player for RPG Maker XP/VX/VX Ace.
- not universal compatibility; demanding games may need porting.
- GPL-2.0+ licensing statement and build-option license caveat.

### S10 — TyranoScript official repository
URL: https://github.com/ShikemokuMK/tyranoscript

Supports:
- TyranoScript uses HTML/JavaScript.
- repository layout includes `data`, `tyrano`, `index.html`.
- project states cross-platform targets.
- README terms include restrictions around redistribution and smartphone conversion-tool commercialization/use, requiring consultation in those cases.

**This source creates `TYRANO-LEGAL` gate.**

### S12 — TyranoScript index.html
URL: https://github.com/ShikemokuMK/tyranoscript/blob/master/index.html

Supports:
- specific runtime files loaded from `tyrano/`;
- KAG plugin stack;
- JS/browser architecture and optional desktop `require` behavior.

## Class C — JoiPlay public implementation evidence

### S03 — JoiPlay RGA templates
URL: https://github.com/joiplay/rga

Supports:
- public JoiPlay game archive metadata structure;
- legacy/public type values and engine template directories.

Do not treat this old type list as the sole current support list; S01 is current product evidence.

### S04 — JoiPlay android-mkxp
URL: https://github.com/joiplay/android-mkxp

Supports:
- Android/mobile MKXP lineage used as JoiPlay RPG Maker plugin reference.
- public proof of Android feasibility for RGSS family.

### S05 — JoiPlay android-mkxp app build configuration
URL: https://github.com/joiplay/android-mkxp/blob/master/app/build.gradle

Supports:
- older public build uses Android application module;
- ABI splits include armeabi-v7a, arm64-v8a, x86_64;
- older min/target SDK values (historical evidence only).

### S18 — JoiPlay release: Ren'Py 8.4.1 separate plugin
URL: https://www.patreon.com/joiplay/posts/joiplay-1-20-600-135070650

Supports:
- JoiPlay created a new Ren'Py plugin for 8.4.1 due to breaking changes;
- multiple historical Ren'Py plugin generations are maintained.

### S19 — JoiPlay Android MKXP manifest
URL: https://github.com/joiplay/android-mkxp/blob/master/app/src/main/AndroidManifest.xml

Supports:
- JoiPlay launcher/plugin architecture historically used broad external-storage permissions.
- This is historical implementation evidence, **not** a RuntimePack recommendation.

### S20 — JoiPlay RPG Maker XP RGA template Game.ini
URL: https://github.com/joiplay/rga/blob/master/rpgmxp/Game.ini

Supports:
- `.rxdata` script marker and RGSS1 library example.

### S21 — JoiPlay RPG Maker VX RGA template Game.ini
URL: https://github.com/joiplay/rga/blob/master/rpgmvx/Game.ini

Supports:
- `.rvdata` script marker and RGSS2 library example.

### S22 — JoiPlay RPG Maker VX Ace RGA template Game.ini
URL: https://github.com/joiplay/rga/blob/master/rpgmvxace/Game.ini

Supports:
- `.rvdata2` script marker and RGSS3 library example.

### S23 — JoiPlay Tyrano RGA template
URL: https://github.com/joiplay/rga/blob/master/tyrano/game.cfg

Supports:
- Tyrano `index.html` entrypoint in JoiPlay's public archive template.

### S24 — JoiPlay JoiPad
URL: https://github.com/joiplay/joipad

Supports:
- JoiPlay separates virtual gamepad into a reusable plugin component.

### S25 — JoiPlay public plugin repository license
URL: https://github.com/joiplay/plugin/blob/master/LICENSE

Supports:
- this specific `joiplay/plugin` repository is MIT licensed.
- does NOT establish licenses for unrelated JoiPlay repositories.

## Historical supplemental reference

### S26 — Archived JoiPlay site source
URL: https://github.com/joiplay/joiplay.github.io-archived/blob/master/index.html

Supports historical architecture statements:
- old JoiPlay used Ren'Py and RPG Maker plugins based on Ren'Py/RAPT/MKXP/android-mkxp.
- compatibility affected by native libraries, Win32API and Node.js modules.

Use only as historical context when current docs are silent.

## Source hygiene

Implementation agents must not introduce new external claims without adding an Sxx entry.

When a source changes materially:
- record the retrieval date;
- pin repository commits when code becomes a dependency;
- update affected requirement/ADR.


<!-- END REFERENCES.md -->
