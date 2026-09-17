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
