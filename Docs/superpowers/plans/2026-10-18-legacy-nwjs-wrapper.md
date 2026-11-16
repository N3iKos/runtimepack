# Legacy NW.js wrapper DLL classification

Approved: user requested permanent classification fix and retry release build; implementation inline, bug-hunter/reviewer/QA delegated.

Scope: MV/MZ outer wrapper pdf.dll and ffmpegsumo.dll only. Keep payload native DLLs and unknown outer DLLs fatal. No VNM/Tyrano change, no asset-audit change or bypass, no source-game edits. Requirements: PRD-008, TECH-CORE-003/004; root-discovery payload isolation. Evidence: official NW.js v0.12.0 Windows IA32 archive, S50 to record in REFERENCES.md.

- [x] Investigate source and upstream archive; exact DLL hashes match game wrapper.
- [x] Add regression tests; observed CMP-102 on outer pdf.dll before fix (red).
- [x] Minimal classifier fix and S50 evidence; integration_compatibility: 35 passed.
- [x] Independent review approved scoped change; flat-layout packaging exclusion asymmetry remains a separate known gap. QA reports compatibility 35 passed, build 96 passed, detection 52 passed. Full workspace is NOT green: 639 passed / 2 failed / 2 ignored; stale batch Wizard contract fails, doctor test ETXTBSY passed on isolated serial rerun. No device/gameplay test.
- [x] Rebuild CLI (exit 0) and retry release/packed from original game root; no --accept-audit-fail.
- [x] Retry stopped at BLD-104 (exit 1), no APK: 13 errors, 4 warnings, 282 info. No additional fix/bypass. Log: approved Temp/opencode/fbc-release-retry-20261018.log.

Known separate audit findings: 13 errors, 4 warnings, 282 info. Static non-use evidence is not proof of runtime non-use. Do not automatically waive them.

Verification note: git diff --check on touched tracked files reports a blank line at EOF in compatibility_test.rs outside the inserted test block; broad existing uncommitted diff preserved, no unrelated formatting rewrite.
