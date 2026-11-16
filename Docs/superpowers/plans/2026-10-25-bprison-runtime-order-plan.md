# B-Prison (g03) — runtime ordering plan (core-completion identity handoff)

Created: 2026-10-25 (revised after independent validation)
Status: **implemented, committed as `a1c1045`, and independently reviewed PASS**
(`runtime-handoff/FINAL-REVIEW.md`). Runtime fix is complete; **g03 remains BLOCKED** (7 findings), audit
FAIL, `runtime_validation` NOT_RUN, no APK, no device run. This revision is documentation-only: the plan
text now matches the committed tests and the final review.

Owner of this file: this task only. Parent ledger: `2026-10-25-bprison-investigation-progress.md`
(not edited here). Historical record: `2026-10-24-tahap4-coffin-bprison.md` Part C (not edited).

Base: worktree `runtime-v2`, base HEAD `77d077e`, final HEAD `a1c1045`, branch
`fix/linux-batch-compat-20261022`.

Provenance correction (supersedes an earlier verbal claim): `runtime-v2` **is** version-controlled.
`.git` in that directory is a file containing
`gitdir: /mnt/d/02_Development/Projects_1/runtimepack-vnm-reconstruction/.git/worktrees/runtime-v2`;
WSL `git -C /mnt/d/.../runtime-v2 rev-parse HEAD` returns `77d077e`. A Windows-native `git` invocation
from that directory reported `not a git repository (NULL)` — that is a tooling artifact of the linked
worktree, not absence of VCS. Normal git history therefore covers this source tree; no extra backup
scheme is needed. Preserve the user's tracked edits (`AGENTS.md`, `memory/*`) and all untracked
reports/plans/`R/`; no commit, merge or push from this work stream.

## 1. Precondition (hard gate)

Independent runtime-ordering validation `ses_e90b9223fffeZMsQZV0EtEhQB1` has now returned
**confirmed**. This revision applies that finding. If any later check contradicts it, stop and
re-plan instead of proceeding.

## 1.1 Supervisor spec ruling (authoritative for this revision)

The approved design contract for `CAP-WEB-029` is the deterministic core-completion fence, not a
pending-only handoff. Spec: `Docs/superpowers/specs/2026-10-10-web-platform-external-url-design.md`
§2.2 (trailer fence, printed at lines 110-141; Layer 3 descriptor at lines 173-201). Per the
supervisor ruling, that contract includes a **synchronous inline `defineProperty` fallback in the
core trailer** when `RuntimePack.applyPlatformIdentity` is not yet available; production omitted it,
leaving only the diagnostic branch. The printed snippet shows only the diagnostic path, so this plan
**restores the intended synchronous fallback** rather than inventing new identity semantics.

Consequence for the design: pending-only is insufficient. Between the core script and the shim
`<script>`, an intermediate engine script (`rpg_managers.js`, `rpg_objects.js`, `plugins.js`) can
define `require`/`process`, and `Utils.isNwjs()` (`typeof require === 'function' && typeof process
=== 'object'`) would then evaluate **true** before the shim ever loads. The trailer must therefore
pin `isNwjs` false in the same evaluation as the core, using the approved small configurable
accessor, and also record the pending target so the canonical shim path can adopt it and set
`platformIdentityActive`.

This is **minimal restoration of the approved design**, not a general identity redesign: same
descriptor shape as `applyPlatformIdentity` (getter returning the false-fn, absorbing setter,
`configurable: true`), same false-fn marker, no new capability, no new public API.

## 2. Verified defect (source, not runtime observation)

- `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt:100-124`
  (`transformWebHtml`) inserts `web_shims/runtimepack_web_shims.js` immediately before `js/main.js`.
  In MV/MZ `index.html` the core script (`js/rpg_core.js` / `js/rmmz_core.js`) comes **earlier**,
  so the shim executes **after** the core.
- `VfsResourceInterceptor.kt:157-170` (`SCRIPT_COMPLETION_TRAILER`) is appended to the hook file
  (`:443-462`); the MV hook is `js/rpg_core.js`, the MZ hook is `js/rmmz_core.js`
  (`engines/rpgmaker_web/src/lib.rs:495-537`). The trailer currently requires
  `window.RuntimePack.applyPlatformIdentity` to exist **at end of core**, i.e. before the shim tag
  has been parsed. On the deterministic path the `else` branch fires
  (`[RuntimePack] WEB-029: ... unavailable in core script completion hook`) and no identity pin
  happens.
