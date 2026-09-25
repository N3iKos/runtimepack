import os
import sys

def write_long(val: int) -> bytes:
    if val == 0:
        return b'\x00'
    elif 0 < val <= 122:
        return bytes([val + 5])
    elif -122 <= val < 0:
        return bytes([(val - 5) & 0xff])
    elif val > 0:
        bs = val.to_bytes(4, 'little')
        if val <= 0xff:
            return b'\x01' + bs[:1]
        elif val <= 0xffff:
            return b'\x02' + bs[:2]
        elif val <= 0xffffff:
            return b'\x03' + bs[:3]
        else:
            return b'\x04' + bs[:4]
    else:
        bs = val.to_bytes(4, 'little', signed=True)
        return b'\xfc' + bs

class RubyObj:
    def __init__(self, class_name: str, ivars: dict):
        self.class_name = class_name
        self.ivars = ivars

class RubyUserDefined:
    def __init__(self, class_name: str, data: bytes):
        self.class_name = class_name
        self.data = data

def encode_value(v) -> bytes:
    if v is None:
        return b'0'
    elif isinstance(v, bool):
        return b'T' if v else b'F'
    elif isinstance(v, int):
        return b'i' + write_long(v)
    elif isinstance(v, str):
        # Ruby string with UTF-8 ivar wrapper: I"...\x06:ET
        s_bytes = v.encode('utf-8')
        inner = b'"' + write_long(len(s_bytes)) + s_bytes
        # return I wrapper
        ivar_e = b':' + write_long(1) + b'E'
        return b'I' + inner + write_long(1) + ivar_e + b'T'
    elif isinstance(v, list):
        res = bytearray(b'[' + write_long(len(v)))
        for item in v:
            res.extend(encode_value(item))
        return bytes(res)
    elif isinstance(v, dict):
        res = bytearray(b'{' + write_long(len(v)))
        for k, val in v.items():
            res.extend(encode_value(k))
            res.extend(encode_value(val))
        return bytes(res)
    elif isinstance(v, RubyUserDefined):
        c_bytes = v.class_name.encode('utf-8')
        return b'u:' + write_long(len(c_bytes)) + c_bytes + write_long(len(v.data)) + v.data
    elif isinstance(v, RubyObj):
        c_bytes = v.class_name.encode('utf-8')
        res = bytearray(b'o:' + write_long(len(c_bytes)) + c_bytes + write_long(len(v.ivars)))
        for k, val in v.ivars.items():
            k_name = k if k.startswith('@') else f"@{k}"
            k_bytes = k_name.encode('utf-8')
            res.extend(b':' + write_long(len(k_bytes)) + k_bytes)
            res.extend(encode_value(val))
        return bytes(res)
    else:
        raise ValueError(f"Unsupported value: {v}")

def dump_marshal(v) -> bytes:
    return b'\x04\x08' + encode_value(v)

def audio_file(name: str, volume: int = 100, pitch: int = 100, class_name: str = "RPG::AudioFile") -> RubyObj:
    return RubyObj(class_name, {
        "@name": name,
        "@volume": volume,
        "@pitch": pitch,
    })

def event_cmd(code: int, indent: int, params: list) -> RubyObj:
    return RubyObj("RPG::EventCommand", {
        "@code": code,
        "@indent": indent,
        "@parameters": params,
    })

def move_cmd(code: int, params: list) -> RubyObj:
    return RubyObj("RPG::MoveCommand", {
        "@code": code,
        "@parameters": params,
    })

def make_file(path: str, data: bytes):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(data)

def make_dummy(path: str):
    make_file(path, b"DUMMY_ASSET_CONTENT")

