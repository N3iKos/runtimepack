# Forensic Report: King Exit (RGSS3) Text & Font Rendering Latency Investigation

- **Date:** 2026-10-19
- **Target Game:** King Exit (RPG Maker VX Ace / RGSS3)
- **Status:** **MEASURED EVIDENCE & ISOLATED WORKAROUND (Sections 1–4, user-reported stability); CLI DESIGN PROPOSAL ONLY — NOT IMPLEMENTED — NOT APPROVED (Section 5)**
- **Evidence Reference:** `Docs/REFERENCES.md` (S52)
- **Worktree:** `.worktrees/fix-mvmz-logical-root`
- **Consolidated issue/fix recap:** [King Exit Android — masalah, perbaikan, dan status verifikasi](2026-10-19-king-exit-issues-and-fixes.md).

---

## 1. Context & Problem Statement

- **Title Screen Resolution:** Prior custom title black screen was fixed in-product (`Bitmap.new` and `Cache.normal_bitmap` preserve native `Errno::ENOENT` exceptions, allowing Langscore dynamic localization probes to fail cleanly and load base assets).
- **Observed Action Freeze:** User reported normal performance on Windows PC, but ~1s action freeze on physical phone and MuMu emulator during menu/action transitions; cursor/arrow navigation remains fast.
- **Baseline vs Instrumented Timings:**
  - Baseline uninstrumented MuMu menu SurfaceFlinger submission gaps: **~538–552 ms**.
  - Repeated instrumented menu transitions: **3.4–3.7 s** (elevated timing with possible observer/environment confounders including concurrent emulator processes, scheduler contention, and diagnostic wrappers; not an isolated A/B measurement).
  - Automated QA battle testing: historical automated QA suite remains untested for battle.
- **Data Boundary Note:** Input source game was deleted in a separate incident; container content (`assets/game.pack`) hash `7b23df6bcd1ba5ff26dcaf8917ee482d866621aaeeff05550d197edbbf05bf08` remains unchanged inside the pack.

---

## 2. Text Attribution & Boundary Isolation

In a representative 3,515.21 ms freeze window (`wp_rep1_01_menu_open`):
- **41 `Bitmap#draw_text` calls:** 3,121.92 ms disjoint runtime.
- **24 `Bitmap#text_size` calls:** 233.95 ms disjoint runtime.
- **Combined text rendering union:** **3,355.87 ms (95.47% of window)**, correcting earlier misattributed 96.9% / 75% claims.
- **Sampled-negative observations in measured window only (not universally ruled out):** In the sampled menu transition window, screen blur / snapshots totaled 0.80 ms and missing-file probes (`ENOENT`) were 0.00 ms (0 misses). In separate disposable baseline control trials, canvas allocation and native `fill_rect` executed in <0.15 ms.
- **Measured Boundary:** Measured strictly at the native method boundary: 41 `Bitmap#draw_text` (3,121.92 ms) + 24 `Bitmap#text_size` (233.95 ms) account for 3,355.87 ms (95.47% of freeze gap). The 41 draw calls comprise 24 single-character calls (len=1, ~25 ms avg) and 17 multi-character string calls (len>1, ~148 ms avg, multi-word dominant). Internal native rendering stages (FreeType rasterization, outline generation, software blending, texture uploads) are hypothesized from mkxp-z reference code but remain unverified internal provenance in this prebuilt binary.

---

## 3. Discriminating Microbenchmark Findings (S52)

Isolated microbenchmarks on disposable bitmaps (referenced from transient host/emulator diagnostic artifacts at `Temp/opencode/mumu_text_microbench_qa_20261019_181648`, not repository-portable paths) confirmed native C bindings (`source_location` is `nil` under alias):

| Test Scenario | VL Gothic (`outline=true`) | VL Gothic (`outline=false`) | Source Han Sans CN (`outline=true`) |
|---|---|---|---|
| Single char `'A'` (warm median) | **28.52 ms** | **0.23 ms** (124x faster) | **2.92 ms** (9.7x faster) |
| `"Sample text"` len=11 (warm median) | **232.09 ms** | **0.31 ms** (748x faster) | **5.93 ms** (39x faster) |

- **Font Files:**
  - Original `VL Gothic`: 3,883,852 bytes, SHA-256 `7bc537ad07905dfdb6296d7e30b205fd71b8d6159702f4ead167cab3fb9032f6`. Hashes are identical across distribution pack copies (distribution copy consistency, not TTF structural validation).
  - Alternative `Source Han Sans CN`: bundled `Fonts/SourceHanSansCN-Regular-2.ttf` (container-relative path in `game.pack`), 8,364,840 bytes, SHA-256 `14ec9d09dad83098938370be17170a747d63084022b05710e3d34b4ddb359091`.
- **Scope Limit:** Measurements reflect isolated MuMu execution; exact internal causes (FT_Stroker allocation, glyph cache hash misses, Bezier complexity, GPU queue state) remain unproven on physical hardware.

---

## 4. Native Prebuilt & Isolated Workaround Candidate

