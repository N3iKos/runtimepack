# frozen_string_literal: true
# RuntimePack Asset Audit Module for RGSS (XP, VX, VX Ace)
# Captures asset load failures and fallback substitutions at runtime.

require 'json'
require 'fileutils'

module RuntimePackAssetAudit
  EVENTS_FILE = "rp_audit_events.json"
  MAX_EVENTS = 1000

  @events = []
  @seq_id = 0
  @mutex = Mutex.new rescue nil

  class << self
    attr_reader :events

    def reset!
      synchronize do
        @events = []
        @seq_id = 0
      end
    end

    def synchronize(&block)
      if @mutex
        @mutex.synchronize(&block)
      else
        yield
      end
    end

    def record_event(stage, asset_path, outcome, fallback_action: nil, error_message: nil, source_location: nil)
      event = nil
      synchronize do
        @seq_id = (@seq_id || 0) + 1
        timestamp_ns = if defined?(Process::CLOCK_MONOTONIC)
          Process.clock_gettime(Process::CLOCK_MONOTONIC, :nanosecond) rescue (Time.now.to_f * 1_000_000_000).to_i
        else
          (Time.now.to_f * 1_000_000_000).to_i
        end

        event = {
          "sequence_id" => @seq_id,
          "timestamp_ns" => timestamp_ns,
          "stage" => stage.to_s,
          "asset_path" => asset_path.to_s,
          "outcome" => outcome.to_s,
          "fallback_action" => fallback_action ? fallback_action.to_s : nil,
          "error_message" => error_message ? error_message.to_s : nil,
          "source_location" => source_location ? source_location.to_s : nil
        }
        @events ||= []
        @events << event
        if @events.size > MAX_EVENTS
          @events.shift(@events.size - MAX_EVENTS)
        end
        persist_events_atomic
      end
      event
    end

    def persist_events_atomic
      targets = [EVENTS_FILE]
      targets << File.join("game", EVENTS_FILE) if Dir.exist?("game")

      json_str = JSON.generate(@events)
      targets.each do |target|
        tmp = "#{target}.tmp"
        begin
          File.open(tmp, "wb") { |f| f.write(json_str) }
          File.rename(tmp, target)
        rescue => e
          # Fallback if atomic rename fails on some Android filesystem mounts
          begin
            File.open(target, "wb") { |f| f.write(json_str) }
            File.delete(tmp) if File.exist?(tmp)
          rescue
          end
        end
      end
    end

    def install_hooks!
      install_bitmap_hook!
      install_cache_hooks!
      install_audio_hooks!
    end

    def install_bitmap_hook!
      return unless defined?(Bitmap)
      target = (class << Bitmap; self; end)
      return if target.method_defined?(:rp_audit_orig_new)

      target.class_eval do
        alias_method :rp_audit_orig_new, :new rescue nil
        def new(*args, &block)
          begin
            rp_audit_orig_new(*args, &block)
          rescue Errno::ENOENT, (defined?(RGSSError) ? RGSSError : StandardError), StandardError => e
            if args.size == 1 && args[0].is_a?(String)
              path = args[0]
              RuntimePackAssetAudit.record_event(
                "FILE_EXISTS",
                path,
                "FAIL",
                fallback_action: "PLACEHOLDER_SUBSTITUTED",
                error_message: "#{e.class}: #{e.message}",
                source_location: "Bitmap.new"
              )
              dummy = (rp_audit_orig_new(32, 32) rescue nil)
              dummy
            else
              raise e
            end
          end
        end
      end
    end

    def install_cache_hooks!
      [defined?(Cache) ? Cache : nil, defined?(RPG::Cache) ? RPG::Cache : nil].compact.each do |cache_mod|
        cache_target = (class << cache_mod; self; end)
        cache_methods = [
          :animation, :autotile, :battleback, :battleback1, :battleback2, :battler,
          :character, :face, :fog, :gameover, :icon, :picture, :system, :tile,
          :tileset, :title, :title1, :title2, :windowskin
        ]
        cache_methods.each do |m|
          if cache_mod.respond_to?(m)
            orig_m = :"rp_audit_orig_cache_#{m}"
            unless cache_mod.respond_to?(orig_m)
              cache_target.class_eval do
                alias_method orig_m, m rescue nil
                define_method(m) do |*args|
                  begin
                    send(orig_m, *args)
                  rescue Errno::ENOENT, (defined?(RGSSError) ? RGSSError : StandardError), StandardError => e
                    path = args.first.to_s
                    RuntimePackAssetAudit.record_event(
                      "FILE_EXISTS",
                      path,
                      "FAIL",
                      fallback_action: "PLACEHOLDER_SUBSTITUTED",
                      error_message: "#{e.class}: #{e.message}",
                      source_location: "#{cache_mod}.#{m}"
                    )
                    (Bitmap.new(32, 32) rescue nil)
                  end
                end
              end
            end
          end
        end

        if cache_mod.respond_to?(:normal_bitmap) && !cache_mod.respond_to?(:rp_audit_orig_normal_bitmap)
          cache_target.class_eval do
            alias_method :rp_audit_orig_normal_bitmap, :normal_bitmap rescue nil
            def normal_bitmap(path)
              begin
                rp_audit_orig_normal_bitmap(path)
              rescue Errno::ENOENT, (defined?(RGSSError) ? RGSSError : StandardError), StandardError => e
                RuntimePackAssetAudit.record_event(
                  "FILE_EXISTS",
                  path.to_s,
                  "FAIL",
                  fallback_action: "PLACEHOLDER_SUBSTITUTED",
                  error_message: "#{e.class}: #{e.message}",
                  source_location: "#{cache_mod}.normal_bitmap"
                )
                @cache ||= {}
                @cache[path] = (Bitmap.new(32, 32) rescue nil)
              end
            end
          end
        end
      end
    end

    def install_audio_hooks!
      return unless defined?(Audio)
      target = (class << Audio; self; end)
      [:se_play, :bgm_play, :bgs_play, :me_play].each do |m|
        if Audio.respond_to?(m)
          orig_m = :"rp_audit_orig_audio_#{m}"
          unless Audio.respond_to?(orig_m)
            target.class_eval do
              alias_method orig_m, m rescue nil
              define_method(m) do |*args|
                begin
                  send(orig_m, *args)
                rescue Errno::ENOENT, (defined?(RGSSError) ? RGSSError : StandardError), StandardError => e
                  audio_path = args.first.respond_to?(:name) ? args.first.name : args.first.to_s
                  RuntimePackAssetAudit.record_event(
                    "FILE_EXISTS",
                    audio_path,
                    "FAIL",
                    fallback_action: "AUDIO_LOAD_SUPPRESSED",
                    error_message: "#{e.class}: #{e.message}",
                    source_location: "Audio.#{m}"
                  )
                  nil
                end
              end
            end
          end
        end
      end
    end
  end
end

# Auto-install hooks when loaded into RGSS runtime
RuntimePackAssetAudit.install_hooks!
