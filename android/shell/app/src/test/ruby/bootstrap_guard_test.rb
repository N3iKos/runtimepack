# frozen_string_literal: false
# bootstrap_guard_test.rb
#
# Static guard: runtimepack_bootstrap.rb must never re-wrap standard Ruby
# methods. Two self-inflicted crash classes came from exactly that:
#
#   * Regexp#=~  -- a re-implementation lost $~ / $1..$n in the CALLER frame,
#                   so game code doing `pattern =~ text; $2.scan(...)` blew up
#                   with "undefined method 'scan' for nil:NilClass"
#                   (Dark Elf Historia line 92, Vampire Notes line 95).
#   * String#scan -- same caller-frame backreference loss, broke Script 0251
#                    (ダメージ計算式簡略化) damage-formula symbol replacement.
#
# This test reads the bootstrap as TEXT and never executes it. It fails when a
# `scan` or `=~` override is reintroduced through any idiom Ruby offers:
#   def scan(...)            def =~(...)
#   def self.scan(...)       def self.=~(...)
#   alias scan other         alias_method :scan, :other
#   define_method(:scan)     String.class_eval { def scan ... }
#
# Allowed and deliberately NOT flagged: ordinary `def` methods the bootstrap
# legitimately defines (Array#to_s, Hash#index, NilClass#id, Bitmap#dispose,
# ...) and the `alias_method` captures on File / Cache / Audio / Input /
# Graphics. Only String#scan and Regexp#=~ overrides are forbidden.
#
# SCOPE AND LIMITS -- READ BEFORE TRUSTING THIS FILE
#
# This is a HELPER LINT, not a merge gate. It is not wired into Gradle or CI;
# nothing runs it automatically. It runs only when someone invokes it.
#
# The real regression gate for this bug class is the behavioural test
# `regexp_backref_regression_test.rb`, which requires the bootstrap, executes
# it, and fails hard on the historical wrapper (verified against f654402^:
# 20+ assertion failures, including the exact $2.scan crash). That test is
# what actually protects the runtime.
#
# Known limits, verified against this file (do not assume they are fixed):
#   * `String.send(:define_method, :scan) { ... }` -- MISSED (proven to
#     override at runtime while this guard reports PASS)
#   * `String.class_eval("def scan ...")` with a STRING body -- MISSED
#   * `alias_method "scan", :other` with a STRING name -- MISSED
#   * An override placed in a `require_relative`d file -- out of scope
#   * A `def scan` on a NON-String class is currently FLAGGED even though it
#     is legitimate (the AST pass does not gate on the enclosing class)
#
# Treat a PASS here as "the obvious textual shapes are absent", never as
# "the runtime cannot be broken".
#
# ANTI-DECORATION: the guard self-tests every run. It re-proves that its own
# detectors still fire on known-violating fixtures and stay silent on
# known-benign fixtures. Delete a pattern and the guard fails immediately,
# because a guard that cannot fail is decoration. Caveat: the fixtures prove
# the UNION of the lexical and AST passes, not each pass separately, so one
# pass could be weakened without the self-test noticing.

require 'ripper'

BOOTSTRAP_PATH = begin
  raw = ENV['RUNTIMEPACK_BOOTSTRAP_PATH']
  if raw && !raw.strip.empty?
    File.expand_path(raw.strip)
  else
    File.expand_path('../../main/assets/runtimepack_bootstrap.rb', __dir__)
  end
end

# ---------------------------------------------------------------------------
# Detector definitions
# ---------------------------------------------------------------------------

# method name => human reason (message text is asserted by the plan)
FORBIDDEN = {
  'scan' => 'String#scan override destroys caller $1/$~',
  '=~'   => 'Regexp#=~ override destroys caller backreferences'
}.freeze

ALIAS_CALLS = %w[alias_method define_method define_singleton_method].freeze

# A token node looks like [:@ident, "scan", [line, col]] / [:@op, "=~", ...].
def ident_pair(node)
  return [nil, nil] unless node.is_a?(Array)
  return [nil, nil] unless node[0].to_s.start_with?('@')
  line = node[2].is_a?(Array) ? node[2][0] : nil
  [node[1].to_s, line]
end

def const_name(node)
  return '' unless node.is_a?(Array)
  case node[0]
  when :const_ref, :top_const_ref, :var_ref, :const_path_field
    node[1].is_a?(Array) ? node[1][1].to_s : node[1].to_s
  when :const_path_ref
    const_name(node[2])
  else
    node[1].is_a?(Array) ? node[1][1].to_s : ''
  end
end

