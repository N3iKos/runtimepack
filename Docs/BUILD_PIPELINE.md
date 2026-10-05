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

Static scanners run here.

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

```text
inspect
resolve
plan
execute
validate
report
```
