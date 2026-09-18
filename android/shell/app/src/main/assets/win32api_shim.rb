# RuntimePack Win32API Compatibility Shim for Android RGSS
class Win32API
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
      if args[4].is_a?(String)
        val = args[1].to_s
        args[4][0, val.bytesize] = val
        val.bytesize
      else
        0
      end
    when 'WritePrivateProfileString'
      1
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