# Resolve a symbol literal to its plain name.
#   :scan          => :symbol        -> [:@ident, "scan"]
#   :'=~'          => :dyna_symbol   -> :string_content -> [:@tstring_content, "=~"]
#   :"=~"          => :dyna_symbol   (same shape; still statically resolvable)
# Truly interpolated symbols (:"a#{b}") are unresolvable statically and yield nil.
def symbol_name(node)
  return [nil, nil] unless node.is_a?(Array)
  case node[0]
  when :symbol_literal
    symbol_name(node[1])
  when :symbol
    ident_pair(node[1])
  when :dyna_symbol
    contents = node[1]
    return [nil, nil] unless contents.is_a?(Array) && contents[0] == :string_content
    parts = contents[1..].select { |c| c.is_a?(Array) }
    return [nil, nil] unless parts.size == 1 && parts[0][0] == :@tstring_content
    [parts[0][1].to_s, parts[0][2].is_a?(Array) ? parts[0][2][0] : nil]
  else
    [nil, nil]
  end
end

def collect_symbols(node, acc)
  return unless node.is_a?(Array)
  if node[0] == :symbol_literal || node[0] == :dyna_symbol
    name, line = symbol_name(node)
    acc << [name, line] if name
  end
  node.each { |child| collect_symbols(child, acc) }
end

def callee(node)
  return [nil, nil] unless node.is_a?(Array)
  case node[0]
  when :command, :fcall, :vcall then ident_pair(node[1])
  when :method_add_arg           then callee(node[1])
  else [nil, nil]
  end
end

# Record a violation once (dedupe by method+line+idiom so nested call nodes
# reached twice do not double-report).
def hit(out, name, line, idiom, ctx)
  return unless name
  return unless FORBIDDEN.key?(name)
  return if out.any? { |r| r[:method] == name && r[:line] == line && r[:idiom] == idiom }
  out << { method: name, line: line, idiom: idiom, ctx: ctx.to_s }
end

# Only the FIRST symbol of an alias/define_method call names the method being
# (re)defined. `alias_method :rp_orig_scan, :scan` merely captures the original
# and is allowed; `alias_method :scan, :rp_orig_scan` overrides and is not.
def first_symbol_name(node)
  return [nil, nil] unless node.is_a?(Array)
  syms = []
  collect_symbols(node, syms)
  syms.first || [nil, nil]
end

# AST pass: exact, handles quoted symbols, class_eval blocks, singletons.
def ast_violations(ast)
  out = []
  walk = lambda do |node, ctx|
    next unless node.is_a?(Array)
    case node[0]
    when :class
      inner = "#{ctx}#{const_name(node[1])}"
      walk.call(node[1], ctx)
      walk.call(node[2], ctx) if node[2]
      walk.call(node[3], inner)
    when :module
      inner = "#{ctx}#{const_name(node[1])}"
      walk.call(node[1], ctx)
      walk.call(node[2], inner)
    when :sclass
      inner = "#{ctx}<<#{const_name(node[1])}"
      walk.call(node[1], ctx)
      walk.call(node[2], inner)
    when :def
      name, line = ident_pair(node[1])
      hit(out, name, line, 'def', ctx)
      node.each_with_index { |child, i| walk.call(child, ctx) if i >= 2 }
    when :defs
      name, line = node[3].is_a?(Array) ? ident_pair(node[3]) : [nil, nil]
      hit(out, name, line, 'def singleton', ctx)
      node.each_with_index { |child, i| walk.call(child, ctx) if i >= 4 }
    when :alias
      n_new, l_new = symbol_name(node[1])
      hit(out, n_new, l_new, 'alias', ctx)
      node.each { |child| walk.call(child, ctx) }
    else
      cname, cline = callee(node)
      if cname && ALIAS_CALLS.include?(cname)
        args = case node[0]
               when :command       then node[2]
               when :method_add_arg then node[2]
               end
        sname, sline = first_symbol_name(args)
        hit(out, sname, sline || cline, cname, ctx)
      end
      node.each { |child| walk.call(child, ctx) }
    end
  end
  walk.call(ast, '')
  out
end

# Replace comments and string/regexp bodies with blanks, keeping newlines and
# character offsets intact so line numbers stay truthful. Quoted symbol bodies
# (:'scan') are preserved so alias targets remain visible.
def mask_source(src)
  masked = +''
  in_quoted_symbol = false
  Ripper.lex(src).each do |(_pos, type, text, _state)|
    case type
    when :on_symbeg
      in_quoted_symbol = (text != ':')
      masked << text
    when :on_tstring_end, :on_regexp_end
      in_quoted_symbol = false
      masked << text
    when :on_comment, :on_embdoc_beg, :on_embdoc, :on_embdoc_end
      masked << text.gsub(/[^\n]/, ' ')
    when :on_tstring_content
      masked << (in_quoted_symbol ? text : text.gsub(/[^\n]/, ' '))
    else
      masked << text
    end
  end
  masked
