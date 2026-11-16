# RuntimePack — Windows Portable Distribution

**Bundle version:** 0.1.0-beta
**Release status:** see `BUILD_PROVENANCE.json` (`release_status`, `distribution_scope`)
**Validation status:** see `BUILD_PROVENANCE.json` (`validation_status`)

This bundle contains a native Windows x86-64 CLI plus the interactive Python
wizard. It needs **no** WSL, Rust, or Cargo to run. Python 3 is required only for
the interactive wizard; the CLI itself is a static PE binary.

---

## 1. Contents

```text
<this folder>\
├── RuntimePack-Wizard.bat   # launcher shim; delegates to wizard.py next to it
├── wizard.py                # interactive menu and build argument construction
├── bin\runtimepack.exe      # native Windows CLI (x86_64-pc-windows-gnullvm)
├── android\                 # Android project template + Ruby/JS runtime assets
├── runtimes\                # runtime manifests, themes, vendored native libraries
├── schemas\                 # JSON schemas for config/build manifests
├── compatibility-db\        # compatibility profiles
├── Docs\                    # license compliance documentation
├── toolchain.lock           # pinned Gradle / AGP / JDK / SDK versions
├── BUILD_PROVENANCE.json    # commit, binary hash, toolchain, honest status
├── ASSETS_CHECKSUM_MANIFEST.json
├── checksums.txt
└── LOCAL_DIAGNOSTIC_ONLY.txt   # present only in local-diagnostic bundles
```

Integrity rules, stated exactly:

- `checksums.txt` covers every other file in the bundle, **including**
  `ASSETS_CHECKSUM_MANIFEST.json`. Only `checksums.txt` itself is excluded
  (a file cannot hash itself).
- `ASSETS_CHECKSUM_MANIFEST.json` covers every file except `checksums.txt` and
  itself, so `total_files` = (files in bundle) − 2.
- File counts: a distribution bundle has 137 hashed files + `checksums.txt`
  = 138 files on disk. A local-diagnostic bundle additionally carries
  `LOCAL_DIAGNOSTIC_ONLY.txt`, and its RGSS native libraries.

Verify with:

```powershell
# PowerShell
Get-Content checksums.txt | ForEach-Object {
  $sha, $rel = $_ -split '  ', 2
  if (-not (Test-Path $rel)) { "MISSING $rel"; return }
  $got = (Get-FileHash $rel -Algorithm SHA256).Hash.ToLower()
  if ($got -ne $sha) { "MISMATCH $rel" }
}
```

```bash
# Linux / WSL
sha256sum -c checksums.txt
```

---

## 2. Running the wizard

Double-click `RuntimePack-Wizard.bat`, or:

```powershell
.\RuntimePack-Wizard.bat
```

The wizard:

- resolves `bin\runtimepack.exe` relative to its own location (never the CWD);
- honours `RUNTIMEPACK_EXE` to point at a different CLI binary;
- strips surrounding quotes from drag-and-dropped paths;
- writes each build to `<output root>\run_<n>`, a fresh numbered directory that
  never overwrites an earlier run;
- takes the output root from `RUNTIMEPACK_WIZARD_OUTPUT_ROOT` when set,
  otherwise `<bundle>\dist`.

Menu: `1` doctor, `2` inspect, `3` explain, `4` asset audit, `5` build, `0` exit.

Failure behaviour:

| Situation | Behaviour |
| :--- | :--- |
| Python 3 missing | launcher prints a diagnostic, exits 1 |
| `bin\runtimepack.exe` missing | `ERR-WIZ-001` with expected locations, exit 1 |
| Output root not writable (read-only install, file in the path, disk full) | `ERR-WIZ-002` with the attempted path, the OS cause, and the `RUNTIMEPACK_WIZARD_OUTPUT_ROOT` remedy; exit 1, no traceback |

The wizard runs each build into its own directory, so a read-only install
location is usable by setting `RUNTIMEPACK_WIZARD_OUTPUT_ROOT` once:

```powershell
set RUNTIMEPACK_WIZARD_OUTPUT_ROOT=%USERPROFILE%\RuntimePackBuilds
```

Builds create `dist\run_<n>` **inside the bundle** by default, so checksum
verification after first use reports those extra files. Point
`RUNTIMEPACK_WIZARD_OUTPUT_ROOT` elsewhere if you need the bundle to stay
byte-exact.

---

## 3. Building an APK

APK compilation requires, on the machine running the build:

