# AGENTS.md

This file is the root instruction contract for every autonomous or agentic coding system working on this repository.

It applies to the entire repository unless a deeper directory contains its own `AGENTS.md` with more specific instructions.

---

## 1. Project objective

RuntimePack is a **build-time compatibility-runtime packager** that takes a supported game, detects its engine, analyzes compatibility, selects the correct Android runtime backend, and emits a standalone Android APK/AAB.

v1 scope is strictly limited to:

- RPG Maker XP
- RPG Maker VX
- RPG Maker VX Ace
- RPG Maker MV
- RPG Maker MZ
- Ren'Py
- TyranoBuilder / TyranoScript

Do not silently extend this scope.

The generated application must behave like a dedicated Android game:

```text
install APK
  -> tap icon
  -> runtime initializes
  -> game starts directly
```

RuntimePack v1 is **not**:

- a general Windows emulator;
- a Wine/x86 compatibility product;
- a source-to-source game transpiler;
- a universal Android game launcher;
- a promise of 100% engine compatibility.

---

## 2. Mandatory documentation reading

Before changing code, an agent MUST read the canonical documents in `docs/`.

Read them in this exact order:

```text
docs/
├── PRD.md
├── TECHNICAL_SPEC_V1.md
├── ARCHITECTURE.md
├── ENGINE_DETECTION.md
├── IMPLEMENTATION_PLAN.md
└── REFERENCES.md
```

### 2.1 Reading requirements

Do not skim only headings.

The agent must understand, at minimum:

1. current v1 scope;
2. product non-goals;
3. architectural boundaries;
4. engine detection rules;
5. runtime-selection model;
6. current implementation milestone;
7. external evidence relevant to the task.

If one of the six required documents is missing:

```text
STOP
-> report DOCS_MISSING
-> identify the missing file
-> do not invent its contents
```

If additional documents exist inside `docs/`, read the ones relevant to the task after the six canonical files.

Examples:

```text
runtime/VFS task
-> also read CONTENT_VFS_SECURITY.md

Android task
-> also read ANDROID_RUNTIME.md

compatibility task
-> also read COMPATIBILITY.md

test task
-> also read TEST_PLAN.md

license/dependency task
-> also read LICENSE_COMPLIANCE.md

runtime API task
-> also read RUNTIME_CONTRACT.md
```

---

## 3. Documentation precedence

If two documents appear to conflict, use the following precedence:

```text
1. AGENTS.md
2. docs/TECHNICAL_SPEC_V1.md
3. accepted ADRs
4. docs/ARCHITECTURE.md
5. engine/runtime-specific specification
6. docs/PRD.md
7. docs/IMPLEMENTATION_PLAN.md
8. examples
```

`docs/REFERENCES.md` is the authority for external factual evidence, not for overriding an explicit project design decision.

If a conflict remains unresolved:

```text
STOP
-> report SPEC_CONFLICT
-> quote the conflicting requirement IDs/files
-> do not choose arbitrarily
```

---

## 4. Evidence-first rule

Do not implement external engine behavior from memory.

External claims must be traceable to an evidence entry in:

```text
docs/REFERENCES.md
```

Reference IDs use the form:

```text
S01
S02
S03
...
```

Before using an external claim:

```text
claim
  -> locate Sxx
  -> verify that the source actually supports the claim
  -> identify affected requirement
  -> add/update test
  -> implement
```

If no evidence exists:

```text
do not guess
do not silently assume
do not search for a convenient implementation and call it authoritative
```

Instead create or use an explicit:

```text
OPEN QUESTION
SPIKE
BLOCKER
```

---

## 5. Anti-hallucination rules

Agents MUST NOT:

1. invent engine APIs;
2. invent compatibility guarantees;
3. invent RPG Maker file signatures;
4. invent Ren'Py version mappings;
5. invent Tyrano redistribution permissions;
6. claim a Windows API shim exists when it has not been implemented and tested;
7. silently treat an unknown native dependency as supported;
8. silently change engine family after failed detection;
9. use `"latest"` for runtime/toolchain resolution;
10. introduce unpinned dependencies into production build logic;
11. assume all JoiPlay repositories share the same license;
12. use public source availability as proof that redistribution is permitted;
13. output fake compatibility percentages;
14. execute untrusted game code during inspection/detection;
15. mark `UNKNOWN` as compatible.

Unknown information must remain unknown until verified.

---

## 6. Core architectural invariants

These rules are non-negotiable unless changed by an accepted ADR.

### 6.1 Build-time runtime selection

Runtime selection happens during build:

```text
Game Source
   ↓
Detect
   ↓
Analyze
   ↓
Resolve Runtime
   ↓
Generate Android Application
```

Do not move engine selection into the generated production APK.

---

### 6.2 One primary runtime family per production APK

Default production output:

```text
XP/VX/VX Ace
-> RGSS runtime

MV/MZ
-> RPG Maker Web runtime

Ren'Py
-> versioned Ren'Py runtime

Tyrano
-> Tyrano adapter over Web runtime
```

Do not bundle every runtime into every APK.

---

### 6.3 Core must remain engine-agnostic

Forbidden:

```text
core/
  if engine == VX_Ace ...
```

Preferred:

```text
Detector
Analyzer
RuntimeResolver
RuntimeBackend
Adapter
```

Engine-specific logic belongs in engine modules.

---

### 6.4 Input game is immutable

Do not modify the user's source directory during normal operation.

All transformations happen in:

```text
working directory
generated overlay
content container
Android build directory
```

---

### 6.5 Save data is separate

Immutable game content and mutable saves are distinct.

```text
game content
-> read-only

save/config
-> app-specific writable storage
```

---

### 6.6 VFS-first architecture

Runtime integrations should access game content through RuntimePack's content/VFS abstraction where technically practical.

Do not design production encrypted mode around:

```text
decrypt entire game
-> write plaintext directory
-> run
```

---

## 7. Engine rules

### 7.1 RPG Maker XP

Expected primary markers include:

```text
Game.ini
Data/Scripts.rxdata
RGSS1-family metadata
```

Do not classify a project as XP solely because `Game.exe` exists.

---

### 7.2 RPG Maker VX

Expected primary markers include:

```text
Game.ini
Data/Scripts.rvdata
RGSS2-family metadata
```

---

### 7.3 RPG Maker VX Ace

Expected primary markers include:

```text
Game.ini
Data/Scripts.rvdata2
RGSS3-family metadata
```

---

### 7.4 RPG Maker MV

Expected primary markers include:

```text
index.html
js/rpg_core.js
js/rpg_managers.js
data/System.json
```

A deployed `www/` root may need normalization.

---

### 7.5 RPG Maker MZ

Expected primary markers include:

```text
index.html
js/rmmz_core.js
js/rmmz_managers.js
data/System.json
```

MV and MZ are separate engine profiles.

---

### 7.6 Ren'Py

Detection and runtime-version resolution are separate.

Valid state:

```text
engine = RenPy
version = UNKNOWN
```

In that state, do not guess a runtime.

Return:

```text
RUNTIME_VERSION_UNRESOLVED
```

unless an explicitly documented safe resolution rule exists.

---

### 7.7 Tyrano

Tyrano is technically part of v1 architecture.

However distribution remains subject to:

```text
TYRANO-LEGAL
```

Until that gate is explicitly cleared:

- detector work is allowed;
- analyzer work is allowed;
- interface work is allowed;
- synthetic/permitted testing is allowed;
- bundling restricted Tyrano engine files into release artifacts is NOT allowed.

Do not bypass this gate.

---

## 8. Detection rules

Detection must be:

```text
static
deterministic
non-executing
evidence-based
```

All detectors run independently and return candidates.

Do not implement:

```text
first detector that matches wins
```

