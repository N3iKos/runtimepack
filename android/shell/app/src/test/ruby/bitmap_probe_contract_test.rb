# frozen_string_literal: true
# bitmap_probe_contract_test.rb
# TDD Behavioral Regression Suite for RGSS Low-Level Bitmap & Cache Error Semantics
# Tests actual production hooks from runtimepack_bootstrap.rb and runtimepack_asset_audit.rb

require 'tmpdir'
require 'fileutils'
require 'json'

$tests_run = 0
$tests_passed = 0
$tests_failed = 0

def assert(condition, message = "Assertion failed")
  $tests_run += 1
  if condition
    $tests_passed += 1
    print "."
  else
    $tests_failed += 1
    puts "\nFAILED: #{message}"
    caller_locations(1, 2).each { |loc| puts "  at #{loc}" }
  end
end

def assert_equal(expected, actual, message = nil)
  msg = message || "Expected #{expected.inspect}, got #{actual.inspect}"
  assert(expected == actual, msg)
end

def assert_raises(exception_class, message = "Expected #{exception_class} to be raised")
  $tests_run += 1
  begin
    yield
    $tests_failed += 1
    puts "\nFAILED: #{message} (nothing was raised)"
    caller_locations(1, 2).each { |loc| puts "  at #{loc}" }
  rescue exception_class => e
    $tests_passed += 1
    print "."
    e
  rescue Exception => e
    $tests_failed += 1
    puts "\nFAILED: #{message} (raised #{e.class}: #{e.message} instead of #{exception_class})"
    caller_locations(1, 2).each { |loc| puts "  at #{loc}" }
    e
  end
end

puts "============================================================"
puts "Running RGSS Bitmap Probe & Error Semantics Contract Tests"
puts "============================================================"

# --- 1. Set up Stub Native Environment (RGSS C host representation) ---
$mock_fs = {}

class StubNativeBitmap
  attr_reader :width, :height, :path, :hue

  def initialize(*args)
    if args.size == 1
      raw_path = args[0].to_s
      resolved_key = if $mock_fs
                       if $mock_fs.key?(raw_path)
                         raw_path
                       else
                         [".png", ".PNG", ".jpg", ".bmp"].map { |ext| raw_path + ext }.find { |k| $mock_fs.key?(k) }
                       end
                     end

      if $mock_fs && !resolved_key && !File.exist?(raw_path)
        raise Errno::ENOENT, "No such file or directory - #{raw_path}"
      end
      if $mock_fs && resolved_key
        info = $mock_fs[resolved_key]
        if info[:corrupt]
          raise ArgumentError, "Failed to decode image data: #{raw_path}"
        end
        @width = info[:width] || 640
        @height = info[:height] || 480
      else
        @width = 640
        @height = 480
      end
      @path = raw_path
    elsif args.size >= 2
      @width = args[0]
      @height = args[1]
      @path = nil
    else
      raise ArgumentError, "wrong number of arguments"
    end
    @hue = 0
  end

  def hue_change(h)
    @hue = h
  end

  def dup
    copy = StubNativeBitmap.new(@width, @height)
    copy.instance_variable_set(:@hue, @hue)
    copy.instance_variable_set(:@path, @path)
    copy
  end

  def disposed?
    false
  end
end

Bitmap = StubNativeBitmap unless defined?(Bitmap)

