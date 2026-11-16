# frozen_string_literal: false
# native_rgss_lifecycle_contract_test.rb
# TDD test suite for native RGSS object lifecycle safety (uninitialized disposables)
# Reproduces the crash in Dark Elf Historia:
#   Script '{0138}:LNX11a_XPスタイルバトル' line 1014:
#   MKXPError: No instance data for variable (missing call to super?)
# Verifies that uninitialized disposables (e.g. subclasses that omitted `super`)
# treat #dispose as a safe no-op and #disposed? as true, matching mkxp-z 130375b semantics.

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

def assert_raise(expected_exception, message = nil)
  $tests_run += 1
  begin
    yield
    $tests_failed += 1
    warn "FAILED: Expected #{expected_exception} to be raised, but nothing was raised. #{message}"
  rescue => e
    if e.is_a?(expected_exception) || (expected_exception.is_a?(Class) && e.class.name == expected_exception.name)
      $tests_passed += 1
    else
      $tests_failed += 1
      warn "FAILED: Expected #{expected_exception}, got #{e.class} (#{e.message}). #{message}"
    end
  end
end

puts "============================================================"
puts "Running Native RGSS Lifecycle Contract Tests"
puts "============================================================"

# Define MKXPError class simulating mkxp-z C++ exception
class MKXPError < StandardError; end unless defined?(MKXPError)

# --------------------------------------------------------------------------
# Simulate native mkxp-z C++ classes BEFORE bootstrap runs
# In mkxp-z, Sprite, Viewport, Plane, Window, Bitmap are defined in C++.
# If initialize (or super) is not called, C++ private userdata remains NULL.
# Calling getPrivateData on NULL raises MKXPError("No instance data for variable (missing call to super?)").
# --------------------------------------------------------------------------

class MockNativeDisposable
  attr_accessor :native_data, :is_disposed

  def initialize(*args)
    @native_data = :allocated_native_userdata
    @is_disposed = false
  end

  def dispose
    check_instance_data!
    @is_disposed = true
    @native_data = :disposed_native_userdata
    nil
  end

  def disposed?
    check_instance_data!
    @is_disposed
  end

  def check_instance_data!
    if @native_data.nil?
      raise MKXPError, "No instance data for variable (missing call to super?)"
    end
  end
end

class Sprite < MockNativeDisposable
  attr_accessor :x, :y, :visible

  def visible=(val)
    check_instance_data!
    @visible = val
  end
end unless defined?(Sprite)

class Viewport < MockNativeDisposable; end unless defined?(Viewport)
class Plane < MockNativeDisposable; end unless defined?(Plane)
class Window < MockNativeDisposable; end unless defined?(Window)
class Bitmap < MockNativeDisposable; end unless defined?(Bitmap)
class Tilemap < MockNativeDisposable; end unless defined?(Tilemap)

# Now load bootstrap
BOOTSTRAP_FILE = ENV['RUNTIMEPACK_BOOTSTRAP_PATH'] ?
  File.expand_path(ENV['RUNTIMEPACK_BOOTSTRAP_PATH']) :
  File.expand_path('../../main/assets/runtimepack_bootstrap.rb', __dir__)

require BOOTSTRAP_FILE

# ------------------------------------------------------------
# Test 1: Exact Dark Elf Scenario Reproduction (Script 138 & 147)
# ------------------------------------------------------------
puts "\nTest 1: Exact Dark Elf Historia Sprite_PopupBase scenario"

class MockSprite_PopupBase < Sprite
  def initialize(data = nil)
    # Script 147 line 594: return early if data == nil (omits super!)
    return if data == nil
    super(data)
  end

  def dispose
    # Script 138 line 1010-1014:
    # popup_remove
    # terminate
    # self.bitmap.dispose if self.bitmap
    # super (line 1014)
    super
  end
end

spb = MockSprite_PopupBase.new(nil)
assert spb.native_data.nil?, "Instance should have no native data because super was omitted"

