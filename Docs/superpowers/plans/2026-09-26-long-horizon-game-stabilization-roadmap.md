# RuntimePack Long-Horizon Game Stabilization Roadmap (No Hard Guardrails)

**Date:** 2026-09-26
**Status:** Proposed
**Owner:** User (Shirai-TL)
**Executor:** Sub-Agent Driven (Supervisor = main agent)

---

## 1. Objective

Deliver stable, minimal-error Android builds for **every game in `Raw-Game/`** through incremental, game-by-game optimization. Each game gets its own dedicated plan; we do not advance to the next game until the current one reaches a safe/minor-error state.

## 2. Guiding Principles

1. **RuntimePack is a technical compiler, not a policy enforcer.** It packages what the user asks. It informs; it does not refuse.
2. **No hard guardrails.** No permanent build rejection based on license, DLL presence, or path traversal. Such conditions become **warnings or informational findings only**, overridable by the user.
3. **Game-by-game iteration.** One game = one plan = one stabilization cycle.
4. **Evidence-first.** Every fix must be backed by audit output, source trace, or runtime log.
5. **Source immutability.** `Raw-Game/` is never modified in place; all transforms occur in working/overlay/container space.
6. **Sub-Agent driven.** Main agent only supervises, dispatches, and integrates. Code edits, builds, and tests are delegated.

## 3. Phase 0 — Remove Hard Guardrails (Prerequisite)

This phase unblocks all previously blocked games.

### 0.1 Tyrano Legal Gate (`TYR-101`)
- **Current behavior:** Tyrano packaging hard-blocked; cannot be overridden.
- **Target behavior:** Convert to an informational/warning finding. Packaging proceeds normally.
- **Rationale:** Redistribution policy is the user's responsibility. A local open-source converter must not refuse to package a user-provided project.

### 0.2 Windows Native Dependency Block (`CMP-102`)
- **Current behavior:** Presence of Windows PE DLLs (e.g. Ren'Py `lib/py3-windows-x86_64/*.dll`, `steam_api64.dll`) hard-blocks build.
- **Target behavior:** Detect, report as warning, **exclude from packaging**, and proceed. Android uses native `.so` binaries; desktop DLLs are inert leftovers.
- **Scope:** Applies to Ren'Py, RGSS desktop packages, MV/MZ NW.js wrappers.

### 0.3 Gradle Project Name Sanitization
- **Current behavior:** Names containing `: / \ < > " ? * |` break `settings.gradle.kts` (e.g. *Fear & Hunger 2: Termina*).
- **Target behavior:** Auto-sanitize invalid characters (replace with `-`/space) for the Gradle `rootProject.name` while preserving the display name (`app_name`).

### 0.4 Audit Outcome Policy Adjustment
- `FAIL` / `INDETERMINATE` remain informative. Build proceeds by default for packaging; the user may opt into strict mode with a future flag if desired.
- Missing-asset findings are reported but do not stop APK emission.

**Phase 0 Exit Criteria:**
- All 8 games can reach the packaging stage without a hard refusal.
- Existing safety/security behavior (path traversal rejection at container level, source immutability, save isolation) remains intact — only *policy* guardrails are removed, not *technical integrity* checks.

---

## 4. Game-by-Game Plan Queue

Each plan follows the same cycle:

```text
Audit (CLI)  ->  Analyze root cause  ->  Implement fix  ->  Build APK  ->  Re-audit  ->  Advance
```

### Batch 1 — Previously Stopped / Blocked

| Plan | Game | Engine | Primary Problems | Target |
|:---:|:---|:---|:---|:---|
| Plan 1 | Fear & Hunger 2: Termina | RPG Maker MV | Gradle name `:`, 571 missing, 15 case mismatch | Build succeeds; missing assets resolved or safely degraded |
| Plan 2 | [Kimochi] NTRdemic | TyranoScript | `TYR-101`, 438 missing, 92 dynamic | Build succeeds; assets resolved |
| Plan 3 | [Kimochi] show_off | TyranoScript | `TYR-101`, 891 path traversal, 34 missing | Build succeeds; paths normalized |
| Plan 4 | Mizuki And Me | Ren'Py | `CMP-102` DLLs, python imports | Build succeeds; DLLs excluded; runtime mapped |

### Batch 2 — APK Built but Audit FAIL

| Plan | Game | Engine | Primary Problems | Target |
|:---:|:---|:---|:---|:---|
| Plan 5 | Forestia | RPG Maker MZ | 22 plugin root deps, 1 case mismatch | Audit clean/minor |
| Plan 6 | The Witch's House MV | RPG Maker MV | 34 missing, 5 case mismatch | Audit clean/minor |

### Batch 3 — Encrypted Archives (INDETERMINATE)

| Plan | Game | Engine | Primary Problems | Target |
|:---:|:---|:---|:---|:---|
| Plan 7 | Hello Charlotte EP1 | RPG Maker VX Ace | `Game.rgss3a` encrypted archive | Archive integrity validated; boot stable |
| Plan 8 | To the Moon | RPG Maker XP | `To the Moon.rgssad` encrypted archive | Archive integrity validated; boot stable |

---

## 5. Per-Plan Standard Structure

Every game plan document must contain:

1. **Audit Snapshot** — engine, outcome, findings breakdown.
2. **Root Cause Analysis** — why each finding occurs (RTP, case, plugin path, archive, DLL, etc.).
3. **Fix Tasks** — delegated to sub-agents, TDD where applicable.
4. **Build Task** — `--flavor beta` build.
5. **Re-Audit Task** — confirm findings reduced to minor/none.
6. **Exit Criteria** — explicit PASS conditions before advancing.

---

## 6. Advancement Rule

```text
Plan N complete  AND  audit outcome in {PASS, minor WARNING only}
        |
        v
Proceed to Plan N+1
```

If a game cannot reach a clean state due to a genuine technical blocker, record it as a **documented limitation** (not a silent failure) and continue — never re-introduce a hard guardrail.

## 7. Out of Scope

- Re-adding any permanent build refusal.
- Modifying `Raw-Game/` sources.
- Redistributing third-party engine binaries (Tyrano) inside RuntimePack itself — this remains a **provenance note**, not a build blocker.