- JDK 17+ (`JAVA_HOME` set, a bundled `~/.runtimepack/toolchains/jdk-17`, or
  `java` on `PATH`)
- Android SDK with `platforms/android-34` and `build-tools/34.0.0`

### How the SDK is found

RuntimePack resolves **one** toolchain and uses it for the preflight, the Gradle
child process and the generated project. Precedence:

1. `ANDROID_HOME`
2. `ANDROID_SDK_ROOT`
3. `sdk.dir` in the generated project's `local.properties`
   (this is what RuntimePack itself writes, and it is honoured on the next run
   when the project is retained with `--keep-build-files`)
4. the conventional install path (`%LOCALAPPDATA%\Android\Sdk`)

The SDK that is resolved is written into `<output>\android-project\local.properties`
and exported as `ANDROID_HOME`/`ANDROID_SDK_ROOT` to the Gradle process, so a
build that passes the preflight is the same build Gradle runs.

The simplest setup is the environment variable:

```powershell
$env:ANDROID_HOME = "C:\Android\Sdk"
bin\runtimepack.exe build "D:\Path\To\Game" --flavor beta --output dist\run_1
```

### Validation is strict and offline-first

Before Gradle starts, the CLI checks: SDK present, required platform present,
required build-tools present, JDK present, JDK major detectable and >= 17. The
required platform/build-tools versions are read from the shipped `toolchain.lock`
when it is readable; otherwise built-in defaults are used and the message says so.

**RuntimePack does not auto-download SDK components.** A host that would normally
let AGP fetch `platforms;android-34` or `build-tools;34.0.0` on demand will fail
closed here instead. That is deliberate: a build that silently mutates the
machine's SDK is not reproducible. Install the components once, explicitly:

```powershell
sdkmanager "platforms;android-34" "build-tools;34.0.0"
```

Any failure produces `BLD-101` naming the missing component, the pin source, and
the fact that nothing was downloaded, for example:

```text
[BLD-101] Android SDK at 'C:\Android\Sdk' is missing the required platform component 'platforms/android-34' (from toolchain.lock) (offline-first: RuntimePack does not auto-download SDK components). Install it with: sdkmanager "platforms;android-34"
```

```text
[BLD-101] JDK 17 or newer required by AGP/Gradle (from toolchain.lock), but the resolved JDK reports major version 11. Set JAVA_HOME to a JDK 17 installation.
```

Generate the Android project without compiling (works with no SDK/JDK):

```powershell
bin\runtimepack.exe build "D:\Path\To\Game" --flavor beta --no-compile --output dist\run_probe
```

`--no-compile` keeps the previous behaviour: no Gradle, no toolchain preflight,
project + manifest written to the output directory.

---

## 4. Engine support in this bundle

| Engine | Detection / audit | APK build from this bundle |
| :--- | :--- | :--- |
| RPG Maker MV | yes | expected; **not verified on this host** (web runtime, no native libs) |
| RPG Maker MZ | yes | expected; **not verified on this host** (web runtime, no native libs) |
| TyranoScript / TyranoBuilder | yes | expected; **not verified on this host** (web runtime, no native libs) |
| Ren'Py 8 | yes | expected; **not verified on this host** (`librenpython.so`, arm64-v8a) |
| Ren'Py 7 | yes | **no** — no verified native runtime in this tree; stops with `BLD-109` naming `runtimes/renpy-7/jni` |
| RPG Maker XP / VX / VX Ace (RGSS) | yes | **only in a local-diagnostic bundle** — see §5 |

"Expected" means the runtime, template and native libraries for that engine are
present and host-project generation is verified; no APK has been compiled or run
on Windows because no Android SDK was available on the build host. See §6.


Only `arm64-v8a` native libraries exist in this tree. The CLI derives the
advertised ABIs from the ABI directories that actually exist, so an APK never
declares an ABI it cannot load.

---

## 5. RGSS native runtime: prebuilt-only, distribution BLOCKED

The RGSS backend (RPG Maker XP/VX/VX Ace) runs on prebuilt native libraries in
`runtimes/rgss/jni/arm64-v8a/` (mkxp-z lineage, SDL2, Ruby, OpenAL).

- RuntimePack's **host source is complete** in the repository: detection, audit,
  packaging, project generation and the Ruby shims all build from source.
- The **native engine binary cannot be reproduced from this tree**. There is no
  engine source, no build recipe, and no source offer here. The binary hash and
  build id are recorded in the repository's `Docs/REFERENCES.md` (S52), which
  states that matching buildable source is unverified; the shipped
  `Docs/LICENSE_COMPLIANCE.md` §3.1 records the same status for this bundle.
