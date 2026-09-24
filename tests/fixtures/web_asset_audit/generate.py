import os
import json
import shutil

BASE_DIR = os.path.dirname(os.path.abspath(__file__))

DUMMY_PNG = b'\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15c4\x00\x00\x00\nIDATx\x9cc\x00\x01\x00\x00\x05\x00\x01\r\n-\xb4\x00\x00\x00\x00IEND\xaeB`\x82'
DUMMY_OGG = b'OggS\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01\x1evorbis'
DUMMY_WEBM = b'\x1a\x45\xdf\xa3\x9f\x42\x86\x81\x01\x42\xf7\x81\x01\x42\xf2\x81\x04\x42\xf3\x81\x08'
DUMMY_EFK = b'Effekseer\x00\x01\x00\x00'
DUMMY_RPGMVP = b'RPGMV\x00\x00\x00\x00\x03\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00' + DUMMY_PNG
DUMMY_RPGMVO = b'RPGMV\x00\x00\x00\x00\x03\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00' + DUMMY_OGG

def write_file(path: str, content: bytes | str):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    if isinstance(content, str):
        with open(path, 'w', encoding='utf-8') as f:
            f.write(content)
    else:
        with open(path, 'wb') as f:
            f.write(content)

def make_base_database(is_mz: bool = False, encryption: dict | None = None) -> dict[str, str]:
    enc = encryption or {}
    has_enc_img = enc.get("hasEncryptedImages", False)
    has_enc_aud = enc.get("hasEncryptedAudio", False)
    enc_key = enc.get("encryptionKey", "")

    system = {
        "gameTitle": "Test Web Game",
        "title1Name": "TitleMV1",
        "title2Name": "TitleMV2",
        "windowskinName": "Window",
        "sounds": [
            {"name": "Cursor", "pan": 0, "pitch": 100, "volume": 90},
            {"name": "Decision", "pan": 0, "pitch": 100, "volume": 90}
        ],
        "titleBgm": {"name": "ThemeMV", "pan": 0, "pitch": 100, "volume": 90},
        "battleBgm": {"name": "BattleMV", "pan": 0, "pitch": 100, "volume": 90},
        "victoryMe": {"name": "VictoryMV", "pan": 0, "pitch": 100, "volume": 90},
        "defeatMe": {"name": "DefeatMV", "pan": 0, "pitch": 100, "volume": 90},
        "gameoverMe": {"name": "GameOverMV", "pan": 0, "pitch": 100, "volume": 90},
        "boat": {"characterName": "VehicleMV", "bgm": {"name": "ShipBgm", "pan": 0, "pitch": 100, "volume": 90}},
        "ship": {"characterName": "VehicleMV", "bgm": {"name": "ShipBgm", "pan": 0, "pitch": 100, "volume": 90}},
        "airship": {"characterName": "VehicleMV", "bgm": {"name": "ShipBgm", "pan": 0, "pitch": 100, "volume": 90}},
        "hasEncryptedImages": has_enc_img,
        "hasEncryptedAudio": has_enc_aud,
        "encryptionKey": enc_key
    }

    actors = [
        None,
        {
            "id": 1,
            "name": "Harold",
            "characterName": "Actor1",
            "characterIndex": 0,
            "faceName": "Actor1_face",
            "faceIndex": 0,
            "battlerName": "Actor1_sv"
        }
    ]

    enemies = [
        None,
        {
            "id": 1,
            "name": "Bat",
            "battlerName": "Bat"
        }
    ]

    tilesets = [
        None,
        {
            "id": 1,
            "name": "Overworld",
            "tilesetNames": ["Overworld_A1", "Overworld_B", "", "", "", "", "", "", ""]
        }
    ]

    animations = [
        None,
        {
            "id": 1,
            "name": "Slash",
            "animation1Name": "Hit1",
            "animation2Name": "Hit2",
            "timings": [
                {"frame": 1, "se": {"name": "SlashSE", "pan": 0, "pitch": 100, "volume": 90}}
            ]
        }
    ]
    if is_mz:
        animations.append({
            "id": 2,
            "name": "Laser",
            "effectName": "LaserBeam",
            "timings": []
        })

    common_events = [
        None,
        {
            "id": 1,
            "name": "Intro",
            "list": [
                {"code": 231, "indent": 0, "parameters": [1, "CommonPic", 0, 0, 0, 0, 100, 100, 255, 0]},
                {"code": 0, "indent": 0, "parameters": []}
            ]
        }
    ]

    troops = [
        None,
        {
            "id": 1,
            "name": "Bats*2",
            "pages": [
                {
                    "list": [
                        {"code": 250, "indent": 0, "parameters": [{"name": "TroopSE", "pan": 0, "pitch": 100, "volume": 90}]},
                        {"code": 0, "indent": 0, "parameters": []}
                    ]
                }
            ]
        }
    ]

    skills = [
        None,
        {"id": 1, "name": "Attack", "iconIndex": 76, "animationId": 1}
    ]
    items = [
        None,
        {"id": 1, "name": "Potion", "iconIndex": 176}
    ]
    weapons = [
        None,
        {"id": 1, "name": "Sword", "iconIndex": 96}
    ]
    armors = [
        None,
        {"id": 1, "name": "Shield", "iconIndex": 128}
    ]
    states = [
        None,
        {"id": 1, "name": "Knockout", "iconIndex": 1}
    ]

    return {
        "System.json": json.dumps(system, indent=2),
        "Actors.json": json.dumps(actors, indent=2),
        "Enemies.json": json.dumps(enemies, indent=2),
        "Tilesets.json": json.dumps(tilesets, indent=2),
        "Animations.json": json.dumps(animations, indent=2),
        "CommonEvents.json": json.dumps(common_events, indent=2),
        "Troops.json": json.dumps(troops, indent=2),
        "Skills.json": json.dumps(skills, indent=2),
        "Items.json": json.dumps(items, indent=2),
        "Weapons.json": json.dumps(weapons, indent=2),
        "Armors.json": json.dumps(armors, indent=2),
        "States.json": json.dumps(states, indent=2)
    }

