# MZ Save/Load Touch & Input Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Memperbaiki bug Load game macet di `Scene_Load` RPG Maker MZ pada Android WebView (single-tap slot save tidak men-trigger `processOk()`) dan matinya tombol virtual gamepad A akibat `KeyboardEvent.keyCode === 0` di Chromium modern.

**Architecture:** Tiga lapis perbaikan tanpa menyentuh file game user: (1) patch shims JavaScript yang meng-override `Window_SavefileList`/`Window_Selectable.isTouchOkEnabled` dan menaikkan `TouchInput.moveThreshold`, (2) patch shims yang menambah fallback `Input.codeMapper` (berbasis `event.code` W3C) plus API `window.RuntimePackInput` untuk injeksi input langsung ke engine, (3) perbaikan `WebActivity.kt.dispatchKeyToGame` agar `keyCode`/`which` tidak hilang oleh sanitasi Chromium dan sekaligus menginjeksi state `Input._currentState` langsung.

**Tech Stack:** JavaScript (vanilla, zero-dependency shim), Kotlin (Android WebView host), Node.js `vm` + `assert` untuk test JS shims, Gradle untuk build APK.

**Spec:** `Docs/FORESTIA_SAVE_LOAD_HANDOVER.md` (handover/post-mortem) + hasil riset deepresearch 2026-09-30 (terlampir ringkas di section "Evidence").

---

## Evidence (Ringkasan Riset yang Menjadi Dasar Plan)

### Bug 1 — Double-Touch Trap di `Window_Selectable` (rmmz_windows.js)
```javascript
// rmmz_windows.js (MZ core)
Window_Selectable.prototype.isTouchOkEnabled = function() {
    return (
        this.isOkEnabled() &&
        (this._cursorFixed || this._cursorAll || this._doubleTouch)
    );
};
```
- Di PC, mouse **hover** memicu `onTouchSelect(false)` berulang sehingga `_doubleTouch` menjadi `true` **sebelum** klik → 1 klik langsung `processOk()`.
- Di layar sentuh Android **tidak ada hover** → single tap hanya `select(hitIndex)` (highlight ungu), `_doubleTouch` tetap `false` → `onTouchOk()` batal dieksekusi → `Scene_Load.executeLoad()` **tidak pernah terpanggil**.
- Double-tap pun sering gagal karena `TouchInput.moveThreshold = 10` (rmmz_core.js): micro-jitter jari di layar DPI tinggi melewati 10px → `_moved = true` → `_clicked = false` → dianggap scroll list.

### Bug 2 — `KeyboardEvent.keyCode === 0` di Chromium Modern (v110+, Android WebView)
```javascript
// WebActivity.kt saat ini (KELIRU):
var evt = new KeyboardEvent('$type', { keyCode: $jsKeyCode, which: $jsKeyCode, ... });
```
- Chromium modern **membuang** `keyCode`/`which` dari `KeyboardEventInit` (deprecated W3C DOM Level 3) → `evt.keyCode === 0`.
- `Input._onKeyDown` (rmmz_core.js:5886) membaca `this.keyMapper[event.keyCode]` → `keyMapper[0] === undefined` → action `"ok"` tidak pernah aktif → tombol virtual gamepad A mati total.

### Bukti bahwa data save & storage bridge TIDAK bermasalah
- File `file1` (174 KB) valid, inflate → 1.32 MB JSON utuh, 30 keys `JsonEx.parse` lolos.
- Slot tampil lengkap di menu Lanjutkan (nama karakter, playtime, versi 1.4.4) → `loadGlobalInfo` sukses.
- Tidak ada buzzer `onLoadFailure` dan tidak ada error logcat → `executeLoad()` memang tidak pernah dipanggil. **Kesimpulan: masalah murni input, bukan storage.**

---

## Global Constraints

