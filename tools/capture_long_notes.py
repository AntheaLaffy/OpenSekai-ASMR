#!/usr/bin/env python3
"""Run original LongNoteLinePreview mesh methods without a Unity renderer."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import tempfile

from capture_baseline import ROOT, OUT, SEKAI, check, method
from capture_minimap import instance_method

SOURCE = SEKAI + "MusicScoreMaker/Ingame/Views/LongNoteLinePreview.cs"


def capture():
    source = (ROOT / SOURCE).read_text()
    methods = "\n".join(instance_method(source, name) for name in (
        "EnsureSegmentLengthFromConfig", "PopulateStraightMesh", "PopulateEasedMesh",
        "CalculateSegmentCount", "AddQuad"))
    methods += "\n" + "\n".join(method(source, name) for name in ("GetClampedSegmentRate", "ApplyEasing"))
    enum = (ROOT / (SEKAI + "Live/NoteLineType.cs")).read_text()
    prefab = (ROOT / "Assets/GameObject/LongNoteLinePreviewPrefab.prefab").read_text()
    length = float(re.search(r"_segmentLength: ([\d.]+)", prefab)[1])
    program = r'''
using System;
using System.Linq;
using System.Collections.Generic;
using System.Text.Json;
using Sekai.Live;
struct Color32 {}
struct Vector2 {
public float x,y; public Vector2(float x,float y){this.x=x;this.y=y;}
public static float Distance(Vector2 a,Vector2 b){float x=a.x-b.x,y=a.y-b.y;return (float)Math.Sqrt(x*x+y*y);}
}
struct Vector4 { public float x,y,z,w; public Vector4(float x,float y,float z,float w){this.x=x;this.y=y;this.z=z;this.w=w;} }
static class Mathf {
public static float Clamp01(float x)=>Math.Clamp(x,0f,1f);
public static int Max(int a,int b)=>Math.Max(a,b);
public static int CeilToInt(float x)=>(int)Math.Ceiling(x);
public static float Lerp(float a,float b,float t)=>a+(b-a)*Clamp01(t);
}
class VertexHelper {
public List<float[]> vertices=new(); public List<int> triangles=new();
public int currentVertCount=>vertices.Count;
public void AddVert(Vector2 p,Color32 c,Vector4 uv)=>vertices.Add(new[]{p.x,p.y,uv.x,uv.y});
public void AddTriangle(int a,int b,int c)=>triangles.AddRange(new[]{a,b,c});
}
class ViewData {
public Vector2 StartLeft,StartRight,EndLeft,EndRight;
public NoteLineType LineType; public int Index,IndexMax;
}
class Oracle {
const float DefaultSegmentLength=30f;
float _segmentLength=SEGMENT_LENGTHf; bool _isSegmentLengthInitialized;
ViewData _meshViewData; Color32 color;
float _cachedUV_u0,_cachedUV_u1,_cachedUV_v0y,_cachedUV_v2y;
METHODS
static object Run(int kind,int index,int count,float[] start,float[] end,float[] uv) {
var oracle=new Oracle();
oracle._meshViewData=new(){StartLeft=new(start[0],start[1]),StartRight=new(start[2],start[3]),
EndLeft=new(end[0],end[1]),EndRight=new(end[2],end[3]),LineType=(NoteLineType)kind,Index=index,IndexMax=count};
oracle._cachedUV_u0=uv[0];oracle._cachedUV_u1=uv[2];oracle._cachedUV_v0y=uv[1];oracle._cachedUV_v2y=uv[3];
var mesh=new VertexHelper();
if(kind==0) oracle.PopulateStraightMesh(mesh); else oracle.PopulateEasedMesh(mesh);
return new {kind,index,count,start,end,uv,mesh.vertices,mesh.triangles};
}
static void Main() {
float[] start={-350,-420,-90,-420},end={30,420,400,420};
float[] uv={1084f/2048f,987f/1024f,1324f/2048f,984f/1024f};
var cases=Enumerable.Range(0,3).SelectMany(kind=>new[]{Run(kind,0,1,start,end,uv),Run(kind,1,4,start,end,uv)});
Console.Write(JsonSerializer.Serialize(cases));
}
}
ENUM
'''.replace("METHODS", methods).replace("ENUM", enum).replace("SEGMENT_LENGTH", str(length))
    with tempfile.TemporaryDirectory(prefix="opensekai-long-oracle-") as temp:
        temp = Path(temp)
        (temp / "oracle.csproj").write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>')
        (temp / "Program.cs").write_text(program)
        subprocess.run(["dotnet", "build", str(temp / "oracle.csproj"), "--nologo", "-v:q"], check=True)
        cases = json.loads(subprocess.check_output(["dotnet", str(temp / "bin/Debug/net10.0/oracle.dll")], text=True))
    return {"reference_kind": "original-csharp-mesh-methods-with-unity-value-type-stubs",
            "source": SOURCE, "segment_length": length, "cases": cases}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capture", action="store_true")
    args = parser.parse_args()
    check()
    data = capture()
    output = OUT / "long-notes.json"
    if args.capture:
        output.write_text(json.dumps(data, separators=(",", ":")) + "\n")
        print(f"Captured original C# long-note meshes: {output.stat().st_size} bytes")
    else:
        assert data == json.loads(output.read_text()), "Original long-note mesh behavior changed"
        print("Original C# long-note mesh oracle unchanged")
