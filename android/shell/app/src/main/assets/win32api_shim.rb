# RuntimePack Win32API Compatibility Shim for Android RGSS

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
    when 'GetKeyboardState', 'GetAsyncKeyState'
      if args[0].is_a?(String)
        args[0].replace("\0" * args[0].bytesize)
      end
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
    when 'GetWindowRect'
      if args[1].is_a?(String) && args[1].bytesize >= 16
        args[1][0, 16] = [0, 0, 544, 416].pack('llll')
      end
      1
    when 'FindWindowEx', 'ShowCursor', 'ClipCursor'
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
        len = args[2] || args[1].bytesize
        args[0][0, len] = args[1][0, len]
      end
      0
    when 'get_mouse_wheel', 'unload'
      0
    when 'hook'
      0
    else
      0
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

