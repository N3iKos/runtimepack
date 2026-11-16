# frozen_string_literal: true

# Regression for the King Exit save-load SystemStackError (line 209 symptom).
#
# Real lifecycle being reproduced with the production RuntimePackLangscore
# module (loaded from the packaged asset, not a copied hook implementation):
#
#   script 004 DataManager -> install_save_load_hook
#   script 121 langscore   -> alias + replace extract_save_contents
#   after script 121       -> install_save_load_hook again
#   user load              -> DataManager.extract_save_contents
#
# The old hook used a shared mutable alias (rp_orig_extract_save_contents).
# Re-installation rebound that alias for every wrapper already in the chain,
# so the Langscore wrapper and the first RuntimePack wrapper called each other
# forever. This suite asserts the call chain terminates and every real layer
# runs once per load.
#
# Fixtures are authored. No game.pack, no game scripts, no Marshal load of a
# user save, no device/APK interaction.

require 'tmpdir'
require 'fileutils'
require 'stringio'

target_lib = ENV['TEST_TARGET_LIB'] || 'runtimepack_bootstrap'

if target_lib == 'win32api_shim'
  require_relative '../../main/assets/win32api_shim'
else
  require_relative '../../main/assets/runtimepack_bootstrap'
end

$tests_run = 0
$tests_passed = 0
$tests_failed = 0

def assert(condition, message = 'Assertion failed')
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

# SystemStackError is not a StandardError. Catch it here (test harness only) so
# a recursion regression is reported as a bounded assertion failure instead of
# an unbounded crash dump. Production stdout is muted while loading so a
# recursive run cannot flood the log with per-iteration messages.
def assert_no_stack_overflow(label)
  previous_stdout = $stdout
  $stdout = StringIO.new
  yield
  true
rescue SystemStackError => e
  assert(false, "#{label}: SystemStackError escaped the load path (#{e.message})")
  false
ensure
  $stdout = previous_stdout
end

# Build a fresh top-level DataManager (and optional Langscore) exactly like a
# new script-evaluation world. The base implementation counts calls and records
# the language visible at entry, then returns a distinct value.
def fresh_world!(with_langscore: true)
  Object.send(:remove_const, :DataManager) if Object.const_defined?(:DataManager)
  Object.send(:remove_const, :Langscore) if Object.const_defined?(:Langscore)

  Object.const_set(:Langscore, Module.new) if with_langscore

  dm = Module.new
  dm.singleton_class.class_eval do
    attr_accessor :base_calls, :base_payloads, :base_language_seen

    def extract_save_contents(contents)
      self.base_calls += 1
      self.base_payloads << contents
      self.base_language_seen << $langscore_current_language
      ($rp_test_order ||= []) << :base
      [:base_result, contents]
    end
  end
  dm.base_calls = 0
  dm.base_payloads = []
  dm.base_language_seen = []
  Object.const_set(:DataManager, dm)

  $rp_test_order = []
  $rp_test_layer_counts = Hash.new(0)
  dm
end

# Same shape as Langscore script 121: alias the currently installed method,
# then replace extract_save_contents with a wrapper that calls the alias and
# runs its own post-load step.
def add_external_alias_layer!(alias_name, label)
  dm = DataManager
  dm.singleton_class.class_eval do
    alias_method alias_name, :extract_save_contents
    define_method(:extract_save_contents) do |contents|
      $rp_test_layer_counts[label] += 1
      $rp_test_order << label
      result = send(alias_name, contents)
      $rp_test_order << :"#{label}_post"
      result
    end
  end
end

def add_raising_external_layer!(alias_name, label, message)
  dm = DataManager
  dm.singleton_class.class_eval do
    alias_method alias_name, :extract_save_contents
    define_method(:extract_save_contents) do |_contents|
      $rp_test_layer_counts[label] += 1
      raise RuntimeError, message
    end
  end
end

def install_hook
  RuntimePackLangscore.install_save_load_hook
end

def hooked_method
  RuntimePackLangscore.instance_variable_get(:@hooked_method)
end

puts '============================================================'
puts "Running Langscore save/load hook interleaving regression (#{target_lib})"
puts '============================================================'

test_sandbox = Dir.mktmpdir('rp_load_hook_interleaving_')
original_pwd = Dir.pwd

