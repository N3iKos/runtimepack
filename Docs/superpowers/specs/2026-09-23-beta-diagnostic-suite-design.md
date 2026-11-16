# Specification: Beta Diagnostic Suite, Build Flavors & Auto-Recovery Engine

- **Date:** 2026-09-23
- **Status:** Approved / In Design
- **Target Milestones:** Milestone 10 (Developer Experience & Stability Hardening)
- **First Benchmark Target:** *BLACK SOULS* (RPG Maker VX Ace / RGSS3)

---

## 1. Overview & Objective

Manual end-to-end playtesting of extensive RPG Maker games (which often span 30–80+ hours) is inefficient for detecting subtle missing assets, script typos, or map transfer crashes.

This specification introduces:
1. **Universal Auto-Recovery Engine:** Active in both *Beta* and *Release* builds, providing transparent dummy fallbacks for missing assets to prevent hard freezes.
2. **Build Flavor Pipeline (`--flavor beta` vs `--flavor release`):**
   - `--flavor beta` (**Mandatory default for autonomous agents**): Permanent top-right watermark, floating diagnostic panel, live error capture, automated 1-click map/asset stress test, map warper, and machine-readable `audit_report.json` / `bug_report.json`.
   - `--flavor release`: Clean production build without watermarks or debug panels, keeping the auto-recovery failsafes intact in the background.
3. **Agentic Diagnostics Proof of Value:** Using the suite to autonomously detect, localize, and report the existing bug in *BLACK SOULS* without requiring dozens of hours of manual play.

---

## 2. Core Architecture & Components

```text
Build Command: runtimepack build <game> [--flavor beta|release]
                           │
             ┌─────────────┴─────────────┐
             ▼                           ▼
     [--flavor beta]             [--flavor release]
  - Permanent Watermark       - Clean UI (No WM)
  - Floating Diagnostic HUD   - No Debug Panel
  - 1-Click Stress Test       - Auto-Recovery ACTIVE
  - Map Warper                - Fast Execution
  - Live Error Interceptor
  - Auto-Recovery ACTIVE
```

---

## 3. Detailed Specifications

### 3.1 Build Flavor System & CLI Integration
- **CLI Options in `cli/src/main.rs`:**
  ```rust
  #[arg(long, default_value = "beta")]
  flavor: String, // "beta" or "release"
  ```
- **Manifest Representation:**
  Recorded in `BuildManifest`:
  ```json
  "build": {
    "flavor": "beta"
  }
  ```
- **Agent Mandatory Policy:**
  Autonomous agents MUST always compile using `--flavor beta` unless explicitly instructed otherwise by the human partner.

### 3.2 Permanent Beta Watermark
- **Location:** Top-right corner of the viewport.
- **Styling:** CSS `#fnh-watermark` with `position: fixed`, `pointer-events: none`, `z-index: 99999`.
- **Text Format:**
  ```text
  Beta Test
  Veldra Community
  ```
- **Behavior:** Never hidden during gameplay, map changes, or when the virtual gamepad is toggled off. Disabled completely in `--flavor release`.

### 3.3 Universal Auto-Recovery Engine (Active in Beta & Release)
Prevents unhandled crashes caused by missing sound, image, or font files:
- **RGSS (Ruby / mkxp-z):**
  - `Bitmap.new(filename)`: If the file is missing or corrupt, catches `Errno::ENOENT` / `RGSSError` and supplies a 1x1 transparent dummy bitmap or placeholder (`Graphics/System/SaveNotFound.png`).
  - `Audio.bgm_play`, `bgs_play`, `se_play`, `me_play`: If the audio file is missing, logs a warning and fails silently without terminating the game thread.
  - `Cache.*`: Returns dummy bitmaps instead of raising unhandled exceptions.
- **Web (MV / MZ / Chromium):**
  - `VfsResourceInterceptor.kt` & `runtimepack_web_shims.js`: When a resource request encounters HTTP 404 (missing file), returns an in-memory 1x1 transparent PNG or empty audio buffer, preventing `Image.onload` / `WebAudio` hangs.

### 3.4 Floating Diagnostic & Stress-Test Suite (Beta Flavor Only)

