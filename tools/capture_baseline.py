#!/usr/bin/env python3
"""Capture the checked-out C# project, never a Rust-generated visual reference.

The oracle compiles original pure C# methods in isolation. It does not emulate
Unity rendering. Updating reference files requires an explicit --capture.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "native/baseline"
SEKAI = "Assets/Scripts/Assembly-CSharp/Sekai/"
MANIFEST = SEKAI + "MusicScoreMaker/Common/CustomMusicScoreManifest.cs"
UTILITY = SEKAI + "MusicScoreMaker/Ingame/Utilities/MusicScoreMakerUtility.cs"
MANAGER = "Assets/CustomMusicScoreManager/Runtime/UI/ScreenLayerCustomMusicScoreManager.cs"
ENTRY = SEKAI + "MusicScoreMaker/Common/CustomMusicScoreEntry.cs"
PRESENTER = SEKAI + "MusicScoreMaker/Ingame/Presenters/MusicScoreMakerPresenter.cs"


def run(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def write(name, value):
    (OUT / name).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def method(source, name):
    # These oracle methods contain no interpolated braces or brace literals.
    match = re.search(r"(?:private|public) static [^\n]+\b" + name + r"\([^\n]*\)", source)
    if not match:
        raise ValueError(f"Original method missing: {name}")
    start = source.index("{", match.end())
    depth, end = 1, start + 1
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    return source[match.start():end].replace("private static", "public static", 1)


def capture_oracle():
    util = (ROOT / UTILITY).read_text()
    entry = (ROOT / ENTRY).read_text()
    functions = "\n".join(method(util, m) for m in
        ("GetTicksFromTimeBpmOnly", "GetTimeFromTicksBpmOnly", "QuantizeTicks", "QuantizeTicksInBar"))
    functions += "\n" + method(entry, "CreateStableMusicId")
    functions += "\n" + method((ROOT / PRESENTER).read_text(), "GetLaneRangeFromCenterLane")
    # Only the random ID provider and unused attribute are substituted. Existing
    # IDs remain unchanged, so normalization can be compared deterministically.
    stub = '''
namespace Newtonsoft.Json { class JsonIgnoreAttribute : System.Attribute {} }
namespace Sekai.MusicScoreMaker.Common {
static class CustomMusicScoreStorage {
public static string GenerateShortId() => "baseline0001";
}}
'''
    program = r'''
using System;
using System.Linq;
using System.Text.Json;
using Sekai.MusicScoreMaker.Common;
class MusicScoreMakerModel { public const int LaneCountMinus1 = 11, DEFAULT_LAST_NOTE_WIDTH = 2; }
class Oracle {
const float DEFAULT_BPM = 120f, SECONDS_PER_MINUTES = 60f;
const long TICKS_PER_BEAT = 480L, TICKS_PER_BAR = 1920L;
FUNCTIONS
static void Main() {
  var options = new JsonSerializerOptions { IncludeFields = true };
  var cases = new[] {
    "{}",
    "{\"id\":\"ABC123\",\"title\":\"测试谱面\",\"audioFileName\":\"\",\"scoreFileName\":null}",
    "{\"formatVersion\":-1,\"id\":\" \",\"title\":\" \",\"scoreTitle\":\"\",\"fillerSec\":-5,\"secForMusicScoreMaker\":-2,\"previewStartTimeSec\":-1,\"playLevel\":-3,\"musicDifficultyType\":\" \"}",
    "{\"id\":\"曲目😀\",\"title\":\"song\",\"scoreTitle\":\"chart\",\"fillerSec\":1.25,\"playLevel\":32,\"musicDifficultyType\":\"append\"}"
  };
  var manifests = cases.Select(input => {
    var m = JsonSerializer.Deserialize<CustomMusicScoreManifest>(input, options)!;
    m.Normalize();
    // Unity manifest JSON serializes fields; computed flags are excluded.
    var normalized = typeof(CustomMusicScoreManifest).GetFields()
      .ToDictionary(f => f.Name, f => f.GetValue(m));
    return new { input = JsonSerializer.Deserialize<JsonElement>(input), normalized,
      musicId = CreateStableMusicId(m.id) };
  });
  var bpms = new (long ticks, float bpm)[][] {
    new[] {(0L,120f)}, new[] {(0L,120f),(960L,180f),(2400L,90f)},
    new[] {(0L,120f),(1440L,0f),(1920L,256.5f)}
  };
  var timeline = bpms.Select(bpm => new {
    bpm = bpm.Select(p => new { ticks=p.ticks, bpm=p.bpm }),
    ticks = new long[] {-240,0,1,240,480,959,960,1440,1920,2400,987654}
      .Select(t => new { ticks=t, seconds=GetTimeFromTicksBpmOnly(t,bpm,bpm.Length) }),
    seconds = new float[] {-.25f,0f,0.00052083336f,0.0015625f,.25f,.5f,1f,1.2f,2f,3.14159f,123.456f}
      .Select(t => new { seconds=t, ticks=GetTicksFromTimeBpmOnly(t,bpm,bpm.Length) })
  });
  var quantization = new[] {0,120,160,480}.SelectMany(step =>
    new long[] {-1980,-1920,-1919,-60,-59,0,59,60,61,1919,1920,1980,2147483648}
      .Select(ticks => new { step, ticks, result = QuantizeTicksInBar(ticks, step) }));
  var lanes = new[] {0,1,2,5,10,11}.SelectMany(center => new[] {0,1,2,5,12}.Select(width => {
    var result = GetLaneRangeFromCenterLane(center, width);
    return new { center, width, start = result.Item1, end = result.Item2 };
  }));
  Console.Write(JsonSerializer.Serialize(new { manifests, timeline, quantization, lanes }, options));
}}
'''.replace("FUNCTIONS", functions)
    with tempfile.TemporaryDirectory(prefix="opensekai-oracle-") as tmp:
        tmp = Path(tmp)
        (tmp / "oracle.csproj").write_text(
            '<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType>'
            '<TargetFramework>net10.0</TargetFramework><EnableDefaultCompileItems>true</EnableDefaultCompileItems>'
            '</PropertyGroup></Project>')
        (tmp / "Manifest.cs").write_text((ROOT / MANIFEST).read_text())
        (tmp / "Stubs.cs").write_text(stub)
        (tmp / "Program.cs").write_text(program)
        subprocess.run(["dotnet", "build", str(tmp / "oracle.csproj"), "--nologo", "-v:q"], check=True)
        data = subprocess.check_output(["dotnet", str(tmp / "bin/Debug/net10.0/oracle.dll")], text=True)
    return json.loads(data)


def capture():
    OUT.mkdir(parents=True, exist_ok=True)
    paths = run("git", "ls-files", "Assets", "Packages", "ProjectSettings").splitlines()
    files = []
    counts = Counter()
    for name in sorted(paths):
        p = ROOT / name
        if not p.is_file():
            raise ValueError(f"Tracked source missing: {name}")
        data = p.read_bytes()
        counts[p.suffix.lower()] += 1
        files.append({"path": name, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()})
    write("files.json", files)
    settings = (ROOT / "ProjectSettings/ProjectSettings.asset").read_text()
    source = (ROOT / MANAGER).read_text()
    constants = dict(re.findall(r"private const (?:int|float) (\w+) = ([\d.]+)f?;", source))
    texts = re.findall(r'CreateText\("([^"]+)", [^,]+, "([^"]*)", ([\d.]+), FontStyles.(\w+), TextAlignmentOptions.(\w+)\)', source)
    colors = sorted(set(tuple(map(int, c)) for c in re.findall(r"new Color32\((\d+), (\d+), (\d+), (\d+)\)", source)))
    write("manager-source.json", {
        "source": MANAGER, "constants": constants, "texts": [dict(zip(
            ("node", "text", "size", "style", "alignment"), t)) for t in texts],
        "colors_rgba": colors,
        "reference_font_eb": "Assets/Resources/font/FOT-RodinNTLGPro-EB.otf",
        "reference_font_db": "Assets/Resources/font/FOT-RodinNTLGPro-DB.otf",
    })
    write("oracle.json", capture_oracle())
    write("project.json", {
        "schema": 1, "reference_kind": "source-contract-not-unity-raster",
        "commit": run("git", "rev-parse", "HEAD"),
        "unity": (ROOT / "ProjectSettings/ProjectVersion.txt").read_text().strip(),
        "version": re.search(r"bundleVersion: (.+)", settings)[1],
        "default_resolution": [int(re.search(r"defaultScreen" + dim + r": (\d+)", settings)[1]) for dim in ("Width", "Height")],
        "files": len(files), "bytes": sum(f["bytes"] for f in files),
        "extensions": dict(sorted(counts.items())),
        "unity_raster_reference": None,
        "oracle": "Original C# normalization, music ID and BPM math; deterministic random-ID stub only.",
    })
    print(f"Captured {len(files)} original files and C# oracle to {OUT}")


def check():
    failures = []
    files = json.loads((OUT / "files.json").read_text())
    for f in files:
        p = ROOT / f["path"]
        if not p.is_file() or hashlib.sha256(p.read_bytes()).hexdigest() != f["sha256"]:
            failures.append(f["path"])
    if failures:
        raise SystemExit("Original baseline changed:\n" + "\n".join(failures))
    print(f"Original source/assets unchanged: {len(files)} SHA-256 matches")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capture", action="store_true")
    parser.add_argument("--capture-oracle", action="store_true", help="Extend behavior fixtures only, after checking original hashes")
    args = parser.parse_args()
    if args.capture_oracle:
        check()
        write("oracle.json", capture_oracle())
    else:
        capture() if args.capture else check()
