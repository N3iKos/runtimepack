# Diagnostic Review Round 3: Strict Snapshot Identity and Atomic Run Commits

**Author:** Supervisor main agent. Final focused remediation; do not rewrite prior plans.

## Verified Remaining Gaps (live source)

- AssetAuditService.getFailuresJson disk fallback accepts missing engine/source or currentSourceHash=null as match; it returns all persisted items ignoring requested limit and can expose previous run after reset.
- AssetProbeRunner checks run ID before writes without shared serialization with reset/newRunId. Checking a volatile value alone is not atomic commit protection.

## Required Fixes

1. Add explicit session/run state: process-recreated idle may consult persisted snapshot; once a new audit/reset/run has begun, persisted prior snapshot cannot be fallback for that session. New run with zero current issues returns zero current issues with currentrun ID, not old failure data.
2. Persisted restore requires nonnull exact sourceHash and engine identity plus supported snapshot schema/profile version. Missing/mismatched identity yields explicit unavailable/notrestored metadata, not fake historicalsuccess. Read through size-limited stream before MiniJson; cap includes JSON overhead and UTF8 budget. Returned snapshot respects limit0..2000 on both cached and persisted paths and exact omitted/truncated fields; preserve original auditID/timestamp and historical=true label when restoring. Do not replace previousrun auditID with currentrun pretend.
3. Use one actual runLock around reset/newRunId/run transition and recordEvent tag eligibility+append, plus narrow `withCurrentRun(auditId, action)` for committing checkpoint/report/snapshot/combinedsummary. Writes under lock must be lightweight metadata only; don't hold lock during media/decoding. Caller interrupted/cancelled must pass activecondition and current ID before commit; samepath temporary filenames cannot race unsynchronized.
4. On completion mark activeRunner idle only if its runID matches; earlier worker cannot null out new activeRunner. No broad async service redesign.
5. Tests production service and runner: existing currentissues1 plus oldsnapshot890 -> currentonly1; newaudit noissues -> zero; restart exactidentity -> allbounded890; restart missing/mismatchedsource/engine/profile -> no restore; persisted limit10 -> correctcounts; oversize malformed snapshot bounded failure; orchestrated reset between runner check and commit never overwritesnewrun metadata; oldDONE ignored.
6. Fix diagnostic report counts later through independentQA; no arithmeticplaceholder assertions or claims exact380 unless rawverified.

## Constraints

Only diagnostic service/runner/tests/report and necessary models metadata. No audio/native save/Gpack changes, APKbuild/devices/deletion/Git/newcomments/workerplans. Code-reviewer rechecks these exact defects before build.
