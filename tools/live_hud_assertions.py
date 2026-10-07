"""Read original atlas digits emitted by the native HUD, without text fallbacks."""
import json
from native_abi import ROOT

scene = json.loads((ROOT / "native/generated/editor-scene.json").read_text())
digits = []
for widget in scene["live"]["hud"]["score_numbers"][1]["digits"]:
    sprite = scene["sprites"][widget["guid"]]
    x, y, w, h = sprite["rect"]
    tw, th = scene["textures"][sprite["texture"]]
    digits.append((sprite["texture"], (x/tw, 1-(y+h)/th, (x+w)/tw, 1-y/th)))


def hud_number(frame, area, minimum_height=0):
    left, top, right, bottom = area
    visible = []
    for draw in frame.draws[:frame.count]:
        if draw.kind != 2 or not draw.text or draw.rgba & 255 == 0:
            continue
        if not (left < draw.rect[0] < right and top < draw.rect[1] < bottom):
            continue
        if draw.rect[3] < minimum_height:
            continue
        for digit, (texture, uv) in enumerate(digits):
            if (draw.text.decode().endswith(texture)
                    and all(abs(a-b) < 1e-6 for a, b in zip(draw.uv, uv))):
                visible.append((draw.rect[0], str(digit)))
                break
    assert visible, "Original HUD digit sprites were not rendered"
    return "".join(digit for _, digit in sorted(visible))


def score_number(frame):
    return hud_number(frame, (100, 70, 750, 150), minimum_height=38)


def life_number(frame):
    return hud_number(frame, (1650, -10, 1820, 70))


def gain_number(frame):
    """Decode the visible white gain popup using its own authored glyphs."""
    number = scene["live"]["hud"]["add_score_numbers"][1]
    visible = []
    for draw in frame.draws[:frame.count]:
        if draw.kind != 2 or not draw.text or draw.rgba & 255 == 0 or draw.rgba >> 8 != 0xffffff:
            continue
        for digit, glyph in enumerate(number["digits"]):
            sprite = scene["sprites"][glyph["guid"]]
            x, y, w, h = sprite["rect"]
            tw, th = scene["textures"][sprite["texture"]]
            uv = (x/tw, 1-(y+h)/th, (x+w)/tw, 1-y/th)
            if (draw.text.decode().endswith(sprite["texture"])
                    and all(abs(a-b) < 1e-5 for a, b in zip(draw.uv, uv))
                    and abs(draw.rect[1] - number["origin"][1] - glyph["rect"][1]) < .1
                    and abs(draw.rect[3] - glyph["rect"][3]) < .1):
                visible.append((draw.rect[0], str(digit)))
                break
    return "".join(digit for _, digit in sorted(visible))
