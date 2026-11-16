# frozen_string_literal: true

require 'zlib'
require 'open3'
require 'json'
require 'rbconfig'
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

puts "============================================================"
puts "Running Script Source Encoding & Line Mapping Tests"
puts "============================================================"

BOOTSTRAP_PATH = File.expand_path('../../main/assets/runtimepack_bootstrap.rb', __dir__)

# Helper to execute a script fixture through the real production bootstrap loader in an isolated subprocess.
# This runs the actual production load_data loop and eval pathway end-to-end.
def run_production_loader(script_bytes, script_name = "FixtureScript", script_id = 1)
  runner = <<~'RUNNER_RUBY'
    require 'zlib'
    require 'json'

    bootstrap_path = ARGV[0]
    fixture_path   = ARGV[1]
    script_name    = ARGV[2]
    script_id      = ARGV[3].to_i

    raw_bytes = File.binread(fixture_path)
    compressed = Zlib::Deflate.deflate(raw_bytes)
    $production_loader_scripts = [[script_id, script_name, compressed]]

    # Provide load_data mock returning the fixture script so bootstrap loader executes
    def load_data(_path)
      $production_loader_scripts
    end

    begin
      load bootstrap_path
      status_hash = {
        "status"        => "ok",
        "error_class"   => nil,
        "error_message" => nil,
        "error_line"    => nil,
        "assigned_var"  => defined?($fixture_assigned) ? $fixture_assigned : nil,
        "assigned_str"  => defined?($fixture_string) ? $fixture_string : nil,
        "str_encoding"  => defined?($fixture_string) && $fixture_string.is_a?(String) ? $fixture_string.encoding.name : nil,
        "str_bytesize"  => defined?($fixture_string) && $fixture_string.is_a?(String) ? $fixture_string.bytesize : nil
      }
      puts JSON.dump(status_hash)
    rescue Exception => e
      line = nil
      if e.backtrace && e.backtrace.first =~ /:(\d+):in/
        line = $1.to_i
      end
      status_hash = {
        "status"        => "error",
        "error_class"   => e.class.to_s,
        "error_message" => e.message.to_s,
        "error_line"    => line,
        "backtrace"     => e.backtrace ? e.backtrace.first(3) : []
      }
      puts JSON.dump(status_hash)
    end
  RUNNER_RUBY

  Dir.mktmpdir("rp_loader_subproc_") do |tmp|
    runner_path  = File.join(tmp, "runner.rb")
    fixture_path = File.join(tmp, "fixture.bin")
    File.write(runner_path, runner)
    File.binwrite(fixture_path, script_bytes)

    out, err, status = Open3.capture3(RbConfig.ruby, runner_path, BOOTSTRAP_PATH, fixture_path, script_name, script_id.to_s, chdir: tmp)
    json_line = out.lines.map(&:strip).reverse.find { |l| l.start_with?('{') && l.end_with?('}') }
    if json_line
      JSON.parse(json_line)
    else
      { "status" => "crash", "error_class" => "ProcessCrash", "error_message" => "Subprocess failed: #{err}\nStdout: #{out}" }
    end
  end
end

UTF8_BOM = "\xEF\xBB\xBF".b

# ------------------------------------------------------------
# Test 1: UTF-8 BOM + comment + assignment fixture
# (Reproduces historical NameError: undefined local variable or method '﻿' for main in old loader)
# ------------------------------------------------------------
puts "\nTest 1: UTF-8 BOM + comment + assignment fixture executes cleanly without NameError"
bom_fixture = UTF8_BOM + "# Leading comment after UTF-8 BOM\n$fixture_assigned = 100\n".b
res1 = run_production_loader(bom_fixture, "BOMAssignScript", 1)
assert_equal "ok", res1["status"], "BOM script must complete with status 'ok', got #{res1['error_class']}: #{res1['error_message']}"
assert_equal nil, res1["error_class"], "BOM script must not raise NameError"
assert_equal 100, res1["assigned_var"], "BOM script assignment must succeed"

# ------------------------------------------------------------
# Test 2: Controlled error on line N reports line N, not line N+1
# (Line 1 comment, line 2 assignment, line 3 deliberate exception)
# ------------------------------------------------------------
puts "\nTest 2: Controlled error on line 3 reports exact line 3, not line 4 (no N+1 shift)"
line3_fixture = "# line 1: comment\n$fixture_assigned = 200\nraise 'deliberate_line_3_fault'\n".b
res2 = run_production_loader(line3_fixture, "Line3ErrorScript", 2)
assert_equal "error", res2["status"], "Script must raise error"
assert_equal "RuntimeError", res2["error_class"], "Expected RuntimeError"
assert_equal 3, res2["error_line"], "Error on line 3 must report line 3 (prevent N+1 line shift)"