#### A. Floating Debug Launcher Button
- Semi-transparent bug icon `[ 🐞 ]` in the top toolbar or edge of the screen.
- Tapping opens the **Diagnostic Suite Modal**.

#### B. Live Error Interceptor (Transparent HUD)
- Catches Ruby exceptions, JavaScript `window.onerror`, and console warnings.
- Formats error entries:
  - Error class / message
  - Script name / file path
  - Line number & stack trace
- Displays unobtrusively on-screen and persists in memory.

#### C. Automated 1-Click Stress Test (Agent & Tester Tool)
- **Execution:** Clicking `[ Run Full Stress Test ]` or invoking via IPC.
- **Audit Steps (10–30 Seconds):**
  1. **Map Traversal:** Iterates through all map data (`Data/Map*.rvdata2` or `data/Map*.json`).
  2. **Event & Asset Graph Scanning:** Inspects all event commands for graphics (character sheets, pictures, animations, tilesets) and audio (BGM, BGS, SE).
  3. **Case-Sensitivity & Existence Check:** Verifies that every referenced file exists on disk (case-insensitively and exact).
  4. **Database Audit:** Checks Items, Weapons, Armors, Enemies, Skills, and States.
- **Machine-Readable Output:** Writes results to `audit_report.json` in `files/` (readily accessible via `adb` or export button):
  ```json
  {
    "total_assets_checked": 5210,
    "missing_assets": [
      { "type": "audio", "path": "Audio/BGM/lost_track.ogg", "referenced_in": "Map042" }
    ],
    "syntax_issues": [],
    "status": "COMPLETED"
  }
  ```

#### D. Map Warper / Teleport
- Input field: `Map ID: [  ]`, `X: [  ]`, `Y: [  ]`.
- Button: `[ Teleport ]`.
- Executes `$game_player.reserve_transfer(map_id, x, y)` (RGSS) or `$gamePlayer.reserveTransfer(map_id, x, y)` (Web MV/MZ).
- Allows instant traversal to late-game areas to reproduce bugs in seconds.

#### E. Machine-Readable Bug Report Export
- Button: `[ Export Bug Report ]`.
- Serializes active Map ID, player coordinates, current scene name, and the last 50 log lines into `bug_report.json` and takes a screenshot.

#### F. Lightweight Performance Meter
- Real-time indicator displaying FPS and memory usage (e.g. `60 FPS | 148 MB`).

### 3.5 Native Touchscreen & TouchInput Pass-Through (Web MV / MZ)
Solves the issue where games like *Forestia* (RPG Maker MZ) cannot be tapped or clicked directly on-screen:
- **Root Cause:** In `WebActivity.kt`, `overlayWebView` sits on top of `gameWebView` at `MATCH_PARENT x MATCH_PARENT`. Android View hierarchy intercepts all `MotionEvent` touches on the top view regardless of CSS pointer-events.
- **Pass-Through Architecture in `WebActivity.kt`:**
  - Override `dispatchTouchEvent(event)` in `WebActivity`.
  - When touches occur outside active virtual button boundaries or when the virtual gamepad is toggled hidden:
    - Forward `MotionEvent` directly to `gameWebView.dispatchTouchEvent(event)`.
    - Allows native RPG Maker MV/MZ `TouchInput` (`_onTouchStart`, `_onTouchMove`, `_onTouchEnd`) to process direct screen taps, map movement, and cursor clicks cleanly.
- **Cursor Emulation:** Provides optional visual cursor feedback on touch down for games utilizing custom mouse cursors.

---

## 4. Benchmark Validation: BLACK SOULS Bug Detection

To prove the diagnostic suite's efficacy:
1. Compile *BLACK SOULS* with `--flavor beta`.
2. Launch on MuMu emulator (`127.0.0.1:16384`).
3. Run the automated stress test via subagent.
4. Extract `audit_report.json` or inspect the live error log to pinpoint the exact bug without manual playthrough.
5. Apply the targeted fix, verify resolution, and update `Docs/test_ports/rpgvxa_black_souls/report.md`.

---

## 5. Specification Review Gate

1. **Placeholders:** None (exact mechanics, files, and outputs specified).
2. **Consistency:** Conforms to `AGENTS.md` and dual-layer architecture.
3. **Scope:** Focused on developer/agent test tooling and asset auto-recovery.
