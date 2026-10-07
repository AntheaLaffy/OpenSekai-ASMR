#!/usr/bin/env python3
"""Drive real Rust sound commands using source media, including polyphonic cases."""
import json
import tempfile
from native_abi import ROOT, C, Frame, Event, api
from live_fixture import create
from live_hud_assertions import life_number

bank = json.loads((ROOT / "resources/opensekai/soundbank.json").read_text())
names = [cue["name"] for cue in bank["cues"] for _ in cue["clips"]]


class App:
    def __init__(self, path):
        self.handle = api.create(str(ROOT).encode(), str(path).encode())
        assert self.handle

    def event(self, kind, **kwargs):
        e = Event(kind=kind, **kwargs)
        return api.event(self.handle, C.byref(e))

    def frame(self):
        f = Frame()
        assert api.frame(self.handle, 0, 0, C.byref(f))
        commands = [(c.action, c.voice, names[c.clip] if c.action != 2 else "stop", c.clip)
                    for c in f.sound_commands[:f.sound_command_count]]
        return f, commands

    def key(self, key, down=True):
        self.event(5 if down else 12, key=ord(key) if isinstance(key, str) else key)

    def tick(self, dt):
        self.event(15, delta=dt)

    def begin(self, auto=False):
        assert self.event(11, key=4 if auto else 3)
        f, commands = self.frame()
        assert f.sound_count == 18 and f.sound_state == 2 and not commands
        assert all(s.frames > 0 and bool(s.pcm) for s in f.sounds[:f.sound_count])
        self.event(16)
        return f.audio_epoch

    def close(self):
        api.destroy(self.handle)


with tempfile.TemporaryDirectory(prefix="opensekai-sound-manual-") as data:
    path = create(data)
    app = App(data)
    try:
        epoch = app.begin()
        app.tick(3); app.key("a")
        assert [c[2] for c in app.frame()[1]] == ["se_live_perfect"]
        app.key("a", False)
        assert not app.frame()[1]  # frame rebuilding cannot replay a hit
        app.tick(1); app.key("f")
        _, commands = app.frame()
        assert [(c[0], c[2]) for c in commands] == [(0, "se_live_perfect"), (1, "se_live_long")]
        app.tick(.25)
        assert not app.frame()[1]  # generated hold combo has no tap SE
        app.key(27)
        f, commands = app.frame()
        assert f.sound_state == 2 and [c[0] for c in commands] == [2]
        app.key(13); app.key("f")
        f, commands = app.frame()
        assert f.sound_state == 1 and any(c[0] == 1 for c in commands)
        for _ in range(3):
            app.tick(.25); app.frame()
        app.key("f", False)
        assert any(c[2] == "se_live_perfect" for c in app.frame()[1])
        app.tick(1); app.key("k"); app.key("k", False); app.key("i")
        assert any(c[2] == "se_live_flick" for c in app.frame()[1])
        app.tick(0)  # Last note starts the separate end presentation clock.
        assert not app.frame()[1]
        app.tick(.99)
        assert not app.frame()[1]
        app.tick(.01)
        f, commands = app.frame()
        assert any(c[2] == "se_live_all_perfect" for c in commands)
        assert f.sound_state == 1 and f.audio_state == 0
        assert "PERFECT" not in [d.text.decode() for d in f.draws[:f.count] if d.kind == 1 and d.text]
        assert not app.frame()[1]
        app.key("r")
        assert app.frame()[0].audio_epoch == epoch  # Cannot skip the ending.
        app.tick(6)
        assert not app.frame()[0].reserved & 2  # The result page has its own entrance.
        app.tick(4.2)
        assert app.frame()[0].reserved & 2
        app.key("r")
        f, commands = app.frame()
        assert f.audio_epoch != epoch and not commands
        app.key(27); app.key(8)
        assert app.frame()[0].sound_state == 0
    finally:
        app.close()

for count in (2, 3):
    with tempfile.TemporaryDirectory(prefix="opensekai-sound-pair-") as data:
        path = create(data)
        score = json.loads((path / "score.json").read_text())
        first = score["NoteList"][0]
        score["NoteList"] = [dict(first, id=i+1, laneStart=i*4, laneEnd=i*4+1) for i in range(count)]
        (path / "score.json").write_text(json.dumps(score))
        app = App(data)
        try:
            app.begin(True); app.tick(3)
            commands = app.frame()[1]
            assert len(commands) == (1 if count == 2 else 3), commands
        finally:
            app.close()

with tempfile.TemporaryDirectory(prefix="opensekai-sound-variants-") as data:
    path = create(data)
    score = json.loads((path / "score.json").read_text())
    first = score["NoteList"][0]
    score["NoteList"] = [dict(first, id=i+1, category=4, type=1, ticks=960+i*960) for i in range(2)]
    (path / "score.json").write_text(json.dumps(score))
    app = App(data)
    try:
        app.begin(True); app.tick(3); first_clip = app.frame()[1][0][3]
        app.tick(1); second_clip = app.frame()[1][0][3]
        assert first_clip != second_clip and names[first_clip] == names[second_clip] == "se_live_trace_critical"
        app.tick(3)
        assert not app.frame()[1]  # source default: no Auto result animation/SE
    finally:
        app.close()

with tempfile.TemporaryDirectory(prefix="opensekai-life-zero-") as data:
    path = create(data, music=True, music_seconds=14)
    score = json.loads((path / "score.json").read_text())
    first = score["NoteList"][0]
    score["NoteList"] = [dict(first, id=i, ticks=i*480) for i in range(1, 21)]
    score["MusicScoreTicksMax"] = 20*480
    (path / "score.json").write_text(json.dumps(score))
    app = App(data)
    try:
        app.begin(); app.tick(2.1)
        app.event(13, x=7.7); app.tick(0)  # Chart 7.2: fourteen Misses, Life zero.
        frame, commands = app.frame()
        assert frame.audio_state == 1 and not frame.reserved & 2 and not commands
        assert life_number(frame) == "0"
        assert not (path / "native-last-result.json").exists()
        app.event(13, x=8.0); app.tick(0); app.key("a"); app.key("a", False)
        assert [c[2] for c in app.frame()[1]] == ["se_live_perfect"]
        app.event(13, x=10.7); app.tick(0)
        frame, commands = app.frame()
        assert frame.audio_state == 1 and not commands  # Music tail survives the last note.
        result = json.loads((path / "native-last-result.json").read_text())["result"]
        assert result["life"] == 0 and result["counts"] == [1,0,0,0,0,19,0]
        assert result["score"] == 40404  # Dynamic rule: missed streak has no combo bonus.
        app.tick(1.25)
        assert not app.frame()[1]
        app.tick(.02)
        frame, commands = app.frame()
        assert [c[2] for c in commands] == ["se_live_finish"]
        assert any(d.kind == 3 for d in frame.draws[:frame.count])
        assert not app.frame()[1]
        app.tick(5.74)
        frame, commands = app.frame()
        assert not frame.reserved & 2 and frame.audio_state == 0 and not commands
        app.tick(4.2)
        assert app.frame()[0].reserved & 2  # Result data finishes entering before controls unlock.
    finally:
        app.close()

print("Sound C ABI: source bank, hit/flick, silent combos, hold/pause/retry, AP timeline, pairs/chords, critical trace, Life zero continuation, FINISH animation event and music tail OK")
