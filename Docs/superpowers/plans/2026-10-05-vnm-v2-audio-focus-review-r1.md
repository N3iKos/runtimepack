# Audio/Focus Review Round 1 Addendum

**Author:** Supervisor main agent. Applies to 2026-10-05-vnm-v2-audio-focus-stabilization.md.

## Verified Corrections

- Two Rust build integrity tests fail because they hardcode full production asset hashes. Preserve dynamic independent oracle comparison and add a separate tiny immutable fixture hash test; do not keep refreshing constants whenever shim changes.
- Current key script dispatches the same bubbling event to window AND document (not body as review paraphrased). Document events bubble to window, so duplicates remain. Use one target with full existing code/keyCode/which semantics; don't replace correct keyCode getters with ineffective constructor-only keyCode.
- Modal open drops captured game gesture without CANCEL. Deliver ACTION_CANCEL on UI thread before clearing ownership; reuse method on hide/show/background when active. Release keys and feedback independently; no double raw gesture streams.
- Unsupported own volume/playbackRate/stream isPlaying descriptors need bounded explicit diagnostics, consistent with reference isPlaying.
- Poll audio namespace fallback includes gs and GS in installer and coordinator.
- Reconcile snapshot/report hashes and XML test counts from actual bytes/results. Do not call actionable Gradle tasks tests.

## Files

Existing patch files plus tests/integration/build_test.rs. Safe prepatch snapshot of added file required. No generator/runtime integrity behavior change required. No save/Gpack changes.

## Regression Acceptance

1. Actual generated key script executed with document->window bubbling simulation yields one window handler per keydown/up, while original property getter engine codes remain correct. Retain canonical MV/MZ direct-state injection behavior without duplicate DOM stream.
2. Actual Kotlin modal/hide/activity path active gesture => one CANCEL to game before state release, no overlay event leak; idle path no invented gesture.
3. Tiny constant-byte fixture over existing hash roots tests normalized sorted path-null-bytes-null algorithm against permanent SHA256 known vector independently computed. Full generated project tests compare to independent current byte-tree oracle and manifest/generated constant equality without full-project fixed values.
4. Existing audio contract tests plus unsupported own descriptors and GS-only registration pass.
5. Whole workspace tests green, lint failures disclosed. Worker updates report only, not plans. Code-reviewer rechecks these findings before diagnostic changes begin.
