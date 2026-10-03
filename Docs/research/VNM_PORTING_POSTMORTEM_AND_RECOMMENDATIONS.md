# VNM Porting Postmortem and Recommendations

## Status and purpose
VNM experiment is paused, not stable, and NOT merged to master. Preserve `feature/vnm-engine` and its source/tests/evidence in an archive. This document lives OUTSIDE that worktree. Latest reviewed experimental revision: `47a5af41622cfff376891aed50c3291cbb14f832`; check actual Git state during archival rather than relying on this note.

## What actually worked
User could play normally through chapter 3 while leaving default overlay untouched. QA observed title and some dialogue progression. This is partial gameplay evidence, NOT ending coverage or proof of save/load, repeated layout toggles or general VNM compatibility. Build/test success is not device acceptance.

## Problems, evidence, and limits

| Area | Observed / demonstrated problem | Current confidence / recommendation |
| --- | --- | --- |
| Buffer / JSZip | Start Game raised Buffer.isBuffer missing; shim Buffer was a plain object while JSZip detects Node support and expects callable Buffer. | Real-library ZIP regression and reviewed constructor fix exist. Preserve tests; do not expose incomplete Node capabilities in new adapter. |
| Save | Slot headers visible in session, restart empty; same-session load JSON.parse empty. VNM selected desktop route because `$PARAMS.isOffline` and supplied `window.nw`; fs.writeFileSync no-op discarded saves. | Source-derived GameStorage native boundary fix and multi-VM tests exist. Real slots 1/2/3 save/restart/load NOT adequately verified. Preserve any saves; do not claim recovered progress. |
| Display settings | setResizable missing in enterFullscreen. | Reviewed window contract fix exists. Prefer Android capability mapping, not pretending full NW.js environment exists. |
| Audio stop/dispose | Native stop before start exception during scene teardown. | Reviewed source-node lifecycle patch; not a general audio stability claim. |
| Audio late callbacks | User skip error AudioNode.connect overload at ENGINE44215 through onended44194; original loop callback may play disposed buffer with null gain node. | Reviewed callback ownership/generation guard and native WebIDL setter delegation exist. Natural looping and scene transitions require real device checks. |
| Normal dialogue / voice | User later observed null.buffer in Component_MessageBehavior.voiceDuration; skip bypassed it. | Unresolved. Inspect real voice-reference lifetime and loading vs disposed state; do not merely return zero duration or suppress exception. |
| Overlay show/off / editor | User repeatedly correlated toggles with frozen Live2D, intro/title clicks and occasional errors; untouched default overlay permitted chapter3. | Correlation strong, not proof all bugs are overlay. Focus/blur pause, touch ownership and release errors found; reviewed fixes still lack complete 10-cycle device proof. Preserve full overlay support in future solution. |
| Font | User observed PC/Android typography difference; Rubik WOFF ciphertext rejected by browser OTS while Tallys plain WOFF works. | Source/header evidence supports font pipeline mismatch. Verify PC font path before choosing explicit decrypt/load adaptation. |
| Static audit | Encrypted RESOURCES/SUMMARIES parsed as plaintext, zero references, 1722 assets falsely called unused, misleading PASS. | Corrected to INDETERMINATE/incomplete, orphan inference suppressed, ruleset bumped. Inventory is not dependency coverage. |
| Runtime probe | Initially encrypted images yielded890 decode failures; later incomplete graph checked only5 bootstrap files and claimed EXHAUSTIVE PASS. | Reviewed full inventory union and ciphertext INDETERMINATE fixes exist. 890 are encrypted PNG/JPG, NOT WAVs. Audio readability is not audio playback validation. |
| Intro / emulator | Intermittent perceived stall; some QA relaunches reached title. Wrong MuMu display mapping and coordinates corrupted several tests. | Video MIME/range or codec warnings alone do not prove intro cause. Native crash claim retracted for missing raw evidence. |

## Process errors to avoid
- VNM ease underestimated: reusing WebView backend does not imply engine compatibility.
- Original task-level reviews were skipped; synthetic mocks missed actual offline storage route, WebIDL event registration and real pointer releases.
- QA false positives included stale five-file reports after update, globalData/settings mistaken for save slots, audio playback mistaken for Options disposal success, and wrong button coordinates mistaken for modal defects.
- Unverified renderer crash and broad runtime stability claims must not be treated as facts. More than one hour process lifetime does not prove sustained interactive acceptance.
- Keep raw logs with APK hash, installed build identity, timestamps, display/coordinates and observed scene after each action. Report NOT_TESTED when blocked.

## Recommended next architecture (separate experiment)
1. Start from master in a NEW worktree; old branch is reference, not automatic cherry-pick source.
2. Engine-specific VNM adapter: verified browser/desktop selection, storage, input/focus and compatibility contracts. Do not inject RPG Maker/NW APIs just because other games need them.
3. Shared infrastructure may remain Android WebView/GPACK/security/build plumbing; engine behavior must be isolated explicitly at build time.
4. Preserve watermark, vpad, show/off, editor, diagnostics and touch feedback. Establish exclusive gesture ownership and genuine focus/lifecycle transitions; no double full gesture delivery or routine focus events that clear input/recreate audio.
5. Baseline no-overlay test, if needed, is TEMPORARY diagnosis only; acceptance build must contain full stable overlay. Native Android overlay is an option only after comparison demonstrates two-WebView limits; it will not itself solve storage/audio contracts.
6. Audit inventory, decoded format verification, dependency graph completeness and gameplay QA are separate outcomes. Never mark ciphertext decoded or unreadable manifests complete.
7. Prioritize durable authentic save roundtrip before cosmetic changes, then Options, overlay cycles, voice/audio skip, font.

## Required future acceptance
- Normal dialogue and choices, not only skip; continuous measured gameplay through relevant chapter3 scenes.
- Slots1/2/3 actual game UI save -> title load -> process restart -> correct story restoration, payload and Header preserved.
- Overlay show/off/editor at least10 cycles at title/Live2D with responding taps/keys afterward; no feature removal.
- Options fullscreen/window toggles, natural BGM endings/loops, fast skip scene transitions and voice-duration logic without exceptions.
- User HP confirmation separate from MuMu; no blanket stability for all VNM versions/titles.

## Archive requirements
Approved cleanup ONLY generated dist* directories explicitly inventoried, target, Android local .gradle/app build and runtimepack cache. Preserve source, runtimes/libs, tests, notes, logs, screenshots and untracked plans. ZIP is private (may include QA/private evidence); do not publish it unreviewed.
Linked `.git` points to main repo and is not portable: preserve independent verified FULL Git bundle plus archive manifest/file hashes. Do not remove branch/worktree registrations or force cleanup as part of this archive task. Do not delete Raw-Game or backups outside the worktree.
