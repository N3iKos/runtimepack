# RuntimePack post-binding bootstrap for RGSS3 Android.
# This script is executed by mkxp-z after native bindings are initialized.

Encoding.default_external = Encoding::UTF_8
Encoding.default_internal = Encoding::UTF_8

# Ruby 1.8 compatibility helpers for RGSS1 (RPG Maker XP) and RGSS2 (RPG Maker VX)
class NilClass
  def id
    4
  end unless method_defined?(:id)

  def type
    "NilClass"
  end unless method_defined?(:type)

  def empty?
    true
  end unless method_defined?(:empty?)
end

class Hash
  def index(*args)
    key(*args)
  end unless method_defined?(:index)
end

class Array
  def to_s
    join
  end
end

module MKXP
  def self.puts(*args)
    Kernel.puts(*args)
  end

  def self.print(*args)
    Kernel.print(*args)
  end

  def self.p(*args)
    Kernel.p(*args)
  end

  def self.raw_key_states
    "\0" * 256
  end

  def self.data_directory
    "./."
  end

  def self.save_directory
    "./."
  end

  def self.custom_save_directory
    "./."
  end

  def self.title
    "To the Moon"
  end

  def self.title=(t)
  end

  def self.platform
    "android"
  end

  def self.ready
    true
  end

  def self.method_missing(method_name, *args, &block)
    nil
  end

  def self.const_missing(const_name)
    nil
  end
end

module Kernel
  alias_method :runtimepack_orig_open, :open rescue nil
  def open(name, *args, &block)
    if args.first == "w"
      args[0] = "wb"
    end
    if respond_to?(:runtimepack_orig_open)
      runtimepack_orig_open(name, *args, &block)
    else
      File.open(name, *args, &block)
    end
  end
end

module RuntimePackCrashReport
  MAX_BYTES = 64 * 1024

  # Find the (script_name, line) pair that actually belongs to the script being
  # evaluated. Frame 0 is often the bootstrap itself (every File.read goes
  # through a wrapper), so trusting it attributes the crash to the wrong file.
  # Returns [name, nil] rather than a wrong line when nothing matches.
  def self.attribute(exception, script_name = nil)
    return [script_name, nil] unless exception

    frames = exception.backtrace || []
    if script_name
      frames.each do |frame|
        s = frame.to_s
        next unless s.include?(script_name)
        line = s[/:(\d+):/, 1]
        return [script_name, line] if line
      end
      return [script_name, nil]
    end

    [nil, nil]
  end

  # Build a bounded, attributable crash report. Identity fields first so the
  # first lines of Error.log always answer "which build, which engine, where".
  def self.format(exception, script_name = nil, script_line = nil)
    return '' unless exception

    header = []
    header << "time:      #{Time.now.utc.strftime('%Y-%m-%dT%H:%M:%SZ')}"
    header << "ruby:      #{RUBY_VERSION} (#{RUBY_PLATFORM})"
    header << "engine:    RuntimePack RGSS"
    header << "script:    #{script_name}" if script_name
    header << "line:      #{script_line}" if script_line
    header << "exception: #{exception.class}"
    header << "message:   #{exception.message}"

    body = header.join("\n")

    trace = exception.backtrace
    if trace && !trace.empty?
      body += "\n\nbacktrace:\n" + trace.join("\n")
    end

    return body if body.bytesize <= MAX_BYTES

    # Slice on a character boundary: a raw byteslice can cut a multibyte
    # character in half and leave Error.log with invalid encoding.
    kept = body.byteslice(0, MAX_BYTES - 16).to_s
    kept = kept.scrub('') unless kept.valid_encoding?
    kept + "\n[truncated]\n"
  end
end

module TKG
  module ErrorLog
    def self.save(exception = $!)
      return unless exception
      msg = RuntimePackCrashReport.format(exception)
      p msg
      begin
        File.open("Error.log", "wb") { |f| f.write(msg) }
      rescue => ex
        p "Failed writing Error.log: #{ex}"
      end
    end
  end
end

begin
  require 'zlib'
rescue LoadError, ScriptError, StandardError
end

begin
  require 'json'
rescue LoadError, ScriptError, StandardError
end

begin
  require_relative 'runtimepack_asset_audit'
rescue Exception
  begin
    require './runtimepack_asset_audit'
  rescue Exception
    begin
      audit_file = File.expand_path('runtimepack_asset_audit.rb', File.dirname(__FILE__))
      load audit_file if File.file?(audit_file)
    rescue Exception
    end
  end
end

class << File
  unless method_defined?(:runtimepack_original_exist?)
    alias_method :runtimepack_original_exist?, :exist?
    alias_method :runtimepack_original_size, :size
    alias_method :runtimepack_original_file?, :file?
    alias_method :runtimepack_original_directory?, :directory?
    alias_method :runtimepack_original_open, :open
    alias_method :runtimepack_original_read, :read
    alias_method :runtimepack_original_binread, :binread if method_defined?(:binread)
  end
end

module RuntimePackCasePath
  IMAGE_EXTENSIONS = ['.png', '.jpg', '.jpeg', '.bmp'].freeze

  def self.resolve(path)
    return path unless path.is_a?(String)
    return path if File.runtimepack_original_exist?(path)

    normalized = path.tr('\\', '/')
    return path if normalized.start_with?('/')

    current = '.'
    normalized.split('/').reject { |part| part.empty? || part == '.' }.each do |segment|
      entries = Dir.entries(current)
      actual = entries.find { |entry| entry.casecmp(segment).zero? }
      return path unless actual
      current = File.join(current, actual)
    end
    current
  rescue
    path
  end

  def self.resolve_bitmap_path(path)
    return path unless path.is_a?(String)
    return path if path.empty?

    normalized = path.tr('\\', '/')
    return path if normalized.start_with?('/') || normalized.include?('../') || normalized =~ /\A[a-zA-Z]:/ || normalized =~ /\A[a-zA-Z][a-zA-Z0-9+.-]*:\/\//

    segments = normalized.split('/').reject { |p| p.empty? || p == '.' }
    return path if segments.empty?

    root_path = (File.realpath('.') rescue Dir.pwd).tr('\\', '/')
    current = '.'
    resolved_segments = []

    # Traverse directory segments with unique-only case fold & symlink containment
    segments[0...-1].each do |seg|
      cur_real = (File.realpath(current) rescue nil)
      if cur_real
        cur_norm = cur_real.tr('\\', '/')
        return path unless cur_norm == root_path || cur_norm.start_with?(root_path + '/')
      end

      entries = Dir.entries(current) rescue nil
      return path unless entries

      match = nil
      if entries.include?(seg)
        match = seg
      else
        matches = entries.select { |e| e.casecmp(seg)&.zero? }
        return path unless matches.size == 1
        match = matches.first
      end

      current = File.join(current, match)
      resolved_segments << match
    end

    cur_real = (File.realpath(current) rescue nil)
    if cur_real
      cur_norm = cur_real.tr('\\', '/')
      return path unless cur_norm == root_path || cur_norm.start_with?(root_path + '/')
    end

    leaf = segments.last
    entries = Dir.entries(current) rescue nil
    return path unless entries

    raw_ext = File.extname(leaf)
    has_image_ext = !raw_ext.empty? && IMAGE_EXTENSIONS.include?(raw_ext.downcase)

    leaf_stem = has_image_ext ? leaf[0...-raw_ext.length] : leaf
    actual_leaf = nil

    # Pass 1: Exact matches across candidates
    if entries.include?(leaf)
      actual_leaf = leaf
    elsif !has_image_ext
      exact_cand = IMAGE_EXTENSIONS.map { |ext| leaf_stem + ext }.find { |target| entries.include?(target) }
      actual_leaf = exact_cand if exact_cand
    end

    # Pass 2: Case-folded matches across candidates if no exact match
    unless actual_leaf
      if has_image_ext
        matches = entries.select { |e| e.casecmp(leaf)&.zero? }
        actual_leaf = matches.first if matches.size == 1
      else
        IMAGE_EXTENSIONS.each do |ext|
          target = leaf_stem + ext
          matches = entries.select { |e| e.casecmp(target)&.zero? }
          if matches.size == 1
            actual_leaf = matches.first
            break
          elsif matches.size > 1
            exact = matches.find { |e| e == target }
            if exact
              actual_leaf = exact
              break
            end
            return path # Ambiguous collision
          end
        end
      end
    end

    return path unless actual_leaf

    cand_path = File.join(current, actual_leaf)
    cand_real = (File.realpath(cand_path) rescue nil)
    if cand_real
      cand_norm = cand_real.tr('\\', '/')
      return path unless cand_norm == root_path || cand_norm.start_with?(root_path + '/')
    end

    resolved_segments << actual_leaf
    resolved_segments.join('/')
  rescue
    path
  end
end

class << File
  def exist?(path)
    runtimepack_original_exist?(RuntimePackCasePath.resolve(path))
  end

  alias_method :exists?, :exist?

  def size(path)
    runtimepack_original_size(RuntimePackCasePath.resolve(path))
  end

  def file?(path)
    runtimepack_original_file?(RuntimePackCasePath.resolve(path))
  end

  def directory?(path)
    runtimepack_original_directory?(RuntimePackCasePath.resolve(path))
  end

  def open(path, *args, **kwargs, &block)
    resolved = RuntimePackCasePath.resolve(path)
    if kwargs.empty?
      runtimepack_original_open(resolved, *args, &block)
    else
      runtimepack_original_open(resolved, *args, **kwargs, &block)
    end
  end

  def read(path, *args, **kwargs)
    resolved = RuntimePackCasePath.resolve(path)
    if kwargs.empty?
      runtimepack_original_read(resolved, *args)
    else
      runtimepack_original_read(resolved, *args, **kwargs)
    end
  end

  if method_defined?(:runtimepack_original_binread)
    def binread(path, *args, **kwargs)
      resolved = RuntimePackCasePath.resolve(path)
      if kwargs.empty?
        runtimepack_original_binread(resolved, *args)
      else
        runtimepack_original_binread(resolved, *args, **kwargs)
      end
    end
  end
end

module DL
  def self.malloc(size)
    "\0" * size
  end

  class CPtr
    @instances = {}
    class << self
      attr_accessor :instances
    end

    def initialize(pointer, size = nil)
      @data = pointer.is_a?(String) ? pointer : ("\0" * (size || 0))
      @data = @data.ljust(size, "\0") if size && @data.bytesize < size
      (DL::CPtr.instances ||= {})[object_id] = self
    end

    def [](index)
      @data.getbyte(index) || 0
    end

    def []=(index, value)
      @data = @data.ljust(index + 1, "\0") if index >= @data.bytesize
      @data.setbyte(index, value.to_i & 0xff)
    end

    def to_i
      object_id
    end
  end
end

