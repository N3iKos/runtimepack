/**
 * RuntimePack - Clean Category-Based Cheat Profile (Fear & Hunger 2: Termina)
 * Structured with SVG icons, action buttons, and clean labels.
 */

window.GAME_CHEATS = {
  title: "Fear & Hunger 2: Termina Cheats",
  sections: [
    {
      id: "system",
      name: "System & Save",
      icon: `<svg viewBox="0 0 24 24"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/></svg>`,
      items: [
        {
          id: "save_anywhere",
          name: "Open Save Menu Anywhere",
          type: "script",
          script: "if (typeof SceneManager !== 'undefined' && typeof Scene_Save !== 'undefined') { SceneManager.push(Scene_Save); } 'Save Menu Opened'"
        },
        {
          id: "coin_flip_always_win",
          name: "Always Win Coin Flip",
          type: "script",
          script: "if (typeof $gameVariables !== 'undefined' && typeof $gameVariables.setValue === 'function') { $gameVariables.setValue(1, 1); $gameVariables.setValue(14, 1); } 'Coin Flip: Win'"
        },
        {
          id: "quick_save",
          name: "Quick Save",
          type: "script",
          script: "if (typeof $gameSystem !== 'undefined' && typeof DataManager !== 'undefined') { $gameSystem.onBeforeSave(); DataManager.saveGame(1); } 'Game Saved to Slot 1'"
        }
      ]
    },
    {
      id: "vitals",
      name: "Vitals & Body",
      icon: `<svg viewBox="0 0 24 24"><path d="M22 12h-4l-3 9L9 3l-3 9H2"/></svg>`,
      items: [
        {
          id: "full_restore_body_mind",
          name: "Full Restore Body & Mind",
          type: "script",
          script: "if (typeof $gameParty !== 'undefined') { $gameParty.members().forEach(function(m) { if (m) { m.setHp(m.mhp); m.setMp(m.mmp); } }); } 'Body & Mind Restored'"
        },
        {
          id: "clear_hunger",
          name: "Clear Hunger",
          type: "script",
          script: "if (typeof $gameVariables !== 'undefined' && typeof $gameVariables.setValue === 'function') { $gameVariables.setValue(10, 0); $gameVariables.setValue(27, 0); } 'Hunger Cleared'"
        },
        {
          id: "restore_limbs_statuses",
          name: "Restore Limbs & Statuses",
          type: "script",
          script: "if (typeof $gameParty !== 'undefined') { $gameParty.members().forEach(function(m) { if (m) { if (typeof m.recoverAll === 'function') { m.recoverAll(); } else { m.clearStates(); m.setHp(m.mhp); m.setMp(m.mmp); } [3, 14, 55, 79].forEach(function(s) { m.removeState(s); }); } }); } if (typeof $gamePlayer !== 'undefined' && typeof $gamePlayer.refresh === 'function') { $gamePlayer.refresh(); } 'Limbs & Statuses Restored'"
        }
      ]
    },
    {
      id: "economy",
      name: "Economy",
      icon: `<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="9"/><path d="M14.8 9A2 2 0 0 0 13 8h-2a2 2 0 0 0 0 4h2a2 2 0 0 1 0 4h-2a2 2 0 0 1-1.8-1"/><path d="M12 6v2m0 8v2"/></svg>`,
      items: [
        {
          id: "add_shillings",
          name: "Add 100 Shillings",
          type: "action",
          action: "gain_gold",
          value: 100
        },
        {
          id: "add_soul_stone",
          name: "Add Soul Stone",
          type: "script",
          script: "if (typeof $gameParty !== 'undefined' && typeof $dataItems !== 'undefined') { var item = $dataItems.find(function(i) { return i && (/soul stone|batu jiwa/i.test(i.name) || i.id === 116 || i.id === 98); }) || $dataItems[116] || $dataItems[98]; if (item) { $gameParty.gainItem(item, 1); 'Added: ' + item.name; } else { 'Soul Stone not found'; } }"
        }
      ]
    }
  ]
};