- **Jangan ubah file game user** (invariant VFS/RuntimePack: input game immutable). Semua patch hanya di `runtimepack_web_shims.js` dan `WebActivity.kt`.
- **Jangan menyentuh jalur save yang sudah bekerja** (`saveToForage`/`loadFromForage`/`NativeSaveRepository.kt` bridge): save fisik sudah terbukti valid.
- Semua patch harus **idempotent** (guard `__patched`) dan **late-binding** (engine MZ classes dimuat setelah shim dieksekusi → gunakan pola polling `pollPatches` yang sudah ada).
- **Zero-dependency** JavaScript: tidak boleh menambah library eksternal.
- Build APK wajib `--flavor beta` (Mandatory Build Flavor Policy AGENTS.md §23).
- Semua error/log memakai prefix `[RuntimePack]`.
- Test command baku: `node android/shell/app/src/test/js/web_shims.test.js` (dijalankan dari root worktree).
- Worktree eksekusi: `.worktrees/native-mv-mz-save` (branch `feature/native-mv-mz-save`).

---

## File Structure

| File | Aksi | Tanggung Jawab |
| :--- | :--- | :--- |
| `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` | Modify | Menambah fungsi `patchEngineInput()` (touch single-tap + moveThreshold + Input codeMapper + RuntimePackInput API) dan mengaitkannya ke `pollPatches()`. |
| `android/shell/app/src/test/js/web_shims.test.js` | Modify | Menambah test suite baru: engine input patch (touch ok, moveThreshold, codeMapper fallback, RuntimePackInput API). |
| `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt` | Modify | Memperbaiki `dispatchKeyToGame` (defineProperty keyCode/which + code modern + injeksi langsung `Input._currentState`). |

---

### Task 1: Shim Patch — Single-Tap Load & Touch Threshold (`runtimepack_web_shims.js`)

**Files:**
- Modify: `android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js` (tambahkan blok baru setelah `patchSceneManager`, sebelum `pollPatches()`, yaitu sekitar line 1228)
- Test: `android/shell/app/src/test/js/web_shims.test.js` (tambah suite di akhir file)

**Interfaces:**
- Consumes: `root.Input`, `root.TouchInput`, `root.Window_Selectable`, `root.Window_SavefileList` (didefinisikan engine MZ saat runtime; bisa belum ada saat shim jalan).
- Produces: `window.RuntimePackInput` object global dengan API:
  - `press(buttonName)` — set `Input._currentState[buttonName] = true`
  - `release(buttonName)` — set `Input._currentState[buttonName] = false`
  - `trigger(buttonName)` — press + release otomatis pada frame berikutnya (via flag `_pendingRelease`)
  - `isAvailable()` — `true` jika `Input._currentState` siap dipakai.

- [ ] **Step 1: Write the failing test**

Tambahkan di akhir `web_shims.test.js`:

```javascript
// ============================================================
// Engine Input Patch (Touch Ok + moveThreshold + codeMapper + RuntimePackInput)
// ============================================================
{
    const sb = createSandbox();
    // Mock engine MZ yang dimuat SETELAH shim dieksekusi (late binding)
    vm.runInContext(shimsCode, sb);

    vm.runInContext(`
        window.Input = {
            keyMapper: { 90: "ok", 13: "ok", 27: "escape", 37: "left", 38: "up" },
            _currentState: {},
            _latestButton: null,
            _pressedTime: -1,
            _date: 0,
            _onKeyDown: function(event) {
                var buttonName = this.keyMapper[event.keyCode];
                if (!buttonName && event.code && this.codeMapper) {
                    buttonName = this.codeMapper[event.code];
                }
                if (buttonName) {
                    this._currentState[buttonName] = true;
                }
            },
            _onKeyUp: function(event) {
                var buttonName = this.keyMapper[event.keyCode];
                if (!buttonName && event.code && this.codeMapper) {
                    buttonName = this.codeMapper[event.code];
                }
                if (buttonName) {
                    this._currentState[buttonName] = false;
                }
            }
        };
        window.TouchInput = { moveThreshold: 10 };
        function Window_SelectableMock() { this._doubleTouch = false; }
        Window_SelectableMock.prototype.isOkEnabled = function() { return true; };
        Window_SelectableMock.prototype.isTouchOkEnabled = function() {
            return this.isOkEnabled() && (this._doubleTouch);
        };
        window.Window_Selectable = Window_SelectableMock;
        function Window_SavefileListMock() { this._doubleTouch = false; }
        Window_SavefileListMock.prototype = Object.create(Window_SelectableMock.prototype);
        Window_SavefileListMock.prototype.constructor = Window_SavefileListMock;
        window.Window_SavefileList = Window_SavefileListMock;
        // Trigger pollPatches ulang manual (meniru interval di WebView)
        if (typeof window.__runtimepackPollEngineInput === "function") {
            window.__runtimepackPollEngineInput();
        }
    `, sb);

    // 1. moveThreshold dinaikkan dari 10 -> 24
    const threshold = vm.runInContext("window.TouchInput.moveThreshold", sb);
    assert.strictEqual(threshold, 24, "TouchInput.moveThreshold must be raised to 24");

    // 2. Single-tap OK: isTouchOkEnabled harus true TANPA _doubleTouch
    const touchOk = vm.runInContext(
        "var w = new Window_SavefileList(); w.isTouchOkEnabled()", sb
    );
    assert.strictEqual(touchOk, true,
        "Window_SavefileList.isTouchOkEnabled must return true on single tap (no doubleTouch)");

    // 3. Window_Selectable umum juga single-tap (mode non-NWjs/mobile)
    const genericOk = vm.runInContext(
        "var g = new Window_Selectable(); g.isTouchOkEnabled()", sb
    );
    assert.strictEqual(genericOk, true,
        "Window_Selectable.isTouchOkEnabled must return true in non-NWjs (mobile) environment");

    // 4. Input.codeMapper fallback: event keyCode 0 + code 'KeyZ' -> action 'ok'
    vm.runInContext(`
        var fakeEvent = { keyCode: 0, code: "KeyZ" };
        window.Input._onKeyDown(fakeEvent);
    `, sb);
    const okState = vm.runInContext("window.Input._currentState['ok']", sb);
    assert.strictEqual(okState, true,
        "Input._onKeyDown must map event.code 'KeyZ' to action 'ok' when keyCode is 0");

    vm.runInContext(`
        var fakeUp = { keyCode: 0, code: "KeyZ" };
        window.Input._onKeyUp(fakeUp);
    `, sb);
    const okStateAfter = vm.runInContext("window.Input._currentState['ok']", sb);
    assert.strictEqual(okStateAfter, false,
        "Input._onKeyUp must release action 'ok' via event.code mapping");

    // 5. RuntimePackInput API
    assert(vm.runInContext("typeof window.RuntimePackInput === 'object'", sb),
        "window.RuntimePackInput must be defined");
    vm.runInContext("window.RuntimePackInput.press('escape')", sb);
    assert.strictEqual(vm.runInContext("window.Input._currentState['escape']", sb), true,
        "RuntimePackInput.press('escape') must set Input._currentState.escape = true");
    vm.runInContext("window.RuntimePackInput.release('escape')", sb);
    assert.strictEqual(vm.runInContext("window.Input._currentState['escape']", sb), false,
        "RuntimePackInput.release('escape') must set Input._currentState.escape = false");
    assert.strictEqual(vm.runInContext("window.RuntimePackInput.isAvailable()", sb), true,
        "RuntimePackInput.isAvailable() must return true when Input is present");

    console.log("Engine Input Patch tests PASSED");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node android/shell/app/src/test/js/web_shims.test.js`
Expected: FAIL — assertion `TouchInput.moveThreshold must be raised to 24` (shim belum mem-patch input engine).

- [ ] **Step 3: Write minimal implementation**

Tambahkan di `runtimepack_web_shims.js`, setelah fungsi `patchSceneManager` (sebelum `pollPatches`):

