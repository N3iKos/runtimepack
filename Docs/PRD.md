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
- TyranoBuilder/TyranoScript

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
| PRD-018 | TyranoScript supported via Web runtime packaging | P0 |
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
4. Tyrano desktop plugins may rely on NW.js/Node APIs not present in Android WebView.
5. Licenses of runtime dependencies can impose redistribution/source/notice obligations.
6. Asset encryption is hardening, not absolute secrecy.

## 10. v1 release gate

v1 may be declared complete only when:

- XP/VX/VX Ace minimal fixtures boot;
- MV/MZ minimal fixtures boot;
- selected Ren'Py version fixtures boot;
- Tyrano minimal fixtures boot;
- detector and reports are deterministic;
- APK signing works;
- save isolation works;
- no normal game requires broad storage permissions;
- license report is emitted.