def create_xp_valid(root: str):
    # Game.ini
    make_file(os.path.join(root, "Game.ini"), b"[Game]\nLibrary=RGSS104E.dll\nScripts=Data\\Scripts.rxdata\nTitle=XP Valid\nRTP1=Standard\n")
    # System.rxdata
    sys_obj = RubyObj("RPG::System", {
        "@title_name": "TitleXP",
        "@windowskin_name": "WindowskinXP",
        "@gameover_name": "GameOverXP",
        "@transition_name": "TransitionXP",
        "@battleback_name": "BattlebackXP",
        "@battler_name": "BattlerXP",
        "@title_bgm": audio_file("TitleBgm"),
        "@battle_bgm": audio_file("BattleBgm"),
        "@battle_end_me": audio_file("VictoryMe"),
        "@gameover_me": audio_file("GameOverMe"),
        "@cursor_se": audio_file("CursorSe"),
    })
    make_file(os.path.join(root, "Data", "System.rxdata"), dump_marshal(sys_obj))

    # Actors.rxdata
    actor = RubyObj("RPG::Actor", {
        "@id": 1,
        "@character_name": "HeroXP",
        "@battler_name": "HeroBattlerXP",
    })
    make_file(os.path.join(root, "Data", "Actors.rxdata"), dump_marshal([None, actor]))

    # Enemies.rxdata
    enemy = RubyObj("RPG::Enemy", {
        "@id": 1,
        "@battler_name": "SlimeXP",
    })
    make_file(os.path.join(root, "Data", "Enemies.rxdata"), dump_marshal([None, enemy]))

    # Tilesets.rxdata
    tileset = RubyObj("RPG::Tileset", {
        "@id": 1,
        "@tileset_name": "TilesetXP",
        "@autotile_names": ["Autotile1", "", ""],
        "@panorama_name": "PanoramaXP",
        "@fog_name": "FogXP",
        "@battleback_name": "BattlebackTilesetXP",
    })
    make_file(os.path.join(root, "Data", "Tilesets.rxdata"), dump_marshal([None, tileset]))

    # Animations.rxdata
    timing = RubyObj("RPG::Animation::Timing", {
        "@se": audio_file("AnimSeXP"),
    })
    anim = RubyObj("RPG::Animation", {
        "@id": 1,
        "@animation_name": "AnimationXP",
        "@timings": [timing],
    })
    make_file(os.path.join(root, "Data", "Animations.rxdata"), dump_marshal([None, anim]))

    # CommonEvents.rxdata
    ce = RubyObj("RPG::CommonEvent", {
        "@id": 1,
        "@list": [
            event_cmd(241, 0, [audio_file("CommonBgm")]),
            event_cmd(231, 0, [1, "CommonPic", 0, 0, 0, 0, 100, 100, 255, 0]),
            event_cmd(0, 0, []),
        ]
    })
    make_file(os.path.join(root, "Data", "CommonEvents.rxdata"), dump_marshal([None, ce]))

    # Classes, Skills, Items, Weapons, Armors, States, MapInfos
    make_file(os.path.join(root, "Data", "Classes.rxdata"), dump_marshal([None, RubyObj("RPG::Class", {"@id": 1, "@name": "Warrior"})]))
    make_file(os.path.join(root, "Data", "Skills.rxdata"), dump_marshal([None, RubyObj("RPG::Skill", {"@id": 1, "@icon_name": "SkillIconXP", "@sound_effect": audio_file("SkillSeXP")})]))
    make_file(os.path.join(root, "Data", "Items.rxdata"), dump_marshal([None, RubyObj("RPG::Item", {"@id": 1, "@icon_name": "ItemIconXP", "@menu_se_name": "ItemSeXP"})]))
    make_file(os.path.join(root, "Data", "Weapons.rxdata"), dump_marshal([None, RubyObj("RPG::Weapon", {"@id": 1, "@icon_name": "WeaponIconXP"})]))
    make_file(os.path.join(root, "Data", "Armors.rxdata"), dump_marshal([None, RubyObj("RPG::Armor", {"@id": 1, "@icon_name": "ArmorIconXP"})]))
    make_file(os.path.join(root, "Data", "States.rxdata"), dump_marshal([None, RubyObj("RPG::State", {"@id": 1, "@name": "Normal"})]))
    make_file(os.path.join(root, "Data", "MapInfos.rxdata"), dump_marshal({ 1: RubyObj("RPG::MapInfo", {"@name": "Map 1", "@parent_id": 0, "@order": 1}) }))

    # Troops.rxdata
    troop_page = RubyObj("RPG::Troop::Page", {
        "@list": [
            event_cmd(250, 0, [audio_file("BattleSe")]),
            event_cmd(0, 0, []),
        ]
    })
    troop = RubyObj("RPG::Troop", {
        "@id": 1,
        "@pages": [troop_page],
    })
    make_file(os.path.join(root, "Data", "Troops.rxdata"), dump_marshal([None, troop]))

    # Map001.rxdata
    graphic = RubyObj("RPG::Event::Page::Graphic", {
        "@character_name": "NpcXP",
    })
    move_route = RubyObj("RPG::MoveRoute", {
        "@list": [
            move_cmd(41, ["NpcMoveXP"]),
            move_cmd(44, [audio_file("StepSe")]),
        ]
    })
    event_page = RubyObj("RPG::Event::Page", {
        "@graphic": graphic,
        "@move_route": move_route,
        "@list": [
            event_cmd(201, 0, [0, 1, 0, 0, 0]), # direct transfer to Map 1
            event_cmd(231, 0, [1, "MapPicXP", 0, 0, 0, 0, 100, 100, 255, 0]),
            event_cmd(245, 0, [audio_file("MapEventBgs")]),
            event_cmd(249, 0, [audio_file("MapEventMe")]),
            event_cmd(322, 0, [1, "HeroNewXP", 0, "HeroBattlerNewXP", 0]),
            event_cmd(0, 0, []),
        ]
    })
    event = RubyObj("RPG::Event", {
        "@id": 1,
        "@pages": [event_page],
    })
    map_obj = RubyObj("RPG::Map", {
        "@bgm": audio_file("MapBgm"),
        "@bgs": audio_file("MapBgs"),
        "@panorama_name": "MapPanorama",
        "@fog_name": "MapFog",
        "@battleback_name": "MapBattleback",
        "@tileset_id": 1,
        "@events": { 1: event },
    })
    make_file(os.path.join(root, "Data", "Map001.rxdata"), dump_marshal(map_obj))

    # Assets
    for g in [
        "Graphics/Titles/TitleXP.png",
        "Graphics/Windowskins/WindowskinXP.png",
        "Graphics/Gameovers/GameOverXP.png",
        "Graphics/Transitions/TransitionXP.png",
        "Graphics/Battlebacks/BattlebackXP.png",
        "Graphics/Battlebacks/BattlebackTilesetXP.png",
        "Graphics/Battlebacks/MapBattleback.png",
        "Graphics/Battlers/BattlerXP.png",
        "Graphics/Battlers/HeroBattlerXP.png",
        "Graphics/Battlers/HeroBattlerNewXP.png",
        "Graphics/Battlers/SlimeXP.png",
        "Graphics/Characters/HeroXP.png",
        "Graphics/Characters/HeroNewXP.png",
        "Graphics/Characters/NpcXP.png",
        "Graphics/Characters/NpcMoveXP.png",
        "Graphics/Tilesets/TilesetXP.png",
        "Graphics/Autotiles/Autotile1.png",
        "Graphics/Panoramas/PanoramaXP.png",
        "Graphics/Panoramas/MapPanorama.png",
        "Graphics/Fogs/FogXP.png",
        "Graphics/Fogs/MapFog.png",
        "Graphics/Animations/AnimationXP.png",
        "Graphics/Pictures/CommonPic.png",
        "Graphics/Pictures/MapPicXP.png",
        "Graphics/Icons/SkillIconXP.png",
        "Graphics/Icons/ItemIconXP.png",
        "Graphics/Icons/WeaponIconXP.png",
        "Graphics/Icons/ArmorIconXP.png",
    ]:
        make_dummy(os.path.join(root, g))

    for a in [
        "Audio/BGM/TitleBgm.ogg",
        "Audio/BGM/BattleBgm.ogg",
        "Audio/BGM/CommonBgm.ogg",
        "Audio/BGM/MapBgm.ogg",
        "Audio/BGS/MapBgs.ogg",
        "Audio/BGS/MapEventBgs.ogg",
        "Audio/ME/VictoryMe.ogg",
        "Audio/ME/GameOverMe.ogg",
        "Audio/ME/MapEventMe.ogg",
        "Audio/SE/CursorSe.ogg",
        "Audio/SE/AnimSeXP.ogg",
        "Audio/SE/BattleSe.ogg",
        "Audio/SE/StepSe.ogg",
        "Audio/SE/SkillSeXP.ogg",
        "Audio/SE/ItemSeXP.ogg",
    ]:
        make_dummy(os.path.join(root, a))