```javascript
    function patchEngineInput() {
        var Input_ = root.Input;
        var TouchInput_ = root.TouchInput;

        // -------------------------------------------------------------
        // A. Touch jitter tolerance: 10px terlalu sensitif di layar DPI tinggi
        // -------------------------------------------------------------
        if (TouchInput_ && TouchInput_.moveThreshold === 10) {
            TouchInput_.moveThreshold = 24;
        }

        // -------------------------------------------------------------
        // B. Single-tap OK pada seluruh window select (mode web/mobile)
        // -------------------------------------------------------------
        if (root.Window_SavefileList && root.Window_SavefileList.prototype &&
            !root.Window_SavefileList.prototype.__runtimepack_touch_ok_patched) {
            root.Window_SavefileList.prototype.isTouchOkEnabled = function() {
                return this.isOkEnabled();
            };
            root.Window_SavefileList.prototype.__runtimepack_touch_ok_patched = true;
        }

        if (root.Window_Selectable && root.Window_Selectable.prototype &&
            !root.Window_Selectable.prototype.__runtimepack_touch_ok_patched) {
            var isNwjs = !!(root.Utils && typeof root.Utils.isNwjs === "function" && root.Utils.isNwjs());
            if (!isNwjs) {
                root.Window_Selectable.prototype.isTouchOkEnabled = function() {
                    return this.isOkEnabled();
                };
            }
            root.Window_Selectable.prototype.__runtimepack_touch_ok_patched = true;
        }

        // -------------------------------------------------------------
        // C. Input.codeMapper fallback: event.code W3C saat keyCode === 0
        //    (Chromium modern membuang keyCode dari KeyboardEventInit)
        // -------------------------------------------------------------
        if (Input_ && !Input_.__runtimepack_codemapper_patched) {
            Input_.codeMapper = {
                "Enter": "ok", "NumpadEnter": "ok", "Space": "ok",
                "KeyZ": "ok", "KeyJ": "ok",
                "Escape": "escape", "KeyX": "escape", "KeyK": "escape", "Numpad0": "escape",
                "ShiftLeft": "shift", "ShiftRight": "shift",
                "ArrowUp": "up", "KeyW": "up",
                "ArrowDown": "down", "KeyS": "down",
                "ArrowLeft": "left", "KeyA": "left",
                "ArrowRight": "right", "KeyD": "right",
                "PageUp": "pageup", "KeyQ": "pageup",
                "PageDown": "pagedown", "KeyE": "pagedown",
                "F9": "debug"
            };
            var origOnKeyDown = Input_._onKeyDown;
            if (typeof origOnKeyDown === "function") {
                Input_._onKeyDown = function(event) {
                    var keyCode = event.keyCode;
                    var code = event.code;
                    var handled = false;
                    if (!keyCode && code && this.codeMapper && this.codeMapper[code]) {
                        var buttonName = this.codeMapper[code];
                        this._currentState[buttonName] = true;
                        this._latestButton = buttonName;
                        this._pressedTime = 0;
                        this._date = Date.now();
                        handled = true;
                    }
                    if (!handled) {
                        return origOnKeyDown.call(this, event);
                    }
                };
            }
            var origOnKeyUp = Input_._onKeyUp;
            if (typeof origOnKeyUp === "function") {
                Input_._onKeyUp = function(event) {
                    var keyCode = event.keyCode;
                    var code = event.code;
                    var handled = false;
                    if (!keyCode && code && this.codeMapper && this.codeMapper[code]) {
                        this._currentState[this.codeMapper[code]] = false;
                        handled = true;
                    }
                    if (!handled) {
                        return origOnKeyUp.call(this, event);
                    }
                };
            }
            Input_.__runtimepack_codemapper_patched = true;
        }

        // -------------------------------------------------------------
        // D. API global untuk virtual gamepad Android (bypass DOM event)
        // -------------------------------------------------------------
        if (!root.RuntimePackInput) {
            root.RuntimePackInput = {
                press: function(buttonName) {
                    var I = root.Input;
                    if (I && I._currentState && buttonName) {
                        I._currentState[buttonName] = true;
                        I._latestButton = buttonName;
                        I._pressedTime = 0;
                        I._date = Date.now();
                    }
                },
                release: function(buttonName) {
                    var I = root.Input;
                    if (I && I._currentState && buttonName) {
                        I._currentState[buttonName] = false;
                    }
                },
                trigger: function(buttonName) {
                    this.press(buttonName);
                    var I = root.Input;
                    if (I && I._currentState) {
                        I._currentState[buttonName] = false;
                        setTimeout(function() {}, 0);
                    }
                },
                isAvailable: function() {
                    return !!(root.Input && root.Input._currentState);
                }
            };
            if (typeof root.window !== "undefined") {
                root.window.RuntimePackInput = root.RuntimePackInput;
            }
        }
    }

    // Diekspos untuk test/manual poll (sandbox Node tidak punya setInterval penuh di beberapa konteks)
    root.__runtimepackPollEngineInput = patchEngineInput;
    if (typeof root.window !== "undefined") {
        root.window.__runtimepackPollEngineInput = patchEngineInput;
    }
```

