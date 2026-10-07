#!/usr/bin/env python3
"""Exercise the actual Rust shared library and its C ABI against temporary data."""
import ctypes as C
import json
from pathlib import Path
import tempfile
import wave

from native_abi import ROOT, C, Frame, Event, api

with tempfile.TemporaryDirectory(prefix="opensekai-abi-") as data:
    app=api.create(str(ROOT).encode(),data.encode())
    assert app
    try:
        def frame(english=False):
            result=Frame()
            assert api.frame(app,0,int(english),C.byref(result))==1
            assert (result.width,result.height)==(1920,1080)
            assert 0<result.count<2048
            # Copy before the next ABI call invalidates the borrowed frame.
            return [(d.kind,tuple(d.rect),d.rgba,d.font,d.text.decode() if d.text else "")
                    for d in result.draws[:result.count]]
        def event(kind,**kwargs):
            e=Event(kind=kind,**kwargs)
            return api.event(app,C.byref(e))
        def click(x,y):
            event(1,x=x,y=y,key=1)
            event(2,x=x,y=y,key=1)
        initial=frame()
        assert any(v[4]=="暂无本地谱面" for v in initial)
        assert any(v[:3]==(0,(56.,136.,620.,916.),0x1a1f26ff) for v in initial)
        assert all(Path(api.font_path(app,i).decode()).is_file() for i in (1,2))
        click(1600,50) # original New button
        assert len(list(Path(data).glob("*/manifest.json")))==1
        frame()
        click(900,565) # original title input
        e=Frame()
        assert api.frame(app,0,0,C.byref(e)) and e.wants_text
        event(7,text="测试输入".encode())
        assert any("测试输入" in v[4] for v in frame())
        event(6,text="中文与English 🎵".encode())
        event(5,key=13)
        frame()
        click(850,990)
        manifest_path=next(Path(data).glob("*/manifest.json"))
        manifest=json.loads(manifest_path.read_text())
        assert manifest["title"]=="中文与English 🎵"
        score=json.loads((manifest_path.parent/"score.json").read_text())
        assert [e["eventType"] for e in score["MusicScoreEventDataList"]]==[0,3,1,2]
        frame()
        click(1400,420) # Duplicate
        assert len(list(Path(data).glob("*/manifest.json")))==2
        frame()
        click(1580,420) # Export ZIP
        assert api.take_request(app)==6
        exported=Path(data)/"package.zip"
        event(9,key=6,text=str(exported).encode())
        assert exported.is_file()
        frame()
        click(1790,50) # Import ZIP
        assert api.take_request(app)==1
        event(9,key=1,text=str(exported).encode())
        assert len(list(Path(data).glob("*/manifest.json")))==3
        # Imported copies can have identical manifest IDs. A file dialog must
        # retain the exact folder selected when it opened, even after selection
        # changes while the asynchronous platform dialog is pending.
        frame()
        click(160,390)
        selected=frame()
        target=Path(next(v[4][3:] for v in selected if v[4].startswith("路径：")))
        click(1050,700)
        assert api.take_request(app)==2
        frame()
        click(160,270)
        source=Path(data)/"input.wav"
        with wave.open(str(source),"wb") as audio:
            audio.setnchannels(1);audio.setsampwidth(2);audio.setframerate(48000)
            audio.writeframes(b"\0\0"*480)
        event(9,key=2,text=str(source).encode())
        assert json.loads((target/"manifest.json").read_text())["audioFileName"]=="audio.wav"
        assert (target/"audio.wav").read_bytes()==source.read_bytes()
        event(4,x=1200,y=750,delta=-20)
        assert any(v[4]=="描述" for v in frame())
        assert any(v[4]=="Local charts" for v in frame(True))
        event(5,key=102,modifiers=1)
        event(6,text="没有这种曲名".encode())
        assert any(v[4]=="没有匹配的谱面" for v in frame())
        event(5,key=97,modifiers=1)
        event(6,text="中文与English".encode())
        assert any(v[4]=="中文与English 🎵" for v in frame())
        event(5,key=97,modifiers=1)
        event(5,key=8)
        event(5,key=13)
        frame()
        click(1330,50)
        assert any(v[:3]==(0,(580.,60.,760.,960.),0x1f252dff) for v in frame())
        event(5,key=27)
        assert not any(v[:3]==(0,(580.,60.,760.,960.),0x1f252dff) for v in frame())
        selected=frame()
        maker_target=Path(next(v[4][3:] for v in selected if v[4].startswith("路径：")))
        assert event(11,key=2)
        maker_frame=frame()
        assert sum(v[0]==2 for v in maker_frame)>100
        assert any(v[4]=="保存" for v in maker_frame)
        assert any(v[4]=="Save" for v in frame(True))
        frame()
        click(100,212)
        frame()
        click(650,599)
        assert event(11,key=2) # repeated host navigation retains unsaved edits
        event(5,key=115,modifiers=1)
        saved=json.loads((maker_target/"score.json").read_text())
        assert len(saved["NoteList"])==1
        assert saved["NoteList"][0]["noteBaseType"]==1
        event(5,key=27)
        assert any(v[4]=="本地谱面" for v in frame())
    finally:
        api.destroy(app)
    # Loading after destruction verifies persistence through a fresh Rust state.
    app=api.create(str(ROOT).encode(),data.encode())
    assert app
    result=Frame()
    assert api.frame(app,0,0,C.byref(result))
    assert any(d.text and "中文与English" in d.text.decode() for d in result.draws[:result.count])
    api.destroy(app)
print("Rust C ABI: layout, fonts, create/save/reload, Chinese IME, duplicate, ZIP import/export, asynchronous file target, scroll, locale, modal, maker sprites/edit/save/navigation, teardown OK")
