#!/usr/bin/env python3
"""Execute original C# minimap pixel loops with small Unity value-type stubs.

The fixture is a component behavior reference, not a Unity raster capture.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess
import tempfile

from capture_baseline import ROOT, OUT, SEKAI, UTILITY, check, method

SOURCE = SEKAI + "MusicScoreMaker/Ingame/Views/MusicScoreMinimapView.cs"


def instance_method(source, name):
    # Reuse the balanced-body extractor without changing method bodies.
    static = re.sub(r"private (void|int|long) " + name + r"\(",
                    r"private static \1 " + name + "(", source)
    result = method(static, name)
    return result.replace("public static", "public", 1)


def capture():
    source = (ROOT / SOURCE).read_text()
    util = (ROOT / UTILITY).read_text()
    methods = "\n".join(instance_method(source, name) for name in (
        "UpdateDisplayRange", "DrawNote", "DrawLongNoteBand", "DrawBarLines",
        "DrawBarLinesInRange", "TicksToPixelY", "PixelYToTicks"))
    methods += "\n" + "\n".join(method(source, name) for name in (
        "CalculateTextureHeight", "ApplyEasing", "GetNoteColor", "IsHiddenMinimapCategory"))
    utilities = "\n".join(method(util, name) for name in (
        "TryParseTimeSignature", "FormatTimeSignatureText", "ConvertFloatToTimeSignatureString",
        "GetTimeSignatureFromChangeValue", "ConvertToFloat"))
    constants = "\n".join(re.findall(r"private const [^;]+;", source))
    colors = "\n".join("static readonly Color32 " + line for line in re.findall(r"(Color\w+ = new Color32\([^;]+;)", source))
    enums = "\n".join((ROOT / (SEKAI + "Live/" + name + ".cs")).read_text()
                      for name in ("NoteCategory", "NoteType", "NoteLineType"))
    enums += (ROOT / (SEKAI + "MusicScoreMaker/Ingame/Models/MusicScoreEventType.cs")).read_text()
    program = r'''
using System;
using System.Linq;
using System.Collections.Generic;
using System.Globalization;
using System.Text.Json;
using Sekai.Live;
using Sekai.MusicScoreMaker.Ingame.Models;
struct Color32 { public byte r,g,b,a; public Color32(byte r,byte g,byte b,byte a) { this.r=r;this.g=g;this.b=b;this.a=a; } }
static class Mathf {
public static int Clamp(int x,int a,int b)=>Math.Clamp(x,a,b);
public static float Clamp01(float x)=>Math.Clamp(x,0f,1f);
public static int Max(int a,int b)=>Math.Max(a,b);
public static int RoundToInt(float x)=>(int)Math.Round(x);
public static float Lerp(float a,float b,float t)=>a+(b-a)*Clamp01(t);
public static bool Approximately(float a,float b)=>Math.Abs(b-a)<Math.Max(0.000001f*Math.Max(Math.Abs(a),Math.Abs(b)),float.Epsilon*8f);
}
class MusicScoreNoteBase {
public int id,laneStart,laneEnd,nextConnectionId=-1; public long ticks;
public NoteCategory category; public NoteType type; public NoteLineType noteLineType; public bool isSkip;
}
class MusicScoreEventData { public long ticks; public MusicScoreEventType eventType; public object changeValue; }
static class MusicScoreMakerUtility {
public const long TICKS_PER_BAR=1920; const float DEFAULT_TIME_SIGNATURE=4;
public static long Max,Focus;
public static long GetMusicScoreTicksMax()=>Max;
public static long GetFocusTicks()=>Focus;
UTILITIES
}
class Oracle {
CONSTANTS
COLORS
long _displayStartTicks,_displayEndTicks,_displayRangeTicks,_cachedMaxTicks;
bool _isDirty; Color32[] _pixelBuffer;
METHODS
static object Run(long focus, long maxTicks) {
MusicScoreMakerUtility.Focus=focus; MusicScoreMakerUtility.Max=maxTicks;
var map=new Oracle(); map.UpdateDisplayRange();
int height=CalculateTextureHeight(map._displayRangeTicks);
map._pixelBuffer=new Color32[TEXTURE_WIDTH*height];
var events=new List<MusicScoreEventData> {
new(){ticks=0,eventType=MusicScoreEventType.TimeSignature,changeValue="4/4"},
new(){ticks=3840,eventType=MusicScoreEventType.TimeSignature,changeValue="3/4"},
new(){ticks=24000,eventType=MusicScoreEventType.TimeSignature,changeValue="7/8"}};
var notes=new List<MusicScoreNoteBase>();
for(int i=0;i<16;i++) notes.Add(new(){id=i,ticks=i*240,laneStart=i%10,laneEnd=i%10+2,
category=(NoteCategory)i,type=(NoteType)(i%2),isSkip=i==14});
notes.Add(new(){id=20,ticks=2400,laneStart=0,laneEnd=2,category=NoteCategory.Long,nextConnectionId=21,noteLineType=NoteLineType.EaseIn});
notes.Add(new(){id=21,ticks=10800,laneStart=7,laneEnd=11,category=NoteCategory.Long});
notes.Add(new(){id=22,ticks=52800,laneStart=8,laneEnd=11,category=NoteCategory.Long,type=NoteType.Critical,nextConnectionId=23,noteLineType=NoteLineType.EaseOut});
notes.Add(new(){id=23,ticks=60000,laneStart=0,laneEnd=3,category=NoteCategory.Flick,type=NoteType.Critical});
notes.Add(new(){id=24,ticks=maxTicks,laneStart=11,laneEnd=11,category=NoteCategory.Normal});
map.DrawBarLines(events,height);
var ids=notes.ToDictionary(n=>n.id);
foreach(var note in notes) { if(note.nextConnectionId!=-1) map.DrawLongNoteBand(note,ids,height); map.DrawNote(note,height); }
var runs=new List<uint[]>();
foreach(var c in map._pixelBuffer) {
uint rgba=((uint)c.r<<24)|((uint)c.g<<16)|((uint)c.b<<8)|c.a;
if(runs.Count>0 && runs[^1][0]==rgba) runs[^1][1]++; else runs.Add(new[]{rgba,1u});
}
return new {focus,maxTicks,start=map._displayStartTicks,end=map._displayEndTicks,range=map._displayRangeTicks,height,notes,events,runs};
}
static void Main() {
var maps=new[]{Run(0,960),Run(480,960),Run(0,115200),Run(57600,115200),Run(115200,115200),Run(0,0)};
object[] inputs={"7/8","3/4","1.5","0.25",4,2.5,"bad","0/0"," -2/4 "};
var signatures=inputs.Select(input=>{var pair=MusicScoreMakerUtility.GetTimeSignatureFromChangeValue(input); return new {input,numerator=pair.Item1,denominator=pair.Item2};});
Console.Write(JsonSerializer.Serialize(new {maps,signatures},new JsonSerializerOptions {IncludeFields=true}));
}
}
ENUMS
'''.replace("UTILITIES", utilities).replace("CONSTANTS", constants).replace("COLORS", colors).replace("METHODS", methods).replace("ENUMS", enums)
    with tempfile.TemporaryDirectory(prefix="opensekai-minimap-oracle-") as temp:
        temp = Path(temp)
        (temp / "oracle.csproj").write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>')
        (temp / "Program.cs").write_text(program)
        subprocess.run(["dotnet", "build", str(temp / "oracle.csproj"), "--nologo", "-v:q"], check=True)
        data = json.loads(subprocess.check_output(["dotnet", str(temp / "bin/Debug/net10.0/oracle.dll")], text=True))
    data["reference_kind"] = "original-csharp-pixel-loops-with-unity-value-type-stubs"
    data["source"] = SOURCE
    return data


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capture", action="store_true")
    args = parser.parse_args()
    check()
    data = capture()
    output = OUT / "minimap.json"
    if args.capture:
        output.write_text(json.dumps(data, ensure_ascii=False, separators=(",", ":")) + "\n")
        print(f"Captured original C# minimap pixel loops: {output.stat().st_size} bytes")
    else:
        assert data == json.loads(output.read_text()), "Original minimap behavior changed"
        print("Original C# minimap oracle unchanged")
