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

  # 3b. Test ALT (0x12, VK_MENU) and Direction (0x26, VK_UP) Coexistence
  puts "Test 3b: ALT (0x12, VK_MENU) and Direction (0x26, VK_UP) Coexistence"
  combo_buffer = "\0" * 256
  combo_buffer.setbyte(0x12, 0x80) # VK_MENU (Alt)
  combo_buffer.setbyte(0x26, 0x80) # VK_UP
  File.binwrite(STATE_FILE, combo_buffer)

  combo_ptr = DL::CPtr.new(DL.malloc(256))
  Win32API.call('GetKeyboardState', combo_ptr.to_i)
  assert_equal 0x80, combo_ptr[0x12], "combo_ptr VK_MENU (0x12) should be 0x80"
  assert_equal 0x80, combo_ptr[0x26], "combo_ptr VK_UP (0x26) should be 0x80"
  assert_equal(-32767, Win32API.call('GetAsyncKeyState', 0x12), "GetAsyncKeyState for VK_MENU should return -32767")
  assert_equal(-32767, Win32API.call('GetAsyncKeyState', 0x26), "GetAsyncKeyState for VK_UP should return -32767")

  # Release Direction (0x26), keep ALT (0x12)
  combo_buffer.setbyte(0x26, 0x00)
  File.binwrite(STATE_FILE, combo_buffer)
  Win32API.call('GetKeyboardState', combo_ptr.to_i)
  assert_equal 0x80, combo_ptr[0x12], "combo_ptr VK_MENU (0x12) should remain 0x80 after direction release"
  assert_equal 0, combo_ptr[0x26], "combo_ptr VK_UP (0x26) should be 0 after release"
  assert_equal(-32767, Win32API.call('GetAsyncKeyState', 0x12), "GetAsyncKeyState for VK_MENU should remain -32767")
  assert_equal 0, Win32API.call('GetAsyncKeyState', 0x26), "GetAsyncKeyState for VK_UP should return 0 after release"

  # Release ALT (0x12)
  combo_buffer.setbyte(0x12, 0x00)
  File.binwrite(STATE_FILE, combo_buffer)
  Win32API.call('GetKeyboardState', combo_ptr.to_i)
  assert_equal 0, combo_ptr[0x12], "combo_ptr VK_MENU (0x12) should be 0 after release"
  assert_equal 0, Win32API.call('GetAsyncKeyState', 0x12), "GetAsyncKeyState for VK_MENU should return 0 after release"

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

  # 5. Test Auto-Recovery for Bitmap.new
  puts "Test 5: Auto-Recovery for Bitmap.new"
  dummy_bmp = Bitmap.new("non_existent_file.png")
  assert !dummy_bmp.nil?, "Bitmap.new with non-existent file should return dummy bitmap"
  assert dummy_bmp.is_a?(Bitmap), "dummy_bmp should be an instance of Bitmap"
  assert_equal 32, dummy_bmp.width, "Dummy bitmap width should be 32"
  assert_equal 32, dummy_bmp.height, "Dummy bitmap height should be 32"
  assert !dummy_bmp.disposed?, "Dummy bitmap should not be disposed"

  # Normal Bitmap.new(w, h) should still function normally
  normal_bmp = Bitmap.new(64, 48)
  assert !normal_bmp.nil?, "Bitmap.new(64, 48) should create bitmap"
  assert_equal 64, normal_bmp.width, "normal_bmp width should be 64"
  assert_equal 48, normal_bmp.height, "normal_bmp height should be 48"

  # 6. Test Auto-Recovery for Audio
  puts "Test 6: Auto-Recovery for Audio"
  audio_bgm_res = (Audio.bgm_play("non_existent_bgm.ogg") rescue :raised)
  assert audio_bgm_res != :raised, "Audio.bgm_play should not raise exception on missing file"

  audio_bgs_res = (Audio.bgs_play("non_existent_bgs.ogg") rescue :raised)
  assert audio_bgs_res != :raised, "Audio.bgs_play should not raise exception on missing file"

  audio_me_res = (Audio.me_play("non_existent_me.ogg") rescue :raised)
  assert audio_me_res != :raised, "Audio.me_play should not raise exception on missing file"

  audio_se_res = (Audio.se_play("non_existent_se.ogg") rescue :raised)
  assert audio_se_res != :raised, "Audio.se_play should not raise exception on missing file"

  # 7. Test Auto-Recovery for Cache
  puts "Test 7: Auto-Recovery for Cache"
  module MockGameCache
    def self.character(name)
      raise Errno::ENOENT, "Graphics/Characters/#{name} not found"
    end

    def self.picture(name)
      raise Errno::ENOENT, "Graphics/Pictures/#{name} not found"
    end

    def self.animation(name, hue)
      raise Errno::ENOENT, "Graphics/Animations/#{name} not found"
    end

    def self.battleback1(name)
      raise Errno::ENOENT, "Graphics/Battlebacks1/#{name} not found"
    end

    def self.battleback2(name)
      raise Errno::ENOENT, "Graphics/Battlebacks2/#{name} not found"
    end

    def self.battler(name, hue)
      raise Errno::ENOENT, "Graphics/Battlers/#{name} not found"
    end

    def self.face(name)
      raise Errno::ENOENT, "Graphics/Faces/#{name} not found"
    end

    def self.system(name)
      raise Errno::ENOENT, "Graphics/System/#{name} not found"
    end

    def self.tileset(name)
      raise Errno::ENOENT, "Graphics/Tilesets/#{name} not found"
    end

    def self.title1(name)
      raise Errno::ENOENT, "Graphics/Titles1/#{name} not found"
    end

    def self.title2(name)
      raise Errno::ENOENT, "Graphics/Titles2/#{name} not found"
    end
  end

  Cache = MockGameCache unless defined?(Cache)
  RuntimePackCheatRunner.install_save_and_cache_hooks

  cache_methods = [
    [:character, "Hero"],
    [:picture, "Cutscene"],
    [:animation, "Thunder", 0],
    [:battleback1, "Grassland"],
    [:battleback2, "Forest"],
    [:battler, "Dragon", 0],
    [:face, "Actor1"],
    [:system, "Window"],
    [:tileset, "Dungeon"],
    [:title1, "Book"],
    [:title2, "Sword"]
  ]

  cache_methods.each do |method_name, *args|
    cbmp = Cache.send(method_name, *args)
    assert !cbmp.nil?, "Cache.#{method_name} should return dummy bitmap on missing file"
    assert cbmp.is_a?(Bitmap), "Cache.#{method_name} should return Bitmap instance"
    assert_equal 32, cbmp.width, "Cache.#{method_name} dummy bitmap width should be 32"
    assert_equal 32, cbmp.height, "Cache.#{method_name} dummy bitmap height should be 32"
  end

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