# RGSS3 Cache Baseline Module
module MockRGSS3Cache
  class << self
    def reset_cache!
      @cache = {}
    end

    def load_bitmap(dir_name, filename, hue = 0)
      @cache ||= {}
      if hue == 0
        @cache[dir_name + filename] ||= normal_bitmap(dir_name + filename)
      else
        @cache[dir_name + filename + hue.to_s] ||= (
          b = load_bitmap(dir_name, filename).dup
          b.hue_change(hue)
          b
        )
      end
    end

    def normal_bitmap(path)
      @cache ||= {}
      @cache[path] ||= Bitmap.new(path)
    end

    def animation(filename, hue = 0)
      load_bitmap("Graphics/Animations/", filename, hue)
    end

    def battleback1(filename)
      load_bitmap("Graphics/Battlebacks1/", filename)
    end

    def battleback2(filename)
      load_bitmap("Graphics/Battlebacks2/", filename)
    end

    def battler(filename, hue = 0)
      load_bitmap("Graphics/Battlers/", filename, hue)
    end

    def character(filename)
      load_bitmap("Graphics/Characters/", filename)
    end

    def face(filename)
      load_bitmap("Graphics/Faces/", filename)
    end

    def parallax(filename)
      load_bitmap("Graphics/Parallaxes/", filename)
    end

    def picture(filename)
      load_bitmap("Graphics/Pictures/", filename)
    end

    def system(filename)
      load_bitmap("Graphics/System/", filename)
    end

    def tileset(filename)
      load_bitmap("Graphics/Tilesets/", filename)
    end

    def title1(filename)
      load_bitmap("Graphics/Titles1/", filename)
    end

    def title2(filename)
      load_bitmap("Graphics/Titles2/", filename)
    end
  end
end

Cache = MockRGSS3Cache unless defined?(Cache)

# RGSS1 RPG::Cache Baseline Module
module RPG
  module Cache
    class << self
      def reset_cache!
        @cache = {}
      end

      def load_bitmap(dir_name, filename, hue = 0)
        @cache ||= {}
        path = dir_name + filename
        if !@cache.include?(path) || @cache[path].nil? || @cache[path].disposed?
          if filename.empty?
            @cache[path] = Bitmap.new(32, 32)
          else
            @cache[path] = Bitmap.new(path)
          end
        end
        if hue == 0
          @cache[path]
        else
          key = [path, hue]
          if !@cache.include?(key) || @cache[key].nil? || @cache[key].disposed?
            @cache[key] = @cache[path].dup
            @cache[key].hue_change(hue)
          end
          @cache[key]
        end
      end

      def character(filename, hue = 0)
        load_bitmap("Graphics/Characters/", filename, hue)
      end

      def tileset(filename)
        load_bitmap("Graphics/Tilesets/", filename)
      end

      def parallax(filename)
        load_bitmap("Graphics/Parallaxes/", filename)
      end

      def picture(filename)
        load_bitmap("Graphics/Pictures/", filename)
      end

      def autotile(filename)
        load_bitmap("Graphics/Autotiles/", filename)
      end

      def fog(filename, hue = 0)
        load_bitmap("Graphics/Fogs/", filename, hue)
      end

      def battleback(filename)
        load_bitmap("Graphics/Battlebacks/", filename)
      end

      def windowskin(filename)
        load_bitmap("Graphics/Windowskins/", filename)
      end

      def tile(filename, tile_id, hue = 0)
        load_bitmap("Graphics/Tilesets/", filename, hue)
      end
    end
  end
end unless defined?(RPG) && defined?(RPG::Cache)

# --- 2. Load Production Bootstrap & Asset Audit Code ---
bootstrap_file = File.expand_path("../../main/assets/runtimepack_bootstrap.rb", __dir__)
require bootstrap_file

# Cleanup any previous audit file
[RuntimePackAssetAudit::EVENTS_FILE, "game/#{RuntimePackAssetAudit::EVENTS_FILE}"].each do |f|
  File.delete(f) if File.exist?(f)
end
RuntimePackAssetAudit.reset! if defined?(RuntimePackAssetAudit.reset!)

# Ensure production hooks are installed
RuntimePackCheatRunner.install_save_and_cache_hooks if defined?(RuntimePackCheatRunner)
RuntimePackAssetAudit.install_hooks! if defined?(RuntimePackAssetAudit)

# --- Suite 1: Raw Bitmap.new & Cache.normal_bitmap error semantics ---
puts "\n--- Suite 1: Primitive Error Semantics ---"

# 1.1: Direct Bitmap.new on missing path MUST raise original Errno::ENOENT
$mock_fs.clear
assert_raises(Errno::ENOENT, "Direct Bitmap.new on missing file must raise Errno::ENOENT") do
  Bitmap.new("Graphics/Pictures/missing_primitive.png")
