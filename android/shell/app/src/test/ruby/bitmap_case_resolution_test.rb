# frozen_string_literal: true
# bitmap_case_resolution_test.rb
# TDD Behavioral Regression Suite for Case-Insensitive Bitmap Resolution and Extension Probing
# Tests production RuntimePackCasePath.resolve_bitmap_path and low-level Bitmap/Cache hooks.

require 'tmpdir'
require 'fileutils'

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

def assert_raises(exception_class, message = "Expected #{exception_class} to be raised")
  $tests_run += 1
  begin
    yield
    $tests_failed += 1
    warn "FAILED: #{message} (nothing was raised)"
    caller_locations(1, 1).each { |loc| warn "  at #{loc}" }
  rescue exception_class => e
    $tests_passed += 1
    e
  rescue Exception => e
    $tests_failed += 1
    warn "FAILED: #{message} (raised #{e.class}: #{e.message} instead of #{exception_class})"
    caller_locations(1, 1).each { |loc| warn "  at #{loc}" }
    e
  end
end

puts "============================================================"
puts "Running Bitmap Case Resolution & Extension Probing Tests"
puts "============================================================"

# --- 1. Explicit Native-Boundary Test Double (emulating case-sensitive Android mkxp-z) ---
# On Android (Linux ext4), the native C++ Bitmap loader requires exact case and extension.
# To ensure tests do not falsely pass due to Windows NTFS case-insensitivity, this double
# inspects the physical disk entries and strictly raises Errno::ENOENT if the path passed
# to the native boundary does not match the physical file name case-sensitively.
class StubCaseSensitiveBitmap
  attr_reader :width, :height, :path, :hue

  def initialize(*args)
    if args.size == 1
      raw_path = args[0].to_s
      if raw_path.empty?
        raise Errno::ENOENT, "No such file or directory - (empty path)"
      end

      # Emulate native Android case sensitivity: verify path against physical directory entries
      unless StubCaseSensitiveBitmap.physically_exists_case_sensitive?(raw_path)
        raise Errno::ENOENT, "No such file or directory - #{raw_path}"
      end

      # Decode check: check if marked corrupt
      if File.exist?(raw_path) && File.binread(raw_path) == "CORRUPT_DATA"
        raise ArgumentError, "Failed to decode image data: #{raw_path}"
      end

      @width = 640
      @height = 480
      @path = raw_path
      @hue = 0
    elsif args.size >= 2
      @width = args[0]
      @height = args[1]
      @path = nil
      @hue = 0
    else
      raise ArgumentError, "wrong number of arguments"
    end
  end

  def hue_change(h)
    @hue = h
  end

  def dup
    copy = StubCaseSensitiveBitmap.new(@width, @height)
    copy.instance_variable_set(:@hue, @hue)
    copy.instance_variable_set(:@path, @path)
    copy
  end

  def disposed?
    false
  end

  # Helper to verify whether raw_path matches physical disk entries strictly case-sensitively
  def self.physically_exists_case_sensitive?(path)
    normalized = path.tr('\\', '/')
    parts = normalized.split('/').reject { |p| p.empty? || p == '.' }
    return false if parts.empty?

    current = '.'
    parts.each do |segment|
      return false unless Dir.exist?(current)
      entries = Dir.entries(current) rescue []
      # Must match exact case on disk!
      return false unless entries.include?(segment)
      current = File.join(current, segment)
    end
    File.file?(current)
  end
end

Bitmap = StubCaseSensitiveBitmap unless defined?(Bitmap)

# --- 2. Load Production Bootstrap ---
BOOTSTRAP_FILE = ENV['RUNTIMEPACK_BOOTSTRAP_PATH'] ? File.expand_path(ENV['RUNTIMEPACK_BOOTSTRAP_PATH']) : File.expand_path('../../main/assets/runtimepack_bootstrap.rb', __dir__)
require BOOTSTRAP_FILE

# RGSS3 Cache Module Double (defined after bootstrap require, exactly as in game eval load order)
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

    def picture(filename)
      load_bitmap("Graphics/Pictures/", filename)
    end

    def animation(filename, hue = 0)
      load_bitmap("Graphics/Animations/", filename, hue)
    end

    def system(filename)
      load_bitmap("Graphics/System/", filename)
    end

    def battler(filename, hue = 0)
      load_bitmap("Graphics/Battlers/", filename, hue)
    end

    def parallax(filename)
      load_bitmap("Graphics/Parallaxes/", filename)
    end
  end
