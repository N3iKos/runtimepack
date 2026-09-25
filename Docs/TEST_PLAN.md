# Test Plan v1

## 1. Test philosophy

The unit of quality is not "APK compiled." It is:

```text
correct detection
+ correct runtime resolution
+ boot
+ first frame
+ input
+ audio
+ save/load
+ lifecycle
+ reproducible metadata
```

## 2. Fixture policy

Use:

- project-owned minimal fixtures;
- upstream sample projects whose licenses explicitly allow use;
- synthetic files for detection.

Do not commit commercial games or copyrighted third-party game assets as test fixtures.

## 3. Detector fixtures

Required:

```text
xp_min/
vx_min/
vxace_min/
mv_min/
mz_min/
renpy_supported_version_A/
renpy_supported_version_B/
tyrano_min/
generic_html_negative/
ambiguous_mv_mz/
invalid_rgss_game_ini/
```

## 4. Detector tests

Examples:

```text
DET-XP-001:
  input xp_min
  expect RPG_MAKER_XP / EXACT

DET-HTML-NEG-001:
  index.html only
  expect UNKNOWN

DET-AMB-001:
  both rpg_core.js and rmmz_core.js
  expect AMBIGUOUS
```

## 5. Build unit tests

- project generation snapshots;
- manifest generation;
- permission allowlist;
- path normalization;
- runtime lock resolution;
- signing configuration validation.

## 6. Container tests

- round-trip pack/read;
- random seek;
- corrupted index rejection;
- corrupted chunk rejection;
- path traversal rejection;
- wrong key rejection;
- zero-byte files;
- large files;
- Unicode filenames;
- case-collision behavior.

## 7. Android instrumentation tests

For every runtime family:

```text
launch
FIRST_FRAME marker
input event accepted
pause
resume
save
process kill
relaunch
load
```

## 8. Device/API matrix

Minimum CI categories:

- current Android emulator API;
- one older supported API;
- x86_64 emulator if backend supports it;
- physical ARM64 device in release qualification.

Do not hardcode "current" SDK in tests. Toolchain lock defines tested values.

## 9. RGSS compatibility tests

Synthetic scripts for:

- bitmap;
- sprite;
- audio;
- keyboard;
- save/load;
- supported Win32 shim;
- unsupported Win32 call;
- native DLL detection.

## 10. MV/MZ tests

- canvas/WebGL start;
- WebAudio;
- local asset load;
- JS plugin;
- supported compatibility shim;
- unsupported `.node` module detection;
- save persistence;
- stable local origin.

## 11. Ren'Py tests

At least two pinned runtime generations.

Tests:

- basic dialogue;
- image;
- audio;
- menu;
- save/load;
- rollback if supported by selected runtime;
- unsupported native extension detection.

## 12. Tyrano tests

Before legal gate clearance, testing may use developer-owned/synthetic structures without redistributing upstream engine files in release artifacts.

Tests:

- detection;
- adapter bootstrap using permitted fixture strategy;
- scenario navigation;
- assets;
- save if supported by approved model.

## 13. Performance tests

Record:

- cold startup to runtime start;
- runtime start to first frame;
- peak memory;
- pack read throughput;
- random VFS lookup latency;
- build time;
- temp disk amplification.

No fixed performance SLA is imposed before baseline measurement.

## 14. Regression policy

Every fixed compatibility bug gets a regression fixture if legally possible.

Issue closure requires:

```text
bug
 -> fixture
 -> failing test
 -> fix
 -> passing test
```

## 15. Release acceptance

No P0 release if:

- detector has unresolved false positive on fixture corpus;
- release APK asks for forbidden broad storage permission;
- save is lost across restart;
- unsigned APK emitted as release success;
- license notices missing;
- Tyrano legal gate bypassed.

## 16. Asset audit tests (static and deep)

### 16.1 Authoritative terminology and domain tests

- **Monotonic stage sequence:**
  - Verify that evidence stages progress strictly monotonically (`REFERENCE_FOUND` -> `PATH_RESOLVED` -> `FILE_EXISTS` -> `FILE_READABLE` -> `INTEGRITY_VERIFIED` -> `DECODE_SUCCESS` -> `ENGINE_LOAD_SUCCESS` -> `RUNTIME_USE_SUCCESS`).
  - Verify that deserialization and validation reject evidence chains that skip intermediate stages (e.g. `ReferenceFound` directly to `DecodeSuccess`).
