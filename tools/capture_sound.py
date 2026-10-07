#!/usr/bin/env python3
"""Execute the original C# cue table and result selection without Unity audio."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import tempfile
from import_opensekai_resources import baseline_check

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "Assets/Scripts/Assembly-CSharp/Sekai"
OUT = ROOT / "native/baseline/sound.json"


def method(text, name):
    match = re.search(r"private (?:static )?[^\n]+\b" + name + r"\([^\n]*\)", text)
    start = text.index("{", match.end())
    end, depth = start + 1, 1
    while depth:
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    return text[match.start():end]


def capture():
    baseline_check()
    source = (SOURCE / "TapEffectView.cs").read_text()
    methods = "\n".join(method(source, name) for name in ("SetupSEDict", "GetTapSeKey", "PlaySe"))
    program = r'''
using System;
using System.Collections.Generic;
using System.Text.Json;
using Sekai.Live;
class Oracle {
  struct NoteInfo { public NoteCategory category; public NoteType type; public NoteResult result; public int noteId; public object pairNote; }
  Dictionary<int,string> tapSEDict;
  Dictionary<int,string> pairNoteSeMap;
  NoteInfo noteInfo;
  string cue, group;
  void PlaySE(string name,string category,int id,object pair) { cue=name; group=category; }
  METHODS
  static void Main() {
    var oracle=new Oracle();oracle.SetupSEDict(false);
    var cases=new List<object>();
    foreach(int category in new[]{0,1,2,3,4,6,8,12})
    foreach(int type in new[]{0,1})
    foreach(var result in new[]{NoteResult.JustPerfect,NoteResult.Perfect,NoteResult.Great,NoteResult.Good,NoteResult.Bad,NoteResult.Miss,NoteResult.Auto}) {
      oracle.cue=null;oracle.group=null;
      oracle.noteInfo=new NoteInfo{category=(NoteCategory)category,type=(NoteType)type,result=result,noteId=1,pairNote=null};
      // Excute returns before invoking PlaySe for Miss and generated Combo.
      if(result!=NoteResult.Miss && category!=12) oracle.PlaySe();
      cases.Add(new{category,critical=type==1,judge=result.ToString(),cue=oracle.cue,group=oracle.group});
    }
    Console.Write(JsonSerializer.Serialize(cases));
  }
}
'''.replace("METHODS", methods)
    with tempfile.TemporaryDirectory(prefix="opensekai-sound-oracle-") as temp:
        temp = Path(temp)
        (temp / "oracle.csproj").write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>')
        (temp / "Program.cs").write_text(program)
        for name in ("NoteCategory", "NoteType", "NoteResult", "LiveSoundDefine"):
            (temp / (name + ".cs")).write_bytes((SOURCE / "Live" / (name + ".cs")).read_bytes())
        subprocess.run(["dotnet", "build", str(temp / "oracle.csproj"), "--nologo", "-v:q"], check=True)
        cases = json.loads(subprocess.check_output(["dotnet", str(temp / "bin/Debug/net10.0/oracle.dll")]))
    return {"source": "Assets/Scripts/Assembly-CSharp/Sekai/TapEffectView.cs", "cases": cases,
            "scope": "Original SetupSEDict/GetTapSeKey/PlaySe; PlaySE records its arguments, no Unity/audio simulation."}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capture", action="store_true")
    args = parser.parse_args()
    data = capture()
    if args.capture:
        OUT.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")
    else:
        assert data == json.loads(OUT.read_text()), "Original sound oracle changed"
    print(f"Original C# sound selection: {len(data['cases'])} cases verified")
