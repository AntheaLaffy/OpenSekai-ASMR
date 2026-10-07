"""Keep live effect layers and the source camera, rather than flattened tiles."""
from pathlib import Path


def compile_effect_camera(index):
    objects = index["assets"]["Assets/Resources/live/view/FrontUIView.prefab"]["objects"]
    camera = next(o["fields"] for o in objects if o["kind"] == "Camera" and not o["fields"]["orthographic"])
    transform = next(o["fields"] for o in objects if o["kind"] == "Transform"
                     and o["fields"]["m_GameObject"] == camera["m_GameObject"])
    return {"position": [transform["m_LocalPosition"][c] for c in "xyz"],
            "rotation": [transform["m_LocalRotation"][c] for c in "xyzw"],
            "fov": camera["field of view"]}


def compile_hit_styles(index):
    from compile_live_presentations import curve

    def minmax(v):
        assert v["minMaxState"] in range(4)
        return {"mode": v["minMaxState"], "scalar": v["scalar"], "minimum": v["minScalar"],
                "lower": curve(v["minCurve"]), "upper": curve(v["maxCurve"])}

    def gradient(v):
        return {"step": v["m_Mode"] == 1,
                "rgb": [[v[f"ctime{i}"]/65535] + [v[f"key{i}"][c] for c in "rgb"]
                        for i in range(v["m_NumColorKeys"])],
                "alpha": [[v[f"atime{i}"]/65535, v[f"key{i}"]["a"]]
                          for i in range(v["m_NumAlphaKeys"])]}

    def color(v):
        assert v["minMaxState"] in range(5)
        return {"mode": v["minMaxState"], "minimum": [v["minColor"][c] for c in "rgba"],
                "maximum": [v["maxColor"][c] for c in "rgba"],
                "lower": gradient(v["minGradient"]), "upper": gradient(v["maxGradient"])}

    def particle(o, objects, per_lane, span):
        f = o["fields"]
        by_id = {n["id"]: n for n in objects}
        main, emission, shape, uv = [f[k] for k in ("InitialModule", "EmissionModule", "ShapeModule", "UVModule")]
        render = next(n["fields"] for n in objects if n["kind"] == "ParticleSystemRenderer"
                      and n["fields"]["m_GameObject"] == f["m_GameObject"])
        material_ref = render["m_Materials"][0]
        if not material_ref.get("guid"):
            # The untextured legacy lane-base renderer has no material.
            return None
        material = index["assets"][index["guid_paths"][material_ref["guid"]]]["objects"][0]["fields"]
        texture = index["guid_paths"][material["m_SavedProperties"]["m_TexEnvs"]["_MainTex"]["m_Texture"]["guid"]]
        transforms = {n["id"]: n["fields"] for n in objects if n["kind"] == "Transform"}

        def model(tid):
            t = transforms[tid]
            if not by_id[t["m_GameObject"]["fileID"]]["fields"]["m_IsActive"]:
                return None
            x, y, z, w = [t["m_LocalRotation"][c] for c in "xyzw"]
            r = [[1-2*(y*y+z*z), 2*(x*y-z*w), 2*(x*z+y*w)],
                 [2*(x*y+z*w), 1-2*(x*x+z*z), 2*(y*z-x*w)],
                 [2*(x*z-y*w), 2*(y*z+x*w), 1-2*(x*x+y*y)]]
            local = [[row[i]*t["m_LocalScale"][c] for i,c in enumerate("xyz")]
                     + [t["m_LocalPosition"][axis]] for row,axis in zip(r,"xyz")]
            parent_id = t["m_Father"]["fileID"]
            if parent_id not in transforms:
                return local
            parent = model(parent_id)
            if parent is None:
                return None
            return [[sum(parent[i][k]*local[k][j] for k in range(3)) + (parent[i][3] if j == 3 else 0)
                     for j in range(4)] for i in range(3)]

        tid = next(tid for tid,t in transforms.items() if t["m_GameObject"] == f["m_GameObject"])
        matrix = model(tid)
        if matrix is None or not main["enabled"]:
            return None
        size = f["SizeModule"]
        clamp = f["ClampVelocityModule"]
        custom = f["CustomDataModule"]
        streams = bytes.fromhex(render["m_VertexStreams"]) if render["m_UseCustomVertexStreams"] else b""
        if 31 in streams and custom["enabled"]:
            assert custom["mode0"] == 1 and custom["vectorComponentCount0"] >= 1
            blend = minmax(custom["vector0_0"])
        else:
            blend = {"mode": 0, "scalar": 0, "minimum": 0, "lower": [], "upper": []}
        return {"name": by_id[f["m_GameObject"]["fileID"]]["fields"]["m_Name"],
                "per_lane": per_lane, "span": span, "model": matrix,
                "lifetime": minmax(main["startLifetime"]), "speed": minmax(main["startSpeed"]),
                "size": minmax(main["startSize"]),
                "size_y": minmax(main["startSizeY"] if main["size3D"] else main["startSize"]),
                "size_over_life": minmax(size["curve"]) if size["enabled"] else None,
                "size_over_life_y": minmax(size["y"] if size["separateAxes"] else size["curve"])
                                    if size["enabled"] else None,
                "rotation": [minmax(main[k]) for k in ("startRotationX", "startRotationY", "startRotation")],
                "rotation3d": bool(main["rotation3D"]), "pivot": [render["m_Pivot"][c] for c in "xyz"],
                "start_color": color(main["startColor"]),
                "blend": blend,
                "color": color(f["ColorModule"]["gradient"]) if f["ColorModule"]["enabled"] else None,
                "bursts": [{"time": b["time"], "count": minmax(b["countCurve"]),
                            "cycles": b["cycleCount"], "interval": b["repeatInterval"],
                            "probability": b["probability"]} for b in emission["m_Bursts"]],
                "rate": minmax(emission["rateOverTime"]), "looping": bool(f["looping"]),
                "duration": f["lengthInSec"], "sheet": [uv["tilesX"], uv["tilesY"]],
                "frame": minmax(uv["startFrame"]), "sheet_enabled": bool(uv["enabled"]),
                "shape": shape["type"], "shape_enabled": bool(shape["enabled"]),
                "shape_position": [shape["m_Position"][c] for c in "xyz"],
                "shape_scale": [shape["m_Scale"][c] for c in "xyz"],
                "radius": shape["radius"]["value"],
                "order": render["m_SortingOrder"],
                "min_size": render["m_MinParticleSize"], "render_mode": render["m_RenderMode"],
                "gravity": minmax(main["gravityModifier"]),
                "limit": minmax(clamp["magnitude"]) if clamp["enabled"] else None,
                "dampen": clamp["dampen"], "texture": texture}

    prefix = "Assets/Sekai/assetbundle/resources/tutorial/effect_asset/live/tap_effect/1/"
    styles = {}

    def style(name, prefabs):
        layers, sources = [], {}
        for filename, per_lane, span in prefabs:
            path = prefix + filename + ".prefab"
            asset = index["assets"][path]
            sources[path] = asset["sha256"]
            for o in asset["objects"]:
                if o["kind"] == "ParticleSystem":
                    value = particle(o, asset["objects"], per_lane, span)
                    if value is not None:
                        layers.append(value)
        assert layers, name
        styles[name] = {"sources": sources, "layers": layers}

    for name in ("normal", "flick", "long", "critical_normal", "critical_flick", "critical_long"):
        style(name, [(f"fx_note_{name}_aura", True, False), (f"fx_note_{name}_gen", False, False)])
    for name in ("flash", "critical_flash"):
        style(name, [("fx_note_" + ("critical_" if name.startswith("critical") else "") + "flick_flash", False, True)])
    style("lane", [("fx_lane_tap", False, False)])
    style("hold", [("fx_note_hold_aura", False, True), ("fx_note_long_hold_gen", False, False)])
    style("critical_hold", [("fx_note_critical_long_hold_gen_aura", False, True),
                            ("fx_note_critical_long_hold_gen", False, False)])
    return styles
