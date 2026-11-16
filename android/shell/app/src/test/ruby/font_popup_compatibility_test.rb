# frozen_string_literal: true

class Font
  attr_reader :assigned

  def initialize
    @assigned = {}
  end

  [:shadow, :outline, :bold, :italic].each do |name|
    define_method("#{name}=") do |value|
      raise TypeError, "Expected bool" unless value == true || value == false
      @assigned[name] = value
    end
  end
end

module InputUtils
  @keySYM_in_UI = {
    up: "\u2b06",
    down: "\u2b07",
    left: "\u2b05",
    right: "\u27a1",
    confirm: "OK"
  }
end

bootstrap_path = File.expand_path('../../main/assets/runtimepack_bootstrap.rb', __dir__)
require bootstrap_path

[:shadow, :outline, :bold, :italic].each do |name|
  raise "#{name}= original setter alias missing" unless Font.method_defined?("runtimepack_orig_#{name}=".to_sym)
end

font = Font.new
{
  shadow: [[20, true], [0, true], [nil, false], [false, false]],
  outline: [[Object.new, true], [nil, false]],
  bold: [[1, true], [false, false]],
  italic: [["yes", true], [nil, false]]
}.each do |setter, cases|
  cases.each do |value, expected|
    font.public_send("#{setter}=", value)
    raise "#{setter}= did not coerce #{value.inspect}" unless font.assigned[setter] == expected
  end
end

verbose, $VERBOSE = $VERBOSE, nil
load bootstrap_path
$VERBOSE = verbose
font.shadow = 20
raise "repeated bootstrap loading changed coercion" unless font.assigned[:shadow] == true

RuntimePackCompatibility.normalize_input_utils
expected = { up: "Up", down: "Down", left: "Left", right: "Right", confirm: "OK" }
actual = InputUtils.instance_variable_get(:@keySYM_in_UI)
raise "direction labels were not normalized" unless actual == expected

binary_mapping = {
  up: "\xE2\xAC\x86".b,
  down: "\xE2\xAC\x87".b,
  left: "\xE2\xAC\x85".b,
  right: "\xE2\x9E\xA1".b,
  confirm: "OK"
}
InputUtils.instance_variable_set(:@keySYM_in_UI, binary_mapping)
RuntimePackCompatibility.normalize_input_utils
binary_expected = { up: "Up", down: "Down", left: "Left", right: "Right", confirm: "OK" }
raise "binary-encoded direction labels were not normalized: #{binary_mapping.inspect}" unless binary_mapping == binary_expected

puts "font_popup_compatibility_test: PASS"
