# References and Evidence Registry

Access baseline: **2026-09-18**

Use source IDs in implementation comments/design docs. URLs should be re-verified before major release because external documentation can change.

## Class A — Official product/vendor documentation

### S01 — JoiPlay official product page
URL: https://www.joiplay.cyou/index.html

Supports the claims:
- JoiPlay is a game interpreter and launcher.
- Listed families include RPG Maker XP/VX/VX Ace/MV/MZ, Ren'Py, TyranoBuilder, Construct, Flash, HTML.
- JoiPlay explicitly says it does not emulate Windows/another OS.
- Windows APIs and uncommon Node.js APIs can affect compatibility.

### S02 — JoiPlay RPGPacker
URL: https://joiplay.cyou/rpgpacker/

Supports:
- RPGPacker targets RPG Maker XP/VX/VX Ace -> Android.
- It generates a project from user/game files and builds with Gradle.
- It produces APK/AAB and signing artifacts.

### S07 — Ren'Py Android documentation
URL: https://www.renpy.org/doc/html/android.html

Supports:
- RAPT creates Android packages.
- Ren'Py Android packages run as Android apps.
- Android packaging has upstream-supported workflow.
- Ren'Py Android distributions include license obligations.

### S08 — Ren'Py command-line documentation
URL: https://www.renpy.org/doc/html/cli.html

Supports:
- upstream Android build CLI;
- APK/AAB build modes.

### S09 — Ren'Py license documentation
URL: https://www.renpy.org/doc/html/license.html

Supports:
- most Ren'Py under MIT;
- portions/dependencies under LGPL and other licenses;
- redistribution notice/compliance requirement.

### S11 — TyranoBuilder export tutorial
URL: https://tyranobuilder.com/tutorials/all-done-time-to-export

Supports:
- TyranoBuilder exports browser, Windows, macOS, iOS and Android targets.
- Browser export is a web-project model.

### S13 — RPG Maker MV official Android conversion documentation
URL: https://rpgmakerofficial.com/product/MV_Help/page/01_11_05.html

Supports:
- MV uses HTML5/WebGL/Canvas/WebAudio/JavaScript.
- Android packaging is described as a hybrid application using a browser engine/WebView.

### S14 — RPG Maker MZ official output documentation
URL: https://rpgmakerofficial.com/product/MZ_help-en/01_11_03.html

Supports:
- MZ web-browser deployment.
- HTML5/WebGL-capable Android browser environment is part of documented web target requirements.

### S15 — Android app-specific storage
URL: https://developer.android.com/training/data-storage/app-specific

Supports:
- internal app-specific storage isolation;
- no system storage permission needed for app-private files.

### S16 — Android Keystore
URL: https://developer.android.com/privacy-and-security/keystore

Supports:
- non-exportable key material;
- possible secure-hardware binding.

### S17 — Android App Bundle / signing FAQ
URL: https://developer.android.com/guide/app-bundle/faq

Supports:
- APKs need cryptographic signing;
- signing key is part of application update identity.

## Class B — Upstream/open-source runtime repositories

### S06 — mkxp-z
URL: https://github.com/mkxp-z/mkxp-z

Supports:
- open-source player for RPG Maker XP/VX/VX Ace.
- not universal compatibility; demanding games may need porting.
- GPL-2.0+ licensing statement and build-option license caveat.

### S10 — TyranoScript official repository
URL: https://github.com/ShikemokuMK/tyranoscript

Supports:
- TyranoScript uses HTML/JavaScript.
- repository layout includes `data`, `tyrano`, `index.html`.
- project states cross-platform targets.
- README terms include restrictions around redistribution and smartphone conversion-tool commercialization/use, requiring consultation in those cases.

### S12 — TyranoScript index.html
URL: https://github.com/ShikemokuMK/tyranoscript/blob/master/index.html

Supports:
- specific runtime files loaded from `tyrano/`;
- KAG plugin stack;
- JS/browser architecture and optional desktop `require` behavior.

## Class C — JoiPlay public implementation evidence

### S03 — JoiPlay RGA templates
URL: https://github.com/joiplay/rga

Supports:
- public JoiPlay game archive metadata structure;
- legacy/public type values and engine template directories.

Do not treat this old type list as the sole current support list; S01 is current product evidence.

### S04 — JoiPlay android-mkxp
URL: https://github.com/joiplay/android-mkxp

Supports:
- Android/mobile MKXP lineage used as JoiPlay RPG Maker plugin reference.
- public proof of Android feasibility for RGSS family.

### S05 — JoiPlay android-mkxp app build configuration
URL: https://github.com/joiplay/android-mkxp/blob/master/app/build.gradle

Supports:
- older public build uses Android application module;
- ABI splits include armeabi-v7a, arm64-v8a, x86_64;
- older min/target SDK values (historical evidence only).

### S18 — JoiPlay release: Ren'Py 8.4.1 separate plugin
URL: https://www.patreon.com/joiplay/posts/joiplay-1-20-600-135070650

Supports:
- JoiPlay created a new Ren'Py plugin for 8.4.1 due to breaking changes;
- multiple historical Ren'Py plugin generations are maintained.

### S19 — JoiPlay Android MKXP manifest
URL: https://github.com/joiplay/android-mkxp/blob/master/app/src/main/AndroidManifest.xml

Supports:
- JoiPlay launcher/plugin architecture historically used broad external-storage permissions.
- This is historical implementation evidence, **not** a RuntimePack recommendation.

### S20 — JoiPlay RPG Maker XP RGA template Game.ini
URL: https://github.com/joiplay/rga/blob/master/rpgmxp/Game.ini

Supports:
- `.rxdata` script marker and RGSS1 library example.

### S21 — JoiPlay RPG Maker VX RGA template Game.ini
URL: https://github.com/joiplay/rga/blob/master/rpgmvx/Game.ini

Supports:
- `.rvdata` script marker and RGSS2 library example.

### S22 — JoiPlay RPG Maker VX Ace RGA template Game.ini
URL: https://github.com/joiplay/rga/blob/master/rpgmvxace/Game.ini

Supports:
- `.rvdata2` script marker and RGSS3 library example.

### S23 — JoiPlay Tyrano RGA template
URL: https://github.com/joiplay/rga/blob/master/tyrano/game.cfg

Supports:
- Tyrano `index.html` entrypoint in JoiPlay's public archive template.

