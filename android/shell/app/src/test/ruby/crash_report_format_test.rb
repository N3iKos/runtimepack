# frozen_string_literal: false
# crash_report_format_test.rb
#
# Verifies that fatal crash reports carry enough identity to attribute the
# failure without guesswork: engine, Ruby version, platform, script + line,
# exception class/message, and a bounded stack.
#
# Motivation (2026-11 session): attributing a crash to a specific APK build
# required days of manual work because Error.log recorded only the exception
# and a bare backtrace. See docs/superpowers/plans/2026-11-08-stability-guards-and-haven-seams.md Task 2.

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

puts '============================================================'
puts 'Running Crash Report Format Tests'
puts '============================================================'

BOOTSTRAP_FILE = ENV['RUNTIMEPACK_BOOTSTRAP_PATH'] ?
  File.expand_path(ENV['RUNTIMEPACK_BOOTSTRAP_PATH']) :
  File.expand_path('../../main/assets/runtimepack_bootstrap.rb', __dir__)

require BOOTSTRAP_FILE

# ------------------------------------------------------------
# Test 1: Helper exists and is callable
# ------------------------------------------------------------
puts "\nTest 1: RuntimePackCrashReport.format is available"
assert(defined?(RuntimePackCrashReport), 'RuntimePackCrashReport must be defined')
assert(RuntimePackCrashReport.respond_to?(:format), 'format must be a module method')

# ------------------------------------------------------------
# Test 2: Report carries engine identity
# ------------------------------------------------------------
puts "\nTest 2: Report carries engine and runtime identity"
exception = RuntimeError.new('boom-identity')
report = RuntimePackCrashReport.format(exception, '{0138}:test_script', 1014)

assert(report.include?(RUBY_VERSION), "report must include RUBY_VERSION (#{RUBY_VERSION})")
assert(report.include?(RUBY_PLATFORM), "report must include RUBY_PLATFORM (#{RUBY_PLATFORM})")
assert(report.include?('RuntimeError'), 'report must include exception class')
assert(report.include?('boom-identity'), 'report must include exception message')

# ------------------------------------------------------------
# Test 3: Report carries script attribution
# ------------------------------------------------------------
puts "\nTest 3: Report carries script name and line"
assert(report.include?('{0138}:test_script'), 'report must include script name')
assert(report.include?('1014'), 'report must include script line')

# ------------------------------------------------------------
# Test 4: Report survives missing script attribution
# ------------------------------------------------------------
puts "\nTest 4: Report is usable when script attribution is unknown"
no_script = RuntimePackCrashReport.format(RuntimeError.new('no-script-here'))
assert(no_script.include?('no-script-here'), 'report without script must still carry message')
assert(no_script.include?(RUBY_VERSION), 'report without script must still carry ruby version')

# ------------------------------------------------------------
# Test 5: Report includes the backtrace
# ------------------------------------------------------------
puts "\nTest 5: Report includes a backtrace when available"
begin
  raise ArgumentError, 'with-backtrace'
rescue ArgumentError => e
  traced = RuntimePackCrashReport.format(e)
end
assert(traced.include?('crash_report_format_test'), 'report must include backtrace frames')

# ------------------------------------------------------------
# Test 6: Report is size-capped
# ------------------------------------------------------------
puts "\nTest 6: Report is bounded to 64 KiB"
huge = RuntimeError.new('x' * 200_000)
capped = RuntimePackCrashReport.format(huge)
assert(capped.bytesize <= 64 * 1024, "report must be capped, got #{capped.bytesize} bytes")
assert(capped.include?('[truncated]'), 'capped report must disclose truncation')

# ------------------------------------------------------------
# Test 7: Report does not leak save data or keystore paths
# ------------------------------------------------------------
puts "\nTest 7: Report contains no keystore or save-file references"
assert(!report.downcase.include?('keystore'), 'report must not mention keystore')
assert(!report.downcase.include?('.jks'), 'report must not mention jks files')
assert(!report.include?('files/saves'), 'report must not mention save directory')

# ------------------------------------------------------------
# Test 8: TKG::ErrorLog still writes a usable file
# ------------------------------------------------------------
puts "\nTest 8: TKG::ErrorLog uses the enriched format"
require 'tmpdir'
Dir.mktmpdir do |dir|
  Dir.chdir(dir) do
    begin
      raise 'tkg-path'
    rescue RuntimeError => e
      TKG::ErrorLog.save(e)
    end
    written = File.exist?('Error.log') ? File.read('Error.log', encoding: 'UTF-8') : ''
    assert(written.include?('tkg-path'), 'Error.log must contain the message')
    assert(written.include?(RUBY_VERSION), 'Error.log must contain the ruby version')
  end
end

# ------------------------------------------------------------
# Test 9: Script attribution matches the crashing frame, not frame 0
# ------------------------------------------------------------
puts "\nTest 9: Attribution picks the frame belonging to the evaluated script"
begin
  RuntimePackScriptLoader.eval_script(138, 'Scene_Map', "\n\n\n\nFile.read(\"Audio/BGM/missing-probe.ogg\")\n")
rescue StandardError => e
  name, line = RuntimePackCrashReport.attribute(e, '{0138}:Scene_Map')
end
assert(name == '{0138}:Scene_Map', "attribution must keep the evaluated script, got #{name.inspect}")
assert(line == '5', "attribution must report the game line (5), got #{line.inspect}")

# ------------------------------------------------------------
# Test 10: Attribution returns no line rather than a wrong one
# ------------------------------------------------------------
puts "\nTest 10: Unknown attribution yields nil line, never a bootstrap line"
begin
  raise 'no-script-frame-here'
rescue StandardError => e
  _name, line = RuntimePackCrashReport.attribute(e, '{0999}:NotInBacktrace')
end
assert(line.nil?, "line must be nil when no frame matches, got #{line.inspect}")

# ------------------------------------------------------------
# Test 11: Truncation keeps valid UTF-8
# ------------------------------------------------------------
puts "\nTest 11: Truncation does not split a multibyte character"
wide = RuntimeError.new('Missing assets: ' + ('サウンド.ogg ' * 12_000))
capped_wide = RuntimePackCrashReport.format(wide)
assert(capped_wide.bytesize <= 64 * 1024, "must stay under cap, got #{capped_wide.bytesize}")
assert(capped_wide.valid_encoding?, 'truncated report must remain valid UTF-8')

puts "\n============================================================"
puts "Tests run: #{$tests_run}, Passed: #{$tests_passed}, Failed: #{$tests_failed}"
puts '============================================================'

if $tests_failed > 0
  puts "STATUS: RED (#{$tests_failed} assertions failed)"
  exit 1
else
  puts 'STATUS: GREEN (All crash report format tests passed)'
  exit 0
end

