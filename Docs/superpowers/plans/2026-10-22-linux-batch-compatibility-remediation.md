# Linux batch compatibility remediation — versioned execution record (2026-10-22)

Status: **PARTIAL / IN PROGRESS.** Companion to
`Docs/superpowers/specs/2026-10-22-linux-batch-compatibility-remediation-design.md`.
Checkboxes reflect real, evidenced state on this branch, not intended completion.

- Branch: `fix/linux-batch-compat-20261022`; BASE `dd9e3f71794f660a57bbe983dea6def4a325b247`
  (`release/runtime-v2`, unchanged).
- Requirements: defined inline in the companion design record
  `Docs/superpowers/specs/2026-10-22-linux-batch-compatibility-remediation-design.md`
  (see its §0.1 self-containment rule and §4 for `LM-T1-04`). This plan and that
  record are the authoritative in-repository statements; no external file must be
  read to resolve a requirement ID.
- Supporting batch artifacts (evidence only, outside the repository, under
  `_reports/linux-remediation-20261022/`): `SPEC.md`, `IMPLEMENTATION-PLAN.md`,
  `INTEGRATION-GATES.md`, `T1-REVIEW-RULING.md`, `R1A-FINAL-REVIEW.md`,
  `T5-RUNTIME-REVIEW.md`, `D1-APPROVAL.md`. Sizes and SHA-256 for the exact bytes
  read are tabulated in the design record §0.2.
- Ownership: engine slices own their engine files; one serial integration owner
  owns shared CLI/build-plan/schema/cache/docs files and the Git index.

## Task 0 — baseline and audit intake

- [x] Exact worktree/branch/HEAD captured before staging; unrelated dirty work
      (D1 engine files, packaging slice, review reports) preserved, never reset.
- [x] Coordination audits and addenda accepted for R1 artifacts/license/environment.
- [x] Ready slices copied into versioned docs (this plan + the design record).
- [x] Local fix branch exists in the existing worktree; release reference untouched.
- [x] Integration ledger kept in coordination root, not in product source.

## Task 1 — T1 Tyrano base parity

- [x] Authored fixtures for runtime-base `loadjs`, true escape, safe-missing and
      no-root-fallback; ASAR shape duplicated.
- [x] RED observed against BASE (in-root refs reported `CMP-AUD-009`).
- [x] Single `data/others/{storage}` candidate for `loadjs`; explicit `data/`
      passthrough, unsafe shapes and `call`/`jump` untouched.
- [x] GREEN: 48 tests.
- [x] Review PASS; corrective commit `4da7a27` narrows the claim to `loadjs` and
      restores the pre-T1 `loadcss` behaviour verbatim.
- [ ] Real `loadcss` (`vital: ["file"]`, `pm.file`) modelling — explicitly out of
      scope here; documented coverage gap, no adapter invented.

## Task 2 — D1 live-root arbitration

- [x] Authored g06-shaped and decoy fixtures in `tests/integration/detection_test.rs`.
- [x] Bounded recognised-loader arbitration; stale cores kept as zero-weight
      `DET-ARB-EXCLUDED-CORE` evidence; unsupported shapes stay `DET-110`.
- [x] Scoped engine commits `8e036ed`, `6bbfdbf`, `e04f0f0`, `b39ee64`.
- [x] Round-4 fix (nearest-binding reachability, named function expressions,
      block-scoped declarations) implemented test-first.
- [x] Committed as `f7405ba3049aa1716468701172d91191e65ebeb9`
      (`fix(detection): bind live loader evidence to reachable lexical owners`);
      SPEC/QUALITY PASS per `D1-APPROVAL.md` (114 passed / 0 failed / 0 ignored
      detection, 40 passed / 0 failed engine package).
- [x] CLI fixture that exercised a dead loader chain corrected so the chain is
      genuinely invoked (`this.loadMainScripts()` via `main.run()`); no assertion
      was removed or weakened.

## Task 3 — R1 detection

- [x] Inline fixtures for recognised `if PY2:`/`else:` metadata, Python-2-only and
      contradictory evidence, docstrings/comments, budgets, existing 8.x rules.
- [x] Bounded parser state added; public extraction signature unchanged; nothing
      executed.
- [x] GREEN 43 passed / 1 ignored; real distributions proven opt-in (g01 8.2.1,
      g02 7.5.3).
- [x] Review PASS; committed as `0c11a40`.

## Task 4 — P1 desktop classification

- [ ] **Blocked: independent desktop audit report not delivered.** No allowlist,
      adapter, callsite classification or signature is invented in its place.
      This task stays empty by design.

## Task 5 — Ren'Py 7.5.3 runtime resources

- [x] Authored resource contract tests written first (`python3
      scripts/tests/test_renpy7_runtime.py`, standard library only).
- [x] Six frozen product files under `runtimes/renpy-7/` (native arm64 payload,
      upstream LICENSE, OpenSSL license, authored NOTICE/components/manifest).
- [x] GREEN: 18 tests, real bytes, negative cases on copied fixtures only.
- [x] Resource review PASS; committed as `785096d`; Ren'Py 8 payload untouched.
- [ ] Serial handoff (notice routing into APK + durable outputs, pack regression
      preserving `.pyo`, generated-project/APK assertions) — owned by the
      integration step, still pending.

## Task 6 — shared integration, cache and full verification

- [x] Ruleset 21 → 22 bumped once with stale-cache regression, version pins
      updated, existing caches not deleted (requirement `LM-T1-04`, design record §4).
- [x] Versioned evidence registered (`Docs/REFERENCES.md` S55–S58) and versioned
      design/execution records added.
- [ ] Full `cargo test --workspace --no-fail-fast` — **not run** for this revision;
      only targeted gates were run while other owners hold shared files. Gate counts
      recorded in the design record §6 apply to evidence HEAD `f7405ba`.
- [ ] Authored Ruby/JS suites, generated-project Gradle unit tests, black-box CLI
      sweep over all inputs — pending.
- [ ] Independent integrated review of the branch — pending (this slice's review is
      `INTEGRATION-GATES-REVIEW.md`: code APPROVED, doc pointers fixed by this
      revision and awaiting scoped re-check).

## Task 7 — Linux beta batch

- [ ] Not started. No APK, no Gradle packaging, no device run. Runtime validation
      remains `NOT_RUN`, gameplay `NOT_TESTED`.

## Open gates carried forward

1. Scoped doc re-check of this slice (M1/M2 closure).
2. R1 packaging slice (notice routing, fail-closed fixture relocation, distribution
   allowlists) — separate owner, review pending.
3. P1 desktop audit and any resulting classification work.
4. Full workspace + packaging + artifact QA on one sealed revision.
5. `cargo fmt`/`clippy` verification once the toolchain components are available.

## Continuity rules

Every step preserves: immutable game inputs, `release/runtime-v2`, existing
Windows APKs, cache files and prior review artifacts. Failures keep their staging
and logs. Rollback on this branch is revert-based only — no reset, clean, stash,
delete, push or master merge.
