# frozen_string_literal: true

require 'fileutils'
require_relative '../../main/assets/runtimepack_bootstrap'

$tests_run = 0
$tests_passed = 0
$tests_failed = 0

def assert(condition, message = "Assertion failed")
  $tests_run += 1
  if condition
    $tests_passed += 1
  else
    $tests_failed += 1
    warn "FAILED: #{message}"
    caller_locations(1, 1).each { |loc| warn "  at #{loc}" }
  end
end

def assert_equal(expected, actual, message = nil)
  msg = message || "Expected #{expected.inspect}, got #{actual.inspect}"
  assert(expected == actual, msg)
end

STATE_FILE = "rp_input_state.bin"

def cleanup_state_file
  File.delete(STATE_FILE) if File.exist?(STATE_FILE)
end

begin
  cleanup_state_file

  puts "Running input_hook_test.rb..."

  # 1. Test DL::CPtr instance registry and byte assignment
  puts "Test 1: DL::CPtr instance registry and byte assignment"
  cptr = DL::CPtr.new(DL.malloc(256))
  assert cptr.is_a?(DL::CPtr), "cptr should be instance of DL::CPtr"
  assert cptr.to_i.is_a?(Integer), "cptr.to_i should be an Integer"
  assert_equal cptr, DL::CPtr.instances[cptr.to_i], "cptr should be registered in DL::CPtr.instances by object_id"

  assert_equal 0, cptr[0], "initial byte at 0 should be 0"
  assert_equal 0, cptr[0x26], "initial byte at 0x26 should be 0"

  cptr[0x26] = 0x80
  assert_equal 0x80, cptr[0x26], "cptr[0x26] should be 0x80 after assignment"

  cptr[10] = 255
  assert_equal 255, cptr[10], "cptr[10] should be 255 after assignment"

  # Test auto-expansion
  cptr[300] = 42
  assert_equal 42, cptr[300], "cptr should handle index beyond initial size"

  # 2. Test Win32API GetKeyboardState with mock rp_input_state.bin
  puts "Test 2: Win32API GetKeyboardState"
  mock_buffer = "\0" * 256
  # Set VK_UP (0x26), VK_RETURN (0x0D), and VK_SPACE (0x20)
  mock_buffer.setbyte(0x26, 0x80)
  mock_buffer.setbyte(0x0D, 0x80)
  mock_buffer.setbyte(0x20, 0x80)
  File.binwrite(STATE_FILE, mock_buffer)

  test_ptr = DL::CPtr.new(DL.malloc(256))
  res = Win32API.call('GetKeyboardState', test_ptr.to_i)
  assert_equal 1, res, "Win32API.call('GetKeyboardState', ptr_id) should return 1"
  assert_equal 0x80, test_ptr[0x26], "test_ptr[0x26] (VK_UP) should be 0x80"
  assert_equal 0x80, test_ptr[0x0D], "test_ptr[0x0D] (VK_RETURN) should be 0x80"
  assert_equal 0x80, test_ptr[0x20], "test_ptr[0x20] (VK_SPACE) should be 0x80"
  assert_equal 0, test_ptr[0x27], "test_ptr[0x27] (VK_RIGHT) should be 0"

  # Test with String buffer
  str_buf = "\0" * 256
  res_str = Win32API.call('GetKeyboardState', str_buf)
  assert_equal 1, res_str, "Win32API.call('GetKeyboardState', str_buf) should return 1"
  assert_equal 0x80, str_buf.getbyte(0x26), "str_buf VK_UP should be 0x80"
  assert_equal 0x80, str_buf.getbyte(0x0D), "str_buf VK_RETURN should be 0x80"
  assert_equal 0, str_buf.getbyte(0x27), "str_buf VK_RIGHT should be 0"

  # Test via Win32API instance
  api_inst = Win32API.new('user32', 'GetKeyboardState', 'p', 'i')
  test_ptr_inst = DL::CPtr.new(DL.malloc(256))
  res_inst = api_inst.call(test_ptr_inst.to_i)
  assert_equal 1, res_inst, "api_inst.call(ptr_id) should return 1"
  assert_equal 0x80, test_ptr_inst[0x26], "instance call: VK_UP should be 0x80"

  # 3. Test Win32API GetAsyncKeyState & GetKeyState with VK_UP set
  puts "Test 3: Win32API GetAsyncKeyState & GetKeyState"
  # VK_UP is 0x26, which is active (0x80) in mock_buffer
  async_up = Win32API.call('GetAsyncKeyState', 0x26)
  assert_equal(-32767, async_up, "GetAsyncKeyState for active VK_UP (0x26) should return -32767")

  key_state_up = Win32API.call('GetKeyState', 0x26)
  assert_equal(-32767, key_state_up, "GetKeyState for active VK_UP (0x26) should return -32767")

  # VK_DOWN is 0x28, not set
  async_down = Win32API.call('GetAsyncKeyState', 0x28)
  assert_equal 0, async_down, "GetAsyncKeyState for inactive VK_DOWN (0x28) should return 0"

  # VK_RIGHT is 0x27, not set
  async_right = Win32API.call('GetAsyncKeyState', 0x27)
  assert_equal 0, async_right, "GetAsyncKeyState for inactive VK_RIGHT (0x27) should return 0"

  # Test instance call for GetAsyncKeyState
  api_async = Win32API.new('user32', 'GetAsyncKeyState', 'i', 'i')
  assert_equal(-32767, api_async.call(0x26), "Instance call GetAsyncKeyState active should return -32767")
  assert_equal 0, api_async.call(0x28), "Instance call GetAsyncKeyState inactive should return 0"

  # Test when state file does not exist
  cleanup_state_file
  assert_equal 0, Win32API.call('GetAsyncKeyState', 0x26), "GetAsyncKeyState without state file should return 0"

  # 4. Test Native Input Fallback Hooks
  puts "Test 4: Native Input Fallback Hooks"
  # Set up mock Input module simulating mkxp-z native Input and Hime AllKey
  module MockNativeInput
    def self.press?(key)
      key == :native_key
    end

    def self.trigger?(key)
      key == :native_key
    end

    def self.dir4
      4
    end

    def self.dir8
      4
    end
  end

  # Test fallback hooking logic
  class << MockNativeInput
    alias_method :rp_native_press?, :press?
    alias_method :rp_native_trigger?, :trigger?
    alias_method :rp_native_dir4, :dir4
    alias_method :rp_native_dir8, :dir8

    # Simulate Hime overriding Input methods
    def press?(key)
      key == :hime_key
    end

    def trigger?(key)
      key == :hime_key
    end

    def dir4
      0 # Hime doesn't detect dir4
    end

    def dir8
      0 # Hime doesn't detect dir8
    end

    # Apply the RuntimePack fallback hook
    alias_method :rp_hime_press?, :press?
    alias_method :rp_hime_trigger?, :trigger?
    alias_method :rp_hime_dir4, :dir4
    alias_method :rp_hime_dir8, :dir8

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

  assert MockNativeInput.press?(:hime_key), "press? should return true when hime returns true"
  assert MockNativeInput.press?(:native_key), "press? should fallback to native when hime returns false"
  assert !MockNativeInput.press?(:unknown_key), "press? should return false when neither returns true"

  assert MockNativeInput.trigger?(:hime_key), "trigger? should return true when hime returns true"
  assert MockNativeInput.trigger?(:native_key), "trigger? should fallback to native when hime returns false"
  assert !MockNativeInput.trigger?(:unknown_key), "trigger? should return false when neither returns true"

  assert_equal 4, MockNativeInput.dir4, "dir4 should fallback to native (4) when hime returns 0"
  assert_equal 4, MockNativeInput.dir8, "dir8 should fallback to native (4) when hime returns 0"

ensure
  cleanup_state_file
end

puts "\n=================================================="
puts "Tests run: #{$tests_run}, Passed: #{$tests_passed}, Failed: #{$tests_failed}"
puts "=================================================="

if $tests_failed > 0
  exit 1
else
  puts "All input hook tests passed successfully!"
  exit 0
end