- Therefore no redistributable third-party notice bundle for these binaries
  exists, and the declared `notice_bundle = "licenses/rgss"` is **absent**.
  This is a `SOURCE_UNVERIFIED` blocker, not a documentation gap.

Consequences:

- A **distribution** bundle excludes `runtimes/rgss/**`. RGSS builds from such a
  bundle stop with `BLD-109` naming `runtimes/rgss/jni`.
- A **local diagnostic** bundle (`runtimepack pack-bundle --local-diagnostic-native`)
  includes the prebuilt libraries plus a `LOCAL_DIAGNOSTIC_ONLY.txt` marker. It is
  for local testing only: **no remote distribution**, and it does not establish
  licensing compliance.
- No generic GPL text is shipped as a substitute for the actual upstream source
  and notice obligations. RGSS distribution stays BLOCKED until verified upstream
  source and notices are obtained.

---

## 6. Verified state of this bundle

The authoritative, per-build record is `BUILD_PROVENANCE.json`
(`validation_status`, `android_compile_status`) plus the review report in the
repository. Claims are limited to what was executed:

| Check | How it is established |
| :--- | :--- |
| `bin\runtimepack.exe --version` / `--help` | native Windows execution, exit 0 |
| Wizard startup (menu reachable, `RUNTIMEPACK_EXE`, `ERR-WIZ-001`) | `tests\windows\wizard_startup_test.ps1`, 3/3 PASS |
| Wizard output-root handling | `tests\windows\test_wizard_output_root.py` |
| `--no-compile` host-project generation | executed per engine, fixture bytes unchanged |
| Bundle integrity | `checksums.txt` re-hashed, 0 mismatches |
| Windows APK compile (`--flavor beta`) | only when a Windows Android SDK is present; otherwise `BLD-101` and **unverified** |
| Device / gameplay behaviour | not performed by build tooling |

`--help` or `--version` success is **not** evidence of engine support.

---

## 7. Scope limits

- **Beta watermark.** `--flavor beta` is the default and embeds the
  `Beta Test Veldra Community` watermark plus the diagnostic suite.
- **Asset hardening, not secrecy.** Packed/encrypted content raises extraction
  cost; it is not a guarantee against a rooted or instrumented device.
- **No broad storage permissions.** Generated apps stay in app-specific storage.

---

## 8. Rebuilding this bundle (one command)

From the repository checkout, after cross-compiling the CLI:

```bash
# 1. Cross-compile (WSL + pinned llvm-mingw, or native Windows cargo)
cargo build --release --target x86_64-pc-windows-gnullvm -p runtimepack

# 2. Package everything in one deterministic step.
#    --source-root is the checkout that produced the payload: launcher files are
#    copied from it and --commit is verified against its HEAD.
python3 scripts/package_windows_bundle.py \
  --output ../dist_runtimepack_windows_<UTC timestamp>_runtime_v2 \
  --bin target/x86_64-pc-windows-gnullvm/release/runtimepack.exe \
  --packer target/debug/runtimepack \
  --source-root . \
  --commit "$(git rev-parse HEAD)" \
  --branch release/runtime-v2 \
  --worktree runtime-v2 \
  --toolchain llvm-mingw-20261006-ucrt \
  --build-env "WSL2 Ubuntu 24.04 x86_64"
```

That single command:

1. refuses a non-empty `--output` (nothing is written on refusal);
2. runs `runtimepack pack-bundle` with the positive allowlist;
3. copies `wizard.py`, `RuntimePack-Wizard.bat` and this `README_WINDOWS.md`
   **from `--source-root`**, so the launcher cannot come from a different commit
   than the binary;
4. writes `BUILD_PROVENANCE.json`, verifying `--commit` against
   `git -C <source-root> rev-parse HEAD` (a mismatch is `BLD-111` and nothing is
   written);
5. regenerates `ASSETS_CHECKSUM_MANIFEST.json` and `checksums.txt`;
6. re-verifies every checksum and exits non-zero on any mismatch.

Provenance honesty: `--toolchain` and `--build-env` default to `unspecified` and
are recorded as operator-supplied claims, never measured. Omit them rather than
guessing. `--packer` exists because a Windows PE cannot read `/mnt` paths when
packaging from WSL.

Add `--local-diagnostic-native` for the local RGSS testing bundle described in
§5. The script has no default output path and no hardcoded machine paths.

See the repository `README.md` for Windows/Linux path conventions and
cross-build prerequisites.
