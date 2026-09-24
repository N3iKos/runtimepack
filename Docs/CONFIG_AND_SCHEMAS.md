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
runtimepack audit <game> [--mode standard|deep] [--allow-network-audit] [--output <path>]
runtimepack build <game> [--audit-mode standard|deep] [--accept-audit-fail] [--accept-audit-unknown]
```

- `--mode standard|deep` / `--audit-mode standard|deep`: Chooses bounded standard dependency check or exhaustive deep analysis (scripts, AST, archives, transitive assets, and orphans).
- `--allow-network-audit`: Allows bounded network checks for remote HTTP(S) asset availability without executing remote payloads.
- `--accept-audit-fail`: Explicitly accepts an audit outcome of `FAIL`; rejects `INDETERMINATE`.
- `--accept-audit-unknown`: Explicitly accepts an audit outcome of `INDETERMINATE`; rejects `FAIL`.
- Non-bypassable hard blockers: Security violations, container corruption (`CNT-*`), Tyrano legal gate (`TYR-101`), ambiguous engine detection, unresolved runtime version, and unsupported native modules/libraries (`CMP-102`) cannot be overridden.

### 8.3 Core invariants

- **Source immutability:** Game source files are strictly read-only and immutable. Output paths inside the source root are rejected (`CMP-AUD-009`).
- **Atomic emission:** All reports are written to sibling temporary files and replaced atomically.
- **PASS definition:** Outcome `PASS` certifies **only that declared static scope passed**. It does **not** certify runtime playability or coverage of undeclared dynamic assets.
- **Static and runtime separation:** Static audit executes no game code. Telemetry progress bars or UI indicators are non-authoritative viewers and never constitute validation proof.

## 9. Asset audit report schema and manifest integration

Authoritative schema: `schemas/asset-audit-report.schema.json`.

Authoritative static report emitted at `asset-audit-report.json`:

```json
{
  "$schema": "./schemas/asset-audit-report.schema.json",
  "schema_version": 1,
  "engine": "rpgmaker-vxace",
  "mode": "standard",
  "outcome": "PASS",
  "completeness": "complete",
  "statistics": {
    "total_nodes": 120,
    "total_edges": 145,
    "missing_required": 0,
    "missing_optional": 0,
    "orphans": 3
  },
  "graph": {
    "nodes": [],
    "edges": []
  },
  "findings": [],
  "overrides": [],
  "runtime_validation": {
    "state": "not_run"
  }
}
```

### 9.1 Authoritative schema enumerations

- `mode`: `"standard"`, `"deep"`
- `outcome`: `"PASS"`, `"WARNING"`, `"FAIL"`, `"INDETERMINATE"`
- `completeness`: `"complete"`, `"partial"`, `"indeterminate"`
- `evidence.stage`:
  1. `"REFERENCE_FOUND"`
  2. `"PATH_RESOLVED"`
  3. `"FILE_EXISTS"`
  4. `"FILE_READABLE"`
  5. `"INTEGRITY_VERIFIED"`
  6. `"DECODE_SUCCESS"`
  7. `"ENGINE_LOAD_SUCCESS"`
  8. `"RUNTIME_USE_SUCCESS"`
- `edge.requiredness`: `"required"`, `"optional"` (defaults to `"required"`)
- `runtime_validation.state`: `"not_run"`, `"sampled"`, `"exhaustive"`

### 9.2 Build manifest integration

In `schemas/build-manifest.schema.json`, the optional `audit` property records:

```json
{
  "audit": {
    "report_path": "asset-audit-report.json",
    "report_sha256": "4a5e3...",
    "mode": "standard",
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

