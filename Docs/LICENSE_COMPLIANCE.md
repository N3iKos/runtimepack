# License and Redistribution Compliance

> This is engineering compliance guidance, not legal advice.

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

### 3.1 Current status: `SOURCE_UNVERIFIED` (distribution BLOCKED)

The eight prebuilt libraries in `runtimes/rgss/jni/arm64-v8a/` were produced in a
third-party working tree (`/home/daniel/mkxp-z-android/app/jni/...`, visible in
embedded build paths) from Android clang 12.0.9.

| Requirement | State in this repository |
| :--- | :--- |
| Matching buildable engine source | **absent** — no mkxp-z/SDL2/Ruby/OpenAL source tree |
| Source commit of the shipped binary | **unrecoverable** — the only commit string in the binaries is the LLVM `c935d99d…` toolchain commit (see [S52]) |
| Build recipe / build options | **absent** |
| Redistributable third-party notice bundle (`runtimes/rgss/licenses/`) | **absent** — the manifest's `notice_bundle = "licenses/rgss"` points at nothing |
| Written offer of source | **absent** |

Consequences enforced by code:

- `pack-bundle` excludes `runtimes/rgss/**` from distribution bundles by
  default; an RGSS build from such a bundle fails closed with `BLD-109`.
- `pack-bundle --local-diagnostic-native` may include the libraries **for local
  testing only**, stamping the bundle `LOCAL_DIAGNOSTIC_ONLY`,
  `NO_REMOTE_DISTRIBUTION` and `SOURCE_UNVERIFIED`. That is not a compliance
  claim and not a source offer.
- Shipping generic GPL text is explicitly **not** treated as satisfying these
  obligations, because the obligation attaches to the actual upstream source and
  its notices, not to a licence text.

Resolving this requires obtaining the exact upstream source matching the shipped
binaries (with a verifiable commit), the build recipe/options, and the dependency
notice set. Until then RGSS distribution stays BLOCKED. RuntimePack's own host
source is complete and buildable; only the native engine binary is not
reproducible from this tree.

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

## 5. TyranoScript

TyranoScript and TyranoBuilder games are packaged using the user-provided game content and the Web runtime backend. RuntimePack packages the web assets provided in the source game distribution to run in an Android WebView shell.

## 6. RPG Maker game runtime/assets

RuntimePack does not grant rights to RPG Maker engine/game assets. A user must supply the input game.

Avoid bundling proprietary RPG Maker editor/runtime assets into RuntimePack unless distribution rights are verified.

The packager should prefer using files already present in the user's game distribution plus independently licensable compatibility runtimes.

## 6.1 Host-Side Build Dependencies

### `zip` (v8.6.0)
- **Role:** Build-time APK archive structural and CRC-32 validation reader.
- **License:** MIT (Copyright (c) 2014 Mathijs van de Nes, 2021-2024 zip-rs contributors).
- **Features:** `default-features = false`, `features = ["deflate-flate2-zlib-rs"]`.
- **Transitive dependencies:** `typed-path 0.12.3` (MIT OR Apache-2.0, Copyright (c) 2020 Chip Senkbeil).
- **Redistribution obligation:** The `zip` crate and `typed-path` are host-side Rust build dependencies linked into the `runtimepack` binary. Their full MIT license terms and copyright notices must be preserved and included in the host tool distribution's notice bundle. No code from `zip` or `typed-path` is embedded into generated standalone Android application packages.
- **Usage constraint:** High-level filesystem extraction routines (`ZipArchive::extract`) are strictly forbidden. Only non-extracting archive inspection and streaming byte validation APIs are used.

```text
The MIT License (MIT)

Copyright (c) 2014 Mathijs van de Nes

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

```text
The MIT License (MIT)

Copyright (c) 2020 Chip Senkbeil

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

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
- label a repository "MIT" based only on another repo by the same author.
