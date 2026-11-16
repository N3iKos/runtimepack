# B-Prison (g03) — Shipped-Payload Consistency Plan

- Date: 2026-10-25
- Status: **COMPLETE** — implemented and committed as `72ff669`
  (`fix(web): judge the shipped payload and stop shipping Windows binaries`, 2026-10-25 10:27:51 +0700).
  Final numbers: `cargo test --workspace --no-fail-fast` = 1293 passed / 0 failed / 5 ignored /
  43 suites; Kotlin `:app:testDebugUnitTest` = 19 suites / 236 tests / 0 failures. g03 built:
  APK `g03-BPrison-beta-linux-20261025.apk`, 1,471,989,809 bytes, sha256
  `d149266ed5075408eb5ea5b0ffae33dbc4b47fb474b008126bbe18db49185092`, 0 blocked dispositions.
  See §7 for the outcome and remaining follow-on items.
- Base: `a1c1045` (runtime identity fix, already committed)
- Goal: produce a B-Prison beta APK **only if** the build gate is satisfied honestly.

## 1. What the investigation actually found

Not a policy gap first — an **inconsistency**:

- `pack/src/gpack.rs::should_include_in_pack` already excludes `steam_api*.dll`
  (line 128-129) and other Windows runtime DLLs. Those files are **not shipped**
  in `game.pack`.
- The audit does not use that decision. `asset_audit.rs:1298` and
  `scanner.rs:278` analyze `context.payload()`, which is only the content-root
  scoped index (`analysis_context.rs:42-58`) — no packaging exclusions applied.
- Result: the auditor blocks the build over `lib/steam_api.dll`, a file the
  packer was never going to ship.

So part of the 7 blockers is a **false positive by construction**: the report
describes the source tree, not the artifact.

The other part is genuine: `sdkencryptedappticket*.dll` and `greenworks-*.node`
are **not** excluded today, so they really would ship, and they are Windows
binaries that cannot execute on Android.

## 2. Design principle (general, not per-game)

> The audit must judge the payload that will be shipped, and Windows-native
> binaries that cannot execute on Android must not be shipped at all.

Two consequences, both general:

1. **One definition of "shipped payload".** The audit, the scanner and the
   packer consume the same exclusion decision. No per-engine, per-title or
   per-hash rule.
2. **Windows-native binary exclusion is a platform fact**, not a capability
   claim: a PE `.dll` or a Windows `.node` cannot run under Android. `.so`
   stays hard-blocked (it *can* run), and JS/asset files are never excluded by
   this rule.

## 3. What this deliberately does NOT do

- It does **not** delete or modify the game folder. Exclusion happens at pack
  time only; `packaging-exclusions.json` records every dropped path with a
  reason, and the source tree is untouched.
- It does **not** waive `CMP-104`, path safety, signing, toolchain, integrity,
  or `.so`. Only structurally-Windows binaries leave the shipped set.
- It does **not** make an excluded file "proven not required". Exclusion means
  "not shipped", and the report must say exactly that, plus the runtime risk:
  a game that checks for the file's presence will fail at runtime.

## 4. Tasks

### Task 1 — Single shipped-payload view

**Files:** `core/src/build_plan/*` (context construction), `engines/rpgmaker_web/src/analysis_context.rs`, `engines/rpgmaker_web/src/asset_audit.rs`, `engines/rpgmaker_web/src/scanner.rs`, `pack/src/gpack.rs`

**Ruling (2026-10-25): Option B — keep the analyzed payload intact; route pack-excluded paths to disclosure.**

Pruning the payload before analysis was rejected after measuring it: `desktop_components.rs:98` anchors NW.js boundary detection on `nw.dll` / `nw_elf.dll` / `node.dll`, and the packer excludes all three. Pruning would make `BoundaryState::NotProven` universal, collapsing every `CMP-106` classification and turning all runner inventory hard. It would also invert an existing pinned contract (a runner-named DLL *inside* the payload is hard regardless of basename) and require rewriting 15 tests.

Chosen design: the analysis graph and boundary detection keep seeing the full payload; native findings whose path the packer will not ship are replaced by one non-blocking disclosure finding per path, stating the path, the exclusion reason, and that the artifact does not contain it.

- RED: fixture with `lib/steam_api64.dll` (pack-excluded) produces no blocking native finding and exactly one disclosure naming it.
- RED: a `.so` still blocks (never pack-excluded).
- RED: a payload-internal runner DLL (`lib/node.dll`) still produces its existing hard finding — boundary detection unchanged.
- RED: `CMP-106` desktop-component classification is unchanged for a normal wrapper.
- Implement the disclosure projection in the native scan path; the exclusion decision comes from the packer's single function.
- Disclosure severity `INFO`, non-blocking, never suppressed, listed in the build report.
- No change to `.so`, path safety, signing, toolchain, or integrity semantics.