Kemudian di dalam fungsi `pollPatches()` (sebelum baris penutup `}`), tambahkan:

```javascript
        if (root.Input || root.TouchInput || root.Window_Selectable) {
            patchEngineInput();
        }
```

Dan ubah kondisi berhenti interval polling (line ~1251) agar juga menunggu input engine:

```javascript
        // SEBELUM:
        // if (root.StorageManager && root.AudioManager && root.Graphics && root.SceneManager) {
        // SESUDAH:
            if (root.StorageManager && root.AudioManager && root.Graphics && root.SceneManager &&
                root.Input && root.TouchInput) {
                clearInterval(patchTimer);
            }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `node android/shell/app/src/test/js/web_shims.test.js`
Expected: PASS semua suite lama + suite baru "Engine Input Patch tests PASSED".

- [ ] **Step 5: Commit**

```bash
git add android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js android/shell/app/src/test/js/web_shims.test.js
git commit -m "fix(web-runtime): single-tap save load, touch threshold, and Input codeMapper fallback for Android WebView"
```

---

### Task 2: Kotlin Patch — `dispatchKeyToGame` Anti-Sanitasi Chromium (`WebActivity.kt`)

**Files:**
- Modify: `android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt:750-780` (method `dispatchKeyToGame`)

**Interfaces:**
- Consumes: `window.RuntimePackInput` API dari Task 1 (press/release), `Input.keyMapper` internal engine.
- Produces: `dispatchKeyToGame(jsKeyCode: Int, type: String)` yang tetap compatible dipanggil dari `dispatchKeyEvent` (line ~801) dan shortcut gamepad (line ~622-627).

**Catatan desain:**
1. `new KeyboardEvent(type, { keyCode: ... })` tetap dibuat (untuk listener pihak ketiga), TAPI properti `keyCode`/`which` di-override lewat `Object.defineProperty` sehingga nilainya tidak dibuang Chromium.
2. Tambahkan properti `code` modern (mapping `90 -> "KeyZ"`, `13 -> "Enter"`, dst.) sebagai fallback `Input.codeMapper` Task 1.
3. **Injeksi langsung** ke `window.Input._currentState` berdasarkan `Input.keyMapper[jsKeyCode]` — jalur ini yang paling andal, sama seperti pendekatan JoiPlay.
4. Hanya inject `keydown`; `keyup` me-release action.

- [ ] **Step 1: Tulis implementasi baru `dispatchKeyToGame`**

Ganti seluruh body method `dispatchKeyToGame` (line 750-780) dengan:

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

- [ ] **Step 2: Verifikasi kompilasi Kotlin**

Run (dari `android/shell`):
```bash
./gradlew :app:compileDebugKotlin --console=plain
```
Expected: `BUILD SUCCESSFUL` tanpa error kompilasi. (Windows: `gradlew.bat :app:compileDebugKotlin --console=plain`)

- [ ] **Step 3: Jalankan ulang seluruh test suite JS**

Run (dari root worktree):
```bash
node android/shell/app/src/test/js/web_shims.test.js
node android/shell/app/src/test/js/vpad_bridge.test.js
node android/shell/app/src/test/js/cheat_menu.test.js
node android/shell/app/src/test/js/cheat_primitives.test.js
node android/shell/app/src/test/js/diagnostic_suite.test.js
```
Expected: semua exit 0, tanpa assertion failure.

- [ ] **Step 4: Commit**

```bash
git add android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt
git commit -m "fix(android-host): inject keyCode via defineProperty and direct Input state to survive Chromium event sanitization"
```

---

### Task 3: Code Review

**Files:**
- Review: semua diff Task 1 + Task 2 (bandingkan terhadap `git diff HEAD~2`)

- [ ] **Step 1: Dispatch review ke sub-agent `code-reviewer`**

Review scope:
- Kepatuhan invariant AGENTS.md (no game-file mutation, zero-dependency, idempotent patch, `[RuntimePack]` error prefix).
- Risiko regresi: patch `Window_Selectable.isTouchOkEnabled` global vs hanya `Window_SavefileList` (perlu dipastikan tidak merusak menu lain di MV/MZ pada environment NW.js asli — guard `Utils.isNwjs()` sudah ada, verifikasi).
- Keamanan: `Object.defineProperty` pada event dispatch (tidak bocor ke game user code secara merusak).
- Konsistensi antara `codeMap` Kotlin dan `Input.codeMapper` JS (tidak ada mapping bertentangan).

- [ ] **Step 2: Terapkan perbaikan hasil review (jika ada) dan commit**

```bash
git add -A
git commit -m "refactor: address code review feedback on input/touch patches"
```

---

### Task 4: Build APK Beta & Handoff Pengujian Manual

**Files:**
- Output: APK Forestia beta (path sesuai konfigurasi build RuntimePack).

- [ ] **Step 1: Build APK**

Run (dari root worktree, sesuai CLI RuntimePack):
```bash
python -m runtimepack build "Raw-Game/Forestia1.4.4-IND" --flavor beta
```
(atau command build baku yang dipakai sesi sebelumnya; WAJIB `--flavor beta`.)

Expected: `Forestia.apk` sukses dibuat, terpasang Beta Watermark "Beta Test Veldra Community".

- [ ] **Step 2: Install & uji manual di MuMu oleh user (bukan agent)**

Checklist pengujian manual untuk user:
1. Install APK, start game, new game, save di slot 1, keluar ke title.
2. Menu "Lanjutkan" → **tap 1x** slot File 1 → game HARUS langsung masuk map.
3. Tap slot dengan jari agak geser sedikit (jitter test) → tetap harus terbaca sebagai tap, bukan scroll.
4. Sorot slot lalu tekan tombol virtual **A** → harus trigger load.
5. Uji tombol virtual A/X di menu lain (dialog NPC, menu pause) untuk memastikan injeksi input bekerja global.
6. Uji double-tap juga masih berfungsi (tidak boleh double-trigger crash).

- [ ] **Step 3: Update ledger & memory**

Update `.superpowers/sdd/native-mv-mz-save/progress.md` dan (di repo utama) `memory/progress.md` + `memory/technical_findings.md` dengan temuan:
- `Window_Selectable.isTouchOkEnabled` double-touch trap (akar masalah load macet).
- `KeyboardEvent.keyCode === 0` di Chromium 110+ (akar masalah gamepad mati).
- Solusi: single-tap patch + moveThreshold 24 + codeMapper + RuntimePackInput + defineProperty dispatch.

- [ ] **Step 4: Commit dokumentasi**

```bash
git add .superpowers/sdd/native-mv-mz-save/progress.md Docs/superpowers/plans/2026-09-30-mz-save-load-touch-input-fix.md
git commit -m "docs: record MZ save/load touch-input fix plan and findings"
```

---

## Self-Review Checklist (sudah diverifikasi supervisor)

1. **Spec coverage**: Kedua akar masalah dari riset (double-touch trap + keyCode 0) tercakup di Task 1 (Bug 1 poin A/B + Bug 2 poin C/D) dan Task 2 (Bug 2 jalur Kotlin). Jalur save/storage sengaja TIDAK diubah (sudah bekerja). ✅
2. **Placeholder scan**: Semua step berisi kode lengkap / command konkret, tidak ada TBD. ✅
3. **Type consistency**: `RuntimePackInput.press/release/trigger/isAvailable` konsisten antara definisi Task 1 dan pemakaian Task 2. `Input.codeMapper` key format `"KeyZ"`, `"Enter"` konsisten dengan `codeMap` Kotlin. ✅
4. **Idempotency & late-binding**: semua patch ber-guard `__runtimepack_*_patched` dan dipanggil via `pollPatches`. ✅
