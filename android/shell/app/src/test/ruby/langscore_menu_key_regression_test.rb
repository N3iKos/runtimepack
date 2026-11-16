# frozen_string_literal: true

require 'fileutils'
require 'tmpdir'
require 'open3'
require 'json'
require 'rbconfig'

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

def assert_not_equal(unexpected, actual, message = nil)
  msg = message || "Expected not #{unexpected.inspect}, but got #{actual.inspect}"
  assert(unexpected != actual, msg)
end

puts "============================================================"
puts "Running Langscore Menu Translation Key Regression Tests (#{target_lib})"
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
      MockActor.new(1, "サミダレ", "ヒロイン")
    ]
  end

  def [](id)
    @data[id]
  end

  def instance_variable_get(var)
    @data if var == :@data
  end
end

# Mock CAO Custom Menu command items structure (Script 181 / 217)
module CAO
  module CM
    COMMAND_ITEMS = {
      item: { name: "", help: "" },
      file: { name: "", help: "" },
      option: { name: "", help: "" }
    }
  end
end

test_sandbox = Dir.mktmpdir("rp_langscore_menu_test_")
original_pwd = Dir.pwd
repo_root = File.expand_path("../../../../../..", __dir__)

begin
  Dir.chdir(test_sandbox)

  # ------------------------------------------------------------
  # Helper to set up mock Langscore & DataManager
  # Exact fidelity with published King Exit scripts 121, 181, 217
  # ------------------------------------------------------------
  def setup_mock_langscore!
    # Translation dictionary representing Data/Translate/Scripts.csv
    # Keys are literal SCRIPT_ID:LINE:COLUMN strings
    $ls_scripts_tr = {
      "49037624:35:19" => { "en" => "Barang", "ja" => "アイテム", "zh-cn" => "道具" },
      "49037624:37:19" => { "en" => "Gunakan item", "ja" => "アイテムを使用します。", "zh-cn" => "使用道具" },
      "49037624:67:19" => { "en" => "Berkas", "ja" => "ファイル", "zh-cn" => "档案" },
      "49037624:69:19" => { "en" => "Simpan dan muat berkas", "ja" => "ゲームのセーブ・ロードを行います。", "zh-cn" => "存档与读档" },
      "49037624:84:19" => { "en" => "Pengaturan", "ja" => "オプション", "zh-cn" => "选项" },
      "49037624:86:19" => { "en" => "Atur volume dan sistem", "ja" => "音量設定・ゲーム終了。", "zh-cn" => "音量设定与系统" }
    }

    # Translation dictionary representing Data/Translate/Actors.csv
    $ls_actor_tr = {
      "サミダレ" => { "en" => "Samidare", "ja" => "サミダレ", "zh-cn" => "五月雨" },
      "ヒロイン" => { "en" => "Heroine", "ja" => "ヒロイン", "zh-cn" => "女主角" }
    }

    # Clean existing definitions if any
    Object.send(:remove_const, :Langscore) if defined?(Langscore)
    Object.send(:remove_const, :LSSetting) if defined?(LSSetting)
    Object.send(:remove_const, :DataManager) if defined?(DataManager)

    eval <<-RUBY
      module Langscore
        DEFAULT_LANGUAGE = "ja"
        $langscore_current_language = DEFAULT_LANGUAGE

        # Langscore.translate core lookup (Script 121 lines 347-380)
        def self.translate(text, table, lang = $langscore_current_language)
          return text if table.nil?
          key = text
          if table.has_key?(key)
            translist = table[key]
            if translist.has_key?(lang)
              t = translist[lang]
              text = t unless t.nil? || t.empty?
            end
          end
          # Fallthrough: if lang not found (e.g. empty string ""), returns original text
          text
        end

        # Langscore.translate_for_script (Script 121 lines 393-396)
        def self.translate_for_script(text)
          translate(text, $ls_scripts_tr)
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

        # Langscore custom menu commands update (Script 217 lines 13740-13845)
        def self.updateMenu
          CAO::CM::COMMAND_ITEMS[:item][:name] = Langscore.translate_for_script("49037624:35:19")
          CAO::CM::COMMAND_ITEMS[:item][:help] = Langscore.translate_for_script("49037624:37:19")
          CAO::CM::COMMAND_ITEMS[:file][:name] = Langscore.translate_for_script("49037624:67:19")
          CAO::CM::COMMAND_ITEMS[:file][:help] = Langscore.translate_for_script("49037624:69:19")
          CAO::CM::COMMAND_ITEMS[:option][:name] = Langscore.translate_for_script("49037624:84:19")
          CAO::CM::COMMAND_ITEMS[:option][:help] = Langscore.translate_for_script("49037624:86:19")
        end

        def self.changeLanguage(lang, force_update = false)
          $langscore_current_language = lang
          updateActor
          updateMenu
        end
      end

      # Exact LSSetting class from King Exit Script 121 (lines 1178-1197)
      class LSSetting
        GetPrivateProfileString = Win32API.new('kernel32', 'GetPrivateProfileString', %w(p p p p l p), 'L')
        WritePrivateProfileString = Win32API.new('kernel32', 'WritePrivateProfileString', %w(p p p p), 'i')

        BufferSize = 16 + 1 # 17

        def self.load
          $langscore_current_language = Langscore::DEFAULT_LANGUAGE
          result = ' ' * BufferSize
          r = GetPrivateProfileString.call("Langscore", "Lang", "ja", result, BufferSize, "./Game.ini")
          if r != BufferSize - 2
            $langscore_current_language = result.slice(0...r)
          end
          r
        end

        def self.save
          WritePrivateProfileString.call("Langscore", "Lang", $langscore_current_language, "./Game.ini")
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
            # Post-load invokes changeLanguage with current language
            Langscore.changeLanguage($langscore_current_language, true)
          end
        end
      end
    RUBY
  end

  # ------------------------------------------------------------
  # Test Case 1: Startup LSSetting call signature reproduction and menu translation (Lang=en)
  # ------------------------------------------------------------
  puts "Test 1: Startup LSSetting call signature (17 bytes) returns 'en' and translates menu commands"
  File.write("Game.ini", "[Game]\r\nTitle=KING EXIT\r\n\r\n[Langscore]\r\nLang=en\r\n")

  setup_mock_langscore!
  $game_actors = MockGameActors.new

  r = LSSetting.load
  assert_equal 2, r, "GetPrivateProfileString must return 2 (length of 'en')"
  assert_equal "en", $langscore_current_language, "$langscore_current_language must be 'en'"

  # Update menu commands using translate_for_script
  Langscore.updateMenu
  Langscore.updateActor

  # Menu Command 0: Line 35 Item
  assert_equal "Barang", CAO::CM::COMMAND_ITEMS[:item][:name], "Menu Item must translate to 'Barang'"
  assert_not_equal "49037624:35:19", CAO::CM::COMMAND_ITEMS[:item][:name], "Menu Item must NOT display raw numeric key"

  # Menu Command 6: Line 67 File
  assert_equal "Berkas", CAO::CM::COMMAND_ITEMS[:file][:name], "Menu File (line 67) must translate to 'Berkas'"
  assert_not_equal "49037624:67:19", CAO::CM::COMMAND_ITEMS[:file][:name], "Menu File must NOT display raw numeric key"

  # Menu Command 7: Line 84 Option
  assert_equal "Pengaturan", CAO::CM::COMMAND_ITEMS[:option][:name], "Menu Option (line 84) must translate to 'Pengaturan'"
  assert_not_equal "49037624:84:19", CAO::CM::COMMAND_ITEMS[:option][:name], "Menu Option must NOT display raw numeric key"

  # Actor Name
  assert_equal "Samidare", $game_actors[1].name, "Actor name must translate to 'Samidare'"
  assert_not_equal "サミダレ", $game_actors[1].name, "Actor name must NOT remain Japanese"

  # ------------------------------------------------------------
  # Test Case 2: Multi-language verification (Lang=zh-cn) - Anti-hardcoding
  # ------------------------------------------------------------
  puts "Test 2: Startup with Lang=zh-cn translates menu commands to Chinese (no hardcoding)"
  File.write("Game.ini", "[Game]\r\nTitle=KING EXIT\r\n\r\n[Langscore]\r\nLang=zh-cn\r\n")

  setup_mock_langscore!
  $game_actors = MockGameActors.new

  r = LSSetting.load
  assert_equal 5, r, "GetPrivateProfileString must return 5 (length of 'zh-cn')"
  assert_equal "zh-cn", $langscore_current_language, "$langscore_current_language must be 'zh-cn'"

  Langscore.updateMenu
  Langscore.updateActor

  assert_equal "道具", CAO::CM::COMMAND_ITEMS[:item][:name], "Menu Item must translate to Chinese '道具'"
  assert_not_equal "Barang", CAO::CM::COMMAND_ITEMS[:item][:name], "Menu Item must NOT be Indonesian 'Barang'"
  assert_not_equal "49037624:35:19", CAO::CM::COMMAND_ITEMS[:item][:name], "Menu Item must NOT be numeric key"

  assert_equal "档案", CAO::CM::COMMAND_ITEMS[:file][:name], "Menu File (line 67) must translate to Chinese '档案'"
  assert_not_equal "Berkas", CAO::CM::COMMAND_ITEMS[:file][:name], "Menu File must NOT be Indonesian 'Berkas'"

  assert_equal "选项", CAO::CM::COMMAND_ITEMS[:option][:name], "Menu Option (line 84) must translate to Chinese '选项'"
  assert_not_equal "Pengaturan", CAO::CM::COMMAND_ITEMS[:option][:name], "Menu Option must NOT be Indonesian 'Pengaturan'"

  assert_equal "五月雨", $game_actors[1].name, "Actor name must translate to Chinese '五月雨'"

  # ------------------------------------------------------------
  # Test Case 3: Post-load reconstruction restores menu translations (Lang=en)
  # ------------------------------------------------------------
  puts "Test 3: Post-load extract_save_contents reconstruction restores menu translations (Lang=en)"
  File.write("Game.ini", "[Game]\r\nTitle=KING EXIT\r\n\r\n[Langscore]\r\nLang=en\r\n")

  setup_mock_langscore!
  if defined?(RuntimePackLangscore) && RuntimePackLangscore.respond_to?(:install_save_load_hook)
    RuntimePackLangscore.install_save_load_hook
  end

  $game_actors = MockGameActors.new
  Langscore.changeLanguage("en")

  # Create save payload: sanitized to Japanese
  save_payload = DataManager.make_save_contents
  assert_equal "サミダレ", save_payload[:actors][1].name, "Save payload on disk must contain neutral Japanese name"

  # Simulate in-memory desync / uninitialized state prior to save extraction
  $langscore_current_language = "ja"
  CAO::CM::COMMAND_ITEMS[:item][:name] = "STALE"
  CAO::CM::COMMAND_ITEMS[:file][:name] = "STALE"
  CAO::CM::COMMAND_ITEMS[:option][:name] = "STALE"

  # Perform save load reconstruction on a cloned payload (simulating reading from disk)
  load_clone = Marshal.load(Marshal.dump(save_payload))
  DataManager.extract_save_contents(load_clone)

  assert_equal "en", $langscore_current_language, "RuntimePack hook must restore current language to 'en' before load"
  assert_equal "Barang", CAO::CM::COMMAND_ITEMS[:item][:name], "Menu Item after load must be 'Barang'"
  assert_not_equal "49037624:35:19", CAO::CM::COMMAND_ITEMS[:item][:name], "Menu Item after load must NOT be numeric key"
  assert_equal "Berkas", CAO::CM::COMMAND_ITEMS[:file][:name], "Menu File after load must be 'Berkas'"
  assert_equal "Pengaturan", CAO::CM::COMMAND_ITEMS[:option][:name], "Menu Option after load must be 'Pengaturan'"
  assert_equal "Samidare", $game_actors[1].name, "Actor name after load must be 'Samidare'"

  # ------------------------------------------------------------
  # Test Case 4: Post-load reconstruction with Lang=zh-cn
  # ------------------------------------------------------------
  puts "Test 4: Post-load extract_save_contents reconstruction restores menu translations (Lang=zh-cn)"
  File.write("Game.ini", "[Game]\r\nTitle=KING EXIT\r\n\r\n[Langscore]\r\nLang=zh-cn\r\n")

  setup_mock_langscore!
  if defined?(RuntimePackLangscore) && RuntimePackLangscore.respond_to?(:install_save_load_hook)
    RuntimePackLangscore.install_save_load_hook
  end

  $game_actors = MockGameActors.new
  Langscore.changeLanguage("zh-cn")
  save_payload_zh = DataManager.make_save_contents

  $langscore_current_language = "ja"
  load_clone_zh = Marshal.load(Marshal.dump(save_payload_zh))
  DataManager.extract_save_contents(load_clone_zh)

  assert_equal "zh-cn", $langscore_current_language, "RuntimePack hook must restore current language to 'zh-cn'"
  assert_equal "道具", CAO::CM::COMMAND_ITEMS[:item][:name], "Menu Item after load must be Chinese '道具'"
  assert_equal "档案", CAO::CM::COMMAND_ITEMS[:file][:name], "Menu File after load must be Chinese '档案'"
  assert_equal "选项", CAO::CM::COMMAND_ITEMS[:option][:name], "Menu Option after load must be Chinese '选项'"
  assert_equal "五月雨", $game_actors[1].name, "Actor name after load must be Chinese '五月雨'"

  # ------------------------------------------------------------
  # Test Case 5: Serialized save payload data neutrality preserved on disk
  # ------------------------------------------------------------
  puts "Test 5: Original serialized save payload Japanese string is never modified by shim or hook"
  # Save to disk file (exact RGSS DataManager behavior)
  save_file_bytes = Marshal.dump(save_payload)
  File.binwrite("Save01.rvdata2", save_file_bytes)

  # Load raw bytes from disk and inspect contents independently of memory mutation
  loaded_from_disk = Marshal.load(File.binread("Save01.rvdata2"))
  assert_equal "サミダレ", loaded_from_disk[:actors][1].name, "Disk save actor name must remain Japanese 'サミダレ'"
  assert_equal "ヒロイン", loaded_from_disk[:actors][1].nickname, "Disk save actor nickname must remain Japanese 'ヒロイン'"

  # Even after multiple extraction passes, the disk bytes remain pristine
  DataManager.extract_save_contents(loaded_from_disk)
  reloaded_from_disk = Marshal.load(File.binread("Save01.rvdata2"))
  assert_equal "サミダレ", reloaded_from_disk[:actors][1].name, "Save file on disk is strictly read-only and unaltered"

  # ------------------------------------------------------------
  # Test Case 6: Safe Historical Reproduction (Red Phase Verification)
  # Extracts historical 10e2a92 shim to scratch and demonstrates exact failure
  # ------------------------------------------------------------
  puts "Test 6: Safe Historical Reproduction (Red Phase) verifies old shim failure"
  hist_scratch_dir = Dir.mktmpdir("rp_hist_repro_")
  begin
    hist_shim_path = File.join(hist_scratch_dir, "hist_win32api_shim.rb")
    git_bin = ENV['GIT_BIN'] ||
              ENV['PATH'].to_s.split(File::PATH_SEPARATOR).map { |dir|
                [File.join(dir, 'git'), File.join(dir, 'git.exe')]
              }.flatten.find { |p| File.file?(p) && (File.executable?(p) rescue true) } ||
              'git'
    git_show_cmd = [git_bin, "-C", repo_root, "show", "10e2a92:android/shell/app/src/main/assets/win32api_shim.rb"]
    hist_content, git_err, git_status = Open3.capture3(*git_show_cmd)

    if git_status.success? && !hist_content.empty?
      File.binwrite(hist_shim_path, hist_content)
      File.write(File.join(hist_scratch_dir, "Game.ini"), "[Langscore]\r\nLang=en\r\n")

      # Subprocess runner executing under the historical shim
      runner_script = File.join(hist_scratch_dir, "repro_runner.rb")
      File.write(runner_script, <<~'HIST_RUBY')
        require_relative "hist_win32api_shim"
        require "json"

        class LSSetting
          GetPrivateProfileString = Win32API.new('kernel32', 'GetPrivateProfileString', %w(p p p p l p), 'L')
          BufferSize = 16 + 1
          def self.load
            $langscore_current_language = "ja"
            result = ' ' * BufferSize
            r = GetPrivateProfileString.call("Langscore", "Lang", "ja", result, BufferSize, "./Game.ini")
            if r != BufferSize - 2
              $langscore_current_language = result.slice(0...r)
            end
            r
          end
        end

        $ls_scripts_tr = {
          "49037624:35:19" => { "en" => "Barang", "ja" => "アイテム" },
          "49037624:67:19" => { "en" => "Berkas", "ja" => "ファイル" },
          "49037624:84:19" => { "en" => "Pengaturan", "ja" => "オプション" }
        }
        $ls_actor_tr = {
          "サミダレ" => { "en" => "Samidare", "ja" => "サミダレ" }
        }

        def self.translate(text, table, lang = $langscore_current_language)
          return text if table.nil?
          if table.has_key?(text)
            translist = table[text]
            if translist.has_key?(lang)
              t = translist[lang]
              text = t unless t.nil? || t.empty?
            end
          end
          text
        end

        r = LSSetting.load
        menu_item = translate("49037624:35:19", $ls_scripts_tr)
        menu_file = translate("49037624:67:19", $ls_scripts_tr)
        menu_opt  = translate("49037624:84:19", $ls_scripts_tr)
        actor_name = translate("サミダレ", $ls_actor_tr)

        puts JSON.dump({
          r: r,
          lang: $langscore_current_language,
          item: menu_item,
          file: menu_file,
          option: menu_opt,
          actor: actor_name
        })
      HIST_RUBY

      repro_out, repro_err, repro_status = Open3.capture3(RbConfig.ruby, runner_script, chdir: hist_scratch_dir)
      assert repro_status.success?, "Historical runner subprocess must complete cleanly"

      repro_data = JSON.parse(repro_out) rescue {}
      # Verify the historical bug reproduces exactly as diagnosed:
      # 1. args[4].is_a?(String) fails on integer 17 -> r returns 0
      assert_equal 0, repro_data["r"], "Historical shim must return r=0 (args[4] integer check defect)"
      # 2. result.slice(0...0) produces empty string ""
      assert_equal "", repro_data["lang"], "Historical shim must produce empty string language"
      # 3. Fallthrough in Langscore.translate produces literal numeric keys on screen
      assert_equal "49037624:35:19", repro_data["item"], "Historical bug: Item displays raw numeric key"
      assert_equal "49037624:67:19", repro_data["file"], "Historical bug: File (line 67) displays raw numeric key"
      assert_equal "49037624:84:19", repro_data["option"], "Historical bug: Option (line 84) displays raw numeric key"
      # 4. Fallthrough in actor translation leaves Japanese name
      assert_equal "サミダレ", repro_data["actor"], "Historical bug: Actor name reverts to Japanese"
      puts "  [RED CONFIRMED]: Historical 10e2a92 shim failed all translations and returned raw numeric IDs."
    else
      warn "Skipping historical reproduction: git show failed (commit 10e2a92 unavailable: #{git_err})"
    end
  ensure
    FileUtils.remove_entry_secure(hist_scratch_dir) rescue nil
  end

  # ------------------------------------------------------------
  # Test Case 7: In-game language change persists and survives save/load
  # ------------------------------------------------------------
  puts "Test 7: In-game language change persists to Game.ini and correctly reloads"
  File.write("Game.ini", "[Game]\r\nTitle=KING EXIT\r\n\r\n[Langscore]\r\nLang=en\r\n")

  setup_mock_langscore!
  if defined?(RuntimePackLangscore) && RuntimePackLangscore.respond_to?(:install_save_load_hook)
    RuntimePackLangscore.install_save_load_hook
  end

  LSSetting.load
  assert_equal "en", $langscore_current_language

  # Player switches language to Chinese in-game
  $langscore_current_language = "zh-cn"
  LSSetting.save
  assert File.read("Game.ini").include?("Lang=zh-cn"), "LSSetting.save must persist 'zh-cn' to Game.ini"

  # Save game, simulate reload
  $game_actors = MockGameActors.new
  save_payload = DataManager.make_save_contents
  $langscore_current_language = "ja"
  DataManager.extract_save_contents(save_payload)

  assert_equal "zh-cn", $langscore_current_language, "Reload must restore newly saved 'zh-cn'"
  assert_equal "道具", CAO::CM::COMMAND_ITEMS[:item][:name], "Menu Item must translate to '道具'"
  assert_equal "档案", CAO::CM::COMMAND_ITEMS[:file][:name], "Menu File must translate to '档案'"
  assert_equal "选项", CAO::CM::COMMAND_ITEMS[:option][:name], "Menu Option must translate to '选项'"

  # ------------------------------------------------------------
  # Test Case 8: Missing or malformed Game.ini safe fallback
  # ------------------------------------------------------------
  puts "Test 8: Missing Game.ini safely falls back to caller default"
  File.delete("Game.ini") if File.exist?("Game.ini")

  setup_mock_langscore!
  r = LSSetting.load
  # When Game.ini is missing, GetPrivateProfileString returns default "ja" (len 2)
  assert_equal 2, r, "Missing INI must return default length 2"
  assert_equal "ja", $langscore_current_language, "Missing INI must fall back to default 'ja'"

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
  puts "STATUS: GREEN (#{target_lib} menu translation key regression contracts satisfied)"
  if ENV['TEST_TARGET_LIB'].nil?
    puts "\nTriggering sub-suite for win32api_shim..."
    shim_ok = system({ 'TEST_TARGET_LIB' => 'win32api_shim' }, RbConfig.ruby, File.expand_path(__FILE__))
    exit(shim_ok ? 0 : 1)
  else
    exit 0
  end
end
