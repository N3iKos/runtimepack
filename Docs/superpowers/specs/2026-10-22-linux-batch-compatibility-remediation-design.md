# Linux batch compatibility remediation — versioned design record (2026-10-22)

Status: **PARTIAL.** Records what is actually implemented and verified on the Linux
fix branch, and what remains gated. It is **not** a completion claim: no APK has
been produced on Linux for this revision, Ren'Py runtime boot is `NOT_RUN`, and
desktop dependency classification (P1) is still pending an independent audit.

- Canonical source: `runtime-v2` worktree, branch `fix/linux-batch-compat-20261022`.
- BASE (release reference): `dd9e3f71794f660a57bbe983dea6def4a325b247` (`release/runtime-v2`, unchanged).
- Upstream evidence IDs are registered in `Docs/REFERENCES.md` (S55–S58 added here).

### 0.1 Self-containment rule

This record is the authoritative statement of the requirements it uses. Requirement
IDs and acceptance rules are defined **inline** below; the external coordination
artifacts in §0.2 are supporting evidence only. A reader of this file alone can
resolve every ID and rule cited in this repository's tests and docs without access
to the coordination batch directory.

### 0.2 External coordination artifacts (evidence only, not repository content)

These files live outside the repository, under the batch evidence directory
`_reports/linux-remediation-20261022/` (a coordination/report tree, not product
source). They are recorded by artifact name plus SHA-256 of the exact bytes read;
no machine-local absolute path is embedded in this record.

| Artifact (relative to `_reports/linux-remediation-20261022/`) | Bytes | SHA-256 |
|---|---|---|
| `SPEC.md` | 22,965 | `0c594324730c80f67c951a20e7dcd5ef6a47a1cadcf004a9db47a7ea6aef4e0e` |
| `IMPLEMENTATION-PLAN.md` | 21,902 | `ef02a055bf87e799fb165cfaba55ea1834421b70e1811d1408b7e0ef3e4554b8` |
| `INTEGRATION-GATES.md` | 3,571 | `6999aba7840763cab360d9b82428160690c520890f76c12242ff9ecad74d96c5` |
| `T1-REVIEW-RULING.md` | 1,647 | `c82f727494d266b2d74773487e4c609ee6f2175f241b308ec1f03cd72524d446` |
| `T1-FIX-REVIEW.md` | 9,756 | `5597bc500ffc2071d3b783a35181cbafe106fe35e5d7227fb3b852628709ee32` |
| `R1A-FINAL-REVIEW.md` | 12,061 | `433246d33a73cb57261c0d1fa38a117d77e433fb32a93119b1c615fa19b340ae` |
| `T5-RUNTIME-REVIEW.md` | 9,601 | `7cb7df474f5b5dc50b22a32c16b8d52bde60dc074a1b2d2be56aed0d1c2f7e14` |
| `D1-APPROVAL.md` | 11,669 | `89c2a544a0dc16c8be1b20dc74c44a8fa67e43082d6211243316c5fcc41af74b` |
| `D1-FIX4-REPORT.md` | 13,295 | `b17badeead1b2b6cdb8a9b05b8cabd058698a2f751510ab533c7d82d724feeb7` |
| `audit-detector-and-tyrano.md` | 23,224 | `310bc94a8a963eee3e00f5f97e5fced9dd988e52cdfa7368fbd1ab271049f863` |
| `INTEGRATION-GATES-IMPLEMENTATION.md` | 11,342 | `10e26622fe1098d4d7d87ab0fedd26bdc2ba529bf688e0e386ec746d461fea66` |
| `D1-FIX2-REAL-G06-EXTRACT.log` | 639 | `aeff21464cb7df05f9f264b8a0975459a53218b09cf7bfc80b0e13155ef1ece0` |
| `D1-FIX4-REAL-G06-PROBE.log` | 2,640 | `3d9fbb8acb815896d6a28a00a793fe1ed39602cbfd05bd546b69a51232bce1ef` |