### S24 — JoiPlay JoiPad
URL: https://github.com/joiplay/joipad

Supports:
- JoiPlay separates virtual gamepad into a reusable plugin component.

### S25 — JoiPlay public plugin repository license
URL: https://github.com/joiplay/plugin/blob/master/LICENSE

Supports:
- this specific `joiplay/plugin` repository is MIT licensed.
- does NOT establish licenses for unrelated JoiPlay repositories.

## Historical supplemental reference

### S26 — Archived JoiPlay site source
URL: https://github.com/joiplay/joiplay.github.io-archived/blob/master/index.html

Supports historical architecture statements:
- old JoiPlay used Ren'Py and RPG Maker plugins based on Ren'Py/RAPT/MKXP/android-mkxp.
- compatibility affected by native libraries, Win32API and Node.js modules.

Use only as historical context when current docs are silent.

### S27 — Ruby Marshal 4.8 Format and RGSS Data Specifications
URL: https://docs.ruby-lang.org/en/3.2/marshal_rdoc.html
Preserved RGSS1/RGSS2/RGSS3 specifications and Enterbrain documentation for RPG Maker XP / VX / VX Ace.

Supports:
- Ruby Marshal 4.8 binary format specification: major 4 minor 8 magic bytes `\x04\x08`, type tokens for nil (`0`), bool (`T`/`F`), fixnum (`i`), string (`"`), symbol (`:`), symbol link (`;`), object link (`@`), array (`[`), hash (`{`), object (`o`), instance variable wrapper (`I`), user-defined dump (`u`), user marshal (`U`).
- RGSS file extensions: `.rxdata` (XP / RGSS1), `.rvdata` (VX / RGSS2), `.rvdata2` (VX Ace / RGSS3).
- RGSS data structure schemas: `RPG::System`, `RPG::Map`, `RPG::Event`, `RPG::Event::Page`, `RPG::EventCommand`, `RPG::MoveRoute`, `RPG::MoveCommand`, `RPG::Actor`, `RPG::Enemy`, `RPG::Animation`, `RPG::Tileset`, `RPG::CommonEvent`, `RPG::Troop`, `RPG::AudioFile`, `RPG::BGM`, `RPG::BGS`, `RPG::ME`, `RPG::SE`.
- Event command codes: 201 (Player Transfer), 212 (Show Animation), 231 (Show Picture), 241 (Play BGM), 245 (Play BGS), 249 (Play ME), 250 (Play SE), 322 (Change Actor Graphic).
- Move route command codes: 41 (Change Graphic), 44 (Play SE).

### S28 — Ren'Py Script Language Syntax and Archive Specifications
URL: https://www.renpy.org/doc/html/quickstart.html
URL: https://www.renpy.org/doc/html/audio.html
URL: https://www.renpy.org/doc/html/displayables.html
URL: https://www.renpy.org/doc/html/python.html

Supports:
- Ren'Py script file extension: `.rpy` and compiled bytecode `.rpyc`.
- Ren'Py archive format extension: `.rpa` (RPAv3 / RPAv2 / RPAv1 archive packages).
- Declarative image statement syntax: `image <tag> = "<path>"`, `image <tag> = Movie(...)`.
- Audio statement syntax: `play music "<path>"`, `play sound "<path>"`, `play voice "<path>"`, `define audio.<name> = "<path>"`, dynamic channel expressions (`play music <var>`).
- Scene and show statement syntax: `scene "<path>"`, `show "<path>"`, `scene <tag>`, `show <tag>`.
- Video playback statement syntax: `movie "<path>"`, `renpy.movie_cutscene("<path>")`.
- Python code execution blocks: `init python:`, `python:`, `$ <stmt>`.
- Python import restrictions for standalone mobile runtimes: native C extensions (`.pyd`, `.so`, Windows `.dll`) and unsupported native imports (e.g. `ctypes`, `win32api`) cannot run on Android ARM64 pure-Python runtimes.

### S29 — Visual Novel Maker HTML5 Export Specifications and Legacy Architecture
URL: https://asset.visualnovelmaker.com/help/Engine_Getting_Started.htm
URL: https://asset.visualnovelmaker.com/help/engine_doc/classes/gs.Graphics.html
URL: https://asset.visualnovelmaker.com/help/engine_doc/classes/gs.GameStorage.html
URL: https://asset.visualnovelmaker.com/help/game_script_doc/classes/Component_MessageBehavior.html
URL: https://asset.visualnovelmaker.com/help/Create_Game_Package.htm

Supports:
- Visual Novel Maker uses an HTML5 JavaScript framework ("Basic Engine") separating core engine components (`data/ENGINE.js`) from game behaviors and scene scripts (`data/SCRIPTS.js`).
- Verified core engine export contains `window.gs` namespace initialization and `GameStorage` persistence management.
- Verified game scripts export contains `vn.Component_MessageBehavior` and `gs.Application`.
- Entry point `index.html` loads platform libraries and scripts in defined order, referencing `data/ENGINE.js` and `data/SCRIPTS.js`.
- Legacy desktop packaging produces a web-application bundle with `package.json` (`main: "index.html"`).
- Supported experimental export profile identifies this specific legacy export layout verified from fresh game research; does not imply universal support for all past and future VNM generations.

### S30 — Visual Novel Maker Static Asset Manifest and Encryption Baseline
URL: local://docs/research/VNM_PORTING_POSTMORTEM_AND_RECOMMENDATIONS.md
URL: https://asset.visualnovelmaker.com/help/Resource_Standards.htm
URL: https://asset.visualnovelmaker.com/help/Create_Game_Package.htm

Supports:
- Visual Novel Maker encrypted exports contain encrypted asset tables in `data/RESOURCES.json.js` and `data/SUMMARIES.json.js`.
- Opaque/encrypted manifests cannot be statically resolved without encryption keys and game execution; static audit must report INDETERMINATE with incomplete reference scope.
- Unreferenced asset files cannot be inferred as orphans while manifest resolution is indeterminate.
- Native Node.js modules (`.node`) and unsupported native desktop libraries (`.dll`, `.so`) are incompatible with Android WebView.

### S31 — JSZip NodeBuffer and ArrayBuffer Specifications
URL: https://github.com/Stuk/jszip
URL: https://stuk.github.io/jszip/documentation/api_jszip/generate.html