def create_vx_valid(root: str):
    make_file(os.path.join(root, "Game.ini"), b"[Game]\nLibrary=RGSS202E.dll\nScripts=Data\\Scripts.rvdata\nTitle=VX Valid\nRTP=RPGVX\n")
    sys_obj = RubyObj("RPG::System", {
        "@gameover_name": "GameOverVX",
        "@title_bgm": audio_file("TitleBgmVX"),
        "@battle_bgm": audio_file("BattleBgmVX"),
        "@battle_end_me": audio_file("VictoryMeVX"),
        "@gameover_me": audio_file("GameOverMeVX"),
        "@sounds": [audio_file("CursorSeVX")],
        "@boat": RubyObj("RPG::System::Vehicle", {
            "@character_name": "BoatVX",
            "@bgm": audio_file("BoatBgmVX"),
        }),
        "@ship": RubyObj("RPG::System::Vehicle", {
            "@character_name": "ShipVX",
            "@bgm": audio_file("ShipBgmVX"),
        }),
        "@airship": RubyObj("RPG::System::Vehicle", {
            "@character_name": "AirshipVX",
            "@bgm": audio_file("AirshipBgmVX"),
        }),
    })
    make_file(os.path.join(root, "Data", "System.rvdata"), dump_marshal(sys_obj))

    actor = RubyObj("RPG::Actor", {
        "@id": 1,
        "@character_name": "HeroVX",
        "@face_name": "HeroFaceVX",
    })
    make_file(os.path.join(root, "Data", "Actors.rvdata"), dump_marshal([None, actor]))

    enemy = RubyObj("RPG::Enemy", {
        "@id": 1,
        "@battler_name": "SlimeVX",
    })
    make_file(os.path.join(root, "Data", "Enemies.rvdata"), dump_marshal([None, enemy]))

    anim = RubyObj("RPG::Animation", {
        "@id": 1,
        "@animation1_name": "Anim1VX",
        "@animation2_name": "Anim2VX",
        "@timings": [RubyObj("RPG::Animation::Timing", { "@se": audio_file("AnimSeVX") })],
    })
    make_file(os.path.join(root, "Data", "Animations.rvdata"), dump_marshal([None, anim]))

    make_file(os.path.join(root, "Data", "Classes.rvdata"), dump_marshal([None, RubyObj("RPG::Class", {"@id": 1})]))
    make_file(os.path.join(root, "Data", "Skills.rvdata"), dump_marshal([None, RubyObj("RPG::Skill", {"@id": 1, "@icon_index": 1})]))
    make_file(os.path.join(root, "Data", "Items.rvdata"), dump_marshal([None, RubyObj("RPG::Item", {"@id": 1, "@icon_index": 2})]))
    make_file(os.path.join(root, "Data", "Weapons.rvdata"), dump_marshal([None, RubyObj("RPG::Weapon", {"@id": 1, "@icon_index": 3})]))
    make_file(os.path.join(root, "Data", "Armors.rvdata"), dump_marshal([None, RubyObj("RPG::Armor", {"@id": 1, "@icon_index": 4})]))
    make_file(os.path.join(root, "Data", "States.rvdata"), dump_marshal([None, RubyObj("RPG::State", {"@id": 1, "@icon_index": 5})]))
    make_file(os.path.join(root, "Data", "CommonEvents.rvdata"), dump_marshal([None, RubyObj("RPG::CommonEvent", {"@id": 1, "@list": [event_cmd(0, 0, [])]})]))
    make_file(os.path.join(root, "Data", "Troops.rvdata"), dump_marshal([None, RubyObj("RPG::Troop", {"@id": 1, "@pages": [RubyObj("RPG::Troop::Page", {"@list": [event_cmd(0, 0, [])]})]})]))
    make_file(os.path.join(root, "Data", "MapInfos.rvdata"), dump_marshal({ 1: RubyObj("RPG::MapInfo", {"@name": "Map 1"}) }))

    event_page = RubyObj("RPG::Event::Page", {
        "@graphic": RubyObj("RPG::Event::Page::Graphic", { "@character_name": "NpcVX" }),
        "@move_route": RubyObj("RPG::MoveRoute", { "@list": [] }),
        "@list": [
            event_cmd(201, 0, [0, 1, 0, 0, 0]),
            event_cmd(231, 0, [1, "MapPicVX", 0, 0, 0, 0, 100, 100, 255, 0]),
            event_cmd(322, 0, [1, "HeroNewVX", 0, "HeroFaceNewVX", 0]),
            event_cmd(0, 0, []),
        ]
    })
    map_obj = RubyObj("RPG::Map", {
        "@parallax_name": "ParallaxVX",
        "@events": { 1: RubyObj("RPG::Event", { "@id": 1, "@pages": [event_page] }) },
    })
    make_file(os.path.join(root, "Data", "Map001.rvdata"), dump_marshal(map_obj))

    for g in [
        "Graphics/System/GameOverVX.png",
        "Graphics/Characters/BoatVX.png",
        "Graphics/Characters/ShipVX.png",
        "Graphics/Characters/AirshipVX.png",
        "Graphics/Characters/HeroVX.png",
        "Graphics/Characters/HeroNewVX.png",
        "Graphics/Characters/NpcVX.png",
        "Graphics/Faces/HeroFaceVX.png",
        "Graphics/Faces/HeroFaceNewVX.png",
        "Graphics/Battlers/SlimeVX.png",
        "Graphics/Animations/Anim1VX.png",
        "Graphics/Animations/Anim2VX.png",
        "Graphics/Parallaxes/ParallaxVX.png",
        "Graphics/Pictures/MapPicVX.png",
        "Graphics/System/IconSet.png",
    ]:
        make_dummy(os.path.join(root, g))

    for a in [
        "Audio/BGM/TitleBgmVX.ogg",
        "Audio/BGM/BattleBgmVX.ogg",
        "Audio/BGM/BoatBgmVX.ogg",
        "Audio/BGM/ShipBgmVX.ogg",
        "Audio/BGM/AirshipBgmVX.ogg",
        "Audio/ME/VictoryMeVX.ogg",
        "Audio/ME/GameOverMeVX.ogg",
        "Audio/SE/CursorSeVX.ogg",
        "Audio/SE/AnimSeVX.ogg",
    ]:
        make_dummy(os.path.join(root, a))


