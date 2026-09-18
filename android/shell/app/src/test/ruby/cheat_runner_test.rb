# frozen_string_literal: true

require 'json'
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

CMD_FILE = RuntimePackCheatRunner::CMD_FILE
STATE_FILE = RuntimePackCheatRunner::STATE_FILE
EVAL_FILE = RuntimePackCheatRunner::EVAL_FILE
ERROR_FILE = RuntimePackCheatRunner::ERROR_FILE

def cleanup_cheat_files
  [CMD_FILE, STATE_FILE, EVAL_FILE, ERROR_FILE].each do |file|
    File.delete(file) if File.exist?(file)
  end
end

# Mock Classes for RGSS
class MockVariables
  def initialize
    @data = {}
  end
  def [](id)
    @data[id] || 0
  end
  def []=(id, val)
    @data[id] = val
  end
end

class MockSwitches
  def initialize
    @data = {}
  end
  def [](id)
    @data[id] || false
  end
  def []=(id, val)
    @data[id] = val
  end
end

class MockActor
  attr_accessor :hp, :max_hp, :mp, :max_mp, :recovered
  def initialize(max_hp = 100, max_mp = 50)
    @max_hp = max_hp
    @max_mp = max_mp
    @hp = 1
    @mp = 0
    @recovered = false
  end
  def recover_all
    @hp = @max_hp
    @mp = @max_mp
    @recovered = true
  end
end

class MockParty
  attr_accessor :gold, :items, :members
  def initialize
    @gold = 0
    @items = Hash.new(0)
    @members = [MockActor.new, MockActor.new]
  end
  def gain_gold(amount)
    @gold += amount
  end
  def all_members
    @members
  end
  def gain_item(item, amount)
    return unless item
    @items[item.id] += amount
  end
end

MockItem = Struct.new(:id, :name)