Use explicit evidence/conflict resolution.

Supported confidence states:

```text
EXACT
HIGH
MEDIUM
LOW
NONE
AMBIGUOUS
```

Default automatic build policy:

```text
EXACT -> allowed
HIGH  -> allowed

MEDIUM -> explicit acceptance required
LOW    -> fail
NONE   -> fail
AMBIGUOUS -> fail
```

Forced engine override must be recorded in the build report.

---

## 9. Compatibility policy

Supported states:

```text
COMPATIBLE
COMPATIBLE_WITH_SHIMS
DEGRADED
UNSUPPORTED_DEPENDENCY
UNKNOWN
BLOCKED
```

Never replace these with an arbitrary score.

A compatibility shim requires:

```text
capability ID
implementation
test
compatibility-db entry
diagnostic behavior
```

Unknown Windows APIs, native Node modules, Windows DLLs, or binary Python extensions must never silently succeed.

---

## 10. Runtime contracts

Runtime-specific code must implement the documented Runtime Contract.

Do not expose engine internals across the whole project.

Expected conceptual boundary:

```text
RuntimeBackend
├── create
├── mountContent
├── configure
├── attachSurface
├── start
├── onInput
├── pause
├── resume
└── stop
```

If the Runtime Contract needs a breaking change:

```text
do not patch around it locally
-> create ADR
-> increment runtime API if necessary
```

---

## 11. Android rules

Generated application is a game, not a launcher.

`BootstrapActivity` must not:

- scan the source game;
- detect engine;
- choose between unrelated runtimes;
- display a game library.

Those decisions belong to host-side build tooling.

### Storage

Normal generated applications must use app-specific storage.

Do not add:

```text
MANAGE_EXTERNAL_STORAGE
```

or broad legacy external-storage permissions merely because older launcher-oriented software used them.

### Permissions

Permissions are capability-driven.

Example:

```text
offline game
-> no INTERNET permission

network-dependent game
-> INTERNET may be generated
```

No universal "just in case" permission manifest.

---

## 12. Dependency and license rules

Before adding third-party code:

```text
1. identify repository/source
2. pin exact version/commit
3. verify license
4. record provenance
5. verify redistribution obligations
6. add notices
7. only then integrate
```

Never infer one repository's license from another repository owned by the same organization.

Special cases:

```text
JoiPlay
-> architectural/public reference
-> not assumed reusable as one monolithic codebase

mkxp-z / RGSS runtime
-> license review mandatory

Ren'Py
-> exact runtime dependency notice bundle mandatory

Tyrano
-> legal gate currently blocks unrestricted bundling
```

---

## 13. Work procedure for every task

Before coding:

```text
1. Read required docs.
2. Identify requirement IDs.
3. Identify relevant Sxx evidence.
4. Inspect existing code.
5. State assumptions internally.
6. Check whether a SPIKE/BLOCKER already exists.
7. Define acceptance tests.
8. Implement smallest compliant change.
9. Run tests.
10. Update docs if behavior changed.
```

Do not redesign unrelated subsystems during a scoped task.

---

## 14. Agent task format

Prefer bounded tasks.

Good:

```yaml
id: DET-VXA-001
goal: Detect RPG Maker VX Ace using static evidence.
requirements:
  - PRD-001
  - PRD-002
evidence:
  - S22
acceptance:
  - "VX Ace fixture => EXACT"
  - "Game.exe-only fixture => not VX Ace"
  - "no process execution during detection"
```

Bad:

```text
implement RPG Maker
```

---

## 15. Definition of done

A feature is not complete unless all applicable items are satisfied:

- implementation exists;
- tests exist;
- relevant requirement IDs are satisfied;
- errors use stable diagnostic codes;
- no unrelated scope expansion occurred;
- no unpinned dependency was introduced;
- license/provenance was updated if needed;
- user source game remains untouched;
- documentation reflects behavior;
- build/test command passes.

---

## 16. Test requirements