def create_vxace_valid(root: str):
    make_file(os.path.join(root, "Game.ini"), b"[Game]\nLibrary=System\\RGSS301.dll\nScripts=Data\\Scripts.rvdata2\nTitle=Ace Valid\nRTP=RPGVXAce\n")
    sys_obj = RubyObj("RPG::System", {
        "@title1_name": "Title1Ace",
        "@title2_name": "Title2Ace",
        "@gameover_name": "GameOverAce",
        "@windowskin_name": "Window",
        "@title_bgm": audio_file("TitleBgmAce", class_name="RPG::BGM"),
        "@battle_bgm": audio_file("BattleBgmAce", class_name="RPG::BGM"),
        "@battle_end_me": audio_file("VictoryMeAce", class_name="RPG::ME"),
        "@gameover_me": audio_file("GameOverMeAce", class_name="RPG::ME"),
        "@sounds": [audio_file("CursorSeAce", class_name="RPG::SE")],
        "@boat": RubyObj("RPG::System::Vehicle", {
            "@character_name": "BoatAce",
            "@bgm": audio_file("BoatBgmAce", class_name="RPG::BGM"),
        }),
        "@ship": RubyObj("RPG::System::Vehicle", {
            "@character_name": "ShipAce",
            "@bgm": audio_file("ShipBgmAce", class_name="RPG::BGM"),
        }),
        "@airship": RubyObj("RPG::System::Vehicle", {
            "@character_name": "AirshipAce",
            "@bgm": audio_file("AirshipBgmAce", class_name="RPG::BGM"),
        }),
    })
    make_file(os.path.join(root, "Data", "System.rvdata2"), dump_marshal(sys_obj))

    actor = RubyObj("RPG::Actor", {
        "@id": 1,
        "@character_name": "Actor1",
        "@face_name": "ActorFace1",
    })
    make_file(os.path.join(root, "Data", "Actors.rvdata2"), dump_marshal([None, actor]))

    enemy = RubyObj("RPG::Enemy", {
        "@id": 1,
        "@battler_name": "SlimeAce",
    })
    make_file(os.path.join(root, "Data", "Enemies.rvdata2"), dump_marshal([None, enemy]))

    tileset = RubyObj("RPG::Tileset", {
        "@id": 1,
        "@tileset_names": ["TilesetAce1", ""],
    })
    make_file(os.path.join(root, "Data", "Tilesets.rvdata2"), dump_marshal([None, tileset]))

    make_file(os.path.join(root, "Data", "Classes.rvdata2"), dump_marshal([None, RubyObj("RPG::Class", {"@id": 1})]))
    make_file(os.path.join(root, "Data", "Skills.rvdata2"), dump_marshal([None, RubyObj("RPG::Skill", {"@id": 1, "@icon_index": 1})]))
    make_file(os.path.join(root, "Data", "Items.rvdata2"), dump_marshal([None, RubyObj("RPG::Item", {"@id": 1, "@icon_index": 2})]))
    make_file(os.path.join(root, "Data", "Weapons.rvdata2"), dump_marshal([None, RubyObj("RPG::Weapon", {"@id": 1, "@icon_index": 3})]))
    make_file(os.path.join(root, "Data", "Armors.rvdata2"), dump_marshal([None, RubyObj("RPG::Armor", {"@id": 1, "@icon_index": 4})]))
    make_file(os.path.join(root, "Data", "States.rvdata2"), dump_marshal([None, RubyObj("RPG::State", {"@id": 1, "@icon_index": 5})]))
    make_file(os.path.join(root, "Data", "CommonEvents.rvdata2"), dump_marshal([None, RubyObj("RPG::CommonEvent", {"@id": 1, "@list": [event_cmd(0, 0, [])]})]))
    make_file(os.path.join(root, "Data", "Troops.rvdata2"), dump_marshal([None, RubyObj("RPG::Troop", {"@id": 1, "@pages": [RubyObj("RPG::Troop::Page", {"@list": [event_cmd(0, 0, [])]})]})]))
    make_file(os.path.join(root, "Data", "MapInfos.rvdata2"), dump_marshal({ 1: RubyObj("RPG::MapInfo", {"@name": "Map 1"}) }))

    anim = RubyObj("RPG::Animation", {
        "@id": 1,
        "@animation1_name": "Anim1Ace",
        "@animation2_name": "Anim2Ace",
        "@timings": [RubyObj("RPG::Animation::Timing", { "@se": audio_file("AnimSeAce", class_name="RPG::SE") })],
    })
    make_file(os.path.join(root, "Data", "Animations.rvdata2"), dump_marshal([None, anim]))

    event_page = RubyObj("RPG::Event::Page", {
        "@graphic": RubyObj("RPG::Event::Page::Graphic", { "@character_name": "NpcAce" }),
        "@move_route": RubyObj("RPG::MoveRoute", { "@list": [] }),
        "@list": [
            event_cmd(201, 0, [0, 1, 0, 0, 0, 0]), # direct transfer to Map 1
            event_cmd(231, 0, [1, "PictureAce", 0, 0, 0, 0, 100, 100, 255, 0]),
            event_cmd(250, 0, [audio_file("SeAce", class_name="RPG::SE")]),
            event_cmd(322, 0, [1, "HeroNewAce", 0, "FaceNewAce", 0]),
            event_cmd(0, 0, []),
        ]
    })
    map_obj = RubyObj("RPG::Map", {
        "@parallax_name": "ParallaxAce",
        "@battleback1_name": "Grassland",
        "@battleback2_name": "Grassland",
        "@tileset_id": 1,
        "@events": { 1: RubyObj("RPG::Event", { "@id": 1, "@pages": [event_page] }) },
    })
    make_file(os.path.join(root, "Data", "Map001.rvdata2"), dump_marshal(map_obj))

    for g in [
        "Graphics/Titles1/Title1Ace.png",
        "Graphics/Titles2/Title2Ace.png",
        "Graphics/Gameovers/GameOverAce.png",
        "Graphics/System/Window.png",
        "Graphics/Characters/BoatAce.png",
        "Graphics/Characters/ShipAce.png",
        "Graphics/Characters/AirshipAce.png",
        "Graphics/Characters/Actor1.png",
        "Graphics/Characters/HeroNewAce.png",
        "Graphics/Characters/NpcAce.png",
        "Graphics/Faces/ActorFace1.png",
        "Graphics/Faces/FaceNewAce.png",
        "Graphics/Battlers/SlimeAce.png",
        "Graphics/Tilesets/TilesetAce1.png",
        "Graphics/Animations/Anim1Ace.png",
        "Graphics/Animations/Anim2Ace.png",
        "Graphics/Parallaxes/ParallaxAce.png",
        "Graphics/Battlebacks1/Grassland.png",
        "Graphics/Battlebacks2/Grassland.png",
        "Graphics/Pictures/PictureAce.png",
        "Graphics/System/IconSet.png",
    ]:
        make_dummy(os.path.join(root, g))

    for a in [
        "Audio/BGM/TitleBgmAce.ogg",
        "Audio/BGM/BattleBgmAce.ogg",
        "Audio/BGM/BoatBgmAce.ogg",
        "Audio/BGM/ShipBgmAce.ogg",
        "Audio/BGM/AirshipBgmAce.ogg",
        "Audio/ME/VictoryMeAce.ogg",
        "Audio/ME/GameOverMeAce.ogg",
        "Audio/SE/CursorSeAce.ogg",
        "Audio/SE/AnimSeAce.ogg",
        "Audio/SE/SeAce.ogg",
    ]:
        make_dummy(os.path.join(root, a))