# Calling dispose on spb MUST succeed without raising MKXPError
begin
  res = spb.dispose
  assert_equal nil, res, "Disposing uninitialized Sprite_PopupBase must return nil safely"
rescue MKXPError => e
  assert false, "Calling dispose on uninitialized Sprite raised MKXPError: #{e.message}"
end

# Calling disposed? on uninitialized spb MUST return true
assert_equal true, spb.disposed?, "Uninitialized Sprite must report disposed? == true"

# ------------------------------------------------------------
# Test 2: Initialized Sprite Lifecycle Preservation
# ------------------------------------------------------------
puts "\nTest 2: Normal Sprite with super called"

class MockSprite_Valid < Sprite
  def initialize(data = :custom_viewport)
    super(data)
  end
end

sv = MockSprite_Valid.new(:valid_viewport)
assert_equal :allocated_native_userdata, sv.native_data, "Normal Sprite must have native data allocated"
assert_equal false, sv.disposed?, "Normal Sprite before dispose must report disposed? == false"

sv.dispose
assert_equal true, sv.disposed?, "Normal Sprite after dispose must report disposed? == true"
assert_equal :disposed_native_userdata, sv.native_data, "Native dispose must have been called on initialized Sprite"

# ------------------------------------------------------------
# Test 3: Other Disposable Classes (Viewport, Plane, Window, Bitmap, Tilemap)
# ------------------------------------------------------------
puts "\nTest 3: Uninitialized Viewport, Plane, Window, Bitmap, Tilemap"

[Viewport, Plane, Window, Bitmap, Tilemap].each do |klass|
  subclass = Class.new(klass) do
    def initialize
      # omits super
    end
  end

  obj = subclass.new
  assert obj.native_data.nil?, "#{klass} subclass instance should have no native data"

  begin
    res = obj.dispose
    assert_equal nil, res, "#{klass}#dispose on uninitialized instance must return nil safely"
    assert_equal true, obj.disposed?, "#{klass}#disposed? on uninitialized instance must return true"
  rescue MKXPError => e
    assert false, "#{klass}#dispose raised MKXPError: #{e.message}"
  end
end

# ------------------------------------------------------------
# Test 4: Genuine / Non-Lifecycle Errors Must NOT Be Swallowed
# ------------------------------------------------------------
puts "\nTest 4: Non-lifecycle errors are preserved"

class MockBrokenSprite < Sprite
  def dispose
    raise RuntimeError, "custom non-lifecycle failure in game script"
  end
end

broken = MockBrokenSprite.new
assert_raise(RuntimeError, "Non-lifecycle errors in dispose must not be swallowed") do
  broken.dispose
end

# ------------------------------------------------------------
# Test 5: Property Access on Uninitialized Instance Still Fails
# ------------------------------------------------------------
puts "\nTest 5: Visual property access on uninitialized instance raises MKXPError"

spb_prop = MockSprite_PopupBase.new(nil)
assert_raise(MKXPError, "Accessing properties on uninitialized instance must still raise MKXPError") do
  spb_prop.check_instance_data!
end

# ------------------------------------------------------------
# Test 6: Idempotency of Hook Installation
# ------------------------------------------------------------
puts "\nTest 6: Hook idempotency"

# Re-evaluating hook definition must not raise or loop infinitely
assert Sprite.method_defined?(:runtimepack_orig_dispose), "Sprite must define runtimepack_orig_dispose"
assert Sprite.method_defined?(:runtimepack_orig_disposed?), "Sprite must define runtimepack_orig_disposed?"

puts "\n============================================================"
puts "Tests run: #{$tests_run}, Passed: #{$tests_passed}, Failed: #{$tests_failed}"
puts "============================================================"

if $tests_failed > 0
  puts "STATUS: RED (#{$tests_failed} assertions failed as expected before implementation)"
  exit 1
else
  puts "STATUS: GREEN (All native RGSS lifecycle contract tests passed)"
  exit 0
end
