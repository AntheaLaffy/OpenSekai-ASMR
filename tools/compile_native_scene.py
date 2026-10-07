#!/usr/bin/env python3
"""Bundle original prefab objects and resolved sprites for the Rust UI runtime.

Coordinates, hierarchy, GUIDs and component fields remain source data. No UI
geometry is redrawn in this compiler, and original assets are never modified.
"""
import hashlib
import json
from pathlib import Path
import struct
from compile_live_presentations import compile_presentations
from compile_live_background import compile_background
from compile_live_hud import compile_hud
from compile_live_hit import compile_hit_styles, compile_effect_camera

ROOT = Path(__file__).resolve().parents[1]
MAIN = "Assets/Resources/screen/prefabs/ScreenLayerMusicScoreMaker.prefab"
NOTE = "Assets/GameObject/NotePreviewPrefab.prefab"


def compile_live(index):
    path = "Assets/Resources/live/view/FrontUIView.prefab"
    objects = index["assets"][path]["objects"]
    by_id = {o["id"]: o for o in objects}
    transforms = {o["fields"]["m_GameObject"]["fileID"]: o for o in objects if o["kind"] == "Transform"}
    def world(go):
        x = y = 0.0
        names = []
        t = transforms.get(go)
        while t:
            f = t["fields"]
            name = by_id[f["m_GameObject"]["fileID"]]["fields"]["m_Name"]
            names.append(name)
            # The source anchors place their normalized offsets at the camera's
            # orthographic edges; FrontUIView uses 108 pixels per world unit.
            factor = 5 if name in ("LeftTopAnchor", "RightTopAnchor") else 1
            x += f["m_LocalPosition"]["x"] * factor
            y += f["m_LocalPosition"]["y"] * factor
            t = by_id.get(f["m_Father"]["fileID"])
        return x, y, names
    static = []
    for o in objects:
        if o["kind"] != "SpriteRenderer":
            continue
        f = o["fields"]
        go = f["m_GameObject"]["fileID"]
        name = by_id[go]["fields"]["m_Name"]
        x, y, ancestors = world(go)
        if name not in ("LaneDefault", "LaneLine", "JudgeLane"):
            continue
        if name == "PauseButton" and "LifeView" not in ancestors:
            continue
        guid = f["m_Sprite"].get("guid")
        asset = index["assets"][index["guid_paths"][guid]]["objects"][0]["fields"]
        pivot = asset.get("m_Pivot", {"x": .5, "y": .5})
        size = f["m_Size"]
        if f["m_DrawMode"] == 0:
            w = asset["m_Rect"]["width"] / asset["m_PixelsToUnits"] * 108
            h = asset["m_Rect"]["height"] / asset["m_PixelsToUnits"] * 108
        else:
            w, h = size["x"] * 108, size["y"] * 108
        static.append({"name": name, "guid": guid, "order": f["m_SortingOrder"],
                       "rect": [960 + x * 108 - w * pivot["x"], 540 - y * 108 - h * (1-pivot["y"]), w, h],
                       "sliced": f["m_DrawMode"] != 0})
    skins = {}
    arrows = {}
    prefix = "Assets/Sekai/assetbundle/resources/startapp/live/note/custom01/"
    for name, asset in index["assets"].items():
        if not name.startswith(prefix) or not name.endswith(".prefab"):
            continue
        objs = {o["id"]: o for o in asset["objects"]}
        view = next((o for o in objs.values() if "spriteRenderer" in o["fields"]), None)
        if view:
            renderer = objs[view["fields"]["spriteRenderer"]["fileID"]]["fields"]
            skins[Path(name).stem] = renderer["m_Sprite"]["guid"]
            for key in ("defaultArrowSprites", "slanArrowSprites"):
                if key in view["fields"]:
                    arrows[Path(name).stem + "." + key] = [r["guid"] for r in view["fields"][key]]
    return {"source": path, "pixels_per_unit": 108, "static": sorted(static, key=lambda s:s["order"]),
            "notes": skins, "arrows": arrows, "hud": compile_hud(index, objects),
            "hit_styles": compile_hit_styles(index),
            "effect_camera": compile_effect_camera(index),
            "line_texture": prefix + "textures/longNoteLine.png"}


def compile_scene():
    index = json.loads((ROOT / "native/generated/unity-assets.json").read_text())
    prefabs = {name: index["assets"][name] for name in (MAIN, NOTE)}
    for obj in prefabs[MAIN]["objects"]:
        if obj.get("script", {}).get("path", "").endswith("BarLinePreview.cs"):
            for key, reference in obj["fields"].items():
                if key.startswith("_") and isinstance(reference, dict) and "guid" in reference:
                    name = index["guid_paths"][reference["guid"]]
                    prefabs[name] = index["assets"][name]
    sprites, textures = {}, {}
    for guid, name in index["guid_paths"].items():
        for obj in index["assets"].get(name, {}).get("objects", []):
            if obj["kind"] != "Sprite":
                continue
            fields = obj["fields"]
            render = fields["m_RD"]
            texture = index["guid_paths"].get(render["texture"].get("guid"))
            if texture is None or not texture.lower().endswith(".png"):
                continue
            if texture not in textures:
                header = (ROOT / texture).read_bytes()[:24]
                assert header[:8] == b"\x89PNG\r\n\x1a\n", texture
                textures[texture] = list(struct.unpack(">II", header[16:24]))
            rect = render["textureRect"]
            border = fields["m_Border"]
            sprites[guid] = {
                "name": fields["m_Name"], "texture": texture,
                "rect": [rect[k] for k in ("x", "y", "width", "height")],
                "border": [border[k] for k in ("x", "y", "z", "w")],
                "pixels_per_unit": fields["m_PixelsToUnits"],
                "settings_raw": render["settingsRaw"],
            }
    keys = {o["fields"].get("wordingKey") for a in prefabs.values() for o in a["objects"]}
    wording = {}
    for filename in ("wording_zh.txt", "master_wording_zh.txt"):
        for line in (ROOT / "Assets/Resources/wording" / filename).read_text(encoding="utf-8-sig").splitlines():
            columns = line.split(",")
            if len(columns) == 2 and columns[0] in keys:
                wording[columns[0]] = columns[1].replace("_x_COMMA_x_", ",").replace("\\n", "\n")
    for name, asset in prefabs.items():
        assert hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == asset["sha256"], name
    bundle = {"schema": 1, "source_commit": index["source_commit"], "main": MAIN,
              "note": NOTE, "prefabs": prefabs, "sprites": sprites, "textures": textures,
              "wording_zh": wording, "live": compile_live(index),
              "live_background": compile_background(index),
              "presentations": compile_presentations(index)}
    line = bundle["live"]["line_texture"]
    header = (ROOT / line).read_bytes()[:24]
    assert header[:8] == b"\x89PNG\r\n\x1a\n", line
    textures[line] = list(struct.unpack(">II", header[16:24]))
    for presentation in bundle["presentations"]["results"].values():
        for renderer in presentation["meshes"] + presentation["particles"]:
            texture = renderer["texture"]
            header = (ROOT / texture).read_bytes()[:24]
            assert header[:8] == b"\x89PNG\r\n\x1a\n", texture
            textures[texture] = list(struct.unpack(">II", header[16:24]))
    output = ROOT / "native/generated/editor-scene.json"
    output.write_text(json.dumps(bundle, ensure_ascii=False, separators=(",", ":")) + "\n")
    print(f"Native scene: {len(prefabs)} original prefabs, {len(sprites)} sprites, {len(textures)} textures; {output.stat().st_size} bytes")


if __name__ == "__main__":
    compile_scene()