def make_map001(
    picture_name: str = "MapPicMV",
    transfer_mode: int = 0,
    transfer_map_id: int = 2,
    extra_commands: list | None = None
) -> str:
    commands = [
        {"code": 201, "indent": 0, "parameters": [transfer_mode, transfer_map_id, 5, 5, 0, 0]},
        {"code": 231, "indent": 0, "parameters": [1, picture_name, 0, 0, 0, 0, 100, 100, 255, 0]},
        {"code": 241, "indent": 0, "parameters": [{"name": "EventBgm", "pan": 0, "pitch": 100, "volume": 90}]},
        {"code": 245, "indent": 0, "parameters": [{"name": "EventBgs", "pan": 0, "pitch": 100, "volume": 90}]},
        {"code": 249, "indent": 0, "parameters": [{"name": "EventMe", "pan": 0, "pitch": 100, "volume": 90}]},
        {"code": 250, "indent": 0, "parameters": [{"name": "EventSe", "pan": 0, "pitch": 100, "volume": 90}]},
        {"code": 261, "indent": 0, "parameters": ["IntroMovie"]},
        {"code": 322, "indent": 0, "parameters": [1, "HeroNew", 0, "HeroNewFace", 0, "HeroNewSV"]},
        {"code": 212, "indent": 0, "parameters": [0, 1, False]},
    ]
    if extra_commands:
        commands.extend(extra_commands)
    commands.append({"code": 0, "indent": 0, "parameters": []})

    map_data = {
        "displayName": "Starting Town",
        "autoplayBgm": True,
        "bgm": {"name": "MapBgm", "pan": 0, "pitch": 100, "volume": 90},
        "autoplayBgs": True,
        "bgs": {"name": "MapBgs", "pan": 0, "pitch": 100, "volume": 90},
        "parallaxName": "Parallax1",
        "battleback1Name": "Grassland",
        "battleback2Name": "Forest",
        "tilesetId": 1,
        "events": [
            None,
            {
                "id": 1,
                "name": "EV001",
                "pages": [
                    {
                        "image": {"characterName": "NPC1", "characterIndex": 0},
                        "moveRoute": {
                            "list": [
                                {"code": 41, "parameters": ["NPC2", 0]},
                                {"code": 44, "parameters": [{"name": "StepSE", "pan": 0, "pitch": 100, "volume": 90}]},
                                {"code": 0, "parameters": []}
                            ]
                        },
                        "list": commands
                    }
                ]
            }
        ]
    }
    return json.dumps(map_data, indent=2)

