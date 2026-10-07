#!/usr/bin/env python3
"""Execute original SUS row parsing and overlays independently of Rust."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import tempfile

from capture_baseline import ROOT, SEKAI, method, check

def instance_method(source, name):
    static = re.sub(r"private (\w+) " + name + r"\(", r"private static \1 " + name + "(", source)
    return method(static, name).replace("public static", "public", 1)


def capture():
    source = (ROOT / (SEKAI + "SUS/Converter.cs")).read_text()
    names = ["Load", "LeadHeader", "LoadBpmInfo", "LoadHighSpeedInfo", "LoadVolumeInfo", "LoadEvent",
             "CreateTimeSignaturesEventBarData", "CreateBpmEventBarData", "CreateBarData", "CreateNoteBarData",
             "AddEventInfo", "AddNoteInfo", "AddNormalNoteInfo", "AddGuideInfo", "GetNoteLine", "GetNoteWidth",
             "AddNoteInfoToDictionary", "UpdateGuideFromNormalNote"]
    code = "\n".join(instance_method(source, name) for name in names)
    code += "\n" + "\n".join(method(source, name) for name in ["FindNoteInfo", "IsGuideCategory", "ParseNoteValues", "IsSusChar", "ParseSusDigit", "ParseInt", "ParseFloat"])
    patterns = re.search(r"static Converter\(\)\s*\{(.*?)\n\t\t\}", source, re.S)[1]
    program = r'''
using System; using System.IO; using System.Linq; using System.Collections.Generic;
using System.Globalization; using System.Text.RegularExpressions; using System.Text.Json;
using Sekai.SUS; using Sekai.Live;
class Oracle {
const float DefaultBpm=120, DefaultTimeSignature=4, DefaultSpeedRatio=1, DefaultSeVolume=1;
int ticksPerBeat=480;
Dictionary<float,Dictionary<int,NoteInfo>> noteInfoDict=new(),guideInfoDict=new();
Dictionary<float,List<EventInfo>> eventInfoDict=new();
List<TimeSignatureInfo> timeSignaturesInfos=new(); Dictionary<string,float> bpmMap=new();
List<BpmInfo> bpmInfos=new(); List<HighSpeedInfo> highSpeedInfos=new(); List<VolumeInfo> volumeInfos=new();
static readonly Regex EventNotesRegex,EventLongNotesRegex,BpmRegex,VolumeRegex,TicksPerBeatRegex,HighSpeedRegex;
static Oracle(){ PATTERNS }
METHODS
static void Main(string[] args) {
var input=JsonSerializer.Deserialize<string[]>(File.ReadAllText(args[0]));
var results=input.Select(text=>{var o=new Oracle();o.Load(text);
return new {text,notes=o.noteInfoDict.Concat(o.guideInfoDict).SelectMany(k=>k.Value.Values)
.OrderBy(n=>n.Bar+n.BarProgress).ThenBy(n=>n.Lane).ThenBy(n=>(int)n.Category)
.Select(n=>new {bar=n.Bar,progress=n.BarProgress,lane=n.Lane,width=n.Width,category=(int)n.Category,
direction=(int)n.Direction,line=(int)n.LineType,skip=n.IsSkip,speed=n.SpeedRatio}).ToArray()};});
Console.Write(JsonSerializer.Serialize(results));
}
}
'''.replace("PATTERNS", patterns).replace("METHODS", code)
    files = ["SUS/NoteInfo.cs", "SUS/EventInfo.cs", "SUS/EventType.cs", "SUS/BpmInfo.cs", "SUS/HighSpeedInfo.cs", "SUS/VolumeInfo.cs", "SUS/TimeSignatureInfo.cs", "Live/NoteCategory.cs", "Live/NoteType.cs", "Live/NoteDirection.cs", "Live/NoteLineType.cs"]
    cases = ["#BPM01:120\n#00008:01\n#00012:31\n#000320:31\n#00053:21\n#00033a:14\n#00133a:24\n#00014:51\n#00054:31\n#00095b:12\n#00195b:22\n#00016:11,0.5 00 21,2\n"]
    for name in ["0001_01/easy.txt", "0001_01/expert.txt", "tutorial/normal.txt"]:
        cases.append((ROOT / "content/official/source/music_score" / name).read_text())
    with tempfile.TemporaryDirectory(prefix="opensekai-sus-oracle-") as temporary:
        work = Path(temporary)
        (work / "Oracle.csproj").write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><EnableDefaultCompileItems>true</EnableDefaultCompileItems><WarningLevel>0</WarningLevel></PropertyGroup></Project>')
        (work / "Program.cs").write_text(program)
        for name in files:
            (work / Path(name).name).write_text((ROOT / (SEKAI + name)).read_text())
        (work / "input.json").write_text(json.dumps(cases))
        subprocess.run(["dotnet", "build", "-o", "out", "-v:q"], cwd=work, check=True, stdout=subprocess.PIPE)
        result = subprocess.run(["dotnet", "out/Oracle.dll", "input.json"], cwd=work, check=True, capture_output=True, text=True)
        return json.loads(result.stdout)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capture", action="store_true")
    args = parser.parse_args()
    check()
    result = capture()
    path = ROOT / "native/baseline/sus.json"
    if args.capture:
        path.write_text(json.dumps(result, ensure_ascii=False, separators=(",", ":")) + "\n")
    else:
        assert result == json.loads(path.read_text()), "Original SUS oracle changed"
    print(f"Original SUS: {len(result)} cases, {sum(len(c['notes']) for c in result)} merged rows")