Every compatibility bug should become a regression test when legally possible.

Required test hierarchy:

```text
unit
integration
runtime
Android instrumentation/device
regression fixture
```

Do not consider:

```text
Gradle build succeeded
```

equivalent to runtime compatibility.

For playable engine paths, validation should eventually cover:

```text
boot
first frame
input
audio
pause/resume
save
process restart
load
```

---

## 17. Security rules

Do not market or document encrypted content as impossible to extract.

Use the term:

```text
asset hardening
```

or:

```text
encrypted content protection
```

Threat model does not guarantee secrecy against a rooted/instrumented device.

Do not implement custom cryptographic primitives.

Use vetted AEAD implementations selected by an approved security decision.

---

## 18. Error handling

Never swallow errors that affect compatibility.

Use stable domains:

```text
ING-xxx
DET-xxx
CMP-xxx
RT-xxx
RGS-xxx
WEB-xxx
RNP-xxx
TYR-xxx
CNT-xxx
AND-xxx
BLD-xxx
SIG-xxx
LIC-xxx
```

An error should contain enough context for another agent to reproduce it.

---

## 19. When the agent must stop

Stop the affected implementation path and report a blocker for:

```text
DOCS_MISSING
SPEC_CONFLICT
SOURCE_UNVERIFIED
LICENSE_BLOCKED
AMBIGUOUS_ENGINE
RUNTIME_VERSION_UNRESOLVED
UNSUPPORTED_NATIVE_DEPENDENCY
TOOLCHAIN_MISSING
```

Do not resolve these by guessing.

---

## 20. Current implementation priority

Unless the issue tracker or user explicitly changes priority, follow:

```text
Phase 0
repository + schemas + diagnostics

Phase 1
static detection / inspect / explain

Phase 2
Android host shell

Phase 3
VFS + packed content

Phase 4
RGSS vertical slice

Phase 5
MV/MZ web runtime

Phase 6
Ren'Py

Phase 7
encrypted-streaming content

Phase 8
Tyrano technical integration, subject to legal gate

Phase 9
release hardening / AAB / provenance
```

Do not jump to complex compatibility hacks before shared foundations exist.

---

## 21. Expected repository behavior

An agent entering the repository should be able to run or eventually implement:

```bash
runtimepack doctor
runtimepack inspect <game>
runtimepack explain <game>
runtimepack build <game>
runtimepack runtime list
runtimepack licenses
```

The important architectural flow is always:

```text
source game
  ↓
ingest
  ↓
static detection
  ↓
compatibility analysis
  ↓
runtime resolution
  ↓
build plan
  ↓
content packaging
  ↓
Android project
  ↓
compile/sign
  ↓
validate/report
```

Preserve this boundary unless an accepted ADR explicitly changes it.

---

## 22. Final rule

When documentation, source evidence, and assumptions disagree:

```text
documentation + verified evidence win
```

When documentation and verified evidence are insufficient:

```text
UNKNOWN wins over invention
```

The project is intended to be maintainable by multiple autonomous agents. Predictability, traceability, reproducibility, and explicit uncertainty are more important than producing a speculative implementation quickly.

---

# Memory Protocol
Di awal SETIAP sesi baru, baca berurutan:
1. memory/memory.md
2. memory/progress.md
3. memory/decisions_log.md (3 entry teratas cukup, kecuali perlu history lengkap)
4. memory/technical_findings.md (scan judul-judul saja, buka detail kalau relevan)

Update di akhir sesi kerja atau saat topik besar selesai:
- progress.md → status terbaru
- decisions_log.md → kalau ada keputusan baru
- technical_findings.md → kalau nemu bug/gotcha baru
- memory.md → HANYA kalau ada perubahan di "3 hal paling penting" atau preferensi

Jangan duplikat informasi antar file. Kalau ragu taruh dimana, technical detail → technical_findings.md, 
keputusan strategis → decisions_log.md, status kerja → progress.md.