def make_map002() -> str:
    map_data = {
        "displayName": "Target Map",
        "autoplayBgm": False,
        "bgm": {"name": ""},
        "autoplayBgs": False,
        "bgs": {"name": ""},
        "parallaxName": "",
        "battleback1Name": "",
        "battleback2Name": "",
        "tilesetId": 1,
        "events": [None]
    }
    return json.dumps(map_data, indent=2)

def populate_valid_assets(root: str, prefix: str = "", is_mz: bool = False, encrypted: str | None = None):
    # Standard images
    images = [
        "img/titles1/TitleMV1.png",
        "img/titles2/TitleMV2.png",
        "img/system/Window.png",
        "img/system/IconSet.png",
        "img/characters/VehicleMV.png",
        "img/characters/Actor1.png",
        "img/faces/Actor1_face.png",
        "img/sv_actors/Actor1_sv.png",
        "img/enemies/Bat.png",
        "img/tilesets/Overworld_A1.png",
        "img/tilesets/Overworld_B.png",
        "img/animations/Hit1.png",
        "img/animations/Hit2.png",
        "img/pictures/CommonPic.png",
        "img/parallaxes/Parallax1.png",
        "img/battlebacks1/Grassland.png",
        "img/battlebacks2/Forest.png",
        "img/characters/NPC1.png",
        "img/characters/NPC2.png",
        "img/pictures/MapPicMV.png",
        "img/characters/HeroNew.png",
        "img/faces/HeroNewFace.png",
        "img/sv_actors/HeroNewSV.png",
        "img/pictures/PluginPic.png",
    ]

    audio = [
        "audio/se/Cursor.ogg",
        "audio/se/Decision.ogg",
        "audio/se/SlashSE.ogg",
        "audio/se/TroopSE.ogg",
        "audio/se/StepSE.ogg",
        "audio/se/EventSe.ogg",
        "audio/bgm/ThemeMV.ogg",
        "audio/bgm/BattleMV.ogg",
        "audio/bgm/ShipBgm.ogg",
        "audio/bgm/MapBgm.ogg",
        "audio/bgm/EventBgm.ogg",
        "audio/bgs/MapBgs.ogg",
        "audio/bgs/EventBgs.ogg",
        "audio/me/VictoryMV.ogg",
        "audio/me/DefeatMV.ogg",
        "audio/me/GameOverMV.ogg",
        "audio/me/EventMe.ogg",
    ]

    for img in images:
        p = os.path.join(root, prefix, img)
        if encrypted == "mv":
            enc_p = p.replace(".png", ".rpgmvp")
            write_file(enc_p, DUMMY_RPGMVP)
        elif encrypted == "mz":
            enc_p = p.replace(".png", ".png_")
            write_file(enc_p, DUMMY_PNG)
        else:
            write_file(p, DUMMY_PNG)

    for aud in audio:
        p = os.path.join(root, prefix, aud)
        if encrypted == "mv":
            enc_p = p.replace(".ogg", ".rpgmvo")
            write_file(enc_p, DUMMY_RPGMVO)
        elif encrypted == "mz":
            enc_p = p.replace(".ogg", ".ogg_")
            write_file(enc_p, DUMMY_OGG)
        else:
            write_file(p, DUMMY_OGG)

    write_file(os.path.join(root, prefix, "movies/IntroMovie.webm"), DUMMY_WEBM)

    if is_mz:
        write_file(os.path.join(root, prefix, "effects/LaserBeam.efkefc"), DUMMY_EFK)