Supports:
- JSZip uses `new Buffer(data, encoding)` and `Buffer.isBuffer(b)`.
- When type is `"nodebuffer"`, output satisfies `Buffer.isBuffer` and provides Uint8Array/Buffer methods (`slice`, `toString`, `copy`, `alloc`, `from`).
- Multibyte UTF-8 and base64 transformations must maintain exact byte fidelity.

### S32 — Visual Novel Maker Storage and NW.js Window Contract Baseline
URL: local://fresh-game-inspection/data/ENGINE.js

Supports:
- `GameStorage` in `data/ENGINE.js` routes persistence via `window.nw` and `require("fs")` when `$PARAMS.isOffline && !$PARAMS.platform.isSteamOS && null != window.nw`, else fallback to `localStorage.getItem($PARAMS.uid + "_" + e)`.
- `nw.Window.get()` provides `isFullscreen`, `setResizable`, `enterFullscreen`, `leaveFullscreen`, and event handling `on("resize", ...)`.

### S33 — Web Audio API AudioScheduledSourceNode Lifecycle and WebIDL onended Setter Specifications
URL: https://www.w3.org/TR/webaudio/#AudioScheduledSourceNode

Supports:
- Calling `stop()` on an `AudioScheduledSourceNode` before `start()` has been invoked throws `InvalidStateError`.
- `onended` is an IDL attribute on `AudioScheduledSourceNode` whose setter accepts an event handler callback.
- Stale or asynchronous `onended` callbacks running after reference disposal must be detached or guarded from manipulating nulled destination or restarting unreferenced loops.

### S34 — Visual Novel Maker Voice Duration and Skip Lifecycle Specifications
URL: local://fresh-game-inspection/data/SCRIPTS.js

Supports:
- Line 11501: `vn.Component_MessageBehavior.prototype.voiceDuration` accesses `this.voice.source.buffer.duration` and `this.voice.source.playbackRate.value`.
- When `this.voice` has been disposed, `voice.source` is null, causing `TypeError: Cannot read properties of null (reading 'buffer')`.
- When `this.tempSettings.skip` is true, the method returns `1` frame.

### S35 — Visual Novel Maker Synchronous HTML5 Script Execution Order
URL: local://fresh-game-inspection/index.html

Supports:
- `index.html` loads libraries synchronously in `<head>` before `data/ENGINE.js` and `data/SCRIPTS.js`.
- Shims must execute before `data/ENGINE.js`, and message behavior hooks must execute immediately following `data/SCRIPTS.js` before DOM-ready `Main.start()`.

