# Implementation Plan: Web Platform Early Identity Virtualization & Bounded External URL Dispatch

- **Plan ID:** PLAN-2026-10-10-WEB-PLATFORM-EXTERNAL-URL
- **Design Spec:** `Docs/superpowers/specs/2026-10-10-web-platform-external-url-design.md`
- **Worktree:** `D:\02_Development\Projects_1\runtimepack-vnm-reconstruction\.worktrees\fix-mvmz-logical-root`
- **Status:** DRAFT (FOR CODE REVIEW & SUPERVISOR APPROVAL BEFORE PRODUCTION IMPLEMENTATION)
- **Engine Scope:** RPG Maker MV, RPG Maker MZ (Web Runtime Backend, `web-rpgmaker`)
- **Flavor Policy:** `--flavor beta` (Mandatory Beta Watermark `Beta Test Veldra Community`)

---

## Task 1: Early Platform Identity Virtualization (`Utils.isNwjs` Synchronous Guard)

### 1.1 Goal
Ensure that `Utils.isNwjs()` synchronously evaluates to `false` before any game plugin or scene executes in RPG Maker MV and MZ, preventing desktop OS detection crashes (`os.tmpdir()`) in `shiroin_multilingualSupport.js` and ensuring `StorageManager.isLocalMode()` remains `false`.

### 1.2 Interfaces & File Touch Map
- `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`:
  - Layer 1: Extend HTML interception to inject `<script type="text/javascript" src="web_shims/runtimepack_web_shims.js"></script>` in `<head>` for MV and MZ, guaranteeing synchronous execution before `main.js`.
  - Layer 2: Implement VFS core trailer fence: when streaming `js/rmmz_core.js` or `js/rpg_core.js`, return `SequenceInputStream(rawStream, ByteArrayInputStream(trailerBytes))` where trailer appends `\n;\nif (typeof window !== "undefined" && window.RuntimePack && typeof window.RuntimePack.applyPlatformIdentity === "function" && typeof Utils !== "undefined") { window.RuntimePack.applyPlatformIdentity(Utils); }`.
- `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`:
  - Define `RuntimePack.applyPlatformIdentity(target)` locking `target.isNwjs` via getter `() => false` and no-op setter.
  - Layer 3: Accessor property trap on `window.Utils` with getter returning the underlying instance and setter installing `RuntimePack.applyPlatformIdentity(val)`.
  - Layer 4: Fallback assertion in `pollPatches()`.
- `android/shell/app/src/test/js/web_shims.test.js`:
  - Add unit test suite `RPG Maker MV/MZ Platform Identity Virtualization (CAP-WEB-029)` covering strict/non-strict mode, function declaration hoisting, VFS core trailer execution, and boot environment bypass.

### 1.3 Red Test Specification
In `android/shell/app/src/test/js/web_shims.test.js`:
1. Simulate `rmmz_core.js` loading by evaluating:
   ```javascript
   function Utils() { throw new Error("static class"); }
   Utils.isNwjs = function() { return typeof require === "function" && typeof process === "object"; };
   ```
2. Assert `Utils.isNwjs()` returns `false` (fails on unmodified shims).
3. Simulate `Scene_Boot.prototype.detectLaunchEnvironment()`:
   ```javascript
   let osCalled = false;
   const osMock = { tmpdir: () => { osCalled = true; return "/tmp"; } };
   // Verify that detectLaunchEnvironment returns early when Utils.isNwjs() === false
   ```
4. Verify `osCalled === false`.
5. Verify `StorageManager.isLocalMode()` returns `false`.

### 1.4 Verification Command
```bash
wsl -d Ubuntu-24.04 -- bash -c "cd /mnt/d/02_Development/Projects_1/runtimepack-vnm-reconstruction/.worktrees/fix-mvmz-logical-root && node android/shell/app/src/test/js/web_shims.test.js"
```

---

## Task 2: Bounded External URL Bridge, Native WebView Security, JS Namespace & Declarative Profile