def create_vxace_missing_asset(root: str):
    create_vxace_valid(root)
    # Remove one required picture asset
    pic_path = os.path.join(root, "Graphics", "Pictures", "PictureAce.png")
    if os.path.exists(pic_path):
        os.remove(pic_path)


def create_vxace_case_mismatch(root: str):
    create_vxace_valid(root)
    # Rename PictureAce.png to pictureace.png
    old_pic = os.path.join(root, "Graphics", "Pictures", "PictureAce.png")
    new_pic = os.path.join(root, "Graphics", "Pictures", "pictureace.png")
    if os.path.exists(old_pic):
        os.rename(old_pic, new_pic + ".tmp")
        os.rename(new_pic + ".tmp", new_pic)


def create_vxace_corrupt_marshal(root: str):
    create_vxace_valid(root)
    # Overwrite System.rvdata2 with corrupt bytes
    sys_path = os.path.join(root, "Data", "System.rvdata2")
    make_file(sys_path, b"\x04\x08\xff\xfe\x00\x12\x34\x56")


def create_vxace_broken_transfer(root: str):
    create_vxace_valid(root)
    # In Map001, command 201 transfers to map 999 which does not exist
    event_page = RubyObj("RPG::Event::Page", {
        "@graphic": RubyObj("RPG::Event::Page::Graphic", { "@character_name": "" }),
        "@move_route": RubyObj("RPG::MoveRoute", { "@list": [] }),
        "@list": [
            event_cmd(201, 0, [0, 999, 0, 0, 0, 0]), # broken transfer to Map 999
            event_cmd(0, 0, []),
        ]
    })
    map_obj = RubyObj("RPG::Map", {
        "@events": { 1: RubyObj("RPG::Event", { "@id": 1, "@pages": [event_page] }) },
    })
    make_file(os.path.join(root, "Data", "Map001.rvdata2"), dump_marshal(map_obj))