end

# 1.2: Direct Cache.normal_bitmap on missing path MUST raise Errno::ENOENT without negative dummy caching
assert_raises(Errno::ENOENT, "Cache.normal_bitmap on missing file must raise Errno::ENOENT") do
  Cache.normal_bitmap("Graphics/Pictures/missing_raw_normal.png")
end

# Verify no dummy entry is cached in Cache.@cache
raw_cache = Cache.instance_variable_get(:@cache) || {}
assert(!raw_cache.key?("Graphics/Pictures/missing_raw_normal.png"), "Cache.normal_bitmap must not cache negative dummy on error")

# 1.3: Direct decode failure (corrupt image data) MUST propagate exception, not hide behind placeholder
$mock_fs["Graphics/Pictures/corrupt_image.png"] = { corrupt: true }
assert_raises(ArgumentError, "Corrupt file decode must raise original decode exception at primitive level") do
  Bitmap.new("Graphics/Pictures/corrupt_image.png")
end

# --- Suite 2: Localization Probe Fallback (Langscore Contract) ---
puts "\n--- Suite 2: Localization Probe Contract (Langscore / Multi-Language) ---"

# Set up mock filesystem with base asset only (localized candidate missing)
$mock_fs.clear
$mock_fs["Graphics/Parallaxes/generic_title.png"] = { width: 640, height: 480 }
$mock_fs["Graphics/Characters/generic_casor.png"] = { width: 128, height: 128 }

# Install Langscore-style dynamic loader probe on Cache
$lang_cache = {}
$current_lang = "" # empty language suffix produces trailing '_'

class << Cache
  alias_method :test_lang_base_load_bitmap, :load_bitmap unless method_defined?(:test_lang_base_load_bitmap)

  def load_bitmap(folder_name, filename, hue = 0)
    new_filename = filename + "_" + ($current_lang || "")
    has_key = $lang_cache.has_key?(filename)

    if !has_key
      begin
        # Localization probe: tries localized candidate expecting exception if absent
        bitmap = test_lang_base_load_bitmap(folder_name, new_filename, hue)
        $lang_cache[filename] = true
        return bitmap
      rescue => e
        # Exception caught: records absence in cache and falls back to base
        $lang_cache[filename] = false
      end
    end

    if $lang_cache[filename]
      filename = new_filename
    end

    test_lang_base_load_bitmap(folder_name, filename, hue)
  end
end

# 2.1: Missing localized candidate -> base real dimensions loaded, cache records false
Cache.reset_cache!
$lang_cache.clear

title_bmp = Cache.parallax("generic_title")
assert(!title_bmp.nil?, "Cache.parallax with missing localized candidate must return valid base bitmap")
assert_equal(640, title_bmp.width, "Cache.parallax base image width must be 640 (real dimension, not 32x32 dummy)")
assert_equal(480, title_bmp.height, "Cache.parallax base image height must be 480 (real dimension, not 32x32 dummy)")
assert_equal(false, $lang_cache["generic_title"], "Localization cache for 'generic_title' must be false (not false-positive true)")

# 2.2: Character missing localized candidate -> base character loaded
casor_bmp = Cache.character("generic_casor")
assert(!casor_bmp.nil?, "Cache.character with missing localized candidate must return valid base bitmap")
assert_equal(128, casor_bmp.width, "Cache.character base image width must be 128 (real dimension, not dummy)")
assert_equal(128, casor_bmp.height, "Cache.character base image height must be 128 (real dimension, not dummy)")
assert_equal(false, $lang_cache["generic_casor"], "Localization cache for 'generic_casor' must be false")

# 2.3: When localized file ACTUALLY exists -> localized asset wins
$mock_fs["Graphics/Parallaxes/generic_title_en.png"] = { width: 1024, height: 768 }
$current_lang = "en"
Cache.reset_cache!
$lang_cache.clear

