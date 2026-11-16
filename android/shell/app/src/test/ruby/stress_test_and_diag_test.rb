# frozen_string_literal: true

require 'json'
require 'fileutils'
require 'tmpdir'
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

CMD_FILE = RuntimePackCheatRunner::CMD_FILE
EVAL_FILE = RuntimePackCheatRunner::EVAL_FILE
ERROR_FILE = RuntimePackCheatRunner::ERROR_FILE

def cleanup_files
  [CMD_FILE, EVAL_FILE, ERROR_FILE, "bug_report.json", "audit_report.json"].each do |f|
    File.delete(f) if File.exist?(f)
  end
end

# Mock Classes for Testing
class MockPlayer
  attr_accessor :x, :y, :transfers
  def initialize(x = 10, y = 20)
    @x = x
    @y = y
    @transfers = []
  end
  def reserve_transfer(map_id, x, y)
    @transfers << [map_id, x, y]
    @x = x
    @y = y
  end
end

class MockMap
  attr_accessor :map_id
  def initialize(id = 1)
    @map_id = id
  end
end

class MockParty
  attr_accessor :gold
  def initialize(gold = 1000)
    @gold = gold
  end
end

class Scene_Map
end

class MockSceneManager
  class << self
    attr_accessor :scene
    def goto(scene_class)
      @scene = scene_class.is_a?(Class) ? scene_class.new : scene_class
    end
  end
end

# Mock Event Command
MockEventCmd = Struct.new(:code, :parameters)
MockEventPage = Struct.new(:list)
MockEvent = Struct.new(:pages)
MockRPGMap = Struct.new(:events, :autoplay_bgm, :bgm, :autoplay_bgs, :bgs)
MockAudioFile = Struct.new(:name)
MockActorData = Struct.new(:character_name, :face_name, :battler_name)

