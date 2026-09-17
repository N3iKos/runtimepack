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
