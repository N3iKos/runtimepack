# Audio/Save Review Round 1: Process-Restart Namespace Safety

**Author:** Supervisor main agent. Applies to selective-restoration-audio-save Task2.

## Verified Defect

Read operations stopped registering UID namespaces, but NativeSaveRepository still rejects every namespace absent from its in-memory allowlist. Fresh process repository only initializes mz/mv/vnm; therefore current UID saves disappear on restart. Tests must not pre-register the fresh instance or use unguarded mock maps.

## Ruling and Minimal Fix

1. Separate read-only path validation from mutation authorization. `get`/`contains` may read an already-existing namespace directory with validated namespace/slot names under app-private root even when fresh in-memory allowlist lacks it. No mkdir/registration/write on read. Reject traversal, dot-prefixed/invalid names and paths escaping root. Preserve write/delete authorization; simply changing createDir=false gating must not accidentally grant remove/delete access to arbitrary unregistered namespaces.
2. Existing put/registerNamespace behavior and atomic write format remain unchanged. Remove/clear still current authorized namespace only. Legacyvnm reads are alreadyallowed; no migration/cachewrite.
3. Current record presence must be checked independently of nullable read failure: if contains says current exists but get returns null (unreadable/error), raise explicit loadfailure instead of silently falling back to old progress. Empty/corrupt currentcontent continues current dominance.
4. Native regression: repository1registerUID andput currentpayload; repository2 created with DEFAULT constructor reads samepayload/contains true without registering UID. Missingquery creates no directory, bytes unchanged; invalidnamespace/slot rejected; unregistered mutation denied. JS mock must reflect production gating and load/exists behavior rather than unconstrained map.
5. Audio pool diagnostics use own bounded `[CAP-WEB-025]` logger, not lifecycle CAP022. Existing lifecycle logger remainsunchanged.
6. Fix coveringtests and rerun full commands then scopedreview. No plans by worker, savepayload export, APKbuild/device/Git/deletion/cleanup/comments.