### S36 — NW.js v0.114.0 Windows x64 Distribution Archive
URL: https://dl.nwjs.io/v0.114.0/nwjs-v0.114.0-win-x64.zip
(redirected to https://dl.node-webkit.org/v0.114.0/nwjs-v0.114.0-win-x64.zip)

Supports:
- Official NW.js v0.114.0 Windows x64 distribution archive (SHA-256: `4acd0e6a14086ed4397cbb64b0a163030f72a17cfd9a76c8a61cec160e0c0e01` from official `SHASUMS256.txt`).
- Contains DirectX Shader Compiler runtime redistributables:
  - `dxcompiler.dll` (SHA-256: `474781c2883a1071484c1eb2dd4e403cd31f491b0896a1a30336acd22e16d7a8`)
  - `dxil.dll` (SHA-256: `058f2f52a680c38b223a5615b7df969def21f77e577562e5099d971dede992de`)
- Verified upstream match with desktop wrapper binaries in NW.js desktop packages; these are wrapper runtime components excluded from Android mobile package, not bundled application payload assets.
- No third-party DLL redistribution: wrapper DLLs are excluded from packaging, and no foreign dependencies are added.

### S37 — RPG Maker MZ Utils.isNwjs Implementation Specification
URL: official RPG Maker MZ core script documentation / `rmmz_core.js`

Supports:
- Official RPG Maker MZ `rmmz_core.js` (lines 242–244): `Utils.isNwjs = function() { return typeof require === "function" && typeof process === "object"; };`.
- Evaluates to `true` when synthetic Node.js `require` and `process` mocks are injected by runtime polyfills.
- Plugins branching on `Utils.isNwjs()` execute desktop paths (such as `require("os")` or `require("nw.gui")`), requiring synchronous virtualization.

### S38 — RPG Maker MV Utils.isNwjs Implementation Specification
URL: official RPG Maker MV core script documentation / `rpg_core.js`

Supports:
- Official RPG Maker MV `rpg_core.js` (lines 228–230): `Utils.isNwjs = function() { return typeof require === "function" && typeof process === "object"; };`.
- Parity with MZ detection logic across Web runtime family.

### S39 — RPG Maker MZ StorageManager.isLocalMode Specification
URL: official RPG Maker MZ `rmmz_managers.js`

Supports:
- In `rmmz_managers.js`: `StorageManager.isLocalMode = function() { return Utils.isNwjs(); };`.
- When `Utils.isNwjs()` returns `false`, `StorageManager` defaults to web storage (`saveToForage` in MZ, `saveToWebStorage` in MV).

### S40 — RPG Maker MZ Internal Fixture Platform Identity Analysis
URL: local internal inspection of subject game `rmmz_core.js`

Supports:
- Verified internal file evidence of `Utils.isNwjs` definition pattern across inspected RPG Maker MZ core versions.
- Confirms requirement for synchronous platform identity virtualization at script completion boundary before downstream plugins evaluate.

### S41 — Apache Cordova InAppBrowser Plugin API Specification
URL: https://cordova.apache.org/docs/en/latest/reference/cordova-plugin-inappbrowser/
Retrieval Date: 2026-10-10

Supports:
- `cordova.InAppBrowser.open(url, target, options)` API lifecycle and return value.
- When `target` is specified as `'_system'`, external navigation is handed off directly to the system's native web browser.
- The return object exposes event handlers for in-app views, but `_system` transfers external URL navigation to native OS browser handlers.
- RuntimePack provides dedicated native bridge `openExternalUrl` rather than full Cordova runtime emulation.

### S42 — Android Developers Intent.ACTION_VIEW & @JavascriptInterface Threading Specification
URL: https://developer.android.com/reference/android/content/Intent#ACTION_VIEW
URL: https://developer.android.com/reference/android/webkit/JavascriptInterface
Retrieval Date: 2026-10-10

Supports:
- `Intent.ACTION_VIEW` combined with `Intent.CATEGORY_BROWSABLE` is the standard Android mechanism for delegating web URLs to external browsers.
- Methods annotated with `@JavascriptInterface` run on a private background WebView thread, not the Android UI main thread.
- UI interactions and Activity launches initiated from background threads must be posted to the main looper or use `FLAG_ACTIVITY_NEW_TASK`.
- Missing browser activity results in `ActivityNotFoundException`. Bridge methods return synchronous acceptance boolean without blocking the JS worker thread.

### S43 — Android WebView Cross-Origin Interface Security Guidelines
URL: https://developer.android.com/privacy-and-security/risks/webview-attacks
Retrieval Date: 2026-10-10

Supports:
- `WebView.addJavascriptInterface` exposes Java objects to all frames in the WebView.
- External URL bridges and WebViewClients must strictly enforce URI scheme allowlists (`http`/`https`), override URL loadings to prevent in-place navigation of external domains, and reject dangerous schemes (`javascript:`, `file:`, `intent:`) to prevent sandbox bypass and origin leak.
- Privileged WebViews with native bridges must isolate internal origins from remote documents.

### S44 — RFC 3986 Uniform Resource Identifier (URI): Generic Syntax
URL: https://www.rfc-editor.org/rfc/rfc3986
Retrieval Date: 2026-10-10

Supports:
- Section 3.2 (Authority): Authority syntax `[ userinfo "@" ] host [ ":" port ]`. Characters `@` and `%40` in the authority component denote userinfo credentials and require rejection for origin isolation.
- Section 3.3 (Path): Path segments consist of `pchar = unreserved / pct-encoded / sub-delims / ":" / "@"`. Character `@` and percent-encoded `%40` are valid path segment characters in hierarchical URIs.
- Disallows treating `@` and `%40` inside the URI path/query as credential injection or authority delimiter.

### S45 — Electron ASAR Archive Format & Pickle Specification
URL: https://github.com/electron/asar (tag v4.3.0, commit 01bc908255640c544ac68ea2fe14926a7b80da37)
URL: https://raw.githubusercontent.com/electron/asar/01bc908255640c544ac68ea2fe14926a7b80da37/src/disk.ts
URL: https://raw.githubusercontent.com/electron/asar/01bc908255640c544ac68ea2fe14926a7b80da37/src/pickle.ts
URL: https://raw.githubusercontent.com/electron/asar/01bc908255640c544ac68ea2fe14926a7b80da37/src/filesystem.ts
Retrieval Date: 2026-10-10

Supports:
- ASAR binary header layout: Chromium Pickle framing. Outer pickle (first 8 bytes) with uint32 size magic 4 and uint32 `header_size` (inner pickle payload length). Inner pickle contains uint32 `json_len` at bytes 12..16 followed by UTF-8 JSON header string. Payload base offset is `8 + header_size` aligned to 4-byte boundary.
- Header validation: `8 + header_size <= file_len` and `json_len <= header_size - 4`. Checked arithmetic before memory allocation.
- Directory and entry model: Hierarchical `files` map. Leaf file entries contain `size` (unsigned integer) and `offset` (decimal string or unsigned integer). File payload absolute offset is `data_offset + offset`, satisfying `offset + size <= payload_len` and absolute file bounds.
- Deduplication: Official ASAR format permits distinct files to reference overlapping or identical `offset` and `size` when payload contents are deduplicated.
- Entries with `"link"` or `"unpacked": true` represent symlinks or external loose directory references. Standalone mobile container requires explicit rejection of unsupported link/unpacked entries rather than fallback or offset 0.
- Entry name safety: Entry names may contain periods (`index.html`, `tyrano.js`), but exact `.` or `..`, embedded slashes (`/`, `\`), Windows drive letters, UNC prefixes, and NUL bytes are prohibited. Pre-extraction prevalidation ensures entries remain strictly within the extraction destination.

### S46 — Official Ren'Py Release Packaging & Static Version Metadata Schema
URL: https://github.com/renpy/renpy (tag 8.3.7.25031702, annotated tag 982f4ea9740c7f5da9bc6675609be7e5793b549b, target commit b38cc6a5018edc7da3f201c33f46de666a64f3cd)
URL: https://github.com/renpy/renpy (tag 7.4.11.2266, commit 0dbbf5bb8298dcba2725fa58066f272a8169cb84)
URL: https://raw.githubusercontent.com/renpy/renpy/b38cc6a5018edc7da3f201c33f46de666a64f3cd/renpy/versions.py
URL: https://raw.githubusercontent.com/renpy/renpy/b38cc6a5018edc7da3f201c33f46de666a64f3cd/renpy/__init__.py
Retrieval Date: 2026-10-10

Supports:
- Official Ren'Py distribution packaging generates `renpy/vc_version.py` containing single-line literal assignment `version = '8.3.7.25031702'` with official/nightly boolean flags.
- Static version metadata grammar: full-line literal assignment `version = '<major>.<minor>.<patch>[.<build>]'` where components consist strictly of ASCII digits. Alphanumeric suffixes and dynamic concatenation expressions are unverified and rejected.
- Historical packaged distributions define static `version_tuple` in `renpy/__init__.py` as 3-integer tuple `(major, minor, patch)` or 4-integer tuple `(major, minor, patch, build)` or 4-element tuple `(major, minor, patch, vc_version)` with identifier `vc_version` (where commit is unknown).
- Standalone `versions.py` listing historical branch tables, game-level configuration `config.version` in `game/options.rpy`, script dialogue, comments, and unverified `.rpyc` bytecode headers do not define the authoritative engine version.

### S47 — Official RGSS Archive Format (RGSSAD) Specification & Version Marker Invariants
URL: https://raw.githubusercontent.com/mkxp-z/mkxp-z/290ffe1135142d91a40b8f295a7b8e284f432c9a/src/crypto/rgssad.cpp
URL: https://docs.rs/crate/rpgmad-lib/5.0.2/source/src/lib.rs
Retrieval Date: 2026-10-10

Supports:
- Official RGSS archive header layout (`mkxp-z` v2.4.2 commit `290ffe1135142d91a40b8f295a7b8e284f432c9a`, `src/crypto/rgssad.cpp`, `verifyHeader(const char header[8])`): header consists of exactly 8 bytes (`char[8]`). First 7 bytes are magic signature `b"RGSSAD\0"` (`[0x52, 0x47, 0x53, 0x53, 0x41, 0x44, 0x00]`). The 8th byte (`header[7]`) is the archive format version number.
- Official version mapping:
  - RPG Maker XP (`.rgssad`): Header version byte 1 (`b"RGSSAD\0\x01"`).
  - RPG Maker VX (`.rgss2a`): Header version byte 1 ALSO (`b"RGSSAD\0\x01"`). Upstream official Enterbrain archivers for VX emit version 1; standard official version 2 does not exist.
  - RPG Maker VX Ace (`.rgss3a`): Header version byte 3 (`b"RGSSAD\0\x03"`).
- Engine classification invariant: because both XP and VX archives emit version byte 1, archive header inspection alone cannot differentiate between XP and VX. Engine classification strictly requires the combination of header version (1) + file extension (`.rgssad` vs `.rgss2a`) + `Game.ini` script/library references + script archive markers.
- Non-standard header byte 2 tolerance: legacy decoder tolerance for format byte 2 in `parse_rgssad_v1_v2` is retained for non-standard third-party utilities, but automatic detection awards zero (+0) archive detection points for archives with non-standard header byte 2. Emits an unsupported/degraded finding; detection never auto-changes engine family.
- Static non-executing guarantee: inspection reads only file header and TOC metadata into memory; zero plaintext decryption to disk and zero Ruby interpreter execution during detection or audit.

### S48 — zip crate (zip2 fork) v8.6.0 Specification & Security Invariants
URL: https://github.com/zip-rs/zip2 (tag v8.6.0, commit 771dfc534d2614158af5497ea3dff4d4208d7db1)
URL: https://crates.io/api/v1/crates/zip/8.6.0
URL: https://raw.githubusercontent.com/rustsec/advisory-db/master/crates/zip/RUSTSEC-2025-0168.md
Retrieval Date: 2026-10-12

Supports:
- Crate version: `zip 8.6.0`, published 2026-04-25, license `MIT`, MSRV `1.88`, crates.io SHA-256 `2d04a6b5381502aa6087c94c669499eb1602eb9c5e8198e534de571f7154809b`.
- Security advisory audit: Historical vulnerability RUSTSEC-2025-0168 / CVE-2025-29787 (GHSA-94vh-gphv-8pm8) affected archive extraction functions (`ZipArchive::extract` and `ZipStreamReader::extract`) in versions `>= 1.3.0, < 2.3.0`. Later advisories (GHSA-4684-h2q9-2vcp, GHSA-vv5m-jxxh-4627, GHSA-4v9h-p7m9-rh26) also exclusively affect archive filesystem extraction routines (`extract()`, permissions handling, and symlink targets).
- RuntimePack usage invariant: RuntimePack never invokes high-level filesystem extraction routines (`extract()` or `extract_unwrapped_root_dir()`). The crate is strictly used as a static inspection and validation reader (`ZipArchive::new(reader)`, checking entry headers, central directory records, and streaming entry payloads through `Crc32Reader` to EOF without writing entries or symlinks to disk).
- Resource bounding & physical archive limit: In `zip 8.6.0`, `ZipArchive::new` inspects the central directory from the end of the file. To prevent physical archive OOM and decompression exhaustion, `ZipValidator` enforces a pre-construction physical archive size check (`max_archive_bytes`, default 8 GiB), followed by post-parsing limits on entry count (`max_entries`, default 100,000), uncompressed aggregate payload bytes (`max_expanded_bytes`, default 8 GiB), and timeout budget (`timeout`, default 120s).
- Dependency lock: transitive dependency `typed-path 0.12.3` (license `MIT OR Apache-2.0`, crates.io SHA-256 `8e28f89b80c87b8fb0cf04ab448d5dd0dd0ade2f8891bae878de66a75a28600e`).

### S49 — Android SDK apksigner Verification Architecture & CLI Contract
URL: https://developer.android.com/tools/apksigner
URL: https://android.googlesource.com/platform/tools/apksig/
Retrieval Date: 2026-10-12

Supports:
- Android SDK build-tools `apksigner` is the official utility to verify APK signature schemes (v1, v2, v3, v4).
- Invocation model: `apksigner` is distributed as a shell script/batch file and a direct Java archive located at `build-tools/<version>/lib/apksigner.jar`.
- Windows argument escaping safety: Direct execution of `java.exe -jar <sdk_root>/build-tools/<version>/lib/apksigner.jar verify --verbose --print-certs <apk_path>` via argument array avoids command-line batch parameter injection (`%`, `!`, `&`, `^`) and quoting defects on Windows. On Windows, executing `apksigner.bat` via cmd.exe is strictly prohibited and rejected with `SIG-104` to eliminate batch injection risks.
- Process management & bounded I/O: Verification executes with a deterministic deadline (default 120 seconds), polling child status with `child.try_wait()`, draining stdout and stderr via concurrent background reader threads into bounded buffers (default 4 MiB) to prevent pipe deadlock, and terminating the owned child process upon timeout before reporting `SIG-103`.
- Verification output grammar: On successful verification, `apksigner verify` returns exit code 0. Standard output contains `Verifies\n` and lines `Verified using v2 scheme (APK Signature Scheme v2): true` (or v3/v4). If signatures are missing, corrupt, or invalid, `apksigner` exits with a non-zero exit code and outputs `DOES NOT VERIFY` or specific failure diagnostics.

### S50 — Legacy NW.js v0.12.0 Windows IA32 Distribution
URL: https://dl.nwjs.io/v0.12.0/nwjs-v0.12.0-win-ia32.zip
Mirror: https://registry.npmmirror.com/-/binary/nwjs/v0.12.0/nwjs-v0.12.0-win-ia32.zip
Retrieval Date: 2026-10-18

Supports:
- The versioned Windows desktop distribution contains `pdf.dll` (12,247,040 bytes, SHA-256 `7ef64b7a3f2d48075d7cb31dd359ccee0281fbb0d245d6d974886544282dfa89`) and `ffmpegsumo.dll` (961,536 bytes, SHA-256 `794d6bb8bc1ae294ba37dc5a7931a0801bb767f208eb894295812d778bc6349e`).
- Archive SHA-256: `3a4e2c3408e9e1cfe53dfbe0f4e4c0b42fb0081eff4f41ab17d8cba8734e1867`. Both DLLs were byte-for-byte matched against the reported game wrapper without loading or executing them.
- MV/MZ classification may recognize these legacy desktop wrapper filenames outside a resolved payload. Native binaries inside a resolved payload remain unsupported; this is not an exemption for arbitrary DLLs or proof of game playability.
- These Windows components are excluded from Android packaging, not redistributed as Android dependencies. Regression: `test_web_legacy_nwjs_wrapper_dlls_respect_payload_boundary` in `tests/integration/compatibility_test.rs`.

### S51 — RGSS Cache and Bitmap Error Semantics & Dynamic Localization Probe Contract
URL: https://github.com/mkxp-z/mkxp-z/blob/master/src/sharedstate.cpp
URL: https://github.com/mkxp-z/mkxp-z/blob/master/src/bitmap.cpp
Reference: RGSS3 / Enterbrain official script library `Cache` implementation; King Exit Script 121 (`Langscore`) lines 940–978.

Supports:
- RGSS primitive `Bitmap.new(filename)` raises `Errno::ENOENT` (or `RGSSError`) when an asset file does not exist on disk or in mounted archives. It does not return dummy placeholders.
- RGSS `Cache.normal_bitmap(path)` delegates directly to `Bitmap.new(path)` into `@cache[path]` and propagates file-not-found exceptions without negative caching.
- Community and commercial localization frameworks (such as `Langscore` in King Exit) dynamically alias `Cache.load_bitmap` (e.g. `ls_base_load_bitmap`) and probe localized candidates with trailing language suffixes (e.g. `title_`, `casor_`). The probe relies on receiving `Errno::ENOENT` to record candidate absence (`$ls_graphic_cache[filename] = false`) and fall back to the base asset.
- Swallowing exceptions at low-level `Bitmap.new` or `Cache.normal_bitmap` into dummy bitmaps breaks this probe contract, causing false-positive cache entries (`$ls_graphic_cache = true`) that prevent loading existing base assets.
- Missing asset recovery (e.g. tileset dummy, character marker, transparent dummy) belongs strictly at public typed `Cache` endpoints (`Cache.<method>` / `RPG::Cache.<method>`) after localization and alias chains have resolved.

### S52 — RGSS mkxp-z Font Rendering Latency and Outline Microbenchmark Telemetry
URL: https://github.com/mkxp-z/mkxp-z/blob/master/src/display/font.cpp
URL: https://github.com/mkxp-z/mkxp-z/blob/master/src/display/bitmap.cpp
Reference: Transient host/emulator diagnostic artifacts at `Temp/opencode/mumu_text_microbench_qa_20261019_181648/QA-report.md`; `Temp/opencode/mumu_text_attribution_20261019_180559/analysis-text-boundary.md`; `Temp/opencode/mumu_font_workaround_20261019_190949/executor-report.md`.
Retrieval Date: 2026-10-19

Supports:
- Text rasterization latency under `outline=true` on Android x86_64 translation layer varies significantly by font family (e.g. `VL Gothic` warm median 232.09 ms for len=11 vs `Source Han Sans CN` 5.93 ms; warm 'A' 28.52 ms vs 2.92 ms vs 0.23 ms outline=false).
- Causal boundary isolation: native `Bitmap#draw_text` and `Bitmap#text_size` account for 3,355.87 ms (95.47%) of observed 3,515.21 ms menu transition freeze in King Exit (41 draw calls [24 single-char, 17 multi-char] + 24 size calls; internal FreeType/GPU pipeline stages unverified); memory allocation and fill mutations execute in <0.15 ms.
- Native prebuilt binary (`runtimes/rgss/jni/arm64-v8a/libmkxp-z.so` SHA-256 `df8dc305b44206da7134d6b4ff9dc34f07ff0c63cb10a4d47f763d6a2d1123db`, build ID `8550ebc98e9e2191460d9a386daaf7cf58431b91`) parses declarative `fontSub` array in `mkxp.json` with single separator `>` (`"FROM>TO"`, e.g. `"VL Gothic>Source Han Sans CN"`) in `SharedFontState::SharedFontState` and substitutes requested font family in `SharedFontState::getFont` via single-pass map lookup without chaining.
- **Provenance correction (2026-10-21).** An earlier revision of this entry recorded commit `c935d99d7cf2016289302412d708641d52d2f7ee` as "reported commit ... from metadata alongside binary hash/build ID". Scoped inspection of the shipped `.so` files shows that string is part of the toolchain banner `Android (8481493, based on r416183c2) clang version 12.0.9 (https://android.googlesource.com/toolchain/llvm-project c935d99d7cf2016289302412d708641d52d2f7ee)` in `libSDL2_image.so` and `libc++_shared.so`. It is the **LLVM/llvm-project commit**, not an mkxp-z source commit. No mkxp-z source commit is recoverable from the binaries. Build paths embedded in the libraries reference a third-party working tree (`/home/daniel/mkxp-z-android/app/jni/...`). Matching buildable source therefore remains **UNVERIFIED**, and no source offer, build recipe, or redistributable notice bundle for these binaries exists in this repository.
- Font file distribution consistency: `VL Gothic` (3,883,852 bytes, SHA-256 `7bc537ad07905dfdb6296d7e30b205fd71b8d6159702f4ead167cab3fb9032f6`) matches identical distribution copy hashes across pack copies (distribution copy consistency, not TTF structural validation); bundled alternative `SourceHanSansCN-Regular-2.ttf` (container-relative path in `game.pack`, 8,364,840 bytes, SHA-256 `14ec9d09dad83098938370be17170a747d63084022b05710e3d34b4ddb359091`) verified present.
- Native C method reflection confirms `Bitmap#draw_text` `source_location` is `nil` (pure native binding).

### S53 — Microsoft Win32 Private Profile String Contract
URL: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getprivateprofilestringa
URL: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-writeprivateprofilestringa
Retrieval Date: 2026-10-20

Supports:
- Read signature is `(section, key, default, returned_buffer, nSize, filename)`; destination is zero-based argument 3, not argument 4.
- Reads use case-insensitive section/key matching, return the supplied default for missing keys, copy a NUL-terminated truncated value when necessary, and return copied characters excluding the terminator.
- Nil section/key reads enumerate sections/keys using double-NUL termination. This is part of the Windows contract, not a claim that the current bounded Android shim implements enumeration.
- Write signature is `(section, key, value, filename)`, returning nonzero for successful copy and zero for failure. Nil key/value may request deletion in the Windows contract.
- RuntimePack's bounded Android implementation intentionally resolves game-relative INI paths in app-private storage rather than using the Windows directory/registry. It is not full Windows API emulation. See Ruby profile regression tests and save/load plan for the implemented subset and limitations.

### S54 — King Exit Langscore Static Script Translation Keys and Empty-Language Fallback
Reference: Non-executing GPACK/Marshal/Zlib inspection of King Exit `Data/Scripts.rvdata2` (Script 121 Langscore, Script 217 langscore_custom) and `Data/Translate/Scripts.csv` from the retained diagnostic pack; investigation `.superpowers/sdd/2026-10-20-king-exit-save-load-runtime-fix/task-4-reinvestigation.md`.
Retrieval Date: 2026-10-20

Supports:
- Screenshot numbers are literal localization keys `SCRIPT_ID:SOURCE_LINE:SOURCE_COLUMN`, not time values. Examples: `49037624:35:19` maps to `Barang`, `49037624:37:19` to `Gunakan item`, `49037624:67:19` to `Berkas`, `49037624:84:19` to `Pengaturan` in the game's `en` column.
- `LSSetting.load` calls profile read with a 17-byte buffer and assigns `result.slice(0...r)`; old Android stub returns zero, producing empty active language `""`.
- `ENABLE_PATCH_MODE=false` bypasses the patch-mode validation branch; translation lookup for missing language returns the input. Database input is original Japanese; script translation input is a numeric location key, explaining the two observed symptoms together.
- Authored bounded regression `langscore_menu_key_regression_test.rb` exercises actual production shims, historical stub failure and current configured-language success at boot/load. This is host contract evidence, not device-playability certification or proof of integrity of an absent user save.

### S55 — RPG Maker MV/MZ Entrypoint Loader Grammar Accepted by RuntimePack (bounded reader policy)
Reference: `engines/rpgmaker_web/src/detection_arbitration.rs` module contract (`ENTRY_EVIDENCE_BUDGET_BYTES = 64 KiB`, rule IDs `DET-ARB-CONFLICT`, `DET-ARB-EXCLUDED-CORE`, per-family liveness rules); `tests/integration/detection_test.rs`; evidence of the real deployed payload in S58.
Retrieval Date: 2026-10-22

Supports:
- RuntimePack accepts only two static entrypoint shapes for resolving a same-root MV/MZ leftover conflict: (a) a direct literal `<script src="...">` in `index.html` naming an engine core/manager script, or (b) `index.html` referencing `js/main.js` whose literal `scriptUrls` array is actually consumed by the recognised loader loop.
- Evidence is read through the existing bounded ingest reader with a 64 KiB per-file budget for `index.html`/`js/main.js` and for corroboration files (`package.json`, `data/System.json`). Oversize, unreadable or non-UTF-8 evidence is `Insufficient`, never a guess.
- HTML is tokenised rather than regex-scanned: attribute names match exactly (`data-src` is not `src`), quoted attribute values are consumed whole, comments and inert containers (`<template>`, `<textarea>`, `<title>`, `<xmp>`, `<style>`, `<noscript>`) never produce a reference, and only executable script types count.
- `js/main.js` is analysed lexically and conservatively: one `const|let|var scriptUrls` literal array, no reassignment/shadowing, one `for (... of scriptUrls)` loop assigning `.src` inside its own braced body, and the loader must be provably reachable from module top level through a literal bounded call chain with nearest-binding scope checks.
- **Scope limit:** these rules and rule IDs are RuntimePack policy, not an upstream guarantee. RPG Maker MV/MZ core and loader scripts are commercial distribution artifacts; this repository does not redistribute them, and no upstream contract is claimed for shapes the reader rejects. Unsupported loader shapes remain `DET-110` ambiguity rather than a fabricated family.

### S56 — TyranoScript 5 `kag.tag_ext.js` / `kag.tag_system.js` Load Tag Base and Attribute Contract
URL: https://github.com/ShikemokuMK/tyranoscript (repository; see S10)
Reference: shipped runtime of the audited TyranoScript 5 title, read as text only and never executed (an extracted copy retained in the batch port workspace; not redistributed and not part of this repository). Facts also recorded by the batch audit artifact `audit-detector-and-tyrano.md` §2.2–2.3, stored as `_reports/linux-remediation-20261022/audit-detector-and-tyrano.md` (23,224 B, SHA-256 `310bc94a8a963eee3e00f5f97e5fced9dd988e52cdfa7368fbd1ab271049f863`).
Retrieval Date: 2026-10-22

Supports:
- `tyrano.plugin.kag.tag.loadjs` declares `vital: ["storage"]` and resolves storage as `$.getScript("./data/others/" + pm.storage)`, with the module branch `import("../../../data/others/" + pm.storage)`. The base is the document root plus the fixed prefix `data/others/`; it is not relative to the calling `.ks` directory and there is no raw-storage runtime form.
- `tyrano.plugin.kag.tag.loadcss` declares `vital: ["file"]` and builds its stylesheet href from the literal `pm.file` (`href` = the value as given plus a cache-busting query), i.e. it uses the attribute verbatim. It has **no** `storage` attribute and **no** `data/others/` base.
- `tyrano.plugin.kag.tag.loadScenario` in `kag.js` builds `"./data/scenario/" + file_name`, so `call`/`jump` scenario resolution keeps its own base and is unaffected by the loadjs rule.
- Version identity of the audited distribution: `package.json` description `TyranoScript｜ティラノスクリプト Ver5`, `data/system/Config.tjs` `config_version = "5.00"`.
- **Scope limit:** these facts are for TyranoScript 5 as shipped in that audited title, verified by symbol-level reading of the shipped tag definitions. Do not generalise them to every TyranoBuilder/TyranoScript version, and do not treat them as an upstream-published API contract; a different shipped `kag.tag_ext.js`/`kag.tag_system.js` must be re-read as evidence.
- **Known auditor coverage gap (not an upstream guarantee):** the asset auditor currently models a `loadcss storage=` attribute with its legacy two-candidate rule and does not model the real `vital: ["file"]` / `pm.file` form. A missing `loadcss` finding is therefore not proof the reference is unused at runtime, and no CSS adapter is claimed here.

### S57 — Official Ren'Py 7.5.3 RAPT Archive and Source Archive (pinned artifacts)
URL: https://www.renpy.org/dl/7.5.3/renpy-7.5.3-rapt.zip
URL: https://www.renpy.org/dl/7.5.3/renpy-7.5.3-source.tar.bz2
URL: https://raw.githubusercontent.com/openssl/openssl/OpenSSL_1_1_1g/LICENSE (tag `OpenSSL_1_1_1g`)
Reference: batch artifacts `R1-BRIEF.md` §B (13,354 B, SHA-256 `86677c47814b167d5b96f37ef0c32cde8fa6729b5ef4b82bc728f406338a4aff`), `R1-ARTIFACT-ADDENDUM.md` (2,358 B, SHA-256 `72b3161df0089abc3a88fb4354f7281b2864a9a1ca5dec14176afe13bbce630e`), `NOTICE-PROVENANCE-ADDENDUM.md` (9,612 B, SHA-256 `0d7b02759549c9dc055685ef43b35e95a777386822adef0ec086e30d441daf04`) and `T5-RUNTIME-REPORT.md` (9,934 B, SHA-256 `3e9b214a5ffc6cb4e798fdc2c51b1b6a0f6b0e5456e14a5c777d2c49f236ae18`), all under `_reports/linux-remediation-20261022/`; vendored files under `runtimes/renpy-7/**` in this repository.
Retrieval Date: 2026-10-22

Supports:
- RAPT archive: 48,922,348 bytes, SHA-256 `af0f357dffa167f1f0c72ba0c0ba35cd0f2e34ca39f4831fbd6eae8c4c41f89a`; member `rapt/prototype/renpyandroid/src/main/jniLibs/arm64-v8a/librenpython.so`.
- Source archive: 66,160,915 bytes, SHA-256 `76ad915323cd4e77e8b8f1c872ac363927ec701f4ba0175b13db02fa624f5760`; member `renpy-7.5.3-source/LICENSE.txt`.
- Vendored native `runtimes/renpy-7/jni/arm64-v8a/librenpython.so`: 19,456,096 bytes, SHA-256 `4ae92731e5f7a736900d35b94e16d4ebba78c306a6b374436c7928960df1bd53`, ELF64 little-endian `ET_DYN` AArch64, CPython 2.7.18, SDL2 2.0.20, NDK r21d / API 21 per shipped strings.
- Vendored licenses: `licenses/LICENSE.txt` 80,401 bytes, SHA-256 `5cedd397622937915847083e4f9caedda9198e6f660aea30e149e19c2c7b33d9`; `licenses/OPENSSL-LICENSE.txt` 6,121 bytes, SHA-256 `c32913b33252e71190af2066f08115c69bc9fddadf3bf29296e20c835389841c`.
- OpenSSL 1.1.1g is statically linked (no `libssl.so`/`libcrypto.so` in `DT_NEEDED`), so the obligation recorded is retained attribution/notice only; no OpenSSL source offer or relink requirement is invented.
- **Scope limits:** checksums were matched against HTTPS-published values in the signed checksum file; the PGP signature was **not** verified. Exact `renpy-build` revision and upstream source commit remain **UNKNOWN**; the RAPT `hash.txt` value is a payload marker, never a Git revision. Ren'Py 8 parity is unchanged (`e111aa6c02b040019e68f0e36361574d1110b577105de09636864c3a38fed80a`). Runtime/APK/device validation for this payload remains `NOT_RUN`; nothing here claims boot or playability.

### S58 — Deployed Mixed MV/MZ Web Payload Evidence (stale-core MZ shape)
Reference: read-only archive probes of the audited MZ title, recorded as `_reports/linux-remediation-20261022/D1-FIX2-REAL-G06-EXTRACT.log` (639 B, SHA-256 `aeff21464cb7df05f9f264b8a0975459a53218b09cf7bfc80b0e13155ef1ece0`) and `_reports/linux-remediation-20261022/D1-FIX4-REAL-G06-PROBE.log` (2,640 B, SHA-256 `3d9fbb8acb815896d6a28a00a793fe1ed39602cbfd05bd546b69a51232bce1ef`). Byte-identical copies of these two probe logs also exist in the gate working directory. Archive SHA-256 before/after probe identical (`20a5f5ab28ae944a028454337c0c1cd8768bda916a8ca4d7f1f5be7c113a704f`), so the probe did not modify the input.
Retrieval Date: 2026-10-22

Supports:
- Real deployed payload carries both core sets: `js/rmmz_core.js` 173,519 B / `b9e5806516d068a7b82d59374e46d07c82c26abb966fb06878a90615cd3649ca`, `js/rmmz_managers.js` 82,232 B / `3a12371f02a9c22c41d54d0f974bcf4c4f04859a2f2e174b36d1bfe69fe02359`, plus dead leftovers `js/rpg_core.js` 240,820 B / `e9d52fbce06360f00cd3a61a22eaf1c60790c72b6c2c803ac5af9c9be268e7c7` and `js/rpg_managers.js` 83,521 B / `80702499c1885625a5b5d002050b66b6251f8a92d974d7e548a4a42d51ba77a9`.
- Entrypoint evidence: `index.html` 665 B / `3951e1d05885455a0803463b65d3e9f374c8e7957ce0f12a4b65d87f598710af` references only `js/main.js`; `js/main.js` 4,984 B / `cf90cf748c5563e8161eeb479fe6b93e621f7315332c61bd8184f798135a0002`; `package.json` 307 B / `c17abf2b87c9c9af12036f8267b8f462fae27ff5cf7a8d70df689f45339533c0`; `data/System.json` 13,237 B / `a582ec7f147ce9edf0966b1d539b0cef3082cbc974532f8ec4d3a1aea44d52cd`.
- Static resolution of this payload is MZ `EXACT` score 210 with `DET-ARB-MZ-MAIN` plus zero-weight `DET-ARB-EXCLUDED-CORE`; a forced MV selection is rejected with `DET-101` and no product output.
- **Scope limit:** this is evidence about one deployed payload shape and the accepted bounded grammar (S55), not a claim that arbitrary MV/MZ mixtures resolve. No game file is redistributed, and no private filesystem path is recorded here.

## Source hygiene

Implementation agents must not introduce new external claims without adding an Sxx entry.

When a source changes materially:
- record the retrieval date;
- pin repository commits when code becomes a dependency;
- update affected requirement/ADR.