def create_vxace_variable_transfer(root: str):
    create_vxace_valid(root)
    # In Map001, command 201 has appointment_method = 1 (variable transfer)
    event_page = RubyObj("RPG::Event::Page", {
        "@graphic": RubyObj("RPG::Event::Page::Graphic", { "@character_name": "" }),
        "@move_route": RubyObj("RPG::MoveRoute", { "@list": [] }),
        "@list": [
            event_cmd(201, 0, [1, 42, 0, 0, 0, 0]), # variable transfer (variable id 42)
            event_cmd(0, 0, []),
        ]
    })
    map_obj = RubyObj("RPG::Map", {
        "@events": { 1: RubyObj("RPG::Event", { "@id": 1, "@pages": [event_page] }) },
    })
    make_file(os.path.join(root, "Data", "Map001.rvdata2"), dump_marshal(map_obj))


def create_vxace_transfer_case_mismatch(root: str):
    create_vxace_valid(root)
    # Rename Data/Map001.rvdata2 to Data/map001.rvdata2
    old_p = os.path.join(root, "Data", "Map001.rvdata2")
    new_p = os.path.join(root, "Data", "map001.rvdata2")
    if os.path.exists(old_p):
        os.rename(old_p, new_p + ".tmp")
        os.rename(new_p + ".tmp", new_p)