- **Fail-closed outcome recomputation:**
  - Table-driven tests verifying that any blocking finding on a required asset yields `FAIL`.
  - Incomplete required scan scope or missing adapter yields `INDETERMINATE`.
  - Non-blocking/optional issues yield `WARNING`.
  - Fully verified declared scope yields `PASS`.
- **Authoritative PASS definition:**
  - Tests assert that `PASS` certifies **only that declared static scope passed**. Undeclared dynamic assets or unanalyzable expressions must yield `INDETERMINATE` and cannot be reported as `PASS`.
- **Orphan asset classification:**
  - Verify that unreferenced assets on disk are classified as `INFO | POSSIBLY_UNUSED` (`CMP-AUD-011`), never escalate outcome, and never block builds.
- **Canonical serialization:**
  - Verify that arbitrary insertion order of findings, nodes, and edges canonicalizes to byte-identical JSON.

### 16.2 CLI isolation and source immutability tests

- **Source immutability:**
  - Verify that `runtimepack audit` leaves the source directory 100% untouched (SHA-256 hashes, file counts, and timestamps unchanged).
- **Source output rejection:**
  - Verify that specifying `--output` inside the source directory triggers stable diagnostic `CMP-AUD-009` and creates no files.
- **Atomic file replacement:**
  - Verify that report emission writes to a sibling temporary file first and replaces the target atomically.
  - Verify that failed replacement cleans temporary files and preserves any existing report.
- **Concurrency safety:**
  - Verify that parallel audit runs on the same output path result in a single valid report with zero temporary file leaks.
- **Process isolation:**
  - Verify statically and via tests that the audit execution module has no process launch capability (`Command::new` or shell execution).

### 16.3 Build policy gate and override tests

- **Override matrix testing:**
  - `PASS` / `WARNING` allow build to proceed.
  - `FAIL` blocks build with `BLD-104`; allowed only when `--accept-audit-fail` is supplied; blocked when `--accept-audit-unknown` is supplied.
  - `INDETERMINATE` blocks build with `BLD-104`; allowed only when `--accept-audit-unknown` is supplied; blocked when `--accept-audit-fail` is supplied.
- **Hard blocker non-bypassability:**
  - Verify that neither `--accept-audit-fail` nor `--accept-audit-unknown` bypasses `CMP-102` (native Windows DLL), `CNT-*` (container corruption), `TYR-101` (Tyrano legal gate), ambiguous engine detection, or unresolved runtime versions.
- **Pre-build side-effect gating:**
  - Verify that un-overridden audit failures abort the build before creating the output directory, packing content, generating Android projects, or running Gradle.
- **Static and runtime separation:**
  - Verify that static audit reports remain decoupled from runtime probe reports. In-game telemetry indicators (Diagnostic Suite) are tested as viewers only and cannot satisfy static verification gates.

### 16.4 Runtime probe and checkpoint tests

- **Sampled probe mode:**
  - Verifies deterministic selection of representative unique assets across all categories (image, audio, font, video, data).
  - Verifies that repeated runs select identical assets in deterministic sorted order.
- **Exhaustive probe mode & memory safety:**
  - Queues all unique assets and processes them sequentially.
  - Enforces memory boundaries: verifies bitmaps are decoded via `BitmapFactory.decodeStream` and immediately freed via `Bitmap.recycle()`.
  - Verifies zero retention of decoded image buffers across iterations.
- **Checkpoint persistence & crash resilience:**
  - Verifies that checkpoints are persisted to `filesDir/runtime-state/audit/probe_checkpoint.json` every 50-100 items.
  - Verifies resume capability: when a valid checkpoint exists, the probe resumes from the checkpoint index without re-evaluating completed assets.
- **Cancellation & timeout bounds:**
  - Verifies that calling `cancel()` cleanly terminates execution and sets outcome to `INDETERMINATE`.
  - Verifies that exceeding the configured timeout cleanly terminates execution and records `INDETERMINATE`.
- **Validation schemas & report generation:**
  - Verifies generation and atomic persistence of `runtime-validation-report.json` conforming to `schemas/runtime-validation-report.schema.json`.
  - Verifies generation and atomic persistence of `combined-validation-summary.json` conforming to `schemas/combined-validation-summary.schema.json`.
- **Diagnostic Suite overlay integration:**
  - Verifies renamed tab "Asset Audit & Probe" and triggers for `Run Sampled Probe` and `Run Exhaustive Probe`.
  - Verifies real authentic live counters for Scanned, Valid, Missing, and Fallback stages.
  - Verifies inclusion of `combined_validation_summary` in the exported Bug Report payload.

