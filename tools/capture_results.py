#!/usr/bin/env python3
"""Execute original result selection and inspect the shipped DOTween ease enum."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile
from capture_sound import method, ROOT, SOURCE
from import_opensekai_resources import baseline_check

OUT = ROOT / "native/baseline/results.json"


def capture():
    baseline_check()
    selection = SOURCE / "Core/Live/SoloLiveController.cs"
    settings = SOURCE / "LiveSettingData.cs"
    constants = "\n".join(re.findall(r"public const int AutoResultAnimation\w+ = \d+;", settings.read_text()))
    assert len(constants.splitlines()) == 5
    program = r'''
using System;
using System.Collections.Generic;
using System.Text.Json;
using System.Reflection;
using Sekai.Core.Live;
class Oracle {
  class Music { public bool IsTestPlay=false; }
  class Boot { public Music MusicData=new(); public bool IsAuto; }
  struct LiveScore { public int totalComboCount, autoCount, perfectCount, maxCombo, life; }
  class Logic { public LiveScore Score; }
  class LiveSettingData {
    CONSTANTS
    public int AutoResultAnimationMode;
    public static LiveSettingData value=new();
    public static LiveSettingData LoadFromStorage() => value;
  }
  static class Debug { public static void LogErrorFormat(string message, object value) { throw new Exception(message); } }
  Boot BootData=new(); int result=3; Logic liveLogic=new();
  METHOD
  static void Main(string[] args) {
    var oracle=new Oracle();var cases=new List<object>();
    foreach(bool auto in new[]{false,true})
    foreach(int mode in new[]{0,1,2,3,4,5})
    foreach(int life in new[]{0,1000})
    foreach(int outcome in new[]{0,1,2,3}) {
      int perfect=outcome==0?3:outcome==1?2:0;
      int maxCombo=outcome<=1?3:outcome==2?1:3;
      int autoCount=outcome==3?3:0;
      oracle.BootData.IsAuto=auto;
      LiveSettingData.value.AutoResultAnimationMode=mode;
      oracle.liveLogic.Score=new LiveScore{totalComboCount=3,autoCount=autoCount,perfectCount=perfect,maxCombo=maxCombo,life=life};
      cases.Add(new{auto,mode,life,perfect,maxCombo,autoCount,expected=oracle.GetLiveResultAnimationType().ToString()});
    }
    Type ease=Assembly.LoadFrom(args[0]).GetType("DG.Tweening.Ease");
    Console.Write(JsonSerializer.Serialize(new{cases,fadeEase=Enum.GetName(ease,6)}));
  }
}
'''.replace("CONSTANTS", constants).replace("METHOD", method(selection.read_text(), "GetLiveResultAnimationType"))
    with tempfile.TemporaryDirectory(prefix="opensekai-result-oracle-") as temp:
        temp = Path(temp)
        (temp / "oracle.csproj").write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>')
        (temp / "Program.cs").write_text(program)
        (temp / "LiveResultAnimationType.cs").write_bytes((SOURCE / "Core/Live/LiveResultAnimationType.cs").read_bytes())
        subprocess.run(["dotnet", "build", str(temp / "oracle.csproj"), "--nologo", "-v:q"], check=True)
        output = json.loads(subprocess.check_output(["dotnet", str(temp / "bin/Debug/net10.0/oracle.dll"),
                           str(ROOT / "Assets/Plugins/Demigiant/DOTween/DOTween.dll")]))
    assert output["fadeEase"] == "OutQuad"
    return {"source": str(selection.relative_to(ROOT)), "sha256": hashlib.sha256(selection.read_bytes()).hexdigest(),
            "cases": output["cases"], "fade_ease": output["fadeEase"],
            "scope": "Original completed-song selection with score/settings stubs; shipped DOTween enum value 6. No Unity raster or particle execution."}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capture", action="store_true")
    args = parser.parse_args()
    data = capture()
    if args.capture: OUT.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")
    else: assert data == json.loads(OUT.read_text()), "Original result oracle changed"
    print(f"Original C# result selection: {len(data['cases'])} cases, DOTween {data['fade_ease']}")