module RuntimePackProfile
  def self.safe_path(filename)
    return nil if filename.nil?
    clean = filename.to_s.strip
    return nil if clean.empty?

    # Normalize Windows backslashes to forward slashes
    clean = clean.tr('\\', '/')

    base_dir = File.expand_path(Dir.pwd)
    canonical_base = (File.exist?(base_dir) ? (File.realpath(base_dir) rescue base_dir) : base_dir)

    target = File.expand_path(clean, base_dir)

    # 1. Lexical prefix check
    base_prefix = base_dir.end_with?('/') ? base_dir : (base_dir + '/')
    return nil unless target == base_dir || target.start_with?(base_prefix)

    # 2. Reject if target itself is a symlink
    return nil if File.symlink?(target)

    # 3. Component symlink check & canonicalization of existing ancestors
    rel = target.sub(/\A#{Regexp.escape(base_dir)}\/?/, '')
    curr = base_dir
    rel.split('/').reject { |p| p.empty? || p == '.' }.each do |part|
      curr = File.join(curr, part)
      return nil if File.symlink?(curr)
      break unless File.exist?(curr)
    end

    # Nearest existing ancestor must reside inside canonical_base
    ancestor = target
    while !File.exist?(ancestor) && ancestor != File.dirname(ancestor)
      ancestor = File.dirname(ancestor)
    end

    if File.exist?(ancestor)
      real_ancestor = (File.realpath(ancestor) rescue nil)
      return nil if real_ancestor.nil?
      c_base_prefix = canonical_base.end_with?('/') ? canonical_base : (canonical_base + '/')
      return nil unless real_ancestor == canonical_base || real_ancestor.start_with?(c_base_prefix)
    end

    # 4. On Linux/Android, handle case-insensitivity if file exists with different casing
    dir = File.dirname(target)
    if Dir.exist?(dir)
      base = File.basename(target).downcase
      entries = Dir.entries(dir) rescue []
      match = entries.find { |e| e != '.' && e != '..' && e.downcase == base }
      if match
        matched_target = File.join(dir, match)
        return nil if File.symlink?(matched_target)
        real_matched = (File.realpath(matched_target) rescue nil)
        return nil if real_matched.nil?
        c_base_prefix = canonical_base.end_with?('/') ? canonical_base : (canonical_base + '/')
        return nil unless real_matched == canonical_base || real_matched.start_with?(c_base_prefix)
        return matched_target
      end
    end

    target
  end

  def self.byte_slice_valid(str, max_bytes)
    return "" if max_bytes.to_i <= 0 || str.nil? || str.empty?
    str = str.to_s
    return str if str.bytesize <= max_bytes

    if str.respond_to?(:each_char) && str.valid_encoding? && (defined?(Encoding) && str.encoding != Encoding::BINARY)
      accum = ""
      accum = accum.force_encoding(str.encoding) if accum.respond_to?(:force_encoding)
      str.each_char do |ch|
        break if accum.bytesize + ch.bytesize > max_bytes
        accum << ch
      end
      accum
    else
      if str.respond_to?(:byteslice)
        str.byteslice(0, max_bytes)
      else
        str[0, max_bytes]
      end
    end
  end

  def self.read_ini(filepath)
    return nil unless filepath && File.file?(filepath)

    content = if File.respond_to?(:binread)
      File.binread(filepath) rescue nil
    else
      File.open(filepath, "rb") { |f| f.read } rescue nil
    end
    return nil unless content

    # Strip UTF-8 BOM if present
    bom = "\xEF\xBB\xBF".b rescue "\xEF\xBB\xBF"
    if content.start_with?(bom)
      content = content.byteslice(3..-1) || ""
    end
    content = content.force_encoding(Encoding::UTF_8) if defined?(Encoding) && content.respond_to?(:force_encoding)

    data = {}
    current_section = ''

    content.each_line do |line|
      line = line.strip
      next if line.empty? || line.start_with?(';') || line.start_with?('#')

      if line =~ /\A\[([^\]]+)\]\z/
        current_section = $1.strip
      elsif line =~ /\A([^=]+)=(.*)\z/
        k = $1.strip
        v = $2.strip
        if (v.start_with?('"') && v.end_with?('"') && v.length >= 2) ||
           (v.start_with?("'") && v.end_with?("'") && v.length >= 2)
          v = v[1..-2]
        end
        data[current_section] ||= {}
        data[current_section][k] = v
      end
    end
    data
  end

  def self.get_value(filepath, section_name, key_name, default_value)
    return default_value if filepath.nil?
    data = read_ini(filepath)
    return default_value if data.nil?

    sec_key = data.keys.find { |s| s.casecmp(section_name.to_s) == 0 }
    return default_value unless sec_key

    section = data[sec_key]
    k_key = section.keys.find { |k| k.casecmp(key_name.to_s) == 0 }
    return default_value unless k_key

    section[k_key]
  end

  def self.write_value(filepath, section_name, key_name, value)
    return 0 if filepath.nil?
    sec_str = section_name.to_s.strip
    k_str = key_name.to_s.strip
    return 0 if sec_str.empty? || k_str.empty?

    # Reject if filepath is an existing directory or symlink
    return 0 if Dir.exist?(filepath) || File.directory?(filepath) || File.symlink?(filepath)

    # Reject if file exists and is not writable
    return 0 if File.exist?(filepath) && !File.writable?(filepath)

    dir = File.dirname(filepath)
    unless Dir.exist?(dir)
      begin
        if defined?(FileUtils) && FileUtils.respond_to?(:mkdir_p)
          FileUtils.mkdir_p(dir)
        else
          Dir.mkdir(dir)
        end
      rescue
        return 0
      end
      return 0 unless Dir.exist?(dir)
    end

    # Check directory writability
    return 0 unless File.writable?(dir)

    lines = []
    has_crlf = true
    if File.file?(filepath)
      raw = if File.respond_to?(:binread)
        File.binread(filepath) rescue nil
      else
        File.open(filepath, "rb") { |f| f.read } rescue nil
      end
      return 0 if raw.nil?
      has_crlf = raw.include?("\r\n")
      bom = "\xEF\xBB\xBF".b rescue "\xEF\xBB\xBF"
      if raw.start_with?(bom)
        raw = raw.byteslice(3..-1) || ""
      end
      raw = raw.force_encoding(Encoding::UTF_8) if defined?(Encoding) && raw.respond_to?(:force_encoding)
      lines = raw.lines.map(&:to_s)
    end

    eol = has_crlf ? "\r\n" : "\n"
    new_lines = []
    current_sec = nil
    sec_found = false
    key_found = false
    i = 0

    while i < lines.length
      line = lines[i]
      trimmed = line.strip

      if trimmed =~ /\A\[([^\]]+)\]\z/
        this_sec = $1.strip
        if sec_found && !key_found && !value.nil?
          new_lines << "#{k_str}=#{value}#{eol}"
          key_found = true
        end
        current_sec = this_sec
        sec_found = true if current_sec.casecmp(sec_str) == 0
        new_lines << line
        i += 1
        next
      end

      if current_sec && current_sec.casecmp(sec_str) == 0 && trimmed =~ /\A([^=]+)=(.*)\z/
        line_k = $1.strip
        if line_k.casecmp(k_str) == 0
          key_found = true
          unless value.nil?
            new_lines << "#{k_str}=#{value}#{eol}"
          end
          i += 1
          next
        end
      end

      new_lines << line
      i += 1
    end

    if sec_found
      if !key_found && !value.nil?
        if new_lines.any? && !new_lines.last.end_with?("\n")
          new_lines[-1] = new_lines.last + eol
        end
        new_lines << "#{k_str}=#{value}#{eol}"
      end
    else
      unless value.nil?
        if new_lines.any? && !new_lines.last.end_with?("\n")
          new_lines[-1] = new_lines.last + eol
        end
        new_lines << "#{eol}" unless new_lines.empty? || new_lines.last == "#{eol}"
        new_lines << "[#{sec_str}]#{eol}"
        new_lines << "#{k_str}=#{value}#{eol}"
      end
    end

    output_str = new_lines.join
    tmp_path = File.join(dir, ".rp_ini_tmp_#{Process.pid}_#{Time.now.to_i}_#{(rand * 100000).to_i}.tmp")
    begin
      File.open(tmp_path, "wb") do |f|
        f.write(output_str)
        f.flush
        if f.respond_to?(:fsync)
          begin
            f.fsync
          rescue Errno::EINVAL, Errno::ENOSYS, NotImplementedError
            # Non-fatal only if underlying mount does not support fsync primitive
          end
        end
      end

      # Verify temp file content before replacement
      tmp_content = (File.respond_to?(:binread) ? File.binread(tmp_path) : File.open(tmp_path, "rb") { |f| f.read }) rescue nil
      if tmp_content != output_str
        File.delete(tmp_path) if File.exist?(tmp_path)
        return 0
      end

      # Pre-replacement revalidation of target and parent symlink state (fail-closed)
      if File.symlink?(filepath) || File.symlink?(dir)
        File.delete(tmp_path) if File.exist?(tmp_path)
        return 0
      end

      base_dir = File.expand_path(Dir.pwd)
      canonical_base = (File.exist?(base_dir) ? (File.realpath(base_dir) rescue base_dir) : base_dir)
      real_dir = (File.realpath(dir) rescue nil)
      c_base_prefix = canonical_base.end_with?('/') ? canonical_base : (canonical_base + '/')
      unless real_dir && (real_dir == canonical_base || real_dir.start_with?(c_base_prefix))
        File.delete(tmp_path) if File.exist?(tmp_path)
        return 0
      end

      # Atomic replacement - fail-closed without in-place overwrite fallback
      File.rename(tmp_path, filepath)

      # Deterministically verify target file existence and content
      return 0 unless File.exist?(filepath) && File.file?(filepath)
      actual = (File.respond_to?(:binread) ? File.binread(filepath) : File.open(filepath, "rb") { |f| f.read }) rescue nil
      return 0 unless actual == output_str

      1
    rescue => e
      File.delete(tmp_path) if File.exist?(tmp_path)
      0
    end
  end
end

class Win32API
  def self.call(func, *args)
    new('', func).call(*args)
  end

  def initialize(dll, func, import_types = nil, export_type = nil)
    @dll = dll.to_s.downcase.gsub('\\', '/')
    @func = func.to_s
    @import_types = import_types
    @export_type = export_type
  end

  def call(*args)
    case @func
    when 'GetCommandLine'
      ''
    when 'GetKeyboardState'
      ptr_target = if args[0].is_a?(Numeric)
        (DL::CPtr.instances[args[0].to_i] if defined?(DL::CPtr) && DL::CPtr.respond_to?(:instances) && DL::CPtr.instances)
      elsif defined?(DL::CPtr) && args[0].is_a?(DL::CPtr)
        args[0]
      elsif args[0].is_a?(String)
        args[0]
      else
        nil
      end

      input_file = File.exist?("rp_input_state.bin") ? "rp_input_state.bin" : (File.exist?("../rp_input_state.bin") ? "../rp_input_state.bin" : nil)
      if input_file
        data = File.binread(input_file) rescue nil
        if data
          if ptr_target.is_a?(String)
            ptr_target[0, 256] = data[0, 256]
          elsif defined?(DL::CPtr) && ptr_target.is_a?(DL::CPtr)
            0.upto(255) { |k| ptr_target[k] = data.getbyte(k) || 0 }
          end
        end
      end
      1
    when 'GetAsyncKeyState', 'GetKeyState'
      vk = args[0].to_i
      input_file = File.exist?("rp_input_state.bin") ? "rp_input_state.bin" : (File.exist?("../rp_input_state.bin") ? "../rp_input_state.bin" : nil)
      if input_file
        data = File.binread(input_file) rescue nil
        if data && vk >= 0 && data.bytesize > vk
          return ((data.getbyte(vk) & 0x80) != 0) ? -32767 : 0
        end
      end
      0
    when 'MapVirtualKeyEx', 'ToUnicodeEx', 'GetKeyboardLayout', 'SetKeyboardState'
      0
    when 'GetCursorPos'
      if args[0].is_a?(String) && args[0].bytesize >= 8
        args[0][0, 8] = [0, 0].pack('ll')
      end
      1
    when 'GetSystemMetrics'
      case args[0]
      when 0 then 544
      when 1 then 416
      else 0
      end
    when 'GetWindowRect', 'GetClientRect'
      buffer = args.find { |arg| arg.is_a?(String) && arg.bytesize >= 16 }
      buffer[0, 16] = [0, 0, 544, 416].pack('llll') if buffer
      1
    when 'SystemParametersInfo'
      buffer = args[2]
      buffer[0, 16] = [0, 0, 1920, 1080].pack('l4') if buffer.is_a?(String) && buffer.bytesize >= 16
      1
    when 'FindWindow', 'FindWindowEx', 'CreateWindowEx'
      1
    when 'ShowCursor', 'ClipCursor', 'ShowWindow',
         'SetWindowLong', 'SetWindowPos', 'SendInput', 'UpdateWindow',
         'GetDC', 'ReleaseDC', 'FillRect', 'CreateSolidBrush', 'DeleteObject', 'DeleteDC',
         'CreateCompatibleBitmap', 'CreateCompatibleDC', 'SelectObject', 'BitBlt',
         'GetDIBits', 'OpenClipboard', 'CloseClipboard', 'EmptyClipboard',
         'GetClipboardData', 'SetClipboardData', 'GlobalAlloc', 'GlobalLock',
         'GlobalUnlock', 'lstrlenA', 'lstrcpyA'
      0
    when 'GetPrivateProfileString'
      section = args[0]
      key = args[1]
      default_val = args[2] ? args[2].to_s : ''
      buffer = args[3]
      size = args[4].to_i
      filename = args[5]

      return 0 unless buffer.is_a?(String)
      return 0 if size <= 0

      safe_file = RuntimePackProfile.safe_path(filename)
      val = if safe_file
        RuntimePackProfile.get_value(safe_file, section, key, default_val)
      else
        default_val
      end

      val = val.to_s
      buf_len = buffer.bytesize
      effective_cap = [size, buf_len].min
      max_bytes = [effective_cap - 1, 0].max

      to_copy = RuntimePackProfile.byte_slice_valid(val, max_bytes)
      c_len = to_copy.bytesize

      orig_enc = (buffer.respond_to?(:encoding) ? buffer.encoding : nil)
      buffer.force_encoding(Encoding::BINARY) if buffer.respond_to?(:force_encoding)
      to_copy_b = (to_copy.respond_to?(:b) ? to_copy.b : to_copy)

      if c_len > 0
        buffer[0, c_len] = to_copy_b
      end
      if c_len < buf_len && c_len < size
        buffer[c_len] = "\0"
      end
      buffer.force_encoding(orig_enc) if orig_enc && buffer.respond_to?(:force_encoding)
      c_len
    when 'WritePrivateProfileString'
      section = args[0]
      key = args[1]
      value = args[2]
      filename = args[3]

      safe_file = RuntimePackProfile.safe_path(filename)
      if safe_file
        RuntimePackProfile.write_value(safe_file, section, key, value)
      else
        0
      end
    when 'RtlMoveMemory'
      if args[0].is_a?(String) && args[1].is_a?(String)
        length = args[2] || args[1].bytesize
        args[0][0, length] = args[1][0, length]
      end
      0
    when 'get_mouse_wheel', 'unload', 'hook', 'XInputSetState', 'XInputGetState'
      0
    when 'add_setting', 'merge_settings', 'exclusive_merge',
         'add_swap', 'remove_swap', 'process_bitmap', 'remove_setting',
         'clear_settings', 'process_bitmap_limited', 'remove_intersecting_settings',
         'schedule_mode', 'execute'
      0
    else
      # Default stub returns 1 (Win32 TRUE / success for wrapper DLLs like Utils, Steamworks)
      1
    end
  end
