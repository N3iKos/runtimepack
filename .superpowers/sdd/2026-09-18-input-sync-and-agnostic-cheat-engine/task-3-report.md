# Task 3 Report: Win32API Hook & Native Input Synchronization in `runtimepack_bootstrap.rb`

## Summary
Implemented the Win32API input state synchronization hooks (`GetKeyboardState`, `GetAsyncKeyState`, `GetKeyState`), enhanced `DL::CPtr` with instance registry and auto-expansion, and added native RGSS `Input` fallback aliasing in `runtimepack_bootstrap.rb`. Added a complete unit test suite in `input_hook_test.rb` and an integration test in Rust.

## Scope of Changes
1. **`android/shell/app/src/main/assets/runtimepack_bootstrap.rb`**:
   - Enhanced `DL::CPtr` with `@instances = {}` class registry, registering every instance by its `object_id` (`to_i`).
   - Added auto-expansion in `DL::CPtr#[]=` to avoid index out of bounds.
   - Enhanced `Win32API`:
     - Added class-level `Win32API.call(func, *args)` proxy.
     - Implemented `GetKeyboardState` handler that reads from `rp_input_state.bin` into either `DL::CPtr` instances (looked up by numeric ID) or `String` buffers.
     - Implemented `GetAsyncKeyState` and `GetKeyState` handlers returning `-32767` when the requested virtual key is active (MSB `0x80` set) in `rp_input_state.bin`, and `0` otherwise.
   - Implemented Native Input Fallback:
     - Aliased native `Input` methods (`rp_native_press?`, `rp_native_trigger?`, `rp_native_dir4`, `rp_native_dir8`) prior to loading `Data/Scripts.rvdata2`.
     - After all scripts are loaded, aliased Hime AllKey methods (`rp_hime_*`) and wrapped `press?`, `trigger?`, `dir4`, `dir8` to query Hime first, falling back to native mkxp-z RGSS inputs if Hime returns false / 0.
   - Replaced unquoted global constants `[Sprite, Viewport, Plane, Window]` with safe symbol reflection `[:Sprite, :Viewport, :Plane, :Window]` to support standalone Ruby VM execution outside mkxp-z.
   - Guarded `load_data` script loop to only execute if `load_data` is defined and `Data/Scripts.rvdata2` exists.

2. **`android/shell/app/src/test/ruby/input_hook_test.rb`**:
   - Created standalone Ruby test suite covering:
     - `DL::CPtr` instance registry, getter/setter, auto-expansion.
     - `Win32API.call('GetKeyboardState', cptr.to_i)` with binary state file.
     - `Win32API.call('GetKeyboardState', str_buffer)` with string buffer.
     - `Win32API.call('GetAsyncKeyState', 0x26)` with active key returning `-32767`, inactive key returning `0`.
     - `Win32API.call('GetKeyState', 0x26)` behavior.
     - Native `Input` fallback mechanics (Hime priority, native fallback, direction propagation).

3. **`tests/integration/build_test.rs`**:
   - Added `test_rgss_bootstrap_hooks_win32_input_and_native_fallback` integration test in Rust.

## Verification & Test Results
- **Ruby Unit Tests**:
  - Command: `ruby android/shell/app/src/test/ruby/input_hook_test.rb`
  - Result: `Tests run: 34, Passed: 34, Failed: 0` (100% pass)
- **Rust Integration & Workspace Tests**:
  - Command: `cargo test`
  - Result: All unit and integration test suites passed (17 integration build tests, 6 compatibility tests, 9 detection tests, all crate tests).

## Git Commits
- `e09b692`: `feat(rgss): implement Win32API keyboard hook and native Input fallback in bootstrap`
