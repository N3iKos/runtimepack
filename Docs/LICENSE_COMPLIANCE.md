# License and Redistribution Compliance

> This is engineering compliance guidance, not legal advice. Ambiguous redistribution questions are release blockers pending qualified review or permission.

## 1. Principle

RuntimePack has three separate intellectual-property layers:

```text
A. RuntimePack original code
B. Third-party runtime/dependencies
C. User/game content
```

They must not be conflated.

## 2. JoiPlay

JoiPlay is used as:

- public behavioral reference;
- public architecture reference;
- public repository evidence.

Do not depend on proprietary/non-public JoiPlay core code.

JoiPlay's public `plugin` repository is MIT-licensed [S25], but that does not automatically establish the license of every JoiPlay repository or every submodule. Each dependency must be verified separately.

## 3. RGSS runtime

`mkxp-z` publicly states GPL-2.0+ and notes that a default HTTPS/OpenSSL build combination can create GPLv3 implications [S06].

Engineering consequence:

- do not copy/link/distribute mkxp-z until the intended distribution model has a documented compliance plan;
- record exact build options and dependency licenses;
- generated notices/source-offer requirements must be reviewed for the chosen linkage and distribution architecture.

The JoiPlay `android-mkxp` repository proves Android technical feasibility [S04/S05], but the implementation agent must not assume its entire combined tree has one simple license merely because it is public. Audit repository/submodule licenses before code reuse.

## 4. Ren'Py

Official Ren'Py documentation says most of Ren'Py uses MIT, while portions/dependencies use LGPL and other licenses, and distribution must satisfy those licenses [S09].

Therefore the Ren'Py runtime package must ship an automatically generated third-party notice bundle based on the exact pinned runtime version.

### 4.1 Pinned Android runtime (`renpy-android-8`)

RuntimePack vendors the official RAPT native runtime for `arm64-v8a`:

| Field | Value |
| :--- | :--- |
| Runtime id | `renpy-android-8` |
| Ren'Py SDK version | `8.3.7.25031702` ("Second Star to the Right", official, branch `fix`) |
| Python | 3.9 (statically linked) |
| Source artifact | `renpy-8.3.7-rapt.zip` |
| Source URL | https://www.renpy.org/dl/8.3.7/renpy-8.3.7-rapt.zip |
| Artifact SHA-256 | `ea3a96dc59a276464fcec30eb5371e12178be563348b1e318a5f236c3511ef9f` |
| Retrieved | 2026-09-27 (verified against PGP-signed `checksums.txt`) |

Vendored binary (single fat library — CPython 3.9, SDL2, pygame_sdl2, Ren'Py
Cython modules and libc++ are statically linked):

| File | Size (bytes) | SHA-256 | ELF machine |
| :--- | ---: | :--- | :--- |
| `runtimes/renpy/jni/arm64-v8a/librenpython.so` | 27918624 | `e111aa6c02b040019e68f0e36361574d1110b577105de09636864c3a38fed80a` | AArch64 |

Machine-readable provenance: `runtimes/renpy/manifest.json`.
Notice bundle: `runtimes/renpy/licenses/` (`LICENSE.txt`, `NOTICE.txt`,
`components.json`).

Obligations:

- the notice bundle and `licenses/LICENSE.txt` MUST be bundled into every
  generated APK/AAB that contains `librenpython.so` (do not strip notices);
- the exact pinned version, source URL and SHA-256 MUST be preserved to satisfy
  the LGPL source/relink obligations of bundled components (FFmpeg, libusb,
  fribidi, chardet, Pygame_SDL2 and others);
- RuntimePack does not relicense Ren'Py or its bundled dependencies.

## 5. TyranoScript — hard gate

The official public TyranoScript repository states that it is free/commercially usable for making works, but also lists prohibitions including redistributing TyranoScript itself and treating TyranoScript as a smartphone conversion-tool product, with instruction to consult the developer for such cases [S10].

RuntimePack is explicitly a conversion/packaging tool.

Therefore:

```text
TYRANO-LEGAL = BLOCKED
```

until one of the following is documented:

1. explicit permission/license from the Tyrano rights holder;
2. a reviewed architecture that does not redistribute restricted engine material and is legally acceptable;
3. an updated official license/term clearly permitting the intended model.

Implementation agents may build detector/analyzer abstractions, but MUST NOT ship Tyrano engine binaries/source in RuntimePack distributions while the gate is blocked.

## 6. RPG Maker game runtime/assets

RuntimePack does not grant rights to RPG Maker engine/game assets. A user must supply the input game.

Avoid bundling proprietary RPG Maker editor/runtime assets into RuntimePack unless distribution rights are verified.

The packager should prefer using files already present in the user's game distribution plus independently licensable compatibility runtimes.

## 7. License manifest

Every runtime package has:

```json
{
  "component": "example",
  "version": "1.2.3",
  "source": "https://...",
  "license": "MIT",
  "notice_files": [],
  "source_obligation": "reviewed"
}
```

Build fails with `LIC-120_MISSING_MANIFEST` for a bundled third-party component without license metadata.

## 8. Generated notices

Output:

```text
third-party-notices/
├── NOTICE.txt
├── components.json
└── licenses/
```

Only components actually bundled in the output APK/AAB belong in its notices.

## 9. Provenance

Each third-party source dependency must record:

- repository URL;
- commit/tag;
- cryptographic source archive hash;
- local patches;
- build options;
- license files;
- approval status.

## 10. Forbidden agent behaviors

An agent must not:

- scrape closed binaries and copy implementation;
- decompile proprietary engine/runtime code as an implementation shortcut;
- remove notices to reduce APK size;
- label a repository "MIT" based only on another repo by the same author;
- bypass Tyrano terms because the project is technically feasible.