end

module RuntimePackLangscore
  VALID_LANG_REGEX = /\A[a-zA-Z0-9_\-]{2,16}\z/

  class << self
    def read_configured_language(ini_path = "./Game.ini")
      safe_file = RuntimePackProfile.safe_path(ini_path)
      return nil unless safe_file && File.file?(safe_file)

      val = RuntimePackProfile.get_value(safe_file, "Langscore", "Lang", nil)
      return nil if val.nil?

      val_str = val.to_s.strip
      return nil if val_str.empty?
      return nil unless val_str =~ VALID_LANG_REGEX

      val_str
    end

    def restore_bootstrap_language(ini_path = "./Game.ini")
      lang = read_configured_language(ini_path)
      if lang
        $langscore_current_language = lang
        puts "[RuntimePack] Langscore bootstrap language restored: #{lang}"
        lang
      else
        nil
      end
    end

    def reapply_language_before_load(ini_path = "./Game.ini")
      return nil unless defined?(Langscore)

      lang = read_configured_language(ini_path)
      if lang
        $langscore_current_language = lang
        puts "[RuntimePack] Langscore pre-load language restored: #{lang}"
        lang
      else
        nil
      end
    end

    def install_save_load_hook
      return unless defined?(DataManager)
      return unless DataManager.respond_to?(:extract_save_contents)

      dm_class = (class << DataManager; self; end)
      current_m = DataManager.method(:extract_save_contents)
      return if @hooked_method == current_m

      # Capture the implementation that is current right now as a bound Method
      # and close over it. Re-installation (for example after Langscore aliases
      # and replaces extract_save_contents) wraps that new implementation
      # without rebinding any earlier wrapper, so the chain cannot cycle.
      orig = current_m
      dm_class.class_eval do
        define_method(:extract_save_contents) do |contents|
          if defined?(RuntimePackLangscore)
            RuntimePackLangscore.reapply_language_before_load
          end
          orig.call(contents)
        end
      end
      @hooked_method = DataManager.method(:extract_save_contents)
    end
  end
end

# Restore configured language at earliest safe RGSS bootstrap point
RuntimePackLangscore.restore_bootstrap_language rescue nil


class Plane
  attr_reader :viewport
  attr_accessor :bitmap, :visible, :z, :ox, :oy, :zoom_x, :zoom_y, :opacity, :blend_type, :color, :tone

  def initialize(viewport = nil)
    @viewport = viewport
    @bitmap = nil
    @visible = true
    @z = 0
    @ox = 0
    @oy = 0
    @zoom_x = 1.0
    @zoom_y = 1.0
    @opacity = 255
    @blend_type = 0
    @color = nil
    @tone = nil
    @disposed = false
  end

  def dispose
    @disposed = true
  end

  def disposed?
    @disposed
  end
end unless defined?(Plane)

class MKXPError < StandardError; end unless defined?(MKXPError)
class RGSSError < StandardError; end unless defined?(RGSSError)

[:Sprite, :Viewport, :Plane, :Window, :Tilemap].each do |klass_name|
  if Object.const_defined?(klass_name)
    klass = Object.const_get(klass_name)
    if klass.method_defined?(:visible=)
      klass.class_eval do
        alias_method :runtimepack_orig_visible=, :visible= unless method_defined?(:runtimepack_orig_visible=)
        def visible=(val)
          self.runtimepack_orig_visible = (val ? true : false)
        end
      end
    end

    if klass.method_defined?(:dispose)
      klass.class_eval do
        alias_method :runtimepack_orig_dispose, :dispose unless method_defined?(:runtimepack_orig_dispose)
        def dispose
          begin
            runtimepack_orig_dispose
          rescue Exception => e
            if (defined?(MKXPError) && e.is_a?(MKXPError) || e.class.name == "MKXPError") && e.message =~ /No instance data for variable/i
              return nil
            end
            raise
          end
        end
      end
    end

    if klass.method_defined?(:disposed?)
      klass.class_eval do
        alias_method :runtimepack_orig_disposed?, :disposed? unless method_defined?(:runtimepack_orig_disposed?)
        def disposed?
          begin
            runtimepack_orig_disposed?
          rescue Exception => e
            if (defined?(MKXPError) && e.is_a?(MKXPError) || e.class.name == "MKXPError") && e.message =~ /No instance data for variable/i
              return true
            end
            raise
          end
        end
      end
    end
  end
end

class Bitmap
  attr_reader :width, :height, :disposed

  def initialize(*args)
    if args.size == 1 && args[0].is_a?(String)
      filename = args[0]
      unless File.exist?(filename)
        raise Errno::ENOENT, "No such file or directory - #{filename}"
      end
      @filename = filename
      @width = 32
      @height = 32
    elsif args.size >= 2
      @width = args[0]
      @height = args[1]
    else
      @width = 32
      @height = 32
    end
    @disposed = false
  end

  def dispose
    @disposed = true
  end

  def disposed?
    @disposed
  end

  def fill_rect(*args)
  end
end unless defined?(Bitmap)

if defined?(Bitmap)
  class << Bitmap
    alias_method :runtimepack_original_new, :new unless method_defined?(:runtimepack_original_new)

    def new(*args, &block)
      if args.size == 1 && args[0].is_a?(String)
        resolved = RuntimePackCasePath.resolve_bitmap_path(args[0])
        runtimepack_original_new(resolved, &block)
      else
        runtimepack_original_new(*args, &block)
      end
    end
  end

  if Bitmap.method_defined?(:dispose)
    Bitmap.class_eval do
      alias_method :runtimepack_orig_dispose, :dispose unless method_defined?(:runtimepack_orig_dispose)
      def dispose
        begin
          runtimepack_orig_dispose
        rescue Exception => e
          if (defined?(MKXPError) && e.is_a?(MKXPError) || e.class.name == "MKXPError") && e.message =~ /No instance data for variable/i
            return nil
          end
          raise
        end
      end
    end
  end

  if Bitmap.method_defined?(:disposed?)
    Bitmap.class_eval do
      alias_method :runtimepack_orig_disposed?, :disposed? unless method_defined?(:runtimepack_orig_disposed?)
      def disposed?
        begin
          runtimepack_orig_disposed?
        rescue Exception => e
          if (defined?(MKXPError) && e.is_a?(MKXPError) || e.class.name == "MKXPError") && e.message =~ /No instance data for variable/i
            return true
          end
          raise
        end
      end
    end
  end
end

class Color
  attr_accessor :red, :green, :blue, :alpha
  def initialize(r = 0, g = 0, b = 0, a = 255)
    @red, @green, @blue, @alpha = r, g, b, a
  end
end unless defined?(Color)

class Rect
  attr_accessor :x, :y, :width, :height
  def initialize(x = 0, y = 0, w = 0, h = 0)
    @x, @y, @width, @height = x, y, w, h
  end
end unless defined?(Rect)

module Audio
  class << self
    def bgm_play(*args)
      check_audio_stub(args.first)
    end
    def bgs_play(*args)
      check_audio_stub(args.first)
    end
    def me_play(*args)
      check_audio_stub(args.first)
    end
    def se_play(*args)
      check_audio_stub(args.first)
    end
    def bgm_stop; end
    def bgs_stop; end
    def me_stop; end
    def se_stop; end

    private
    def check_audio_stub(item)
      return unless item
      name = item.respond_to?(:name) ? item.name : item.to_s
      return if name.empty?
      unless File.exist?(name)
        raise Errno::ENOENT, "No such file or directory - #{name}"
      end
    end
  end
end unless defined?(Audio)

