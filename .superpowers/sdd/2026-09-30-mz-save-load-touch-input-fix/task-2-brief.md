# Task 2 Brief: Kotlin Patch — `dispatchKeyToGame` Anti-Sanitasi Chromium (`WebActivity.kt`)

**Plan:** `Docs/superpowers/plans/2026-09-30-mz-save-load-touch-input-fix.md` (Task 2, verbatim)

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt:750-780` (method `dispatchKeyToGame`)

**Interfaces:**
- Consumes: `window.RuntimePackInput` API dari Task 1 (press/release), `Input.keyMapper` dan `Input.codeMapper` internal engine.
- Produces: `dispatchKeyToGame(jsKeyCode: Int, type: String)` yang tetap compatible dipanggil dari `dispatchKeyEvent` (line ~801) dan shortcut gamepad (line ~622-627).

## Global Constraints (binding, verbatim dari plan)

- **Jangan ubah file game user** (invariant VFS/RuntimePack: input game immutable). Semua patch hanya di `runtimepack_web_shims.js` dan `WebActivity.kt`.
- **Jangan menyentuh jalur save yang sudah bekerja** (`saveToForage`/`loadFromForage`/`NativeSaveRepository.kt` bridge): save fisik sudah terbukti valid.
- Build APK wajib `--flavor beta` (Mandatory Build Flavor Policy AGENTS.md §23) — TIDAK berlaku untuk task ini (task ini hanya patch + kompilasi).
- Semua error/log memakai prefix `[RuntimePack]`.
- Worktree eksekusi: `.worktrees/native-mv-mz-save` (branch `feature/native-mv-mz-save`).

## Catatan desain

1. `new KeyboardEvent(type, { keyCode: ... })` tetap dibuat (untuk listener pihak ketiga), TAPI properti `keyCode`/`which` di-override lewat `Object.defineProperty` sehingga nilainya tidak dibuang Chromium.
2. Tambahkan properti `code` modern (mapping `90 -> "KeyZ"`, `13 -> "Enter"`, dst.) sebagai fallback `Input.codeMapper` Task 1.
3. **Injeksi langsung** ke `window.Input._currentState` berdasarkan `Input.keyMapper[jsKeyCode]` — jalur ini yang paling andal, sama seperti pendekatan JoiPlay.
4. Hanya inject `keydown`; `keyup` me-release action.

## Kondisi worktree

Ada file untracked lain (`WebBackend.kt` termodifikasi, `NativeSaveRepository.kt`, `NativeSaveRepositoryTest.kt`, `Docs/`, `.superpowers/`) — itu kerjaan plan sebelumnya. JANGAN `git add` mereka. Commit Anda HANYA `WebActivity.kt`.

## Step 1: Tulis implementasi baru `dispatchKeyToGame`

Ganti seluruh body method `dispatchKeyToGame` (line 750-780, mulai `open fun dispatchKeyToGame(jsKeyCode: Int, type: String) {` sampai penutup methodnya sebelum `override fun dispatchKeyEvent`) dengan:

```kotlin
    open fun dispatchKeyToGame(jsKeyCode: Int, type: String) {
        runOnUiThread {
            val script = """
                (function() {
                    var codeMap = {
                        8: "Backspace", 9: "Tab", 13: "Enter", 16: "ShiftLeft", 17: "ControlLeft",
                        18: "AltLeft", 19: "Pause", 27: "Escape", 32: "Space", 33: "PageUp",
                        34: "PageDown", 35: "End", 36: "Home", 37: "ArrowLeft", 38: "ArrowUp",
                        39: "ArrowRight", 40: "ArrowDown", 45: "Insert", 46: "Delete",
                        48: "Digit0", 49: "Digit1", 50: "Digit2", 51: "Digit3", 52: "Digit4",
                        53: "Digit5", 54: "Digit6", 55: "Digit7", 56: "Digit8", 57: "Digit9",
                        65: "KeyA", 66: "KeyB", 67: "KeyC", 68: "KeyD", 69: "KeyE", 70: "KeyF",
                        71: "KeyG", 72: "KeyH", 73: "KeyI", 74: "KeyJ", 75: "KeyK", 76: "KeyL",
                        77: "KeyM", 78: "KeyN", 79: "KeyO", 80: "KeyP", 81: "KeyQ", 82: "KeyR",
                        83: "KeyS", 84: "KeyT", 85: "KeyU", 86: "KeyV", 87: "KeyW", 88: "KeyX",
                        89: "KeyY", 90: "KeyZ", 96: "Numpad0", 97: "Numpad1", 98: "Numpad2",
                        99: "Numpad3", 100: "Numpad4", 101: "Numpad5", 102: "Numpad6",
                        103: "Numpad7", 104: "Numpad8", 105: "Numpad9", 108: "NumpadEnter",
                        112: "F1", 113: "F2", 114: "F3", 115: "F4", 116: "F5", 117: "F6",
                        118: "F7", 119: "F8", 120: "F9", 121: "F10", 122: "F11", 123: "F12"
                    };
                    var evt;
                    try {
                        evt = new KeyboardEvent('$type', {
                            bubbles: true,
                            cancelable: true
                        });
                        Object.defineProperty(evt, 'keyCode', {
                            get: function() { return $jsKeyCode; },
                            configurable: true
                        });
                        Object.defineProperty(evt, 'which', {
                            get: function() { return $jsKeyCode; },
                            configurable: true
                        });
                        var evtCode = codeMap[$jsKeyCode] || "";
                        try {
                            Object.defineProperty(evt, 'code', {
                                get: function() { return evtCode; },
                                configurable: true
                            });
                        } catch (e) {}
                    } catch (e) {
                        evt = null;
                    }
                    if (evt) {
                        window.dispatchEvent(evt);
                        document.dispatchEvent(evt);
                    }

                    // Jalur utama: injeksi langsung ke state engine RPG Maker.
                    // (Chromium modern membuang keyCode dari KeyboardEventInit,
                    //  sehingga dispatch DOM saja tidak pernah sampai ke Input.keyMapper.)
                    if (window.Input) {
                        if ('$type' === 'keydown' && typeof window.Input._onKeyDown === 'function' && evt) {
                            window.Input._onKeyDown(evt);
                        } else if ('$type' === 'keyup' && typeof window.Input._onKeyUp === 'function' && evt) {
                            window.Input._onKeyUp(evt);
                        }
                    }
                    if (window.TouchInput) {
                        if ('$type' === 'keydown' && typeof window.TouchInput._onKeyDown === 'function' && evt) {
                            window.TouchInput._onKeyDown(evt);
                        } else if ('$type' === 'keyup' && typeof window.TouchInput._onKeyUp === 'function' && evt) {
                            window.TouchInput._onKeyUp(evt);
                        }
                    }

                    // Safety net: jika event tetap tidak diproses engine (keyCode hilang),
                    // set state input secara langsung via RuntimePackInput (Task 1 shim).
                    if (window.RuntimePackInput && window.RuntimePackInput.isAvailable()) {
                        var action = null;
                        if (window.Input && window.Input.keyMapper) {
                            action = window.Input.keyMapper[$jsKeyCode] || null;
                        }
                        if (!action && evt && evt.code && window.Input && window.Input.codeMapper) {
                            action = window.Input.codeMapper[evt.code] || null;
                        }
                        if (action) {
                            if ('$type' === 'keydown') {
                                window.RuntimePackInput.press(action);
                            } else {
                                window.RuntimePackInput.release(action);
                            }
                        }
                    }
                })();
            """.trimIndent()
            gameWebView?.evaluateJavascript(script, null)
        }
    }
```

## Step 2: Verifikasi kompilasi Kotlin

Run (dari `android/shell`, Windows):
```bash
gradlew.bat :app:compileDebugKotlin --console=plain
```
Expected: `BUILD SUCCESSFUL` tanpa error kompilasi.

Jika `gradlew.bat` tidak ada di `android/shell`, cari lokasi gradle wrapper (mis. root repo) dan jalankan dari sana; jika tetap tidak ketemu, laporkan NEEDS_CONTEXT dengan path yang sudah dicoba.

## Step 3: Jalankan ulang seluruh test suite JS

Run (dari root worktree):
```bash
node android/shell/app/src/test/js/web_shims.test.js
node android/shell/app/src/test/js/vpad_bridge.test.js
node android/shell/app/src/test/js/cheat_menu.test.js
node android/shell/app/src/test/js/cheat_primitives.test.js
node android/shell/app/src/test/js/diagnostic_suite.test.js
```
Expected: semua exit 0, tanpa assertion failure.

## Step 4: Commit

```bash
git add android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt
git commit -m "fix(android-host): inject keyCode via defineProperty and direct Input state to survive Chromium event sanitization"
```

(JANGAN `git add` file lain — `WebBackend.kt`, `NativeSaveRepository.kt`, dll adalah kerjaan plan sebelumnya yang tidak boleh ikut commit ini.)