def generate_mv_valid_root():
    root = os.path.join(BASE_DIR, "mv_valid_root")
    shutil.rmtree(root, ignore_errors=True)
    write_file(os.path.join(root, "index.html"), "<html><body></body></html>")
    write_file(os.path.join(root, "js/rpg_core.js"), "// MV rpg_core.js")
    write_file(os.path.join(root, "js/rpg_managers.js"), "// MV rpg_managers.js")

    db = make_base_database(is_mz=False)
    for name, content in db.items():
        write_file(os.path.join(root, "data", name), content)

    write_file(os.path.join(root, "data/Map001.json"), make_map001())
    write_file(os.path.join(root, "data/Map002.json"), make_map002())

    plugins_js = 'var $plugins = [\n{"name":"SamplePlugin","status":true,"description":"test","parameters":{"bg":"img/pictures/PluginPic.png"}}\n];'
    write_file(os.path.join(root, "js/plugins.js"), plugins_js)
    write_file(os.path.join(root, "js/plugins/SamplePlugin.js"), "// SamplePlugin code\nvar x = 'img/pictures/PluginPic.png';")

    populate_valid_assets(root)

def generate_mv_valid_www():
    root = os.path.join(BASE_DIR, "mv_valid_www")
    shutil.rmtree(root, ignore_errors=True)
    write_file(os.path.join(root, "package.json"), '{"name": "test_game", "main": "www/index.html"}')
    write_file(os.path.join(root, "www/index.html"), "<html><body></body></html>")
    write_file(os.path.join(root, "www/js/rpg_core.js"), "// MV rpg_core.js")
    write_file(os.path.join(root, "www/js/rpg_managers.js"), "// MV rpg_managers.js")

    db = make_base_database(is_mz=False)
    for name, content in db.items():
        write_file(os.path.join(root, "www/data", name), content)

    write_file(os.path.join(root, "www/data/Map001.json"), make_map001())
    write_file(os.path.join(root, "www/data/Map002.json"), make_map002())

    plugins_js = 'var $plugins = [\n{"name":"SamplePlugin","status":true,"description":"test","parameters":{"bg":"img/pictures/PluginPic.png"}}\n];'
    write_file(os.path.join(root, "www/js/plugins.js"), plugins_js)
    write_file(os.path.join(root, "www/js/plugins/SamplePlugin.js"), "// SamplePlugin code")

    populate_valid_assets(root, prefix="www")

def generate_mz_valid_root():
    root = os.path.join(BASE_DIR, "mz_valid_root")
    shutil.rmtree(root, ignore_errors=True)
    write_file(os.path.join(root, "index.html"), "<html><body></body></html>")
    write_file(os.path.join(root, "js/rmmz_core.js"), "// MZ rmmz_core.js")
    write_file(os.path.join(root, "js/rmmz_managers.js"), "// MZ rmmz_managers.js")

    db = make_base_database(is_mz=True)
    for name, content in db.items():
        write_file(os.path.join(root, "data", name), content)

    extra = [{"code": 212, "indent": 0, "parameters": [0, 2, False]}]
    write_file(os.path.join(root, "data/Map001.json"), make_map001(extra_commands=extra))
    write_file(os.path.join(root, "data/Map002.json"), make_map002())

    plugins_js = 'var $plugins = [\n{"name":"SamplePlugin","status":true,"description":"test","parameters":{"bg":"img/pictures/PluginPic.png"}}\n];'
    write_file(os.path.join(root, "js/plugins.js"), plugins_js)
    write_file(os.path.join(root, "js/plugins/SamplePlugin.js"), "// MZ Plugin")

    populate_valid_assets(root, is_mz=True)