en_title_bmp = Cache.parallax("generic_title")
assert(!en_title_bmp.nil?, "Cache.parallax with existing localized candidate must return valid localized bitmap")
assert_equal(1024, en_title_bmp.width, "Localized asset must win when present (1024 width)")
assert_equal(768, en_title_bmp.height, "Localized asset must win when present (768 height)")
assert_equal(true, $lang_cache["generic_title"], "Localization cache for existing localized asset must be true")

# Reset probe wrapper to base
class << Cache
  if method_defined?(:test_lang_base_load_bitmap)
    alias_method :load_bitmap, :test_lang_base_load_bitmap
  end
end

# --- Suite 3: Genuinely Missing Base Asset Recovery at Public Typed Endpoints ---
puts "\n--- Suite 3: Public Typed Endpoint Fallback for Genuinely Missing Assets ---"

$mock_fs.clear
Cache.reset_cache!
RuntimePackAssetAudit.reset! if defined?(RuntimePackAssetAudit.reset!)

# 3.1: Genuinely missing tileset -> 512x384 checkered dummy bitmap + audit event
ts_fallback = Cache.tileset("Truly_Missing_Tileset")
assert(!ts_fallback.nil?, "Cache.tileset on genuinely missing tileset must return fallback bitmap")
assert_equal(512, ts_fallback.width, "Missing tileset fallback width must be 512")
assert_equal(384, ts_fallback.height, "Missing tileset fallback height must be 384")

# 3.2: Genuinely missing character -> > 32x32 smart character dummy bitmap + audit event
char_fallback = Cache.character("Truly_Missing_Character")
assert(!char_fallback.nil?, "Cache.character on genuinely missing character must return fallback bitmap")
assert(char_fallback.width > 32, "Missing character fallback width must be > 32 (got #{char_fallback.width})")
assert(char_fallback.height > 32, "Missing character fallback height must be > 32 (got #{char_fallback.height})")

# 3.3: Genuinely missing parallax -> 32x32 transparent empty bitmap + audit event
parallax_fallback = Cache.parallax("Truly_Missing_Parallax")
assert(!parallax_fallback.nil?, "Cache.parallax on genuinely missing parallax must return fallback bitmap")
assert_equal(32, parallax_fallback.width, "Missing parallax fallback width must be 32")
assert_equal(32, parallax_fallback.height, "Missing parallax fallback height must be 32")

# 3.4: Genuinely missing picture -> 32x32 transparent empty bitmap + audit event
pic_fallback = Cache.picture("Truly_Missing_Picture")
assert(!pic_fallback.nil?, "Cache.picture on genuinely missing picture must return fallback bitmap")
assert_equal(32, pic_fallback.width, "Missing picture fallback width must be 32")
assert_equal(32, pic_fallback.height, "Missing picture fallback height must be 32")

# Verify audit events were recorded for each genuinely missing asset
audit_events = RuntimePackAssetAudit.events
assert(audit_events.size >= 4, "Audit must record events for genuinely missing typed assets (recorded #{audit_events.size})")

ts_ev = audit_events.find { |e| e["asset_path"].include?("Truly_Missing_Tileset") }
assert(ts_ev, "Audit event for missing tileset must exist")
assert_equal("TILESET_FALLBACK_DUMMY", ts_ev["fallback_action"], "Tileset fallback action must be TILESET_FALLBACK_DUMMY")

char_ev = audit_events.find { |e| e["asset_path"].include?("Truly_Missing_Character") }
assert(char_ev, "Audit event for missing character must exist")
assert_equal("CHARACTER_FALLBACK_DUMMY", char_ev["fallback_action"], "Character fallback action must be CHARACTER_FALLBACK_DUMMY")

parallax_ev = audit_events.find { |e| e["asset_path"].include?("Truly_Missing_Parallax") }
assert(parallax_ev, "Audit event for missing parallax must exist")
assert_equal("PLACEHOLDER_SUBSTITUTED", parallax_ev["fallback_action"], "Parallax fallback action must be PLACEHOLDER_SUBSTITUTED")