# ------------------------------------------------------------
# Test 3: Line 1 immediate error reports line 1
# ------------------------------------------------------------
puts "\nTest 3: Controlled error on line 1 reports line 1"
line1_fixture = "raise 'immediate_line_1_fault'\n".b
res3 = run_production_loader(line1_fixture, "Line1ErrorScript", 3)
assert_equal "error", res3["status"], "Script must raise error"
assert_equal 1, res3["error_line"], "Error on line 1 must report line 1"

# ------------------------------------------------------------
# Test 4: CRLF line endings error reporting accuracy
# ------------------------------------------------------------
puts "\nTest 4: CRLF line endings report exact line number"
crlf_fixture = "# line 1 comment\r\n# line 2 comment\r\nraise 'crlf_fault_line_3'\r\n".b
res4 = run_production_loader(crlf_fixture, "CRLFErrorScript", 4)
assert_equal "error", res4["status"], "CRLF script must raise error"
assert_equal 3, res4["error_line"], "CRLF error on line 3 must report line 3"

# ------------------------------------------------------------
# Test 5: Plain UTF-8 source without BOM evaluates cleanly
# ------------------------------------------------------------
puts "\nTest 5: Plain UTF-8 without BOM evaluates cleanly"
plain_fixture = "# Plain UTF-8 script\n$fixture_assigned = 300\n".b
res5 = run_production_loader(plain_fixture, "PlainScript", 5)
assert_equal "ok", res5["status"], "Plain script must succeed"
assert_equal 300, res5["assigned_var"], "Plain script assignment must succeed"

# ------------------------------------------------------------
# Test 6: UTF-8 non-ASCII characters preserved with UTF-8 encoding
# ------------------------------------------------------------
puts "\nTest 6: UTF-8 non-ASCII characters preserved with UTF-8 encoding"
utf8_chars_fixture = "# UTF-8 script with Japanese text\n$fixture_string = '日本語テスト'\n".b
res6 = run_production_loader(utf8_chars_fixture, "UTF8CharsScript", 6)
assert_equal "ok", res6["status"], "UTF-8 non-ASCII script must succeed"
assert_equal "日本語テスト", res6["assigned_str"], "UTF-8 string value must match"
assert_equal "UTF-8", res6["str_encoding"], "String encoding must be UTF-8"

# ------------------------------------------------------------
# Test 7: Existing magic comments in body (e.g. # encoding: shift_jis)
# Must not trigger SyntaxError: invalid multibyte char (Shift_JIS)
# ------------------------------------------------------------
puts "\nTest 7: Existing magic comment in body does not cause SyntaxError"
magic_comment_fixture = "# encoding: shift_jis\n$fixture_string = '日本語テキスト'\n".b
res7 = run_production_loader(magic_comment_fixture, "MagicCommentScript", 7)
assert_equal "ok", res7["status"], "Script with existing magic comment must evaluate cleanly"
assert_equal "日本語テキスト", res7["assigned_str"], "String with existing magic comment must be UTF-8 text"

# ------------------------------------------------------------
# Test 8: Interior U+FEFF preserved intact
# ------------------------------------------------------------
puts "\nTest 8: Interior U+FEFF inside string is preserved (not stripped)"
interior_feff_fixture = UTF8_BOM + "# Leading BOM\n$fixture_string = \"prefix\uFEFFsuffix\"\n".b
res8 = run_production_loader(interior_feff_fixture, "InteriorFEFFScript", 8)
assert_equal "ok", res8["status"], "Script with interior U+FEFF must succeed"
# "prefix" (6) + U+FEFF (3 bytes in UTF-8: EF BB BF) + "suffix" (6) = 15 bytes
assert_equal 15, res8["str_bytesize"], "Interior U+FEFF bytes must remain intact (15 bytes total)"
assert res8["assigned_str"] && res8["assigned_str"].include?("\uFEFF"), "Interior U+FEFF must be present in string"

# ------------------------------------------------------------
# Test 9: Invalid UTF-8 bytes fallback to Shift_JIS
# ------------------------------------------------------------
puts "\nTest 9: Invalid UTF-8 bytes fallback to Shift_JIS (legacy compatibility semantics)"
# "日本語" encoded in Shift_JIS: \x93\xFA\x96\x7B\x8C\xEA
sjis_raw = ("$fixture_string = '" + "\x93\xFA\x96\x7B\x8C\xEA" + "'\n").b
res9 = run_production_loader(sjis_raw, "ShiftJISFallbackScript", 9)
assert_equal "ok", res9["status"], "Shift_JIS fallback script must evaluate cleanly"
assert_equal "日本語", res9["assigned_str"], "Shift_JIS bytes must be decoded to UTF-8 '日本語'"

