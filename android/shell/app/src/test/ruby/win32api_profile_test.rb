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
puts "Running Win32API Profile String Contract Tests (#{target_lib})"
puts "============================================================"

# We execute tests within an isolated sandbox directory mimicking app-private game storage
test_sandbox = Dir.mktmpdir("rp_profile_test_")
original_pwd = Dir.pwd

begin
  Dir.chdir(test_sandbox)

  # ------------------------------------------------------------
  # Test Case 1: Existing Game.ini with [Langscore] Lang=en returns 'en'
  # ------------------------------------------------------------
  puts "Test 1: Existing Game.ini with [Langscore] Lang=en returns 'en'"
  File.write("Game.ini", "[Game]\r\nTitle=KING EXIT\r\n\r\n[Langscore]\r\nLang=en\r\n")

  # Exact Langscore call shape from Script 121
  get_api = Win32API.new('kernel32', 'GetPrivateProfileString', %w(p p p p l p), 'L')
  buf = ' ' * 17
  r = get_api.call("Langscore", "Lang", "ja", buf, 17, "./Game.ini")

  assert_equal 2, r, "Return value for 'en' should be 2"
  assert_equal "en", buf.slice(0...r), "Buffer slice should equal 'en'"
  assert_equal "\0", buf[2], "Buffer byte at index 2 must be NUL"

  # Class-level Win32API.call convenience method
  buf_class = ' ' * 17
  r_class = Win32API.call('GetPrivateProfileString', "Langscore", "Lang", "ja", buf_class, 17, "./Game.ini")
  assert_equal 2, r_class, "Class-level Win32API.call return value should be 2"
  assert_equal "en", buf_class.slice(0...r_class), "Class-level Win32API.call buffer slice should equal 'en'"
  assert_equal "\0", buf_class[2], "Class-level Win32API.call byte at index 2 must be NUL"

  # Case-insensitive section and key lookup
  buf2 = ' ' * 17
  r2 = get_api.call("langscore", "lang", "ja", buf2, 17, "./Game.ini")
  assert_equal 2, r2, "Case-insensitive section and key lookup should succeed"
  assert_equal "en", buf2.slice(0...r2), "Case-insensitive lookup should return 'en'"

  # ------------------------------------------------------------
  # Test Case 2: Missing key returns caller-provided default
  # ------------------------------------------------------------
  puts "Test 2: Missing key returns caller-provided default"
  buf3 = ' ' * 17
  r3 = get_api.call("Langscore", "NonExistentKey", "ja", buf3, 17, "./Game.ini")
  assert_equal 2, r3, "Missing key should return length of default 'ja' (2)"
  assert_equal "ja", buf3.slice(0...r3), "Missing key buffer should contain default 'ja'"

  buf3b = ' ' * 17
  r3b = get_api.call("NonExistentSection", "Lang", "ja", buf3b, 17, "./Game.ini")
  assert_equal 2, r3b, "Missing section should return length of default 'ja' (2)"
  assert_equal "ja", buf3b.slice(0...r3b), "Missing section buffer should contain default 'ja'"

  # ------------------------------------------------------------
  # Test Case 3: Missing file returns caller-provided default
  # ------------------------------------------------------------
  puts "Test 3: Missing file returns caller-provided default"
  buf4 = ' ' * 32
  r4 = get_api.call("Langscore", "Lang", "fallback_lang", buf4, 32, "./NoSuchFile.ini")
  assert_equal 13, r4, "Missing file should return length of default 'fallback_lang' (13)"
  assert_equal "fallback_lang", buf4.slice(0...r4), "Missing file buffer should contain 'fallback_lang'"

  # ------------------------------------------------------------
  # Test Case 4: Buffer writes are NUL-safe and bounded
  # ------------------------------------------------------------
  puts "Test 4: Buffer writes are NUL-safe and bounded"
  File.write("Bounds.ini", "[LongSection]\r\nLongKey=very_long_string_value_for_testing\r\nActor=サミダレ\r\nDesc=Terowongan Bawah Tanah\r\n")

  # Destination buffer capacity is bounded by nSize (e.g. nSize = 8 -> max 7 chars + NUL)
  buf5 = "X" * 16
  r5 = get_api.call("LongSection", "LongKey", "def", buf5, 8, "./Bounds.ini")
  assert_equal 7, r5, "Return value bounded by nSize - 1 (8 - 1 = 7)"
  assert_equal "very_lo", buf5.slice(0...r5), "Buffer should contain truncated string of 7 chars"
  assert_equal "\0", buf5[7], "Buffer at index 7 must be NUL"
  assert_equal "XXXXXXXX", buf5.slice(8..-1), "Buffer beyond nSize must not be modified"

  # Preallocated buffer smaller than nSize: must not expand buffer beyond original bytesize
  buf6 = "Y" * 6
  r6 = get_api.call("LongSection", "LongKey", "def", buf6, 32, "./Bounds.ini")
  assert_equal 5, r6, "Return value bounded by buffer.bytesize - 1 (6 - 1 = 5)"
  assert_equal "very_", buf6.slice(0...r6), "Buffer slice should be bounded to 5 chars"
  assert_equal "\0", buf6[5], "Buffer at index 5 must be NUL"
  assert_equal 6, buf6.bytesize, "Buffer bytesize must not be expanded"

  # Invalid buffer or nSize <= 0 returns 0
  assert_equal 0, get_api.call("LongSection", "LongKey", "def", nil, 16, "./Bounds.ini")
  assert_equal 0, get_api.call("LongSection", "LongKey", "def", buf5, 0, "./Bounds.ini")

  # ------------------------------------------------------------
  # Test Case 4b: Multibyte UTF-8 byte bounds & encoding preservation
  # ------------------------------------------------------------
  puts "Test 4b: Multibyte UTF-8 byte bounds & encoding preservation"
  # 'サミダレ' is 4 chars, 12 UTF-8 bytes (3 bytes each)
  # nSize = 8 -> max_bytes = 7: 2 chars = 6 bytes ('サミ') fits, 3 chars = 9 bytes exceeds
  buf_mb1 = "Z" * 16
  r_mb1 = get_api.call("LongSection", "Actor", "def", buf_mb1, 8, "./Bounds.ini")
  assert_equal 6, r_mb1, "Multibyte slice with max_bytes=7 must return 6 bytes (2 katakana chars)"
  mb_str1 = buf_mb1.byteslice(0, r_mb1)
  assert_equal "サミ", mb_str1, "Buffer slice must be 'サミ'"
  assert mb_str1.valid_encoding?, "Multibyte slice must remain valid UTF-8"
  assert_equal "\0", buf_mb1.byteslice(6, 1), "Buffer byte at index 6 must be NUL"
  assert_equal "ZZZZZZZZZ", buf_mb1.byteslice(7..-1), "Buffer beyond nSize must not be touched"

  # nSize = 4 -> max_bytes = 3: 1 char = 3 bytes ('サ') fits
  buf_mb2 = "Z" * 16
  r_mb2 = get_api.call("LongSection", "Actor", "def", buf_mb2, 4, "./Bounds.ini")
  assert_equal 3, r_mb2, "Multibyte slice with max_bytes=3 must return 3 bytes (1 katakana char)"
  mb_str2 = buf_mb2.byteslice(0, r_mb2)
  assert_equal "サ", mb_str2, "Buffer slice must be 'サ'"
  assert mb_str2.valid_encoding?, "Single char slice must remain valid UTF-8"
  assert_equal "\0", buf_mb2.byteslice(3, 1), "Buffer byte at index 3 must be NUL"

  # nSize = 3 -> max_bytes = 2: 0 chars fit without splitting multibyte sequence
  buf_mb3 = "Z" * 16
  r_mb3 = get_api.call("LongSection", "Actor", "def", buf_mb3, 3, "./Bounds.ini")
  assert_equal 0, r_mb3, "Multibyte slice with max_bytes=2 cannot fit 3-byte char; must return 0"
  assert_equal "\0", buf_mb3.byteslice(0, 1), "Buffer byte at index 0 must be NUL"

  # Indonesian string: 'Terowongan Bawah Tanah' (22 bytes) with nSize = 11 -> max_bytes = 10
  buf_id = "Z" * 24
  r_id = get_api.call("LongSection", "Desc", "def", buf_id, 11, "./Bounds.ini")
  assert_equal 10, r_id, "ASCII/Indonesian string with max_bytes=10 must return 10 bytes"
  assert_equal "Terowongan", buf_id.byteslice(0, r_id), "Buffer slice should equal 'Terowongan'"
  assert_equal "\0", buf_id.byteslice(10, 1), "Buffer byte at index 10 must be NUL"

  # ------------------------------------------------------------
  # Test Case 5: Writes persist to the approved app-private configuration path
  # ------------------------------------------------------------
  puts "Test 5: Writes persist to the approved app-private configuration path"
  write_api = Win32API.new('kernel32', 'WritePrivateProfileString', %w(p p p p), 'i')

  # Write to existing Game.ini
  r_write1 = write_api.call("Langscore", "Lang", "en", "./Game.ini")
  assert_equal 1, r_write1, "WritePrivateProfileString should return 1 on success"

  ini_content1 = File.read("Game.ini")
  assert ini_content1.include?("Title=KING EXIT"), "Existing Game.ini content must be preserved"
  assert ini_content1.include?("Lang=en"), "Game.ini must contain updated Lang=en"

  # Update value to 'id'
  r_write2 = write_api.call("Langscore", "Lang", "id", "./Game.ini")
  assert_equal 1, r_write2, "Second write should return 1 on success"
  ini_content2 = File.read("Game.ini")
  assert ini_content2.include?("Lang=id"), "Game.ini must contain updated Lang=id"

  # Verify persistence via GetPrivateProfileString
  buf7 = ' ' * 17
  r7 = get_api.call("Langscore", "Lang", "ja", buf7, 17, "./Game.ini")
  assert_equal 2, r7, "Read after write should return length 2"
  assert_equal "id", buf7.slice(0...r7), "Read after write should return 'id'"

  # Write to a new config file
  r_write3 = write_api.call("Audio", "BgmVolume", "85", "./NewConfig.ini")
  assert_equal 1, r_write3, "Write to new file should return 1"
  assert File.exist?("NewConfig.ini"), "NewConfig.ini must be created"
  new_content = File.read("NewConfig.ini")
  assert new_content.include?("[Audio]"), "New file must have [Audio] section"
  assert new_content.include?("BgmVolume=85"), "New file must have BgmVolume=85"

  # Class-level Win32API.call write
  r_write_class = Win32API.call('WritePrivateProfileString', "Audio", "SeVolume", "70", "./NewConfig.ini")
  assert_equal 1, r_write_class, "Class-level Win32API.call write should return 1"
  new_content2 = File.read("NewConfig.ini")
  assert new_content2.include?("SeVolume=70"), "New file must have SeVolume=70 from class-level call"

  # ------------------------------------------------------------
  # Test Case 6: Failed writes return failure rather than synthetic success
  # ------------------------------------------------------------
  puts "Test 6: Failed writes return failure rather than synthetic success"
  readonly_file = File.join(test_sandbox, "Readonly.ini")
  File.write(readonly_file, "[Settings]\r\nKey=Init\r\n")
  File.chmod(0444, readonly_file)

  # Attempt write to read-only file
  r_write_fail = write_api.call("Settings", "Key", "NewVal", readonly_file)
  assert_equal 0, r_write_fail, "Write to read-only file must return 0 (failure)"

  # Target is a directory path
  dir_target = File.join(test_sandbox, "SubDir")
  FileUtils.mkdir_p(dir_target)
  r_write_dir = write_api.call("Settings", "Key", "Val", dir_target)
  assert_equal 0, r_write_dir, "Write to a directory path must return 0 (failure)"

  # Verify deterministic persistence & content match
  r_write_verify = write_api.call("VerifiedSec", "VerifiedKey", "VerifiedVal", "./Verified.ini")
  assert_equal 1, r_write_verify, "Write with verification must return 1"
  assert File.exist?("Verified.ini"), "Verified.ini must exist"
  v_content = File.binread("Verified.ini")
  assert v_content.include?("[VerifiedSec]") && v_content.include?("VerifiedKey=VerifiedVal"), "Content must match expected"

  # Verify no orphaned temporary files remain
  tmp_leftovers = Dir.glob(".*.tmp") + Dir.glob("*.tmp")
  assert tmp_leftovers.empty?, "No temporary swap files should remain orphaned after writes: #{tmp_leftovers.inspect}"

  # ------------------------------------------------------------
  # Test Case 6b: Forced replacement failure preserves existing INI byte-for-byte
  # ------------------------------------------------------------
  puts "Test 6b: Forced replacement failure preserves existing INI byte-for-byte"
  rename_fail_file = File.join(test_sandbox, "RenameFailTest.ini")
  initial_content = "[CriticalConfig]\r\nOriginalKey=KeepMeByteExact123\r\nChecksum=0xDEADBEEF\r\n"
  File.binwrite(rename_fail_file, initial_content)

  class << File
    alias_method :test_backup_rename, :rename
    def rename(from, to)
      if to.include?("RenameFailTest.ini")
        raise Errno::EACCES, "Simulated atomic rename failure"
      end
      test_backup_rename(from, to)
    end
  end

  begin
    r_rename_fail = write_api.call("CriticalConfig", "OriginalKey", "CORRUPTED_OVERWRITE", rename_fail_file)
    assert_equal 0, r_rename_fail, "Write must return 0 when atomic rename fails"
    assert_equal initial_content, File.binread(rename_fail_file), "Existing INI must remain 100% byte-for-byte preserved after rename failure"

    orphans = Dir.glob(".*.tmp") + Dir.glob("*.tmp")
    assert orphans.empty?, "No orphan temporary files should remain after rename failure: #{orphans.inspect}"
  ensure
    class << File
      alias_method :rename, :test_backup_rename
      remove_method :test_backup_rename
    end
  end

  # ------------------------------------------------------------
  # Test Case 7: Path traversal or non-game configuration paths are rejected or safely isolated
  # ------------------------------------------------------------
  puts "Test 7: Path traversal or non-game configuration paths are rejected"

  # Read traversal
  outside_file = File.join(Dir.tmpdir, "outside_rp_secret.ini")
  File.write(outside_file, "[Secret]\r\nPassword=leaked\r\n")

  buf_trav = ' ' * 32
  r_trav = get_api.call("Secret", "Password", "safe_default", buf_trav, 32, "../../outside_rp_secret.ini")
  assert_equal 12, r_trav, "Traversal read should return default length"
  assert_equal "safe_default", buf_trav.slice(0...r_trav), "Traversal read must return caller default"

  # Write traversal
  outside_write_target = "../../outside_rp_write_test.ini"
  r_trav_write = write_api.call("Exploit", "Key", "Val", outside_write_target)
  assert_equal 0, r_trav_write, "Traversal write must return 0 (failure)"

  # Windows-style backslash traversal
  r_trav_write_bs = write_api.call("Exploit", "Key", "Val", "..\\..\\outside_rp_write_bs.ini")
  assert_equal 0, r_trav_write_bs, "Backslash traversal write must return 0 (failure)"

  # ------------------------------------------------------------
  # Test Case 8: Symlink escape rejection (file and directory)
  # ------------------------------------------------------------
  puts "Test 8: Symlink escape rejection (file and directory)"
  outside_symlink_target = File.join(Dir.tmpdir, "outside_symlink_secret_#{Process.pid}.ini")
  File.write(outside_symlink_target, "[TopSecret]\r\nAdmin=hacked\r\n")

  outside_symlink_dir = Dir.mktmpdir("outside_symlink_dir_")
  outside_symlink_dir_file = File.join(outside_symlink_dir, "dir_secret.ini")
  File.write(outside_symlink_dir_file, "[DirSecret]\r\nToken=compromised\r\n")

  # Create symlink file inside sandbox pointing outside
  File.symlink(outside_symlink_target, "link_to_outside.ini") rescue nil
  if File.symlink?("link_to_outside.ini")
    buf_sym = ' ' * 32
    r_sym_read = get_api.call("TopSecret", "Admin", "sym_default", buf_sym, 32, "./link_to_outside.ini")
    assert_equal 11, r_sym_read, "Symlink file read must reject target and return default length"
    assert_equal "sym_default", buf_sym.slice(0...r_sym_read), "Symlink file read must return caller default"

    r_sym_write = write_api.call("TopSecret", "Admin", "injected", "./link_to_outside.ini")
    assert_equal 0, r_sym_write, "Symlink file write must return 0 (failure)"
    assert_equal "[TopSecret]\r\nAdmin=hacked\r\n", File.read(outside_symlink_target), "Outside file must not be modified via symlink"
  end

  # Create symlink directory inside sandbox pointing outside
  File.symlink(outside_symlink_dir, "link_dir") rescue nil
  if File.symlink?("link_dir")
    buf_sdir = ' ' * 32
    r_sdir_read = get_api.call("DirSecret", "Token", "dir_default", buf_sdir, 32, "./link_dir/dir_secret.ini")
    assert_equal 11, r_sdir_read, "Symlink directory read must reject traversal and return default length"
    assert_equal "dir_default", buf_sdir.slice(0...r_sdir_read), "Symlink directory read must return caller default"

    r_sdir_write = write_api.call("DirSecret", "Token", "injected", "./link_dir/dir_secret.ini")
    assert_equal 0, r_sdir_write, "Symlink directory write must return 0 (failure)"

    r_sdir_write_new = write_api.call("NewSec", "NewKey", "Val", "./link_dir/new_file.ini")
    assert_equal 0, r_sdir_write_new, "Symlink directory write to new file must return 0 (failure)"
    assert !File.exist?(File.join(outside_symlink_dir, "new_file.ini")), "No file should be created in outside directory via symlink"
  end

ensure
  File.chmod(0644, readonly_file) rescue nil
  File.delete(outside_file) rescue nil
  File.delete(outside_symlink_target) rescue nil
  FileUtils.remove_entry_secure(outside_symlink_dir) rescue nil
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
  puts "STATUS: GREEN (#{target_lib} contracts satisfied)"
  if ENV['TEST_TARGET_LIB'].nil?
    puts "\nTriggering sub-suite for win32api_shim..."
    require 'rbconfig'
    shim_ok = system({ 'TEST_TARGET_LIB' => 'win32api_shim' }, RbConfig.ruby, File.expand_path(__FILE__))
    exit(shim_ok ? 0 : 1)
  else
    exit 0
  end
end
