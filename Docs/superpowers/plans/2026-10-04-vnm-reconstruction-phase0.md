# VNM Reconstruction Phase 0: Read-Only Evidence Audit

**Author:** Supervisor (main agent).
**Goal:** Identify what survives and what must be reconstructed before any implementation or build.
**Architecture:** Evidence-first recovery from the current repository, readable recovery artifacts, and the newly downloaded game. No recovered code is executed. Reconstruction remains an explicitly approved VNM experiment, not an implicit expansion of canonical v1 support.
**Implementation spec:** To be authored by the supervisor after this audit; this document authorizes investigation only.

## Global Constraints

- Working root: `D:\02_Development\Projects_2\runtimepack-master`.
- Recovery evidence root: `D:\Old-Recvery\Project_2\RPGM-P\.worktrees\vnm-exp2`, read-only.
- Game input: `D:\Old-Recvery\Project_2\RPGM-P\Raw-Games\Time for Punishment Class ~Taking Lessons for My Boyfriend~`, read-only. User confirms a fresh download of the same version.
- No deletion, cleanup, reset, restore-in-place, package installation, commits, merges, pushes, or builds during Phase 0.
- No MuMu, ADB, emulator launch, UI interaction, or gameplay testing. User owns gameplay QA.
- No source modifications by audit workers. Return findings in tool response; supervisor authors plans and specifications.
- Do not run recovered scripts, executables, or game bootstrap.
- Do not log secrets or dump provider configuration.
- Inspect only named roots and known evidence locations; no broad drive scan.
- Preserve uncertainties. A historical passing test or playthrough is not proof of current reconstruction correctness.

## Historical Acceptance Target

The previous full-overlay beta with raw-entry GPACK streaming was manually played to completion by the user. Reconstruct its behavior, not a byte-identical promise without valid surviving artifacts.

Components to inventory:
1. VNM static detector, runtime resolver, asset audit with encrypted manifest INDETERMINATE.
2. Buffer/JSZip, native GameStorage, desktop window compatibility.
3. Audio start/stop and disposed callback ownership.
4. CAP-WEB-020 voiceDuration delegation, unavailable source/buffer degradation diagnostics, valid original behavior and early skip semantics.
5. CAP-WEB-021 pre-engine environment script plus synchronous post-SCRIPTS voice hook.
6. Full overlay input ownership/focus lifecycle; diagnostic passive-watermark mode if evidence supports restoration.
7. Bounded raw-entry GPACK streams, independent stream closure, positional source concurrency/base offset, rejection of unsupported flags; authenticated encrypted path retained with documented memory limits.

## Task 1: Repository and artifact inventory — executor

- [ ] Read AGENTS.md and applicable nested instructions. Read memory protocol and six canonical documents in mandated order. Report missing documents explicitly.
- [ ] Determine whether the working root is a Git checkout or source export. Check `git rev-parse`, status, recent log, refs and relevant historical commit availability only if Git metadata exists. Do not initialize Git or fetch.
- [ ] Inventory current component source/tests and research/postmortem/plans. Return exact paths and meaningful line references.
- [ ] Inspect selected recovery source, patch, ledger, and report files for readable text and plausible structure. Do not apply patches; do not treat filenames as proof of valid contents.
- [ ] Check the known historical Backup location and selected recovery equivalent for `feature-vnm-engine.bundle`, `vnm-engine-archive.zip`, last streaming APK and review patches, only where paths exist. Archive listing/testing is read-only; no extraction into repository.
- [ ] Statically inspect fresh game entry and targeted source methods. Record critical hashes/version markers and identify representative asset signatures without executing game code.
- [ ] Return a component matrix: current source / valid surviving artifact / reconstructed requirement / unknown; include testing commands from actual configs and any missing binary/runtime/toolchain requirements.

## Task 2: Evidence review — code-reviewer

- [ ] Verify the component matrix against concrete files and historical requirements.
- [ ] Flag corrupted or unsubstantiated recovery claims and spec conflicts.
- [ ] Distinguish direct source restoration from behavior reconstruction.
- [ ] Return approved inventory or specific corrections. No changes or execution of recovered code.

## Task 3: Supervisor specification and implementation plan

- [ ] Read reviewed evidence and relevant canonical source context.
- [ ] Choose safe isolation based on actual Git state; do not assume worktree support exists.
- [ ] Author reconstruction design and staged implementation plan personally. Define precise files/interfaces, failing regression cases, review gates and build acceptance.
- [ ] Define baseline preservation and non-destructive backup before code changes; do not invent an external backup destination.
- [ ] Delegate bounded implementation only after its plan is written and checked. Use executor and code-reviewer, with non-device QA verification; no general/explore agents.
- [ ] Deliver beta APK with full overlay and exact identity/hash after verification. Leave gameplay stability pending user testing.

## Phase 0 Exit Criteria

Reviewed inventory identifies the actual baseline, readable evidence, missing components, source-input condition, repository isolation feasibility and build prerequisites. Any blocker is explicit. No implementation, deletion or game execution has occurred.