# ------------------------------------------------------------
# Test 10: Completely corrupt bytes handle safely without crash
# ------------------------------------------------------------
puts "\nTest 10: Corrupt bytes handled safely"
corrupt_raw = ("# Comment\n$fixture_string = '" + "\xFF\xFF\xFF" + "'\n").b
res10 = run_production_loader(corrupt_raw, "CorruptScript", 10)
assert_equal "ok", res10["status"], "Corrupt bytes fallback must not crash loader"

# ------------------------------------------------------------
# Test 11: Production preparation unit contract (RuntimePackScriptLoader)
# ------------------------------------------------------------
puts "\nTest 11: Production preparation unit contract (RuntimePackScriptLoader)"
# Require the production bootstrap in-process to test unit methods
require BOOTSTRAP_PATH

assert defined?(RuntimePackScriptLoader), "RuntimePackScriptLoader must be defined in bootstrap"

if defined?(RuntimePackScriptLoader)
  # Contract A: Leading BOM stripped
  src_with_bom = UTF8_BOM + "A = 1\n".b
  prep1 = RuntimePackScriptLoader.prepare_script_source(src_with_bom)
  assert_equal false, prep1.start_with?(UTF8_BOM), "prepare_script_source must strip leading UTF-8 BOM"
  assert prep1.start_with?("# encoding: utf-8\n"), "prepare_script_source must prepend UTF-8 magic comment"

  # Contract B: Body text unchanged
  body_text = "def calculate_score\n  return 42\nend\n"
  prep2 = RuntimePackScriptLoader.prepare_script_source(body_text)
  assert_equal "# encoding: utf-8\n" + body_text, prep2, "prepare_script_source must not alter body text"

  # Contract C: Interior U+FEFF preserved
  interior_src = "STR = 'hello\uFEFFworld'\n".b
  prep3 = RuntimePackScriptLoader.prepare_script_source(UTF8_BOM + interior_src)
  assert prep3.include?("\uFEFF"), "prepare_script_source must preserve interior U+FEFF"

  # Contract D: Eval start line is 0
  assert_equal 0, RuntimePackScriptLoader.eval_start_line, "eval_start_line must be 0 to map original line 1 to line 1"

  # Contract E: Direct eval_script evaluates with correct line numbering
  begin
    RuntimePackScriptLoader.eval_script(99, "UnitErrorScript", "# l1\n# l2\nraise 'unit_error_line_3'\n")
    assert false, "eval_script must raise exception"
  rescue RuntimeError => e
    line = e.backtrace.first =~ /:(\d+):in/ ? $1.to_i : nil
    assert_equal 3, line, "eval_script must report line 3"
  end

  # Contract F: Byte-level BOM trimming on invalid UTF-8 byte sequence avoids ArgumentError
  class FailingScriptString < String
    def to_s; self; end
    def dup
      c = FailingScriptString.new(super)
      c.force_encoding(encoding)
      c
    end
    def encode(*args, **kwargs)
      raise Encoding::ConverterNotFoundError, "code converter not found"
    end
  end

  # When encoding conversion fails and leaves invalid bytes, prepare_script_source must not raise ArgumentError
  fs = FailingScriptString.new(("\xFF\xFF" + "invalid_script = 1\n").b)
  fs.force_encoding("UTF-8")

  prep_invalid = nil
  begin
    prep_invalid = RuntimePackScriptLoader.prepare_script_source(fs)
    assert true, "prepare_script_source on invalid UTF-8 must not raise ArgumentError"
  rescue ArgumentError => e
    assert false, "prepare_script_source raised ArgumentError: #{e.message} (must use byte-level BOM trimming)"
  end
  assert prep_invalid&.start_with?("# encoding: utf-8\n"), "prepare_script_source must prepend UTF-8 magic comment"

  # Evaluating invalid syntax must raise SyntaxError, not ArgumentError
  begin
    RuntimePackScriptLoader.eval_script(100, "InvalidByteScript", fs)
    assert false, "eval_script on invalid byte syntax must raise SyntaxError"
  rescue SyntaxError
    assert true, "eval_script on invalid byte sequence raises SyntaxError as expected"
  rescue Exception => e
    assert false, "eval_script raised #{e.class} instead of SyntaxError: #{e.message}"
  end
end

puts "\n============================================================"
puts "Tests run: #{$tests_run}, Passed: #{$tests_passed}, Failed: #{$tests_failed}"
puts "============================================================"

if $tests_failed > 0
  puts "STATUS: RED (#{$tests_failed} assertions failed)"
  exit 1
else
  puts "STATUS: GREEN (All script source encoding and line mapping tests passed)"
  exit 0
end
