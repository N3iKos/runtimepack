# SDD ledger — plan: Docs/superpowers/plans/2026-09-20-rpgmaker-web-runtime.md

## Pre-flight Scan
| Task A | Task B | Shared Interface / File | Finding | Ruling |
|---|---|---|---|---|
| Task 1 | Task 2 | `BuildManifest.runtime.family` | `RPG Maker Web` vs `web` routing to `WebActivity` | Both variants mapped to `io.runtimepack.host.WebActivity` in generator.rs |
| Task 2 | Task 3 | `WebActivity` -> `runtimepack_web_shims.js` | Shims injection into game WebView | Injected on `onPageStarted` / injected to DOM |
| Task 2 | Task 4 | `WebActivity` -> `vpad.js` | Virtual pad event bridge | Exposed via `@JavascriptInterface` injecting synthetic `KeyboardEvent` |
| Task 1 | Task 5 | Packaging `www/` content root | Exclude desktop NW.js binaries | Verified by APK file size (< 850 MB vs 2.2 GB) |

Task 1: complete (commits f58cb8a..767f2e8, review clean)
Task 2: complete (commits 767f2e8..54077e7, review clean)
Task 3: complete (commits 54077e7..ab0ccdf, review clean)
Task 4: complete (commits ab0ccdf..57d36ff, review clean)
Task 5: complete (commits 57d36ff..HEAD, review clean)
