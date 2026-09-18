# Task 4 Brief: Engine-Agnostic Core Cheat Runner in `runtimepack_bootstrap.rb` and `cheat_engine.js`

## Context & Principles
The cheat system must adhere strictly to the engine-agnostic core architecture:
- Core runner only provides official engine execution primitives (`$game_variables`, `$game_switches`, `$game_party`, eval) without ANY hardcoded game-specific stats or formulas.
- Communication between WebView overlay and RGSS mkxp-z thread uses the atomic IPC queue in the app's internal working directory.

## Requirements & Scope
- Files to modify:
  - `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
  - `android/shell/app/src/main/java/io/runtimepack/host/RgssActivity.kt`
  - `android/shell/app/src/main/assets/overlay/cheat_engine.js`
- Files to create:
  - `android/shell/app/src/test/ruby/cheat_runner_test.rb`
  - `android/shell/app/src/test/js/cheat_primitives.test.js`

### Detailed Changes:

1. In `runtimepack_bootstrap.rb`:
   - Hook `Graphics.update`:
     Every frame, call `RuntimePackCheatRunner.update`:
     ```ruby
     module RuntimePackCheatRunner
       CMD_FILE = "rp_cheat_cmd.json"
       STATE_FILE = "rp_cheat_state.json"
       EVAL_FILE = "rp_cheat_eval_res.txt"
       ERROR_FILE = "rp_cheat_error.txt"

       def self.update
         return unless File.exist?(CMD_FILE)
         begin
           content = File.read(CMD_FILE)
           File.delete(CMD_FILE) rescue nil
           return if content.nil? || content.strip.empty?
           
           # Parse command (handle single hash or array of hashes)
           cmds = JSON.decode(content) rescue nil
           cmds = [cmds] if cmds.is_a?(Hash)
           return unless cmds.is_a?(Array)

           cmds.each { |cmd| process_cmd(cmd) }
         rescue => e
           File.open(ERROR_FILE, "wb") { |f| f.write("[CHT-001] #{e.class}: #{e.message}\n#{e.backtrace.join("\n") rescue ''}") }
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
           $game_party.members.each(&:recover_all) if defined?($game_party) && $game_party
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
         when "dump_state"
           dump_game_state
         when "eval"
           code = cmd["code"].to_s
           begin
             res = eval(code, TOPLEVEL_BINDING)
             File.open(EVAL_FILE, "wb") { |f| f.write(res.inspect) }
           rescue => err
             File.open(EVAL_FILE, "wb") { |f| f.write("[CHT-002] #{err.class}: #{err.message}") }
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
         payload = { "gold" => gold, "variables" => vars, "switches" => switches }
         # Write JSON
         File.open(STATE_FILE, "wb") { |f| f.write(JSON.encode(payload)) } rescue nil
       end
     end

     if defined?(Graphics)
       class << Graphics
         alias_method :rp_orig_cheat_update, :update rescue nil
         def update
           RuntimePackCheatRunner.update
           rp_orig_cheat_update
         end
       end
     end
     ```

2. In `RgssActivity.kt`:
   In `RuntimePackVPad` JavascriptInterface:
   - `@JavascriptInterface fun postCheatCmd(jsonCmd: String): Boolean`:
     Write `jsonCmd` to `File(filesDir, "rp_cheat_cmd.json")` safely via atomic temp file write.
   - `@JavascriptInterface fun getCheatState(): String`:
     Read `File(filesDir, "rp_cheat_state.json")` if it exists and return content, else `"{}"`.
   - `@JavascriptInterface fun getEvalResult(): String`:
     Read `File(filesDir, "rp_cheat_eval_res.txt")` if it exists, delete it, and return text.

3. In `cheat_engine.js`:
   Expose unified `window.CheatEngine`:
   - Abstract primitives:
     - `setVariable(id, value)`
     - `setSwitch(id, value)`
     - `gainGold(amount)`
     - `recoverAll()`
     - `gainItem(type, id, amount)`
     - `evalScript(code, callback)`
     - `requestStateDump()`
     - `getLoadedState()`
   - Support both Native RGSS (via `window.RuntimePackVPad.postCheatCmd`) and Web MV/MZ (via direct `$gameVariables`, `$gameSwitches`, `$gameParty`).
   - Module export for Node.js testing.

4. Tests:
   - `android/shell/app/src/test/ruby/cheat_runner_test.rb`:
     Mock `$game_variables`, `$game_switches`, `$game_party`, verify command queue execution.
     Run: `ruby android/shell/app/src/test/ruby/cheat_runner_test.rb`
   - `android/shell/app/src/test/js/cheat_primitives.test.js`:
     Verify `CheatEngine` command formulation and execution in both web and native modes.
     Run: `node android/shell/app/src/test/js/cheat_primitives.test.js`

5. Report Contract:
   - Full report in `.superpowers/sdd/2026-09-18-input-sync-and-agnostic-cheat-engine/task-4-report.md`.
   - Return status `DONE`, commits, test output summary, concerns.