def create_vxace_deep_encrypted(root: str):
    create_vxace_valid(root)
    # Add Game.rgss3a
    make_file(os.path.join(root, "Game.rgss3a"), b"RGSSAD\x03\x00\x00\x00ENCRYPTED_HEADER_TEST")


def create_xp_missing_autotile(root: str):
    create_xp_valid(root)
    p = os.path.join(root, "Graphics", "Autotiles", "Autotile1.png")
    if os.path.exists(p):
        os.remove(p)


def create_xp_missing_animation_se(root: str):
    create_xp_valid(root)
    p = os.path.join(root, "Audio", "SE", "AnimSeXP.ogg")
    if os.path.exists(p):
        os.remove(p)


def create_xp_missing_move_route_se(root: str):
    create_xp_valid(root)
    p = os.path.join(root, "Audio", "SE", "StepSe.ogg")
    if os.path.exists(p):
        os.remove(p)


def create_xp_missing_common_event_pic(root: str):
    create_xp_valid(root)
    p = os.path.join(root, "Graphics", "Pictures", "CommonPic.png")
    if os.path.exists(p):
        os.remove(p)


def create_xp_missing_troop_se(root: str):
    create_xp_valid(root)
    p = os.path.join(root, "Audio", "SE", "BattleSe.ogg")
    if os.path.exists(p):
        os.remove(p)


