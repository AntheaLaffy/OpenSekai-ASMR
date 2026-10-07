"""Small, isolated live chart and clearly identified synthetic audio for QA."""
import json
import math
from pathlib import Path
import struct
import wave


def create(root, music=False, music_seconds=7):
    path = Path(root) / "native-live-fixture"
    path.mkdir(parents=True, exist_ok=True)
    manifest = {"formatVersion": 1, "id": "native-live-fixture", "title": "原生演奏测试",
                "scoreTitle": "原生演奏测试 / Native input test", "musicDifficultyType": "easy",
                "playLevel": 1, "audioFileName": "test-tone.wav", "scoreFileName": "score.json",
                "fillerSec": .5 if music else 0}
    def note(i, ticks, lane, **extra):
        return {"id": i, "ticks": ticks, "laneStart": lane, "laneEnd": lane + 1,
                "category": 0, "type": 0, "speedRatio": 1, "previousConnectionId": -1,
                "nextConnectionId": -1, **extra}
    score = {"VersionCode": 1, "MusicScoreTicksMax": 3840,
             "MusicScoreEventDataList": [{"id": 1, "eventType": 0, "ticks": 0, "changeValue": 120}],
             "NoteList": [note(1, 960, 0), note(2, 1920, 3, category=1, nextConnectionId=3),
                          note(3, 2880, 3, category=1, previousConnectionId=2),
                          note(4, 3840, 7, category=3, direction=1)]}
    (path / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False))
    (path / "score.json").write_text(json.dumps(score))
    if music:
        with wave.open(str(path / "test-tone.wav"), "wb") as out:
            out.setnchannels(1)
            out.setsampwidth(2)
            out.setframerate(48000)
            # Low amplitude signal, not music or a replacement for user audio.
            out.writeframes(b"".join(struct.pack("<h", int(300 * math.sin(2 * math.pi * 440 * i / 48000)))
                                     for i in range(music_seconds * 48000)))
    return path
