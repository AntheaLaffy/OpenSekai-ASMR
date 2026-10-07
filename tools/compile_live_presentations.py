#!/usr/bin/env python3
"""Resolve result prefab hierarchies and their actual Animator default clips."""
import hashlib
from pathlib import Path
import struct
from compile_particles import compile_result_particles

ROOT = Path(__file__).resolve().parents[1]
PREFIX = "Assets/Sekai/assetbundle/resources/tutorial/effect_asset/live/result/default/"
RESULTS = {"all_perfect": "fx_result_all_perfect_v2", "full_combo": "fx_result_full_combo_v2",
           "clear": "fx_result_clear_v2", "finish": "fx_result_finish", "failed": "fx_result_failed_v2"}


def curve(data, axis=None):
    keys = []
    for key in data["m_Curve"]:
        def channel(name, default):
            value = key.get(name, default)
            return value[axis] if axis is not None and isinstance(value, dict) else value
        incoming, outgoing = channel("inSlope", 0), channel("outSlope", 0)
        step = isinstance(outgoing, str)
        mode = key.get("weightedMode", 0)
        keys.append({"time": key["time"], "value": channel("value", 0),
                     "incoming": 0 if isinstance(incoming, str) else incoming,
                     "outgoing": 0 if isinstance(outgoing, str) else outgoing,
                     "in_weight": channel("inWeight", 1 / 3) if mode & 1 else 1 / 3,
                     "out_weight": channel("outWeight", 1 / 3) if mode & 2 else 1 / 3,
                     "step": step, "step_in": isinstance(incoming, str)})
    return keys


