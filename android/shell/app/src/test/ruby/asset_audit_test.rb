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

# Define mock Cache so runtimepack_bootstrap can hook Cache.normal_bitmap
module MockCache
  class << self
    def normal_bitmap(path)
      Bitmap.new(path)
    end
    def tileset(name)
      normal_bitmap("Graphics/Tilesets/" + name)
    end
  end
end
Cache = MockCache unless defined?(Cache)

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
missing_path = "Graphics/Pictures/NonExistentPicture.png"
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
assert_equal(32, dummy_bmp.width, "Dummy bitmap width must be 32")
assert_equal(32, dummy_bmp.height, "Dummy bitmap height must be 32")

events = RuntimePackAssetAudit.events
assert(events.size >= 1, "At least one audit event should be recorded")
last_ev = events.last
assert_equal("FILE_EXISTS", last_ev["stage"], "Stage should be FILE_EXISTS")
assert_equal("Graphics/Pictures/NonExistentPicture.png", last_ev["asset_path"], "Asset path should match requested")
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
  assert_equal(32, cached_bmp.width, "Cached dummy bitmap width must be 32")
  assert_equal(32, cached_bmp.height, "Cached dummy bitmap height must be 32")
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

# Test 7: Font probing
puts "Test 7: Font probing"
class MockFont
  class << self
    attr_accessor :existing_fonts
    def default_name
      "VL Gothic"
    end
    def exist?(name)
      @existing_fonts ||= []
      @existing_fonts.include?(name)
    end
  end
end

Object.const_set(:Font, MockFont) unless defined?(Font)
MockFont.existing_fonts = []

RuntimePackAssetAudit.probe_fonts!
font_events = RuntimePackAssetAudit.events.select { |e| e["fallback_action"] == "FONT_FALLBACK_LIBERATION_SANS" }
assert(font_events.size >= 1, "Font probe should record fallback event when fonts are missing")
font_ev = font_events.last
assert_equal("FILE_EXISTS", font_ev["stage"], "Font probe stage must be FILE_EXISTS")
assert_equal("FAIL", font_ev["outcome"], "Font probe outcome must be FAIL")
assert_equal("FONT_FALLBACK_LIBERATION_SANS", font_ev["fallback_action"])
assert_equal("Font.exist?", font_ev["source_location"])

# Now test when fonts exist
MockFont.existing_fonts = ["VL Gothic", "UmePlus Gothic"]
count_before_found = RuntimePackAssetAudit.events.size
RuntimePackAssetAudit.probe_fonts!
count_after_found = RuntimePackAssetAudit.events.size
assert_equal(count_before_found, count_after_found, "Font probe should not record events when fonts exist")

# Test 8: Missing tileset in Cache.normal_bitmap emits TILESET_FALLBACK_DUMMY and 512x384 checkered bitmap
puts "Test 8: Cache.normal_bitmap missing tileset fallback"
tileset_path = "Graphics/Tilesets/Missing_Tileset_A2.png"
tileset_bmp = Cache.normal_bitmap(tileset_path)
assert(!tileset_bmp.nil?, "Cache.normal_bitmap should return tileset fallback bitmap")
assert_equal(512, tileset_bmp.width, "Tileset fallback bitmap width must be 512")
assert_equal(384, tileset_bmp.height, "Tileset fallback bitmap height must be 384")
ev = RuntimePackAssetAudit.events.last
assert_equal("FILE_EXISTS", ev["stage"], "Tileset fallback stage must be FILE_EXISTS")
assert_equal("FAIL", ev["outcome"], "Tileset fallback outcome must be FAIL")
assert_equal("TILESET_FALLBACK_DUMMY", ev["fallback_action"], "Tileset fallback action must be TILESET_FALLBACK_DUMMY")
assert_equal("RTP tileset missing; Layer 0/1 map tiles will render transparent/black", ev["error_message"])
assert_equal(tileset_path, ev["asset_path"])

# Also test Cache.tileset
ts_bmp = Cache.tileset("Another_Missing_Tileset.png")
assert(!ts_bmp.nil?, "Cache.tileset should return tileset fallback bitmap")
assert_equal(512, ts_bmp.width, "Cache.tileset fallback bitmap width must be 512")
assert_equal(384, ts_bmp.height, "Cache.tileset fallback bitmap height must be 384")
ev2 = RuntimePackAssetAudit.events.last
assert_equal("TILESET_FALLBACK_DUMMY", ev2["fallback_action"])
assert_equal("RTP tileset missing; Layer 0/1 map tiles will render transparent/black", ev2["error_message"])

