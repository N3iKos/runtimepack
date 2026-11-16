# Implementation Plan: Web Asset URI Path Normalization & Encrypted Fallback Fix

**Date:** 2026-10-10  
**Tracking ID:** BUGFIX-WEB-URI-001  
**Scope:** Generic URI path normalization for canonical game origin, consistent percent-decoding in VFS resource interception, and encrypted asset fallback resolution without game-specific hardcoding.

---

## 1. Problem Statement & Root Cause

1. **False HTTP 403 on Canonical Game URIs with `@` / `%40`:**
   - In `SharedNavigationPolicy.isCanonicalGameUri`, the check `rawUrl.contains('@') || rawUrl.contains("%40")` was applied globally to the entire URL string instead of strictly to the `authority` / `userinfo` component.
   - Per RFC 3986 §3.2 and §3.3, `@` and `%40` are valid characters in path segments (`pchar`) and query strings, but represent user credentials when in the authority component.
   - Internal game asset requests such as `https://game.local/img/pictures/mio_tachie_%40_%40.png_` were falsely rejected as untrusted remote requests and returned HTTP 403 Forbidden.

2. **Inconsistent Encrypted Fallback in `VfsResourceInterceptor`:**
   - When requests for `.png` or `.ogg` were intercepted, direct percent-decoding occurred, but the encrypted extension fallback (e.g. `logicalPath + "_"`, `.rpgmvp`) constructed fallback paths from the raw percent-encoded `logicalPath` (containing `%40`) rather than the decoded candidate path (containing literal `@`).
   - When the physical file on disk or GPACK container is named with literal `@` and encrypted extension `.png_`, lookup failed and the interceptor substituted `TRANSPARENT_1X1_PNG`.

3. **Security Invariants Maintained:**
   - Strict hierarchical URI parsing via `java.net.URI` comparing actual parsed scheme (`https`), host (`game.local`), and port (443/-1) against trusted origin to prevent spoofing.
   - Rejection of `@` and `%40` in `rawAuthority` / `rawUserInfo`.
   - Rejection of backslashes (`\\`, `%5c`) and path traversal (`..`, `%2e%2e`, `%2f`, NUL byte) in authority and VFS logical paths.
   - Single percent-decode per request preserving literal `+` (replacing `+` with `%2B` before `URLDecoder`).
   - No double-decoding (e.g., `%2540` decodes once to `%40`).
   - Zero game-specific basename exceptions or hardcoding.
   - Source game directory remains 100% immutable.

---

## 2. Tasks Breakdown

### Task 1: URI Parsing & VFS Resource Interceptor Path Fix (Implementation & TDD)
- **Files Modified:**
  - `Docs/REFERENCES.md`: Add canonical reference S44 (RFC 3986 §3.2 & §3.3).
  - `android/shell/app/src/main/java/io/runtimepack/host/SharedNavigationPolicy.kt`:
    - Strict `java.net.URI` parsing in `isCanonicalGameUri`.
    - Confine `@`, `%40`, and credential checks to `rawAuthority` / `rawUserInfo`.
    - Allow `@` and `%40` in path/query for canonical game origin.
    - Reject backslashes and mismatch between parsed URI components and arguments.
  - `android/shell/app/src/main/java/io/runtimepack/host/VfsResourceInterceptor.kt`:
    - Robust percent-canonicalization helper (`safePercentDecode`) preserving `+`, failing on NUL, encoded slashes, backslashes, malformed sequences, and single-decoding `%2540` to `%40`.
    - Shared `resolveStream` examining both raw and decoded candidate paths across `streamProvider`, lowercase fallback, and `reader.getEntry`.
    - Consistent candidate path usage for direct lookup, `.png_` / `.ogg_` encrypted extension fallback, `.rpgmvp` / `.rpgmvo` / `.rpgmvm` fallback, and audio fallbacks.
  - `android/shell/app/src/test/java/io/runtimepack/host/WebBridgeExternalUrlTest.kt`:
    - Unit tests verifying canonical game URI acceptance for path/query with `@` and `%40`.
    - Unit tests verifying authority credential rejection (`user@game.local`, `game.local%40evil.com`, rawUrl spoofing).
  - `android/shell/app/src/test/java/io/runtimepack/host/VfsResourceInterceptorTest.kt`:
    - Unit tests for `@` and `%40` asset resolution (both direct `.png_` and fallback from `.png`).
    - Unit tests verifying byte-exact provider stream return (not transparent PNG).
    - Unit tests for percent-decoding edge cases (literal `+`, single-decode `%2540`, malformed `%`, path traversal rejection).
- **Verification:** Execute `./gradlew testDebugUnitTest` and cargo tests in WSL Ubuntu 24.04.

### Task 2: Candidate APK Build & Verification (QA & Review)
- Build Android candidate APK using `--flavor beta`.
- Verify GPACK container integrity and asset delivery.
- Verify Beta Watermark and app-specific storage invariants.
- Verify source tree immutability (pre/post-build SHA-256 comparison).
- Document deliverable artifacts in `dist_mz_beta_20261010/` without overwriting 20261009 deliverables.
