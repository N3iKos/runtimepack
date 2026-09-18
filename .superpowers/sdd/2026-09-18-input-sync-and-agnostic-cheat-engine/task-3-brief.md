# Task 3 Brief: Win32API Hook & Native Input Synchronization in `runtimepack_bootstrap.rb`

## Context
Custom input drivers (such as Hime AllKey in Lona RPG) bypass native RGSS `Input` and poll Windows virtual key buffers directly via Win32API `GetKeyboardState` and `GetAsyncKeyState`. `RgssActivity.kt` writes active key states to `rp_input_state.bin` in the game's working directory.

## Requirements & Scope
- Files to modify:
  - `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
- Files to create:
  - `android/shell/app/src/test/ruby/input_hook_test.rb`

### Detailed Changes in `runtimepack_bootstrap.rb`:
1. In `module DL`:
   Enhance `class CPtr`:
   - Keep a static dictionary of instances:
     ```ruby
     module DL
       class CPtr
         @instances = {}
         class << self
           attr_accessor :instances
         end
         def initialize(pointer, size = nil)
           @data = pointer.is_a?(String) ? pointer : ("\0" * (size || 0))
           @data = @data.ljust(size, "\0") if size && @data.bytesize < size
           (DL::CPtr.instances ||= {})[object_id] = self
         end
         def [](index)
           @data.getbyte(index) || 0
         end
         def []=(index, value)
           @data.setbyte(index, value.to_i & 0xff)
         end
         def to_i
           object_id
         end
       end
     end
     ```

2. In `class Win32API`:
   - When `@func == 'GetKeyboardState'`:
     - Look up `ptr_target`: if `args[0].is_a?(Numeric)`, find in `DL::CPtr.instances[args[0].to_i]`; if `args[0].is_a?(String)`, use `args[0]`.
     - Check if `File.exist?("rp_input_state.bin")`.
     - Read 256 bytes from `"rp_input_state.bin"`.
     - If `ptr_target.is_a?(String)`: `ptr_target[0, 256] = data[0, 256]`.
     - If `ptr_target.is_a?(DL::CPtr)`: `0.upto(255) { |k| ptr_target[k] = data.getbyte(k) || 0 }`.
     - Return 1.
   - When `@func == 'GetAsyncKeyState'` or `@func == 'GetKeyState'`:
     - `vk = args[0].to_i`
     - If `File.exist?("rp_input_state.bin")`:
       - `data = File.binread("rp_input_state.bin") rescue nil`
       - If `data && data.bytesize > vk`:
         - `return ((data.getbyte(vk) & 0x80) != 0) ? -32767 : 0`
     - Return 0.

3. Native Input Fallback:
   - Before `load_data('Data/Scripts.rvdata2')`:
     If `defined?(Input)`:
     Alias native C++ methods on `Input`:
     ```ruby
     if defined?(Input)
       class << Input
         alias_method :rp_native_press?, :press? rescue nil
         alias_method :rp_native_trigger?, :trigger? rescue nil
         alias_method :rp_native_dir4, :dir4 rescue nil
         alias_method :rp_native_dir8, :dir8 rescue nil
       end
     end
     ```
   - After `eval(script_source...)` loop (after all scripts are loaded):
     If `Input.respond_to?(:rp_native_press?)`:
     Hook `press?`, `trigger?`, `dir4`, `dir8`:
     ```ruby
     if defined?(Input) && Input.respond_to?(:rp_native_press?)
       class << Input
         alias_method :rp_hime_press?, :press? rescue nil
         alias_method :rp_hime_trigger?, :trigger? rescue nil
         alias_method :rp_hime_dir4, :dir4 rescue nil
         alias_method :rp_hime_dir8, :dir8 rescue nil

         def press?(keys)
           res = rp_hime_press?(keys) rescue false
           return true if res
           rp_native_press?(keys) rescue false
         end

         def trigger?(keys)
           res = rp_hime_trigger?(keys) rescue false
           return true if res
           rp_native_trigger?(keys) rescue false
         end

         def dir4
           d = rp_hime_dir4 rescue 0
           return d if d && d > 0
           rp_native_dir4 rescue 0
         end

         def dir8
           d = rp_hime_dir8 rescue 0
           return d if d && d > 0
           rp_native_dir8 rescue 0
         end
       end
     end
     ```

4. In `android/shell/app/src/test/ruby/input_hook_test.rb`:
   - Test `DL::CPtr` instance registry and byte assignment.
   - Test `Win32API.call('GetKeyboardState', cptr.to_i)` with a mock `rp_input_state.bin`.
   - Test `Win32API.call('GetAsyncKeyState', 0x26)` with VK_UP set.
   - Run tests with: `ruby android/shell/app/src/test/ruby/input_hook_test.rb`.

5. Report Contract:
   - Full report in `.superpowers/sdd/2026-09-18-input-sync-and-agnostic-cheat-engine/task-3-report.md`.
   - Return status `DONE`, commits, test output summary, concerns.
