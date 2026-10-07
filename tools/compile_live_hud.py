"""Resolve the original front HUD, including sprite digits and their pivots."""


def compile_hud(index, objects):
    by_id = {o["id"]: o for o in objects}
    transforms = {o["fields"]["m_GameObject"]["fileID"]: o for o in objects
                  if o["kind"] in ("Transform", "RectTransform")}
    runtime_positions = {}
    runtime_scales = {}

    def transform(go):
        t = transforms[go]["fields"]
        parent = by_id.get(t["m_Father"]["fileID"])
        p, s = transform(parent["fields"]["m_GameObject"]["fileID"]) if parent else ([0., 0.], [1., 1.])
        factor = 5 if by_id[go]["fields"]["m_Name"] in ("LeftTopAnchor", "RightTopAnchor") else 1
        local = runtime_positions.get(transforms[go]["id"], t["m_LocalPosition"])
        return ([p[i] + local[k] * factor * s[i] for i, k in enumerate(("x", "y"))],
                [s[i] * runtime_scales.get(transforms[go]["id"], t["m_LocalScale"])[k] for i, k in enumerate(("x", "y"))])

    def glyph(guid, scale, origin=(0., 0.)):
        f = index["assets"][index["guid_paths"][guid]]["objects"][0]["fields"]
        rect, pivot = f["m_Rect"], f["m_Pivot"]
        w, h = [rect[k] * scale[i] * 108 / f["m_PixelsToUnits"] for i, k in enumerate(("width", "height"))]
        return {"guid": guid, "rect": [origin[0] - w * pivot["x"], origin[1] - h * (1-pivot["y"]), w, h],
                "sliced": False, "color": [1., 1., 1., 1.]}

    def widget(renderer_id):
        f = by_id[renderer_id]["fields"]
        pos, scale = transform(f["m_GameObject"]["fileID"])
        origin = [960 + pos[0] * 108, 540 - pos[1] * 108]
        value = glyph(f["m_Sprite"]["guid"], scale, origin)
        if f["m_DrawMode"] != 0:
            w, h = [f["m_Size"][k] * scale[i] * 108 for i, k in enumerate(("x", "y"))]
            asset = index["assets"][index["guid_paths"][value["guid"]]]["objects"][0]["fields"]
            pivot = asset["m_Pivot"]
            value["rect"] = [origin[0]-w*pivot["x"], origin[1]-h*(1-pivot["y"]), w, h]
            value["sliced"] = True
        value["color"] = [f["m_Color"][k] for k in "rgba"]
        value["order"] = f["m_SortingOrder"]
        value["name"] = by_id[f["m_GameObject"]["fileID"]]["fields"]["m_Name"]
        return value

    def view(class_name):
        return next(o["fields"] for o in objects if o.get("script", {}).get("path", "").endswith(f"/{class_name}.cs"))

    def number(component):
        f = by_id[component]["fields"]
        root = transforms[f["m_GameObject"]["fileID"]]["fields"]
        first = by_id[root["m_Children"][0]["fileID"]]["fields"]
        pos, scale = transform(f["m_GameObject"]["fileID"])
        glyph_scale = [scale[i] * first["m_LocalScale"][k] for i, k in enumerate(("x", "y"))]
        return {"origin": [960+pos[0]*108, 540-pos[1]*108], "step": f["numberSpacing"]*scale[0]*108,
                "count": len(root["m_Children"]), "left": f["align"] == 1,
                "plus": glyph(f["plusIconSprite"]["guid"], glyph_scale) if f["plusIconSprite"].get("guid") else None,
                "digits": [glyph(g["guid"], glyph_scale) for g in f["numberSprites"]]}

    score, life, combo, judge = [view(n) for n in ("ScoreView", "LifeView", "ComboView", "JudgmentView")]
    add_root = score["addScoreRoot"]["fileID"]
    runtime_scales[add_root] = {"x": 1, "y": 1, "z": 1}
    runtime_positions[add_root] = {**by_id[add_root]["fields"]["m_LocalPosition"], "x": 0}
    gauge = by_id[score["scoreGauge"]["fileID"]]["fields"]
    gauge_pos = transforms[gauge["m_GameObject"]["fileID"]]["fields"]["m_LocalPosition"]
    for letter, rate in (("C", .45), ("B", .6), ("A", .75), ("S", .9)):
        tid = score[f"gaugeRank{letter}"]["fileID"]
        runtime_positions[tid] = {**by_id[tid]["fields"]["m_LocalPosition"],
                                  "x": gauge_pos["x"] + rate*gauge["m_Size"]["x"]}
    static_names = {"ScoreBase", "ScoreGaugeBase", "RankBase", "LifeBackground",
                    "PauseButton", "C", "B", "A", "S", "lineC", "lineB", "lineS", "Line"}
    static = []
    for o in objects:
        if o["kind"] != "SpriteRenderer":
            continue
        name = by_id[o["fields"]["m_GameObject"]["fileID"]]["fields"]["m_Name"]
        if name not in static_names and o["id"] != 212958492976087124:
            continue
        w = widget(o["id"])
        # The prefab has other pause buttons, including the editor's overlay.
        if w["name"] in static_names and (w["name"] != "PauseButton" or o["id"] == life["pauseButton"]["fileID"]):
            static.append(w)
        if o["id"] == 212958492976087124:  # LifeView's authored heart Icon
            static.append(w)

    combo_transform = by_id[combo["numberRoot"]["fileID"]]["fields"]
    pos, scale = transform(combo_transform["m_GameObject"]["fileID"])
    renderer = by_id[judge["spriteRenderer"]["fileID"]]["fields"]
    judge_pos, judge_scale = transform(renderer["m_GameObject"]["fileID"])
    judge_origin = [960+judge_pos[0]*108, 540-judge_pos[1]*108]
    return {"static": sorted(static, key=lambda w: w["order"]),
            "score_gauge": widget(score["scoreGauge"]["fileID"]),
            "life_gauge": widget(life["lifeDefaultGauge"]["fileID"]),
            "life_damage_gauge": widget(life["lifeDamageGauge"]["fileID"]),
            "life_bases": [widget(next(o["id"] for o in objects if o["kind"] == "SpriteRenderer"
                                       and by_id[o["fields"]["m_GameObject"]["fileID"]]["fields"]["m_Name"] == n))
                           for n in ("LifeBaseDefault", "LifeBaseDamage")],
            "rank": widget(score["rankSpriteRenderer"]["fileID"]),
            "rank_text": widget(score["rankTextSpriteRenderer"]["fileID"]),
            "ranks": [glyph(g["guid"], (1., 1.)) for g in score["rankSprites"]],
            "score_numbers": [number(score[n]["fileID"]) for n in ("outlineNumber", "number")],
            "add_score_numbers": [number(score[n]["fileID"]) for n in ("addScoreOutlineNumber", "addScoreNumber")],
            "life_numbers": [number(life[n]["fileID"]) for n in ("outlineNumber", "number")],
            "combo_label": widget(combo["comboSpriteRenderer"]["fileID"]),
            "combo_ap_label": combo["allPerfectComboTextSprite"]["guid"],
            "combo_origin": [960+pos[0]*108, 540-pos[1]*108], "combo_step": .95*scale[0]*108,
            "combo_digits": [glyph(g["guid"], scale) for g in combo["numberSprites"]],
            "combo_ap_digits": [glyph(g["guid"], scale) for g in combo["allPerfectNumberSprites"]],
            "judges": [glyph(judge[n]["guid"], judge_scale, judge_origin)
                       for n in ("justPerfect", "perfect", "great", "good", "bad", "miss", "auto")]}
