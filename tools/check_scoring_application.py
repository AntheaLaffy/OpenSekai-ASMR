#!/usr/bin/env python3
"""Check varied real-chart HUD gains and exact normalization through the C ABI."""
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
from native_abi import ROOT, C, Event, Frame, api
from live_hud_assertions import gain_number, score_number


for chart in ("0001_01-master", "0114_01-master"):
    source = ROOT / "content/library" / chart
    manifest = json.loads((source / "manifest.json").read_text())
    with tempfile.TemporaryDirectory(prefix="ojsk-scoring-") as temporary:
        entry = Path(temporary) / chart
        entry.mkdir()
        for filename in ("manifest.json", manifest["scoreFileName"], manifest["audioFileName"], manifest["jacketFileName"]):
            shutil.copyfile(source / filename, entry / filename)
        duration = float(subprocess.check_output([
            "ffprobe", "-v", "error", "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1", str(entry / manifest["audioFileName"])]))
        app = api.create(str(ROOT).encode(), temporary.encode())
        assert app
        try:
            def event(kind, **kwargs):
                value = Event(kind=kind, **kwargs)
                return api.event(app, C.byref(value))

            def frame():
                value = Frame()
                assert api.frame(app, 0, 0, C.byref(value))
                return value

            def clock(seconds):
                event(13, x=seconds)
                event(15, delta=0)

            assert event(11, key=10, text=manifest["id"].encode())
            assert event(11, key=4)
            assert frame().reserved & 1
            event(16)
            event(15, delta=2.1)
            gains = []
            samples = []
            previous = 0
            for step in range(401):
                time = manifest["fillerSec"] + step * .025
                clock(time)
                score = int(score_number(frame()))
                assert score >= previous
                if score > previous:
                    clock(time + .005)
                    value = gain_number(frame())
                    if value:
                        gains.append(int(value))
                        samples.append({"music_seconds": time + .005, "gain": int(value), "score": int(score_number(frame()))})
                previous = score
            assert len(set(gains)) >= 4, (chart, gains)
            clock(duration + 1)
            saved = json.loads((entry / "native-last-result.json").read_text())
            result = saved["result"]
            assert saved["scoringRule"] == "ojsk-dynamic-v1"
            assert result["score"] == 1000000
            assert result["total"] == result["counts"][6] == manifest["officialNoteCount"]
            assert result["max_combo"] == result["total"]
            examples = list({sample["gain"]: sample for sample in reversed(samples)}.values())[-10:][::-1]
            print(json.dumps({"chart": chart, "hud_examples": examples, "final_score": result["score"]}), flush=True)
        finally:
            api.destroy(app)

print("Scoring C ABI: two official MASTER charts, authored popup digits, varied gains, exact million, Auto counts and rule persistence OK")
