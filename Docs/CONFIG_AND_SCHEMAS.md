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

UNKNOWN may build after explicit CLI opt-in. Hard unsupported native dependencies still fail; audit policy and audit accept flags do not override them.

These compatibility policies govern compatibility analysis, not the separate build audit policy in §8. Default informational audit continuation does not change a compatibility status or bypass other gates.

## 5. Build manifest

See `schemas/build-manifest.schema.json`.

## 6. Runtime manifest

See `schemas/runtime-manifest.schema.json`.

### 6.1 ABI declaration contract

`abis` describes the ABIs a runtime can support, and the rule for backing it depends on whether the runtime ships native code:

- **Runtime with `artifacts.native_libs_dir`** (RGSS, Ren'Py 7, Ren'Py 8): every declared ABI must be backed by that runtime's **own** directory under its own `native_libs_dir` containing at least one non-empty `.so` regular file, inside the resource root. An ABI directory that exists but is empty is **not** artifact support.
- **Runtime without `native_libs_dir`** (`web-rpgmaker`, `tyrano-web`, `vnmaker-web`): the generated APK carries no native library at all, so it is ABI-agnostic for a WebView backend. Absence of `.so` in some *other* runtime cannot narrow this declaration, and `effective_abis(declared, None)` returns the declaration unchanged. The declaration only has to be a valid `Abi` value.

The generated `app/build.gradle.kts` `abiFilters` are derived from the manifest itself (`build_plan::generator`), so the Android shell's `abiFilters` is not independent evidence of backend support.

**Known residual drift (not resolved here).** `runtimes/manifests/web-rpgmaker.toml` and `runtimes/manifests/tyrano-web.toml` declare `["arm64-v8a", "x86_64"]`, while the builtin registry entries for the same ids still declare `arm64-v8a` only. Production runtime selection and `effective_abis` are unchanged in this scope, so nothing narrows today; a future TOML loader that overrides builtins by id would narrow these two declarations to arm64 only. Resolving the mismatch is a separate, explicitly scoped decision. The `x86_64` declaration is a capability statement for a pure-WebView APK and has **not** been verified on an x86_64 device in this change.

## 7. Schema evolution

Rules:

- required semantic changes increment schema integer;
- parsers reject future unknown major schema versions;
- new optional fields may be added without breaking old readers.

## 8. Asset audit configuration and CLI options

### 8.1 Configuration (`runtimepack.toml`)

```toml
[audit]
mode = "standard"              # "standard" | "deep"
allow_network = false          # default false; external network assets evaluate to INDETERMINATE when false
```

### 8.2 CLI commands and flags

```text
runtimepack audit <game> [--mode standard|deep] [--allow-network-audit] [--output <path>] [--no-html]
runtimepack build <game> [--audit-mode standard|deep] [--strict-audit] [--accept-audit-fail] [--accept-audit-unknown] [--no-html]
```

- `--mode standard|deep` / `--audit-mode standard|deep`: Chooses bounded standard dependency check or exhaustive deep analysis (scripts, AST, archives, transitive assets, and candidate orphan classification).
- `--allow-network-audit`: Allows bounded network checks for remote HTTP(S) asset availability (5s timeout, 3 redirects cap, max body buffer limit) without executing remote payloads. Offline audits default external resources to `INDETERMINATE` under `CMP-AUD-011`.
- `--no-html`: Suppresses generation of the self-contained offline HTML audit report viewer (`asset-audit-report.html`).
- Build defaults to **informational** audit policy: the audit still runs and displays original findings/outcome, but `FAIL` and `INDETERMINATE` alone do not stop the build or require an accept flag.
- `--strict-audit`: Selects strict build audit policy. `FAIL` / `INDETERMINATE` stop with `BLD-104` unless the matching explicit override is supplied. `PASS` / `WARNING` proceed under both policies.
- `--accept-audit-fail`: Explicit outcome-specific override for `FAIL` under strict policy; does not accept `INDETERMINATE`.
- `--accept-audit-unknown`: Explicit outcome-specific override for `INDETERMINATE` under strict policy; does not accept `FAIL`.
- Policy selection is a build CLI decision, separate from audit mode and `[compatibility].policy`; no `[audit].policy` configuration key is introduced here. Standalone `runtimepack audit` exit behavior is unchanged.
- Non-bypassable hard blockers: Security violations, container corruption (`CNT-*`), detection failures/ambiguity, unresolved runtime version, unsupported native modules/libraries (`CMP-102`), and signature failures remain blocking under both policies and all accept flags. Audit-only `CNT-*` and unsupported-native findings must also block.

