# Glossary

**Adapter** — engine-specific layer translating a generic RuntimePack service/runtime contract into the behavior expected by an engine.

**Backend** — executable runtime implementation embedded into generated Android application.

**Compatibility layer** — shims/adapters that reproduce a subset of APIs expected by a game.

**Content container** — packed representation of immutable game files, working name `GPACK`.

**Engine detector** — static classifier identifying the game's engine family.

**GameSource** — normalized read-only view of input game files.

**RGSS** — Ruby Game Scripting System used by RPG Maker XP/VX/VX Ace generations.

**Runtime family** — major execution technology: RGSS, web, Ren'Py, Tyrano web adapter.

**Runtime package** — pinned build artifact plus metadata that can be inserted into an Android build.

**Shim** — targeted implementation of a missing external/platform API.

**VFS** — virtual filesystem mediating access to base content, patches and saves.

**Standalone APK** — output Android application dedicated to one game, with no universal game picker.

**Build-time dispatch** — choosing runtime while producing APK, contrasted with JoiPlay's play-time launcher/plugin selection.

**Legal gate** — implementation/distribution is intentionally blocked until redistribution terms are resolved.