def compile_presentations(index):
    sources = {}

    def asset(path):
        row = index["assets"][path]
        if "sha256" in row:
            assert hashlib.sha256((ROOT / path).read_bytes()).hexdigest() == row["sha256"], path
            sources[path] = row["sha256"]
        return row.get("objects", [])

    def resolve(reference):
        return index["guid_paths"][reference["guid"]]

    results = {}
    for name, prefab in RESULTS.items():
        path = PREFIX + prefab + ".prefab"
        objects = asset(path)
        by_id = {o["id"]: o for o in objects}
        transforms = {o["id"]: o["fields"] for o in objects if o["kind"] == "Transform"}
        ordered = []

        def append(tid):
            ordered.append(tid)
            for child in transforms[tid]["m_Children"]:
                append(child["fileID"])

        for tid, transform in transforms.items():
            if transform["m_Father"]["fileID"] == 0:
                append(tid)
        node_ids = {tid: i for i, tid in enumerate(ordered)}
        go_nodes = {transforms[tid]["m_GameObject"]["fileID"]: i for tid, i in node_ids.items()}
        nodes = []
        for tid in ordered:
            t = transforms[tid]
            go = by_id[t["m_GameObject"]["fileID"]]["fields"]
            nodes.append({"name": go["m_Name"], "parent": node_ids.get(t["m_Father"]["fileID"]),
                          "position": [t["m_LocalPosition"][a] for a in "xyz"],
                          "rotation": [t["m_LocalRotation"][a] for a in "xyzw"],
                          "scale": [t["m_LocalScale"][a] for a in "xyz"], "active": bool(go["m_IsActive"])})
        sprites = []
        for obj in objects:
            if obj["kind"] != "SpriteRenderer":
                continue
            f = obj["fields"]
            if not f["m_Sprite"].get("guid"):
                continue
            sprite = asset(resolve(f["m_Sprite"]))[0]["fields"]
            assert len(f["m_Materials"]) == 1
            material_ref = f["m_Materials"][0]
            additive = False
            if material_ref.get("guid") != "0000000000000000f000000000000000":
                material = asset(resolve(material_ref))[0]["fields"]
                shader = resolve(material["m_Shader"])
                assert shader.endswith("shader/particles/particle add.shader"), shader
                asset(shader)
                additive = True
            sprites.append({"node": go_nodes[f["m_GameObject"]["fileID"]], "guid": f["m_Sprite"]["guid"],
                            "color": [f["m_Color"][a] for a in "rgba"],
                            "size": [sprite["m_Rect"][a] / sprite["m_PixelsToUnits"] for a in ("width", "height")],
                            "pivot": [sprite["m_Pivot"][a] for a in "xy"],
                            "order": f["m_SortingOrder"], "enabled": bool(f["m_Enabled"]), "additive": additive})
        meshes = []
        for obj in objects:
            if obj["kind"] != "MeshRenderer":
                continue
            f = obj["fields"]
            mesh_filter = next(o["fields"] for o in objects if o["kind"] == "MeshFilter"
                               and o["fields"]["m_GameObject"] == f["m_GameObject"])
            mesh = asset(resolve(mesh_filter["m_Mesh"]))[0]["fields"]
            vertices = mesh["m_VertexData"]
            channels = vertices["m_Channels"]
            assert channels[0] == {"stream": 0, "offset": 0, "format": 0, "dimension": 3}
            assert channels[4] == {"stream": 0, "offset": 40, "format": 0, "dimension": 2}
            raw = bytes.fromhex(vertices["_typelessdata"])
            assert len(raw) == vertices["m_VertexCount"] * 48
            pos = [struct.unpack_from("<3f", raw, i*48) for i in range(vertices["m_VertexCount"])]
            uv = [struct.unpack_from("<2f", raw, i*48+40) for i in range(vertices["m_VertexCount"])]
            indices = bytes.fromhex(mesh["m_IndexBuffer"])
            assert mesh["m_IndexFormat"] == 0 and len(indices) % 12 == 0
            quads = []
            for a,b,c,d,e,g in struct.iter_unpack("<6H", indices):
                ids = {a,b,c,d,e,g}
                assert len(ids) == 4
                us, vs = sorted({uv[i][0] for i in ids}), sorted({uv[i][1] for i in ids})
                assert len(us) == len(vs) == 2
                lookup = {uv[i]: i for i in ids}
                order = [lookup[(u,v)] for u,v in [(us[0],vs[0]),(us[1],vs[0]),(us[1],vs[1]),(us[0],vs[1])]]
                # Preserve the original triangle diagonal, as UV interpolation
                # on an annular trapezoid changes when that diagonal flips.
                diagonal = {a,b,c} & {d,e,g}
                if {order[0],order[2]} != diagonal:
                    order = [order[1],order[0],order[3],order[2]]
                    us.reverse()
                assert {order[0],order[2]} == diagonal
                quads.append({"vertices": [pos[i] for i in order], "uv": [us[0],vs[0],us[1],vs[1]]})
            assert len(f["m_Materials"]) == 1 and f["m_SortingOrder"] == 0
            material = asset(resolve(f["m_Materials"][0]))[0]["fields"]
            shader = resolve(material["m_Shader"])
            asset(shader)
            assert 'Shader "Sandbox/LegacyShaders/Particles/Additive"' in (ROOT / shader).read_text()
            props = material["m_SavedProperties"]
            tex = props["m_TexEnvs"]["_MainTex"]
            texture = resolve(tex["m_Texture"])
            asset(texture)
            meshes.append({"node": go_nodes[f["m_GameObject"]["fileID"]], "texture": texture,
                           "enabled": bool(f["m_Enabled"]), "quads": quads,
                           "tint": [props["m_Colors"]["_TintColor"][a] for a in "rgba"],
                           "texture_transform": [tex["m_Scale"][a] for a in "xy"] + [tex["m_Offset"][a] for a in "xy"]})
        animator = next(o["fields"] for o in objects if o["kind"] == "Animator")
        controller = asset(resolve(animator["m_Controller"]))
        machine = next(o["fields"] for o in controller if o["kind"] == "AnimatorStateMachine")
        state = next(o["fields"] for o in controller if o["id"] == machine["m_DefaultState"]["fileID"])
        assert state["m_Speed"] == 1 and not state["m_Transitions"]
        clip_path = resolve(state["m_Motion"])
        clip = asset(clip_path)[0]["fields"]
        assert not clip["m_RotationCurves"] and not clip["m_EulerCurves"] and not clip["m_PPtrCurves"]
        animator_node = go_nodes[animator["m_GameObject"]["fileID"]]
        paths = {"": animator_node}
        for i, node in enumerate(nodes):
            parts, current = [], i
            while current is not None and current != animator_node:
                parts.append(nodes[current]["name"])
                current = nodes[current]["parent"]
            if current == animator_node:
                paths["/".join(reversed(parts))] = i
        tracks = []
        unbound = []
        for field, prop in (("m_PositionCurves", "position"), ("m_ScaleCurves", "scale")):
            for track in clip[field]:
                if track["path"] not in paths:
                    unbound.append({"path": track["path"], "property": prop})
                    continue
                for channel, axis in enumerate("xyz"):
                    tracks.append({"node": paths[track["path"]], "property": prop, "channel": channel,
                                   "keys": curve(track["curve"], axis)})
        for track in clip["m_FloatCurves"]:
            attribute = track["attribute"]
            if track["path"] not in paths:
                unbound.append({"path": track["path"], "property": attribute})
                continue
            if attribute == "m_IsActive":
                prop, channel = "active", 0
            elif attribute.startswith("m_Color."):
                prop, channel = "color", "rgba".index(attribute[-1])
            elif attribute.startswith("material._TintColor."):
                prop, channel = "tint", "rgba".index(attribute[-1])
            elif attribute.startswith("material._MainTex_ST."):
                prop, channel = "texture_transform", "xyzw".index(attribute[-1])
            else:
                raise ValueError(f"Unmapped animation property: {clip_path}: {attribute}")
            tracks.append({"node": paths[track["path"]], "property": prop, "channel": channel,
                           "keys": curve(track["curve"])})
        events = []
        for event in clip["m_Events"]:
            assert event["functionName"] in ("PLaySE", "Play"), event
            events.append({"time": event["time"], "cue": event["data"]})
        particles = compile_result_particles(objects, nodes, go_nodes, tracks, asset, resolve, curve)
        results[name] = {"source": path, "clip": clip_path, "duration": clip["m_AnimationClipSettings"]["m_StopTime"],
                         "nodes": nodes, "sprites": sorted(sprites, key=lambda s: s["order"]), "meshes": meshes,
                         "tracks": tracks, "events": events, "particles": particles, "unbound_source_tracks": unbound,
                         "particle_systems": sum(o["kind"] == "ParticleSystem" for o in objects),
                         "mesh_renderers": sum(o["kind"] == "MeshRenderer" for o in objects)}
    return {"results": results, "sources": sources}
