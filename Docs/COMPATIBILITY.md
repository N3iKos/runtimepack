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

## 11. Deep asset audit policy and engine-neutral static audit

### 11.1 Authoritative terminology

- **Monotonic Evidence Stages:**
  Asset evidence progresses monotonically across 8 discrete stages:
  1. `REFERENCE_FOUND` — reference identified in scripts, maps, events, or databases.
  2. `PATH_RESOLVED` — normalized logical path resolved against project root.
  3. `FILE_EXISTS` — target file confirmed present on the filesystem.
  4. `FILE_READABLE` — file permissions and storage grant read access.
  5. `INTEGRITY_VERIFIED` — container/file checksum, magic bytes, and headers verified.
  6. `DECODE_SUCCESS` — media decoder (image/audio/font) parses payload without error.
  7. `ENGINE_LOAD_SUCCESS` — engine-specific loader instantiates asset into memory.
  8. `RUNTIME_USE_SUCCESS` — asset actively referenced or rendered during execution.

  Evidence cannot skip intermediate stages (e.g. from `REFERENCE_FOUND` directly to `DECODE_SUCCESS`). Fallback paths record explicit terminal states (`PLACEHOLDER_SUBSTITUTED`, `LOAD_SUPPRESSED`).

- **Authoritative Static Outcomes (`AuditOutcome`):**
  - `PASS` — declared static audit scope completed with zero blocking findings.
    **CRITICAL DEFINITION:** `PASS` means **only that declared static scope passed**. It does **not** guarantee 100% runtime playability, universal engine support, or coverage of undeclared dynamic assets.
  - `WARNING` — non-blocking issues identified (e.g., missing optional assets, soft audio fallback); build may proceed without overrides.
  - `FAIL` — missing required assets or hard dependency failures; blocks build unless explicitly overridden by `--accept-audit-fail`.
  - `INDETERMINATE` — incomplete scan scope, unresolvable dynamic expressions, unsupported archive/script parsers, or unavailable engine auditor adapters; blocks build unless explicitly overridden by `--accept-audit-unknown`.

- **Runtime Validation State (`RuntimeValidationState`):**
  Distinct from the static `AuditOutcome`, the runtime validation status is tracked separately via scalar `RuntimeValidationState` (`NOT_RUN`, `PASS`, `WARNING`, `FAIL`, `INDETERMINATE`):
  - `NOT_RUN` — runtime probe was not executed; must never be described as runtime-verified.
  - `PASS` / `WARNING` / `FAIL` / `INDETERMINATE` — observed result of in-app runtime instrumentation or probe.

- **Audit Modes:**
  - `STANDARD` / `standard` — default mode for `runtimepack audit` and pre-build audit; bounded required static dependency and asset verification.
  - `DEEP` / `deep` — exhaustive static scan including script/code AST and string literals, container/archive contents, transitive dependencies, format checks, and orphan discovery.
  - Network access is disabled by default; external HTTP(S) asset references evaluate to `INDETERMINATE`. `--allow-network-audit` permits bounded network reachability checks without executing payload.

- **Override Constraints and Fail-Closed Matrix:**
  Build gating enforces an outcome-specific override matrix:

  | Outcome | No Override | `--accept-audit-fail` | `--accept-audit-unknown` |
  |---|---|---|---|
  | `PASS` | Allow | Allow | Allow |
  | `WARNING` | Allow | Allow | Allow |
  | `FAIL` | Block (`BLD-104`) | Allow | Block (`BLD-104`) |
  | `INDETERMINATE` | Block (`BLD-104`) | Block (`BLD-104`) | Allow |

  **Non-Bypassable Hard Blockers:** Neither `--accept-audit-fail` nor `--accept-audit-unknown` can ever bypass:
  1. Security violations or container corruption (`CNT-*`);
  2. Tyrano legal gate (`TYR-101`);
  3. Ambiguous engine detection (`DET-AMBIGUOUS`);
  4. Unresolved runtime version (`RUNTIME_VERSION_UNRESOLVED`);
  5. Unsupported native binary dependencies (`CMP-102` PE DLL / `.node` / `.pyd`).

  Every applied override is immutably recorded:
  - In `asset-audit-report.json`: recorded as `AuditOverride` objects (`flag`, `rationale`).
  - In `build-manifest.json` and build provenance metadata: recorded as `AuditOverrideRecord` structures (`flag`, `original_outcome`, `accepted_finding_ids`, `rationale`).

- **Static and Runtime Separation:**
  Static dependency analysis and runtime execution evidence are strictly decoupled:
  - Static audit operates exclusively at build time via pure static inspection without executing game code.
  - Runtime validation occurs in-app via instrumentation and optional probe (`runtimepack probe`).
  - Runtime probe results are recorded in `runtime-validation-report.json` and combined in `combined-validation-summary.json`.
  - Probe failures do not retroactively invalidate past build success, but combined reports reflect runtime failure.
  - In-game overlay progress bars and telemetry viewers (e.g. Diagnostic Suite) are non-authoritative monitoring interfaces and must never be treated as proof of static or runtime validation.

- **Source Immutability:**
  Game source files are 100% read-only and immutable:
  - Static audit never modifies, moves, or creates files within the source directory.
  - Static audit never executes game code, scripts, runners, Node/Electron runtimes, Python, or native binaries.
  - Output report paths located inside the source tree are strictly rejected with stable diagnostic `CMP-AUD-009`.
  - All report generation uses atomic writes (temporary sibling file followed by atomic rename) strictly outside the source tree.

- **Orphan Asset Semantics:**
  Assets present on disk but unreferenced by declared dependency trees are classified as `INFO | POSSIBLY_UNUSED` (`CMP-AUD-011`). They are informational, never escalate outcome, never block builds, and are never deleted automatically.

