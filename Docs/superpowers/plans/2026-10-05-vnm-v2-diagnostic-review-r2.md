# Diagnostic Review Round 2 Addendum

**Author:** Supervisor main agent. Original diagnostic plan and Round1 addendum remain binding.

## Required Corrections

1. Wire actual manifest sourceHash into BOTH service startAudit and runProbe runner creation, and source identity into retained snapshots. Service init/config API may accept optional sourceHash for testability; no fabricatedhash ifmanifestmissing. Checkpoints with absentproof rerun0, validsourceprofileversion/inventoryfingerprint resumes faithfully.
2. Persist complete bounded per-run problem snapshot (same2000/4MiB cap and exact total/omitted) in runtime report or lightweight adjacent problem-snapshot.json. Across process recreation, read bounded snapshot only matching current engine/source identity; annotate historical runID timestamp. Explicit new audit must suppress previouscompleted data untilnew snapshot. No fallback frompartialeventlist disguising full retention. Limit parameter further trimming must correctly add omitted counts and truncatedflag even if no storage evictions.
3. Every tagged event INCLUDING root/all/DONE rejects mismatchedcurrentrun. Prevent stale runners writing report/checkpoint/combinedsummary with currentrun identity check serialized alongside commit of each app-state write (sourcegame neverwritten). auditIdOverride does not bypass comparison. Preserve standalone runner tests by establishing valid run context rather than disabling safety.
4. MiniJson remains a narrow existing newparser seam for Android/JVM parity, but add explicit maxdepth64/inputUTF8budget and dedicated tests: escapequotes/slashes/newlines/unicode, negative/scientificnumbers, malformedliteral/trailingtext/deepnesting/oversize failsclosed with bounded Exception notStackOverflow. Do not catchThrowable tohide recursion bug. No unpinneddependency/download.
5. Error/problem storage budgets use serialized UTF8 bytes, not KotlinString.length estimates. Count all overhead/stackpaths correctly and ensure oversizeentry omitted metadata exact. Snapshotserde fields remain JSONsafe.
6. JS export enforces <=8MiB UTF8 using TextEncoder or a correct tested fallback; existing JSON.length UTF16invalid. Progressive trimming firstrecentlogs, then boundederrors/problems with precise omitted counters/export_truncated metadata until actual limit, includingprettyprint overhead. Do not silently truncate message paths/stacks without flags or drop allerrors toclaimsuccessfulshare. Ensure multilingualCJK/emoji/unpairedsurrogatehandling tests and nativegetAuditFailures bridge path. Android/share generatedbytes limits validatedatnativeboundary too, oversized report expliciterror notOOM.
7. Correct reportmetrics/lintfail disclosures based rawlogs XML. Avoid irrelevant assumed totals; independentQAfinal willmeasure actual counts. Mirrorpreview after everyAndroiddiagnostic change.

## Scope and Acceptance

No additional runtimebehavior redesign or APKbuild. Sourcegame/saves/streaming/audiofocus sourceprotected. Two missingpersistence/cancelwiring gaps and parser/bytebounds tests are blocking before finalreview. Worker reports corrections, cannot writeplans ordelete/cleanup anyfiles.