### Task 1b — Extend disclosure to pack-excluded `.node` addons

**Files:** `engines/rpgmaker_web/src/shipped_payload.rs`, `scanner.rs`, `asset_audit.rs`, affected tests

Phase 1 applied the disclosure projection to `.dll` only. The same reasoning applies to
Windows `.node` images, which Task 2 now drops from the pack: an artifact that does not
contain the addon cannot load it.

- RED: `lib/greenworks-win64.node` (PE, pack-excluded) yields a `CMP-111` disclosure and no blocking `CMP-103`.
- RED: a shipped non-Windows `.node` keeps its activation verdict (`CMP-103` or `CMP-109`).
- The 20 Class A PE-import fixtures that broke must be renamed to paths the packer ships,
  because their subject is the PE-import companion logic, not the exclusion list. Every
  rename is documented per test as intentional; no asserted logic is changed.


### Task 2 — Complete the Windows-native exclusion set

**Files:** `pack/src/gpack.rs`, `tests/integration/build_test.rs`

- RED: `lib/sdkencryptedappticket.dll`, `lib/sdkencryptedappticket64.dll`, `lib/greenworks-win32.node`, `lib/greenworks-win64.node` are currently shipped.
- RED: a `.node` built for Linux/Android must NOT be excluded (structural check, not filename-only).
- Implement: exclude Windows PE binaries and Windows `.node` images, verified by structure where feasible; keep `.so` and non-Windows `.node` shipped.
- Exclusion entries land in `packaging-exclusions.json` with a reason and are listed in the build report.

### Task 3 — Reassess g03 with a rebuilt CLI

- Rebuild the CLI (force a real rebuild so provenance is attributable), record SHA256.
- `inspect` + `audit` g03.
- Expected remaining blocker if Tasks 1-2 land: `CMP-104` on `js/plugins/OrangeMapshot.js` only.
- That finding is **not** covered by Tasks 1-2: it is JavaScript in the payload, and excluding JS would be excluding game logic. Do not exclude it.

### Task 3b — Classify parse failure honestly (approved-spec alignment)

**Files:** `engines/rpgmaker_web/src/activation.rs`, `tests/integration/web_asset_audit_test.rs`, `engines/rpgmaker_web/src/classify.rs` tests

This is **not** a new policy. The approved capability spec
(`2026-10-23-capability-compatibility-policy-design.md` §4) already states:

> `Blocked` hanya sah bila ada bukti positif ketidaktersediaan, bukan sekadar
> kegagalan analisis.

Today the analyzer violates that for one specific case. `activation.rs:3135-3143`
(and the tokenizer failure branch at `:3088-3100`) marks **any** structural parse
failure as `integrity_taint`, which forces `IntegrityFailure` → `Blocked`
(`activation.rs:3479-3481`, `:3784-3786`). That is "analysis failed", not
"proven unavailable".

B-Prison's `js/plugins/InfoWindow.js` is such a file: unparsable, not listed in
`plugins.js`, and referenced by no literal edge. One broken file the game never
provably loads currently blocks the entire corpus.

**Sound distinction to implement:**

| Case | Class | Why |
|---|---|---|
| Input unreadable (I/O) | integrity failure → `Blocked` | We do not know what is there at all |
| Parse failure of a file the manifest/literal graph **proves** is loaded | integrity failure → `Blocked` | It runs, and we cannot analyze it |
| Parse failure of a file with **no proven load edge** | unmodelled route → `Unknown` | It may be reached dynamically; that is unproven, not proven bad |

Requirements:

- I/O read failures stay `Blocked` unconditionally.
- A parse failure in a file proven loadable (manifest entry, `index.html` script,
  or a resolved literal `require`/`loadScript`) stays `Blocked`.
- A parse failure with no proven load edge is recorded as an unmodelled route of
  that module and yields `Unknown` — never `Advisory`.
- Diagnostics keep naming the file and the reason; nothing is silently dropped.
- No change to `Utils` mutation, dynamic-code, `.so`, native-code or security
  handling.
- RED tests: (a) manifest-registered unparsable plugin still `Blocked`;
  (b) `index.html`-scripted unparsable file still `Blocked`; (c) transitively
  required unparsable file still `Blocked`; (d) unregistered, unreferenced
  unparsable file becomes `Unknown` (not `Blocked`, not `Advisory`);
  (e) I/O-unreadable file stays `Blocked` regardless of reachability.

Expected effect on g03: `CMP-104` on `OrangeMapshot.js` moves from `Blocked` to
`Unknown`, which `--allow-unknown` may forward into a labelled experimental
build. The native `CMP-102/103` findings must already be resolved by Tasks 1-2;
if any remain `Blocked`, g03 still does not build.

