# Tahap 4 — Coffin (g05), B-Prison (g03) & Tahap 3 Debt Closure

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Serial tasks, each ending in an independent review gate.

**Status:** revised 2026-10-24 after validation. Supersedes the first draft; the original Task 2 (binary `fs.readFileSync`) and Task 4 (audio severity) were removed because their premises were disproved. Evidence: `_reports/linux-remediation-20261022/TAHAP4-PLAN-VALIDATION-20261024.md`.

**Goal:** close the reviewed Tahap 3 debt, make Coffin's own loader runnable by supplying the one missing capability it actually needs, and decide — with evidence — whether B-Prison's native chain can be proven dead rather than waived.

**Base:** `47bbb43` on `fix/linux-batch-compat-20261022`.

---

## Part A — Tahap 3 debt closure (must precede everything else)

### A1. M-2 — honest status propagation

**Files:** `cli/src/main.rs`, `tests/integration/asset_audit_report_test.rs`

- Test first: `Default` policy + one `Optional` `Unknown` finding → compatibility report status must not be `COMPATIBLE`.
- Change the downgrade gate at `main.rs:1608` from `typed_decision.is_experimental()` to `typed_decision.status != ReportStatus::Compatible`.
- Keep the stderr experimental block and notice writing gated on `is_experimental()` — a non-experimental `Unknown` must not produce an experimental notice.

### A2. M-1 — durable notice copy

**Files:** `cli/src/main.rs`, `core/src/build_plan/mod.rs`, `schemas/build-manifest.schema.json`

- After `finalize_build`, write `EXPERIMENTAL-NOTICE.txt` to `<output>/android-project/app/src/main/assets/` when the policy is experimental, so the audit trail survives scratch-workspace cleanup.
- Record `notice_sha256` in `provenance.audit`; add it as an optional schema property (not required).
- Fail closed on write error.

### A3. L-3 — notice inside the integrity list

**Files:** `core/src/resource_root.rs`, `cli/src/main.rs`

- Include `EXPERIMENTAL-NOTICE.txt` in `checksums.txt` when the policy is experimental.
- Extend the artifact verifier so a missing or mismatched notice fails verification.
- Do not checksum it for default builds (no file, no entry).

### A4. L-4 — stale notice and retained evidence

**Files:** `cli/src/main.rs`

- A default build into an output directory that already contains `EXPERIMENTAL-NOTICE.txt` must either overwrite it with a policy-bearing marker or refuse; it must not silently leave a stale experimental notice next to a default build.
- Re-run and retain: `cargo test --workspace --no-fail-fast`, `cargo test -p runtimepack --test build_test`, `cargo test -p runtimepack-core --lib`. Store logs under `_reports/.../tahap4/` and record exact counts.

### A5. L-1 — doc comment repair

**Files:** `cli/src/main.rs`

- Move the durable-notice doc block back above `write_durable_runtime_notices_or_exit`; leave only the experimental-notice text above `render_experimental_notice`.

### A6. L-2 — negative coverage

**Files:** `tests/integration/asset_audit_report_test.rs`

- Crafted `state="unknown"` on `CMP-AUD-009` (path safety) still blocks.
- Caller hard blocker (`BLD-*`/`SIG-*`) still blocks under `Experimental`.
- `UnsupportedDependency` with a typed `Unknown`: allowed under `Experimental`, blocked under `Default`. With a typed `Advisory`: allowed under both — assert that decision explicitly.

### A7. Flag help honesty

**Files:** `cli/src/main.rs`

- Document in the `--allow-unknown` help that, under `--strict-audit`, an audit `FAIL` caused only by forwarded findings is treated as `WARNING` while the recorded outcome stays `FAIL`.

**Gate:** independent review of A1–A7 before Part B starts. Part B does not depend on A, but A must not be left half-applied while new builds are produced.

---

## Part B — Coffin (g05): BLOCKED by a vendor gate, not by a missing capability