def create_rgss3_encrypted_only(root: str):
    make_file(os.path.join(root, "Game.ini"), b"[Game]\nLibrary=System\\RGSS301.dll\nScripts=Data\\Scripts.rvdata2\nTitle=Encrypted Only\nRTP=RPGVXAce\n")
    make_file(os.path.join(root, "Game.rgss3a"), b"RGSSAD\x03\x00\x00\x00ENCRYPTED_HEADER_TEST")


def create_xp_missing_database_file(root: str):
    create_xp_valid(root)
    p = os.path.join(root, "Data", "Actors.rxdata")
    if os.path.exists(p):
        os.remove(p)


def create_cjk_font_test(root: str):
    create_vxace_valid(root)
    # In Map001, command 401 has Japanese dialogue with 「 and 」
    event_page = RubyObj("RPG::Event::Page", {
        "@graphic": RubyObj("RPG::Event::Page::Graphic", { "@character_name": "" }),
        "@move_route": RubyObj("RPG::MoveRoute", { "@list": [] }),
        "@list": [
            event_cmd(101, 0, ["", 0, 0, 2]),
            event_cmd(401, 0, ["「テスト対話」"]),
            event_cmd(0, 0, []),
        ]
    })
    map_obj = RubyObj("RPG::Map", {
        "@events": { 1: RubyObj("RPG::Event", { "@id": 1, "@pages": [event_page] }) },
    })
    make_file(os.path.join(root, "Data", "Map001.rvdata2"), dump_marshal(map_obj))


def main():
    base_dir = os.path.dirname(os.path.abspath(__file__))
    fixtures = {
        "xp_valid": create_xp_valid,
        "vx_valid": create_vx_valid,
        "vxace_valid": create_vxace_valid,
        "vxace_missing_asset": create_vxace_missing_asset,
        "vxace_case_mismatch": create_vxace_case_mismatch,
        "vxace_corrupt_marshal": create_vxace_corrupt_marshal,
        "vxace_broken_transfer": create_vxace_broken_transfer,
        "vxace_variable_transfer": create_vxace_variable_transfer,
        "vxace_transfer_case_mismatch": create_vxace_transfer_case_mismatch,
        "vxace_deep_encrypted": create_vxace_deep_encrypted,
        "xp_missing_autotile": create_xp_missing_autotile,
        "xp_missing_animation_se": create_xp_missing_animation_se,
        "xp_missing_move_route_se": create_xp_missing_move_route_se,
        "xp_missing_common_event_pic": create_xp_missing_common_event_pic,
        "xp_missing_troop_se": create_xp_missing_troop_se,
        "xp_missing_database_file": create_xp_missing_database_file,
        "rgss3_encrypted_only": create_rgss3_encrypted_only,
        "cjk_font_test": create_cjk_font_test,
    }
    for name, func in fixtures.items():
        fdir = os.path.join(base_dir, name)
        func(fdir)
        print(f"Generated fixture: {name} in {fdir}")

if __name__ == "__main__":
    main()