module RuntimePackCompatibility
  FONT_SETTERS = [:shadow, :outline, :bold, :italic]
  DIRECTION_LABELS = {
    "\u2b06" => "Up",
    "\u2b07" => "Down",
    "\u2b05" => "Left",
    "\u27a1" => "Right"
  }

  @plane_mitigations = []

  class << self
    attr_reader :plane_mitigations

    def record_plane_mitigation(info)
      @plane_mitigations ||= []
      @plane_mitigations << info
    end

    def reset_plane_mitigations!
      @plane_mitigations = []
    end

    def install_plane_subtractive_fog_hooks
      return unless Object.const_defined?(:Plane)
      klass = Object.const_get(:Plane)
      return if klass.method_defined?(:runtimepack_orig_blend_type=)

      klass.class_eval do
        alias_method :runtimepack_orig_blend_type=, :blend_type= rescue nil
        alias_method :runtimepack_orig_opacity=, :opacity= rescue nil
        alias_method :runtimepack_orig_z=, :z= rescue nil

        def blend_type=(val)
          if respond_to?(:runtimepack_orig_blend_type=)
            send(:runtimepack_orig_blend_type=, val)
          else
            @blend_type = val
          end
          runtimepack_check_subtractive_fog_clamp!
        end

        def opacity=(val)
          target_val = val
          if self.blend_type == 2
            cur_z = self.z
            if cur_z && cur_z < 32
              limit = runtimepack_fog_opacity_limit
              if target_val && target_val > limit
                target_val = limit
                runtimepack_log_fog_mitigation(target_val)
              end
            end
          end
          if respond_to?(:runtimepack_orig_opacity=)
            send(:runtimepack_orig_opacity=, target_val)
          else
            @opacity = target_val
          end
        end

        def z=(val)
          if respond_to?(:runtimepack_orig_z=)
            send(:runtimepack_orig_z=, val)
          else
            @z = val
          end
          runtimepack_check_subtractive_fog_clamp!
        end

        def runtimepack_fog_opacity_limit
          floor_missing = defined?(RuntimePackAssetAudit) && RuntimePackAssetAudit.respond_to?(:events) &&
            RuntimePackAssetAudit.events.any? { |ev| ev["fallback_action"] == "TILESET_FALLBACK_DUMMY" }
          floor_missing ? 60 : 80
        end

        def runtimepack_log_fog_mitigation(clamped_op)
          msg = "[RuntimePack] Subtractive fog Plane mitigation: clamped opacity to #{clamped_op} (Z=#{self.z}) to prevent OpenGL ES black occlusion"
          puts msg
          if defined?(RuntimePackCompatibility) && RuntimePackCompatibility.respond_to?(:record_plane_mitigation)
            RuntimePackCompatibility.record_plane_mitigation({
              plane: self,
              z: self.z,
              opacity: clamped_op,
              blend_type: self.blend_type,
              message: msg
            })
          end
        end

        def runtimepack_check_subtractive_fog_clamp!
          if self.blend_type == 2
            cur_z = self.z
            if cur_z && cur_z < 32
              limit = runtimepack_fog_opacity_limit
              cur_op = self.opacity || 255
              if cur_op > limit
                if respond_to?(:runtimepack_orig_opacity=)
                  send(:runtimepack_orig_opacity=, limit)
                else
                  @opacity = limit
                end
                runtimepack_log_fog_mitigation(limit)
              end
            end
          end
        end
      end
    end
  end

  def self.install_font_setters
    return unless defined?(Font)
    FONT_SETTERS.each do |name|
      setter = "#{name}=".to_sym
      original = "runtimepack_orig_#{name}=".to_sym
      next unless Font.method_defined?(setter)
      next if Font.method_defined?(original)
      Font.class_eval do
        alias_method original, setter
        define_method(setter) do |val|
          send(original, val ? true : false)
        end
      end
    end
  end

  def self.normalize_input_utils
    return unless defined?(InputUtils)
    mapping = InputUtils.instance_variable_get(:@keySYM_in_UI)
    return unless mapping.is_a?(Hash)
    mapping.each do |key, value|
      next unless value.is_a?(String)
      candidate = value.dup.force_encoding(Encoding::UTF_8)
      mapping[key] = DIRECTION_LABELS[candidate] if DIRECTION_LABELS.key?(candidate)
    end
  end

  def self.create_tileset_fallback(name = nil)
    bmp = (Bitmap.new(512, 384) rescue nil)
    return (Bitmap.new(32, 32) rescue nil) unless bmp
    if bmp.respond_to?(:fill_rect) && defined?(Color) && defined?(Rect)
      c1 = Color.new(40, 40, 40)
      c2 = Color.new(70, 70, 70)
      border = Color.new(30, 30, 30)
      (0...12).each do |ty|
        (0...16).each do |tx|
          x = tx * 32
          y = ty * 32
          color = ((tx + ty) % 2 == 0) ? c1 : c2
          bmp.fill_rect(Rect.new(x, y, 32, 32), color) rescue nil
          bmp.fill_rect(Rect.new(x, y, 32, 1), border) rescue nil
          bmp.fill_rect(Rect.new(x, y, 1, 32), border) rescue nil
          bmp.fill_rect(Rect.new(x, y + 31, 32, 1), border) rescue nil
          bmp.fill_rect(Rect.new(x + 31, y, 1, 32), border) rescue nil
        end
      end
    end
    bmp
  rescue
    Bitmap.new(32, 32) rescue nil
  end

  def self.create_tileset_fallback_bitmap(name = nil)
    create_tileset_fallback(name)
  end

  def self.create_smart_tileset_fallback(name = nil)
    create_tileset_fallback(name)
  end

  def self.create_character_fallback(path_or_name = nil)
    bmp = (Bitmap.new(128, 128) rescue nil)
    return (Bitmap.new(32, 32) rescue nil) unless bmp

    raw = path_or_name.to_s
    filename = File.basename(raw)
    base_no_ext = File.basename(filename, ".*")
    lower = base_no_ext.downcase

    is_interactive = base_no_ext.start_with?("!") || filename.start_with?("!") || base_no_ext.start_with?("$!")

    if bmp.respond_to?(:fill_rect) && defined?(Color) && defined?(Rect)
      if is_interactive
        if lower.include?("flame") || lower.include?("fire") || lower.include?("torch") || lower.include?("save") || lower.include?("light")
          # Golden icon / flame box for Flame / Save points
          flame_outer = Color.new(220, 80, 10)
          flame_core  = Color.new(255, 215, 0)
          flame_tip   = Color.new(255, 245, 160)
          pedestal    = Color.new(70, 70, 75)
          pedestal_b  = Color.new(40, 40, 45)

          (0...4).each do |r|
            (0...3).each do |c|
              cx = c * 42 + 21
              cy = r * 32 + 16
              tip_off = (c == 1) ? 2 : 0

              bmp.fill_rect(Rect.new(cx - 8, cy + 8, 16, 4), pedestal_b) rescue nil
              bmp.fill_rect(Rect.new(cx - 7, cy + 9, 14, 2), pedestal) rescue nil
              bmp.fill_rect(Rect.new(cx - 7, cy - 6, 14, 14), flame_outer) rescue nil
              bmp.fill_rect(Rect.new(cx - 4, cy - 4, 8, 10), flame_core) rescue nil
              bmp.fill_rect(Rect.new(cx - 2, cy - 9 - tip_off, 4, 4), flame_tip) rescue nil
            end
          end
        elsif lower.include?("chest") || lower.include?("box") || lower.include?("treasure")
          # Brown wooden box for Chest
          wood_body = Color.new(139, 69, 19)
          wood_lid  = Color.new(110, 50, 15)
          metal_b   = Color.new(50, 35, 15)
          gold_lock = Color.new(255, 200, 0)
          shadow    = Color.new(35, 20, 10)

          (0...4).each do |r|
            (0...3).each do |c|
              cx = c * 42 + 21
              cy = r * 32 + 16

              bmp.fill_rect(Rect.new(cx - 9, cy + 9, 18, 2), shadow) rescue nil
              bmp.fill_rect(Rect.new(cx - 10, cy - 4, 20, 13), wood_body) rescue nil
              bmp.fill_rect(Rect.new(cx - 11, cy - 8, 22, 5), wood_lid) rescue nil
              bmp.fill_rect(Rect.new(cx - 9, cy - 8, 2, 17), metal_b) rescue nil
              bmp.fill_rect(Rect.new(cx + 7, cy - 8, 2, 17), metal_b) rescue nil
              bmp.fill_rect(Rect.new(cx - 2, cy - 4, 4, 5), gold_lock) rescue nil
            end
          end
        elsif lower.include?("door") || lower.include?("gate")
          # Doorway rectangle for Door
          frame_col = Color.new(90, 75, 65)
          panel_col = Color.new(125, 80, 45)
          inset_col = Color.new(70, 45, 25)
          knob_col  = Color.new(240, 205, 50)

          (0...4).each do |r|
            (0...3).each do |c|
              cx = c * 42 + 21
              cy = r * 32 + 16

              bmp.fill_rect(Rect.new(cx - 11, cy - 13, 22, 26), frame_col) rescue nil
              bmp.fill_rect(Rect.new(cx - 9, cy - 11, 18, 24), panel_col) rescue nil
              bmp.fill_rect(Rect.new(cx - 7, cy - 9, 14, 9), inset_col) rescue nil
              bmp.fill_rect(Rect.new(cx - 7, cy + 2, 14, 9), inset_col) rescue nil
              bmp.fill_rect(Rect.new(cx + 4, cy, 3, 3), knob_col) rescue nil
            end
          end
        else
          # Generic colored marker box for other '!' objects
          marker_border = Color.new(0, 90, 110)
          marker_bg     = Color.new(0, 185, 215)
          marker_core   = Color.new(255, 255, 255)

          (0...4).each do |r|
            (0...3).each do |c|
              cx = c * 42 + 21
              cy = r * 32 + 16

              bmp.fill_rect(Rect.new(cx - 9, cy - 9, 18, 18), marker_border) rescue nil
              bmp.fill_rect(Rect.new(cx - 7, cy - 7, 14, 14), marker_bg) rescue nil
              bmp.fill_rect(Rect.new(cx - 1, cy - 5, 3, 6), marker_core) rescue nil
              bmp.fill_rect(Rect.new(cx - 1, cy + 3, 3, 2), marker_core) rescue nil
            end
          end
        end
      else
        # Standard character sheets without '!' (NPCs, enemies, actors)
        # 128x128 3x4 sprite sheet with a simple silhouette / marker
        body_outline = Color.new(45, 55, 75)
        body_fill    = Color.new(90, 115, 150)
        face_fill    = Color.new(220, 190, 160)
        eye_color    = Color.new(20, 25, 40)
        hair_color   = Color.new(65, 75, 95)

        (0...4).each do |r|
          (0...3).each do |c|
            cx = c * 42 + 21
            cy = r * 32 + 16

            leg_l_h = (c == 0) ? 6 : 4
            leg_r_h = (c == 2) ? 6 : 4
            bmp.fill_rect(Rect.new(cx - 5, cy + 7, 3, leg_l_h), body_outline) rescue nil
            bmp.fill_rect(Rect.new(cx + 2, cy + 7, 3, leg_r_h), body_outline) rescue nil

            bmp.fill_rect(Rect.new(cx - 7, cy - 3, 14, 11), body_outline) rescue nil
            bmp.fill_rect(Rect.new(cx - 6, cy - 2, 12, 9), body_fill) rescue nil

            bmp.fill_rect(Rect.new(cx - 6, cy - 13, 12, 10), body_outline) rescue nil
            if r == 3
              bmp.fill_rect(Rect.new(cx - 5, cy - 12, 10, 8), hair_color) rescue nil
            else
              bmp.fill_rect(Rect.new(cx - 5, cy - 12, 10, 8), face_fill) rescue nil
              bmp.fill_rect(Rect.new(cx - 5, cy - 12, 10, 3), hair_color) rescue nil
              if r == 0
                bmp.fill_rect(Rect.new(cx - 4, cy - 8, 2, 2), eye_color) rescue nil
                bmp.fill_rect(Rect.new(cx + 2, cy - 8, 2, 2), eye_color) rescue nil
              elsif r == 1
                bmp.fill_rect(Rect.new(cx - 4, cy - 8, 2, 2), eye_color) rescue nil
              elsif r == 2
                bmp.fill_rect(Rect.new(cx + 2, cy - 8, 2, 2), eye_color) rescue nil
              end
            end
          end
        end
      end
    end
    bmp
  rescue
    Bitmap.new(32, 32) rescue nil
  end

  def self.create_smart_character_fallback(path_or_name = nil)
    create_character_fallback(path_or_name)
  end
end

def create_tileset_fallback(name = nil)
  RuntimePackCompatibility.create_tileset_fallback(name)
end

def create_character_fallback(name = nil)
  RuntimePackCompatibility.create_character_fallback(name)
end

RuntimePackCompatibility.install_font_setters
RuntimePackCompatibility.install_plane_subtractive_fog_hooks

def patch_json_decoder
  if defined?(JSON)
    if JSON.respond_to?(:encode_hash) && !JSON.respond_to?(:__rp_encode_patched)
      class << JSON
        define_method(:__rp_encode_patched) { true }
        alias_method :rp_orig_encode_hash, :encode_hash rescue nil
        alias_method :rp_orig_encode_array, :encode_array rescue nil
        def encode_hash(hash)
          return "{}" if hash.nil? || hash.empty?
          rp_orig_encode_hash(hash)
        end
        def encode_array(array)
          return "[]" if array.nil? || array.empty?
          rp_orig_encode_array(array)
        end
      end
    end

    if JSON.respond_to?(:next_token) && !JSON.respond_to?(:__rp_patched)
    JSON.define_singleton_method(:__rp_patched) { true }
    JSON.define_singleton_method(:next_token) do |debug = 0|
      loop do
        return TOKEN_NONE if @index >= @length
        char = @json[@index, 1]
        @index += 1
        case char
        when '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '-', '.'
          return TOKEN_NUMBER
        when '{'
          return TOKEN_CURLY_OPEN
        when '}'
          return TOKEN_CURLY_CLOSED
        when '"'
          return TOKEN_STRING
        when ','
          return TOKEN_COMMA
        when '['
          return TOKEN_SQUARED_OPEN
        when ']'
          return TOKEN_SQUARED_CLOSED
        when ':'
          return TOKEN_COLON
        when ' ', "\n", "\t", "\r"
          # loop iteratively, NO RECURSION
        when '/'
          if @json[@index, 1] == '/'
            while @index < @length && @json[@index] != "\n"
              @index += 1
            end
            # loop iteratively, NO RECURSION
          else
            @index -= 1
            return TOKEN_NONE
          end
        else
          @index -= 1
          if @json[@index, 5] == "false"
            @index += 5
            return TOKEN_FALSE
          elsif @json[@index, 4] == "true"
            @index += 4
            return TOKEN_TRUE
          elsif @json[@index, 4] == "null"
            @index += 4
            return TOKEN_NULL
          end
          return TOKEN_NONE
        end
      end
    end
  end
end
end

if defined?(Input)
  class << Input
    alias_method :rp_native_press?, :press? rescue nil
    alias_method :rp_native_trigger?, :trigger? rescue nil
    alias_method :rp_native_dir4, :dir4 rescue nil
    alias_method :rp_native_dir8, :dir8 rescue nil
  end
end

