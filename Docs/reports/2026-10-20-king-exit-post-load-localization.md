# King Exit Android — Post-load localization repair

## Diagnosis

User reports translated PC game preserves Indonesian after load. Android fontfix APK instead displays Japanese actor/map text and colon-separated numbers in main-menu labels.

Numbers are Langscore script-location lookup keys, not time:

| Key | `en` translation |
|---|---|
| `49037624:35:19` | Barang |
| `49037624:37:19` | Gunakan item |
| `49037624:67:19` | Berkas |
| `49037624:84:19` | Pengaturan |
| `72266426:38:21` | Lokasi |
| `52781037:44:17` | Waktu Bermain |

The game's `en` column contains Indonesian. `Game.ini` selects `[Langscore] Lang=en`.

Old Android `GetPrivateProfileString` inspected argument 4 (integer capacity) instead of argument 3 (string buffer), returned 0 and never read the INI. `LSSetting.load` assigned `result.slice(0...0)`, yielding empty language `""`. In this game's non-patch mode no validation reset occurred. Missing-language translation returned its input: original Japanese for database text, numeric location key for script text. This accounts for both symptoms together.

Earlier timer/nanosecond/save-header explanations are withdrawn. Do not apply timer clamping or frame-count conversion for this screenshot. See evidence S53/S54 in `Docs/REFERENCES.md`.

## Implementation

- `4cf78b3`: real bounded profile read/write compatibility for RGSS.
- `15a3a07`: UTF-8 byte bounds, symlink rejection, write verification.
- `ef4e3e8`: preserve existing configuration on atomic replacement failure; no direct overwrite fallback.
- `ceca4f7`: config-driven Langscore restoration before load translation reconstruction.
- `dac490e`: numeric menu-key regression, historical old-shim failure and current boot/load success for `en` and `zh-cn`.

No original game scripts/CSV, existing saves, or timer counters are changed. The API subset is not full Windows emulation; profile enumeration and concurrency limits must not be advertised as universally supported.

## Evidence and limits

- Independent profile and ordering reviews passed; final whole-change review SPEC PASS / QUALITY PASS with zero blockers (`final-code-review.md`). Nonblocking limitations: duplicated shim modules, standalone loader explicit hook requirement, stdout/alias assumptions, same-UID concurrent filesystem race not atomically eliminated by Ruby APIs.
- Final independent host QA (`final-host-qa.md`): nine Ruby suites without failure (594 mixed harness checks/cases, not end-to-end scenarios), 12 syntax checks, 5 core generator tests, 2 integration generator tests and 232 Android JVM unit tests passed. Android Gradle test task was `UP-TO-DATE`, reusing existing 232-test outputs rather than rerunning all tests. Full workspace was not rerun; counts are not full-game coverage.
- Authored fixtures exercise real production shims and reflect the game's lookup contract; whole untrusted game script was not run on host.
- Existing user's save bytes are unavailable and unverified. Script design intentionally stores original actor names, but this does not certify arbitrary save integrity.
- No device success claim. User performs final Android gameplay validation.

## Candidate packaging decision

New beta package being assembled: `io.runtimepack.game.rpgmkingexit.diagnostic.loadfix` in a unique new directory. Original pack must be byte-identical. Retain the previously tested diagnostic fontfix mapping `VL Gothic>Source Han Sans CN` with outline enabled to avoid reintroducing known freeze while isolating localization changes. This is a candidate-specific baseline, not a change to global original-font defaults. Universal `--font-policy` CLI is not implemented and must not be used as if it exists.

Debug signing and generated-project provenance must be disclosed. Candidate should not replace/uninstall existing apps or overwrite saves. Retain build scratch.

## Manual test handoff (user only)

1. Install the new beta separately. Keep old installation and saves.
2. Start new game or use a verified copy of an existing save. Different package IDs have isolated saves; do not assume old data automatically appears.
3. Open menu. Check `Barang`, `Skill`, `Perlengkapan`, `Status`, `Berkas`, `Pengaturan`, help, actor/map names, `Lokasi`, `Uang`, and `Waktu Bermain`.
4. Save in a new unused slot; load it and repeat checks.
5. Close/restart APK and load the same test slot again.
6. Check dialogue/menu/battle responsiveness, pause/resume, text wrapping, and normal playtime display.
7. Report screenshot and exact failing step if any. Do not overwrite original save slots while investigating.

## User load failure and follow-up

Original Loadfix APK SHA `b48eba2292b54c45d5d5e8a00ae054a79df833f279414119b04abe2a09c38e58` FAILED_USER_LOAD: screenshot reports `runtimepack_bootstrap.rb` line209 `SystemStackError: stack level too deep`. Prior host/static reviews did not exercise game alias interleaving, so they missed this runtime regression.

Investigation `load-stack-investigation.md` reproduces a cycle: RuntimePack installs around the base load method; Langscore aliases that wrapper; reinstallation overwrites a shared original-method alias, so older RuntimePack wrappers call Langscore again indefinitely. Line209 `File.exist?` is merely where stack exhaustion surfaces. No evidence establishes corruption of the user's save.

Follow-up `ab8bc5d` captures a bound Method per wrapper instead of rebinding a shared alias. Both runtime shims updated; new authored production-module regression models actual script alias interleaving, repeat installs/loads, errors and DataManager replacement. Independent reviewer reproduced RED with delivered assets and GREEN after fix, SPEC/QUALITY PASS no blockers. Embedded Ruby behavior and user gameplay remain pending. A fresh V2 package/build is required; the failed first APK is not replaced or certified healthy.

## Current status

`USER_REPORTED_NORMAL_DEVICE_BEHAVIOR`: Pengguna melaporkan bahwa save/load dan teks menu pada kandidat V2 aligned (`8ad4ed18...`) berperilaku normal pada perangkat/emulator miliknya. Laporan tersebut bersifat umum; tidak ada catatan bukti per-item untuk setiap butir checklist, sehingga laporan ini tidak mengklaim verifikasi menyeluruh. `SystemStackError` tidak teramati lagi pada kandidat yang diuji. Bukti host (regression test) menutup perbaikan lookup Langscore dan rekursi alias; perilaku gameplay pada perangkat bersandar pada satu laporan pengguna.

- APK: `C:\Users\MhsymR\AppData\Local\Temp\opencode\kingexit_loadfix_v2_aligned_20261020_185550\kingexit_diagnostic_loadfixv2_aligned.apk`
- Package: `io.runtimepack.game.rpgmkingexit.diagnostic.loadfixv2`
- SHA-256: `8ad4ed186fc694ec61edbdac155cefb712c723f128d68b84139f3ff5c2156088`
- Size: 1,408,021,684 bytes; debug v2 signature valid; arm64-v8a.
- Integrity pack-first and loose sources each 12/12 match expected `058d5926...`; older V2 loose-only mismatch was not actual pack-first runtime mismatch (prior QA attribution corrected).
- Audit remains FAIL/informational (inherited findings, not hidden); debug-signed diagnostic only; inherited manifest ABI list includes x86_64 although actual candidate is arm64 only. No new save corruption claim; no install/device access by agents.

Static verification does not establish live save/load success. Keep old installations/data and test new unused slots. Final gameplay status stays `PENDING_USER_DEVICE_VALIDATION` until user reports the actual APK test.
