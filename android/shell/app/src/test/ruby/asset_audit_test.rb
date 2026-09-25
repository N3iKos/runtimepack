# frozen_string_literal: true

require 'json'
require 'fileutils'
require 'tmpdir'

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

puts "Running RGSS Asset Audit Unit Tests..."

# Require bootstrap (which should initialize asset audit module and hooks)
require_relative '../../main/assets/runtimepack_bootstrap'

# Cleanup any previous audit files
[RuntimePackAssetAudit::EVENTS_FILE, "game/#{RuntimePackAssetAudit::EVENTS_FILE}"].each do |f|
  File.delete(f) if File.exist?(f)
end
RuntimePackAssetAudit.reset! if defined?(RuntimePackAssetAudit.reset!)

# Test 1: Module defined and initial state
puts "Test 1: RuntimePackAssetAudit module presence"
assert(defined?(RuntimePackAssetAudit), "RuntimePackAssetAudit module must be defined")
assert_equal(0, RuntimePackAssetAudit.events.size, "Initial events list should be empty")

# Test 2: Bitmap.new failure emits PLACEHOLDER_SUBSTITUTED fallback event and returns dummy bitmap
puts "Test 2: Bitmap.new failure fallback"
missing_path = "Graphics/Characters/NonExistentHero.png"
dummy_bmp = nil
assert_nothing_raised = begin
  dummy_bmp = Bitmap.new(missing_path)
  true
rescue => e
  warn "Bitmap.new raised: #{e}"
  false
end

assert(assert_nothing_raised, "Bitmap.new on missing path should not raise unhandled exception")
assert(!dummy_bmp.nil?, "Bitmap.new on missing path should return dummy bitmap")
assert(dummy_bmp.width > 0 && dummy_bmp.height > 0, "Dummy bitmap must have valid dimensions")

events = RuntimePackAssetAudit.events
assert(events.size >= 1, "At least one audit event should be recorded")
last_ev = events.last
assert_equal("FILE_EXISTS", last_ev["stage"], "Stage should be FILE_EXISTS")
assert_equal("Graphics/Characters/NonExistentHero.png", last_ev["asset_path"], "Asset path should match requested")
assert_equal("FAIL", last_ev["outcome"], "Outcome should be FAIL")
assert_equal("PLACEHOLDER_SUBSTITUTED", last_ev["fallback_action"], "Fallback action should be PLACEHOLDER_SUBSTITUTED")
assert(last_ev["error_message"].to_s.include?("Errno::ENOENT") || last_ev["error_message"].to_s.include?("No such file"), "Error message should capture exception")
assert_equal("Bitmap.new", last_ev["source_location"], "Source location should be Bitmap.new")

# Test 3: Cache.normal_bitmap failure emits event and dummy bitmap
puts "Test 3: Cache.normal_bitmap failure fallback"
if defined?(Cache) && Cache.respond_to?(:normal_bitmap)
  cache_path = "Graphics/Pictures/MissingPic.png"
  cached_bmp = Cache.normal_bitmap(cache_path)
  assert(!cached_bmp.nil?, "Cache.normal_bitmap should return dummy bitmap")
  ev = RuntimePackAssetAudit.events.last
  assert_equal("PLACEHOLDER_SUBSTITUTED", ev["fallback_action"], "Fallback action should be PLACEHOLDER_SUBSTITUTED")
  assert_equal(cache_path, ev["asset_path"], "Asset path should match cache request")
  assert_equal("FAIL", ev["outcome"], "Outcome must be FAIL")
end

# Test 4: Audio.bgm_play and se_play failure emits AUDIO_LOAD_SUPPRESSED
puts "Test 4: Audio playback failure fallback"
if defined?(Audio)
  missing_bgm = "Audio/BGM/MissingBgm.ogg"
  assert_nothing_raised = begin
    res = Audio.bgm_play(missing_bgm)
    assert_equal(nil, res, "Audio.bgm_play should return nil on suppressed error")
    true
  rescue => e
    warn "Audio.bgm_play raised: #{e}"
    false
  end
  assert(assert_nothing_raised, "Audio.bgm_play should suppress exception")

  bgm_ev = RuntimePackAssetAudit.events.last
  assert_equal("AUDIO_LOAD_SUPPRESSED", bgm_ev["fallback_action"], "Audio fallback should be AUDIO_LOAD_SUPPRESSED")
  assert_equal("FAIL", bgm_ev["outcome"], "Audio outcome should be FAIL")
  assert_equal(missing_bgm, bgm_ev["asset_path"], "Asset path should match BGM path")

  missing_se = "Audio/SE/MissingSe.ogg"
  Audio.se_play(missing_se)
  se_ev = RuntimePackAssetAudit.events.last
  assert_equal("AUDIO_LOAD_SUPPRESSED", se_ev["fallback_action"])
  assert_equal(missing_se, se_ev["asset_path"])
end

# Test 5: Atomic file persistence
puts "Test 5: Atomic file persistence"
assert(File.exist?(RuntimePackAssetAudit::EVENTS_FILE), "Audit events file must exist on disk")
raw_json = File.read(RuntimePackAssetAudit::EVENTS_FILE)
parsed = JSON.parse(raw_json)
assert(parsed.is_a?(Array), "Events file content must be a JSON array")
assert(parsed.size == RuntimePackAssetAudit.events.size, "Disk file must contain all recorded events")

# Test 6: Monotonic sequence IDs
puts "Test 6: Monotonic sequence IDs"
seqs = parsed.map { |e| e["sequence_id"] }
assert_equal(seqs, seqs.sort, "Sequence IDs must be monotonically increasing")
assert(seqs.uniq.size == seqs.size, "Sequence IDs must be unique")

# Cleanup
[RuntimePackAssetAudit::EVENTS_FILE, "game/#{RuntimePackAssetAudit::EVENTS_FILE}"].each do |f|
  File.delete(f) if File.exist?(f)
end

puts "\n=================================================="
puts "Tests run: #{$tests_run}, Passed: #{$tests_passed}, Failed: #{$tests_failed}"
puts "=================================================="

if $tests_failed > 0
  exit 1
else
  puts "All RGSS Asset Audit tests passed successfully!"
end
