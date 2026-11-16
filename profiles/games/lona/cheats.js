/**
 * RuntimePack - Clean Category-Based Cheat Profile (Lona RPG)
 * Structured with SVG icons, toggle switches, and no filler descriptions.
 */

window.GAME_CHEATS = {
  title: "Lona RPG Quick Cheats",
  sections: [
    {
      id: "vitals",
      name: "Vitals",
      icon: `<svg viewBox="0 0 24 24"><path d="M22 12h-4l-3 9L9 3l-3 9H2"/></svg>`,
      items: [
        {
          id: "restore_vitals",
          name: "Full Restore (Health / STA / Sat)",
          type: "script",
          script: "a = $game_player.actor; a.health = 200; a.sta = 100; a.sat = 100; a.remove_state('Fatigue'); $game_player.move_normal; 'Vitals Restored'"
        },
        {
          id: "max_morale",
          name: "Max Morale & Mood",
          type: "script",
          script: "$game_player.actor.mood = 100; 'Morale Set to 100'"
        },
        {
          id: "clear_pain_fatigue",
          name: "Clear Pain & Nausea",
          type: "script",
          script: "a = $game_player.actor; a.sta = 100; a.remove_state('Fatigue'); a.pain_value_preg = 0; a.puke_value_normal = 0; a.puke_value_preg = 0; $game_player.move_normal; 'Cleared'"
        },
        {
          id: "empty_waste",
          name: "Empty Bladder & Bowels",
          type: "script",
          script: "a = $game_player.actor; a.urinary_level = 0; a.defecate_level = 0; 'Emptied'"
        }
      ]
    },
    {
      id: "medical",
      name: "Medical",
      icon: `<svg viewBox="0 0 24 24"><path d="M12 2v20M2 12h20"/><rect x="4" y="4" width="16" height="16" rx="2"/></svg>`,
      items: [
        {
          id: "heal_all_wounds",
          name: "Heal All Physical Wounds",
          type: "script",
          script: "a = $game_player.actor; a.wound_state_list.each { |w| a.remove_state(w) }; a.stat['WoundCuff'] = 0; a.stat['WoundCollar'] = 0; 'Healed'"
        },
        {
          id: "heal_organ_damage",
          name: "Heal Internal Organ Trauma",
          type: "script",
          script: "a = $game_player.actor; a.vag_damage = 0; a.urinary_damage = 0; a.anal_damage = 0; 'Restored'"
        },
        {
          id: "clean_body_and_cums",
          name: "Wash Body & Clear Fluids",
          type: "script",
          script: "a = $game_player.actor; a.dirt = 0; a.instance_eval { @cumsMeters.each_key { |k| @cumsMeters[k] = 0 }; @vag_cums.clear if @vag_cums; update_cum_state }; 'Cleaned'"
        },
        {
          id: "reset_pregnancy",
          name: "Terminate / Clear Pregnancy",
          type: "script",
          script: "a = $game_player.actor; a.cleanup_after_birth; 'Cleared'"
        },
        {
          id: "remove_bad_traits",
          name: "Remove Negative Traits",
          type: "script",
          script: "a = $game_player.actor; ['WeakSoul', 'Pessimist'].each { |t| a.remove_state(t); a.stat[t] = 0 }; 'Removed'"
        }
      ]
    },
    {
      id: "combat",
      name: "Combat",
      icon: `<svg viewBox="0 0 24 24"><path d="M14.5 17.5L3 6V3h3l11.5 11.5"/><path d="M13 19l6-6"/><path d="M16 16l4 4"/><path d="M19 21l2-2"/></svg>`,
      items: [
        {
          id: "god_mode_toggle",
          name: "God Mode (Immunity & Full Health)",
          type: "toggle",
          action: "toggle_god_mode",
          key: "god_mode"
        },
        {
          id: "one_hit_kill_toggle",
          name: "1-Hit Kill (Instant Defeat)",
          type: "toggle",
          action: "toggle_one_hit_kill",
          key: "one_hit_kill"
        },
        {
          id: "speed_hack_toggle",
          name: "Speed Hack (Fast Move 6.0)",
          type: "toggle",
          scriptOn: "sp = 6.0; $game_player.move_speed = sp; $game_player.actor.move_speed = sp; 'Speed: 6.0'",
          scriptOff: "sp = 4.3; $game_player.move_speed = sp; $game_player.actor.move_speed = sp; 'Speed: 4.3'"
        }
      ]
    },
    {
      id: "system",
      name: "System & Save",
      icon: `<svg viewBox="0 0 24 24"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/></svg>`,
      items: [
        {
          id: "save_menu_anywhere",
          name: "Open Save Menu Anywhere",
          type: "action",
          action: "save_anywhere"
        },
        {
          id: "load_menu_anywhere",
          name: "Open Load Menu Anywhere",
          type: "action",
          action: "load_anywhere"
        },
        {
          id: "quick_save_btn",
          name: "Quick Save (SavQuick)",
          type: "action",
          action: "quick_save"
        },
        {
          id: "quick_load_btn",
          name: "Quick Load (SavQuick)",
          type: "action",
          action: "quick_load"
        }
      ]
    },
    {
      id: "economy",
      name: "Economy",
      icon: `<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="9"/><path d="M14.8 9A2 2 0 0 0 13 8h-2a2 2 0 0 0 0 4h2a2 2 0 0 1 0 4h-2a2 2 0 0 1-1.8-1"/><path d="M12 6v2m0 8v2"/></svg>`,
      items: [
        {
          id: "add_gold_10000",
          name: "Add 10,000 Gold",
          type: "action",
          action: "gain_gold",
          value: 10000
        },
        {
          id: "add_trait_points",
          name: "Add 10 Trait Points",
          type: "script",
          script: "$game_player.actor.trait_point += 10; \"Trait Points: #{$game_player.actor.trait_point}\""
        },
        {
          id: "reset_difficulty",
          name: "Reset World Difficulty (1.0)",
          type: "script",
          script: "$story_stats['WorldDifficulty'] = 1.0; 'Reset to 1.0'"
        }
      ]
    }
  ]
};
