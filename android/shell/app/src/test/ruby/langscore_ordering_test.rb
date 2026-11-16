# frozen_string_literal: true

require 'fileutils'
require 'tmpdir'

target_lib = ENV['TEST_TARGET_LIB'] || 'runtimepack_bootstrap'

if target_lib == 'win32api_shim'
  require_relative '../../main/assets/win32api_shim'
else
  require_relative '../../main/assets/runtimepack_bootstrap'
end

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

puts "============================================================"
puts "Running Langscore Language Restoration Ordering Tests (#{target_lib})"
puts "============================================================"

# Mock RGSS Actor and Data Structures
class MockActor
  attr_accessor :name, :nickname, :actor_id
  def initialize(id, name, nickname)
    @actor_id = id
    @name = name
    @nickname = nickname
  end
end

class MockGameActors
  def initialize
    @data = [
      nil,
      MockActor.new(1, "Samidare", "Heroine")
    ]
  end

  def [](id)
    @data[id]
  end

  def instance_variable_get(var)
    @data if var == :@data
  end
end

test_sandbox = Dir.mktmpdir("rp_langscore_test_")
original_pwd = Dir.pwd

begin
  Dir.chdir(test_sandbox)

  # ------------------------------------------------------------
  # Helper to set up mock Langscore & DataManager
  # ------------------------------------------------------------
  def setup_mock_langscore!
    # Translation dictionary for testing
    $ls_actor_tr = {
      "サミダレ" => {
        "ja" => "サミダレ",
        "en" => "Samidare",
        "zh-cn" => "五月雨"
      },
      "ヒロイン" => {
        "ja" => "ヒロイン",
        "en" => "Heroine",
        "zh-cn" => "女主角"
      }
    }

    # Clean existing definitions if any
    Object.send(:remove_const, :Langscore) if defined?(Langscore)
    Object.send(:remove_const, :DataManager) if defined?(DataManager)

    eval <<-RUBY
      module Langscore
        DEFAULT_LANGUAGE = "ja"
        $langscore_current_language = DEFAULT_LANGUAGE

        def self.translate(text, table, lang = $langscore_current_language)
          return text unless table && table[text]
          table[text][lang] || text
        end

        def self.fetch_original_text(text, table)
          return text unless table
          found = table.find { |_orig, trans| trans.values.include?(text) }
          found ? found[0] : text
        end

        def self.updateActor
          return unless $game_actors
          actors = $game_actors.instance_variable_get(:@data)
          return unless actors
          actors.each do |actor|
            next unless actor
            orig_name = Langscore.fetch_original_text(actor.name, $ls_actor_tr)
            orig_nick = Langscore.fetch_original_text(actor.nickname, $ls_actor_tr)
            actor.name = Langscore.translate(orig_name, $ls_actor_tr, $langscore_current_language)
            actor.nickname = Langscore.translate(orig_nick, $ls_actor_tr, $langscore_current_language)
          end
        end

        def self.changeLanguage(lang, force_update = false)
          $langscore_current_language = lang
          updateActor
        end
      end

      module DataManager
        class << self
          def make_save_contents
            contents = {}
            # Langscore sanitizes actors to original Japanese text before serializing
            data_temp = Marshal.dump($game_actors)
            actors = $game_actors.instance_variable_get(:@data)
            actors.each do |actor|
              next unless actor
              actor.name = Langscore.fetch_original_text(actor.name, $ls_actor_tr)
              actor.nickname = Langscore.fetch_original_text(actor.nickname, $ls_actor_tr)
            end
            contents[:actors] = Marshal.load(Marshal.dump($game_actors))
            $game_actors = Marshal.load(data_temp)
            contents
          end

          def extract_save_contents(contents)
            $game_actors = contents[:actors]
            # Langscore core ordering: post-load invokes changeLanguage with current language
            Langscore.changeLanguage($langscore_current_language, true)
          end
        end
      end
    RUBY
  end

  # ------------------------------------------------------------
  # Test Case 1: Baseline failure & ordering verification with Lang=en
  # ------------------------------------------------------------
  puts "Test 1: Configured language 'en' in Game.ini restores before extract_save_contents"
  File.write("Game.ini", "[Game]\r\nTitle=KING EXIT\r\n\r\n[Langscore]\r\nLang=en\r\n")

  setup_mock_langscore!

  # Install RuntimePack hook if available
  if defined?(RuntimePackLangscore) && RuntimePackLangscore.respond_to?(:install_save_load_hook)
    RuntimePackLangscore.install_save_load_hook
  end

  $game_actors = MockGameActors.new
  assert_equal "Samidare", $game_actors[1].name

  # Save game: verify save payload temporarily sanitized to original Japanese
  save_payload = DataManager.make_save_contents
  assert_equal "サミダレ", save_payload[:actors][1].name, "Save payload must contain original Japanese"
  assert_equal "Samidare", $game_actors[1].name, "In-memory actor must remain translated"

  # Simulate loading in a state where $langscore_current_language was left at default "ja"
  $langscore_current_language = "ja"

  # Perform save load reconstruction
  DataManager.extract_save_contents(save_payload)

  # Assertions: the configured language 'en' must be restored, and actor name must be 'Samidare'
  assert_equal "en", $langscore_current_language, "Current language must be restored to configured 'en'"
  assert_equal "Samidare", $game_actors[1].name, "Actor name after load must be 'Samidare', not Japanese 'サミダレ'"
  assert_equal "Heroine", $game_actors[1].nickname, "Actor nickname after load must be 'Heroine'"

  # ------------------------------------------------------------
  # Test Case 2: Early bootstrap language restoration
  # ------------------------------------------------------------
  puts "Test 2: Early bootstrap restores configured language before scripts evaluate"
  File.write("Game.ini", "[Game]\r\nTitle=KING EXIT\r\n\r\n[Langscore]\r\nLang=en\r\n")

  $langscore_current_language = nil
  if defined?(RuntimePackLangscore) && RuntimePackLangscore.respond_to?(:restore_bootstrap_language)
    boot_lang = RuntimePackLangscore.restore_bootstrap_language("./Game.ini")
    assert_equal "en", boot_lang, "Bootstrap restore must return configured language 'en'"
    assert_equal "en", $langscore_current_language, "$langscore_current_language must be set to 'en'"
  else
    assert false, "RuntimePackLangscore.restore_bootstrap_language must be defined"
  end

  # ------------------------------------------------------------
  # Test Case 3: Arbitrary configured language (e.g. 'zh-cn') - no hardcoded 'en'
  # ------------------------------------------------------------
  puts "Test 3: Arbitrary configured language 'zh-cn' restores correctly (anti-hardcoding)"
  File.write("Game.ini", "[Game]\r\nTitle=KING EXIT\r\n\r\n[Langscore]\r\nLang=zh-cn\r\n")

  setup_mock_langscore!
  if defined?(RuntimePackLangscore) && RuntimePackLangscore.respond_to?(:install_save_load_hook)
    RuntimePackLangscore.install_save_load_hook
  end

  $game_actors = MockGameActors.new
  save_payload = DataManager.make_save_contents

  $langscore_current_language = "ja"
  DataManager.extract_save_contents(save_payload)

  assert_equal "zh-cn", $langscore_current_language, "Current language must be restored to 'zh-cn'"
  assert_equal "五月雨", $game_actors[1].name, "Actor name must be translated to Chinese '五月雨'"
  assert_equal "女主角", $game_actors[1].nickname, "Actor nickname must be translated to Chinese '女主角'"

  # ------------------------------------------------------------
  # Test Case 4: Missing Langscore configuration is a no-op
  # ------------------------------------------------------------
  puts "Test 4: Missing Langscore configuration in Game.ini is a no-op"
  File.write("Game.ini", "[Game]\r\nTitle=Standard Game\r\n")

  setup_mock_langscore!
  if defined?(RuntimePackLangscore) && RuntimePackLangscore.respond_to?(:install_save_load_hook)
    RuntimePackLangscore.install_save_load_hook
  end

  $langscore_current_language = "custom_lang"
  $game_actors = MockGameActors.new
  save_payload = DataManager.make_save_contents

  # extract_save_contents should not overwrite custom_lang when no [Langscore] section exists
  DataManager.extract_save_contents(save_payload)
  assert_equal "custom_lang", $langscore_current_language, "Language should not be altered when config is missing"

  # ------------------------------------------------------------
  # Test Case 5: Missing Game.ini file is a safe no-op
  # ------------------------------------------------------------
  puts "Test 5: Missing Game.ini file is a safe no-op"
  File.delete("Game.ini") if File.exist?("Game.ini")

  setup_mock_langscore!
  if defined?(RuntimePackLangscore) && RuntimePackLangscore.respond_to?(:install_save_load_hook)
    RuntimePackLangscore.install_save_load_hook
  end

  $langscore_current_language = "ja"
  $game_actors = MockGameActors.new
  save_payload = DataManager.make_save_contents

  DataManager.extract_save_contents(save_payload)
  assert_equal "ja", $langscore_current_language, "Language should remain unchanged when Game.ini is missing"

  # ------------------------------------------------------------
  # Test Case 6: Malformed configuration values safely ignored
  # ------------------------------------------------------------
  puts "Test 6: Malformed configuration values safely ignored (empty, whitespace, traversal)"
  malformed_cases = [
    "[Langscore]\r\nLang=\r\n",
    "[Langscore]\r\nLang=   \r\n",
    "[Langscore]\r\nLang=../../etc/passwd\r\n",
    "[Langscore]\r\nLang=very_long_invalid_language_tag_that_exceeds_buffer_capacity\r\n",
    "[Langscore]\r\nLang=bad language tag!\r\n"
  ]

  malformed_cases.each_with_index do |ini_text, idx|
    File.write("Game.ini", ini_text)
    setup_mock_langscore!
    if defined?(RuntimePackLangscore) && RuntimePackLangscore.respond_to?(:install_save_load_hook)
      RuntimePackLangscore.install_save_load_hook
    end

    $langscore_current_language = "safe_existing"
    $game_actors = MockGameActors.new
    save_payload = DataManager.make_save_contents

    DataManager.extract_save_contents(save_payload)
    assert_equal "safe_existing", $langscore_current_language, "Malformed case #{idx} must not overwrite existing language"
  end

  # ------------------------------------------------------------
  # Test Case 7: Non-Langscore RGSS game behaves identically without errors
  # ------------------------------------------------------------
  puts "Test 7: Non-Langscore RGSS game (no Langscore module) is completely unharmed"
  File.write("Game.ini", "[Game]\r\nTitle=Non-Langscore Game\r\n\r\n[Langscore]\r\nLang=en\r\n")

  # Remove Langscore entirely
  Object.send(:remove_const, :Langscore) if defined?(Langscore)
  Object.send(:remove_const, :DataManager) if defined?(DataManager)

  eval <<-RUBY
    module DataManager
      class << self
        attr_accessor :loaded_contents
        def extract_save_contents(contents)
          @loaded_contents = contents
        end
      end
    end
  RUBY

  if defined?(RuntimePackLangscore) && RuntimePackLangscore.respond_to?(:install_save_load_hook)
    RuntimePackLangscore.install_save_load_hook
  end

  test_contents = { test: 123 }
  DataManager.extract_save_contents(test_contents)
  assert_equal test_contents, DataManager.loaded_contents, "Non-Langscore extract_save_contents must pass through unimpeded"

  # ------------------------------------------------------------
  # Test Case 8: In-game language change persisted to Game.ini restores on next load
  # ------------------------------------------------------------
  puts "Test 8: In-game language change persisted to Game.ini restores on subsequent load"
  File.write("Game.ini", "[Game]\r\nTitle=KING EXIT\r\n\r\n[Langscore]\r\nLang=en\r\n")

  setup_mock_langscore!
  if defined?(RuntimePackLangscore) && RuntimePackLangscore.respond_to?(:install_save_load_hook)
    RuntimePackLangscore.install_save_load_hook
  end

  $game_actors = MockGameActors.new
  save_payload = DataManager.make_save_contents

  # First load -> en
  $langscore_current_language = "ja"
  DataManager.extract_save_contents(save_payload)
  assert_equal "Samidare", $game_actors[1].name

  # In-game option changes language to 'zh-cn' and writes to Game.ini
  Win32API.call('WritePrivateProfileString', "Langscore", "Lang", "zh-cn", "./Game.ini")
  assert File.read("Game.ini").include?("Lang=zh-cn")

  # Subsequent load restores 'zh-cn'
  $langscore_current_language = "ja"
  DataManager.extract_save_contents(save_payload)
  assert_equal "zh-cn", $langscore_current_language, "Subsequent load must reflect updated Game.ini language"
  assert_equal "五月雨", $game_actors[1].name, "Actor name must reflect new language 'zh-cn'"

  # ------------------------------------------------------------
  # Test Case 9: Quoted and whitespace-padded language tags in Game.ini
  # ------------------------------------------------------------
  puts "Test 9: Quoted and whitespace-padded language tags in Game.ini"
  File.write("Game.ini", "[Game]\r\nTitle=KING EXIT\r\n\r\n[Langscore]\r\nLang=\"en\"\r\n")

  setup_mock_langscore!
  RuntimePackLangscore.install_save_load_hook

  $langscore_current_language = "ja"
  $game_actors = MockGameActors.new
  save_payload = DataManager.make_save_contents

  DataManager.extract_save_contents(save_payload)
  assert_equal "en", $langscore_current_language, "Quoted 'en' must be normalized to 'en'"
  assert_equal "Samidare", $game_actors[1].name, "Actor name must be 'Samidare'"

  # ------------------------------------------------------------
  # Test Case 10: Hook idempotence under repeated installation
  # ------------------------------------------------------------
  puts "Test 10: Hook idempotence under repeated installation"
  setup_mock_langscore!
  10.times { RuntimePackLangscore.install_save_load_hook }

  $langscore_current_language = "ja"
  $game_actors = MockGameActors.new
  save_payload = DataManager.make_save_contents

  DataManager.extract_save_contents(save_payload)
  assert_equal "en", $langscore_current_language, "Repeated install must not break language restoration"
  assert_equal "Samidare", $game_actors[1].name, "Repeated install must not cause recursion"

  # ------------------------------------------------------------
  # Test Case 11: Exact LSSetting contract as defined in King Exit Script 121
  # ------------------------------------------------------------
  puts "Test 11: Exact LSSetting contract as defined in King Exit Script 121"
  File.write("Game.ini", "[Game]\r\nTitle=KING EXIT\r\n\r\n[Langscore]\r\nLang=en\r\n")

  setup_mock_langscore!

  # Evaluate exact LSSetting class from Script 121 lines 1178-1197
  eval <<-LS_SETTING_CODE
    class LSSetting
      GetPrivateProfileString = Win32API.new('kernel32', 'GetPrivateProfileString', %w(p p p p l p), 'L')
      WritePrivateProfileString = Win32API.new('kernel32', 'WritePrivateProfileString', %w(p p p p), 'i')

      BufferSize = 16+1

      def self.load
        $langscore_current_language = Langscore::DEFAULT_LANGUAGE
        result = ' ' * BufferSize
        r = GetPrivateProfileString.call("Langscore", "Lang", "ja", result, BufferSize, "./Game.ini")
        if r != BufferSize-2
          $langscore_current_language = result.slice(0...r)
        end
      end

      def self.save
        WritePrivateProfileString.call("Langscore", "Lang", $langscore_current_language, "./Game.ini")
      end
    end
  LS_SETTING_CODE

  # Verify LSSetting.load reads 'en'
  $langscore_current_language = "ja"
  LSSetting.load
  assert_equal "en", $langscore_current_language, "LSSetting.load must load 'en' from Game.ini"

  # Verify LSSetting.save updates Game.ini
  $langscore_current_language = "zh-cn"
  LSSetting.save
  assert File.read("Game.ini").include?("Lang=zh-cn"), "LSSetting.save must persist 'zh-cn' to Game.ini"

  # Verify save/load restoration works alongside LSSetting
  RuntimePackLangscore.install_save_load_hook
  $langscore_current_language = "ja"
  $game_actors = MockGameActors.new
  save_payload = DataManager.make_save_contents
  DataManager.extract_save_contents(save_payload)
  assert_equal "zh-cn", $langscore_current_language, "Save load must restore newly saved 'zh-cn'"
  assert_equal "五月雨", $game_actors[1].name, "Actor name must be '五月雨'"

ensure
  Dir.chdir(original_pwd)
  FileUtils.remove_entry_secure(test_sandbox) rescue nil
end

puts "============================================================"
puts "Tests run: #{$tests_run}, Passed: #{$tests_passed}, Failed: #{$tests_failed}"
puts "============================================================"

if $tests_failed > 0
  puts "STATUS: RED (#{$tests_failed} assertions failed)"
  exit 1
else
  puts "STATUS: GREEN (#{target_lib} Langscore ordering contracts satisfied)"
  if ENV['TEST_TARGET_LIB'].nil?
    puts "\nTriggering sub-suite for win32api_shim..."
    require 'rbconfig'
    shim_ok = system({ 'TEST_TARGET_LIB' => 'win32api_shim' }, RbConfig.ruby, File.expand_path(__FILE__))
    exit(shim_ok ? 0 : 1)
  else
    exit 0
  end
end
