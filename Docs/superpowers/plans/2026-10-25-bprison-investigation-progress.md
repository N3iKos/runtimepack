# B-Prison (g03) investigation — durable progress ledger

Created: 2026-10-25
Status: **FINAL. All gates closed. Bounded runtime fix implemented and committed (`a1c1045`), final
scoped independent re-review PASS (`runtime-handoff/FINAL-REVIEW.md`), final QA green. g03 remains BLOCKED
with 7 hard findings (4 `CMP-102` native Steam libraries, 2 `CMP-103` native greenworks addons, 1
`CMP-104` `child_process`), audit FAIL, `runtime_validation` NOT_RUN, no APK built, no waiver flags used,
no device run. The port is not complete and playability is not claimed.**
Supersedes the B-Prison decision framing in `2026-10-24-tahap4-coffin-bprison.md` Part C for planning
purposes only; that file stays as the historical record and is not edited by this ledger.

Final outcomes by gate: (1) investigations closed; (2) plan approved; (3) TDD implementation + independent
review closed (final scoped re-review PASS); (4) regression closed with exact counts; (5) fresh policy
assessment closed — g03 stays blocked, no build attempted; (6) honest report closed. Details in "Final gate
results" below.

Residual non-blocking items carried forward: `COMPAT-REGISTRY.md:292` still describes only the applier
call (deferred, owner action); a retained pending handoff with an unpinnable `Utils` may emit one `WEB-029`
failure diagnostic per poll tick for ~10 s (up to ~1000 ticks), which can evict older diagnostics — no log
throttle is implemented; sticky `platformIdentityActive` after a post-pin `Utils` replacement is pre-existing
behaviour, not introduced by this fix, and untested on device.

## Authorization and working agreement

- User authorized continuing the B-Prison work to **verified completion**, with honest stop conditions.
- User execution scope explicitly permits the **minimal runtime fix required** to remove the confirmed
  ordering defect. It does **not** permit a policy or security-scope change. Because the fix is bounded,
  no additional approval prompt is required before implementing it.
- User prefers the main agent as **supervisor**, delegating independent investigations, planning,
  implementation and review to sub-agents rather than doing the work inline.
- No game input edits, no deletions, no mock success, no relaxation of security gates.
- No merge, no push, no commit from this work stream.
- Toolchain: WSL (`wsl -e bash -lc`), cargo inside WSL. Existing worktree, existing cargo target.
- Candidate build flag `--flavor beta` may be used **only after eligibility is verified**, never assumed.
- No APK is promised. A build artifact is produced only if the gated sequence below reaches it with
  no non-waivable blockers, and it is reported as experimental / not device-validated.

## Workspace and base

- Worktree: `D:\02_Development\Projects_1\runtimepack-vnm-reconstruction\runtime-v2`
- Base commit at start of this stream: `77d077e`
- **Final HEAD: `a1c1045`** — `fix(web): pin platform identity at the core fence regardless of shim order`
  (branch `fix/linux-batch-compat-20261022`, committed 2026-10-25 03:53:58 +0700). The earlier "no commit"
  working rule applied to the investigation/implementation passes; the bounded fix was subsequently
  committed as `a1c1045`, which contains exactly the 4 owned source/test files plus the 2 plan documents.
- **Correction to an earlier claim:** `runtime-v2` is **not** unversioned. WSL git verified it as a
  linked worktree whose tracked source is at HEAD `77d077e`. No source file is modified by this work
  stream.
- Pre-existing tracked modifications, present before this stream and not owned by it: `AGENTS.md`
  (mtime 2026-10-23, replaced by the short reference stub) and `memory/progress.md`
  (mtime 2026-10-22). **Preserve both untouched.**
- Unrelated untracked reports, plans and `R/` may be present. **Preserve all of them.** Only this file
  is owned by this task.
