# frozen_string_literal: false
# regexp_backref_regression_test.rb
# Regression test suite for Regexp#=~ caller-frame backreferences ($~, $&, $1, $2, etc.)
# Reproduces the crash in Dark Elf Historia (line 92) and Vampire Notes (line 95)
# where tomoaky's アニメ付き明かり script failed with:
#   NoMethodError: undefined method 'scan' for nil:NilClass
# caused by Regexp#=~ monkey-patch discarding backreferences in the caller frame.

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

def assert_nil(actual, message = nil)
  msg = message || "Expected nil, got #{actual.inspect}"
  assert(actual.nil?, msg)
end

puts "============================================================"
puts "Running Regexp Backreference Regression Tests"
puts "============================================================"

BOOTSTRAP_FILE = ENV['RUNTIMEPACK_BOOTSTRAP_PATH'] ?
  File.expand_path(ENV['RUNTIMEPACK_BOOTSTRAP_PATH']) :
  File.expand_path('../../main/assets/runtimepack_bootstrap.rb', __dir__)

require BOOTSTRAP_FILE

# ------------------------------------------------------------
# Test 1: Exact Game Crash Scenario (tomoaky's アニメ付き明かり)
# ------------------------------------------------------------
puts "\nTest 1: Exact game crash scenario (setup_light_settings pattern)"
text = "<light red_light_s, 128>"
pattern = /<light\s+(.+?)((?:\s*,\s*\-*\d+\s*)+)>/i

matched = (pattern =~ text)
assert_equal 0, matched, "Pattern must match at position 0"
assert_equal "red_light_s", $1, "Caller-frame $1 must be 'red_light_s'"
assert_equal ", 128", $2, "Caller-frame $2 must be ', 128'"

# Exact crashing expression in game scripts:
# $2.scan(/\-*\d+/).each do |id|
assert $2 != nil, "$2 must not be nil before calling .scan"
ids = begin
  $2.scan(/\-*\d+/)
rescue NoMethodError => e
  assert false, "Calling .scan on $2 raised NoMethodError: #{e.message}"
  []
end
assert_equal ["128"], ids, "$2.scan must extract ['128']"

# Test multi-argument variation
text_multi = "<light blue_glow, 255, -42, 0>"
pattern =~ text_multi
assert_equal "blue_glow", $1, "Multi-arg $1 must be 'blue_glow'"
assert_equal ", 255, -42, 0", $2, "Multi-arg $2 must be ', 255, -42, 0'"
ids_multi = ($2 ? $2.scan(/\-*\d+/) : nil)
assert_equal ["255", "-42", "0"], ids_multi, "Multi-arg $2.scan must extract all numbers"

# ------------------------------------------------------------
# Test 2: General Regex Capture Groups ($1, $2)
# ------------------------------------------------------------
puts "\nTest 2: General regex capture groups ($1, $2)"
/(hello)\s+(world)/i =~ "hello world"
assert_equal "hello", $1, "$1 must capture first group 'hello'"
assert_equal "world", $2, "$2 must capture second group 'world'"

# ------------------------------------------------------------
# Test 3: Multiple Capture Groups ($1..$5)
# ------------------------------------------------------------
puts "\nTest 3: Multiple capture groups ($1..$5)"
/(alpha)-(bravo)-(charlie)-(delta)-(echo)/ =~ "alpha-bravo-charlie-delta-echo"
assert_equal "alpha", $1, "$1 must be 'alpha'"
assert_equal "bravo", $2, "$2 must be 'bravo'"
assert_equal "charlie", $3, "$3 must be 'charlie'"
assert_equal "delta", $4, "$4 must be 'delta'"
assert_equal "echo", $5, "$5 must be 'echo'"

# ------------------------------------------------------------
# Test 4: Special Match Variables ($~, $&, $`, $', $+)
# ------------------------------------------------------------
puts "\nTest 4: Special match variables ($~, $&, $`, $', $+)"
/(quick)\s+(fox)/ =~ "The quick fox jumps"
assert $~.is_a?(MatchData), "$~ must be a MatchData instance"
assert_equal "quick fox", $&, "$& must be full matched text 'quick fox'"
assert_equal "The ", $`, "$` must be prematch 'The '"
assert_equal " jumps", $', "$' must be postmatch ' jumps'"
assert_equal "fox", $+, "$+ must be highest numbered capture 'fox'"

# ------------------------------------------------------------
# Test 5: Non-Match Behavior
# ------------------------------------------------------------
puts "\nTest 5: Non-match behavior"
nomatch_result = (/pattern_not_in_text/ =~ "some completely different text")
assert_nil nomatch_result, "Non-matching regex =~ must return nil"
assert_nil $~, "$~ must be nil after non-match"
assert_nil $1, "$1 must be nil after non-match"

# ------------------------------------------------------------
# Test 6: Method Caller Frame Scoping
# ------------------------------------------------------------
puts "\nTest 6: Method caller frame scoping"
def extract_tag(str)
  /<tag:([a-z]+):(\d+)>/ =~ str
  [$1, $2]
end

tag_captures = extract_tag("<tag:armor:450>")
assert_equal ["armor", "450"], tag_captures, "Method local frame must preserve $1 and $2"

# ------------------------------------------------------------
# Test 7: String#scan Compatibility
# ------------------------------------------------------------
puts "\nTest 7: String#scan compatibility"
scan_result = "id:10, id:20, id:30".scan(/id:(\d+)/)
assert_equal [["10"], ["20"], ["30"]], scan_result, "String#scan must continue to function normally"

# ------------------------------------------------------------
# Test 8: String#scan Caller Frame Backreferences (Script 0251 ダメージ計算式簡略化)
# ------------------------------------------------------------
puts "\nTest 8: String#scan caller frame backreferences (Script 0251)"
def eval_damage_formula_symbol(formula)
  form = formula.dup
  formula.scan(/\:(\w+)/).each do
    assert $1 != nil, "$1 must not be nil in caller frame after scan"
    form = form.sub(":" + $1, "(a.atk * 4 - b.def * 2)")
  end
  form
end

formula_replaced = eval_damage_formula_symbol(":attack")
assert_equal "(a.atk * 4 - b.def * 2)", formula_replaced, "Damage formula symbol must be replaced using caller $1"

# Block-yield form verification
block_captured = []
":atk :def".scan(/\:(\w+)/) { block_captured << $1 }
assert_equal ["atk", "def"], block_captured, "String#scan with block must set $1 per yield"



puts "\n============================================================"
puts "Tests run: #{$tests_run}, Passed: #{$tests_passed}, Failed: #{$tests_failed}"
puts "============================================================"

if $tests_failed > 0
  puts "STATUS: RED (#{$tests_failed} assertions failed)"
  exit 1
else
  puts "STATUS: GREEN (All Regexp =~ caller frame backreference tests passed)"
  exit 0
end