def generate_mv_missing_asset():
    root = os.path.join(BASE_DIR, "mv_missing_asset")
    shutil.rmtree(root, ignore_errors=True)
    generate_mv_valid_root()
    shutil.copytree(os.path.join(BASE_DIR, "mv_valid_root"), root)
    os.remove(os.path.join(root, "img/pictures/MapPicMV.png"))

def generate_mv_case_mismatch():
    root = os.path.join(BASE_DIR, "mv_case_mismatch")
    shutil.rmtree(root, ignore_errors=True)
    generate_mv_valid_root()
    shutil.copytree(os.path.join(BASE_DIR, "mv_valid_root"), root)
    # Map references "mappicmv", disk is "MapPicMV.png"
    write_file(os.path.join(root, "data/Map001.json"), make_map001(picture_name="mappicmv"))

def generate_mv_broken_transfer():
    root = os.path.join(BASE_DIR, "mv_broken_transfer")
    shutil.rmtree(root, ignore_errors=True)
    generate_mv_valid_root()
    shutil.copytree(os.path.join(BASE_DIR, "mv_valid_root"), root)
    # Transfer points to 999
    write_file(os.path.join(root, "data/Map001.json"), make_map001(transfer_mode=0, transfer_map_id=999))

def generate_mv_variable_transfer():
    root = os.path.join(BASE_DIR, "mv_variable_transfer")
    shutil.rmtree(root, ignore_errors=True)
    generate_mv_valid_root()
    shutil.copytree(os.path.join(BASE_DIR, "mv_valid_root"), root)
    # Transfer mode 1 (variable transfer)
    write_file(os.path.join(root, "data/Map001.json"), make_map001(transfer_mode=1, transfer_map_id=10))

def generate_mv_malformed_json():
    root = os.path.join(BASE_DIR, "mv_malformed_json")
    shutil.rmtree(root, ignore_errors=True)
    generate_mv_valid_root()
    shutil.copytree(os.path.join(BASE_DIR, "mv_valid_root"), root)
    write_file(os.path.join(root, "data/System.json"), '{\n  "gameTitle": "Broken",\n  "sounds": [')

def generate_mv_unsafe_path():
    root = os.path.join(BASE_DIR, "mv_unsafe_path")
    shutil.rmtree(root, ignore_errors=True)
    generate_mv_valid_root()
    shutil.copytree(os.path.join(BASE_DIR, "mv_valid_root"), root)
    write_file(os.path.join(root, "data/Map001.json"), make_map001(picture_name="../../secret"))

def generate_mv_encrypted():
    root = os.path.join(BASE_DIR, "mv_encrypted")
    shutil.rmtree(root, ignore_errors=True)
    write_file(os.path.join(root, "index.html"), "<html><body></body></html>")
    write_file(os.path.join(root, "js/rpg_core.js"), "// MV rpg_core.js")
    write_file(os.path.join(root, "js/rpg_managers.js"), "// MV rpg_managers.js")

    db = make_base_database(is_mz=False, encryption={
        "hasEncryptedImages": True,
        "hasEncryptedAudio": True,
        "encryptionKey": "d41d8cd98f00b204e9800998ecf8427e"
    })
    for name, content in db.items():
        write_file(os.path.join(root, "data", name), content)

    write_file(os.path.join(root, "data/Map001.json"), make_map001())
    write_file(os.path.join(root, "data/Map002.json"), make_map002())
    write_file(os.path.join(root, "js/plugins.js"), 'var $plugins = [];')

    populate_valid_assets(root, encrypted="mv")

def generate_mz_encrypted():
    root = os.path.join(BASE_DIR, "mz_encrypted")
    shutil.rmtree(root, ignore_errors=True)
    write_file(os.path.join(root, "index.html"), "<html><body></body></html>")
    write_file(os.path.join(root, "js/rmmz_core.js"), "// MZ rmmz_core.js")
    write_file(os.path.join(root, "js/rmmz_managers.js"), "// MZ rmmz_managers.js")

    db = make_base_database(is_mz=True, encryption={
        "hasEncryptedImages": True,
        "hasEncryptedAudio": True,
        "encryptionKey": "d41d8cd98f00b204e9800998ecf8427e"
    })
    for name, content in db.items():
        write_file(os.path.join(root, "data", name), content)

    write_file(os.path.join(root, "data/Map001.json"), make_map001())
    write_file(os.path.join(root, "data/Map002.json"), make_map002())
    write_file(os.path.join(root, "js/plugins.js"), 'var $plugins = [];')

    populate_valid_assets(root, is_mz=True, encrypted="mz")