begin
  cleanup_cheat_files

  puts "Running cheat_runner_test.rb..."

  # Setup Mocks
  $game_variables = MockVariables.new
  $game_switches = MockSwitches.new
  $game_party = MockParty.new
  $data_items = { 1 => MockItem.new(1, "Potion"), 2 => MockItem.new(2, "Elixir") }
  $data_weapons = { 10 => MockItem.new(10, "Bronze Sword") }
  $data_armors = { 20 => MockItem.new(20, "Leather Shield") }

  # 1. Test set_variable
  puts "Test 1: set_variable"
  File.write(CMD_FILE, JSON.generate({ "action" => "set_variable", "id" => 15, "value" => 42 }))
  RuntimePackCheatRunner.update
  assert_equal 42, $game_variables[15], "Variable 15 should be set to 42"
  assert !File.exist?(CMD_FILE), "CMD_FILE should be removed after processing"

  # 2. Test set_switch
  puts "Test 2: set_switch"
  File.write(CMD_FILE, JSON.generate({ "action" => "set_switch", "id" => 7, "value" => true }))
  RuntimePackCheatRunner.update
  assert_equal true, $game_switches[7], "Switch 7 should be true"

  File.write(CMD_FILE, JSON.generate({ "action" => "set_switch", "id" => 7, "value" => false }))
  RuntimePackCheatRunner.update
  assert_equal false, $game_switches[7], "Switch 7 should be toggled false"

  # 3. Test gain_gold
  puts "Test 3: gain_gold"
  File.write(CMD_FILE, JSON.generate({ "action" => "gain_gold", "amount" => 5000 }))
  RuntimePackCheatRunner.update
  assert_equal 5000, $game_party.gold, "Party gold should be 5000"

  # 4. Test recover_all
  puts "Test 4: recover_all"
  $game_party.members.each { |m| m.hp = 1; m.recovered = false }
  File.write(CMD_FILE, JSON.generate({ "action" => "recover_all" }))
  RuntimePackCheatRunner.update
  assert $game_party.members.all? { |m| m.recovered && m.hp == m.max_hp }, "All members should be recovered"

  # 5. Test gain_item (items, weapons, armors)
  puts "Test 5: gain_item"
  File.write(CMD_FILE, JSON.generate({ "action" => "gain_item", "type" => "item", "id" => 1, "amount" => 5 }))
  RuntimePackCheatRunner.update
  assert_equal 5, $game_party.items[1], "Party should have 5 Potions"

  File.write(CMD_FILE, JSON.generate({ "action" => "gain_item", "type" => "weapon", "id" => 10, "amount" => 1 }))
  RuntimePackCheatRunner.update
  assert_equal 1, $game_party.items[10], "Party should have 1 Bronze Sword"

  File.write(CMD_FILE, JSON.generate({ "action" => "gain_item", "type" => "armor", "id" => 20, "amount" => 2 }))
  RuntimePackCheatRunner.update
  assert_equal 2, $game_party.items[20], "Party should have 2 Leather Shields"

  # 6. Test dump_state
  puts "Test 6: dump_state"
  $game_party.gold = 12345
  $game_variables[1] = 99
  $game_variables[2] = 0 # should not be dumped
  $game_switches[3] = true
  $game_switches[4] = false # should not be dumped

  File.write(CMD_FILE, JSON.generate({ "action" => "dump_state" }))
  RuntimePackCheatRunner.update
  assert File.exist?(STATE_FILE), "STATE_FILE should exist after dump_state"

  state_data = JSON.parse(File.read(STATE_FILE))
  assert_equal 12345, state_data["gold"], "Dumped gold should match"
  assert_equal 99, state_data["variables"]["1"], "Dumped variable 1 should match"
  assert_equal nil, state_data["variables"]["2"], "Zero variable should not be in dump"
  assert_equal true, state_data["switches"]["3"], "Dumped switch 3 should be true"
  assert_equal nil, state_data["switches"]["4"], "False switch should not be in dump"

  # 7. Test eval
  puts "Test 7: eval"
  File.write(CMD_FILE, JSON.generate({ "action" => "eval", "code" => "$eval_test_result = 7 * 6; $eval_test_result" }))
  RuntimePackCheatRunner.update
  assert_equal 42, $eval_test_result, "Eval should execute Ruby code in TOPLEVEL_BINDING"
  assert File.exist?(EVAL_FILE), "EVAL_FILE should exist"
  eval_res = File.read(EVAL_FILE)
  assert_equal "42", eval_res.strip, "EVAL_FILE content should be '42'"

  # 8. Test batch commands
  puts "Test 8: batch commands"
  batch = [
    { "action" => "set_variable", "id" => 50, "value" => 100 },
    { "action" => "set_variable", "id" => 51, "value" => 200 },
    { "action" => "gain_gold", "amount" => 1000 }
  ]
  File.write(CMD_FILE, JSON.generate(batch))
  RuntimePackCheatRunner.update
  assert_equal 100, $game_variables[50], "Batch variable 50 should be 100"
  assert_equal 200, $game_variables[51], "Batch variable 51 should be 200"
  assert_equal 13345, $game_party.gold, "Batch gold should increase by 1000"

  # 9. Test Graphics.update integration hook
  puts "Test 9: Graphics.update hook"
  module Graphics
    @updated = false
    class << self
      attr_accessor :updated
      def update
        @updated = true
      end
    end
  end

  RuntimePackCheatRunner.hook_graphics

  File.write(CMD_FILE, JSON.generate({ "action" => "set_variable", "id" => 99, "value" => 888 }))
  Graphics.update
  assert Graphics.updated, "Graphics original update should be called"
  assert_equal 888, $game_variables[99], "Graphics.update should trigger RuntimePackCheatRunner.update"

  # 10. Test eval error handling [CHT-002]
  puts "Test 10: eval error handling"
  File.write(CMD_FILE, JSON.generate({ "action" => "eval", "code" => "raise 'Intentional error'" }))
  RuntimePackCheatRunner.update
  assert File.exist?(EVAL_FILE), "EVAL_FILE should record error"
  eval_err = File.read(EVAL_FILE)
  assert eval_err.include?("[CHT-002]"), "Eval error should include [CHT-002] error code"

ensure
  cleanup_cheat_files
end

puts "\n=================================================="
puts "Tests run: #{$tests_run}, Passed: #{$tests_passed}, Failed: #{$tests_failed}"
puts "=================================================="

if $tests_failed > 0
  exit 1
else
  puts "All cheat runner tests passed successfully!"
  exit 0
end
