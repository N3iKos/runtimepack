# Master Roadmap: Track D — Game Runtime Fixes (Show Off & Mizuki And Me)

**Date:** 2026-09-27  
**Status:** Approved  
**Spec:** `Docs/execution-plans/track-d-game-runtime-fixes/design.md`

---

## 1. Objective

Make **both** ported games playable, normal, and stable on Android (MuMu emulator), executed strictly sequentially, all work delegated to sub-agents.

## 2. Execution Order & Gates

```text
Track 1: Show Off (TyranoScript)  [Fast — 1-line root cause + integrity gate]
   │  Gate: title screen renders, audio + input work, logcat free of SyntaxError
   ▼
Track 2: Mizuki And Me (Ren'Py)   [Heavy — new Android runtime subsystem]
   │  Gate: boots to title screen, navigation + save/load work on MuMu
   ▼
Done
```

## 3. Plan Directory

| Order | Plan | Engine | Runtime Approach |
| :---: | :--- | :--- | :--- |
| 1 | `./track-1-show-off-plan.md` | TyranoScript | Fix `asar.rs` data offset + post-pack integrity gate |
| 2 | `./track-2-mizuki-renpy-plan.md` | Ren'Py | Bundle official RAPT runtime + `RenpyActivity.kt` |

## 4. Governance

- One-by-one: do not start Track 2 until Track 1 passes its emulator gate.
- `Raw-Game/` immutable; builds use `--flavor beta`.
- Every task reviewed; every fix verified on MuMu before the track is marked complete.
