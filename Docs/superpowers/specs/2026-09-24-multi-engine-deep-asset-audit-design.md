# Multi-Engine Deep Asset Audit Design

**Date:** 2026-09-24
**Status:** Approved

## Objective

RuntimePack will provide an authoritative, multi-engine audit system for all v1 engines: RPG Maker XP/VX/VX Ace, RPG Maker MV/MZ, Ren'Py, and Tyrano. The system separates static dependency coverage from runtime load evidence and never treats UI progress as proof of validation.

## Architecture

```text
runtimepack audit/build
  -> AuditCoordinator
  -> EngineAssetAuditor adapter
  -> Asset Dependency Graph
  -> StaticAuditReport (authoritative JSON)
  -> BuildPolicyGate
  -> APK/AAB
  -> Runtime instrumentation
  -> Optional controlled probe
  -> RuntimeValidationReport
  -> CombinedValidationSummary
```

Core remains engine-agnostic. Engine-specific extraction and reference semantics live in adapters. The overlay is only a viewer, progress monitor, diagnostic console, and probe controller.

## Evidence stages

Stages are independent and monotonic:

```text
REFERENCE_FOUND
PATH_RESOLVED
FILE_EXISTS
FILE_READABLE
INTEGRITY_VERIFIED
DECODE_SUCCESS
ENGINE_LOAD_SUCCESS
RUNTIME_USE_SUCCESS
```

A later stage may only be reported when the corresponding operation was observed. Fallbacks use explicit terminal evidence such as `PLACEHOLDER_SUBSTITUTED` and `LOAD_SUPPRESSED`.

## Outcomes

- `PASS`: declared audit scope completed with no blocking finding.
- `WARNING`: build may continue; all findings remain visible.
- `FAIL`: blocks build unless `--accept-audit-fail` is supplied.
- `INDETERMINATE`: blocks build unless `--accept-audit-unknown` is supplied.
- `NOT_RUN`: runtime probe was not executed; never described as runtime-verified.

Overrides are recorded in the audit report, build manifest, and provenance. They cannot bypass security, container corruption, legal gates, ambiguous engine detection, unresolved runtime selection, or unsupported native dependencies.

## Audit modes

- `standard`: default for `runtimepack audit` and automatic `runtimepack build` audit; bounded required references and dependencies.
- `deep`: script/archive analysis, transitive dependencies, broader format checks, and orphan analysis.

Network access is disabled by default. Required HTTP(S) assets produce `INDETERMINATE`; `--allow-network-audit` permits bounded checks with timeout, redirect, and size limits without executing content.

## CLI

```text
runtimepack audit <game>
runtimepack audit <game> --mode deep
runtimepack audit <game> --allow-network-audit
runtimepack build <game>
runtimepack build <game> --audit-mode deep
runtimepack build <game> --accept-audit-fail
runtimepack build <game> --accept-audit-unknown
runtimepack probe <target> --mode sampled|exhaustive
```

## Dependency graph

Nodes represent source locations and resources: map, event, event page, command, database entity, script, plugin, image, audio, font, video, archive entry, external resource, runtime-provided resource, or unresolved dynamic reference.

Edges preserve every source reference and contain relation, requiredness, extraction evidence, and resolution state. References are required by default; only documented engine/profile rules may mark them optional.

Orphans are reported only as `INFO | POSSIBLY_UNUSED`, never block builds, and are never deleted automatically.

## Engine adapters

- **RGSS:** maps, event/common/troop commands, all core databases and System, script archives, RTP, encrypted RGSS archives, image/audio/font references, and unresolved dynamic Ruby paths.
- **MV/MZ:** map/database JSON, event commands, plugins, JavaScript imports and literals, encrypted assets, Node/NW.js dependencies, WebGL/WebAudio resources.
- **Ren'Py:** `.rpy`, supported `.rpyc/.rpa` inventory/parsers, image/audio/video declarations, Python imports, native extensions, and versioned runtime requirements.
- **Tyrano:** scenario tags and jumps, plugins, unpacked/ASAR assets, Node/Electron dependencies; technical result remains separate from the legal gate.

Unsupported parsers and dynamic references yield `INDETERMINATE`, never success.

## Runtime validation

Runtime instrumentation records request, selected target, read/integrity, decode, engine load, fallback, and runtime use as structured events. Probe is optional after build:

- `sampled`: default, deterministic representative selection.
- `exhaustive`: all unique assets, processed incrementally with bounded concurrency, memory limits, timeout, checkpoint/resume, cancellation, and immediate resource release.

Required asset fallback/suppression is `FAIL`; optional asset fallback/suppression is `WARNING`. Probe failure does not retroactively change historical build success, but CombinedValidationSummary reports the runtime failure.

## Reports

Authoritative artifacts:

```text
asset-audit-report.json
asset-dependency-graph.json
runtime-validation-report.json       # only when probe runs
combined-validation-summary.json     # when static and runtime results are combined
asset-audit-report.html              # viewer generated from JSON
```

Reports use logical/relative paths only. Host/device absolute paths and user-identifying data are sanitized. HTML is a non-authoritative offline viewer generated from JSON and performs no independent status calculation.

Cache validity requires identical content hash, engine profile, audit mode, policy, limits, auditor/parser versions, schema version, and compatibility ruleset version.

## Performance and safety

Audits use one source index, bounded parsers, deterministic merges, explicit resource budgets, and cancellation. Media validation progresses through enumeration, bounded read/header validation, bounded decode, then optional engine probe. Limit exhaustion is reported and prevents `PASS`.

Input source remains immutable. Static audit never executes game scripts, executables, native libraries, Python, NW.js, or RPG Maker runners.

## Acceptance criteria

- Synthetic progress can never produce PASS.
- Zero-scan, timeout, parser failure, limit exhaustion, malformed report, or unavailable required adapter yields FAIL/INDETERMINATE.
- Static PASS and runtime FAIL remain separate and traceable.
- Every finding retains all reference sources.
- Reports and cache output are deterministic.
- Every adapter declares covered and uncovered reference classes.
- Fixtures cover valid, missing, corrupt, unreadable, case mismatch/collision, unsupported format, archive/RTP, external URL, dynamic reference, and native dependency.
- Device tests exercise actual image/audio load, fallback detection, and report persistence.

## Requirement traceability

PRD-001, PRD-007, PRD-008, PRD-017, PRD-020; TECH-CORE-001, TECH-CORE-003, TECH-CORE-004, TECH-INPUT-001/002, TECH-RUNTIME-002, TECH-RGSS-004/005, TECH-WEB-004/005, TECH-RENPY-002/004, and TECH-TYRANO-004.
