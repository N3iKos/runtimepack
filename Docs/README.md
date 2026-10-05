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
- Tyrano support is provided via the Web runtime backend for browser-compatible projects.
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
│   └── tyrano/        # adapter over web runtime
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