pic_ev = audit_events.find { |e| e["asset_path"].include?("Truly_Missing_Picture") }
assert(pic_ev, "Audit event for missing picture must exist")
assert_equal("PLACEHOLDER_SUBSTITUTED", pic_ev["fallback_action"], "Picture fallback action must be PLACEHOLDER_SUBSTITUTED")

# --- Suite 4: Unlocalized Standard Game & Hue/Args Semantics ---
puts "\n--- Suite 4: Unlocalized Standard Game & Hue/Args Return Semantics ---"

$mock_fs.clear
$mock_fs["Graphics/Animations/Thunder.png"] = { width: 192, height: 192 }
$mock_fs["Graphics/Battlers/Dragon.png"] = { width: 300, height: 300 }

Cache.reset_cache!

# 4.1: Normal asset with hue argument
anim_bmp = Cache.animation("Thunder", 120)
assert(!anim_bmp.nil?, "Cache.animation with hue must load successfully")
assert_equal(192, anim_bmp.width, "Animation bitmap width must match")
assert_equal(120, anim_bmp.hue, "Animation bitmap hue must match requested hue")

# 4.2: Normal battler with hue argument
battler_bmp = Cache.battler("Dragon", 240)
assert(!battler_bmp.nil?, "Cache.battler with hue must load successfully")
assert_equal(300, battler_bmp.width, "Battler bitmap width must match")
assert_equal(240, battler_bmp.hue, "Battler bitmap hue must match requested hue")

# 4.3: Missing animation with hue falls back gracefully without crash
anim_missing = Cache.animation("NonExistentAnim", 180)
assert(!anim_missing.nil?, "Missing animation with hue must return fallback bitmap")
assert_equal(32, anim_missing.width, "Missing animation fallback width must be 32")

# --- Suite 5: RPG::Cache (RGSS1 / XP) Compatibility & Idempotence ---
puts "\n--- Suite 5: RGSS1 (RPG::Cache) Idempotence & Multi-Argument Handling ---"

$mock_fs.clear
$mock_fs["Graphics/Autotiles/Ground.png"] = { width: 96, height: 128 }
$mock_fs["Graphics/Fogs/Mist.png"] = { width: 640, height: 480 }

RPG::Cache.reset_cache!
RuntimePackCheatRunner.install_save_and_cache_hooks if defined?(RuntimePackCheatRunner)

# Re-running install_save_and_cache_hooks must be strictly idempotent
RuntimePackCheatRunner.install_save_and_cache_hooks if defined?(RuntimePackCheatRunner)
RuntimePackCheatRunner.install_save_and_cache_hooks if defined?(RuntimePackCheatRunner)

# Normal autotile
ground_bmp = RPG::Cache.autotile("Ground")
assert(!ground_bmp.nil?, "RPG::Cache.autotile must load valid asset")
assert_equal(96, ground_bmp.width, "Autotile width must match")

# Normal fog with hue
fog_bmp = RPG::Cache.fog("Mist", 50)
assert(!fog_bmp.nil?, "RPG::Cache.fog with hue must load valid asset")
assert_equal(640, fog_bmp.width, "Fog width must match")
assert_equal(50, fog_bmp.hue, "Fog hue must match")

# Missing autotile
missing_at = RPG::Cache.autotile("NonExistentAutotile")
assert(!missing_at.nil?, "RPG::Cache.autotile missing must return fallback bitmap")
assert_equal(32, missing_at.width, "Missing autotile fallback width must be 32")

# Missing fog with hue
missing_fog = RPG::Cache.fog("NonExistentFog", 90)
assert(!missing_fog.nil?, "RPG::Cache.fog missing with hue must return fallback bitmap")
assert_equal(32, missing_fog.width, "Missing fog fallback width must be 32")

# --- Final Summary ---
puts "\n============================================================"
puts "Tests run: #{$tests_run}, Passed: #{$tests_passed}, Failed: #{$tests_failed}"
puts "============================================================"

if $tests_failed > 0
  puts "STATUS: RED (Expected regression failures on baseline product code)"
  exit 1
else
  puts "STATUS: GREEN (All contracts satisfied)"
  exit 0
end
