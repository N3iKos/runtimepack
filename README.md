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
| **RPG Maker XP** | `Game.ini`, `Data/Scripts.rxdata` | RGSS1 (`libruby.so`, SDL2, `libmkxp-z.so`) | Supported |
| **RPG Maker VX** | `Game.ini`, `Data/Scripts.rvdata` | RGSS2 (`libruby.so`, SDL2, `libmkxp-z.so`) | Supported |
| **RPG Maker VX Ace** | `Game.ini`, `Data/Scripts.rvdata2` | RGSS3 (`libruby.so`, SDL2, `libmkxp-z.so`, Auto-RTP) | Supported |
| **RPG Maker MV** | `index.html`, `js/rpg_core.js`, `data/System.json` | Web Runtime + Node.js/NW.js Shims | Supported |
| **RPG Maker MZ** | `index.html`, `js/rmmz_core.js`, `data/System.json` | Web Runtime + Pointer Normalization | Supported |
| **Ren'Py** | `renpy/` dir, `*.rpy`, `*.rpyc` archives | Native Ren'Py Android Bridge | Supported |
| **TyranoScript / TyranoBuilder** | `data/system/Config.tjs`, `tyrano/tyrano.js` | Web Runtime Adapter | Supported |

*Note: RPG Maker 2000/2003 and RPG Maker Unite are explicitly out of scope for v1.*

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

## Contributing & Development Workflow

1. **Immutable Source Guarantee**: Never write logic that mutates a user's source game directory in-place.
2. **Deterministic Errors**: All error conditions must yield stable, classified diagnostic codes (`ING-`, `DET-`, `CMP-`, `BLD-`, etc.).
3. **No Unpinned Dependencies**: All third-party dependencies and toolchain versions must be documented in `toolchain.lock`.
4. **Isolated Commits**: Keep changes focused and test-driven. Ensure `cargo test` and `./gradlew test` pass prior to submitting pull requests.

---

## License & Upstream Provenance

RuntimePack is distributed under the **MIT OR Apache-2.0** dual license.

Third-party dependencies and upstream components:
- **mkxp-z**: GPL v2 or later (used under compliance for native RGSS bytecode execution).
- **Ruby**: Ruby License / 2-Clause BSDL.
- **SDL2 & OpenAL Soft**: zlib / LGPL v2+.
- **Ren'Py Engine**: MIT License.

Execute `runtimepack licenses` for detailed attribution, notices, and source provenance.
