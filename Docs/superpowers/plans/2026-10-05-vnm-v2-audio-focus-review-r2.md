# Audio/Focus Review Round 2 Addendum

**Author:** Supervisor main agent. Previous audio/focus plan and round1 constraints remain binding.

## Rulings

1. Remove global gs/GS alias assignment. Installer/coordinator inspect local namespace handles without modifying application globals; GS-only tests supply authentic class aliases in fixture where original game class bodies require gs, not production namespace fabrication.
2. Add monotonic gesture identity owned by production dispatchTouchEvent ACTION_DOWN. Cancel snapshots the active generation; UI-thread runnable dispatches CANCEL only to that same still-active gesture, then clears its ownership. A later DOWN must not receive old CANCEL. De-duplicate repeated cancel requests for same generation. A completed UP invalidates eligibility. Idle cancellation does nothing. Do not synchronously drop an active stream before its CANCEL unless event ordering is separately proven.
3. Cover new DOWN before queued cancel, UP before queued cancel, duplicate cancel requests, and immediate UI cancel with actual production methods. Test state-machine hooks must not duplicate token logic.
4. Inspect queued text-focus exit: disable overlay focusability and restore game focus under a single coherent generation, otherwise requestGameFocusSafe increments token and can invalidate the pending disable runnable. Tests verify actual focus flags after text exit/Done; no stale nested focus runnable after modal reopens.
5. Report full tests from raw results and XML. Previous273 Rust report is invalid. Counts are execution evidence, not acceptance requirement.

## Scope

Only current shim/host/tests/report plus existing registry where applicable; no native save or Gpack changes. No code comments, plans by workers, sourcegame/device/deletion/Git operations. Final review follows before diagnostics implementation.
