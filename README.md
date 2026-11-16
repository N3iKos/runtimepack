# RuntimePack

> **High-Performance Build-Time Compatibility Packager for Desktop & Web Games to Standalone Android Applications**

[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Android](https://img.shields.io/badge/Android-API%2026%2B%20(Target%2034)-green.svg)](https://developer.android.com/)
[![Architecture](https://img.shields.io/badge/Target-arm64--v8a-lightgrey.svg)]()

RuntimePack transforms desktop games created with RPG Maker, Ren'Py, and TyranoScript into native, standalone Android APKs and AAB packages without requiring Windows emulation, Wine/x86 translation layers, or multi-game launcher applications.

The resulting APK installs and launches directly like an official native Android title:
```text
Install Game.apk ──> Tap App Icon ──> Native Runtime Initializes ──> Game Plays Directly
```

---

## Key Features

- **Zero Windows Emulation**: Powered by native C++ runtimes (`mkxp-z` lineage + SDL2 + OpenAL + embedded Ruby VM for RGSS), modern sandboxed Chromium WebViews for HTML5/MV/MZ, and native Android backend for Ren'Py.
- **Strictly Immutable Source Games**: The user's original game directory is 100% read-only. All shims, asset overlays, and container packages are built in isolated temporary workspaces or virtual file systems.
- **GPACK/1 Binary Container & Asset Hardening**: Encrypted streaming game content packaging featuring AEAD ChaCha20-Poly1305 with SHA-256 integrity verification, preventing casual asset extraction without runtime degradation.
- **Deep Static Asset Auditor**: Scans game code, maps, databases, and script archives to identify missing assets, case-sensitivity discrepancies (Windows NTFS vs. Android Linux filesystem), and broken Run-Time Package (RTP) dependencies before compilation.
- **Adaptive Virtual Controller Suite**: Fully responsive touch overlay (`vpad.js` / `vpad.css`) supporting 5 distinct control presets, multiple visual themes (Glass, Matte, Retro OSD), dynamic aspect ratio scaling, and tactile haptic feedback.
- **Integrated Profile-Driven Cheat Engine**: Non-destructive in-game cheat overlay supporting variable/switch modification, speed multipliers, and currency editing.
- **Automated T1 Smoke Probe**: Direct integration with ADB and Android emulators (MuMu, Genymotion, or physical hardware) to verify cold boots, input injection, "New Game" selection, first-frame rendering, and memory stability.
- **Zero Broad Storage Permissions**: Sandboxed inside Android app-specific storage (`Context.filesDir`), eliminating the need for `MANAGE_EXTERNAL_STORAGE` or deprecated broad storage permissions.

---

## Supported Engine Matrix

| Engine | Detection Markers | Backend Runtime | Status |
| :--- | :--- | :--- | :---: |
| **RPG Maker XP** | `Game.ini`, `Data/Scripts.rxdata` | RGSS1 (`libruby.so`, SDL2, `libmkxp-z.so`) | Detection/audit: supported. APK build: prebuilt native libs required (see below) |
| **RPG Maker VX** | `Game.ini`, `Data/Scripts.rvdata` | RGSS2 (`libruby.so`, SDL2, `libmkxp-z.so`) | Detection/audit: supported. APK build: prebuilt native libs required (see below) |
| **RPG Maker VX Ace** | `Game.ini`, `Data/Scripts.rvdata2` | RGSS3 (`libruby.so`, SDL2, `libmkxp-z.so`, Auto-RTP) | Detection/audit: supported. APK build: prebuilt native libs required (see below) |
| **RPG Maker MV** | `index.html`, `js/rpg_core.js`, `data/System.json` | Web Runtime + Node.js/NW.js Shims | Supported |
| **RPG Maker MZ** | `index.html`, `js/rmmz_core.js`, `data/System.json` | Web Runtime + Pointer Normalization | Supported |
| **Ren'Py 8** | `renpy/` dir, `*.rpy`, `*.rpyc` archives | Native Ren'Py 8 Android bridge (`librenpython.so`) | Supported |
| **Ren'Py 7** | `renpy/` dir, `*.rpy`, `*.rpyc` archives | — | **Unavailable**: no verified native runtime in this tree; fails closed with `BLD-109` |
| **TyranoScript / TyranoBuilder** | `data/system/Config.tjs`, `tyrano/tyrano.js` | Web Runtime Adapter | Supported |

*Note: RPG Maker 2000/2003 and RPG Maker Unite are explicitly out of scope for v1.*

### RGSS native runtime status (`SOURCE_UNVERIFIED`)

The RGSS backend runs on **prebuilt** native libraries in
`runtimes/rgss/jni/arm64-v8a/`. RuntimePack's own host source is complete and
builds from this tree; the native engine binary is not.

- No engine source, build recipe or source offer for those binaries exists here.
  `Docs/REFERENCES.md` S52 records the binary hash and build id and states that
  matching buildable source is unverified.
- The runtime manifest declares `notice_bundle = "licenses/rgss"`, but no such
  notice bundle exists in this tree.
- Therefore RGSS **distribution** is BLOCKED: `pack-bundle` excludes
  `runtimes/rgss/**`, and an RGSS build from such a bundle stops with `BLD-109`.
- For local testing only, `runtimepack pack-bundle --local-diagnostic-native`
  includes the prebuilt libraries and stamps the bundle
  `LOCAL_DIAGNOSTIC_ONLY` / `NO_REMOTE_DISTRIBUTION` / `SOURCE_UNVERIFIED`.
  That does not establish licensing compliance and is not a source offer.
- Only `arm64-v8a` native libraries exist; advertised ABIs are derived from the
  ABI directories actually present, so no APK declares an ABI it cannot load.

---

## System Requirements & Prerequisites

### Host Environment
- **Operating System**: Windows 10/11 (64-bit), Linux (x86_64), or macOS (Apple Silicon/Intel).
- **Rust Toolchain**: Rust 1.75 or higher (Edition 2021) with `cargo`.
- **Java Development Kit (JDK)**: JDK 17 (OpenJDK or Oracle).
- **Android SDK**:
  - `compileSdkVersion`: **34**
  - `minSdkVersion`: **26** (Android 8.0 Oreo)
  - `targetSdkVersion`: **34** (Android 14)
  - Android SDK Build-Tools **34.0.0**
  - Android NDK **r26** or newer (with `arm64-v8a` target toolchain).
- **Android Debug Bridge (ADB)** *(Optional)*: Required for `runtimepack probe` smoke tests.

Set the standard environment variables before building:
```bash
# Windows PowerShell
$env:ANDROID_HOME = "C:\Users\<User>\AppData\Local\Android\Sdk"
$env:JAVA_HOME = "C:\Program Files\Eclipse Adoptium\jdk-17.0.x-hotspot"

# Linux / macOS
export ANDROID_HOME="$HOME/Android/Sdk"
export JAVA_HOME="/usr/lib/jvm/java-17-openjdk"
```

---

## Installation & Building from Source

Clone the repository and compile the CLI using Cargo:

```bash
git clone https://github.com/<owner>/runtimepack.git
cd runtimepack

# Build the release binary
cargo build --release

# The compiled binary will be located at:
# ./target/release/runtimepack (Linux/macOS)
# .\target\release\runtimepack.exe (Windows)
```

Add the target directory to your system `PATH` or invoke it directly.

---

## Quick Start Workflow

### 1. Verify Development Environment
Ensure your compiler, JDK, and Android SDK paths are correctly detected:
```bash
runtimepack doctor
```

### 2. Inspect & Explain Game Engine
Analyze an uncompiled game directory to determine engine type, confidence score, and compatibility:
```bash
# Fast static inspection
runtimepack inspect "path/to/game"

# Detailed evidence and rationale breakdown
runtimepack explain "path/to/game"
```

### 3. Audit Game Assets
Verify that all referenced audio, sprites, and data files exist, checking case-sensitivity and identifying missing Run-Time Package (RTP) dependencies:
```bash
runtimepack audit "path/to/game" --rtp-path "path/to/vxace_rtp" --mode deep
```
This generates an authoritative `asset-audit-report.json` and a rich interactive `asset-audit-report.html`.

### 4. Build Standalone Android APK
Package the game into an installable Android APK:
```bash
runtimepack build "path/to/game" \
  --package com.mycompany.mygame \
  --name "My Game Title" \
  --flavor release \
  --content-mode packed \
  --output ./dist
```

---

## CLI Reference

### `runtimepack doctor`
Runs a host diagnostics check on Rust, Cargo, Java JDK, Android SDK/NDK, and connected ADB devices.

### `runtimepack inspect <PATH> [--config <CONFIG>]`
Statically inspects the game folder without executing game code.
- `<PATH>`: Path to the game root directory.
- `-c, --config <FILE>`: Path to custom `AppConfig` or `runtimepack.toml`.

### `runtimepack explain <PATH>`
Outputs deterministic detection evidence, detected engine profiles, confidence ratings (`EXACT`, `HIGH`, `MEDIUM`), and compatibility rationale.

### `runtimepack audit <PATH> [OPTIONS]`
Performs non-destructive static asset analysis:
- `--rtp-path <DIR>`: Path to external Run-Time Package assets to resolve missing standard engine assets.
- `--mode <standard|deep>`: Audit depth. `deep` performs recursive script/data parsing (default: `standard`).
- `--output <FILE>`: Path for output JSON report (default: `asset-audit-report.json`).
- `--no-html`: Skip generating the interactive standalone HTML report viewer.
- `--allow-network-audit`: Allow bounded checks for external network URLs.

### `runtimepack build <PATH> [OPTIONS]`
Orchestrates project generation, VFS containerization, and Gradle compilation:
- `--package <ID>`: Android application package ID (e.g. `com.studio.adventure`). Auto-generated if omitted.
- `--name <NAME>`: Display name shown on the Android home screen.
- `--flavor <beta|release>`:
  - `beta` *(default)*: Includes diagnostic suite, real-time FPS counter, and transparent community watermark.
  - `release`: Production build with clean overlay and stripped diagnostics.
- `--content-mode <plain|packed|encrypted-streaming>`:
  - `packed` *(default)*: Bundles assets into standard GPACK/1 binary container.
  - `encrypted-streaming`: Enables AEAD ChaCha20-Poly1305 encryption for asset protection.
  - `plain`: Loose files copied to app assets (for lightweight debugging).
- `--rtp-path <DIR>`: Path to engine RTP directory to automatically bundle required system assets.
- `--themes <LIST>`: Comma-separated list of Virtual Gamepad themes to bundle (e.g. `matte,minimalist_glass,retro_osd`).
- `--default-theme <ID>`: Active theme ID upon first boot (default: `matte`).
- `--theme-catalog <DIR>`: Directory containing custom theme packages (default: `runtimes/themes`).
- `--no-compile`: Generates the Android project and bundled assets without invoking Gradle.
- `--probe`: Automatically executes a T1 smoke test via ADB after successful compilation.
- `--probe-device <ADDR>`: Specific ADB target address or profile (e.g. `127.0.0.1:16384`).
- `-o, --output <DIR>`: Target directory for artifacts (default: `./dist`).

### `runtimepack probe [OPTIONS]`
Performs automated smoke tests on a built APK using ADB:
- `--apk <FILE>`: Path to the target APK.
- `--device <ADDR>`: Device address or configured profile.
- `--dpad-down <N>`: Number of Down keypresses before Enter to navigate the title menu (default: `0`).
- `--report <FILE>`: Path for the probe report JSON (default: `dist/probe-report.json`).
- `--list-devices`: List all connected ADB devices and configured profiles.

### `runtimepack theme`
- `runtimepack theme list [--catalog <DIR>]`: Inspect discovered themes in catalog.
- `runtimepack theme validate <PATH> [--api-version <VERSION>]`: Validate theme structure, CSS, and manifest.

### `runtimepack runtime list`
Lists all registered Android runtime backends and native libraries.

### `runtimepack licenses`
Displays third-party notices, license compliance data, and upstream credits.

---

## Architecture & Subsystems

```text
Host Workspace (Rust CLI & Core)
┌────────────────────────────────────────────────────────┐
│  runtimepack CLI                                      │
│  ├─ GameSourceIndex (Immutable source game scanner)   │
│  ├─ DetectorRegistry (Static multi-engine detectors)  │
│  ├─ AssetAuditorRegistry (RTP & path normalization)   │
│  ├─ BuildPlanner (Android project generator)          │
│  └─ GpackPacker (Streaming AEAD ChaCha20-Poly1305)    │
└────────────────────────────────────────────────────────┘
                          │ Emits
                          ▼
Android Application Project (Kotlin + Native C++)
┌────────────────────────────────────────────────────────┐
│  BootstrapActivity                                    │
│  ├─ Native VFS & GPACK Stream Reader (libruntimepack)  │
│  ├─ Engine Runtime Backend:                           │
│  │   • RGSS: SDLActivity + mkxp-z + libruby.so        │
│  │   • Web: Hardware-accelerated Chromium WebView     │
│  │   • Ren'Py: Python-Android StoreInterface bridge   │
│  └─ Transparent Touch Surface (Overlay):              │
│      • vpad.js: Virtual D-Pad, Buttons & Toolbar      │
│      • cheat_engine.js: In-game variable modifier     │
│      • diagnostic_suite.js: Realtime FPS & OOM guard  │
└────────────────────────────────────────────────────────┘
```

### 1. Host Toolchain (`cli/`, `core/`, `pack/`, `engines/`)
Written entirely in Rust for performance, memory safety, and cross-platform reliability:
- `runtimepack_core`: Engine-agnostic abstractions, diagnostic taxonomies (`ING-xxx`, `DET-xxx`, `CMP-xxx`, `BLD-xxx`), build pipelines, and theme catalog validators.
- `runtimepack_pack`: Binary specification and streaming parser for `GPACK/1` containers.
- `runtimepack_engine_*`: Decoupled engine implementations containing AST parsers, regex scanners, and compatibility analyzers.

### 2. Android Shell (`android/shell/`)
A modular Android host application written in Kotlin:
- **`io.runtimepack.host.BootstrapActivity`**: Primary lifecycle coordinator initializing audio devices, display surfaces, and mounting the asset container into app-specific storage.
- **Virtual Pad Overlay (`android/shell/app/src/main/assets/overlay/`)**: Lightweight, hardware-accelerated transparent HTML5/CSS3/JS overlay layer communicating via a bi-directional JavascriptInterface bridge.
- **Compatibility Shims (`android/shell/app/src/main/assets/web_shims/`)**: Polyfills Node.js (`fs`, `path`, `os`, `process`) and NW.js window management APIs for browser-based RPG Maker MV/MZ and TyranoScript games.

---

## Directory Structure

```text
RPGM-P/
├── Cargo.toml               # Cargo workspace configuration
├── toolchain.lock           # Pinned toolchain and dependency versions
├── runtimepack.toml         # Base project configuration
├── cli/                     # CLI binary implementation (main, audit, probe)
├── core/                    # Core domain logic, detection, packaging, themes
├── engines/                 # Engine-specific adapters
│   ├── rgss/                # RPG Maker XP, VX, and VX Ace support
│   ├── rpgmaker_web/        # RPG Maker MV and MZ support
│   ├── renpy/               # Ren'Py script and archive scanner
│   └── tyrano/              # TyranoScript and TyranoBuilder adapter
├── pack/                    # GPACK/1 binary container format implementation
├── android/shell/           # Android project template (Kotlin, Gradle, jniLibs)
├── compatibility-db/        # Declarative compatibility and Win32API shim database
├── probe-profiles/          # Target device configurations for automated smoke tests
├── preview/                 # Virtual controller preview and test harness
├── runtimes/                # Runtime package manifests and prebuilt native binaries
├── schemas/                 # JSON Schemas for configuration and build manifests
└── tests/                   # Integration tests and fixture corpus
```

---

## Testing & Quality Assurance

The repository adheres to strict automated testing standards:

```bash
# Run all host Rust unit and integration tests
cargo test --workspace

# Run specific engine detection tests
cargo test --test detection_test

# Run GPACK/1 container crypto and packaging tests
cargo test -p runtimepack_pack

# Run Android Shell unit tests via Gradle
cd android/shell
./gradlew testReleaseUnitTest
```

---

## Platform Paths, Windows Distribution and Cross-Build

All paths below are relative to the repository root. Nothing in the source tree
requires a specific drive letter, WSL mount point, or user profile path; the CLI
resolves its resource root from the executable location (or
`RUNTIMEPACK_RESOURCE_ROOT` when set explicitly).

### Path conventions

| Context | Repository root | Built CLI | Python wizard |
| :--- | :--- | :--- | :--- |
| Windows (PowerShell / cmd) | `D:\path\to\runtimepack` | `target\release\runtimepack.exe` | `wizard.py` + `RuntimePack-Wizard.bat` |
| Linux / WSL / macOS | `/path/to/runtimepack` | `target/release/runtimepack` | `wizard.py` (direct `python3`) |

### Windows portable bundle (end users)

A packaged bundle is self-contained and needs only Python 3 for the interactive
wizard (the CLI itself is a static PE binary and needs no runtime):

```text
<bundle>\
├── RuntimePack-Wizard.bat   # launcher shim, delegates to wizard.py next to it
├── wizard.py                # interactive menu; resolves bin\runtimepack.exe itself
├── bin\runtimepack.exe      # native Windows CLI
├── android\ ...             # Android project template + Ruby/JS runtime assets
├── runtimes\ ...            # runtime manifests, themes, vendored native libraries
├── schemas\ ...             # JSON schemas
├── toolchain.lock           # pinned Gradle/AGP/JDK/SDK versions
├── ASSETS_CHECKSUM_MANIFEST.json
└── checksums.txt
```

Run it by double-clicking `RuntimePack-Wizard.bat` or:

```powershell
.\RuntimePack-Wizard.bat
```

The wizard writes each build to `<bundle>\dist\run_<n>\` (a fresh numbered
directory, never overwriting an earlier run). Set `RUNTIMEPACK_EXE` to point at a
different CLI binary; the wizard strips surrounding quotes from drag-and-dropped
paths.

### Rebuilding the Windows CLI (cross-compile from WSL)

The pinned target is `x86_64-pc-windows-gnullvm` with a static CRT:

```bash
# llvm-mingw must be on PATH (pinned release, sha256-verified archive)
export PATH="$HOME/.local/toolchains/llvm-mingw-20261006-ucrt/bin:$PATH"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER="x86_64-w64-mingw32-clang"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_AR="llvm-ar"

rustup target add x86_64-pc-windows-gnullvm
cargo build --release --target x86_64-pc-windows-gnullvm -p runtimepack

# Result: target/x86_64-pc-windows-gnullvm/release/runtimepack.exe
```

Native Windows builds are also supported where a Rust MSVC/GNU toolchain is
installed: `cargo build --release` produces `target\release\runtimepack.exe`.

### Packaging a portable bundle

One command builds a complete, verified bundle (payload, launcher files,
provenance, checksums, self-check):

```bash
python3 scripts/package_windows_bundle.py \
  --output ../dist_runtimepack_<label> \
  --bin target/release/runtimepack \
  --source-root . \
  --commit "$(git rev-parse HEAD)" \
  --branch <branch> \
  --worktree <worktree>
```

`--source-root` is the checkout that produced the payload: launcher files are
copied from it and `--commit` is verified against its `HEAD` (mismatch →
`BLD-111`, nothing written). `--toolchain` and `--build-env` default to
`unspecified` and are recorded as operator-supplied claims, never guessed.

Add `--local-diagnostic-native` for a local-only RGSS testing bundle (see
"RGSS native runtime status" above).

`pack-bundle` alone copies only the positive allowlist (Android template,
runtime manifests/themes/licenses, schemas, `toolchain.lock`,
`Docs/LICENSE_COMPLIANCE.md`) and **refuses a non-empty output directory before
writing anything**. `wizard.py`, `RuntimePack-Wizard.bat` and
`README_WINDOWS.md` are launcher files outside that allowlist, which is why the
one-command script exists instead of a manual copy step.

RGSS native libraries (`runtimes/rgss/jni/**`) are excluded from distribution
bundles because their buildable source is unverified
(`Docs/LICENSE_COMPLIANCE.md` §3, `Docs/REFERENCES.md` S52); Ren'Py native
(`runtimes/renpy/jni/**`) is allowlisted and shipped.

### Android APK builds

APK compilation requires a JDK 17 and an Android SDK (compileSdk 34,
build-tools 34.0.0). The CLI resolves the SDK from `ANDROID_HOME`,
`ANDROID_SDK_ROOT`, the generated project's `local.properties` `sdk.dir`, or the
conventional install path — one resolution, shared by the preflight, the Gradle
child process and the generated project.

Validation is strict and **offline-first**: RuntimePack never auto-downloads SDK
components, so a component that AGP would fetch on demand must be installed
explicitly (`sdkmanager "platforms;android-34" "build-tools;34.0.0"`). The
required versions are read from `toolchain.lock` when readable, and the message
says which source was used. Without a valid toolchain the CLI stops with
`BLD-101` (`BLD_TOOLCHAIN_MISSING`) and the wizard reports the same failure;
host-project generation still works:

```bash
runtimepack build <game> --flavor beta --no-compile --output <fresh-dir>
```

---

## Contributing & Development Workflow

1. **Immutable Source Guarantee**: Never write logic that mutates a user's source game directory in-place.
2. **Deterministic Errors**: All error conditions must yield stable, classified diagnostic codes (`ING-`, `DET-`, `CMP-`, `BLD-`, etc.).
3. **No Unpinned Dependencies**: All third-party dependencies and toolchain versions must be documented in `toolchain.lock`.
4. **Isolated Commits**: Keep changes focused and test-driven. Ensure `cargo test` and `./gradlew test` pass prior to submitting pull requests.

---

## License & Upstream Provenance

RuntimePack is distributed under the **MIT OR Apache-2.0** dual license.

Third-party dependencies and upstream components:
- **mkxp-z**: GPL v2 or later. **Distribution status: BLOCKED (`SOURCE_UNVERIFIED`).** The RGSS backend ships as prebuilt binaries only; no matching buildable source, source offer or redistributable notice bundle exists in this tree (`Docs/REFERENCES.md` S52). This is not a compliance claim. See "RGSS native runtime status" above.
- **Ruby**: Ruby License / 2-Clause BSDL.
- **SDL2 & OpenAL Soft**: zlib / LGPL v2+.
- **Ren'Py Engine**: MIT License.

Execute `runtimepack licenses` for detailed attribution, notices, and source provenance.