### 8.3 Core invariants

- **Source immutability:** Game source files are strictly read-only and immutable. Output paths inside the source root are rejected (`CMP-AUD-009`).
- **Atomic emission:** All reports are written to sibling temporary files and replaced atomically.
- **PASS definition:** Outcome `PASS` certifies **only that declared static scope passed**. It does **not** certify runtime playability or coverage of undeclared dynamic assets.
- **Static and runtime separation:** Static audit executes no game code. Telemetry progress bars or UI indicators are non-authoritative viewers and never constitute validation proof.
- **Orphan asset classification:** Assets in the source tree with no incoming reference edges in the dependency graph are reported as `INFO | POSSIBLY_UNUSED` (`CMP-AUD-013`). They are purely informational, never escalate outcomes, never block builds, and source files are never deleted. When dynamic coverage exists, the finding message clarifies that dynamic references exist and the asset may be resolved dynamically.
- **Cache invalidation rules:** Cache keys include SHA-256 of all ingested files, engine ID, audit mode, ruleset version, toolchain version, resource limits, network audit permission, and detected version. Any discrepancy forces a clean re-scan. Cache paths within the source directory are rejected. Corrupt cache entries fail-closed.
- **HTML report viewer:** Emits a standalone, zero-CDN, offline HTML report (`asset-audit-report.html`) containing outcome summary cards, completeness badges, statistics, searchable/filterable findings table, and interactive SVG/DOM dependency graph viewer. All game-controlled strings are sanitized via strict HTML entity escaping.

## 9. Asset audit report schema and manifest integration

Authoritative schema: `schemas/asset-audit-report.schema.json`.

Authoritative static report emitted at `asset-audit-report.json`:

```json
{
  "$schema": "./schemas/asset-audit-report.schema.json",
  "schema_version": 1,
  "engine": "rpgmaker-vxace",
  "mode": "STANDARD",
  "outcome": "PASS",
  "completeness": {
    "required_scope_complete": true,
    "covered_reference_classes": ["database", "event", "map", "script"],
    "uncovered_reference_classes": []
  },
  "statistics": {
    "nodes": 120,
    "references": 145,
    "findings": 0
  },
  "graph": {
    "nodes": [],
    "edges": []
  },
  "findings": [],
  "overrides": [],
  "runtime_validation": "NOT_RUN"
}
```

### 9.1 Authoritative schema enumerations

**Canonical engine/family spelling.** `engine` and the runtime manifest's `family` fields use one canonical spelling: `rpgmaker-xp`, `rpgmaker-vx`, `rpgmaker-vxace`, `rpgmaker-mv`, `rpgmaker-mz`, `renpy`, `tyrano`, `visual-novel-maker`, `unknown`; runtime families use `rgss`, `web`, `renpy`, `tyrano`, `vnmaker-web`. Reports and manifests written before this normalization used kebab-case (`rpg-maker-xp`, `rpg-maker-vx`, `rpg-maker-vx-ace`, `rpg-maker-vxace`, `rpg-maker-mv`, `rpg-maker-mz`, `ren-py`, `web-rpg-maker`, `tyrano-web`). Those legacy spellings are **accepted on read and deprecated**: the Rust deserializers map them to the canonical variant, the schemas list them in the same `enum` so pre-normalization archives still validate against `schema_version = 1`, and **no producer writes them** — serialization always emits the canonical spelling. Unknown spellings, including CLI-only abbreviations such as `mv`, `vx`, `vnm`, or `tyranobuilder`, are rejected. `schema_version` stays `1`: this is an additive read-compatibility change, and bumping the version would ripple through every producer and fixture with no functional gain.

