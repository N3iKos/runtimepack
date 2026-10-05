# Runtime Contract v1

## 1. Goal

A runtime backend should be replaceable without changing the CLI core.

There are two contracts:

1. Host-side `RuntimePackage` contract.
2. Android-side `RuntimeHost` lifecycle contract.

## 2. Runtime package manifest

Required fields:

```toml
manifest_version = 1
id = "rgss-android"
version = "0.1.0"
runtime_api = 1
family = "rgss"

engines = [
  "rpgmaker-xp",
  "rpgmaker-vx",
  "rpgmaker-vxace"
]

abis = ["arm64-v8a"]

[android]
min_sdk = 26

[artifacts]
android_module = "runtime-rgss"

[licenses]
notice_bundle = "licenses/"
```

## 3. Capabilities

Capabilities are stable strings.

Initial registry:

```text
fs.vfs.read
fs.vfs.seek
fs.save.write

input.keyboard
input.pointer
input.gamepad
input.virtual_gamepad

graphics.surface
audio.standard
video.standard

rgss.win32.user32.keyboard
rgss.win32.kernel32.temp

web.local_origin
web.javascript
web.webgl
web.webaudio
web.node.path
web.node.fs_subset
web.nwjs_subset

renpy.runtime.versioned
renpy.python.pure

network.http
```

A game requirement must match a runtime or shim capability.

## 4. Build-time hooks

```rust
trait RuntimePackage {
    fn manifest(&self) -> &RuntimeManifest;

    fn validate_plan(
        &self,
        game: &GameDescriptor,
        compatibility: &CompatibilityReport
    ) -> Result<()>;

    fn prepare_content(
        &self,
        ctx: &mut ContentPrepareContext
    ) -> Result<()>;

    fn contribute_android(
        &self,
        project: &mut AndroidProject
    ) -> Result<()>;

    fn emit_runtime_config(
        &self,
        plan: &RuntimePlan
    ) -> Result<serde_json::Value>;
}
```

## 5. Android lifecycle contract

Backend state machine:

```text
NEW
 -> CREATED
 -> CONTENT_MOUNTED
 -> CONFIGURED
 -> SURFACE_ATTACHED
 -> RUNNING
 -> PAUSED
 -> RUNNING
 -> STOPPING
 -> STOPPED
```

Invalid transitions must fail with `RT-120_INVALID_STATE`.

## 6. Android host methods

Pseudo-interface:

```kotlin
interface RuntimeBackend {
    fun create(context: RuntimeContext)
    fun mountContent(provider: GameContentProvider)
    fun configure(config: RuntimeConfig)
    fun attachSurface(surface: RuntimeSurface)
    fun start(entryPoint: String)
    fun onInput(event: RuntimeInputEvent)
    fun pause()
    fun resume()
    fun stop()
}
```

## 7. RuntimeContext

Backend receives only scoped services:

```text
ContentService
SaveService
InputService
LogService
ClockService
NetworkService?  # only if permitted
```

Do not pass unrestricted application/global filesystem helpers.

## 8. Backend-specific adapters

### RGSS

Maps RuntimePack services into MKXP/RGSS runtime interfaces.

### RPG Maker Web

Maps content to a stable local origin and maps input/save services into web-compatible behavior.

### Ren'Py

Integrates upstream runtime expectations and maps Android lifecycle/save paths.

### Tyrano

Uses Web backend primitives but has a separate adapter for Tyrano-specific bootstrap and compatibility analysis.

## 9. ABI policy

v1 required ABI:

```text
arm64-v8a
```

Optional test ABI:

```text
x86_64
```

ARMv7 is deferred unless project requirements explicitly require it.

This is a RuntimePack v1 product decision; it is not a statement about JoiPlay. JoiPlay's older public Android MKXP build contains ARMv7, ARM64, and x86_64 splits [S05].

## 10. Backward compatibility

Runtime API v1 is frozen once the first backend is released.

Breaking host-backend changes require `runtime_api = 2`.

---

## 11. Runtime asset probe engine and validation schemas

RuntimePack host provides a controlled in-app asset probe engine (`AssetProbeRunner`) to verify asset availability and decodability on target Android devices.

### 11.1 Probe modes

- **`SAMPLED`**:
  - Deterministically selects up to N representative unique assets per category (`IMAGE`, `AUDIO`, `FONT`, `VIDEO`, `DATA`).
  - Runs representative assets through staged verification (`FILE_EXISTS` -> `FILE_READABLE` -> `INTEGRITY_VERIFIED` -> `DECODE_SUCCESS`).
  - Fast execution suitable for in-game diagnostic checks and quick validation.
- **`EXHAUSTIVE`**:
  - Queues all unique assets discovered from embedded `asset-dependency-graph.json` / `asset-audit-report.json` or extracted game filesystem.
  - Decodes images sequentially using memory boundaries (`BitmapFactory.decodeStream`, `Bitmap.recycle()`), never keeping multiple decoded bitmaps in memory.
  - Writes a crash checkpoint to `filesDir/runtime-state/audit/probe_checkpoint.json` every 50-100 items. If interrupted or killed, subsequent exhaustive probes resume from the checkpointed index.

### 11.2 Authoritative report artifacts and schemas

Upon probe completion, the host persists authoritative validation artifacts in `filesDir/runtime-state/audit/`:

1. **`runtime-validation-report.json`**:
   - Conforms to `schemas/runtime-validation-report.schema.json`.
   - Records probe mode, outcome (`PASS`, `WARNING`, `FAIL`, `INDETERMINATE`), total assets probed, pass/fail/warning counts, stage counts, detected fallbacks, checkpoint restoration status, and event trace.
2. **`combined-validation-summary.json`**:
   - Conforms to `schemas/combined-validation-summary.schema.json`.
   - Combines static audit report outcome (if available) with runtime probe outcome.
   - Preserves traceability between static results and runtime observations, recording explicit discrepancies.

### 11.3 Host bridge contract

Host activities (`RgssActivity`, `WebActivity`) expose standard JavaScript bridge methods on `RuntimePackVPad` and `RuntimePackWeb`:

```kotlin
@JavascriptInterface fun startAudit(requestJson: String): String
@JavascriptInterface fun getAuditEvents(afterSeq: Long): String
@JavascriptInterface fun getAuditReport(): String
@JavascriptInterface fun getCombinedSummary(): String
@JavascriptInterface fun cancelAudit(): Boolean
```

### 11.4 In-game overlay integration

The Diagnostic Suite overlay (`diagnostic_suite.js`) provides:
- Tab "Asset Audit & Probe" with `Run Sampled Probe`, `Run Exhaustive Probe`, and `Run Full Stress Test`.
- Real authentic live stage counters (`Scanned: X | Valid: Y | Missing: Z | Fallback: W`).
- Inclusion of `combined_validation_summary` in the exported Bug Report JSON payload.