end

Cache = MockRGSS3Cache unless defined?(Cache)

test_sandbox = Dir.mktmpdir("rp_bitmap_case_test_")
original_pwd = Dir.pwd

begin
  Dir.chdir(test_sandbox)

  # Create physical test directory structure and sample assets
  FileUtils.mkdir_p("Graphics/Pictures")
  FileUtils.mkdir_p("Graphics/Animations")
  FileUtils.mkdir_p("Graphics/System")
  FileUtils.mkdir_p("Graphics/Battlers")
  FileUtils.mkdir_p("Graphics/Parallaxes")

  # Physical files with exact casing on disk:
  File.binwrite("Graphics/Pictures/Level Up.png", "VALID_PNG_DATA")
  File.binwrite("Graphics/Animations/reflection.png", "VALID_PNG_DATA")
  File.binwrite("Graphics/System/IconSet.png", "VALID_PNG_DATA")

  # ------------------------------------------------------------
  # Test 1: Core Brief Requirements (Level UP, Reflection, Iconset)
  # ------------------------------------------------------------
  puts "\nTest 1: Level UP -> Level Up.png, Reflection -> reflection.png, Iconset -> IconSet.png"
  Cache.reset_cache!

  bmp_lvl = Cache.picture("Level UP")
  assert_equal "Graphics/Pictures/Level Up.png", bmp_lvl.path, "Cache.picture('Level UP') must resolve to 'Graphics/Pictures/Level Up.png'"

  bmp_ref = Cache.animation("Reflection")
  assert_equal "Graphics/Animations/reflection.png", bmp_ref.path, "Cache.animation('Reflection') must resolve to 'Graphics/Animations/reflection.png'"

  bmp_icon = Cache.system("Iconset")
  assert_equal "Graphics/System/IconSet.png", bmp_icon.path, "Cache.system('Iconset') must resolve to 'Graphics/System/IconSet.png'"

  # Direct Bitmap.new resolution as well:
  direct_lvl = begin
    Bitmap.new("Graphics/Pictures/Level UP")
  rescue Exception => e
    assert false, "Bitmap.new('Graphics/Pictures/Level UP') must not raise #{e.class}: #{e.message}"
    nil
  end
  assert_equal "Graphics/Pictures/Level Up.png", direct_lvl&.path, "Bitmap.new('Graphics/Pictures/Level UP') must resolve to 'Graphics/Pictures/Level Up.png'"

  # ------------------------------------------------------------
  # Test 2: Exact-First vs Extension Search Order
  # ------------------------------------------------------------
  puts "\nTest 2: Exact-first match and extension probing order (.png before .jpg)"
  File.binwrite("Graphics/Pictures/ExactMatch.png", "DATA")
  File.binwrite("Graphics/Pictures/MultiExt.jpg", "JPG_DATA")
  File.binwrite("Graphics/Pictures/MultiExt.png", "PNG_DATA")
  File.binwrite("Graphics/Pictures/photo.jpg", "JPG_DATA")
  File.binwrite("Graphics/Pictures/photo.PNG", "PNG_DATA")

  Cache.reset_cache!
  # Exact match with extension specified
  bmp_exact = Cache.picture("ExactMatch.png")
  assert_equal "Graphics/Pictures/ExactMatch.png", bmp_exact.path

  # Exact .jpg must take priority over case-folded .PNG competitor
  bmp_photo = begin
    Cache.picture("photo")
  rescue Exception => e
    assert false, "Cache.picture('photo') must not raise #{e.class}: #{e.message}"
    nil
  end
  assert_equal "Graphics/Pictures/photo.jpg", bmp_photo&.path, "Exact .jpg must win over folded .PNG across candidate extensions"

  # Extension omitted: .png must take priority over .jpg when both are exact or folded
  bmp_multi = Cache.picture("MultiExt")
  assert_equal "Graphics/Pictures/MultiExt.png", bmp_multi.path, "Omitted extension must select .png before .jpg"

  # Explicit .jpg extension requested: must resolve to .jpg, not .png
  bmp_jpg = Cache.picture("multiext.jpg")
  assert_equal "Graphics/Pictures/MultiExt.jpg", bmp_jpg.path, "Explicit .jpg must select MultiExt.jpg"

  # Dotted stem resolution: icon.v1 -> Icon.v1.png
  File.binwrite("Graphics/Pictures/Icon.v1.png", "DATA")
  bmp_dotted = begin
    Cache.picture("icon.v1")
  rescue Exception => e
    assert false, "Cache.picture('icon.v1') must not raise #{e.class}: #{e.message}"
    nil
  end
  assert_equal "Graphics/Pictures/Icon.v1.png", bmp_dotted&.path, "Dotted stem icon.v1 must resolve to Icon.v1.png"

  # Arbitrary extension exact physical file: custom.dat
  File.binwrite("Graphics/Pictures/custom.dat", "DATA")
  bmp_custom = Bitmap.new("Graphics/Pictures/custom.dat")
  assert_equal "Graphics/Pictures/custom.dat", bmp_custom.path, "Arbitrary extension exact match must be passed to native decode"

  # ------------------------------------------------------------
  # Test 3: Upper-Case Extension (.PNG) and Folder Case Mismatch
  # ------------------------------------------------------------
  puts "\nTest 3: Upper-case extension (.PNG) and folder case mismatch"
  File.binwrite("Graphics/Pictures/Hero_Cap.PNG", "DATA")

  Cache.reset_cache!
  bmp_cap = Cache.picture("hero_cap")
  assert_equal "Graphics/Pictures/Hero_Cap.PNG", bmp_cap.path, "Must resolve to uppercase .PNG"

  # Folder case mismatch: 'graphics/pictures/HERO_CAP.png' -> 'Graphics/Pictures/Hero_Cap.PNG'
  bmp_folder = begin
    Bitmap.new("graphics/pictures/HERO_CAP.png")
  rescue Exception => e
    assert false, "Folder case mismatch must not raise #{e.class}: #{e.message}"
    nil
  end
  assert_equal "Graphics/Pictures/Hero_Cap.PNG", bmp_folder&.path, "Folder case mismatch must resolve to physical directory case"

  # ------------------------------------------------------------
  # Test 4: Spaces and Non-ASCII Characters in Path
  # ------------------------------------------------------------
  puts "\nTest 4: Spaces and non-ASCII (UTF-8) characters"
  File.binwrite("Graphics/Pictures/Special Hero 日本語.png", "DATA")
  File.binwrite("Graphics/Pictures/ lead_space.png", "DATA")

  Cache.reset_cache!
  bmp_utf8 = Cache.picture("special hero 日本語")
  assert_equal "Graphics/Pictures/Special Hero 日本語.png", bmp_utf8.path, "Spaces and UTF-8 characters must resolve correctly"

  # Meaningful leading space in physical file is resolved when requested
  bmp_lead = Bitmap.new("Graphics/Pictures/ lead_space.png")
  assert_equal "Graphics/Pictures/ lead_space.png", bmp_lead.path, "Physical leading space in filename must be preserved"

  # Untrimmed path that does not exist must not be silently stripped/mapped to a different file
  assert_raises(Errno::ENOENT, "Untrimmed non-existent space path must fail with Errno::ENOENT") do
    Bitmap.new(" Graphics/Pictures/normal.png")
  end
  assert_equal " Graphics/Pictures/normal.png", RuntimePackCasePath.resolve_bitmap_path(" Graphics/Pictures/normal.png"), "Untrimmed missing path must remain untouched"

  # ------------------------------------------------------------
  # Test 5: Empty Name Semantics
  # ------------------------------------------------------------
  puts "\nTest 5: Empty name semantics"
  assert_raises(Errno::ENOENT, "Bitmap.new('') must raise Errno::ENOENT") do
    Bitmap.new("")
  end

  # ------------------------------------------------------------
  # Test 6: Hue Modification & Repeat Load Caching
  # ------------------------------------------------------------
  puts "\nTest 6: Hue modification and repeat load caching"
  File.binwrite("Graphics/Battlers/Dragon.png", "DATA")

  Cache.reset_cache!
  d1 = Cache.battler("dragon", 0)
  d2 = Cache.battler("dragon", 0)
  assert d1.equal?(d2), "Repeat load of same key must return same cached instance"
  assert_equal 0, d1.hue, "Base hue must be 0"

  d_hue1 = Cache.battler("dragon", 120)
  d_hue2 = Cache.battler("dragon", 120)
  assert d_hue1.equal?(d_hue2), "Repeat load with hue must return same cached hue instance"
  assert_equal 120, d_hue1.hue, "Hue must be 120"
  assert_equal 0, d1.hue, "Base hue must remain 0"

  # ------------------------------------------------------------
  # Test 7: Case Collisions (Controlled Boundary Double)
  # ------------------------------------------------------------
  puts "\nTest 7: Case collisions (exact match wins; ambiguous collision preserved)"
  if defined?(RuntimePackCasePath) && RuntimePackCasePath.respond_to?(:resolve_bitmap_path)
    # Mock directory entries for collision directory
    class << Dir
      alias_method :test_collision_orig_entries, :entries
      def entries(path)
        p = path.to_s.tr('\\', '/')
        if p == './collision_dir' || p == 'collision_dir'
          ['.', '..', 'item.png', 'Item.png']
        elsif p == '.'
          ['.', '..', 'collision_parent', 'collision_dir']
        elsif p == './collision_parent' || p == 'collision_parent'
          ['.', '..', 'sub_dir', 'Sub_Dir']
        elsif p == './collision_parent/sub_dir' || p == './collision_parent/Sub_Dir' || p == 'collision_parent/sub_dir' || p == 'collision_parent/Sub_Dir'
          ['.', '..', 'item.png']
        else
          test_collision_orig_entries(path)
        end
      end
    end

    begin
      # 7.1: Exact match wins in collision
      res_exact1 = RuntimePackCasePath.resolve_bitmap_path("collision_dir/item.png")
      assert_equal "collision_dir/item.png", res_exact1, "Exact match 'item.png' must win in collision"

      res_exact2 = RuntimePackCasePath.resolve_bitmap_path("collision_dir/Item.png")
      assert_equal "collision_dir/Item.png", res_exact2, "Exact match 'Item.png' must win in collision"

      # 7.2: Ambiguous collision with no exact match: must not guess, preserve original path
      res_ambig1 = RuntimePackCasePath.resolve_bitmap_path("collision_dir/ITEM.png")
      assert_equal "collision_dir/ITEM.png", res_ambig1, "Ambiguous collision must preserve original path"

      res_ambig2 = RuntimePackCasePath.resolve_bitmap_path("collision_dir/ITEM")
      assert_equal "collision_dir/ITEM", res_ambig2, "Ambiguous collision without extension must preserve original path"

      # 7.3: Intermediate directory collision: exact match vs ambiguous
      res_dir_exact1 = RuntimePackCasePath.resolve_bitmap_path("collision_parent/sub_dir/item.png")
      assert_equal "collision_parent/sub_dir/item.png", res_dir_exact1, "Exact directory match 'sub_dir' must win in directory collision"

      res_dir_exact2 = RuntimePackCasePath.resolve_bitmap_path("collision_parent/Sub_Dir/item.png")
      assert_equal "collision_parent/Sub_Dir/item.png", res_dir_exact2, "Exact directory match 'Sub_Dir' must win in directory collision"

      res_dir_ambig = RuntimePackCasePath.resolve_bitmap_path("collision_parent/SUB_DIR/item.png")
      assert_equal "collision_parent/SUB_DIR/item.png", res_dir_ambig, "Ambiguous directory collision with no exact match must preserve original path"
    ensure
      class << Dir
        alias_method :entries, :test_collision_orig_entries
      end
    end
  else
    assert false, "RuntimePackCasePath.resolve_bitmap_path must be defined"
  end

  # ------------------------------------------------------------
  # Test 8: Traversal / Root Containment
  # ------------------------------------------------------------
  puts "\nTest 8: Traversal and root containment"
  assert_raises(Errno::ENOENT, "Path escaping root via ../ must fail safely with Errno::ENOENT") do
    Bitmap.new("../../outside_secret.png")
  end

  # Symlink escaping root containment check (leaf and directory)
  $outside_dir_entries_called = false

  class << File
    alias_method :test_symlink_orig_realpath, :realpath
    def realpath(path)
      p = path.to_s.tr('\\', '/')
      if p.include?("escaped_symlink") || p.include?("Escaped_Symlink")
        "C:/outside_system_directory/secret.png"
      elsif p.include?("escaped_dir") || p.include?("Escaped_Dir")
        "C:/outside_system_directory"
      else
        test_symlink_orig_realpath(path)
      end
    end
  end

  class << Dir
    alias_method :test_symlink_orig_entries, :entries
    def entries(path)
      p = path.to_s.tr('\\', '/')
      if p.include?("escaped_dir") || p.include?("outside_system_directory")
        $outside_dir_entries_called = true
      end
      test_symlink_orig_entries(path)
    end
  end

  begin
    # 8.1 Leaf symlink escaping root must not resolve/casefold to external file
    # Physical file on disk is Escaped_Symlink.png (PascalCase, .png)
    # Requested path is lowercase and omits extension
    File.binwrite("Graphics/Pictures/Escaped_Symlink.png", "DATA")
    res_leaf = RuntimePackCasePath.resolve_bitmap_path("Graphics/Pictures/escaped_symlink")
    assert_equal "Graphics/Pictures/escaped_symlink", res_leaf, "Symlink leaf escaping root must not be case-folded/resolved; must return original requested path"

    # 8.2 Directory symlink escaping root must return original path and never call Dir.entries on outside directory
    FileUtils.mkdir_p("Graphics/Escaped_Dir")
    File.binwrite("Graphics/Escaped_Dir/Hero.png", "DATA")
    res_dir = RuntimePackCasePath.resolve_bitmap_path("Graphics/escaped_dir/hero")
    assert_equal "Graphics/escaped_dir/hero", res_dir, "Directory symlink escaping root must return original path"
    assert_equal false, $outside_dir_entries_called, "Dir.entries must never be called on directory escaping root"
  ensure
    class << File
      alias_method :realpath, :test_symlink_orig_realpath
    end
    class << Dir
      alias_method :entries, :test_symlink_orig_entries
    end
  end

  # ------------------------------------------------------------
  # Test 9: Missing and Corrupt Images Propagate Original Exceptions
  # ------------------------------------------------------------
  puts "\nTest 9: Missing and corrupt images propagate original exceptions"
  # Missing file
  assert_raises(Errno::ENOENT, "Missing image must raise Errno::ENOENT") do
    Bitmap.new("Graphics/Pictures/definitely_not_here")
  end

  # Corrupt image data
  File.binwrite("Graphics/Pictures/corrupt_file.png", "CORRUPT_DATA")
  assert_raises(ArgumentError, "Corrupt image must raise original ArgumentError decode exception") do
    Bitmap.new("Graphics/Pictures/corrupt_file")
  end

  # ------------------------------------------------------------
  # Test 10: Production Load Order & Layered Plugin Aliases (Damage, Composite)
  # ------------------------------------------------------------
  puts "\nTest 10: Production load order & layered plugin aliases (Damage, Composite)"
  FileUtils.mkdir_p("Graphics/Damage")
  File.binwrite("Graphics/Damage/Level Up.png", "DMG_DATA")
  File.binwrite("Graphics/Pictures/Dmg_Slash.png", "DATA")
  File.binwrite("Graphics/Pictures/Base_Layer.png", "DATA")

  $damage_plugin_received = nil
  $composite_plugin_received = nil

  class << Cache
    alias_method :plugin_test_orig_load_bitmap, :load_bitmap

    def damage(filename)
      load_bitmap("Graphics/Damage/", filename)
    end unless method_defined?(:damage)

    def load_bitmap(dir_name, filename, hue = 0)
      if dir_name.include?("Damage")
        $damage_plugin_received = filename
      end
      if filename.to_s.include?("layer")
        $composite_plugin_received = filename
      end
      plugin_test_orig_load_bitmap(dir_name, filename, hue)
    end
  end

  # ------------------------------------------------------------
  # Test 11: Production Load Order: Langscore Localization Probe Contract
  # ------------------------------------------------------------
  puts "\nTest 11: Production load order: Langscore localization probe (present vs missing)"
  # Localized asset present:
  File.binwrite("Graphics/Pictures/Level Up_en.png", "EN_DATA")
  # Base only (localized missing):
  File.binwrite("Graphics/Parallaxes/game_title.png", "BASE_DATA")

  $test_lang_cache = {}
  $test_cur_lang = "en"
  $loc_received_name = nil

  class << Cache
    alias_method :langscore_test_orig_load_bitmap, :load_bitmap

    def load_bitmap(folder_name, filename, hue = 0)
      $loc_received_name = filename
      new_filename = "#{filename}_#{$test_cur_lang}"
      cache_key = "#{folder_name}#{filename}"
      unless $test_lang_cache.key?(cache_key)
        begin
          b = langscore_test_orig_load_bitmap(folder_name, new_filename, hue)
          $test_lang_cache[cache_key] = true
          return b
        rescue Errno::ENOENT
          $test_lang_cache[cache_key] = false
        end
      end

      target_file = $test_lang_cache[cache_key] ? new_filename : filename
      langscore_test_orig_load_bitmap(folder_name, target_file, hue)
    end
  end

  # Invoke late production hook installation (exactly simulating bootstrap line 2573 after script eval)
  RuntimePackCheatRunner.install_save_and_cache_hooks rescue nil

  Cache.reset_cache!
  # Custom damage route in Graphics/Damage/ executes and receives raw requested name
  dmg_lvl = Cache.damage("Level UP")
  assert_equal "Level UP", $damage_plugin_received, "Damage plugin must receive raw requested 'Level UP'"
  assert_equal "Graphics/Damage/Level Up.png", dmg_lvl.path, "Damage bitmap must resolve to Graphics/Damage/Level Up.png"

  dmg_bmp = Cache.picture("dmg_slash")
  assert_equal "Graphics/Pictures/Dmg_Slash.png", dmg_bmp.path, "Damage bitmap must resolve to Dmg_Slash.png"

  comp_bmp = Cache.picture("base_layer")
  assert_equal "base_layer", $composite_plugin_received, "Composite plugin must receive raw requested name"
  assert_equal "Graphics/Pictures/Base_Layer.png", comp_bmp.path, "Composite bitmap must resolve to Base_Layer.png"

  # 11.1 Localized asset present loads translated file and receives raw name
  loc_bmp = Cache.picture("Level UP")
  assert_equal "Level UP", $loc_received_name, "Localization plugin must receive raw requested 'Level UP'"
  assert_equal "Graphics/Pictures/Level Up_en.png", loc_bmp.path, "Must load translated asset 'Level Up_en.png'"
  assert_equal true, $test_lang_cache["Graphics/Pictures/Level UP"], "Localization cache must record true for existing translated asset"

  # 11.2 Localized missing falls back through original exception to base file
  title_bmp = Cache.parallax("game_title")
  assert_equal "Graphics/Parallaxes/game_title.png", title_bmp.path, "Must fall back to base parallax"
  assert_equal false, $test_lang_cache["Graphics/Parallaxes/game_title"], "Localization cache must record false for absent localized image"

  # 11.3 Repeat load and hue semantics under late hook
  loc_repeat = Cache.picture("Level UP")
  assert loc_bmp.equal?(loc_repeat), "Repeat load with localization plugin active must return same cached instance"

  File.binwrite("Graphics/Battlers/Level Up.png", "BAT_DATA")
  bat_base = Cache.battler("Level UP", 0)
  bat_hue1 = Cache.battler("Level UP", 120)
  assert_equal 0, bat_base.hue, "Base battler hue must be 0"
  assert_equal 120, bat_hue1.hue, "Battler with hue must have hue 120"
  bat_hue2 = Cache.battler("Level UP", 120)
  assert bat_hue1.equal?(bat_hue2), "Repeat load with hue must return same cached hue instance"

  # ------------------------------------------------------------
  # Test 12: Runtime-Created Files (Created After Startup)
  # ------------------------------------------------------------
  puts "\nTest 12: Runtime-created files (no stale negative cache, pathCache: false semantics)"
  # File does not exist yet
  assert_raises(Errno::ENOENT, "Non-existent runtime file must fail initially") do
    Bitmap.new("Graphics/Pictures/screenshot_save01")
  end

  # Now file is created dynamically at runtime:
  File.binwrite("Graphics/Pictures/Screenshot_Save01.png", "DATA")

  # Must resolve immediately without needing restart or clearing pathCache
  runtime_bmp = begin
    Bitmap.new("Graphics/Pictures/screenshot_save01")
  rescue Exception => e
    assert false, "Newly created file must not raise #{e.class}: #{e.message}"
    nil
  end
  assert_equal "Graphics/Pictures/Screenshot_Save01.png", runtime_bmp&.path, "Newly created file must resolve immediately"

  # File renamed dynamically at runtime:
  File.rename("Graphics/Pictures/Screenshot_Save01.png", "Graphics/Pictures/Screenshot_Renamed01.png")

  # Old path must fail immediately after rename:
  assert_raises(Errno::ENOENT, "Old path must fail immediately after rename") do
    Bitmap.new("Graphics/Pictures/screenshot_save01")
  end

  # New physical path must resolve immediately after rename:
  renamed_bmp = begin
    Bitmap.new("Graphics/Pictures/screenshot_renamed01")
  rescue Exception => e
    assert false, "Renamed file must not raise #{e.class}: #{e.message}"
    nil
  end
  assert_equal "Graphics/Pictures/Screenshot_Renamed01.png", renamed_bmp&.path, "Renamed file must resolve immediately"

  # File deleted at runtime:
  File.delete("Graphics/Pictures/Screenshot_Renamed01.png")
  assert_raises(Errno::ENOENT, "Deleted file must fail on subsequent load") do
    Bitmap.new("Graphics/Pictures/screenshot_renamed01")
  end

  # ------------------------------------------------------------
  # Test 13: Bitmap.new(width, height) Contract Integrity
  # ------------------------------------------------------------
  puts "\nTest 13: Bitmap.new(width, height) signature contract integrity"
  blank32 = Bitmap.new(32, 32)
  assert_equal 32, blank32.width
  assert_equal 32, blank32.height
  assert_equal nil, blank32.path, "Blank bitmap path must be nil"

  blank640 = Bitmap.new(640, 480)
  assert_equal 640, blank640.width
  assert_equal 480, blank640.height
  assert_equal nil, blank640.path, "Blank bitmap path must be nil"

  # ------------------------------------------------------------
  # Test 14: Direct Unit Contracts on RuntimePackCasePath.resolve_bitmap_path
  # ------------------------------------------------------------
  puts "\nTest 14: Direct unit contracts on RuntimePackCasePath.resolve_bitmap_path"
  if defined?(RuntimePackCasePath) && RuntimePackCasePath.respond_to?(:resolve_bitmap_path)
    assert_equal "Graphics/Pictures/Level Up.png", RuntimePackCasePath.resolve_bitmap_path("Graphics/Pictures/Level UP")
    assert_equal "Graphics/Animations/reflection.png", RuntimePackCasePath.resolve_bitmap_path("Graphics/Animations/Reflection")
    assert_equal "Graphics/System/IconSet.png", RuntimePackCasePath.resolve_bitmap_path("Graphics/System/Iconset")
    assert_equal "Graphics/Pictures/Level Up.png", RuntimePackCasePath.resolve_bitmap_path("./Graphics/Pictures/Level UP")
    assert_equal "missing_file", RuntimePackCasePath.resolve_bitmap_path("missing_file")
  else
    assert false, "RuntimePackCasePath.resolve_bitmap_path must be defined"
  end

ensure
  Dir.chdir(original_pwd)
  FileUtils.remove_entry_secure(test_sandbox) rescue nil
end

puts "\n============================================================"
puts "Tests run: #{$tests_run}, Passed: #{$tests_passed}, Failed: #{$tests_failed}"
puts "============================================================"

if $tests_failed > 0
  puts "STATUS: RED (#{$tests_failed} assertions failed)"
  exit 1
else
  puts "STATUS: GREEN (All bitmap case resolution and extension probing tests passed)"
  exit 0
end
