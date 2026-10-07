"""Compile the particle modules actually used by the original result prefabs.

Unsupported enabled modules fail compilation instead of disappearing from the
effect. Original properties remain the authority for later renderer expansion.
"""


def compile_result_particles(objects, nodes, go_nodes, tracks, asset, resolve, curve):
    def minmax(value):
        assert value["minMaxState"] in (0, 1, 2, 3)
        return {"mode": value["minMaxState"], "scalar": value["scalar"], "minimum": value["minScalar"],
                "lower": curve(value["minCurve"]), "upper": curve(value["maxCurve"])}

    def gradient(value):
        assert value["m_Mode"] in (0, 1) and value.get("m_ColorSpace", -1) in (-1, 0)
        return {"step": value["m_Mode"] == 1,
                "rgb": [[value[f"ctime{i}"]/65535] + [value[f"key{i}"][c] for c in "rgb"]
                        for i in range(value["m_NumColorKeys"])],
                "alpha": [[value[f"atime{i}"]/65535, value[f"key{i}"]["a"]]
                          for i in range(value["m_NumAlphaKeys"])]}

    def color(value):
        assert value["minMaxState"] in range(5)
        return {"mode": value["minMaxState"],
                "minimum": [value["minColor"][c] for c in "rgba"],
                "maximum": [value["maxColor"][c] for c in "rgba"],
                "lower": gradient(value["minGradient"]), "upper": gradient(value["maxGradient"])}

    def constant(value):
        assert value["minMaxState"] == 0
        return value["scalar"]

    def vector(value):
        return [value[c] for c in "xyz"]

    result = []
    supported = {"InitialModule", "ShapeModule", "EmissionModule", "SizeModule", "ColorModule", "ClampVelocityModule"}
    for obj in objects:
        if obj["kind"] != "ParticleSystem":
            continue
        f = obj["fields"]
        node = go_nodes[f["m_GameObject"]["fileID"]]
        name = nodes[node]["name"]
        enabled = {k for k,v in f.items() if isinstance(v, dict) and v.get("enabled")}
        assert enabled <= supported, (name, enabled-supported)
        assert f["moveWithTransform"] == 0 and f["scalingMode"] == 1
        assert f["simulationSpeed"] == 1 and f["playOnAwake"] == 1 and not f["prewarm"]
        main, shape, emission = f["InitialModule"], f["ShapeModule"], f["EmissionModule"]
        assert not main["size3D"] and not main["rotation3D"]
        assert constant(main["gravityModifier"]) == 0 and vector(main["customEmitterVelocity"]) == [0,0,0]
        assert main["randomizeRotationDirection"] == 0
        assert shape["type"] in (5,10) and vector(shape["m_Rotation"]) == [0,0,0]
        assert shape["radius"]["mode"] == 0 and shape["arc"]["mode"] in (0,1) and shape["arc"]["value"] == 360
        assert not any(shape[k] for k in ("randomDirectionAmount", "sphericalDirectionAmount", "alignToDirection"))
        assert constant(emission["rateOverDistance"]) == 0
        assert not f["SizeModule"]["separateAxes"]
        renderer = next(o["fields"] for o in objects if o["kind"] == "ParticleSystemRenderer"
                        and o["fields"]["m_GameObject"] == f["m_GameObject"])
        assert renderer["m_RenderMode"] == renderer["m_RenderAlignment"] == renderer["m_SortMode"] == 0
        assert renderer["m_SortingOrder"] == 0 and vector(renderer["m_Pivot"]) == [0,0,0]
        assert len(renderer["m_Materials"]) == 1
        material = asset(resolve(renderer["m_Materials"][0]))[0]["fields"]
        shader_path = resolve(material["m_Shader"])
        assert shader_path.endswith("shader/particles/particle add.shader"), shader_path
        asset(shader_path)
        texture = material["m_SavedProperties"]["m_TexEnvs"]["_MainTex"]
        texture_path = resolve(texture["m_Texture"])
        asset(texture_path)
        # These result emitters have static transforms. They become active once
        # through GameObject curves; a child's clock starts with its parent.
        position, active_from, parent = [0.,0.,0.], 0., node
        while parent is not None:
            n = nodes[parent]
            assert n["rotation"] == [0,0,0,1] and n["scale"] == [1,1,1], name
            position = [a+b for a,b in zip(position, n["position"])]
            for track in tracks:
                if track["node"] != parent:
                    continue
                assert track["property"] == "active", (name, track["property"])
                keys = track["keys"]
                assert all(k["value"] in (0,1) for k in keys)
                assert all(a["value"] <= b["value"] for a,b in zip(keys, keys[1:]))
                for a,b in zip(keys, keys[1:]):
                    assert a["value"] == b["value"] or a["step"] or b["step_in"], name
                enabled_at = next((k["time"] for k in keys if k["value"]), None)
                active_from = None if active_from is None or enabled_at is None else max(active_from, enabled_at)
                break
            else:
                if not n["active"]:
                    active_from = None
            parent = n["parent"]
        bursts = []
        for burst in emission["m_Bursts"]:
            bursts.append({"time": burst["time"], "count": minmax(burst["countCurve"]),
                           "cycles": burst["cycleCount"], "interval": burst["repeatInterval"],
                           "probability": burst["probability"]})
        limit = None
        clamp = f["ClampVelocityModule"]
        if clamp["enabled"]:
            assert not clamp["separateAxis"] and constant(clamp["drag"]) == 0
            limit = {"magnitude": minmax(clamp["magnitude"]), "dampen": clamp["dampen"]}
        result.append({"name": name, "node": node, "active_from": active_from, "position": position,
                       "duration": f["lengthInSec"], "looping": bool(f["looping"]),
                       "delay": constant(f["startDelay"]), "capacity": main["maxNumParticles"],
                       "auto_seed": bool(f["autoRandomSeed"]), "seed": f["randomSeed"],
                       "lifetime": minmax(main["startLifetime"]), "speed": minmax(main["startSpeed"]),
                       "size": minmax(main["startSize"]), "rotation": minmax(main["startRotation"]),
                       "start_color": color(main["startColor"]), "color": color(f["ColorModule"]["gradient"]),
                       "size_over_life": minmax(f["SizeModule"]["curve"]), "limit": limit,
                       "rate": constant(emission["rateOverTime"]), "bursts": bursts,
                       "shape": {"kind": shape["type"], "position": vector(shape["m_Position"]),
                                 "scale": vector(shape["m_Scale"]), "radius": shape["radius"]["value"],
                                 "thickness": shape["radiusThickness"], "arc_mode": shape["arc"]["mode"],
                                 "arc_speed": constant(shape["arc"]["speed"]), "arc_spread": shape["arc"]["spread"],
                                 "random_position": shape["randomPositionAmount"]},
                       "texture": texture_path, "texture_transform": [texture["m_Scale"][c] for c in "xy"] + [texture["m_Offset"][c] for c in "xy"],
                       "enabled": bool(renderer["m_Enabled"]), "min_size": renderer["m_MinParticleSize"],
                       "max_size": renderer["m_MaxParticleSize"]})
    return result