$runtimepack_god_mode = false unless defined?($runtimepack_god_mode)
$runtimepack_one_hit_kill = false unless defined?($runtimepack_one_hit_kill)

module RuntimePackCheatRunner
  CMD_FILE = "rp_cheat_cmd.json"
  STATE_FILE = "rp_cheat_state.json"
  EVAL_FILE = "rp_cheat_eval_res.txt"
  ERROR_FILE = "rp_cheat_error.txt"

  def self.install_battle_hooks
    return if @battle_hooks_installed
    battle_modules = []
    battle_modules << Battle_System if defined?(Battle_System) && Battle_System.is_a?(Module)
    battle_modules << BattleSystem if defined?(BattleSystem) && BattleSystem.is_a?(Module)

    if !battle_modules.empty?
      @battle_hooks_installed = true
      battle_modules.each do |mod|
        mod.module_eval do
          unless method_defined?(:rp_orig_take_damage)
            alias_method :rp_orig_take_damage, :take_damage rescue nil
            def take_damage(user, skill, force_hit = false, back_stabed = false)
              if $runtimepack_one_hit_kill && defined?($game_player) && $game_player
                is_player = (user == $game_player.actor || (user.respond_to?(:map_token) && user.map_token == $game_player))
                if is_player && (self != $game_player.actor && !(self.respond_to?(:map_token) && self.map_token == $game_player))
                  if self.respond_to?(:battle_stat) && self.battle_stat
                    self.battle_stat.set_stat("health", 0) rescue nil
                    self.battle_stat.set_stat("sta", -100) rescue nil
                  end
                  self.health = 0 if self.respond_to?(:health=)
                  self.sta = -100 if self.respond_to?(:sta=)
                end
              end
              rp_orig_take_damage(user, skill, force_hit, back_stabed) if respond_to?(:rp_orig_take_damage)
            end
          end

          unless method_defined?(:rp_orig_take_skill_effect)
            alias_method :rp_orig_take_skill_effect, :take_skill_effect rescue nil
            def take_skill_effect(user, skill, can_sap = false, force_hit = false)
              if $runtimepack_god_mode && defined?($game_player) && $game_player &&
                 (self == $game_player.actor || (self.respond_to?(:map_token) && self.map_token == $game_player))
                return
              end
              rp_orig_take_skill_effect(user, skill, can_sap, force_hit) if respond_to?(:rp_orig_take_skill_effect)
            end
          end
        end
      end
    end

    if defined?(Game_Actor) && Game_Actor.is_a?(Class)
      Game_Actor.class_eval do
        unless method_defined?(:rp_orig_determine_death)
          alias_method :rp_orig_determine_death, :determine_death rescue nil
          def determine_death
            return if $runtimepack_god_mode
            rp_orig_determine_death if respond_to?(:rp_orig_determine_death)
          end
        end
      end
    end
  end

  def self.install_save_and_cache_hooks
    if defined?(SceneManager)
      class << SceneManager
        unless method_defined?(:rp_orig_scene_call)
          alias_method :rp_orig_scene_call, :call rescue nil
          def call(scene_class)
            @stack ||= []
            @stack.push(@scene)
            @scene = scene_class.new
          end
        end
        unless method_defined?(:rp_orig_scene_return)
          alias_method :rp_orig_scene_return, :return rescue nil
          def return
            @stack ||= []
            @scene = @stack.pop
          end
        end
      end
    end

    [defined?(Cache) ? Cache : nil, defined?(RPG::Cache) ? RPG::Cache : nil].compact.each do |cache_mod|
      cache_target = (class << cache_mod; self; end)
      cache_methods = [
        :animation, :autotile, :battleback, :battleback1, :battleback2, :battler,
        :character, :face, :fog, :gameover, :icon, :parallax, :picture, :system, :tile,
        :tileset, :title, :title1, :title2, :windowskin
      ]
      cache_methods.each do |m|
        if cache_mod.respond_to?(m)
          orig_m = :"rp_orig_cache_#{m}"
          unless cache_mod.respond_to?(orig_m)
            cache_target.class_eval do
              alias_method orig_m, m rescue nil
              define_method(m) do |*args|
                begin
                  send(orig_m, *args)
                rescue Errno::ENOENT, (defined?(RGSSError) ? RGSSError : StandardError), StandardError => e
                  arg_str = args.first.to_s
                  if m == :tileset || arg_str.include?("Graphics/Tilesets/")
                    puts "[RuntimePack] Missing tileset in #{cache_mod}.#{m}(#{arg_str}): #{e.class}: #{e.message}, returning tileset dummy bitmap"
                    if defined?(RuntimePackAssetAudit)
                      RuntimePackAssetAudit.record_event(
                        "FILE_EXISTS",
                        arg_str,
                        "FAIL",
                        fallback_action: "TILESET_FALLBACK_DUMMY",
                        error_message: "RTP tileset missing; Layer 0/1 map tiles will render transparent/black",
                        source_location: "#{cache_mod}.#{m}"
                      )
                    end
                    RuntimePackCompatibility.create_tileset_fallback(arg_str)
                  elsif m == :character || arg_str.include?("Graphics/Characters/")
                    puts "[RuntimePack] Missing character in #{cache_mod}.#{m}(#{arg_str}): #{e.class}: #{e.message}, returning character dummy bitmap"
                    if defined?(RuntimePackAssetAudit)
                      RuntimePackAssetAudit.record_event(
                        "FILE_EXISTS",
                        arg_str,
                        "FAIL",
                        fallback_action: "CHARACTER_FALLBACK_DUMMY",
                        error_message: "RTP character sprite missing; substituted visible marker sheet",
                        source_location: "#{cache_mod}.#{m}"
                      )
                    end
                    RuntimePackCompatibility.create_character_fallback(arg_str)
                  else
                    puts "[RuntimePack] Missing bitmap in #{cache_mod}.#{m}(#{arg_str}): #{e.class}: #{e.message}, returning dummy bitmap"
                    if defined?(RuntimePackAssetAudit)
                      RuntimePackAssetAudit.record_event(
                        "FILE_EXISTS",
                        arg_str,
                        "FAIL",
                        fallback_action: "PLACEHOLDER_SUBSTITUTED",
                        error_message: "#{e.class}: #{e.message}",
                        source_location: "#{cache_mod}.#{m}"
                      )
                    end
                    (Bitmap.new(32, 32) rescue nil)
                  end
                end
              end
            end
          end
        end
      end

      if cache_mod.respond_to?(:savefile_bitmap) && !cache_mod.respond_to?(:rp_orig_savefile_bitmap)
        cache_target.class_eval do
          alias_method :rp_orig_savefile_bitmap, :savefile_bitmap rescue nil
          def savefile_bitmap(path)
            begin
              rp_orig_savefile_bitmap(path)
            rescue Errno::ENOENT, StandardError
              @cache_savefile_bitmap ||= {}
              fallback = "Graphics/System/SaveNotFound.png"
              @cache_savefile_bitmap[path] = (File.exist?(fallback) ? Bitmap.new(fallback) : Bitmap.new(264, 160)) rescue Bitmap.new(32, 32)
            end
          end
        end
      end
    end

    if defined?(Audio)
      class << Audio
        [:se_play, :bgm_play, :bgs_play, :me_play].each do |m|
          if method_defined?(m)
            orig_m = :"rp_orig_audio_#{m}"
            unless method_defined?(orig_m)
              alias_method orig_m, m rescue nil
              define_method(m) do |*args|
                begin
                  send(orig_m, *args)
                rescue Errno::ENOENT, (defined?(RGSSError) ? RGSSError : StandardError), StandardError => e
                  puts "[RuntimePack] Audio asset not found or error in #{m}: #{args[0]} (#{e.class}: #{e.message})"
                  audio_path = args.first.respond_to?(:name) ? args.first.name : args.first.to_s
                  if defined?(RuntimePackAssetAudit)
                    RuntimePackAssetAudit.record_event(
                      "FILE_EXISTS",
                      audio_path,
                      "FAIL",
                      fallback_action: "AUDIO_LOAD_SUPPRESSED",
                      error_message: "#{e.class}: #{e.message}",
                      source_location: "Audio.#{m}"
                    )
                  end
                  nil
                end
              end
            end
          end
        end
      end
    end

    if defined?(Menu_Items) && Menu_Items.is_a?(Class)
      Menu_Items.class_eval do
        unless method_defined?(:rp_orig_update_input)
          alias_method :rp_orig_update_input, :update_input rescue nil
          def update_input
            if @phase == 4
              # Use exclusive if-elsif to ensure USE and DROP can never be triggered concurrently
              if Input.trigger?(:C) || (defined?(WolfPad) && WolfPad.trigger?(:Z_LINK))
                item_use_confirm_scr_use
              elsif Input.trigger?(:UI_DeleteItem)
                item_use_confirm_scr_dorp
              elsif Input.trigger?(:B) || (defined?(WolfPad) && WolfPad.trigger?(:X_LINK))
                item_use_confirm_scr_cancel
              end
            else
              rp_orig_update_input if respond_to?(:rp_orig_update_input)
            end
          end
        end

        unless method_defined?(:rp_orig_item_use_confirm_scr_dorp)
          alias_method :rp_orig_item_use_confirm_scr_dorp, :item_use_confirm_scr_dorp rescue nil
          def item_use_confirm_scr_dorp
            return unless @phase == 4
            item = @container ? @container[@real_index] : nil
            return if item.nil?
            SndLib.sound_equip_armor(80) rescue nil
            process_item_drop
          end
        end

        unless method_defined?(:rp_orig_process_item_drop)
          alias_method :rp_orig_process_item_drop, :process_item_drop rescue nil
          def process_item_drop
            item = @container ? @container[@real_index] : nil
            if item.nil?
              @phase = (@container.nil? || @container.empty?) ? 2 : 3
              @confirm_back.visible = @confirm_layer.visible = false if @confirm_back && @confirm_layer
              return
            end
            return SndLib.sys_buzzer if item.respond_to?(:key_item?) && item.key_item?

            if Input.press?(:SHIFT)
              tmpCount = [5, $game_party.item_number(item)].min
              $game_party.lose_item(item, $game_party.item_number(item))
              tmpCount.times { $game_temp.register_item_drop([item.item_name]) rescue nil } if defined?($game_temp)
            else
              $game_party.lose_item(item, 1)
              $game_temp.register_item_drop([item.item_name]) rescue nil if defined?($game_temp)
            end
            set_container rescue nil
            post_consumtion rescue nil
            @gauge.refresh rescue nil if @gauge
            @menu_pages[0].refresh rescue nil if @menu_pages && @menu_pages[0]
            @menu_pages[1].refresh rescue nil if @menu_pages && @menu_pages[1]
            refresh_info rescue nil
            @phase = (@container.nil? || @container.empty?) ? 2 : 3
            @confirm_back.visible = @confirm_layer.visible = false if @confirm_back && @confirm_layer
          end
        end
      end
    end
    RuntimePackLangscore.install_save_load_hook rescue nil if defined?(RuntimePackLangscore)
  end

  def self.install_map_hud_hooks
    return if @map_hud_hooked
    if defined?(Map_Hud) && Map_Hud.is_a?(Class)
      @map_hud_hooked = true
      Map_Hud.class_eval do
        unless method_defined?(:rp_orig_initialize)
          alias_method :rp_orig_initialize, :initialize rescue nil
          def initialize(*args)
            rp_orig_initialize(*args)
            # Immediately sanitize all wound sprites upon instantiation
            refresh_wound_layout rescue nil
          end
        end

        unless method_defined?(:rp_orig_refresh_wound_layout)
          alias_method :rp_orig_refresh_wound_layout, :refresh_wound_layout rescue nil
          def refresh_wound_layout
            total_wounds = 0
            total_wounds += refresh_wound_total rescue 0

            # Safely hide or show silhouette background
            if @wound_layout
              @wound_layout.opacity = (total_wounds > 0 ? 255 : 0)
              @wound_layout.visible = (total_wounds > 0)
            end

            # Safely update each limb wound layer (clamp src_rect.y >= 0 and hide if 0)
            update_limb = lambda do |sprite, stat_key, max_val|
              next unless sprite
              val = (@actor && @actor.stat) ? (@actor.stat[stat_key] || 0) : 0
              if val > 0
                sprite.visible = true
                sprite.opacity = 255
                target_y = [26 * (val - 1), max_val].min
                sprite.src_rect.y = target_y if sprite.src_rect && sprite.src_rect.y != target_y
              else
                sprite.visible = false
                sprite.opacity = 0
                sprite.src_rect.y = 0 if sprite.src_rect
              end
            end

            update_limb.call(@wound_head, "WoundHead", 52)
            update_limb.call(@wound_chest, "WoundChest", 52)
            update_limb.call(@wound_sarm, "WoundSArm", 52)
            update_limb.call(@wound_marm, "WoundMArm", 52)
            update_limb.call(@wound_belly, "WoundBelly", 52)
            update_limb.call(@wound_groin, "WoundGroin", 52)
            update_limb.call(@wound_mthigh, "WoundMThigh", 52)
            update_limb.call(@wound_sthigh, "WoundSThigh", 52)

            if @wound_Cuff
              val = (@actor && @actor.stat) ? (@actor.stat["WoundCuff"] || 0) : 0
              @wound_Cuff.visible = (val >= 1)
              @wound_Cuff.opacity = (val >= 1 ? 255 : 0)
            end

            if @wound_Collar
              val = (@actor && @actor.stat) ? (@actor.stat["WoundCollar"] || 0) : 0
              @wound_Collar.visible = (val >= 1)
              @wound_Collar.opacity = (val >= 1 ? 255 : 0)
            end

            @total_wounds = total_wounds
          end
        end
      end
    end
  end

  def self.parse_single_object(s)
    res = {}
    s.scan(/"([^"\\]*(?:\\.[^"\\]*)*)"\s*:\s*("(?:[^"\\]*(?:\\.[^"\\]*)*)"|-?\d+(?:\.\d+)?|true|false|null)/) do |k, v|
      val = if v.start_with?('"')
        v[1..-2].gsub(/\\"/, '"').gsub(/\\\\/, '\\')
      elsif v == "true"
        true
      elsif v == "false"
        false
      elsif v == "null"
        nil
      elsif v =~ /\./
        (v.to_f rescue v)
      else
        (v.to_i rescue v)
      end
      res[k] = val
    end
    res.empty? ? nil : res
  end

  def self.parse_json_fallback(str)
    return nil if str.nil? || str.strip.empty?
    s = str.strip
    if s.start_with?("[") && s.end_with?("]")
      items = []
      s.scan(/\{[^{}]*\}/) do |obj_str|
        parsed = parse_single_object(obj_str)
        items << parsed if parsed
      end
      return items unless items.empty?
    elsif s.start_with?("{") && s.end_with?("}")
      parsed = parse_single_object(s)
      return parsed ? [parsed] : nil
    end
    nil
  end

  def self.update
    install_battle_hooks unless @battle_hooks_installed
    install_save_and_cache_hooks
    install_map_hud_hooks unless @map_hud_hooked

    # Enforce God Mode in frame loop if enabled, or clear when untoggled
    if $runtimepack_god_mode
      @was_god_mode = true
      if defined?($game_player) && $game_player && $game_player.respond_to?(:actor) && $game_player.actor
        a = $game_player.actor
        a.immune_damage = true if a.respond_to?(:immune_damage=)
        a.immune_state_effect = true if a.respond_to?(:immune_state_effect=)
        a.health = 200 if a.respond_to?(:health) && a.respond_to?(:health=) && a.health.to_f < 200
        a.sta = 100 if a.respond_to?(:sta) && a.respond_to?(:sta=) && a.sta.to_f < 100
        a.sat = 100 if a.respond_to?(:sat) && a.respond_to?(:sat=) && a.sat.to_f < 100
        a.mood = 100 if a.respond_to?(:mood) && a.respond_to?(:mood=) && a.mood.to_f < 100
        if a.respond_to?(:wound_state_list) && a.wound_state_list.is_a?(Array)
          a.wound_state_list.each { |w| a.remove_state(w) if a.respond_to?(:remove_state) }
        end
      end
    elsif @was_god_mode
      @was_god_mode = false
      if defined?($game_player) && $game_player && $game_player.respond_to?(:actor) && $game_player.actor
        a = $game_player.actor
        a.immune_damage = false if a.respond_to?(:immune_damage=)
        a.immune_state_effect = false if a.respond_to?(:immune_state_effect=)
      end
    end

    cmd_file = File.exist?(CMD_FILE) ? CMD_FILE : (File.exist?("../#{CMD_FILE}") ? "../#{CMD_FILE}" : nil)
    return unless cmd_file
    begin
      content = File.read(cmd_file)
      File.delete(cmd_file) rescue nil
      return if content.nil? || content.strip.empty?

      # Parse command (handle single hash or array of hashes)
      cmds = (JSON.decode(content) rescue nil) || (JSON.parse(content) rescue nil) || parse_json_fallback(content)
      if cmds.nil? && content =~ /"action"\s*:\s*"eval"\s*,\s*"code"\s*:\s*"([^"]+)"/
        cmds = [{"action" => "eval", "code" => $1}]
      end
      cmds = [cmds] if cmds.is_a?(Hash)
      return unless cmds.is_a?(Array)

      cmds.each { |cmd| process_cmd(cmd) }
    rescue Exception => e
      err_msg = "[CHT-001] #{e.class}: #{e.message}\n#{e.backtrace.join("\n") rescue ''}"
      File.open(ERROR_FILE, "wb") { |f| f.write(err_msg) } rescue nil
      File.open("../#{ERROR_FILE}", "wb") { |f| f.write(err_msg) } rescue nil
    end
  end

  def self.process_cmd(cmd)
    return unless cmd.is_a?(Hash)
    action = cmd["action"].to_s
    case action
    when "set_variable"
      id = cmd["id"].to_i
      val = cmd["value"]
      $game_variables[id] = val if defined?($game_variables) && $game_variables
    when "set_switch"
      id = cmd["id"].to_i
      val = cmd["value"] ? true : false
      $game_switches[id] = val if defined?($game_switches) && $game_switches
    when "gain_gold"
      amt = cmd["amount"].to_i
      $game_party.gain_gold(amt) if defined?($game_party) && $game_party
    when "recover_all"
      if defined?($game_party) && $game_party
        if $game_party.respond_to?(:members) && $game_party.members
          $game_party.members.each { |m| m.recover_all if m.respond_to?(:recover_all) }
        elsif $game_party.respond_to?(:all_members) && $game_party.all_members
          $game_party.all_members.each { |m| m.recover_all if m.respond_to?(:recover_all) }
        end
      end
    when "gain_item"
      if defined?($game_party) && $game_party
        type = cmd["type"].to_s
        id = cmd["id"].to_i
        amt = cmd["amount"].to_i
        db = (type == "weapon" && defined?($data_weapons)) ? $data_weapons :
             ((type == "armor" && defined?($data_armors)) ? $data_armors : (defined?($data_items) ? $data_items : nil))
        item = db[id] if db && db.respond_to?(:[])
        $game_party.gain_item(item, amt) if item
      end
    when "toggle_god_mode"
      $runtimepack_god_mode = cmd.key?("value") ? (cmd["value"] ? true : false) : !$runtimepack_god_mode
      if !$runtimepack_god_mode
        @was_god_mode = false
        if defined?($game_player) && $game_player && $game_player.respond_to?(:actor) && $game_player.actor
          a = $game_player.actor
          a.immune_damage = false if a.respond_to?(:immune_damage=)
          a.immune_state_effect = false if a.respond_to?(:immune_state_effect=)
        end
      else
        @was_god_mode = true
      end
      dump_game_state
    when "toggle_one_hit_kill"
      $runtimepack_one_hit_kill = cmd.key?("value") ? (cmd["value"] ? true : false) : !$runtimepack_one_hit_kill
      dump_game_state
    when "save_anywhere"
      begin
        $story_stats["MenuSysSavegameOff"] = 0 if defined?($story_stats) && $story_stats
        SceneManager.goto(Scene_Save) if defined?(SceneManager) && defined?(Scene_Save)
        File.open(EVAL_FILE, "wb") { |f| f.write("Save Menu Opened") } rescue nil
      rescue Exception => e
        File.open(EVAL_FILE, "wb") { |f| f.write("[CHT-003] #{e.message}") } rescue nil
      end
    when "load_anywhere"
      begin
        $story_stats["MenuSysSavegameOff"] = 0 if defined?($story_stats) && $story_stats
        SceneManager.goto(Scene_Load_OnGameMenu) if defined?(SceneManager) && defined?(Scene_Load_OnGameMenu)
        File.open(EVAL_FILE, "wb") { |f| f.write("Load Menu Opened") } rescue nil
      rescue Exception => e
        File.open(EVAL_FILE, "wb") { |f| f.write("[CHT-003] #{e.message}") } rescue nil
      end
    when "quick_save"
      begin
        if defined?(DataManager) && DataManager.respond_to?(:save_custom_without_header)
          DataManager.save_custom_without_header("SavQuick")
          File.open(EVAL_FILE, "wb") { |f| f.write("Quick Saved to SavQuick") } rescue nil
        end
      rescue Exception => e
        File.open(EVAL_FILE, "wb") { |f| f.write("[CHT-003] #{e.message}") } rescue nil
      end
    when "quick_load"
      begin
        if defined?(DataManager) && DataManager.respond_to?(:load_custom_without_rescue)
          if DataManager.load_custom_without_rescue("SavQuick")
            $game_system.on_after_load if defined?($game_system) && $game_system.respond_to?(:on_after_load)
            SceneManager.goto(Scene_Map) if defined?(SceneManager) && defined?(Scene_Map)
            $game_player.refresh_chs if defined?($game_player) && $game_player.respond_to?(:refresh_chs)
            $game_player.actor.portrait.update rescue nil if defined?($game_player) && $game_player && $game_player.actor && $game_player.actor.respond_to?(:portrait)
            File.open(EVAL_FILE, "wb") { |f| f.write("Quick Loaded from SavQuick") } rescue nil
          else
            File.open(EVAL_FILE, "wb") { |f| f.write("No SavQuick file found") } rescue nil
          end
        end
      rescue Exception => e
        File.open(EVAL_FILE, "wb") { |f| f.write("[CHT-003] #{e.message}") } rescue nil
      end
    when "dump_state"
      dump_game_state
    when "eval"
      code = cmd["code"].to_s
      begin
        res = eval(code, TOPLEVEL_BINDING)
        File.open(EVAL_FILE, "wb") { |f| f.write(res.inspect) } rescue nil
        File.open("../#{EVAL_FILE}", "wb") { |f| f.write(res.inspect) } rescue nil
      rescue Exception => err
        File.open(EVAL_FILE, "wb") { |f| f.write("[CHT-002] #{err.class}: #{err.message}") } rescue nil
        File.open("../#{EVAL_FILE}", "wb") { |f| f.write("[CHT-002] #{err.class}: #{err.message}") } rescue nil
      end
    when "teleport_map"
      map_id = (cmd["map_id"] || cmd["mapId"] || 1).to_i
      x = (cmd["x"] || 0).to_i
      y = (cmd["y"] || 0).to_i
      teleport_map(map_id, x, y)
    when "export_bug_report"
      export_bug_report
    when "run_stress_test"
      run_stress_test
    end
  end

  def self.teleport_map(map_id, x, y)
    if defined?($game_player) && $game_player
      if $game_player.respond_to?(:reserve_transfer)
        $game_player.reserve_transfer(map_id, x, y)
      elsif defined?($game_temp) && $game_temp
        $game_temp.player_transferring = true
        $game_temp.player_new_map_id = map_id
        $game_temp.player_new_x = x
        $game_temp.player_new_y = y
      end
      if defined?(SceneManager) && defined?(Scene_Map) && SceneManager.respond_to?(:goto)
        SceneManager.goto(Scene_Map) if SceneManager.scene.class != Scene_Map rescue nil
      end
      msg = "Teleported to Map #{map_id} (#{x}, #{y})"
      File.open(EVAL_FILE, "wb") { |f| f.write(msg) } rescue nil
      File.open("../#{EVAL_FILE}", "wb") { |f| f.write(msg) } rescue nil
      return true
    end
    false
  end

  def self.export_bug_report
    scene_name = if defined?(SceneManager) && SceneManager.respond_to?(:scene) && SceneManager.scene
      SceneManager.scene.class.name
    elsif defined?($scene) && $scene
      $scene.class.name
    else
      "Unknown"
    end
    map_id = (defined?($game_map) && $game_map && $game_map.respond_to?(:map_id)) ? ($game_map.map_id rescue nil) : nil
    player_x = (defined?($game_player) && $game_player && $game_player.respond_to?(:x)) ? ($game_player.x rescue nil) : nil
    player_y = (defined?($game_player) && $game_player && $game_player.respond_to?(:y)) ? ($game_player.y rescue nil) : nil
    gold = (defined?($game_party) && $game_party && $game_party.respond_to?(:gold)) ? ($game_party.gold rescue 0) : 0

    bug_report = {
      "timestamp" => Time.now.to_s,
      "scene" => scene_name,
      "map_id" => map_id,
      "player" => { "x" => player_x, "y" => player_y },
      "gold" => gold,
      "ruby_version" => RUBY_VERSION,
      "platform" => RUBY_PLATFORM,
      "god_mode" => ($runtimepack_god_mode ? true : false),
      "one_hit_kill" => ($runtimepack_one_hit_kill ? true : false)
    }
    if File.exist?(ERROR_FILE)
      bug_report["recent_error"] = (File.read(ERROR_FILE) rescue nil)
    elsif File.exist?("../#{ERROR_FILE}")
      bug_report["recent_error"] = (File.read("../#{ERROR_FILE}") rescue nil)
    end

    json_str = (defined?(JSON) && JSON.respond_to?(:pretty_generate)) ? JSON.pretty_generate(bug_report) : serialize_fallback(bug_report)
    File.open("bug_report.json", "wb") { |f| f.write(json_str) } rescue nil
    File.open("../bug_report.json", "wb") { |f| f.write(json_str) } rescue nil
    File.open("game/bug_report.json", "wb") { |f| f.write(json_str) } rescue nil if File.directory?("game")

    msg = "Bug report exported: Scene=#{scene_name}, Map=#{map_id}, Player=(#{player_x}, #{player_y})"
    File.open(EVAL_FILE, "wb") { |f| f.write(msg) } rescue nil
    File.open("../#{EVAL_FILE}", "wb") { |f| f.write(msg) } rescue nil
    bug_report
  end

  def self.verify_disk_asset(base_path, extensions = [""])
    norm = base_path.to_s.tr('\\', '/').sub(/^\.?\//, '')
    norm_without_ext = norm.sub(/\.[^.\/]+$/, "")
    has_ext = (norm != norm_without_ext)

    exts = has_ext ? [""] : extensions
    prefixes = ["", "game/"]
    found_case_mismatch = nil

    prefixes.each do |prefix|
      exts.each do |ext|
        candidate = "#{prefix}#{norm}#{ext}"
        exact_found = if File.respond_to?(:runtimepack_original_exist?)
          File.runtimepack_original_exist?(candidate)
        else
          File.exist?(candidate)
        end

        return { "status" => "EXACT", "path" => candidate } if exact_found

        if defined?(RuntimePackCasePath) && RuntimePackCasePath.respond_to?(:resolve)
          resolved = RuntimePackCasePath.resolve(candidate) rescue candidate
          res_found = if File.respond_to?(:runtimepack_original_exist?)
            File.runtimepack_original_exist?(resolved)
          else
            File.exist?(resolved)
          end
          if res_found
            if resolved == candidate
              return { "status" => "EXACT", "path" => candidate }
            else
              found_case_mismatch ||= { "status" => "CASE_MISMATCH", "path" => candidate, "resolved" => resolved }
            end
          end
        end
      end
    end

    return found_case_mismatch if found_case_mismatch
    { "status" => "MISSING", "path" => norm }
  end

  def self.load_game_data(path)
    if defined?(load_data) || Kernel.respond_to?(:load_data, true)
      begin
        return load_data(path)
      rescue Exception
      end
      if path.start_with?("game/")
        begin
          return load_data(path.sub(/^game\//, ''))
        rescue Exception
        end
      end
    end
    if File.exist?(path)
      begin
        return File.open(path, "rb") { |f| Marshal.load(f) }
      rescue Exception
      end
    end
    nil
  end

  def self.report_stress_progress(line)
    File.open("rp_audit_progress.txt", "ab") { |f| f.puts(line) } rescue nil
    File.open("../rp_audit_progress.txt", "ab") { |f| f.puts(line) } rescue nil
    File.open("game/rp_audit_progress.txt", "ab") { |f| f.puts(line) } rescue nil if File.directory?("game")
  end

  def self.run_stress_test
    File.delete("rp_audit_progress.txt") rescue nil
    File.delete("../rp_audit_progress.txt") rescue nil
    File.delete("game/rp_audit_progress.txt") rescue nil if File.directory?("game")
    report_stress_progress("START: Initializing Asset Graph & Database Audit...")

    checked_assets = {}
    scanned_maps = []
    all_transfers = []
    broken_transfers = []
    case_mismatches = []
    missing_assets = []
    exact_assets_count = 0

    verify_asset = lambda do |asset_path, exts, asset_type|
      next if asset_path.nil? || asset_path.to_s.strip.empty?
      clean = asset_path.to_s.strip
      base_path = clean
      key = "#{clean}:#{exts.join(',')}"
      next if checked_assets.key?(key)

      res = verify_disk_asset(clean, exts)
      checked_assets[key] = res["status"]
      case res["status"]
      when "EXACT"
        exact_assets_count += 1
      when "CASE_MISMATCH"
        case_mismatches << { "referenced" => clean, "resolved" => res["resolved"], "type" => asset_type }
      when "MISSING"
        missing_assets << { "referenced" => clean, "type" => asset_type }
        report_stress_progress("FAIL: Missing #{asset_type}: #{base_path}")
      end
    end
    audit_asset = verify_asset

    audit_commands = lambda do |cmd_list, from_id|
      next unless cmd_list.is_a?(Array)
      cmd_list.each do |c|
        next unless c && c.respond_to?(:code)
        params = c.parameters || []
        case c.code
        when 201 # Transfer player
          target_id = params[1].to_i
          if params[0] == 0 || params.size > 1
            all_transfers << { "from_map" => from_id, "target_map_id" => target_id, "x" => params[2].to_i, "y" => params[3].to_i }
          end
        when 231 # Show picture
          pic_name = params[1].to_s
          audit_asset.call("Graphics/Pictures/#{pic_name}", [".png", ".jpg", ".jpeg"], "Picture") unless pic_name.empty?
        when 241 # Play BGM
          bgm = params[0]
          name = (bgm.respond_to?(:name) ? bgm.name : bgm.to_s)
          audit_asset.call("Audio/BGM/#{name}", [".ogg", ".wav", ".mp3", ".mid"], "BGM") unless name.empty?
        when 245 # Play BGS
          bgs = params[0]
          name = (bgs.respond_to?(:name) ? bgs.name : bgs.to_s)
          audit_asset.call("Audio/BGS/#{name}", [".ogg", ".wav", ".mp3"], "BGS") unless name.empty?
        when 250 # Play SE
          se = params[0]
          name = (se.respond_to?(:name) ? se.name : se.to_s)
          audit_asset.call("Audio/SE/#{name}", [".ogg", ".wav", ".mp3"], "SE") unless name.empty?
        when 322 # Change Actor Graphic
          char_name = params[1].to_s
          face_name = params[3].to_s
          audit_asset.call("Graphics/Characters/#{char_name}", [".png", ".jpg", ".jpeg"], "Character") unless char_name.empty?
          audit_asset.call("Graphics/Faces/#{face_name}", [".png", ".jpg", ".jpeg"], "Face") unless face_name.empty?
        end
      end
    end

    map_files = (Dir.glob("Data/Map[0-9]*.[rR]*") + Dir.glob("game/Data/Map[0-9]*.[rR]*")).uniq.sort
    known_map_ids = {}

    map_files.each_with_index do |mf, idx|
      next if mf =~ /MapInfos/i
      m_id = (mf =~ /Map(\d+)/i) ? $1.to_i : nil
      known_map_ids[m_id] = true if m_id

      if (idx % 15 == 0) || idx == map_files.size - 1
        report_stress_progress("MAP: Scanning Map #{m_id || idx} (#{idx + 1}/#{map_files.size})...")
      end

      map_obj = load_game_data(mf)
      next unless map_obj

      scanned_maps << { "map_id" => m_id, "file" => mf }

      if map_obj.respond_to?(:autoplay_bgm) && map_obj.autoplay_bgm && map_obj.respond_to?(:bgm) && map_obj.bgm
        bgm_name = map_obj.bgm.name.to_s
        audit_asset.call("Audio/BGM/#{bgm_name}", [".ogg", ".wav", ".mp3", ".mid"], "BGM") unless bgm_name.empty?
      end
      if map_obj.respond_to?(:autoplay_bgs) && map_obj.autoplay_bgs && map_obj.respond_to?(:bgs) && map_obj.bgs
        bgs_name = map_obj.bgs.name.to_s
        audit_asset.call("Audio/BGS/#{bgs_name}", [".ogg", ".wav", ".mp3"], "BGS") unless bgs_name.empty?
      end

      if map_obj.respond_to?(:events) && map_obj.events.is_a?(Hash)
        map_obj.events.values.each do |ev|
          next unless ev && ev.respond_to?(:pages) && ev.pages.is_a?(Array)
          ev.pages.each do |page|
            next unless page && page.respond_to?(:list)
            audit_commands.call(page.list, m_id)
          end
        end
      end
    end

    report_stress_progress("DATABASE: Auditing Database entities (Actors, Enemies, Tilesets, Animations)...")

    # Inspect Database files
    db_candidates = [
      ["Actors", ["Data/Actors.rvdata2", "Data/Actors.rvdata", "Data/Actors.rxdata", "game/Data/Actors.rvdata2"]],
      ["Enemies", ["Data/Enemies.rvdata2", "Data/Enemies.rvdata", "Data/Enemies.rxdata", "game/Data/Enemies.rvdata2"]],
      ["Tilesets", ["Data/Tilesets.rvdata2", "Data/Tilesets.rvdata", "Data/Tilesets.rxdata", "game/Data/Tilesets.rvdata2"]],
      ["Animations", ["Data/Animations.rvdata2", "Data/Animations.rvdata", "Data/Animations.rxdata", "game/Data/Animations.rvdata2"]],
      ["CommonEvents", ["Data/CommonEvents.rvdata2", "Data/CommonEvents.rvdata", "Data/CommonEvents.rxdata", "game/Data/CommonEvents.rvdata2"]]
    ]

    db_candidates.each do |db_name, paths|
      path = paths.find { |p| File.exist?(p) }
      next unless path
      data = load_game_data(path)
      next unless data.is_a?(Array)

      data.compact.each do |entry|
        case db_name
        when "Actors"
          if entry.respond_to?(:character_name) && !entry.character_name.to_s.empty?
            audit_asset.call("Graphics/Characters/#{entry.character_name}", [".png", ".jpg", ".jpeg"], "Character")
          end
          if entry.respond_to?(:face_name) && !entry.face_name.to_s.empty?
            audit_asset.call("Graphics/Faces/#{entry.face_name}", [".png", ".jpg", ".jpeg"], "Face")
          end
          if entry.respond_to?(:battler_name) && !entry.battler_name.to_s.empty?
            audit_asset.call("Graphics/Battlers/#{entry.battler_name}", [".png", ".jpg", ".jpeg"], "Battler")
          end
        when "Enemies"
          if entry.respond_to?(:battler_name) && !entry.battler_name.to_s.empty?
            audit_asset.call("Graphics/Battlers/#{entry.battler_name}", [".png", ".jpg", ".jpeg"], "Battler")
          end
        when "Tilesets"
          if entry.respond_to?(:tileset_names) && entry.tileset_names.is_a?(Array)
            entry.tileset_names.each do |tn|
              audit_asset.call("Graphics/Tilesets/#{tn}", [".png", ".jpg", ".jpeg"], "Tileset") unless tn.to_s.empty?
            end
          end
          if entry.respond_to?(:tileset_name) && !entry.tileset_name.to_s.empty?
            audit_asset.call("Graphics/Tilesets/#{entry.tileset_name}", [".png", ".jpg", ".jpeg"], "Tileset")
          end
          if entry.respond_to?(:autotile_names) && entry.autotile_names.is_a?(Array)
            entry.autotile_names.each do |an|
              audit_asset.call("Graphics/Autotiles/#{an}", [".png", ".jpg", ".jpeg"], "Autotile") unless an.to_s.empty?
            end
          end
        when "Animations"
          if entry.respond_to?(:animation1_name) && !entry.animation1_name.to_s.empty?
            audit_asset.call("Graphics/Animations/#{entry.animation1_name}", [".png", ".jpg", ".jpeg"], "Animation")
          end
          if entry.respond_to?(:animation2_name) && !entry.animation2_name.to_s.empty?
            audit_asset.call("Graphics/Animations/#{entry.animation2_name}", [".png", ".jpg", ".jpeg"], "Animation")
          end
        when "CommonEvents"
          if entry.respond_to?(:list) && entry.list.is_a?(Array)
            audit_commands.call(entry.list, 0)
          end
        end
      end
    end

    all_transfers.each do |tr|
      target_id = tr["target_map_id"] || tr[:target_map] || tr["target_map"]
      if target_id && target_id > 0 && !known_map_ids.key?(target_id)
        source_desc = tr[:source] || tr["source"] || "Map #{tr['from_map'] || tr[:from_map]}"
        t = {
          "from_map" => tr["from_map"] || tr[:from_map],
          "target_map_id" => target_id,
          "x" => tr["x"] || tr[:x],
          "y" => tr["y"] || tr[:y],
          :target_map => target_id,
          :source => source_desc,
          "target_map" => target_id,
          "source" => source_desc
        }
        broken_transfers << t
        report_stress_progress("FAIL: Broken transfer to Map #{t[:target_map]} at #{t[:source]}")
      end
    end

    stats = {
      "total_maps_scanned" => scanned_maps.size,
      "total_assets_checked" => checked_assets.size,
      "exact_count" => exact_assets_count,
      "case_mismatch_count" => case_mismatches.size,
      "missing_count" => missing_assets.size,
      "broken_transfers_count" => broken_transfers.size
    }
    report = {
      "timestamp" => Time.now.to_s,
      "stats" => stats,
      "broken_transfers" => broken_transfers,
      "case_mismatches" => case_mismatches,
      "missing_assets" => missing_assets,
      "scanned_maps" => scanned_maps
    }
    json_str = (defined?(JSON) && JSON.respond_to?(:pretty_generate)) ? JSON.pretty_generate(report) : serialize_fallback(report)
    File.open("audit_report.json", "wb") { |f| f.write(json_str) } rescue nil
    File.open("../audit_report.json", "wb") { |f| f.write(json_str) } rescue nil
    File.open("game/audit_report.json", "wb") { |f| f.write(json_str) } rescue nil if File.directory?("game")

    msg = "Stress test completed: #{stats['total_maps_scanned']} maps, #{stats['exact_count']} exact assets, #{stats['case_mismatch_count']} case mismatches, #{stats['missing_count']} missing."
    report_stress_progress("DONE: #{msg}")
    File.open(EVAL_FILE, "wb") { |f| f.write(msg) } rescue nil
    File.open("../#{EVAL_FILE}", "wb") { |f| f.write(msg) } rescue nil
    report
  end

  def self.dump_game_state
    gold = (defined?($game_party) && $game_party) ? ($game_party.gold rescue 0) : 0
    vars = {}
    switches = {}
    if defined?($game_variables) && $game_variables
      1.upto(300) do |i|
        v = $game_variables[i] rescue nil
        vars[i.to_s] = v if v != nil && v != 0
      end
    end
    if defined?($game_switches) && $game_switches
      1.upto(300) do |i|
        s = $game_switches[i] rescue false
        switches[i.to_s] = s if s
      end
    end
    payload = {
      "gold" => gold,
      "variables" => vars,
      "switches" => switches,
      "god_mode" => ($runtimepack_god_mode ? true : false),
      "one_hit_kill" => ($runtimepack_one_hit_kill ? true : false)
    }
    # Write JSON
    encoded = serialize_fallback(payload)
    File.open(STATE_FILE, "wb") { |f| f.write(encoded.to_s) } rescue nil
    File.open("../#{STATE_FILE}", "wb") { |f| f.write(encoded.to_s) } rescue nil
  end

  def self.serialize_fallback(obj)
    case obj
    when Hash
      "{" + obj.map { |k, v| "#{k.to_s.inspect}:#{serialize_fallback(v)}" }.join(",") + "}"
    when Array
      "[" + obj.map { |v| serialize_fallback(v) }.join(",") + "]"
    when String
      obj.inspect
    when Numeric
      obj.to_s
    when true, false
      obj.to_s
    when nil
      "null"
    else
      obj.to_s.inspect
    end
  end

  def self.hook_engine
    install_save_and_cache_hooks
    if defined?(Graphics)
      class << Graphics
        unless method_defined?(:rp_orig_cheat_update)
          alias_method :rp_orig_cheat_update, :update rescue nil
          def update(*args)
            RuntimePackCheatRunner.update rescue nil
            send(:rp_orig_cheat_update, *args) if respond_to?(:rp_orig_cheat_update, true)
          end
        end
      end
    end
    if defined?(Input)
      class << Input
        unless method_defined?(:rp_orig_cheat_input_update)
          alias_method :rp_orig_cheat_input_update, :update rescue nil
          def update(*args)
            RuntimePackCheatRunner.update rescue nil
            send(:rp_orig_cheat_input_update, *args) if respond_to?(:rp_orig_cheat_input_update, true)
          end
        end
      end
    end
  end

  def self.hook_graphics
    hook_engine
  end
end

RuntimePackCheatRunner.hook_engine

def patch_input_fallbacks
  if defined?(Input) && Input.respond_to?(:rp_native_press?) && !Input.respond_to?(:__rp_input_patched)
    class << Input
      define_method(:__rp_input_patched) { true }
      alias_method :rp_hime_press?, :press? rescue nil
      alias_method :rp_hime_trigger?, :trigger? rescue nil
      alias_method :rp_hime_dir4, :dir4 rescue nil
      alias_method :rp_hime_dir8, :dir8 rescue nil

      def press?(keys)
        res = rp_hime_press?(keys) rescue false
        return true if res
        rp_native_press?(keys) rescue false
      end

      def trigger?(keys)
        res = rp_hime_trigger?(keys) rescue false
        return true if res
        rp_native_trigger?(keys) rescue false
      end

      def dir4
        d = rp_hime_dir4 rescue 0
        return d if d && d > 0
        rp_native_dir4 rescue 0
      end

      def dir8
        d = rp_hime_dir8 rescue 0
        return d if d && d > 0
        rp_native_dir8 rescue 0
      end
    end
  end
end

module RuntimePackScriptLoader
  MAGIC_COMMENT = "# encoding: utf-8\n"
  EVAL_START_LINE = 0

  def self.eval_start_line
    EVAL_START_LINE
  end

  def self.prepare_script_source(raw_source)
    script_source = raw_source.to_s.dup
    if script_source.respond_to?(:force_encoding)
      script_source.force_encoding("UTF-8")
      unless script_source.valid_encoding?
        script_source = script_source.encode("UTF-8", "Shift_JIS", invalid: :replace, undef: :replace, replace: "") rescue script_source.force_encoding("UTF-8")
      end
      if script_source.byteslice(0, 3)&.b == "\xEF\xBB\xBF".b
        orig_enc = script_source.encoding
        script_source = (script_source.byteslice(3..-1) || "").force_encoding(orig_enc)
      end
      script_source = MAGIC_COMMENT + script_source
    end
    script_source
  end

  def self.eval_script(script_id, script_name, raw_source, target_binding = TOPLEVEL_BINDING)
    script_source = prepare_script_source(raw_source)
    eval(script_source, target_binding, "{#{script_id.to_s.rjust(4, '0')}}:#{script_name}", EVAL_START_LINE)
  end
end

if defined?(load_data) || respond_to?(:load_data, true)
  if (File.exist?('preload/ruby18_comp.rb') rescue false)
    begin
      load 'preload/ruby18_comp.rb'
    rescue => e
      puts "[RuntimePack] Warning loading preload/ruby18_comp.rb: #{e}"
    end
  end

  script_candidates = []
  ['To the Moon.ini', 'Game.ini'].each do |ini|
    if (File.exist?(ini) rescue false)
      content = (File.read(ini) rescue '')
      if content =~ /Scripts\s*=\s*(.+)/i
        script_candidates << $1.strip.tr('\\', '/')
      end
    end
  end
  script_candidates += ['Data/Scripts.rxdata', 'Data/Scripts.rvdata', 'Data/Scripts.rvdata2']

  scripts = nil
  script_path_used = nil
  script_candidates.uniq.each do |cand|
    begin
      s = load_data(cand)
      if s && s.is_a?(Array) && !s.empty?
        scripts = s
        script_path_used = cand
        break
      end
    rescue => e
      # continue to next candidate
    end
  end

  if scripts
    puts "[RuntimePack] Loading scripts from #{script_path_used} (#{scripts.size} entries)..."
    File.open("all_script_names.txt", "w") do |f|
      scripts.each_with_index { |s, i| f.puts("#{i}: #{s[1]}") }
    end rescue nil
    scripts.each_with_index do |s, i|
      name = s[1].to_s
      if name.downcase.include?('lang') || name.downcase.include?('splash') || name.downcase.include?('mouse') || name.downcase.include?('input') || name.downcase.include?('main') || name.downcase.include?('title')
        src = (Zlib::Inflate.inflate(s[2]) rescue '')
        File.open("script_#{i}_#{name.gsub(/[^a-zA-Z0-9_]/, '_')}.txt", "wb") { |f| f.write(src) } rescue nil
      end
    end rescue nil
    scripts.each_with_index do |script, idx|
      script_id = script[0]
      script_name = script[1].to_s

      # If this is the main game script or the last script, prepare all hooks and defaults beforehand
      if script_name.downcase == 'main' || idx == scripts.size - 1
        if defined?(TR) && TR.respond_to?(:lang_setting_file_path)
          begin
            path = TR.lang_setting_file_path
            File.open(path, "wb") { |f| f.write("en") } unless (File.exist?(path) rescue false)
            puts "[RuntimePack] Set default language 'en' at #{path}"
          rescue => e
            puts "[RuntimePack] Failed setting default language: #{e}"
          end
        end
        patch_input_fallbacks
        RuntimePackCheatRunner.hook_engine
        if defined?(RuntimePackLangscore)
          RuntimePackLangscore.install_save_load_hook rescue nil
          RuntimePackLangscore.restore_bootstrap_language rescue nil
        end
      end

      script_source = Zlib::Inflate.inflate(script[2])
      begin
        RuntimePackScriptLoader.eval_script(script_id, script_name, script_source)
      rescue => e
        label = "{#{script_id.to_s.rjust(4, '0')}}:#{script_name}"
        _name, line = RuntimePackCrashReport.attribute(e, label)
        report = RuntimePackCrashReport.format(e, label, line)
        puts "=================================================="
        puts report
        puts "=================================================="
        File.open("Error.log", "wb") { |f| f.write(report) } rescue nil
        raise
      end
      BitmapChangerOptimization.invalidate if defined?(BitmapChangerOptimization) && BitmapChangerOptimization.respond_to?(:invalidate)
      patch_json_decoder if respond_to?(:patch_json_decoder)
      RuntimePackCheatRunner.install_save_and_cache_hooks rescue nil
    end
    if defined?(RuntimePackLangscore)
      RuntimePackLangscore.install_save_load_hook rescue nil
      RuntimePackLangscore.restore_bootstrap_language rescue nil
    end
    RuntimePackCompatibility.normalize_input_utils if defined?(RuntimePackCompatibility) && RuntimePackCompatibility.respond_to?(:normalize_input_utils)
    if defined?(Game_Map)
      class Game_Map
        if method_defined?(:bmsp_mapfog_update_fog)
          alias_method :rp_orig_bmsp_mapfog_update_fog, :bmsp_mapfog_update_fog rescue nil
          def bmsp_mapfog_update_fog
            return if !@bmsp_fog || !@bmsp_fog.is_a?(Hash) || !@bmsp_fog[:file] || @bmsp_fog[:file].empty?
            rp_orig_bmsp_mapfog_update_fog
          end
        end
      end
    end
  end
end

if defined?(SceneManager) && SceneManager.respond_to?(:run)
  module SceneManager
    class << self
      alias_method :runtimepack_orig_run, :run rescue nil
      def run
        begin
          runtimepack_orig_run
        rescue => e
          report = RuntimePackCrashReport.format(e)
          puts "=================================================="
          puts report
          puts "=================================================="
          File.open("Error.log", "wb") { |f| f.write(report) } rescue nil
          raise
        end
      end
    end
  end
end
