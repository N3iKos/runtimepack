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

**This source creates `TYRANO-LEGAL` gate.**

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

## Source hygiene

Implementation agents must not introduce new external claims without adding an Sxx entry.

When a source changes materially:
- record the retrieval date;
- pin repository commits when code becomes a dependency;
- update affected requirement/ADR.