`INTEGRATION-GATES-REVIEW.md` (this slice's review) is stored as
`_reports/linux-remediation-20261022/INTEGRATION-GATES-REVIEW.md` (13,027 B, SHA-256
`0ce376c9b11a208e1ef078c0b518f0d4cfb01a00b59a7ad3e151e51e139accdb`). Its verdict was
code APPROVED with two documentation-pointer findings; this revision closes those
pointers, and the review is cited as review commentary rather than as measured
evidence.

Gate logs produced by this slice are summarised inline (§6) rather than linked, so no
dangling log path is required.

## 1. Scope actually delivered

| Slice | Files | State |
|---|---|---|
| T1 Tyrano `loadjs` runtime base | `engines/tyrano/src/asset_audit.rs`, `tests/integration/tyrano_asset_audit_test.rs` | Committed (`4da7a27`), review PASS |
| R1A Ren'Py static version metadata | `engines/renpy/src/scanner.rs`, `version_metadata.rs`, `lib.rs` | Committed (`0c11a40`), review PASS |
| T5 Ren'Py 7.5.3 runtime resources | `runtimes/renpy-7/**` (6 files), `scripts/tests/test_renpy7_runtime.py` | Committed (`785096d`), resource review PASS |
| Audit ruleset bump | `core/src/asset_audit/cache.rs` (21→22) | Committed with this record |
| D1 web arbitration | `engines/rpgmaker_web/src/{lib.rs,detection_arbitration.rs}`, `tests/integration/detection_test.rs` | Committed (`f7405ba`), SPEC/QUALITY PASS per `D1-APPROVAL.md` |

Engine commits `8e036ed`, `6bbfdbf`, `e04f0f0`, `b39ee64` are D1's earlier
scoped commits on this branch.

## 2. T1 — Tyrano tag-base parity (implemented semantics)

- `loadjs` resolves through exactly one candidate: explicit `data/` form passes
  through, otherwise `data/others/{storage}`. The raw-storage alternate is gone.
  Evidence: shipped `tyrano.plugin.kag.tag.loadjs` declares `vital: ["storage"]`
  and loads `./data/others/ + pm.storage` (S56).
- Storage that is itself lexically unsafe (absolute, drive letter, UNC, NUL, or a
  traversal that escapes the content root) keeps its unsafe signal and stays a
  hard `CMP-AUD-009` blocker; missing safe targets stay genuine missing findings.
- `call`/`jump` scenario resolution is unchanged (`data/scenario/` base).
- **Known coverage gap:** the real `loadcss` tag declares `vital: ["file"]` and
  builds its href from the literal `pm.file` (S56). The auditor does not model
  that form; it keeps the legacy pre-T1 two-candidate rule for a `loadcss
  storage=` attribute. No CSS adapter is implemented and none is claimed.

## 3. R1 — Ren'Py 7.5.3 static detection and resources

- Version metadata parsing is extended only for recognised upstream `if PY2:` /
  `else:` dual-generation metadata, with bounded reads, no `eval`/import/pickle
  and no `.rpy`/`.rpyc` execution. Ambiguous or contradictory evidence stays
  `RNP-110`.
- Detected game version and pinned backend version stay distinct: a game shipping
  Ren'Py 8.x metadata remains a game-version result, not a cross-minor
  compatibility claim.
- Vendored resources are the official RAPT arm64 payload plus exact upstream
  license/notice/provenance files (S57). OpenSSL 1.1.1g is statically linked, so
  only retained attribution/notice obligations are recorded — no invented source
  offer, no relink obligation, no blanket compliance statement. PGP signature is
  not verified and the exact build recipe/source commit remains `UNKNOWN`.
- The resource contract is proven by an authored standard-library-only Python
  suite (18 tests) over real bytes. It does **not** prove APK packaging, notice
  routing, Android boot or playability.

## 4. Audit ruleset version 22 (requirement LM-T1-04)

**LM-T1-04 (defined here, authoritative for this repository):** the audit ruleset
version is bumped exactly once per coordinated semantic change. This change bumps
`CURRENT_AUDIT_RULESET_VERSION` from 21 to 22 after the T1 `loadjs` runtime-base
semantics land. The bump must be accompanied by a stale-cache regression proving a
21-keyed entry is a miss (never a fake hit), must not delete existing cache files,
and must not be applied speculatively for a semantic change that has not landed. A
further audit semantic change after caches are produced requires another single
coordinated bump (23), never a silent reuse of 22.

`CURRENT_AUDIT_RULESET_VERSION` is therefore 22. Properties pinned by regression:

- a ruleset-21 cache entry is a miss for a current-ruleset request — no fake hit;
- a current-ruleset entry is a genuine miss, then a real hit after `put`;
- the bump path does not delete existing cache files;
- cache identity still binds source hash, engine family, audit mode, ruleset
  version, toolchain version, limits, network policy, detected version and content
  root, so one engine's result/provenance can never satisfy a different request.

If a later audit semantic change lands after caches are produced, the version is
bumped again (23) in the same coordinated step; it is not bumped speculatively
here for P1, which is still pending.

## 5. Non-goals and limits of this record

- No claim of 19/19 game builds, no Linux beta APK, no Gradle run, no device or
  gameplay validation (`NOT_TESTED`), no Ren'Py 7 boot proof.
- No general `runtimepack.toml` configuration loading; CLI flags remain the only
  implemented override path.
- No desktop dependency allowlist (P1) is frozen here; that task has no invented
  signatures until its audit report exists.
- No change to shared path/URI/ASAR containment, RGSS local-diagnostic
  restrictions, or Android permission posture.
- `cargo fmt --check` and `clippy` remain unverified in this environment
  (components unavailable); formatting is hand-checked only.

## 6. Verification evidence (actual counts)

Evidence HEAD: `f7405ba3049aa1716468701172d91191e65ebeb9`. The gate logs for the
CLI/cache/web targets were produced against the same working-tree content while HEAD
was still `0c11a40`; those two commits differ only in D1-owned engine/test files, so
the counts below apply to this HEAD. No count here is a full-workspace claim.

| Gate | Result | Source |
|---|---|---|
| Tyrano asset audit | 48 passed / 0 failed | committed T1 slice (`4da7a27`) |
| Ren'Py engine package | 43 passed / 0 failed / 1 ignored | `R1A-FIX2-GREEN.log` (batch evidence tree) |
| Ren'Py real distributions (opt-in) | g01 8.2.1, g02 7.5.3, 1 passed each | `R1A-FIX2-REAL-G01.log`, `R1A-FIX2-REAL-G02.log` (batch evidence tree) |
| Ren'Py 7 resource contract | 18 tests OK | `scripts/tests/test_renpy7_runtime.py` (in-repo) |
| CLI root discovery | 17 passed / 0 failed / 1 ignored | gate run for this change |
| Asset audit report + cache | 26 passed / 0 failed | gate run for this change |
| Web asset audit (ruleset pin) | 40 passed / 0 failed | gate run for this change |
| D1 detection arbitration | 114 passed / 0 failed / 0 ignored | `D1-APPROVAL.md` (batch evidence tree), commit `f7405ba` |
| Full workspace / APK / device | **not run** | later serial integration |