- **Native Runtime Prebuilt:** `libmkxp-z.so` (SHA-256 `df8dc305b44206da7134d6b4ff9dc34f07ff0c63cb10a4d47f763d6a2d1123db`, Build ID `8550ebc98e9e2191460d9a386daaf7cf58431b91`, reported commit `c935d99d7cf2016289302412d708641d52d2f7ee` from metadata alongside binary hash/build ID). Matching buildable source is unverified; the embedded commit metadata does not establish build provenance, and no native patching is claimed.
- **Native `fontSub` Handling:** Reverse engineering confirms `SharedFontState::SharedFontState` parses strings with single separator `>` (`FROM>TO`, e.g. `"VL Gothic>Source Han Sans CN"`) from `mkxp.json` and `SharedFontState::getFont` performs single-pass substitution without chaining (aliases pointing to `VL Gothic` do not transitively map to `Source Han Sans CN`).
- **User-Consented Candidate Artifact:**
  - File: `kingexit_diagnostic_fontfix.apk` (transient host/emulator artifact directory `Temp/opencode/mumu_font_workaround_20261019_190949`), SHA-256 `4630f9ed3a4deade9914ca355bca7d30ba465e589ed0c504767289f2b9cde862`, package `io.runtimepack.game.rpgmkingexit.diagnostic.fontfix` (beta/debug flavor).
  - Scope: `GpackExtractor.kt` injected only `"VL Gothic>Source Han Sans CN"`. Retains `outline=true`, bitmap error preservation, zero timing hooks, and unmodified `game.pack`.
  - User-Reported Manual Testing: user explicitly tested dialogue, menus, battle scenes, pause, and save/load in candidate APK and confirmed stability (*"udah aku uji udh stabil"*, *"save load aman terkendali"*). **Correction 2026-10-20:** Subsequent screenshots and user PC/APK comparison expose post-load localization failure (Japanese database text and numeric Langscore script keys). Prior save/load-safe conclusion is withdrawn; profile/localization repair is separate from font latency and awaits new user APK validation. Pause/resume coverage is strictly in-game pause, not process restart or app lifecycle. No quantitative post-change QA counts or automated regression runs were captured; candidate is strictly an isolated spike, distinct from formal automated QA.

---

## 5. Architectural Proposal: Font Policy Design (PROPOSAL NOT IMPLEMENTED NOT APPROVED)

### 5.1 Proposed CLI Design (RECOMMENDED REQUIREMENTS FOR PROPOSAL ONLY)
- Proposed universal flag: `--font-policy original|compat|custom` (naming illustrative; PROPOSAL ONLY, not implemented, not approved; user discussion ongoing regarding universal flag complexity).
- **Default Policy (`original`):** Preserves game-requested font declarations in configuration without extra performance substitutions. Existing runtime fallbacks for missing fonts and engine default alias mappings remain active; no original pixel-identical GDI rendering promise.
- **Policy `compat`:** Applies tested, licensed bundled font profile substitutions for known pathological rendering cases. Initial backend support recommended for RGSS only; other backends explicitly targeted with `compat` or `custom` should reject with a clear diagnostic code rather than silently ignoring the setting.
- **Policy `custom`:** Allows user-specified overrides:
  - `--font-file <relative_path>` to supply a font asset.
  - Repeatable `--font-map "FROM=TO"` to map specific font family names.
  - Font validation must inspect actual font family metadata inside TTF/OTF tables, not rely on filename matching.
  - Substitutions apply strictly to exact matching font family names, prohibiting indiscriminate all-font force overrides.
- **Precedence & Config Merging (Open Design Question):** The interaction between packager policies, custom CLI maps, and pre-existing game/engine `fontSub` entries remains an open design topic; explicit user configuration must be prioritized without silent overrides.
- **Packaging & Provenance:**
  - Font redistributability and license coverage (e.g. SIL OFL / Apache 2.0) must be validated prior to packaging any replacement font.
  - `BuildManifest` should record font provenance: selected policy, applied family mappings, font file hashes, license ID, and runtime version.
  - Normal packager workflow preserves game source immutability (noting the prior historical external incident without claiming untouched source disk state).

### 5.2 Trade-offs and Risks
- **Glyph Coverage & Locales:** CJK fonts such as `Source Han Sans CN` are not universally safe across all locales (e.g. Japanese Kanji variants, Arabic RTL, or Thai complex script rendering).
- **Layout & Metrics:** Font metric differences risk text clipping, dialog box overflow, or misaligned icon/font-sprite systems.

### 5.3 Acceptance Criteria for Future Implementation
1. CLI argument parser validates `--font-policy` options and rejects unsupported engines with clear diagnostics.
2. Default packager invocations produce identical legacy runtime configuration without injected substitutions.
3. Emitted `mkxp.json` contains exact verified `fontSub` strings matching resolved policy.
4. `BuildManifest` records provenance: policy name, applied mappings, font file hashes, license, and runtime version.
5. Future verification requires uninstrumented testing covering boot, dialog, menus, battle, save/load, and pause/resume under identical test conditions.