- Canonical evidence/report directory (outside the tree):
  `D:\02_Development\Raw-Games\Sample\port_apk\_reports\linux-remediation-20261022\`
- B-Prison report directory for this work stream (canonical, matches every evidence reference below):
  `D:\02_Development\Raw-Games\Sample\port_apk\_reports\linux-remediation-20261022\bprison-20261025\`
  (QA run `final-qa-20261024T202057Z\`, executor `runtime-handoff\`, `FINAL-REPORT.md`).
- Disk headroom: D: 5.2 GB free, C: 69 GB free, WSL 919 GB free. Avoid unnecessary copies of the
  worktree, cargo target, or APKs; write logs to the report directory above.

## Verified facts carried in

- A fresh `inspect` on the **existing debug binary** reproduces **7 ERROR** findings with proof
  `blocked`, caused by **global parse-integrity taint** (the `InfoWindow.js` parse failure taints the
  whole audit), plus unresolved `eval` and unresolved bundle edges.
- Debug binary mtime: `2026-10-24`. Source HEAD: `77d077e`. Binary and source are not guaranteed to
  correspond; any behavioral claim must state which artifact produced it.
- Analyzer guard modelling **already exists**; it is not a missing feature. Any statement that guards
  are unmodelled is invalid.
- The Browserify recognizer **exists in the analyzer but is not used by activation**. It therefore does
  not currently contribute to runtime classification.
- **No small sound analyzer patch yields native dormancy while `eval` stays active.** Dormancy cannot be
  produced by a narrow analyzer-side change without weakening an active evaluation path.
- Independent design verdict: **preserve the native hard gates**. Forwarding typed-native `Unknown`
  results would create **new policy/security scope, which is NOT authorized**. This is rejected, not
  deferred for convenience.
- Boot scout (read-only, static): optional `OrangeGreenworks` stubs supply optional achievements
  (7 call sites, `CE950`–`CE956`); `startMap39`; **no static Steam boot requirement**. `OrangeMapshot`
  is unregistered and has **no observed literal inbound reference**, but it is **NOT proven dead**
  because of dynamic `eval`. This is absence of evidence, not evidence of deadness.

## Rejected assumptions — do not carry forward

These were considered and are recorded as **REJECTED** so they are not silently reused:

1. **"Unused files can be treated as dormant."** Rejected. An unreferenced file under dynamic `eval`
   is not provably unreachable. This was the basis of the earlier `OrangeMapshot` reasoning and is not
   sound.
2. **"Forward native `Unknown` to unblock g03."** Rejected. It changes policy and security scope, which
   is outside the authorized execution scope.
3. **"A small analyzer patch can make the native path dormant while `eval` stays active."** Rejected;
   no such sound patch exists.
4. **"`runtime-v2` is unversioned."** Rejected; it is a linked worktree at tracked HEAD `77d077e`.
5. **"Move the whole HTML shim earlier to fix ordering."** Rejected. The initial patch pass needs the
   engine managers to already exist; hoisting the shim in HTML breaks that contract.

## Invalidated claims — do not carry forward

These earlier statements are recorded here as **INVALID** so they are not silently reused:

1. "Guard modelling is absent in the analyzer." **Invalid.** Guard modelling exists and must be
   inspected before any claim about it is made.
2. "`eval` taint is module-local and therefore safe." **Invalid.** The observed behaviour is global
   parse-integrity taint; module-local scoping was an unproven assumption.
3. Consequently, the previous plan algorithm for B-Prison (per-subject closure + guard modelling +
   per-module eval scope, applied as if all three were simply missing) is **not** carried forward.
   Any new design must be derived from fresh evidence, not from that algorithm.

## Closed investigations

| Session | Scope | State |
|---|---|---|
| `ses_e90c09a63ffezZQFPB1nfB8L1o` | Analyzer / taint and closure semantics | **closed** |
| `ses_e90b9223fffeZMsQZV0EtEhQB1` | Runtime ordering (shim before core) | **closed — defect confirmed** |

Findings are claims until independently validated; each records the artifact and commit it was derived
from. Session references are retained for traceability.

### Analyzer investigation outcome

- Guard modelling present; Browserify recognizer present but not wired into activation.
- No small sound analyzer patch produces native dormancy with `eval` active.
- Conclusion: the 7 ERROR findings are **not** removable by an analyzer-only change within authorized
  scope.

### Runtime-ordering investigation outcome

- A **source defect is confirmed**: the web shim is applied after core evaluation, so
  `Utils.isNwjs()` is already `true` while platform identity is not yet active.
- Reproduced in an **isolated synthetic Node harness**, no game code and no device execution:
  - core-first: `isNwjs` = `true`, active = `false`
  - shim-first: `isNwjs` = `false`, active = `true`
- This is synthetic evidence of an ordering defect, **not** device or gameplay evidence.
- Approved original spec `Docs/superpowers/specs/2026-10-10-web-platform-external-url-design.md`
  lines 129–174 already specified a **synchronous inline `Object.defineProperty` fallback** inside the
  core trailer. Production **omitted** that fallback. The omission, not the spec, is the defect.

## Planned remedy (bounded, order-independent) — implemented as specified in `a1c1045`

Restore the specified behaviour in the smallest scope that satisfies the spec. This is a **pending
handoff plus inline core-trailer fallback**, not a pending-only handoff:

- **Core trailer fallback restored.** The core trailer keeps the inline synchronous
  `Object.defineProperty(Utils, "isNwjs", …)` fallback exactly as specified in spec lines 143–166. When
  no applier is available yet, the trailer records a **pending identity handoff** instead of leaving
  `Utils.isNwjs` at the native `true`.
- **`Utils` stays `false` at the core fence.** This must hold even before the later shim or any
  intermediate script runs, because those may probe `isNwjs` before identity is adopted.
- **Canonical shim adopts when ready.** The shim drains the pending handoff **before** patch work and
  again **on re-entry**, and only then marks identity active. Adoption requires an active proof and a
  cleared pending handoff.
- **No early HTML move.** The `before-main` injection position and the VNM path stay unchanged.
- **No new mocks or stubs.** No new `require` semantics, no poll-based recovery, unless a test proves
  it necessary; neither is currently planned.
- **Existing native hard gates unchanged.** `Blocked` remains non-waivable; `--allow-unknown` forwards
  only `Unknown`.

## Ownership for the implementation step

| Session | Role | Scope |
|---|---|---|
| `ses_e90a8d83cffea7dckKif5f0E4r` | Executor | 4 files: Kotlin interceptor, JS shim, and their corresponding tests |
| `ses_e90b055e6ffehSykho00OGm9ML` | Planner | revising the smallest-scope plan |
| `ses_e90aad6b9ffe1S4huG9YBESiwG` | QA | baseline |

Implementation constraints (as applied during the TDD pass; the bounded result was later committed as
`a1c1045`):

- No commit, no merge, no push.
- Only the 4 owned files are edited; no other source files.
- Tests must execute the **actual Kotlin trailer text** inside a **synthetic JS sandbox**. No game JS is
  loaded and no game artifact is executed.
- Required test case: core → real trailer → intermediate script probing `require`/`process` (expect
  `false`) → late shim, and a failed pin must **not** report a false active state.

## Environment and toolchain

- WSL toolchain present: JDK 17, Android SDK at `/home/mhsymr/Android/Sdk`,
  `GRADLE_USER_HOME=/root/.gradle`.
- Gradle **8.7 is cached**, but **test-run availability is not yet proven**. Any Gradle/Android test
  claim must be backed by a log from an actual run.

## Gated sequence (each gate must close before the next opens)

1. **Independent investigations** — closed for analyzer and runtime ordering. Evidence-backed and
   artifact-identified.
2. **Concrete smallest-scope implementation plan** — derived from (1), with explicit files, RED tests,
   and stated non-goals. Planner revising; supervisor reviews.
3. **Executor TDD fixes + independent review** — RED observed before each behavior change; no
   assertion weakened; independent reviewer separate from the implementer.
4. **Regression** — targeted suites for the touched areas, then full workspace suite: cargo, Android JS
   and Kotlin suites. Exact counts recorded with log paths.
5. **Fresh policy assessment** — re-run `inspect`/audit with **provenance** stated. Build artifact only
   if no non-waivable blocker remains. `--flavor beta` only after eligibility is verified.
6. **Honest report** — completed items separated from blocked items; no claim of gameplay, device
   boot, audio, or save/load validation.

## Publicly announced next scope

Announced before implementation, so the boundary is reviewable:

- If the defect is independently confirmed, the executor fixes **only** the `transformWebHtml`
  ordering defect plus its Kotlin/JS synthetic tests, preserving VNM behaviour and idempotence.
- No new mocks, no new `require` semantics, and no poll-based recovery unless strictly necessary.
- Independent review by a session separate from the implementer.
- Full cargo, Android JS, and Kotlin regressions with exact counts.
- Fresh CLI/audit run with provenance recorded.
- If the 7 blockers remain, the final outcome is: **runtime remediation verified, g03 BLOCKED, no APK,
  port not complete.** That is an acceptable honest outcome and must not be reframed as success.

## Final gate results

All gates below are closed. Evidence root:
`D:\02_Development\Raw-Games\Sample\port_apk\_reports\linux-remediation-20261022\bprison-20261025\`
(QA run dir `final-qa-20261024T202057Z/`, executor dir `runtime-handoff/`).

### Gate 3 — implementation and independent review (closed)

- Commit **`a1c1045`** `fix(web): pin platform identity at the core fence regardless of shim order`
  (branch `fix/linux-batch-compat-20261022`). 6 files, `+1104/-17`: shim `+66/-6`,
  `VfsResourceInterceptor.kt` `+34/-6`, `VfsResourceInterceptorTest.kt` `+156/-0`,
  `web_shims.test.js` `+338/-5`, plus 2 plan documents.
- The fix is the planned one: the core-completion trailer pins `Utils.isNwjs` with a configurable
  accessor when the canonical applier is absent, records `window.__runtimepack_pendingIdentity`, and the
  shim drains that pending handoff (after the applier is defined and again on re-entry), clearing it only
  after a verified pin. `platformIdentityActive` is set only after a verified pin.
- TDD honoured: RED observed before each behaviour change (`red-js-web_shims.log`, `red-kotlin-vfs.log`,
  `red2-js-marker-touch.log`, `red2-kotlin-marker.log`, `fix-red-js.log`, `fix-red-js-l3.log`). No
  assertion weakened; the hand-copied trailer snippet in the JS test was replaced by an extractor that
  reads the real Kotlin trailer (strictly stronger, and it fails loudly if the trailer degrades).
- **Fix-pass independent review verdict: APPROVED. No High findings.** Remaining review items were handled
  in the same commit `a1c1045`: **M1** (documentation / CI-wiring note that `app/src/test/js` is not wired
  into Gradle or CI, so JS proof is a manual `node` run reported with its exit code) and **L1, L2, L3, L5**
  (extractor guard fails loudly; Layer-4 poll retry restored without a dishonest `platformIdentityActive`;
  throwing pending assignment cannot skip verification; Kotlin test hygiene). Record:
  `runtime-handoff/IMPLEMENTATION-REPORT.md`, section "Fix pass — review findings L1, L2, L3, L5, M1/I3".
- **Final scoped independent re-review: PASS** (`runtime-handoff/FINAL-REVIEW.md`, 2026-10-25; reviewer
  read-only, no build/test/source edit). The reviewer re-verified the committed blob hashes against the
  regenerated `implementation.diff`, confirmed the reviewed diff, the committed tree and the tree QA froze
  describe the same content, and found L1, L2, L3, L5a, L5b addressed, M1 addressed, L5c retracted as the
  reviewer's own false positive, L5d accepted. Verdict: no blocker for the completion claim, subject to the
  two documentation residues and one Low below.
- Residual items recorded by that review (non-blocking, carried forward, not fixed here):
  - **R1 (Low, doc):** plan §5.2 named a test file that never existed (`web_shims_identity_handoff.test.js`).
    Corrected in this documentation pass to the actual coverage: block `[CAP-WEB-029] Core-first ordering
    handoff` in the existing `android/shell/app/src/test/js/web_shims.test.js`, cases 1-11. No new test file.
  - **R2 (Low, doc):** `COMPAT-REGISTRY.md:292` still describes only the `applyPlatformIdentity(Utils)`
    applier call — no inline fallback, no pending handoff, no verified-pin condition. **Still stale,
    deferred to the owner**; outside this task's file ownership.
  - **R3 (Low, behaviour):** with a retained pending handoff and an unpinnable `Utils`, the re-apply at
    `runtimepack_web_shims.js:2907-2914` runs inside `pollPatches` (every 10 ms, bounded by
    `pollTicks > 1000`), and each failure logs `WEB-029: Failed to pin Utils.isNwjs to false`. That is up to
    ~1000 diagnostics over ~10 s, which can evict older genuine game diagnostics (`MAX_GAME_ERRORS = 2000`,
    `MAX_RECENT_LOGS = 500`). **No log throttle is implemented**; the earlier "twice in one pass" wording
    understated the worst case. Bounded, rare (unpinnable `Utils`), and only on an already-failing path.
  - **R4/R5 (Info):** re-entry can double-drain in one pass; sticky `platformIdentityActive` after a
    post-pin `Utils` replacement is **pre-existing** behaviour, not introduced or fixed by this pass, and
    untested on device.

### Gate 4 — regression (closed, exact counts)

Environment: WSL Ubuntu 24.04, node v22.23.3, JDK 17.0.20.1, cargo/rustc 1.99.0, Gradle run as `root`
with `--offline --no-daemon`, reused `CARGO_TARGET_DIR=/home/mhsymr/linux-remediation-20261022/cargo-target`
(never cleaned). Source freeze verified identical at start and end of QA.

**JS — 13 suites ran, 12 exit 0.**

| Suite | Exit | Note |
|---|---|---|
| `cheat_menu` | 0 | 8 suites |
| `cheat_primitives` | 0 | expected `[CheatEngine] forbidden_action` log on stderr |
| `diagnostic_suite` | 0 | |
| `jszip_buffer_regression` | 0 | one sub-test **env-skipped**: `[NOT_TESTED] Genuine JSZip bundle not found via VNM_JSZIP_PATH/VNM_GAME_ROOT` |
| `overlay_interaction_regression` | 0 | |
| `vnm_audio_lifecycle` | **1** | **NOT_TESTED, env-gated, not a regression**: `Error: NOT_TESTED: VNM_GAME_ROOT not found` at `vnm_audio_lifecycle.test.js:272`; not run in baseline either |
| `vnm_extreme_input_guard` | 0 | 12 tests |
| `vnm_overlay_focus_lifecycle` | 0 | |
| `vnm_save_storage` | 0 | stderr stacks are intentional corrupt-save fixtures |
| `vnm_startup_lifecycle` | 0 | |
| `vnm_window_adapter` | 0 | |
| `vpad_bridge` | 0 | |
| `web_shims` | 0 | 13 blocks, incl. `Platform Identity Virtualization (CAP-WEB-029)` and `Core-first ordering handoff (CAP-WEB-029)` |

Logs: `final-qa-20261024T202057Z/02-js-*`, `02-js-exitcodes.tsv`, `11-js-notes.txt`.

**Kotlin — 19 suites / 236 tests / 0 failures.**

- `./android/shell/gradlew -p android/shell :app:testDebugUnitTest --offline --no-daemon` → exit 0.
- JUnit XML aggregate: `files: 19`, `tests=236`, `failures=0`, `errors=0`, `skipped=0`. XML dir deleted
  before the run, so all 19 files are freshly generated inside the run window.
- `io.runtimepack.host.VfsResourceInterceptorTest`: **30 tests, 0 failures** (baseline 26, +4).
- The first attempt failed for a harness reason (`gradlew` not on PATH → exit 127) and is preserved in
  `03-kotlin-attempt1.*`; the retry used the repo wrapper. No `--rerun-tasks`, no network fetch.
- Logs: `03-kotlin.*`, `03-kotlin-xml-aggregate.txt`, `12-kotlin-xml-detail.txt`.

**Rust — 1252 passed / 0 failed / 5 ignored.**

- `cargo test --workspace --no-fail-fast` → exit 0, `failed_test_names: (none)`,
  aggregate `{'passed': 1252, 'failed': 0, 'ignored': 5, 'measured': 0, 'filtered_out': 0}`.
- The 5 ignored are **opt-in env gates**, not passes:
  1. `test_opt_in_real_game_qa_harness` — needs `RUNTIMEPACK_TEST_GAME_ROOT`
  2. `test_opt_in_subject_game_ansuz_patch_real_bytes` — needs `RUNTIMEPACK_TEST_GAME_ROOT`
  3. `version_metadata::tests::real_distribution_metadata_resolves` — needs `RUNTIMEPACK_R1A_RENPY_SOURCE`
  4. `bundle::tests::browserify_real_shape_static_validation` — needs `RUNTIMEPACK_P2_REAL_PIXI`
  5. `static_constants::tests::opt_in_static_source_validation` — opt-in static source validation
- Logs: `04-cargo.*`, `04-cargo-summary.txt`.

### Gate 5 — fresh policy assessment (closed; g03 blocked)

No waiver flags were used anywhere: no `--allow-unknown`, no `--accept-audit-fail`, no severity override,
no deep mode, no HTML, no network audit.

`inspect` g03 (`/home/mhsymr/g03_fast`): exit 0, status `UNSUPPORTED_DEPENDENCY`, 21 diagnostics, **7
ERROR all with proof `blocked`** — unchanged from baseline (multiset equal).

| Code | Count | Subjects |
|---|---|---|
| `CMP-102` | 4 | `lib/sdkencryptedappticket.dll`, `lib/sdkencryptedappticket64.dll`, `lib/steam_api.dll`, `lib/steam_api64.dll` |
| `CMP-103` | 2 | `lib/greenworks-win32.node`, `lib/greenworks-win64.node` |
| `CMP-104` | 1 | `js/plugins/OrangeMapshot.js` (Node `child_process`) |

`audit` g03 (`--mode standard --no-html`): exit 0, console `Outcome: Fail`; JSON `outcome=FAIL`,
`mode=STANDARD`, `runtime_validation=NOT_RUN`, nodes 5138 / references 3753 / findings 19,
`severity {ERROR:7, WARNING:3, INFO:9}`, `disposition {BLOCKING:7, WARNING:3, INFORMATIONAL:9}`,
`required_scope_complete=true`, `uncovered_reference_classes=[]`. Before vs after: every field and the
full finding-tuple multiset are equal. Logs: `06-inspect-g03.*`, `07-audit-g03.*`,
`g03-audit-after.json`, `asset-dependency-graph.json`.

No regression elsewhere:

| Game | Baseline | Fresh | Multiset equal | stdout byte-identical |
|---|---|---|---|---|
| g07 | `UNSUPPORTED_DEPENDENCY` / 13 | `UNSUPPORTED_DEPENDENCY` / 13 | yes | **yes** (`4305f54e…`) |
| g09 | `DEGRADED` / 16 | `DEGRADED` / 16 | yes | **yes** (`6cb9f5a7…`) |
| g03 | `UNSUPPORTED_DEPENDENCY` / 21 | same / 21 | yes | no — only two `CMP-104` WARN lines swapped position (pre-existing nondeterministic ordering), zero content change |

Verdict: **g03 remains BLOCKED, audit FAIL, no APK built or installed, port not complete.**
`--flavor beta` was never invoked because eligibility was never reached.

### Provenance nuance for the analyzer binary (must be stated with any CLI claim)

- `cargo build -p runtimepack` reported the debug CLI **up to date** (no `Compiling` line, `Finished dev
  profile … in 2.14s`): the cached fingerprint for the current Rust dependencies was accepted, so **no
  recompilation occurred and no forced rebuild was performed** during QA.
- The binary `sha256` was identical before the build, after the build, and after all inspect/audit runs
  (`d98c0764…`), so baseline and post-fix comparisons are artifact-identical.
- The binary mtime is `2026-10-24 20:19:50 +0700` (`13:19:50Z`), which **predates** HEAD `a1c1045`.
  Attribution is cargo-fingerprint based only. **No claim is made that the binary bytes correspond to a
  specific source commit**, and no bit-level source provenance is asserted. This does not weaken the
  before/after comparison: the committed diff touches no Rust/analyzer file, so no analyzer verdict change
  was possible.

### Unresolved and out of scope (carried forward, not fixed here)

1. **Native typed-`Unknown` forwarding is a policy change requiring separate approval — NOT done.** It
   stays rejected/undecided, not deferred for convenience. Changing it would alter policy and security
   scope.
2. **`Docs/superpowers/compat/COMPAT-REGISTRY.md:292` still understates the implementation.** That line
   describes only the `applyPlatformIdentity(Utils)` trailer call plus getter/setter; it does not mention
   the inline `Object.defineProperty` fallback, the `__runtimepack_pendingIdentity` handoff, or that
   `platformIdentityActive` is set only after a verified pin. Outside this task's file ownership — **still
   stale, deferred, owner action required.**
3. **A failed pin on an unpinnable `Utils` can emit one `WEB-029` failure diagnostic per poll tick.** The
   re-apply site (`runtimepack_web_shims.js:2907-2914`) sits inside `pollPatches`, which runs every 10 ms
   and stops at `pollTicks > 1000`, so a retained pending handoff can produce up to ~1000 diagnostics over
   ~10 s and evict older genuine game diagnostics (`MAX_GAME_ERRORS = 2000`, `MAX_RECENT_LOGS = 500`).
   **No log throttle is implemented**; the earlier "twice in one pass" description understated the worst
   case. Bounded, rare, and only on an already-error path; no state or pinning impact.
4. **Sticky `platformIdentityActive` after a post-pin `Utils` replacement is pre-existing, not a new fix.**
   If the engine replaces `Utils` after a verified pin, identity can stay marked active while the new
   object's `isNwjs()` is `true`; the failure is logged. Untested on device. Out of scope for this pass and
   not introduced by it.
5. **Behaviour never observed in a real WebView or APK.** No device or emulator was available and no APK
   was built, so device boot, audio, save/load round-trip, gameplay, and the real-WebView identity handoff
   are all unverified. JS proof is a `node` `vm` sandbox against the extracted production trailer; Kotlin
   proof is structural assertions on the served stream.

## Checklist

- [x] Create this ledger; confirm the path did not already exist.
- [x] Record base commit, workspace state, constraints and invalidated claims.
- [x] Close analyzer investigation (taint/closure semantics).
- [x] Close runtime-ordering investigation (shim before core) — defect confirmed, synthetic repro only.
- [x] Re-derive root cause of the 7 ERROR findings against a binary whose provenance is stated.
- [x] Confirm the approved spec already requires the inline core-trailer fallback (lines 129–174).
- [x] Finalize the smallest-scope implementation plan and get supervisor approval.
- [x] Implement via TDD with independent review — committed `a1c1045`; review APPROVED, no High; M1 +
      L1/L2/L3/L5 fixed in the same commit.
- [x] Final scoped independent re-review PASS (`runtime-handoff/FINAL-REVIEW.md`) with residual lows
      recorded (R1 corrected here, R2 deferred, R3 bounded log amplification, R4/R5 info).
- [x] Targeted regression, then full workspace suite, with exact counts — JS 13 suites / 12 exit 0, Kotlin
      19 suites / 236 tests / 0 failures, cargo 1252 passed / 0 failed / 5 ignored.
- [x] Fresh policy assessment; decide build vs. blocked — g03 stays BLOCKED (7 hard findings, audit FAIL),
      no APK, no waiver flags.
- [x] Final report: completed vs. blocked — canonical user-facing report at
      `D:\02_Development\Raw-Games\Sample\port_apk\_reports\linux-remediation-20261022\bprison-20261025\FINAL-REPORT.md`.
- [x] Documentation-only correction pass: plan §5.2 test-file name, provenance wording, and residual lows
      aligned across this ledger, the plan and the external final report. No source/test/build change.

## Reporting rules

- Distinguish: change made / test passed / behavior unverified.
- Never describe a static analyzer result as runtime or device evidence.
- Never describe a synthetic Node harness result as device or gameplay evidence.
- Every finding names the artifact (binary path + mtime, or commit) that produced it.
- `Blocked` remains non-waivable; `--allow-unknown` forwards only `Unknown`.
- Do not claim the port is fixed, complete, or shipped. g03 is expected to remain BLOCKED.
- The bounded fix is committed (`a1c1045`); this ledger's final pass is documentation-only — no source,
  test, build, or agent-config change, and no further commit from this pass.
- Do not merge. Do not push. Do not build an APK on the strength of this ledger.