begin
  Dir.chdir(test_sandbox)
  File.write('Game.ini', "[Game]\r\nTitle=KING EXIT\r\n\r\n[Langscore]\r\nLang=en\r\n")

  # ------------------------------------------------------------------
  # Cases 1-4: install, Langscore-style alias wrap, reinstall, repeated
  # no-change installs, then load.
  # ------------------------------------------------------------------
  puts 'Case 1-4: install -> external alias wrap -> reinstall -> repeat install -> load'
  fresh_world!
  $langscore_current_language = 'ja'

  install_hook
  first_hook = hooked_method
  assert(first_hook.is_a?(Method), 'first install must record the installed hook Method')

  add_external_alias_layer!(:ls_extract_save_contents, :langscore)
  install_hook
  second_hook = hooked_method
  assert(!first_hook.equal?(second_hook), 'reinstall after an external override must install a new wrapper')

  3.times { install_hook }
  assert_equal second_hook, hooked_method,
               'no-change reinstalls must not add another wrapper layer'

  result = nil
  assert_no_stack_overflow('case1 load') do
    result = DataManager.extract_save_contents(payload: 1)
  end

  assert_equal 1, DataManager.base_calls, 'base extract_save_contents must run exactly once per load'
  assert_equal 1, $rp_test_layer_counts[:langscore], 'Langscore wrapper must run exactly once per load'
  assert_equal [:base_result, { payload: 1 }], result, 'wrapper must preserve the original return value'
  assert_equal ['en'], DataManager.base_language_seen,
               'configured language must be restored before the base method runs'
  assert_equal 'en', $langscore_current_language, 'final language must be the configured language'
  assert_equal [:langscore, :base, :langscore_post], $rp_test_order,
               'order must be: game wrapper pre -> base -> game wrapper post'

  # ------------------------------------------------------------------
  # Case 5: more than one distinct external alias layer, repeated load.
  # ------------------------------------------------------------------
  puts 'Case 5: two distinct external alias layers and repeated load'
  fresh_world!
  $langscore_current_language = 'ja'

  install_hook
  add_external_alias_layer!(:ls_extract_save_contents, :langscore_outer)
  install_hook
  add_external_alias_layer!(:custom_extract_save_contents, :game_layer)
  install_hook

  2.times do
    assert_no_stack_overflow('case5 load') { DataManager.extract_save_contents(0) }
  end

  assert_equal 2, DataManager.base_calls, 'base must run once per load across repeated loads'
  assert_equal 2, $rp_test_layer_counts[:langscore_outer], 'first external layer must run once per load'
  assert_equal 2, $rp_test_layer_counts[:game_layer], 'second external layer must run once per load'
  assert_equal ['en', 'en'], DataManager.base_language_seen,
               'language must be restored before the base method on every load'
  expected_order = [
    :game_layer, :langscore_outer, :base, :langscore_outer_post, :game_layer_post,
    :game_layer, :langscore_outer, :base, :langscore_outer_post, :game_layer_post
  ]
  assert_equal expected_order, $rp_test_order,
               'each layer must execute once per load in pre/outer-to-inner and post/inner-to-outer order'

  # ------------------------------------------------------------------
  # Case 6: base and game-layer errors must propagate, not be swallowed.
  # ------------------------------------------------------------------
  puts 'Case 6: base and game-layer errors propagate'
  fresh_world!
  $langscore_current_language = 'ja'

  DataManager.singleton_class.class_eval do
    def extract_save_contents(_contents)
      self.base_calls += 1
      raise ArgumentError, 'base boom'
    end
  end
  install_hook
  add_external_alias_layer!(:ls_extract_save_contents, :langscore)
  install_hook

  base_error = nil
  assert_no_stack_overflow('case6 base error') do
    begin
      DataManager.extract_save_contents({})
    rescue StandardError => e
      base_error = e
    end
  end
  assert(base_error.is_a?(ArgumentError), "base error must propagate as ArgumentError, got #{base_error.class}")
  assert_equal 'base boom', (base_error && base_error.message), 'base error message must be preserved'

  fresh_world!
  $langscore_current_language = 'ja'
  install_hook
  add_raising_external_layer!(:ls_extract_save_contents, :raising_layer, 'game boom')
  install_hook

  game_error = nil
  assert_no_stack_overflow('case6 game error') do
    begin
      DataManager.extract_save_contents({})
    rescue StandardError => e
      game_error = e
    end
  end
  assert(game_error.is_a?(RuntimeError), "game-layer error must propagate as RuntimeError, got #{game_error.class}")
  assert_equal 'game boom', (game_error && game_error.message), 'game-layer error message must be preserved'
  assert_equal 0, DataManager.base_calls, 'base must not run when the game layer raises before delegating'

  # ------------------------------------------------------------------
  # Case 7: non-Langscore game keeps the original behaviour.
  # ------------------------------------------------------------------
  puts 'Case 7: non-Langscore game calls the original method unchanged'
  fresh_world!(with_langscore: false)
  $langscore_current_language = 'ja'
  assert(!Object.const_defined?(:Langscore), 'fixture must not define Langscore')

  install_hook
  passthrough_result = nil
  assert_no_stack_overflow('case7 load') do
    passthrough_result = DataManager.extract_save_contents({ untouched: true })
  end

  assert_equal 1, DataManager.base_calls, 'non-Langscore base must run exactly once'
  assert_equal [:base_result, { untouched: true }], passthrough_result,
               'non-Langscore return value must pass through unchanged'
  assert_equal 'ja', $langscore_current_language,
               'non-Langscore game must not have its language rewritten'
  assert_equal [:base], $rp_test_order, 'non-Langscore call must reach the base method directly'

  # ------------------------------------------------------------------
  # Case 8: lifecycle reinstall after replacing / resetting DataManager.
  # ------------------------------------------------------------------
  puts 'Case 8: reinstall after replacing and resetting DataManager'
  fresh_world!
  $langscore_current_language = 'ja'
  install_hook
  add_external_alias_layer!(:ls_extract_save_contents, :langscore)
  install_hook
  assert_no_stack_overflow('case8 initial load') { DataManager.extract_save_contents({}) }
  assert_equal 1, DataManager.base_calls, 'initial lifecycle load must reach base once'

  # 8a: replace the whole method on the same module (external script override).
  DataManager.base_calls = 0
  DataManager.base_payloads = []
  DataManager.base_language_seen = []
  $rp_test_order = []
  DataManager.singleton_class.class_eval do
    def extract_save_contents(contents)
      self.base_calls += 1
      self.base_payloads << contents
      self.base_language_seen << $langscore_current_language
      ($rp_test_order ||= []) << :replacement
      [:replacement_result, contents]
    end
  end
  install_hook
  replacement_result = nil
  assert_no_stack_overflow('case8 replacement load') do
    replacement_result = DataManager.extract_save_contents({ replaced: true })
  end
  assert_equal 1, DataManager.base_calls, 'replaced implementation must run exactly once'
  assert_equal [:replacement_result, { replaced: true }], replacement_result,
               'replaced implementation return value must be preserved'
  assert_equal ['en'], DataManager.base_language_seen,
               'hook must still restore language before a replaced implementation'

  # 8b: reset DataManager (new module object), as after a full script reload.
  fresh_world!
  $langscore_current_language = 'ja'
  install_hook
  reset_result = nil
  assert_no_stack_overflow('case8 reset load') do
    reset_result = DataManager.extract_save_contents({ reset: true })
  end
  assert_equal 1, DataManager.base_calls, 'reset DataManager must be hooked exactly once'
  assert_equal [:base_result, { reset: true }], reset_result,
               'reset DataManager return value must be preserved'
  assert_equal 'en', $langscore_current_language,
               'hook must restore language on the reset DataManager'
ensure
  Dir.chdir(original_pwd)
  FileUtils.remove_entry_secure(test_sandbox) rescue nil
end

puts '============================================================'
puts "Tests run: #{$tests_run}, Passed: #{$tests_passed}, Failed: #{$tests_failed}"
puts '============================================================'

if $tests_failed > 0
  puts "STATUS: RED (#{$tests_failed} assertions failed)"
  exit 1
else
  puts "STATUS: GREEN (#{target_lib} save/load hook interleaving contracts satisfied)"
  if ENV['TEST_TARGET_LIB'].nil?
    puts "\nTriggering sub-suite for win32api_shim..."
    require 'rbconfig'
    shim_ok = system({ 'TEST_TARGET_LIB' => 'win32api_shim' }, RbConfig.ruby, File.expand_path(__FILE__))
    exit(shim_ok ? 0 : 1)
  else
    exit 0
  end
end