### 2.1 Goal
Provide a secure, bounded external URL opening service via `openExternalUrl`, block in-place remote navigation in `gameWebView` (`shouldOverrideUrlLoading`), expose `RuntimePack.openExternalUrl` on the documented JS namespace, update the capability registry with `CAP-WEB-029` and `CAP-WEB-030`, and create the combined 5-patch declarative profile `rpgmaker-mz-desktop-subproc-external-url.json`.

### 2.2 Interfaces & File Touch Map
- `android/shell/app/src/main/java/io/runtimepack/host/WebExternalUrlPolicy.kt`:
  - Pure Kotlin class implementing `validateAndNormalize(rawUrl: String?, maxLength: Int = DEFAULT_MAX_URL_LENGTH): String?` using `java.net.URI`.
  - Implements `verifyAndroidUriParity(normalizedUrl, androidScheme, androidHost): Boolean` to eliminate differential parser attacks.
- `android/shell/app/src/test/java/io/runtimepack/host/WebExternalUrlPolicyTest.kt`:
  - Pure JVM unit test testing valid HTTP/HTTPS URLs, scheme allowlist, port ranges, control characters, userinfo rejection, and Android URI parity.
- `android/shell/app/src/main/java/io/runtimepack/host/WebBackend.kt`:
  - Update `WebBridge` constructor to accept lifecycle-aware dispatcher lambda `externalUrlDispatcher: ((String) -> Boolean)?`.
  - Add `@JavascriptInterface fun openExternalUrl(url: String): Boolean` routing to `externalUrlDispatcher`.
- `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt`:
  - Implement `shouldOverrideUrlLoading`: allow internal `https://game.local/` navigation, hand off main-frame user gesture HTTP/HTTPS links to `dispatchExternalUrl`, block and log all other navigations (`[AND-SEC-001]`).
  - Implement `dispatchExternalUrl`: validate via `WebExternalUrlPolicy`, post `ACTION_VIEW` intent on UI looper with `CATEGORY_BROWSABLE`, verify `isFinishing || isDestroyed`, sanitize diagnostics log.
- `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js`:
  - Expose `root.RuntimePack.openExternalUrl = function(url) { ... }` routing to `RuntimePackNative.openExternalUrl`.
- `compatibility-db/profiles/rpgmaker-mz-desktop-subproc-external-url.json`:
  - New declarative profile combining the 4 existing patches from `rpgmaker-mz-desktop-subproc-disable.json` with the 5th patch for `QJ-MPMZLib-AnsuzEffects.js` (offsets 2548..2590, fragment SHA `a388de60...`, replacement calling `RuntimePack.openExternalUrl`).
- `Docs/superpowers/compat/COMPAT-REGISTRY.md`:
  - Register `CAP-WEB-029` and `CAP-WEB-030`.
- `tests/integration/staging_profile_test.rs`:
  - Add integration test verifying that `rpgmaker-mz-desktop-subproc-external-url.json` parses and validates against test fixtures.

### 2.3 Red Test Specification
1. In `WebExternalUrlPolicyTest.kt`:
   - Assert `WebExternalUrlPolicy.validateAndNormalize("https://www.patreon.com/c/nlch/home")` returns `"https://www.patreon.com/c/nlch/home"`.
   - Assert `validateAndNormalize("javascript:alert(1)")` returns `null`.
   - Assert `validateAndNormalize("http://user:pass@evil.com/")` returns `null`.
   - Assert `validateAndNormalize("file:///android_asset/game/index.html")` returns `null`.
   - Assert `validateAndNormalize("http:///path")` returns `null`.
2. In `web_shims.test.js`:
   - Assert `window.RuntimePack.openExternalUrl("https://example.com")` delegates to `mockNative.openExternalUrl` and returns `true`.
3. In `tests/integration/staging_profile_test.rs`:
   - Assert `rpgmaker-mz-desktop-subproc-external-url.json` contains 5 valid patches and matches expected pre/post hashes.

