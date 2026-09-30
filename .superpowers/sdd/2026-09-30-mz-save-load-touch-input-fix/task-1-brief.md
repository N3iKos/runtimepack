# Task 1 Brief: Shim Patch — Single-Tap Load & Touch Threshold (`runtimepack_web_shims.js`)

**Plan:** `Docs/superpowers/plans/2026-09-30-mz-save-load-touch-input-fix.md` (Task 1, verbatim)

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

## Global Constraints (binding, verbatim dari plan)

- **Jangan ubah file game user** (invariant VFS/RuntimePack: input game immutable). Semua patch hanya di `runtimepack_web_shims.js` dan `WebActivity.kt`.
- **Jangan menyentuh jalur save yang sudah bekerja** (`saveToForage`/`loadFromForage`/`NativeSaveRepository.kt` bridge): save fisik sudah terbukti valid.
- Semua patch harus **idempotent** (guard `__patched`) dan **late-binding** (engine MZ classes dimuat setelah shim dieksekusi → gunakan pola polling `pollPatches` yang sudah ada).
- **Zero-dependency** JavaScript: tidak boleh menambah library eksternal.
- Semua error/log memakai prefix `[RuntimePack]`.
- Test command baku: `node android/shell/app/src/test/js/web_shims.test.js` (dijalankan dari root worktree).
- Worktree eksekusi: `.worktrees/native-mv-mz-save` (branch `feature/native-mv-mz-save`).

## PENTING: kondisi worktree

File `runtimepack_web_shims.js` dan `web_shims.test.js` SUDAH punya perubahan uncommitted dari plan sebelumnya (Save Adapter: `patchMzStorage`, `isLocalMode` virtualization, dll). Itu bukan bagian tugas Anda — JANGAN dihapus, JANGAN diubah, JANGAN di-commit. Anda hanya MENAMBAH blok baru. Saat commit, gunakan `git add` pada kedua file (perubahan lama ikut ter-commit dulu boleh — sebenarnya lebih aman: commit dulu kerjaan lama? TIDAK — lihat instruksi commit di bawah).

**Instruksi commit khusus (revisi dari plan):** Karena file yang sama memuat perubahan uncommitted plan sebelumnya yang sudah terverifikasi (APK sudah dibuild, save terbukti valid), commit Anda boleh menyertakan seluruh isi file (perubahan lama + baru) dalam SATU commit dengan pesan:
`fix(web-runtime): single-tap save load, touch threshold, and Input codeMapper fallback for Android WebView`
Ini disetujui sebagai ruling pre-flight oleh controller.

## Step 1: Write the failing test

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

## Step 2: Run test to verify it fails

Run: `node android/shell/app/src/test/js/web_shims.test.js`
Expected: FAIL — assertion `TouchInput.moveThreshold must be raised to 24` (shim belum mem-patch input engine).

## Step 3: Write minimal implementation

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

## Step 4: Run test to verify it passes

Run: `node android/shell/app/src/test/js/web_shims.test.js`
Expected: PASS semua suite lama + suite baru "Engine Input Patch tests PASSED".

## Step 5: Commit

```bash
git add android/shell/app/src/main/assets/web_shims/runtimepack_web_shims.js android/shell/app/src/test/js/web_shims.test.js
git commit -m "fix(web-runtime): single-tap save load, touch threshold, and Input codeMapper fallback for Android WebView"
```

(Catatan ruling: perubahan uncommitted plan sebelumnya pada file yang sama ikut ter-commit dalam commit ini — disetujui controller. JANGAN `git add` file lain seperti `WebBackend.kt`, `NativeSaveRepository.kt`, atau file Kotlin lainnya.)