# Test 9: run_stress_test emits FAIL: lines for missing assets and broken transfers
puts "Test 9: run_stress_test emits FAIL: progress lines"
MockEventCmd = Struct.new(:code, :parameters) unless defined?(MockEventCmd)
MockEventPage = Struct.new(:list) unless defined?(MockEventPage)
MockEvent = Struct.new(:pages) unless defined?(MockEvent)
MockRPGMap = Struct.new(:events, :autoplay_bgm, :bgm, :autoplay_bgs, :bgs) unless defined?(MockRPGMap)
MockAudioFile = Struct.new(:name) unless defined?(MockAudioFile)

tmp_test_dir = Dir.mktmpdir("rp_asset_audit_stress_")
orig_pwd = Dir.pwd
begin
  Dir.chdir(tmp_test_dir)
  FileUtils.mkdir_p("Data")
  FileUtils.mkdir_p("Graphics/Pictures")

  File.write("Graphics/Pictures/pic_present.png", "dummy")

  commands = [
    MockEventCmd.new(201, [0, 999, 0, 0]),
    MockEventCmd.new(231, [1, "pic_present"]),
    MockEventCmd.new(231, [2, "pic_missing_one"]),
    MockEventCmd.new(241, [MockAudioFile.new("bgm_missing_one")])
  ]
  event = MockEvent.new([MockEventPage.new(commands)])
  map001 = MockRPGMap.new({ 1 => event }, false, nil, false, nil)
  File.open("Data/Map001.rvdata2", "wb") { |f| Marshal.dump(map001, f) }

  report = RuntimePackCheatRunner.run_stress_test

  assert(File.exist?("rp_audit_progress.txt"), "rp_audit_progress.txt must exist")
  progress_lines = File.read("rp_audit_progress.txt").lines.map(&:strip)

  fail_lines = progress_lines.select { |l| l.start_with?("FAIL:") }
  assert(fail_lines.any? { |l| l.include?("Missing Picture: Graphics/Pictures/pic_missing_one") }, "Should emit FAIL for missing picture")
  assert(fail_lines.any? { |l| l.include?("Missing BGM: Audio/BGM/bgm_missing_one") }, "Should emit FAIL for missing BGM")
  assert(fail_lines.any? { |l| l.include?("Broken transfer to Map 999") }, "Should emit FAIL for broken transfer")

  done_line = progress_lines.find { |l| l.start_with?("DONE:") }
  assert(done_line, "DONE: line must be present")
  assert(done_line.include?("Stress test completed: 1 maps,"), "DONE line must match completed stats format")
  assert(done_line.include?("2 missing."), "DONE line must show 2 missing assets")
ensure
  Dir.chdir(orig_pwd)
  FileUtils.remove_entry_secure(tmp_test_dir) rescue nil
end

# Test 10: Smart tileset and character fallback generation
puts "Test 10: Smart tileset and character fallback generation"

# Verify create_tileset_fallback produces 512x384 bitmap
ts_fallback = (defined?(create_tileset_fallback) ? create_tileset_fallback : RuntimePackCompatibility.create_tileset_fallback)
assert(!ts_fallback.nil?, "create_tileset_fallback must return a bitmap")
assert_equal(512, ts_fallback.width, "create_tileset_fallback must produce 512 width")
assert_equal(384, ts_fallback.height, "create_tileset_fallback must produce 384 height")

# Verify create_character_fallback produces valid multi-frame bitmap (> 32x32) for !Flame and !Chest
flame_fallback = (defined?(create_character_fallback) ? create_character_fallback("!Flame") : RuntimePackCompatibility.create_character_fallback("!Flame"))
assert(!flame_fallback.nil?, "create_character_fallback(!Flame) must return a bitmap")
assert(flame_fallback.width > 32, "create_character_fallback(!Flame) width must be > 32")
assert(flame_fallback.height > 32, "create_character_fallback(!Flame) height must be > 32")

chest_fallback = (defined?(create_character_fallback) ? create_character_fallback("!Chest") : RuntimePackCompatibility.create_character_fallback("!Chest"))
assert(!chest_fallback.nil?, "create_character_fallback(!Chest) must return a bitmap")
assert(chest_fallback.width > 32, "create_character_fallback(!Chest) width must be > 32")
assert(chest_fallback.height > 32, "create_character_fallback(!Chest) height must be > 32")

# Verify standard character fallback without '!'
actor_fallback = (defined?(create_character_fallback) ? create_character_fallback("Actor1") : RuntimePackCompatibility.create_character_fallback("Actor1"))
assert(!actor_fallback.nil?, "create_character_fallback(Actor1) must return a bitmap")
assert(actor_fallback.width > 32, "create_character_fallback(Actor1) width must be > 32")
assert(actor_fallback.height > 32, "create_character_fallback(Actor1) height must be > 32")