**Spike result (2026-10-25):** the inflated bootstrap is the game's custom NW.js wrapper, not an asset decoder. `BUILD_FLAGS = 0x181` sets `BF_VENDOR_STEAM`, and `Scene_Boot.start` begins with:

```js
if (App.requiresSteam() && !Steam.init()) return;
if (!Crypto.hashMatchDRM(0x6ee0ff1a)) return;
```

`Steam.init()` returns `false` whenever `Utils.isNwjs()` is false, which the runtime shim forces. The game therefore returns before initialising — a black screen, not a crash.

Evidence: `_reports/linux-remediation-20261022/G05-BOOTSTRAP-SPIKE-20261025.md`.

**Consequences:**

- `zlib` is necessary (it is how the wrapper is injected) but **provably insufficient**.
- Making this build boot would require `Steam.init()` to succeed without Steam — DRM circumvention. Not implemented, not planned.
- The DRM hash check itself is legitimate: it hashes the shipped `Copyrights - Coffin of Andy and Leyley.txt`.
- The DRM-free itch.io build has `BF_VENDOR_STEAM` clear and would skip the gate; the remaining work is ordinary capability work (`zlib`, binary `fs`, `crypto.createHash`, `os.networkInterfaces`).

**Decision required from the user:** supply the DRM-free build, or record Coffin as unsupported for the Steam build. No code work starts before that.

### B0 (only if a DRM-free build is supplied) — `zlib` capability

**Files:** `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`, new vendored inflate asset under `web_shims/`, `android/shell/app/src/test/js/web_shims.test.js`, `Docs/REFERENCES.md`, notices.

- Prove RED first: `require('zlib')` currently returns `{}`; `inflateSync` is `undefined`.
- Contract tests before implementation: valid zlib stream → exact bytes; `inflateRawSync` on raw DEFLATE; truncated input **throws**; garbage **throws**; output must not be empty and must not equal the input.
- Vendor a permissively licensed DEFLATE decoder (MIT/BSD). Record source URL, version/commit, and license in `Docs/REFERENCES.md`; ship the license text in the notice set. Pin the file; no CDN, no build step.
- Expose through the existing `modules` table so `require('zlib')` resolves.
- `Buffer` is already present with `from(base64)` and `toString('utf-8')`; add no Buffer work unless a test fails.

### B2. Bootstrap probe (one-off, outside the repo)

- Already executed on 2026-10-25: 58,432 base64 chars → 43,822 bytes → 151,734 bytes inflated, valid UTF-8. Recorded in `G05-BOOTSTRAP-SPIKE-20261025.md`.
- The probe validates the decoder against the real stream. It is not a committed test (it would embed game data) and is not game execution.

### B3. Inline-script injection verification

**Files:** shell instrumentation or a targeted test harness.

- Verify the Android WebView executes `script.textContent` + `document.body.appendChild(script)`.
- No CSP exists in the shell today, so this is expected to work — but it is unverified, and it is a hard prerequisite for the Coffin loader. Record the actual result.
- If it fails, stop and report; do not replace it with an `eval`-based workaround without approval.

### B4. Build and validate

- Rebuild release CLI, record SHA256.
- `inspect`, then `build --flavor beta --allow-unknown --package io.runtimepack.linux20261022.g05 --name Coffin`.
- Validate `apksigner verify`, `aapt2 dump badging`, `unzip -t`, notice presence, CRC.
- Report as experimental / not device-validated. **No claim that Coffin reaches gameplay** — `zlib` removes one known blocker, nothing more.

**Superseded by the spike:** the wrapper calls `Utils.readFile` → `fs.readFileSync(path, encoding)` on shipped files, plus `readdirSync`, `statSync`, `accessSync`, `mkdirSync`, `unlinkSync`, `crypto.createHash`, and `os.networkInterfaces`. A DRM-free build would need all of those, not just `zlib`. See `G05-BOOTSTRAP-SPIKE-20261025.md`.

---

## Part C — B-Prison (g03): decision required

### C1. Facts that changed the plan

