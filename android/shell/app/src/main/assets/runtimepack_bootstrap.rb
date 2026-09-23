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

begin
  require 'zlib'
rescue LoadError, ScriptError, StandardError
end

begin
  require 'json'
rescue LoadError, ScriptError, StandardError
end

class << File
  alias_method :runtimepack_original_exist?, :exist?
  alias_method :runtimepack_original_size, :size
  alias_method :runtimepack_original_file?, :file?
  alias_method :runtimepack_original_directory?, :directory?
  alias_method :runtimepack_original_open, :open
  alias_method :runtimepack_original_read, :read
  alias_method :runtimepack_original_binread, :binread if method_defined?(:binread)
end

class Regexp
  unless method_defined?(:rp_orig_match_op)
    alias_method :rp_orig_match_op, :=~ rescue nil
    def =~(str)
      begin
        rp_orig_match_op(str)
      rescue Encoding::CompatibilityError
        rp_orig_match_op(str.to_s.dup.force_encoding("UTF-8")) rescue nil
      end
    end
  end
end

class String
  unless method_defined?(:rp_orig_scan)
    alias_method :rp_orig_scan, :scan rescue nil
    def scan(pattern, &block)
      begin
        rp_orig_scan(pattern, &block)
      rescue Encoding::CompatibilityError
        if pattern.is_a?(Regexp)
          p_utf8 = Regexp.new(pattern.source.dup.force_encoding("UTF-8"), pattern.options) rescue pattern
          rp_orig_scan(p_utf8, &block) rescue []
        else
          []
        end
      end
    end
  end
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
      # Default stub returns 1 (Win32 TRUE / success for wrapper DLLs like Utils, Steamworks)
      1
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

class RGSSError < StandardError; end unless defined?(RGSSError)

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
end unless defined?(Bitmap)

module Audio
  class << self
    def bgm_play(*args); end
    def bgs_play(*args); end
    def me_play(*args); end
    def se_play(*args); end
    def bgm_stop; end
    def bgs_stop; end
    def me_stop; end
    def se_stop; end
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
end

RuntimePackCompatibility.install_font_setters

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
        :character, :face, :fog, :gameover, :icon, :picture, :system, :tile,
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
                  puts "[RuntimePack] Missing bitmap in #{cache_mod}.#{m}(#{args.first}): #{e.class}: #{e.message}, returning dummy bitmap"
                  (Bitmap.new(32, 32) rescue nil)
                end
              end
            end
          end
        end
      end

      if cache_mod.respond_to?(:normal_bitmap) && !cache_mod.respond_to?(:rp_orig_normal_bitmap)
        cache_target.class_eval do
          alias_method :rp_orig_normal_bitmap, :normal_bitmap rescue nil
          def normal_bitmap(path)
            begin
              rp_orig_normal_bitmap(path)
            rescue Errno::ENOENT, (defined?(RGSSError) ? RGSSError : StandardError), StandardError => e
              puts "[RuntimePack] Missing bitmap in #{cache_mod}.normal_bitmap: #{path}, falling back to empty bitmap"
              @cache ||= {}
              @cache[path] = (Bitmap.new(32, 32) rescue nil)
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

    if defined?(Bitmap)
      class << Bitmap
        unless method_defined?(:rp_orig_bitmap_new)
          alias_method :rp_orig_bitmap_new, :new rescue nil
          def new(*args)
            begin
              rp_orig_bitmap_new(*args)
            rescue Errno::ENOENT, (defined?(RGSSError) ? RGSSError : StandardError), StandardError => e
              if args.size <= 1
                puts "[RuntimePack] Bitmap not found or failed to load: #{args[0].inspect} (#{e.class}: #{e.message}), substituting dummy bitmap"
                if args[0].is_a?(String) && args[0].downcase.include?("save") && (File.exist?("Graphics/System/SaveNotFound.png") rescue false)
                  (rp_orig_bitmap_new("Graphics/System/SaveNotFound.png") rescue rp_orig_bitmap_new(32, 32))
                else
                  rp_orig_bitmap_new(32, 32)
                end
              else
                raise e
              end
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
      cmds = (JSON.decode(content) rescue nil) || (JSON.parse(content) rescue nil)
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
    end
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
      end

      script_source = Zlib::Inflate.inflate(script[2])
      if script_source.respond_to?(:force_encoding)
        script_source = script_source.dup.force_encoding("UTF-8")
        unless script_source.valid_encoding?
          script_source = script_source.encode("UTF-8", "Shift_JIS", invalid: :replace, undef: :replace, replace: "") rescue script_source.force_encoding("UTF-8")
        end
        script_source = "# encoding: utf-8\n" + script_source
      end
      begin
        eval(script_source, TOPLEVEL_BINDING, "{#{script_id.to_s.rjust(4, '0')}}:#{script_name}")
      rescue => e
        puts "=================================================="
        puts "[FATAL SCRIPT ERROR in #{script_name}]: #{e.class}: #{e.message}"
        puts e.backtrace
        puts "=================================================="
        File.open("Error.log", "wb") { |f| f.write("[FATAL]: #{e.class}: #{e.message}\n#{e.backtrace.join("\n")}") } rescue nil
        raise
      end
      BitmapChangerOptimization.invalidate if defined?(BitmapChangerOptimization) && BitmapChangerOptimization.respond_to?(:invalidate)
      patch_json_decoder if respond_to?(:patch_json_decoder)
      RuntimePackCheatRunner.install_save_and_cache_hooks rescue nil
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
