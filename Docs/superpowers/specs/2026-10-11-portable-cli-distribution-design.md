# Technical Design Specification: Portable CLI Distribution & Resource Root Architecture

- **Document ID:** SPEC-2026-10-11-PORTABLE-CLI-DISTRIBUTION
- **Status:** APPROVED DESIGN SPECIFICATION (READY FOR IMPLEMENTATION HANDOFF)
- **Authoritative Worktree:** `.worktrees/fix-mvmz-logical-root` (Commit `10e2a92` / Branch `fix/mvmz-logical-root-20261009`)
- **Canonical Requirement IDs:** `PRD-001`, `PRD-002`, `PRD-003`, `PRD-010`, `TECH-CORE-001`, `TECH-CORE-002`, `TECH-CORE-004`, `TECH-AND-001`
- **Canonical References:** `S01`, `S02`, `S13`, `S14`, `S15`, `S17`, `S22`, `S29`, `S46`
- **Specification IDs:** `PORTDIST-001` through `PORTDIST-025`

---

## 1. Executive Summary & Problem Analysis

### 1.1 Context & Strategic Objective
RuntimePack is a build-time compatibility packager that detects game engines, analyzes compatibility, selects backends, and emits standalone Android APKs. While the core engine discovery and analysis pipelines across all 7 canonical engines (RPG Maker XP, VX, VX Ace, MV, MZ, Ren'Py, Tyrano) have been stabilized and verified across 814 automated checks (574 Rust + 12 JS + 228 JVM tests), the host CLI deployment mechanism currently relies on workstation source checkout assumptions.

This specification establishes an immutable packaging contract for **fully portable, relocatable, and source-independent CLI distributions** on both Linux (`x86_64-unknown-linux-gnu`) and Windows (`x86_64-pc-windows-gnu` / `x86_64-pc-windows-gnullvm`).

### 1.2 Audit of Current Resource Path Deficiencies
An audit of `.worktrees/fix-mvmz-logical-root` reveals critical portability and security deficiencies in how runtime templates and assets are resolved:

1. **Unsafe CWD Fallback (`generator.rs:437-444`, `164-175`):**
   `AndroidProjectGenerator::find_template_file(rel_path)` tests `Path::new(rel_path)` and `Path::new("..").join(rel_path)` relative to the process's Current Working Directory (CWD). If an end-user executes `runtimepack.exe` while their terminal is inside an untrusted game directory (`C:\Games\SubjectGame`), the compiler searches for internal shell templates (`android/shell`) inside the untrusted game source. This exposes the build pipeline to arbitrary template spoofing.

2. **Hardcoded Compile-Time Metadata (`CARGO_MANIFEST_DIR`):**
   `generator.rs:176`, `generator.rs:445`, and `core/src/asset_audit/mod.rs:999` fall back to `std::env::var("CARGO_MANIFEST_DIR")`. In a distributed standalone binary, this variable is either unset or (if embedded via macros) points to the compilation host's local path (e.g., `/home/mhsymr/...`).

3. **Silent Failure & Missing Asset Swallowing (`generator.rs:891`, `975`):**
   When `find_template_file("android/shell/app/src/main/java")` fails to locate the Android shell host sources, the generator silently skips copying the Java/Kotlin sources. Downstream Gradle compilation subsequently crashes with obscure syntax errors rather than failing early and cleanly with a stable diagnostic error code (`BLD-102` / `RT-101`).

4. **Ad-Hoc Environmental Overrides (`RUNTIMEPACK_THEMES_PATH`):**
   Theme catalogs support `RUNTIMEPACK_THEMES_PATH` (`generator.rs:468`), but cheat profiles (`profiles/games`), shell templates (`android/shell`), schemas (`schemas/`), and runtime manifests (`runtimes/manifests`) lack a unified, declarative root configuration.

5. **Insecure Denylist Packaging Risk:**
   Naively copying repository directories into distribution archives risks bundling uncommitted user work, Gradle build caches (`android/shell/app/build/`), developer SDK configs (`local.properties`), and private keys. A positive allowlist manifest is mandatory.

---

## 2. Universal Bundle Directory Layout

Both Linux and Windows standalone distributions adhere to an identical, relocatable bundle hierarchy (`PORTDIST-001`):

```text
runtimepack-v0.1.0-<platform>/
├── bin/
│   └── runtimepack (ELF on Linux) / runtimepack.exe (PE on Windows)
├── android/
│   └── shell/
│       ├── build.gradle.kts
│       ├── settings.gradle.kts
│       ├── gradle.properties
│       ├── gradlew
│       ├── gradlew.bat
│       ├── gradle/wrapper/
│       │   ├── gradle-wrapper.jar
│       │   └── gradle-wrapper.properties
│       └── app/
│           ├── build.gradle.kts
│           ├── proguard-rules.pro
│           └── src/main/
│               ├── AndroidManifest.xml
│               ├── java/io/runtimepack/host/...
│               ├── res/mipmap-*/...
│               └── assets/
│                   ├── overlay/
│                   │   ├── index.html
│                   │   ├── vpad.js
│                   │   ├── vpad.css
│                   │   ├── theme_loader.js
│                   │   ├── diagnostic_suite.js
│                   │   ├── cheat_engine.js
│                   │   ├── cheat_menu.js
│                   │   └── cheat_profile.js
│                   ├── web_shims/
│                   │   ├── runtimepack_web_shims.js
│                   │   ├── runtimepack_vnm_input_guard.js
│                   │   └── runtimepack_asset_audit.js
│                   ├── fonts/
│                   ├── runtimepack_bootstrap.rb
│                   ├── runtimepack_asset_audit.rb
│                   └── win32api_shim.rb
├── runtimes/
│   ├── manifests/
│   │   ├── web-rpgmaker.toml
│   │   ├── vnmaker-web.toml
│   │   ├── tyrano-web.toml
│   │   ├── rgss-android.toml
│   │   ├── renpy-android-7.toml
│   │   └── renpy-android-8.toml
│   ├── themes/
│   │   ├── liquid_glass/
│   │   ├── matte/
│   │   └── retro_osd/
│   ├── renpy/
│   │   ├── manifest.json
│   │   ├── licenses/
│   │   └── jni/arm64-v8a/librenpython.so
│   ├── rgss/
│   │   └── jni/arm64-v8a/*.so
│   └── vnmaker/
│       └── licenses/
├── profiles/
│   └── games/
│       ├── termina/cheats.js
│       └── lona/cheats.js
├── compatibility-db/
│   ├── *.toml
│   └── profiles/
│       ├── rpgmaker-mz-shiroin-saveui-disable-childproc.json
│       ├── rpgmaker-mz-desktop-subproc-disable.json
│       └── rpgmaker-mz-desktop-subproc-external-url.json
├── schemas/
│   ├── build-manifest.schema.json
│   ├── asset-audit-report.schema.json
│   ├── combined-validation-summary.schema.json
│   ├── compatibility-report.schema.json
│   ├── runtime-validation-report.schema.json
│   ├── theme-package.schema.json
│   ├── toolchain-lock.schema.json
│   ├── runtime-manifest.schema.json
│   └── runtimepack-config.schema.json
├── toolchain.lock
├── LICENSE
├── NOTICE.txt
└── checksums.txt (SHA-256 digest of all files in bundle)
```

---

## 3. Normative Resource Root Resolution Contract

### 3.1 Three-Tier Resolution Order (`PORTDIST-002`)
The runtime resource root MUST be determined using a strict three-tier precedence model without CWD probing:

```text
┌────────────────────────────────────────────────────────┐
│ 1. Explicit Declarative Environment Variable           │
│    RUNTIMEPACK_RESOURCE_ROOT                           │
│    (Validated for required markers; fails closed)      │
└───────────────────────────┬────────────────────────────┘
                            │ (Unset)
                            ▼
┌────────────────────────────────────────────────────────┐
│ 2. Executable-Relative Bundle Root                     │
│    current_exe().parent().parent() [for bin/ layout]   │
│    current_exe().parent()          [for flat layout]   │
│    (Validated for required markers)                    │
└───────────────────────────┬────────────────────────────┘
                            │ (Markers missing)
                            ▼
┌────────────────────────────────────────────────────────┐
│ 3. Development Source Root (Dev Only)                  │
│    CARGO_MANIFEST_DIR / workspace parent               │
│    (Allowed ONLY if Cargo.toml exists at resolved dir) │
└───────────────────────────┬────────────────────────────┘
                            │ (Not in dev workspace)
                            ▼
┌────────────────────────────────────────────────────────┐
│ TERMINAL FAILURE: [BLD-102] RESOURCE_ROOT_NOT_FOUND    │
│ (Zero fallback to CWD; immediate error exit)           │
└────────────────────────────────────────────────────────┘
```

### 3.2 Marker Validation Requirements (`PORTDIST-003`)
A directory is recognized as a valid `ResourceRoot` ONLY if all of the following critical marker paths exist:
1. `android/shell/build.gradle.kts`
2. `runtimes/manifests/web-rpgmaker.toml`
3. `schemas/build-manifest.schema.json`
4. `toolchain.lock`

If `RUNTIMEPACK_RESOURCE_ROOT` is explicitly set by the user but fails marker validation, resolution MUST return `DiagnosticError::from_code("BLD-102", "RUNTIMEPACK_RESOURCE_ROOT does not contain valid RuntimePack assets")` and MUST NOT fall back to Tier 2 or Tier 3.

### 3.3 Pure Context-Injected Helper (`PORTDIST-004`)
To ensure complete testability without polluting process-global environment state, resolution logic is decoupled into a pure helper:

```rust
pub fn resolve_resource_root_with_context(
    explicit_override: Option<&Path>,
    current_exe_path: Option<&Path>,
    cargo_manifest_dir: Option<&Path>,
) -> Result<PathBuf, DiagnosticError> {
    // 1. Explicit Override
    if let Some(explicit) = explicit_override {
        if is_valid_resource_root(explicit) {
            return Ok(explicit.to_path_buf());
        }
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
            format!(
                "Configured RUNTIMEPACK_RESOURCE_ROOT '{}' is invalid or missing required markers",
                explicit.display()
            ),
        ));
    }

    // 2. Executable-Relative Bundle Root
    if let Some(exe) = current_exe_path {
        if let Some(bin_dir) = exe.parent() {
            // Check standard layout: bin/../
            if let Some(bundle_root) = bin_dir.parent() {
                if is_valid_resource_root(bundle_root) {
                    return Ok(bundle_root.to_path_buf());
                }
            }
            // Check flat layout: <bundle>/runtimepack.exe
            if is_valid_resource_root(bin_dir) {
                return Ok(bin_dir.to_path_buf());
            }
        }
    }

    // 3. Development Source Fallback
    if let Some(manifest) = cargo_manifest_dir {
        // Direct manifest dir check
        if is_valid_resource_root(manifest) {
            return Ok(manifest.to_path_buf());
        }
        // Workspace root check (parent of crate)
        if let Some(parent) = manifest.parent() {
            if is_valid_resource_root(parent) {
                return Ok(parent.to_path_buf());
            }
        }
    }

    Err(DiagnosticError::from_code(
        DiagnosticCode::BLD_RESOURCE_ROOT_MISSING,
        "RuntimePack resource root could not be located. Ensure application is installed correctly or set RUNTIMEPACK_RESOURCE_ROOT.",
    ))
}
```

Production callers invoke:
```rust
pub fn resolve_resource_root() -> Result<PathBuf, DiagnosticError> {
    let explicit = std::env::var_os("RUNTIMEPACK_RESOURCE_ROOT").map(PathBuf::from);
    let exe = std::env::current_exe().ok();
    let dev = std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from);
    resolve_resource_root_with_context(explicit.as_deref(), exe.as_deref(), dev.as_deref())
}
```

---

## 4. Controlled Packaging Manifest (Positive Allowlist)

### 4.1 Security Invariant: Ban on Naive Tree Copying (`PORTDIST-005`)
Packaging tools MUST NOT execute blanket recursive directory copies of the workspace checkout. All files included in the distribution archive MUST match the positive allowlist below:

| Directory | Allowlist File Patterns | Explicit Exclusion Filter |
|---|---|---|
| `bin/` | `runtimepack` (Linux) or `runtimepack.exe` (Windows) | Any debug `.pdb`, `.dSYM`, or intermediary build objects |
| `android/shell/` | `build.gradle.kts`, `settings.gradle.kts`, `gradle.properties`, `gradlew`, `gradlew.bat` | `.gradle/`, `local.properties`, `app/build/` |
| `android/shell/gradle/wrapper/` | `gradle-wrapper.jar`, `gradle-wrapper.properties` | Any unexpected binary or backup files |
| `android/shell/app/` | `build.gradle.kts`, `proguard-rules.pro` | `build/`, `*.keystore`, `*.jks` |
| `android/shell/app/src/main/` | `AndroidManifest.xml`, `java/**/*.kt`, `java/**/*.java`, `res/**/*.png`, `res/**/*.xml`, `assets/**/*` | `src/test/`, build caches |
| `runtimes/manifests/` | `*.toml` | Temporary files, editor backups |
| `runtimes/themes/` | `**/*.css`, `**/*.json` | Source mockups, SVG sources |
| `runtimes/renpy/` | `manifest.json`, `licenses/**/*`, `jni/arm64-v8a/librenpython.so` | Python byte caches (`__pycache__`, `*.pyc`) |
| `runtimes/rgss/` | `jni/arm64-v8a/*.so` | Unpinned objects, unapproved architectures |
| `runtimes/vnmaker/` | `licenses/**/*` | None |
| `profiles/games/` | `**/cheats.js` | None |
| `compatibility-db/` | `*.toml`, `profiles/*.json` | Scratch manifests, test fixtures |
| `schemas/` | `*.schema.json` | None |
| Root | `toolchain.lock`, `LICENSE`, `NOTICE.txt`, `checksums.txt` | `.git/`, `.worktrees/`, `.cargo/`, `target/` |

---

## 5. Cross-Compilation & Target Architecture

### 5.1 Rust Windows Target Comparison (`PORTDIST-006`)
Two GNU-family cross-compilation targets exist in the official Rust toolchain:

| Dimension | `x86_64-pc-windows-gnu` | `x86_64-pc-windows-gnullvm` (Recommended) |
|---|---|---|
| **Rust Tier** | Tier 1 (Official Stdlib) | Tier 2 (Official Stdlib available via `rustup`) |
| **C Runtime** | Legacy `msvcrt.dll` | Modern Universal CRT (`ucrtbase.dll`) |
| **Linker Driver** | Expects GCC (`x86_64-w64-mingw32-gcc`) | Expects LLVM/Clang (`x86_64-w64-mingw32-clang`) |
| **Startup Code** | MinGW GCC `crt2.o` | MinGW LLVM `crt2.o` |
| **EH / Runtime Libs** | Requires `libgcc`, `libgcc_eh` | Uses LLVM `compiler-rt` (`builtins`) & `libunwind` |
| **Cross-Link Hazard** | Link failure (`missing -lgcc_eh`) when linked via Clang/LLVM-MinGW | Zero missing GCC symbols; matches LLVM-MinGW natively |
| **Windows OS Baseline** | Windows XP / 7+ | Windows 10 / 11 (or Win7/8.1 with UCRT update) |

**Architectural Recommendation:**
The project targets **`x86_64-pc-windows-gnullvm`** as the primary cross-compilation target when using LLVM-MinGW, ensuring clean linking against UCRT without artificial GCC library dependencies. `x86_64-pc-windows-gnu` is retained as a secondary option if legacy MSVCRT compatibility is explicitly required.

### 5.2 Dependency Analysis & Native Libs Matching (`PORTDIST-007`)
An audit of `Cargo.lock` in `.worktrees/fix-mvmz-logical-root` confirms:
- **Zero OpenSSL / Native TLS Dependencies:** Network calls are excluded from the packaging core.
- **Pure Rust Cryptography:** `sha2`, `chacha20poly1305`, and `hkdf` are 100% pure Rust implementations.
- **Pure Rust Compression:** `flate2` utilizes `miniz_oxide 0.9.1` (pure Rust, 0 C dependencies).
- **Pure Rust Image Processing:** `image` crate is pinned with `default-features = false`, including only `png` and `ico` decoders.
- **Conclusion:** The Rust crate graph compiles without requiring cross-compilation of external C/C++ libraries.

### 5.3 Static PE Binary Inspection (`PORTDIST-008`)
Under WSL2 Linux, cross-compiled Windows PE executables cannot be executed. Verification is performed strictly via static PE structure inspection using LLVM binary utilities:

```bash
llvm-readobj --coff-imports target/x86_64-pc-windows-gnullvm/release/runtimepack.exe
```

**Import Table Acceptance Criteria:**
1. Standard Windows System DLLs allowed:
   - `KERNEL32.dll`
   - `ADVAPI32.dll`
   - `WS2_32.dll`
   - `USERENV.dll`
   - `ucrtbase.dll` (or `msvcrt.dll`)
2. Non-system DLL dependencies (`libwinpthread-1.dll`, `libgcc_s_seh-1.dll`):
   - MUST NOT be dynamically required unless bundled with explicit redistribution notices in `bin/`.
   - Prefer static linking via `-C target-feature=+crt-static`.

---

## 6. Diagnostic & CLI Health Check Extensions

### 6.1 `runtimepack doctor` Resource Health Verification (`PORTDIST-009`)
`runtimepack doctor` is extended to inspect and report physical resource root health:

```text
Resource Root & Assets:
  [OK] Resource Root:     /opt/runtimepack (detected: EXECUTABLE_RELATIVE)
  [OK] Android Shell:     android/shell/build.gradle.kts (Gradle 8.7, AGP 8.3.0)
  [OK] Runtime Manifests: 6 registered (rgss, rpgmaker-web, renpy-7/8, tyrano, vnmaker)
  [OK] Theme Profiles:    3 installed (liquid_glass, matte, retro_osd)
  [OK] Cheat Profiles:    2 games registered (termina, lona)
  [OK] Compatibility DB:  9 capability manifests loaded
  [OK] JSON Schemas:      9 validation schemas intact
  [OK] Toolchain Lock:    Pinned baseline verified
```

If resources are missing, `doctor` flags the specific missing components with `[WARN]` or `[FAIL]` and exits with code 1.

---

## 7. Windows Native Validation Disclosure (`PORTDIST-010`)

In strict compliance with `AGENTS.md` and Architectural Ruling 6:
- Cross-compilation on a Linux WSL host and PE structure validation provide proof of compilation and binary structure only.
- Wine emulation under WSL is NOT accepted as proof of native Windows runtime compatibility.
- All Windows binary artifacts emitted by WSL cross-compilation carry the mandatory disclosure status:
  **`NOT_WINDOWS_RUNTIME_VALIDATED`**
- Final certification of native execution requires verification on a physical Windows host by the user or an authorized Windows CI worker.
