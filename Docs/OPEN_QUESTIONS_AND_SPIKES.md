# Open Questions and Required Spikes

## SPIKE-RGSS-ANDROID-001 — RGSS Android runtime base

Question:

Should v1:
A. adapt an Android-MKXP lineage,
B. port/currentize mkxp-z for Android,
C. maintain a narrow custom fork combining pieces?

Required outputs:

- build feasibility;
- ARM64 startup;
- VFS integration points;
- Ruby version mapping;
- SDL/rendering dependencies;
- license matrix;
- maintenance cost;
- Win32 shim extensibility.

Do not decide solely from JoiPlay's old public code.

## SPIKE-RENPY-001 — Supported version set

Question:

Which exact Ren'Py runtime generations should v1 promise?

Need:

- current upstream Android build feasibility;
- historical game compatibility;
- package sizes;
- license bundle;
- static version-detection reliability.

Output must pin exact versions.

## SPIKE-WEB-001 — Web origin/VFS bridge

Compare:

- AndroidX WebView local asset loader;
- custom WebViewClient interception;
- localhost embedded HTTP server.

Criteria:

- same-origin;
- streaming encrypted content;
- MIME handling;
- range requests/video;
- service worker behavior;
- security;
- startup cost.

## SPIKE-MVNODE-001 — Node/NW compatibility minimum

Build a corpus of common API use:

```text
path
fs
process
nw.Window
require dynamic/static
```

Define the smallest subset worth implementing.

Do not emulate Node wholesale.

## SPIKE-TYRANO-LEGAL-001

Status: BLOCKING FOR DISTRIBUTION.

Obtain:
- explicit permission or clarified license;
- allowed redistribution model;
- whether user-supplied engine files may be repackaged by a conversion tool;
- notice/attribution conditions.

Until resolved, release build refuses Tyrano with `LIC-310_TYRANO_REDISTRIBUTION_BLOCKED`.

## SPIKE-CONTENT-001 — Container binary format

Finalize:
- binary layout;
- index encoding;
- compression;
- AEAD choice;
- random access;
- very large file behavior;
- forward compatibility.

Requires ADR before production implementation.

## SPIKE-SAVE-001 — Web save persistence

Verify MV and MZ save implementations under chosen stable local origin and ensure package updates do not alter origin identity.
