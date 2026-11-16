# Tahap 3 debt closure — execution ledger

Plan: `2026-10-24-tahap4-coffin-bprison.md`, Part A only.
Approval: user accepted staged approach on 2026-10-24 with `oke`.
Base: `47bbb43a05fd98e3041d38d0155b02ed859a7303`.
Workspace: existing linked worktree `runtime-v2`, branch `fix/linux-batch-compat-20261022`.

## Scope and protection

No game inputs, web analyzer/shim changes, APK publication, dependency installation, merge or push. Preserve existing AGENTS.md, memory/progress.md and unrelated untracked reports. WSL builds use existing cargo target, root ownership, two jobs. Logs retained outside repository under `_reports/linux-remediation-20261022/tahap3-debt-closure-20261024/`.

## Checklist

- [x] Verify worktree, branch and base.
- [ ] Independent read-only investigation of review assumptions.
- [ ] Baseline full-workspace tests (running).
- [ ] M-2 status propagation with regression.
- [ ] M-1/L-3 notice durability, manifest binding and integrity verification with regressions.
- [ ] L-4 reject stale output without deletion; preserve old artifacts.
- [ ] L-2 negative coverage; L-1 documentation; flag help honesty.
- [ ] Fresh complete workspace/build/core logs.
- [ ] Independent final review and fixes.
- [ ] Report exact results and remaining debt; no claim of gameplay validation.

## Pre-flight rulings

- A2/A3 share notice bytes, manifest hash, checksum list and finalizer. Treat as one lifecycle contract. Prefer existing durable top-level notice + manifest digest over manufacturing an incomplete durable android-project tree after publication. Both were acceptable in original independent review; resolve against actual finalizer before coding.
- A4 uses refusal for stale/mismatched policy output, not deletion or overwriting old notice. Reusing same policy is allowed only when lifecycle consistency is verified.
- Historical plan contains unsupported assertions outside Part A. Coffin binary-read requirements remain undecided until inflated loader inventory. B-Prison global mutation/eval effects cannot be scoped per module without proof; existing guard modelling must be inspected rather than presumed absent. No implementation of Part B/C authorized by this ledger.
- Policy text-carrier trust boundary is investigated, not silently replaced by a broad architectural migration. Hard gate protections must not weaken.

## Evidence

- Initial git status: only pre-existing tracked edits AGENTS.md, memory/progress.md; unrelated untracked reports/plans present.
- Baseline full suite at 47bbb43: exit 0; 1,239 passed, 0 failed, 5 ignored. Full log baseline-workspace.log retained. Earlier missing-log concern is evidence placement, not proof counts were invented.
- M-2 regression RED: serialized COMPATIBLE rather than UNKNOWN; GREEN after policy-status mapping helper wired into CLI. Logs status-red.log/status-green.log. Not yet full-suite reverified after edits.
- L-3 notice end-to-end regression RED: checksums missing exact EXPERIMENTAL-NOTICE.txt entry. No notice production fix yet.
- L-4 stale-output regression RED: Default accepted existing experimental notice. Early refusal added; GREEN not yet run.
- Compiled finalizer notice test added; initial fixture issues corrected (slice coercion/mutable scratch/AuditMode STANDARD). Final RED log must be inspected before implementation; no verifier changes yet.
- Independent negative tests: 50 passed, 0 failed (47 existing + 3 characterization tests); negative-tests.log retained. Covers CMP-AUD-009, caller BLD/SIG blockers, Unknown policy matrix. Advisory suppression not blessed.

## Status

**DONE at `f09fb82`.** All Part A items closed; full workspace suite 1252 passed, 0 failed, 5 ignored.
Report: `_reports/linux-remediation-20261022/TAHAP3-DEBT-CLOSURE-20261024.md`.
Independent review found one residual High (trailing-note carrier injection); fixed with producer reordering plus an end-anchored parser, with regression tests.
Part B (Coffin) and Part C (B-Prison) remain unstarted and unapproved.

Independent investigation found free-text proof parsing in
`core/src/build_plan/audit_policy.rs::class_from_proof_carrier` read the carrier from
an untrusted message that also carries source paths, and that
`split_compatibility` suppressed the `UnsupportedDependency` fallback for any typed
finding including `Advisory`. Both were fixed (see `f09fb82`), with regression tests
for injection before and after the carrier, code mismatch, and the Advisory fallback.
The original review's "Advisory allows" recommendation was rejected: it was fail-open.

Evidence note: the per-item RED logs in `tahap3-debt-closure-20261024/` are
tail/grep excerpts, not full runs. `workspace-final3.log` is the authoritative full
run and the counts below come from it. Re-run any item with
`cargo test -p runtimepack --test <suite>` if fuller evidence is needed.