- The three `魔王城` findings are already `CMP-AUD-003` **WARNING**; `AuditOutcome::FAIL` comes only from the 7 ERROR findings. **No audio-severity change is needed.**
- `is_closed()` is `!self.tainted` — one global boolean. Taint sources in the real report: 49 dynamic-code files (12 registered, 37 unregistered), 1 unparseable unregistered file, 256 unresolved requires from reachable `pixi.js`, 4 from `greenworks.js`, and bare `nw.gui` requires from reachable `rpg_core.js`/`rpg_managers.js`.
- Therefore "remove dead-file taint" removes ~38 of ~327 reasons and leaves every native proof `Unknown`. The first draft's expected outcome was wrong.

### C2. Option A — sound analyzer work (recommended if B-Prison is pursued)

**Files:** `engines/rpgmaker_web/src/activation.rs`, `classify.rs`, `tests/integration/web_asset_audit_test.rs`

1. **Per-subject closure.** Replace the global `tainted`/`is_closed()` gate with per-subject reachability: a native subject's proof considers only modules that can reach it.
2. **Guard modelling.** A `require` inside a guard proven false on Android (`Utils.isNwjs()`, `Utils.isOptionValid('test')`) is a dead edge, not an unmodelled route. This is what makes the `OrangeGreenworks → greenworks.js → lib/*.node → steam_api*.dll` chain `ProvenNotRequiredOnTarget`.
3. **Per-module dynamic-code scope.** `eval` in module X invalidates X's outgoing edges only, not every module's.

Tests required before implementation:

- Unguarded native `require` still `Blocked`.
- `Utils` mutation still `Unknown`.
- Dynamic code in a reachable module still makes **that module's** edges untrusted.
- Guard-proven `require` becomes advisory, and is `Unknown` again if the guard proof is absent.
- `InfoWindow.js` (unregistered, unreferenced, unparseable) produces at most an informational finding.

Regression gate: full workspace suite plus re-audit of two already-built web games (`g07` Forestia, `g09` NTR Hunter) with byte-identical or better findings.

**Risk:** shared-engine surgery affecting all 16 existing web APKs. Requires its own independent review and a rollback point.

### C3. Option B — Coffin only

Leave B-Prison `Blocked`, record the exact typed reason per finding, ship Coffin. Zero analyzer risk. B-Prison stays an honest gap, not a silent one.

### C4. Out of scope for B-Prison

- Audio severity (already correct).
- Filename exemptions, deleting native assets, or stubbing `OrangeMapshot`.
- Making proof-carried `CMP-102/103` waivable — rejected by the approved policy.

### C5. Separate honesty fix (do regardless of A/B)

`engines/rpgmaker_web/src/asset_audit.rs:804` claims a missing optional asset is "safely auto-shimmed by Android VFS runtime fallback" and resolves it as `RuntimeProvided`. Nothing shims a missing BGM; it is silence. Reword to state the actual consequence and stop claiming runtime provision. `tests/integration/web_asset_audit_test.rs:1673` pins the old string and must be updated with it.

---

## Global constraints

- Game inputs immutable; no game execution; no title/hash/filename exemption.
- No stub that returns empty data to satisfy a call. A capability is implemented or reported missing.
- `Blocked` stays non-waivable; `--allow-unknown` may forward only `Unknown`.
- No new Rust dependency. Vendored JS needs a `Docs/REFERENCES.md` entry and a shipped notice.
- TDD: observe RED for changed behaviour; never weaken an existing assertion.
- WSL root cargo, `CARGO_TARGET_DIR=/home/mhsymr/linux-remediation-20261022/cargo-target`, `CARGO_BUILD_JOBS=2`.
- Stage only owned files.

## Explicit boundaries

- OneShot stays deferred: native mkxp extensions (Journal, Wallpaper, `Sprite#obscured=`) are not implemented here.
- No device boot, input, audio, or save/load validation for any build produced by this plan.
- No claim that any game is "playable" from a successful build.
