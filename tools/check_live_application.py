#!/usr/bin/env python3
"""Verify live C ABI input, results, restart and audio clock ownership."""
import json
import tempfile
from native_abi import ROOT, C, Frame, Event, api
from live_fixture import create
from live_hud_assertions import score_number


def run(music):
    with tempfile.TemporaryDirectory(prefix="opensekai-live-abi-") as data:
        path = create(data, music)
        app = api.create(str(ROOT).encode(), data.encode())
        assert app
        try:
            def event(kind, **kwargs):
                e = Event(kind=kind, **kwargs)
                return api.event(app, C.byref(e))
            def frame():
                value = Frame()
                assert api.frame(app, 0, 0, C.byref(value))
                texts = [d.text.decode() for d in value.draws[:value.count] if d.kind == 1 and d.text and d.rgba & 255]
                return value, texts
            def tick(dt):
                event(15, delta=dt)
            def key(letter, down=True):
                event(5 if down else 12, key=ord(letter) if isinstance(letter, str) else letter)
            def clock(time):
                event(13, x=time, key=int(time == 7))
                tick(0)

            assert event(11, key=4 if music else 3)
            first, _ = frame()
            epoch = first.audio_epoch
            assert first.reserved & 1
            tick(100)
            event(16)
            if music:
                assert first.audio_state == 2 and first.audio_frames == 336000
                assert max(abs(first.audio_pcm[i]) for i in range(2000)) > .001
                tick(2.1)
                assert frame()[0].audio_state == 1
                clock(.5)  # filler: audio .5 sec means chart time 0
                tick(100)  # a renderer stall cannot advance the music clock
                assert score_number(frame()[0]) == "00000000"
                clock(1.5)
                assert score_number(frame()[0]) == "00179257"
                key(27)
                clock(7)
                assert frame()[0].audio_state == 2
                assert not (path / "native-last-result.json").exists()
                key(13)
                clock(4.5)
                clock(7)
            else:
                assert first.audio_state == 0
                tick(3)
                key("a"); key("a", False)
                tick(1)
                key("f")
                event(14)  # loss of window focus pauses and releases keys
                tick(100)
                assert "已暂停" in frame()[1]
                key(13); key("f")
                for _ in range(4): tick(.25)
                key("f", False)
                tick(1)
                key("k"); key("k", False); key("i")
                tick(2)
            state, texts = frame()
            assert not state.reserved & 2 and "演奏结果" not in texts
            assert state.audio_state == (1 if music else 0)
            result = json.loads((path / "native-last-result.json").read_text())["result"]
            assert result["total"] == 7 and result["life"] == 1000
            assert result["counts"][6 if music else 0] == 7
            assert result["practice_without_music"] != music
            tick(7.35)
            state, texts = frame()
            assert state.audio_state == 0 and not state.reserved & 2 and "演奏完成！" in texts
            assert event(1, key=1, x=1360, y=1005)  # Intro cannot accidentally retry.
            assert frame()[0].audio_epoch == epoch
            tick(4)
            state, texts = frame()
            assert state.audio_state == 0 and state.reserved & 2 and "演奏结果" in texts
            if music:
                key("r")
            else:
                assert event(1, key=1, x=1360, y=1005)  # visible result Retry button
            retry, texts = frame()
            assert retry.audio_epoch != epoch and "演奏结果" not in texts
            # HUD numbers are now original atlas sprites. Retry must render
            # all eight score digits as zero, independently of the text runs.
            assert score_number(retry) == "00000000"
            if music:
                key(27); key(8)
            else:
                tick(12); tick(11.25)
                assert "演奏结果" in frame()[1]
                assert event(1, key=1, x=1693, y=1005)  # visible result Return button
            assert "本地谱面" in frame()[1]
            assert frame()[0].audio_state == 0
        finally:
            api.destroy(app)


run(False)
run(True)
print("Live C ABI: manual tap/hold/release/flick, focus pause, result, retry, return, decoded PCM, filler, sample clock, audio pause/stop OK")