begin
  cleanup_files
  puts "Running stress_test_and_diag_test.rb..."

  # Setup Globals
  $game_player = MockPlayer.new(15, 25)
  $game_map = MockMap.new(3)
  $game_party = MockParty.new(7777)
  SceneManager = MockSceneManager unless defined?(SceneManager)
  SceneManager.scene = Scene_Map.new

  # --------------------------------------------------------------------------
  # 1. Test teleport_map action
  # --------------------------------------------------------------------------
  puts "Test 1: teleport_map action"
  File.write(CMD_FILE, JSON.generate({ "action" => "teleport_map", "map_id" => 42, "x" => 8, "y" => 16 }))
  RuntimePackCheatRunner.update

  assert_equal [[42, 8, 16]], $game_player.transfers, "Player reserve_transfer should record [42, 8, 16]"
  assert_equal 8, $game_player.x, "Player x should be updated to 8"
  assert_equal 16, $game_player.y, "Player y should be updated to 16"
  assert File.exist?(EVAL_FILE), "EVAL_FILE should exist after teleport_map"
  eval_text = File.read(EVAL_FILE)
  assert eval_text.include?("Teleported to Map 42"), "EVAL_FILE should confirm teleportation"

  # --------------------------------------------------------------------------
  # 2. Test export_bug_report action
  # --------------------------------------------------------------------------
  puts "Test 2: export_bug_report action"
  File.write(CMD_FILE, JSON.generate({ "action" => "export_bug_report" }))
  RuntimePackCheatRunner.update

  assert File.exist?("bug_report.json"), "bug_report.json must be written to disk"
  bug_data = JSON.parse(File.read("bug_report.json"))
  assert_equal "Scene_Map", bug_data["scene"], "Scene name should be Scene_Map"
  assert_equal 3, bug_data["map_id"], "Map ID should be 3"
  assert_equal 8, bug_data["player"]["x"], "Player X coordinate should match"
  assert_equal 16, bug_data["player"]["y"], "Player Y coordinate should match"
  assert_equal 7777, bug_data["gold"], "Party gold should match"
  assert bug_data["ruby_version"], "Ruby version should be present"

  # --------------------------------------------------------------------------
  # 3. Test run_stress_test serialization and disk asset inspection
  # --------------------------------------------------------------------------
  puts "Test 3: run_stress_test serialization and disk asset inspection"
  tmp_test_dir = Dir.mktmpdir("rp_stress_test_")
  orig_pwd = Dir.pwd

  begin
    Dir.chdir(tmp_test_dir)
    FileUtils.mkdir_p("Data")
    FileUtils.mkdir_p("Graphics/Pictures")
    FileUtils.mkdir_p("Graphics/Characters")
    FileUtils.mkdir_p("Audio/BGM")

    # Create dummy assets on disk
    File.write("Graphics/Pictures/pic_exact.png", "dummy picture data")
    File.write("Graphics/Pictures/PIC_UPPERCASE.PNG", "dummy picture data")
    File.write("Graphics/Characters/hero_char.png", "dummy character data")

    # Create Mock Map with:
    # - Code 201: valid transfer to Map002
    # - Code 201: broken transfer to Map999 (does not exist)
    # - Code 231: pic_exact (EXACT)
    # - Code 231: pic_uppercase (CASE_MISMATCH against PIC_UPPERCASE.PNG)
    # - Code 231: pic_missing (MISSING)
    # - Code 241: bgm_missing (MISSING)
    # - Code 322: hero_char (EXACT)
    commands = [
      MockEventCmd.new(201, [0, 2, 5, 5]),
      MockEventCmd.new(201, [0, 999, 0, 0]),
      MockEventCmd.new(231, [1, "pic_exact"]),
      MockEventCmd.new(231, [2, "pic_uppercase"]),
      MockEventCmd.new(231, [3, "pic_missing"]),
      MockEventCmd.new(241, [MockAudioFile.new("bgm_missing")]),
      MockEventCmd.new(322, [1, "hero_char", 0, "", 0])
    ]

    event = MockEvent.new([MockEventPage.new(commands)])
    map001 = MockRPGMap.new({ 1 => event }, false, nil, false, nil)
    map002 = MockRPGMap.new({}, false, nil, false, nil)

    File.open("Data/Map001.rvdata2", "wb") { |f| Marshal.dump(map001, f) }
    File.open("Data/Map002.rvdata2", "wb") { |f| Marshal.dump(map002, f) }

    # Database actors
    actor = MockActorData.new("hero_char", "", "")
    File.open("Data/Actors.rvdata2", "wb") { |f| Marshal.dump([nil, actor], f) }

    # Run stress test
    report = RuntimePackCheatRunner.run_stress_test

    assert File.exist?("audit_report.json"), "audit_report.json must be written"
    audit_json = JSON.parse(File.read("audit_report.json"))

    assert_equal 2, audit_json["stats"]["total_maps_scanned"], "Should have scanned 2 maps"
    assert audit_json["stats"]["exact_count"] >= 2, "Exact count should be at least 2 (pic_exact, hero_char)"
    assert audit_json["stats"]["missing_count"] >= 2, "Missing count should include pic_missing, bgm_missing"
    assert_equal 1, audit_json["stats"]["broken_transfers_count"], "Broken transfer count should be 1 (Map 999)"

    broken = audit_json["broken_transfers"].find { |t| t["target_map_id"] == 999 }
    assert broken, "Broken transfer to Map 999 must be recorded in report"

    missing_names = audit_json["missing_assets"].map { |a| a["referenced"] }
    assert missing_names.any? { |n| n.include?("pic_missing") }, "pic_missing must be in missing_assets"
    assert missing_names.any? { |n| n.include?("bgm_missing") }, "bgm_missing must be in missing_assets"

    puts "Audit report serialization verified successfully."
  ensure
    Dir.chdir(orig_pwd)
    FileUtils.remove_entry_secure(tmp_test_dir) rescue nil
  end

  # --------------------------------------------------------------------------
  # 4. Test parse_json_fallback directly
  # --------------------------------------------------------------------------
  puts "Test 4: parse_json_fallback without standard JSON library"
  cmd_single = '{"action":"run_stress_test"}'
  parsed_single = RuntimePackCheatRunner.parse_json_fallback(cmd_single)
  assert_equal [{"action" => "run_stress_test"}], parsed_single, "Single object should parse into array of 1 hash"

  cmd_params = '{"action":"teleport_map","map_id":77,"x":12,"y":34}'
  parsed_params = RuntimePackCheatRunner.parse_json_fallback(cmd_params)
  assert_equal [{"action" => "teleport_map", "map_id" => 77, "x" => 12, "y" => 34}], parsed_params, "Parameters should parse with integers"

  cmd_arr = '[{"action":"set_variable","id":5,"value":100},{"action":"run_stress_test"}]'
  parsed_arr = RuntimePackCheatRunner.parse_json_fallback(cmd_arr)
  assert_equal 2, parsed_arr.size, "Array of commands should parse 2 items"
  assert_equal "set_variable", parsed_arr[0]["action"]
  assert_equal 5, parsed_arr[0]["id"]
  assert_equal 100, parsed_arr[0]["value"]
  assert_equal "run_stress_test", parsed_arr[1]["action"]

ensure
  cleanup_files
end

puts "\n=================================================="
puts "Tests run: #{$tests_run}, Passed: #{$tests_passed}, Failed: #{$tests_failed}"
puts "=================================================="

if $tests_failed > 0
  exit 1
else
  puts "All stress test and diagnostic tests passed successfully!"
  exit 0
end