- Recovery cannot self-arm: `RuntimePack.platformIdentityActive` starts `false`
  (`runtimepack_web_shims.js:56-58`), is set **only** inside `applyPlatformIdentity` (`:67`), and
  every re-apply is gated on it (`:666-668`, `:2852-2854`). The only other path is the timing-
  dependent `evaluateJavascript` injection at `WebActivity.kt:623-626` and `:660-662`.

## 3. Rejected approach and why

**Rejected: move the HTML shim tag earlier (before the core script).**
The shim's install pass (`pollPatches()` at `runtimepack_web_shims.js:2894`) patches
`StorageManager`, `Graphics`, `SceneManager`, `Input`/`TouchInput`/`Window_Selectable`
(`:2848-2866`) against already-defined engine objects. Injecting before the core makes those
objects absent at install time, and correctness then depends on the 10 ms polling loop
(`:2895-2906`) winning a race against engine initialisation. That trades a deterministic defect for
a timing dependency. `transformWebHtml` therefore stays **unchanged**, including the existing
before-`js/main.js` anchor.

**Rejected: pending-only handoff (the previous revision of this plan).** Storing
`__runtimepack_pendingIdentity` and waiting for the shim leaves `Utils.isNwjs` unpinned for the
whole span between the core script and the shim tag. Any intermediate engine script that defines
`require`/`process` makes `Utils.isNwjs()` return `true` in that window (engine predicate:
`typeof require === "function" && typeof process === "object"`), which is exactly the failure the
deterministic fence exists to prevent. Pending remains part of the design, but only as the adoption
record — never as the sole mechanism.

**Chosen: order-independent core-completion identity handoff with a synchronous inline pin.** The
trailer pins identity immediately when the canonical applier is absent, and records the target so
the shim adopts it, re-applies the canonical descriptor, and marks `platformIdentityActive`. No fake
success: if the inline pin cannot be proven, the trailer keeps the pending record and does not claim
an active pin.

## 4. Production change — exactly two files

### 4.1 `VfsResourceInterceptor.kt` — trailer only (lines 157-170)

`SCRIPT_COMPLETION_TRAILER` becomes a four-branch handoff, still inside the existing IIFE, with the
leading `\n;\n` fence preserved:

1. `RuntimePack` present and `applyPlatformIdentity` is a function → call it with `u` immediately
   (today's behaviour, unchanged).
2. `applyPlatformIdentity` unavailable **and** `u` defined → **synchronous inline pin**, restoring
   the approved design: inside `try`, `Object.defineProperty(u, "isNwjs", { configurable: true,
   enumerable: true, get: function () { return falseFn; }, set: function (_fn) {} })` where
   `falseFn` is `function () { return false; }` marked `__rp_patched = true` — the same descriptor
   shape and marker used by `applyPlatformIdentity` (`runtimepack_web_shims.js:60-88`). Then set
   `window.__runtimepack_pendingIdentity = u` and emit one `console.warn` noting the pin is
   provisional pending canonical adoption. Do **not** set `RuntimePack.platformIdentityActive` (the
   trailer must not claim shim state), do **not** report full success.
3. Inline pin **fails** (target non-configurable/non-extensible, or `isNwjs` not redefinable) →
   keep `window.__runtimepack_pendingIdentity = u`, do **not** claim the pin succeeded, and emit a
   diagnostic stating the identity could not be proven. Pending is preserved for a later canonical
   attempt.
4. `u` undefined → keep the existing `WEB-029: Utils is undefined ...` error path.

The pending value must be the latest resolved `Utils` (`typeof Utils !== "undefined" ? Utils :
window.Utils`), as today's `u` resolution already does. Guard `typeof window === "undefined"` as
today. No other Kotlin file, no constructor parameter, no `transformVnmHtml`/VNM branch change
(`:44-98`, `:423-431`), no change to hook resolution (`:443-462`).

### 4.2 `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`

Add one local `applyPendingPlatformIdentity()` immediately after the
`root.RuntimePack.applyPlatformIdentity` definition (after `:88`), so it is in scope everywhere the
shim can resume:

- read `root.__runtimepack_pendingIdentity` (also accept `window.__runtimepack_pendingIdentity`);
  return when absent;
- target = **latest** `root.Utils` when present, else the pending value (so a core that defines
  `Utils` later is still honoured);
- call `root.RuntimePack.applyPlatformIdentity(target)`; `platformIdentityActive` becomes true only
  through that canonical call — the drain never sets it directly;
- delete `root.__runtimepack_pendingIdentity` **only after** the canonical apply returned without
  throwing; on failure keep the pending record, leave `platformIdentityActive` unchanged, and do not
  pretend adoption happened;
- no polling, no timers, no retry loop.

Call sites: (a) once at the end of the `applyPlatformIdentity` definition block, before the rest of
the install pass, and (b) **before** the `__runtimepack_shims_installed` early return
(`:662-677`) so a re-injected/second shim load also drains. Nothing else in the shim changes:
`require` unknown-specifier semantics (`{}`), all module/API stubs, `nw.Window` surface, and the
`configurable: true` identity descriptor all stay as-is. No new `platformIdentityActive`
self-arming, no unconditional stubs, no broad `pollPatches` rewrite, no extra unrelated stubs.

## 5. Tests

### 5.1 Kotlin — `android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt`

Exercise the **actual served completion trailer stream** through `intercept(...)`, not the constant
in isolation:

1. `testCoreCompletionTrailerPinsIdentityInlineWhenRuntimePackAbsent` — hooked core request
   (`js/rpg_core.js`), read served text via `intercept(...)`: starts with core content, trailer
   appended, contains the inline `Object.defineProperty(u, "isNwjs"` fallback with
   `configurable: true`, contains the `__runtimepack_pendingIdentity` assignment, contains no
   unconditional `platformIdentityActive = true`, and retains the
   `applyPlatformIdentity(u)` immediate branch for the shim-present case.
2. `testCoreCompletionTrailerStillAppendsForMzHook` — same for `js/rmmz_core.js`.
3. Existing hook tests (`testScriptCompletionHookAppendsTrailerStream` `:566-...`, mixed-case
   `:630-646`) and `testWebHtmlTransformInjectsWebShimsWhenHooksActive` (`:648-681`) must stay
   green, unchanged — the HTML anchor is intentionally not modified.

Kotlin cannot execute the trailer; the runtime proof lives in 5.2. Kotlin assertions are structural
only and must not be described as behavioural evidence.

### 5.2 JS — existing `android/shell/app/src/test/js/web_shims.test.js` (no new file)

Coverage lives in the existing suite, block `[CAP-WEB-029] Core-first ordering handoff (shim served after
engine core)` (`:1644-1928`, 11 cases). No `web_shims_identity_handoff.test.js` exists or is created.
Plain `node`, same `vm` sandbox style as the rest of `web_shims.test.js`. **No handwritten copy of the trailer.**
A fixture helper reads `../../main/java/io/runtimepack/host/VfsResourceInterceptor.kt`, extracts the
`SCRIPT_COMPLETION_TRAILER` string-literal fragments, joins them, and unescapes `\"` and `\n`. Fail
the test if extraction is empty, if it lacks `applyPlatformIdentity`, or if it lacks both
`__runtimepack_pendingIdentity` and `Object.defineProperty` — so the harness can never silently run
a stale or mirrored copy. A mutation self-check (`web_shims.test.js:46-62`, in-memory only, disk untouched)
proves the guard rejects a degraded trailer. Sandbox provides `console` capture plus
`setInterval`/`clearInterval` no-op stubs so the process exits without waiting on timers (no wall-clock
assertions).

Cases (all execute the **real extracted trailer**; the shim is the real
`runtimepack_web_shims.js`; numbering matches the source comments):

1. **core-first pin + retained handoff** — sandbox without `RuntimePack`; engine `Utils` with the real MV
   predicate (`isNwjs = function () { return typeof require === "function" && typeof process === "object"; }`)
   exposed as `window.Utils`; run trailer → `Utils.isNwjs() === false` immediately, descriptor
   `configurable: true` with a getter, `__rp_patched` marker, pending set to the pinned `Utils`, no
   `RuntimePack` namespace created, no WEB-029 error, provisional pending warn captured.
2. **intermediate engine script** — between core completion and shim, a script that defines/calls
   `require` and probes `Utils.isNwjs()` must not enter the desktop branch and must perform no
   `require()` (this is the assertion the pending-only design fails).
3. **shim-first, then core** — shim runs first (identity inactive before `Utils` exists), then engine
   `Utils` + trailer → canonical apply, active true, no pending left, no WEB-029 errors.
4. **core-first, then late shim** — shim drains the pending handoff, sets `platformIdentityActive`,
   clears pending, and applies the `Window_Selectable` touch branch
   (`prototype.__runtimepack_touch_ok_patched === true`, `isTouchOkEnabled` present — not
   latch-without-patch).
5. **`Utils` replaced before the shim** — the latest `root.Utils` is pinned, not the stale object;
   pending cleared.
6. **re-entry / late injection with pending** — shim already installed, pending set, re-run the shim
   script → early-return branch drains pending, applies canonically, clears; a second re-entry stays
   idempotent.
7. **missing `Utils`** — explicit `WEB-029: Utils is undefined` diagnostic, no pending record, no
   `RuntimePack` namespace.
8. **fallback absorption** — a plain and a strict-mode reassignment of `Utils.isNwjs` both stay `false`;
   the later shim pin preserves it and clears pending.
9. **unproven pin preserves pending** — non-configurable `isNwjs`: trailer reports the failure, keeps
   `__runtimepack_pendingIdentity`; shim load must not set `platformIdentityActive === true` and must keep
   the pending record. No fake success either side.
10. **failed pin keeps the Layer-4 retry alive** — after both failures, the engine replaces `Utils` with a
    pinnable object and the captured poll callback is invoked directly (no wall-clock): the retry pins it,
    marks active, clears pending.
11. **throwing pending assignment** — a setter that throws on `window.__runtimepack_pendingIdentity` must
    not abort the trailer; the pin is still applied and the provisional warn still emitted.

## 6. Task split and gates

**Task 1 — plan + implementation (Executor), RED→GREEN, then independent review.**
RED first (both suites, failing names captured), then the two-file change, then GREEN, then a
reviewer who did not implement it checks: diff limited to §4 files; trailer has the four branches
with the synchronous inline pin, and no fake success on the failure branch; inline descriptor shape
and `__rp_patched` marker match `applyPlatformIdentity`; pending cleared only after a canonical
apply succeeded; latest `Utils` honoured; drain present at both call sites; no timer/retry addition;
no `require`/stub/policy change; HTML before-main anchor and VNM untouched; existing assertions not
weakened; the JS harness genuinely executes the real trailer and genuinely fails against the old
trailer (verified by running it before the fix, not by inspection alone).

Documentation ownership: the parent ledger `2026-10-25-bprison-investigation-progress.md` is owned by
another executor — **do not edit it** from the implementation task; this plan is the only file that task
owns. The final documentation-only correction pass owns this plan, that ledger, and the external
`bprison-20261025/FINAL-REPORT.md`, and nothing else. No concurrent edit of the ledger, no merge/commit/push.

**Task 2 — QA / final (separate pass, after Task 1 review).** Full `cargo` workspace suite, Android
Kotlin unit suite, JS suite (`web_shims.test.js`, which now contains the CAP-WEB-029 handoff block, plus
the rest of `test/js`), then fresh CLI build and `inspect`/`audit --no-html` on g03 with g07/g09 regression. APK **only** if no non-waivable
blocker remains; otherwise explicit **BLOCKED, no APK**, with the blocking findings and evidence
paths.

Commands (WSL per parent ledger; name the artifact actually used):

```
cd runtime-v2/android/shell
./gradlew :app:testDebugUnitTest --tests "io.runtimepack.host.VfsResourceInterceptorTest"
./gradlew :app:testDebugUnitTest
node app/src/test/js/web_shims.test.js
node app/src/test/js/vnm_startup_lifecycle.test.js
cd runtime-v2 && cargo test --workspace
```

`app/src/test/js` is **not** wired into Gradle/CI: `:app:testDebugUnitTest` only runs the JVM tests,
so the JS behaviour proof is a manual `node` run and must be reported with its exit code.

## 7. Provenance and report directory

Every CLI claim records: binary path, `sha256sum`, size/mtime, source HEAD (base `77d077e`, final
`a1c1045`, plus the diff file list), ruleset/schema version, exact command, exit code, wall time. Game
inputs are read-only and never copied; g03 payload is `/home/mhsymr/g03_fast`.

Analyzer-binary provenance rule (as recorded by QA): `cargo build -p runtimepack` reported the debug CLI
**up to date** from the cached fingerprint for the current Rust dependencies, so no forced rebuild was
performed and no crate was recompiled during QA. The binary `sha256` was identical before the build, after
the build, and after every inspect/audit run, and its mtime (`2026-10-24 20:19:50 +0700`) predates HEAD
`a1c1045`. Attribution is therefore cargo-fingerprint based only: **no bit-level provenance of the binary
against a specific source commit is asserted**, and none is needed, because the committed diff touches no
Rust/analyzer file.

Canonical report directory, outside the source tree:
`D:\02_Development\Raw-Games\Sample\port_apk\_reports\linux-remediation-20261022\bprison-20261025\`
(exists; holds `ANALYZER-INVESTIGATION.md`, `BOOT-INVESTIGATION.md`). New logs go to a
`identity-handoff/` subdirectory: RED, GREEN, JS, review notes, CLI provenance table, g03/g07/g09
audit outputs. Nothing is written into the repo working directory or any game tree; nothing is
deleted from the report directory.

## 8. Expected g03 outcome and scope boundary

Expected after this fix: g03 `inspect`/`audit` **still reports 7 ERROR findings with proof
`blocked`**. Ordering/handoff is not a policy fix and must not change those findings; a changed count
is investigated before being reported.

- **Active `eval` makes the analysis globally uncertain.** `analyze_web_activation_with_bounds`
  (`engines/rpgmaker_web/src/activation.rs:3042-3144`) walks the entire JS payload; one dynamic-code
  module sets `analysis.tainted` and `integrity_taint`. Native precedence (`:3470-3489`) checks
  `integrity_taint` **before** inbound/companion evidence, so unrelated natives become
  `IntegrityFailure`.
- **Guard syntax is already implemented**, not missing: `guard_for_site` (`:1639-1681`) and
  `classify_if_cond` (`:1779-1805`) prove only a depth-0 `&&` chain with exactly one
  `Utils.isNwjs()` conjunct, or the exact single-term negation as an early return, same innermost
  function, `utils_mutated == false`. The general import/require traversal (`:3322-3419`) does not
  consume it, and even a correctly guarded subprocess site stays `Unknown` (`:3806-3824`).
- **Bundle recognizer exists but is unused in activation**: `bundle.rs:138-143` has no production
  caller; all call sites are under `#[cfg(test)]`, and bundle closure would not prove absence of
  dynamic code anyway.
- Therefore an unused-syntax fix alone cannot unblock g03, and neither can this handoff. Any change
  to non-waivable native policy, `accept_unknown` transport, or analyzer taint scope changes policy
  and security scope, needs **separate explicit user approval**, and is not covered by the current
  authorization.

## 9. Stop conditions

Stop and report when: the trailer cannot be extracted by the JS fixture without ambiguity; RED does
not fail; GREEN needs a third production file; an existing assertion would have to be weakened; a
g07/g09 finding regresses; CLI provenance cannot be established; or a policy/non-waivable change
appears necessary.

## 10. Checklist

- [x] Independent validation confirmed (done, `ses_e90b9223fffeZMsQZV0EtEhQB1`); supervisor ruling
      recorded in §1.1 (minimal restoration of the approved synchronous fence).
- [x] RED captured for Kotlin + JS with exact failing names and log paths.
- [x] `SCRIPT_COMPLETION_TRAILER` four-branch handoff implemented (inline pin + pending + failure
      preservation + `Utils` undefined).
- [x] Shim drain implemented at definition block + before `__runtimepack_shims_installed` return.
- [x] GREEN: Kotlin targeted, JS handoff block in the existing `web_shims.test.js` (cases 1-11, incl.
      intermediate `require`/`process` probe and unproven-pin case).
- [x] Independent review passed (reviewer ≠ implementer) — final scoped re-review PASS,
      `runtime-handoff/FINAL-REVIEW.md`.
- [x] Full cargo workspace + Android Kotlin + JS suites with exact counts.
- [x] Fresh CLI provenance; g03 inspect/audit; 7 ERROR still present (multiset unchanged).
- [x] g07/g09 regression recorded (byte-identical inspect stdout).
- [x] Build decision: explicit BLOCKED, no APK built.
- [x] Honest report: change made / test passed / behavior unverified separated.