end

# Lexical pass: second opinion, also the only thing available if Ripper cannot
# parse. Runs on masked text so prose in comments/strings cannot trip it.
# Quoted symbol forms (:'=~') survive masking, so the patterns accept an
# optional quote after the colon.
#
# The alias_method pattern is anchored to the FIRST symbol argument on the
# line, because only that names the method being (re)defined. Capturing the
# original -- `alias_method :rp_orig_scan, :scan` -- is allowed.
def lexical_violations(src)
  out = []
  masked = mask_source(src)
  masked.each_line.with_index(1) do |line, lineno|
    if (m = line.match(/^[ \t]*def[ \t]+(?:self[ \t]*\.[ \t]*)?(scan|=~)(?![A-Za-z0-9_])/))
      hit(out, m[1], lineno, 'def (lexical)', '')
    end
    if (m = line.match(/\balias_method\b[ \t]*\(?[ \t]*:['"]?(scan|=~)(?![A-Za-z0-9_])/))
      hit(out, m[1], lineno, 'alias_method (lexical)', '')
    end
    if (m = line.match(/\bdefine_method\b[ \t]*\(?[ \t]*:['"]?(scan|=~)(?![A-Za-z0-9_])/))
      hit(out, m[1], lineno, 'define_method (lexical)', '')
    end
    if (m = line.match(/^[ \t]*alias[ \t]+(?::['"]?)?(scan|=~)(?![A-Za-z0-9_])/))
      hit(out, m[1], lineno, 'alias (lexical)', '')
    end
  end
  out
end

# Union of both passes, deduped by method+line. Never reports the same
# reintroduced override twice just because two passes saw it.
def violations_in(src)
  ast = Ripper.sexp(src)
  ast_hits = ast ? ast_violations(ast) : []
  lex_hits = lexical_violations(src)
  merged = []
  (ast_hits + lex_hits).each do |v|
    next if merged.any? { |m| m[:method] == v[:method] && m[:line] == v[:line] }
    merged << v
  end
  merged.sort_by { |v| [v[:line].to_i, v[:method]] }
end

# ---------------------------------------------------------------------------
# Anti-decoration: prove the detectors still work, every single run.
# ---------------------------------------------------------------------------

VIOLATING_FIXTURES = {
  'Regexp#=~ def (probe shape)' => <<~RB,
    class Regexp
      def =~(s); end
    end
  RB
  'String#scan def' => <<~RB,
    class String
      def scan(pattern, &block)
        pattern
      end
    end
  RB
  'historical wrapper (alias + def)' => <<~RB,
    class Regexp
      unless method_defined?(:rp_orig_match_op)
        alias_method :rp_orig_match_op, :=~ rescue nil
        def =~(str)
          rp_orig_match_op(str)
        end
      end
    end
    class String
      unless method_defined?(:rp_orig_scan)
        alias_method :rp_orig_scan, :scan rescue nil
        def scan(pattern, &block)
          rp_orig_scan(pattern, &block)
        end
      end
    end
  RB
  'singleton def self.scan' => <<~RB,
    def self.scan(thing)
      thing
    end
  RB
  'define_method(:scan)' => <<~RB,
    class String
      define_method(:scan) { |pattern| pattern }
    end
  RB
  'alias re-points scan' => <<~RB,
    class String
      alias scan scan_impl
    end
  RB
  'class_eval block def scan' => <<~RB,
    String.class_eval do
      def scan(pattern)
        pattern
      end
    end
  RB
  'quoted symbol alias_method' => <<~RB,
    class Regexp
      alias_method :'=~', :rp_orig
    end
  RB
  'alias_method overrides scan (re-point)' => <<~RB,
    class String
      alias_method :scan, :rp_orig_scan
    end
  RB
  'define_method with quoted scan symbol' => <<~RB,
    class String
      define_method(:'scan') { |pattern| pattern }
    end
  RB
}.freeze

BENIGN_FIXTURES = {
  'Array#to_s (allowed)' => <<~RB,
    class Array
      def to_s
        join
      end
    end
  RB
  'File alias capture (allowed)' => <<~RB,
    class << File
      alias_method :runtimepack_original_read, :read
      alias_method :runtimepack_original_binread, :binread if method_defined?(:binread)
    end
  RB
  'Graphics alias capture (allowed)' => <<~RB,
    class << Graphics
      alias_method :rp_orig_update, :update rescue nil
    end
  RB
  'Audio module alias capture (allowed)' => <<~RB,
    module Audio
      class << self
        alias_method :rp_orig_bgm_play, :bgm_play rescue nil
      end
    end
  RB
  'Input alias capture (allowed)' => <<~RB,
    class << Input
      alias_method :rp_native_press?, :press? rescue nil
    end
  RB
  'Kernel singleton helpers (allowed)' => <<~RB,
    module Kernel
      def self.puts(*args)
        Kernel.puts(*args)
      end
      def self.const_missing(const_name)
        nil
      end
    end
  RB
  'comment naming the forbidden methods' => <<~RB,
    # Never add `def scan` or `def =~` here, and never alias_method :scan, :x
    class Array
      def to_s
        join
      end
    end
  RB
  'string literal naming the forbidden methods' => <<~RB,
    MESSAGE = "do not def scan( or def =~ anywhere in this file"
    class Array
      def to_s
        join
      end
    end
  RB
  'capturing the original scan is allowed' => <<~RB,
    class String
      alias_method :rp_orig_scan, :scan rescue nil
      define_method(:__rp_scan_patched) { true }
    end
  RB
  'capturing the original =~ is allowed' => <<~RB,
    class Regexp
      alias_method :rp_orig_match_op, :=~ rescue nil
      alias_method :rp_orig_match_op2, :'=~' rescue nil
    end
  RB
}.freeze

self_test_failures = []

VIOLATING_FIXTURES.each do |label, code|
  found = violations_in(code)
  if found.empty?
    self_test_failures << "detector missed known-violating fixture: #{label}"
  end
end

BENIGN_FIXTURES.each do |label, code|
  found = violations_in(code)
  unless found.empty?
    detail = found.map { |v| "#{v[:method]}@line#{v[:line]} via #{v[:idiom]}" }.join(', ')
    self_test_failures << "detector false-positived on benign fixture: #{label} (#{detail})"
  end
end

unless self_test_failures.empty?
  self_test_failures.each { |f| warn "FAIL: #{f}" }
  warn 'FAIL: guard self-test broken -- detectors are not trustworthy, refusing to certify bootstrap'
  exit 1
end

# ---------------------------------------------------------------------------
# Real check
# ---------------------------------------------------------------------------

unless File.file?(BOOTSTRAP_PATH)
  warn "FAIL: bootstrap not found at #{BOOTSTRAP_PATH}"
  exit 1
end

source = File.read(BOOTSTRAP_PATH, encoding: 'UTF-8')

unless source.valid_encoding?
  warn "FAIL: bootstrap is not valid UTF-8 (#{BOOTSTRAP_PATH}) -- cannot certify, failing closed"
  exit 1
end

if source.bytesize < 40_000
  warn "FAIL: bootstrap is implausibly small (#{source.bytesize} bytes) -- refusing to certify a stub, failing closed"
  exit 1
end

ast = Ripper.sexp(source)
if ast.nil?
  warn "FAIL: bootstrap could not be parsed as Ruby -- cannot certify, failing closed"
  exit 1
end

def_node_count = 0
count_defs = lambda do |node|
  next unless node.is_a?(Array)
  def_node_count += 1 if node[0] == :def || node[0] == :defs
  node.each { |child| count_defs.call(child) }
end
count_defs.call(ast)

if def_node_count < 20
  warn "FAIL: only #{def_node_count} method definitions parsed -- AST looks truncated, failing closed"
  exit 1
end

violations = violations_in(source)

if violations.empty?
  puts 'PASS: no forbidden stdlib wrapping in bootstrap'
  puts "  file:      #{BOOTSTRAP_PATH}"
  puts "  size:      #{source.bytesize} bytes, #{def_node_count} method definitions"
  puts "  forbidden: #{FORBIDDEN.keys.map { |k| k == '=~' ? 'Regexp#=~' : 'String#scan' }.join(', ')}"
  puts "  self-test: #{VIOLATING_FIXTURES.size} violating fixtures caught, #{BENIGN_FIXTURES.size} benign fixtures clean"
  puts '  note:      helper lint only -- the behavioural gate is regexp_backref_regression_test.rb'
  exit 0
else
  violations.each do |v|
    where = v[:ctx].empty? ? '' : " in #{v[:ctx]}"
    warn "FAIL: #{FORBIDDEN[v[:method]]} (line #{v[:line]}, via #{v[:idiom]}#{where})"
  end
  warn "FAIL: #{violations.size} forbidden stdlib override(s) reintroduced -- see Global Constraints"
  exit 1
end