### Task 4 — Decide the OrangeMapshot finding (separate decision)

Options, in order of soundness:

- **4a. Supply the missing guard-precondition evidence.** The analyzer already
  models `Utils.isNwjs()` guards (`activation.rs:1639-1680`) but marks a guarded
  site `Unknown` because it lacks shim-hash and core-completion-trailer evidence
  (`activation.rs:3806-3824`). We just implemented that trailer. Wiring its hash
  into the analysis is the honest path to `Advisory`.
- **4b. Leave g03 blocked.** No APK, no waiver.
- **4c. Exclude the JS file.** Not recommended: it is game logic, and
  "unregistered" is not "unreachable" while dynamic routes exist.

### Task 5 — Build and validate (only if zero non-waivable blockers remain)

- `--flavor beta`, unique output dir, package `io.runtimepack.linux20261022.g03`.
- Validate `apksigner`, `aapt2 dump badging`, `unzip -t`, notice presence, CRC.
- Report as experimental / not device-validated. **No playability claim.**
- Preserve every existing artifact; never overwrite.

## 5. Risk register

| Risk | Mitigation |
|---|---|
| Exclusion becomes a hiding mechanism | Only structural Windows binaries; `.so`, JS and assets never excluded; every drop recorded and reported |
| Game checks for a dropped file's presence at runtime | Disclosure finding + APK labelled experimental; device test required |
| Audit/pack drift returns | Single shared exclusion decision, tested by a fixture where pack and audit disagree |
| Silent behaviour change for the 16 existing APKs | Full workspace suite + re-audit g07/g09; findings must not change |

## 6. Approval required

Task 1 and 2 change **what gets blocked**, so they need explicit approval even
though the reasoning is a consistency fix rather than a waiver.

Task 4a is the honest route to an APK; Task 4c is not proposed.

## 7. Outcome (2026-10-25)

Committed as `72ff669` on `fix/linux-batch-compat-20261022`, on top of `a1c1045`.

- **g03 built.** `--flavor beta --allow-unknown` only (no `--strict-audit`, no
  `--accept-audit-fail`, no `--accept-audit-unknown`). Package `io.runtimepack.linux20261022.g03`,
  1,471,989,809 bytes, sha256 `d149266ed5075408eb5ea5b0ffae33dbc4b47fb474b008126bbe18db49185092`,
  debug-signed, `apksigner` v2 "Verifies", `unzip -t` clean with 131 entries.
- **0 blockers.** 0 `Blocked` dispositions and 0 legacy hard blockers. 4 `.dll` → `CMP-111` Info and
  2 `.node` → `CMP-112` Warning (disclosed, not shipped). The one remaining error is `CMP-104` on
  `js/plugins/OrangeMapshot.js` (class `Unknown` + `Required`, proof state `unknown`, offset 19703),
  forwarded by `--allow-unknown`.
- **Residual:** 4 MSVC link-time `.lib` import archives (~0.6 MB total: `lib/steam_api.lib`
  297,550 B, `lib/steam_api64.lib` 294,986 B, `lib/sdkencryptedappticket.lib` 4,542 B,
  `lib/sdkencryptedappticket64.lib` 4,554 B) remain packed. They are not PE images, contain no
  executable code, are not loadable by any runtime loader, and are not in
  `packaging-exclusions.json`. Not excluded deliberately: a new exclusion rule for zero functional
  gain. Documented as a known minor imperfection.
- GPACK index is authoritative and clean: magic `GPACK1`, 18,055 entries == declared `file_count`,
  0 entries ending `.dll`/`.node`/`.exe`/`.so`. Raw substring hits for `steam_api.dll` (~1725) are
  text inside other files (e.g. `greenworks.js` require paths), not packed entries.
- Game input untouched: `/home/mhsymr/g03_fast` still 18,185 files / 1,627,531,159 bytes, all 6
  excluded files physically present.
- Regression: g07/g09 blocking verdicts unchanged versus `run-20261024T191853Z` (g07 byte-identical,
  g09 identical counts with two WARN lines reordered).

Follow-on items, none of them introduced by this change:

1. **Corpus-level taint carrier when a game has no native/plugin file.** Pre-existing and unrelated to
   this change; a corpus whose only taint evidence would be a native file has no carrier once that
   file is legitimately not shipped. Needs its own investigation before the next corpus-wide audit.
2. **`COMPAT-REGISTRY.md:292`** still carries a stale description of the pre-fix behaviour. Docs-only
   correction, not yet applied.
3. **A device test is the next real gate.** Nothing here proves g03 boots, renders, takes input, plays
   audio, or saves. `runtime_validation` is NOT_RUN; the signing key is the Android Debug key. The APK
   is experimental and not device-validated, and no playability claim is made.

