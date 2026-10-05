# Deep Asset Audit Master Roadmap

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver authoritative multi-engine static asset auditing, build policy gates, deterministic reports, runtime load instrumentation, optional probes, and a non-authoritative in-game viewer.

**Architecture:** Implementation is split into five independently reviewable plans. Host-side Rust owns authoritative static status; engine adapters own extraction semantics; Android/runtime instrumentation produces separate runtime evidence; overlay only renders structured state.

**Tech Stack:** Rust, JSON Schema, Kotlin/Android, Ruby, JavaScript, Node test harness, Gradle/JUnit.

**Spec:** `Docs/superpowers/specs/2026-09-24-multi-engine-deep-asset-audit-design.md`

## Global Constraints

- Support only RuntimePack v1 engines: XP, VX, VX Ace, MV, MZ, Ren'Py, Tyrano.
- Input game directories remain immutable.
- Static audit never executes game code or game-native libraries.
- `FAIL` and `INDETERMINATE` block builds by default.
- Overrides cannot bypass security, legal, container-corruption, ambiguous-engine, unresolved-runtime, or unsupported-native-dependency blockers.
- JSON is authoritative; HTML and overlay are viewers only.
- Autonomous builds use `--flavor beta`.
- No unpinned dependencies.

---

## Execution order

1. `2026-09-24-deep-asset-audit-foundation-plan.md` (6 tasks) — **COMPLETED & MERGED**
2. `2026-09-24-deep-asset-audit-adapters-plan.md` (6 tasks) — **COMPLETED & MERGED**
3. `2026-09-24-deep-asset-audit-reporting-plan.md` (3 tasks) — **CURRENT**
   - Task 1: Canonical Report Writer & Content-Hash Cache
   - Task 2: Standalone Offline HTML Viewer & Manifest Link
   - Task 3: Orphan Analysis, Bounded Network Audit & Verification Docs
4. `2026-09-24-deep-asset-audit-runtime-plan.md` (3 tasks) — **NEXT**
   - Task 1: Runtime Audit Protocol, Native VFS Evidence & In-Game Overlay Cleanup
   - Task 2: Multi-Engine Loader Instrumentation & Android Activity Bridges
   - Task 3: Sampled & Exhaustive Runtime Probe Engine with Checkpoints
5. User Device Live Verification (Manual testing on phone/emulator by user)

Plans 2 and 3 may proceed in parallel after Plan 1. Plan 4 requires the Plan 1 schema and Plan 3 persistence contract. Plan 5 requires Plans 3 and 4.

## Milestone gates

- **Gate A:** `runtimepack audit` emits schema-valid deterministic results and build policy blocks correctly.
- **Gate B:** all v1 adapters produce declared-scope dependency graphs without execution.
- **Gate C:** cache, JSON, HTML, manifest, and provenance are deterministic and sanitized.
- **Gate D:** runtime loader events and sampled/exhaustive probes produce separate runtime reports.
- **Gate E:** overlay cannot fabricate success; device fixtures prove missing/decode/fallback failures are visible.

## Final verification

Run from repository root unless noted:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node android/shell/app/src/test/js/diagnostic_suite.test.js
node android/shell/app/src/test/js/web_shims.test.js
node android/shell/app/src/test/js/overlay_interaction_regression.test.js
ruby android/shell/app/src/test/ruby/asset_audit_test.rb
```

Run from `android/shell`:

```powershell
.\gradlew.bat testDebugUnitTest
```

Device/build validation is performed only after explicit user confirmation and must use `--flavor beta`.