### 2.4 Verification Commands
```bash
# 1. JVM Unit Tests (WSL2 with local Android environment)
wsl -d Ubuntu-24.04 -- bash -c "source /mnt/d/02_Development/Projects_1/runtimepack-vnm-reconstruction/.worktrees/fix-mvmz-logical-root/.superpowers/sdd/2026-10-09-mv-mz-logical-root/android-env.sh && cd /mnt/d/02_Development/Projects_1/runtimepack-vnm-reconstruction/.worktrees/fix-mvmz-logical-root/android/shell && ./gradlew testDebugUnitTest --tests 'io.runtimepack.host.WebExternalUrlPolicyTest'"

# 2. JS Shims Unit Tests
wsl -d Ubuntu-24.04 -- bash -c "cd /mnt/d/02_Development/Projects_1/runtimepack-vnm-reconstruction/.worktrees/fix-mvmz-logical-root && node android/shell/app/src/test/js/web_shims.test.js"

# 3. Rust Integration Tests
wsl -d Ubuntu-24.04 -- bash -c "cd /mnt/d/02_Development/Projects_1/runtimepack-vnm-reconstruction/.worktrees/fix-mvmz-logical-root && cargo test --test staging_profile_test"
```

---

## Task 3: Real Candidate APK Packaging & Validation Review (Mode Beta)

### 3.1 Goal
Execute the real candidate APK build for the RPG Maker MZ subject game using `--flavor beta` and `--compat-profile compatibility-db/profiles/rpgmaker-mz-desktop-subproc-external-url.json`, independently verify embedded GPACK container patch integrity, verify debug certificate `84a9`, verify zero source file mutations, and perform passive device preflight.

### 3.2 Preconditions & Invariants
1. `D:\02_Development\Raw-Games\A_Simple_Life_With_My_Unobtrusive_Sister_Ver.1.3_for_windows` is strictly immutable (pre-build inventory logged).
2. Existing deliverable directory `dist_mz_beta_20261009/` is preserved untouched. Output goes to a new supervisor deliverable folder:
   `dist_mz_beta_20261010/`.
3. Staging occurs on external disjoint disk directory.
4. Passive ADB device check (`adb devices -l`) confirms 0 attached targets or logs connected target ID without executing unapproved installations.
5. Android user saves and existing APKs are preserved without destructive uninstalls.

### 3.3 Build Command
```bash
wsl -d Ubuntu-24.04 -- bash -c "source /mnt/d/02_Development/Projects_1/runtimepack-vnm-reconstruction/.worktrees/fix-mvmz-logical-root/.superpowers/sdd/2026-10-09-mv-mz-logical-root/android-env.sh && cd /mnt/d/02_Development/Projects_1/runtimepack-vnm-reconstruction/.worktrees/fix-mvmz-logical-root && cargo run --offline --locked --bin runtimepack -- build '/mnt/d/02_Development/Raw-Games/A_Simple_Life_With_My_Unobtrusive_Sister_Ver.1.3_for_windows' --flavor beta --compat-profile 'compatibility-db/profiles/rpgmaker-mz-desktop-subproc-external-url.json' --accept-audit-fail --accept-audit-unknown --output '/mnt/d/02_Development/Projects_1/runtimepack-vnm-reconstruction/dist_mz_beta_20261010'"
```

### 3.4 Acceptance & QA Criteria
1. Output directory `dist_mz_beta_20261010/` contains:
   - Deliverable APK (`*.apk`)
   - `build-manifest.json`
   - `compatibility-report.json`
   - `asset-audit-report.html` & `asset-audit-report.json`
   - `checksums.txt`
2. `assets/game.pack` inside APK contains all 5 transformed plugins matching expected post-patch SHA-256 hashes.
3. APK signed with debug key `84a9ebf4...` (v2 scheme verified).
4. Zero broad external storage permissions in manifest (`MANAGE_EXTERNAL_STORAGE` absent).
5. Source game directory SHA-256 inventory identical before and after build (0 files added, modified, or removed).
6. QA report generated: `VALIDATION.md`.