# Verify Bitmap.new on character path uses smart character fallback (> 32x32)
char_missing = Bitmap.new("Graphics/Characters/!Flame.png")
assert(!char_missing.nil?, "Bitmap.new on missing character must return fallback bitmap")
assert(char_missing.width > 32, "Bitmap.new on missing character width must be > 32")
assert(char_missing.height > 32, "Bitmap.new on missing character height must be > 32")

# Verify Cache.normal_bitmap on character path uses smart character fallback (> 32x32)
if defined?(Cache) && Cache.respond_to?(:normal_bitmap)
  cache_char = Cache.normal_bitmap("Graphics/Characters/!Chest.png")
  assert(!cache_char.nil?, "Cache.normal_bitmap on missing character must return fallback bitmap")
  assert(cache_char.width > 32, "Cache.normal_bitmap on missing character width must be > 32")
  assert(cache_char.height > 32, "Cache.normal_bitmap on missing character height must be > 32")
end

# Test 11: Subtractive fog Plane mitigation in OpenGL ES
puts "Test 11: Subtractive fog Plane mitigation in OpenGL ES"
RuntimePackCompatibility.reset_plane_mitigations! if defined?(RuntimePackCompatibility) && RuntimePackCompatibility.respond_to?(:reset_plane_mitigations!)

assert(defined?(Plane), "Plane class must be defined")

if defined?(Plane)
  # 1. Plane with blend_type = 2 and z = 10 clamps opacity to <= 80 and logs mitigation notice
  plane1 = Plane.new
  plane1.z = 10
  plane1.opacity = 255
  plane1.blend_type = 2

  assert(plane1.opacity <= 80, "Plane#blend_type = 2 with z = 10 must clamp opacity to <= 80 (was #{plane1.opacity})")
  assert(plane1.opacity > 0, "Plane opacity must be positive")
  assert_equal(2, plane1.blend_type, "Plane blend_type should remain 2")

  # Verify mitigation notice is logged / recorded
  if defined?(RuntimePackCompatibility) && RuntimePackCompatibility.respond_to?(:plane_mitigations)
    mitigations = RuntimePackCompatibility.plane_mitigations
    assert(mitigations.size >= 1, "At least one Plane mitigation should be recorded")
    last_mit = mitigations.last
    assert_equal(10, last_mit[:z], "Mitigation notice Z should be 10")
    assert(last_mit[:opacity] <= 80, "Mitigation notice opacity should be <= 80")
    assert(last_mit[:message].include?("Subtractive fog Plane mitigation"), "Mitigation message should describe fog mitigation")
  end

  # 2. Intercept Plane#opacity= when blend_type == 2 and z < 32
  plane1.opacity = 200
  assert(plane1.opacity <= 80, "Plane#opacity= must clamp opacity to <= 80 when blend_type is 2 and z < 32")

  # 3. Allow higher opacity when z >= 32 (above character layer)
  plane1.z = 50
  plane1.opacity = 180
  assert_equal(180, plane1.opacity, "Plane#opacity= should allow higher opacity when z >= 32")

  # 4. Lowering z below 32 while blend_type == 2 re-clamps opacity
  plane1.z = 15
  assert(plane1.opacity <= 80, "Plane#z= must clamp opacity to <= 80 when z is lowered below 32 while blend_type == 2")

  # 5. Restoring normal blend_type (0) restores unconstrained opacity
  plane1.blend_type = 0
  plane1.opacity = 250
  assert_equal(250, plane1.opacity, "Plane#opacity= should allow full opacity when blend_type is normal (0)")

  # 6. Verify clamp when floor tilesets are NOT missing (cap to exactly 80)
  saved_events = RuntimePackAssetAudit.events.dup rescue []
  RuntimePackAssetAudit.reset! rescue nil
  begin
    plane2 = Plane.new
    plane2.z = 10
    plane2.opacity = 255
    plane2.blend_type = 2
    assert_equal(80, plane2.opacity, "Plane#blend_type = 2 with z = 10 must clamp opacity to exactly 80 when floor is not missing")
  ensure
    if defined?(RuntimePackAssetAudit)
      saved_events.each do |ev|
        RuntimePackAssetAudit.record_event(
          ev["stage"],
          ev["asset_path"],
          ev["outcome"],
          fallback_action: ev["fallback_action"],
          error_message: ev["error_message"],
          source_location: ev["source_location"]
        ) rescue nil
      end
    end
  end
end

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
