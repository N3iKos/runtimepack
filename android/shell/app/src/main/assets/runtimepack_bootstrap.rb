# RuntimePack post-binding bootstrap for RGSS3 Android.
# This script is executed by mkxp-z after native bindings are initialized.

Encoding.default_external = Encoding::UTF_8
Encoding.default_internal = Encoding::UTF_8

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

module TKG
  module ErrorLog
    def self.save(exception = $!)
      return unless exception
      msg = "[TKG::ErrorLog] #{exception.class}: #{exception.message}\n"
      if exception.backtrace
        msg += exception.backtrace.join("\n")
      end
      p msg
      begin
        File.open("Error.log", "wb") { |f| f.write(msg) }
      rescue => ex
        p "Failed writing Error.log: #{ex}"
      end
    end
  end
end

require 'zlib'

class << File
  alias_method :runtimepack_original_exist?, :exist?
  alias_method :runtimepack_original_size, :size
  alias_method :runtimepack_original_file?, :file?
  alias_method :runtimepack_original_directory?, :directory?
  alias_method :runtimepack_original_open, :open
  alias_method :runtimepack_original_read, :read
  alias_method :runtimepack_original_binread, :binread if method_defined?(:binread)
end

module RuntimePackCasePath
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

  def open(path, *args, &block)
    runtimepack_original_open(RuntimePackCasePath.resolve(path), *args, &block)
  end

  def read(path, *args)
    runtimepack_original_read(RuntimePackCasePath.resolve(path), *args)
  end

  if method_defined?(:runtimepack_original_binread)
    def binread(path, *args)
      runtimepack_original_binread(RuntimePackCasePath.resolve(path), *args)
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

      if File.exist?("rp_input_state.bin")
        data = File.binread("rp_input_state.bin") rescue nil
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
      if File.exist?("rp_input_state.bin")
        data = File.binread("rp_input_state.bin") rescue nil
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
      buffer = args[4]
      if buffer.is_a?(String)
        value = args[1].to_s
        buffer[0, value.bytesize] = value
        value.bytesize
      else
        0
      end
    when 'WritePrivateProfileString'
      1
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
      0
    end
  end
end

[:Sprite, :Viewport, :Plane, :Window].each do |klass_name|
  if Object.const_defined?(klass_name)
    klass = Object.const_get(klass_name)
    if klass.method_defined?(:visible=)
      klass.class_eval do
        alias_method :runtimepack_orig_visible=, :visible=
        def visible=(val)
          self.runtimepack_orig_visible = (val ? true : false)
        end
      end
    end
  end
end

def patch_json_decoder
  if defined?(JSON) && JSON.respond_to?(:next_token) && !JSON.respond_to?(:__rp_patched)
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

if defined?(Input)
  class << Input
    alias_method :rp_native_press?, :press? rescue nil
    alias_method :rp_native_trigger?, :trigger? rescue nil
    alias_method :rp_native_dir4, :dir4 rescue nil
    alias_method :rp_native_dir8, :dir8 rescue nil
  end
end

if (defined?(load_data) || respond_to?(:load_data, true)) && File.exist?('Data/Scripts.rvdata2')
  scripts = load_data('Data/Scripts.rvdata2')
  scripts.each do |script|
    script_id = script[0]
    script_name = script[1].to_s
    script_source = Zlib::Inflate.inflate(script[2])
    eval(script_source, TOPLEVEL_BINDING, "{#{script_id.to_s.rjust(4, '0')}}:#{script_name}")
    BitmapChangerOptimization.invalidate if defined?(BitmapChangerOptimization) && BitmapChangerOptimization.respond_to?(:invalidate)
    patch_json_decoder
  end
end

if defined?(Input) && Input.respond_to?(:rp_native_press?)
  class << Input
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

if defined?(SceneManager) && SceneManager.respond_to?(:run)
  module SceneManager
    class << self
      alias_method :runtimepack_orig_run, :run rescue nil
      def run
        begin
          runtimepack_orig_run
        rescue => e
          puts "=================================================="
          puts "[FATAL SCENEMANAGER ERROR]: #{e.class}: #{e.message}"
          puts e.backtrace
          puts "=================================================="
          raise
        end
      end
    end
  end
end
