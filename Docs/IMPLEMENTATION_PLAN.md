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
