#!/usr/bin/env python3
"""Resolve the restored LiveSoundPlayer bindings and animation event cues."""
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1] / "resources/opensekai"


def compile_bank():
    manifest = json.loads((ROOT / "manifest.json").read_text())
    guid_map = {row["unity_guid"]: row for row in manifest["files"] if "unity_guid" in row}
    prefab = "upstream/Assets/Features/live/view/FrontUIView.prefab"
    data = (ROOT / prefab).read_bytes()
    cues = []
    for name, block in re.findall(r"  - cueName: (\S+)\n(.*?)(?=  - cueName:|  masterVolume:)", data.decode(), re.S):
        clips = []
        for guid in re.findall(r"guid: ([0-9a-f]{32})", block):
            row = guid_map[guid]
            assert row["path"].endswith(".wav"), row
            assert hashlib.sha256((ROOT / row["path"]).read_bytes()).hexdigest() == row["sha256"]
            clips.append({"path": row["path"], "guid": guid, "sha256": row["sha256"]})
        assert clips, name
        cues.append({"name": name, "gain": float(re.search(r"    volume: ([0-9.]+)", block)[1]), "clips": clips})
    assert len(cues) == 16
    animation_events = []
    for row in manifest["files"]:
        if not row["path"].endswith(".anim"):
            continue
        clip = (ROOT / row["path"]).read_bytes()
        assert hashlib.sha256(clip).hexdigest() == row["sha256"]
        events = clip.decode().partition("  m_Events:")[2]
        for time, function, name in re.findall(r"  - time: (\S+)\n    functionName: (\S+)\n    data: (\S+)", events):
            assert function in ("PLaySE", "Play"), (row["path"], function)
            animation_events.append({"clip": row["path"], "sha256": row["sha256"],
                                     "time": float(time), "function": function, "cue": name})
            if any(cue["name"] == name for cue in cues):
                continue
            # Named default SE are restored as WAVs. Ambiguous names must not
            # pick an arbitrary skin or numbered ACB clip.
            matches = [r for r in manifest["files"] if Path(r["path"]).name == name + ".wav"]
            assert len(matches) == 1, (name, matches)
            wav = matches[0]
            assert hashlib.sha256((ROOT / wav["path"]).read_bytes()).hexdigest() == wav["sha256"]
            cues.append({"name": name, "gain": 1.0,
                         "clips": [{"path": wav["path"], "guid": wav["unity_guid"], "sha256": wav["sha256"]}]})
    result = {"schema": 1, "source_commit": manifest["source"]["commit"], "source_prefab": prefab,
              "source_sha256": hashlib.sha256(data).hexdigest(), "animation_events": animation_events, "cues": cues}
    (ROOT / "soundbank.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"Sound bank: {len(cues)} cues, {sum(len(c['clips']) for c in cues)} original clip bindings")


if __name__ == "__main__":
    compile_bank()