def generate_mv_unsupported_node():
    root = os.path.join(BASE_DIR, "mv_unsupported_node")
    shutil.rmtree(root, ignore_errors=True)
    generate_mv_valid_root()
    shutil.copytree(os.path.join(BASE_DIR, "mv_valid_root"), root)
    write_file(os.path.join(root, "js/plugins/native_addon.node"), b"\x7fELF\x02\x01\x01\x00")

def generate_mv_external_url():
    root = os.path.join(BASE_DIR, "mv_external_url")
    shutil.rmtree(root, ignore_errors=True)
    generate_mv_valid_root()
    shutil.copytree(os.path.join(BASE_DIR, "mv_valid_root"), root)
    plugins_js = 'var $plugins = [\n{"name":"NetPlugin","status":true,"description":"net","parameters":{"remote":"https://example.com/assets/banner.png"}}\n];'
    write_file(os.path.join(root, "js/plugins.js"), plugins_js)
    write_file(os.path.join(root, "js/plugins/NetPlugin.js"), "// NetPlugin")

def generate_mv_deep_plugin_literals():
    root = os.path.join(BASE_DIR, "mv_deep_plugin_literals")
    shutil.rmtree(root, ignore_errors=True)
    generate_mv_valid_root()
    shutil.copytree(os.path.join(BASE_DIR, "mv_valid_root"), root)
    extra_plugin = '''
    // ExtraPlugin with static asset literals
    var splashImg = "img/pictures/SecretArt.png";
    var extraSe = "audio/se/SecretSe.ogg";
    '''
    plugins_js = 'var $plugins = [\n{"name":"ExtraPlugin","status":true,"description":"extra","parameters":{}}\n];'
    write_file(os.path.join(root, "js/plugins.js"), plugins_js)
    write_file(os.path.join(root, "js/plugins/ExtraPlugin.js"), extra_plugin)
    write_file(os.path.join(root, "img/pictures/SecretArt.png"), DUMMY_PNG)
    write_file(os.path.join(root, "audio/se/SecretSe.ogg"), DUMMY_OGG)

def generate_mv_deep_dynamic():
    root = os.path.join(BASE_DIR, "mv_deep_dynamic")
    shutil.rmtree(root, ignore_errors=True)
    generate_mv_valid_root()
    shutil.copytree(os.path.join(BASE_DIR, "mv_valid_root"), root)
    dynamic_plugin = '''
    // Dynamic expression that cannot be statically resolved
    var dynamicPath = "img/pictures/" + window.currentLevel + ".png";
    '''
    plugins_js = 'var $plugins = [\n{"name":"DynamicPlugin","status":true,"description":"dynamic","parameters":{}}\n];'
    write_file(os.path.join(root, "js/plugins.js"), plugins_js)
    write_file(os.path.join(root, "js/plugins/DynamicPlugin.js"), dynamic_plugin)

def main():
    print("Generating web asset audit fixtures...")
    generate_mv_valid_root()
    generate_mv_valid_www()
    generate_mz_valid_root()
    generate_mv_missing_asset()
    generate_mv_case_mismatch()
    generate_mv_broken_transfer()
    generate_mv_variable_transfer()
    generate_mv_malformed_json()
    generate_mv_unsafe_path()
    generate_mv_encrypted()
    generate_mz_encrypted()
    generate_mv_unsupported_node()
    generate_mv_external_url()
    generate_mv_deep_plugin_literals()
    generate_mv_deep_dynamic()
    print("Done!")

if __name__ == '__main__':
    main()
