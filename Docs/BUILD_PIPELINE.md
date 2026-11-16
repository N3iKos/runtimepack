# Build Pipeline

## 1. Pipeline overview

```text
Source
  |
  v
1 Ingest
  |
2 Index
  |
3 Detect
  |
4 Analyze
  |
5 Resolve Runtime
  |
6 Plan Build
  |
7 Transform/Patch
  |
8 Pack Content
  |
9 Generate Android Project
  |
10 Compile
  |
11 Sign
  |
12 Validate
  |
13 Report
```

## 2. Stage outputs

### Stage 1 — Ingest

Output:

```text
SourceDescriptor
```

Checks:

- path exists;
- supported input type;
- no source mutation;
- symlink containment.

### Stage 2 — Index

Output:

```text
GameSourceIndex
```

Contains path metadata, size, selected hashes, and safe small-file previews.

### Stage 3 — Detect

Output:

```text
EngineDescriptor
```

Failure:

```text
DET-100 UNKNOWN
DET-110 AMBIGUOUS
```

### Stage 4 — Analyze

Output:

```text
CompatibilityReport
```

Static scanners and the pre-build asset audit run here. Every build runs the audit and displays its original findings and outcome.

### Pre-build audit policy

- The CLI build default is **informational**: `FAIL` and `INDETERMINATE` do not by themselves stop packaging and require no accept flag. The audit is not skipped or converted to `PASS`.
- `--strict-audit` restores strict outcome gating: `FAIL` stops with `BLD-104` unless `--accept-audit-fail` is supplied; `INDETERMINATE` stops unless `--accept-audit-unknown` is supplied. These overrides remain outcome-specific; neither accepts the other outcome. `PASS` and `WARNING` proceed under either policy.
- The manifest records `audit.policy` as `informational` or `strict`, alongside the original outcome and any actually applied overrides. Missing `audit.policy` in a legacy manifest means `strict`, not the new CLI default. Informational continuation must never fabricate an automatic override.
- Both policies retain all security, container-integrity (`CNT-*`), unsupported native-dependency (`CMP-102`), detection, runtime-resolution, and signature hard blockers. This includes hard-blocker findings present only in the audit report, not just compatibility analysis. A policy or accept flag cannot bypass them.
- Policy evaluation and pre-build hard-blocker checks occur before output directory creation, content packaging, Android generation, or Gradle execution. Later artifact/signature gates remain mandatory at their existing stages. Standalone `runtimepack audit` exit behavior is unchanged.

Proceeding with an audit risk does not repair missing assets or game logic, establish runtime compatibility, or prove gameplay. See `COMPATIBILITY.md` §11 and `CONFIG_AND_SCHEMAS.md` §§8–9.

### Stage 5 — Resolve runtime

Output:

```text
RuntimePlan
```

All runtime dependencies must already be installed/pinned or build stops.

### Stage 6 — Build plan

Output:

```text
build-plan.json
```

This is the final pre-side-effect decision record.

### Stage 7 — Transform

Uses temporary working directory only.

Examples:

- generated compatibility JS shim;
- generated RGSS runtime config;
- normalized metadata;
- overlay patches.

### Stage 8 — Content pack

Output:

```text
game.pack
```

or an asset tree for `plain`.

### Stage 9 — Android project generation

Template + runtime module + generated metadata.

No manual editing is expected.

### Stage 10 — Compile

Gradle is invoked with pinned wrapper/tool versions.

### Stage 11 — Sign

Modes:

```text
debug
keystore:<path>
```

Release build without an explicit key is rejected by default.

### Stage 12 — Validate

Minimum:

- APK exists;
- APK signature verifies;
- required ABI library exists;
- build manifest exists;
- no forbidden permission;
- content hash matches.

Optional device validation:

- install;
- launch;
- wait for runtime health marker.

### Stage 13 — Report

Generate:

```text
build-manifest.json
compatibility-report.json
checksums.txt
third-party-notices/
```

## 3. Build directories

```text
.runtimepack/
├── cache/
├── runtimes/
└── toolchains/

project-work/
├── ingest/
├── transformed/
├── content/
└── android/

dist/
```

Build work must not be placed inside the source game directory.

## 4. Incremental caching

Cache keys include:

```text
source content hash
project config hash
runtime version
container version
transform version
```

Never cache based on path name alone.

## 5. Atomic output

Write output to temporary path and atomically promote when validation succeeds.

Failed builds must not replace a previously good artifact.

## 6. Build cancellation

Long stages must be cancelable at safe boundaries.

Content packer and hashing loops should periodically check cancellation tokens.

## 7. Example CLI flow

```bash
runtimepack build ./Game --config runtimepack.toml
```

Equivalent internal sequence:

## 8. Build Workspace Ownership & Intermediate Cleanup

During build execution with Gradle compilation, large build-owned intermediates are created inside an exclusive per-run scratch directory: `<output>/.work_<run_id>/`:
- Staging directory (`.work_<run_id>/staging`)
- Extracted ASAR staging (`.work_<run_id>/staging_content`)
- Intermediate container (`.work_<run_id>/game.pack`)
- Android build project (`.work_<run_id>/android-project`)

### Cleanup Policy
1. **Default Auto-Cleanup:**
   Upon successful compilation, mandatory 5-stage artifact verification gate (streaming SHA-256, byte identity, bounded ZIP CRC, and `apksigner`), and optional probe execution, the owned intermediate workspace `<output>/.work_<run_id>/` is automatically and safely deleted.
2. **Opt-Out Flag (`--keep-build-files`):**
   ```bash
   runtimepack build ./Game --keep-build-files
   ```
   When `--keep-build-files` is provided, intermediate directories and files are preserved intact in `<output>/.work_<run_id>/`, and its path is printed in the summary.
3. **Failure & Error Retention:**
   If compilation fails, artifact verification fails, or `--probe` returns failure, the intermediate scratch directory is strictly retained for developer diagnosis.
4. **Project-Emission Modes:**
   In `--no-compile` mode or when `RUNTIMEPACK_SKIP_GRADLE` is active, deliverables (`game.pack` and `android-project`) are written directly to `<output>/` and cleanup is completely bypassed.
5. **Durable Artifacts & Checksums:**
   Durable deliverables (`<Game>.apk`, `build-manifest.json`, `compatibility-report.json`, `cleanup-summary.json`, `third-party-notices/`) remain in `<output>/`. `checksums.txt` hashes only surviving durable deliverables.

