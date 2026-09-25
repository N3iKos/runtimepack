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
runtimepack build <game> [--audit-mode standard|deep] [--accept-audit-fail] [--accept-audit-unknown] [--no-html]
```

- `--mode standard|deep` / `--audit-mode standard|deep`: Chooses bounded standard dependency check or exhaustive deep analysis (scripts, AST, archives, transitive assets, and candidate orphan classification).
- `--allow-network-audit`: Allows bounded network checks for remote HTTP(S) asset availability (5s timeout, 3 redirects cap, max body buffer limit) without executing remote payloads. Offline audits default external resources to `INDETERMINATE` under `CMP-AUD-011`.
- `--no-html`: Suppresses generation of the self-contained offline HTML audit report viewer (`asset-audit-report.html`).
- `--accept-audit-fail`: Explicitly accepts an audit outcome of `FAIL`; rejects `INDETERMINATE`.
- `--accept-audit-unknown`: Explicitly accepts an audit outcome of `INDETERMINATE`; rejects `FAIL`.
- Non-bypassable hard blockers: Security violations, container corruption (`CNT-*`), Tyrano legal gate (`TYR-101`), ambiguous engine detection, unresolved runtime version, and unsupported native modules/libraries (`CMP-102`) cannot be overridden.

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
  "engine": "rpg-maker-vx-ace",
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

- `engine`: `"rpg-maker-xp"`, `"rpg-maker-vx"`, `"rpg-maker-vx-ace"`, `"rpg-maker-mv"`, `"rpg-maker-mz"`, `"ren-py"`, `"tyrano"`, `"unknown"`
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

In `schemas/build-manifest.schema.json`, the optional `audit` property records:

```json
{
  "audit": {
    "report_path": "asset-audit-report.json",
    "report_sha256": "4a5e3b67c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5",
    "mode": "STANDARD",
    "outcome": "PASS",
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