Alias mapping (read-only; the right-hand side is what gets written):

| Deprecated wire value | Canonical variant written |
|---|---|
| `rpg-maker-xp` | `rpgmaker-xp` |
| `rpg-maker-vx` | `rpgmaker-vx` |
| `rpg-maker-vx-ace`, `rpg-maker-vxace` | `rpgmaker-vxace` |
| `rpg-maker-mv` | `rpgmaker-mv` |
| `rpg-maker-mz` | `rpgmaker-mz` |
| `ren-py` | `renpy` |
| `web-rpg-maker` (manifest `family`) | `web` |
| `tyrano-web` (manifest `family`) | `tyrano` |

- `engine`: `"rpgmaker-xp"`, `"rpgmaker-vx"`, `"rpgmaker-vxace"`, `"rpgmaker-mv"`, `"rpgmaker-mz"`, `"renpy"`, `"tyrano"`, `"visual-novel-maker"`, `"unknown"`
- `mode`: `"STANDARD"`, `"DEEP"`
- `outcome`: `"PASS"`, `"WARNING"`, `"FAIL"`, `"INDETERMINATE"`
- `completeness`: object with `required_scope_complete` (boolean), `covered_reference_classes` (`string[]`), and `uncovered_reference_classes` (`string[]`)
- `statistics`: object with integer counts: `nodes`, `references`, `findings`
- `evidence.stage`:
  1. `"REFERENCE_FOUND"`
  2. `"PATH_RESOLVED"`
  3. `"FILE_EXISTS"`
  4. `"FILE_READABLE"`
  5. `"INTEGRITY_VERIFIED"`
  6. `"DECODE_SUCCESS"`
  7. `"ENGINE_LOAD_SUCCESS"`
  8. `"RUNTIME_USE_SUCCESS"`
- `edge.requiredness` and `finding.requiredness`: `"REQUIRED"`, `"OPTIONAL"` (defaults to `"REQUIRED"`)
- `finding.severity`: `"INFO"`, `"WARNING"`, `"ERROR"`
- `finding.disposition`: `"INFORMATIONAL"`, `"WARNING"`, `"BLOCKING"`, `"INDETERMINATE"`
- `runtime_validation`: scalar enum `"NOT_RUN"`, `"PASS"`, `"WARNING"`, `"FAIL"`, `"INDETERMINATE"`
- `override`: object with `flag` (`string`) and `rationale` (`string`)

### 9.2 Build manifest integration

In `schemas/build-manifest.schema.json`, the optional `audit` property records the policy and original static outcome. `audit.policy` is `"informational"` for new default CLI builds or `"strict"` for `--strict-audit`. When the field is absent in a legacy manifest, readers default it to `"strict"` to preserve historical semantics.

Informational continuation does not add a synthetic override or rewrite the outcome to `PASS`. `overrides` records only actually applied explicit outcome-specific overrides. For an informational build with no applied override, use `"policy": "informational"`, the original outcome, and `"overrides": []`.

Example of a strict build explicitly accepting `INDETERMINATE` (not changing it to `PASS`):

```json
{
  "audit": {
    "report_path": "asset-audit-report.json",
    "report_sha256": "4a5e3b67c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5",
    "mode": "STANDARD",
    "policy": "strict",
    "outcome": "INDETERMINATE",
    "overrides": [
      {
        "flag": "--accept-audit-unknown",
        "original_outcome": "INDETERMINATE",
        "accepted_finding_ids": ["CMP-AUD-001"],
        "rationale": "Placeholder adapter accepted for baseline package build"
      }
    ]
  }
}
```

