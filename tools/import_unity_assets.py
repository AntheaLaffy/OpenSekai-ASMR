#!/usr/bin/env python3
"""Compile local Unity text assets into an inspectable native resource index.

This retains source IDs, GUID links, component fields and original images.
It is a data importer, not a claim that every imported component is executable.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import sys
import yaml

ROOT = Path(__file__).resolve().parents[1]
HEADER = re.compile(r"^--- !u!(\d+) &(-?\d+)(?: stripped)?\s*$", re.M)
LOADER = getattr(yaml, "CSafeLoader", yaml.SafeLoader)
TEXT_TYPES = {".prefab", ".unity", ".mat", ".asset", ".anim", ".controller"}

def unity_yaml(text):
    # A GUID can look numeric to YAML 1.1. Keep its exact spelling and zeros.
    text=re.sub(r'(\bguid:\s*)([0-9a-fA-F]+)(?=[,}\s])',r'\1"\2"',text)
    text=re.sub(r'(?m)^(\s*):',r'\1"":',text)
    # Unity serializes string fields without YAML scalar tags. Numeric sprite
    # names ("0.1"), text ("1"), and binary hex buffers must remain strings.
    text=re.sub(r'(?m)^(\s*(?:m_Name|m_text|spriteName|wordingKey|m_IndexBuffer|_typelessdata):[ \t]*)([^\s\'"|>][^\n]*)$',
                lambda m: m[1] + json.dumps(m[2], ensure_ascii=False), text)
    return yaml.load(text,Loader=LOADER)


def documents(text):
    matches = list(HEADER.finditer(text))
    result = []
    for i, match in enumerate(matches):
        end = matches[i+1].start() if i+1<len(matches) else len(text)
        data = unity_yaml(text[match.end():end])
        if not isinstance(data, dict) or len(data) != 1:
            raise ValueError("Expected one Unity object per YAML document")
        kind, fields = next(iter(data.items()))
        result.append({"id": int(match[2]), "class_id": int(match[1]), "kind": kind, "fields": fields})
    return result


def compile_assets(output):
    baseline = json.loads((ROOT / "native/baseline/files.json").read_text())
    paths = [item["path"] for item in baseline if item["path"].startswith("Assets/")]
    guids, metadata, scripts = {}, {}, {}
    for name in paths:
        if not name.endswith(".meta"):
            continue
        p = ROOT / name
        text=p.read_text(encoding="utf-8-sig")
        # Unity writes an empty platform key as `: Any`; libyaml requires the
        # empty string to be quoted. Source files are never rewritten.
        try:
            data = unity_yaml(text)
        except yaml.YAMLError as error:
            raise ValueError(f"Invalid metadata {name}: {error}") from error
        guid = data.get("guid")
        if not guid:
            raise ValueError(f"No GUID in {name}")
        asset = name[:-5]
        if guid in guids:
            raise ValueError(f"Duplicate GUID: {guids[guid]} and {asset}")
        guids[guid] = asset
        metadata[asset] = data
        if asset.endswith(".cs"):
            source = (ROOT / asset).read_text(encoding="utf-8-sig")
            namespace = re.search(r"\bnamespace\s+([\w.]+)", source)
            classes = re.findall(r"\b(?:class|struct|enum|interface)\s+(\w+)", source)
            scripts[guid] = {"path": asset, "namespace": namespace[1] if namespace else "", "types": classes}
    resources, assets, component_counts = {}, {}, Counter()
    links, unresolved = Counter(), Counter()

    def references(value):
        if isinstance(value, dict):
            guid = value.get("guid")
            if guid and guid != "0000000000000000f0000000000000000" and guid != "0000000000000000e0000000000000000":
                links[guid] += 1
                if guid not in guids:
                    unresolved[guid] += 1
            for v in value.values(): references(v)
        elif isinstance(value, list):
            for v in value: references(v)

    for name in paths:
        p = ROOT / name
        if p.suffix in TEXT_TYPES:
            data = p.read_bytes()
            if not data.startswith(b"%YAML"):
                continue
            objects = documents(data.decode("utf-8-sig"))
            for obj in objects:
                component_counts[obj["kind"]] += 1
                fields = obj["fields"]
                guid = fields.get("m_Script", {}).get("guid")
                if guid in scripts: obj["script"] = scripts[guid]
                references(fields)
            assets[name] = {"sha256":hashlib.sha256(data).hexdigest(), "objects":objects}
        elif p.suffix in {".png", ".jpg", ".jpeg", ".otf", ".ttf", ".shader"}:
            assets[name] = {"source":name, "importer":metadata.get(name,{})}
        if not name.endswith(".meta") and "/resources/" in name.lower():
            resource = name.lower().split("/resources/",1)[1].rsplit(".",1)[0]
            resources.setdefault(resource, []).append(name)
    output.mkdir(parents=True, exist_ok=True)
    index = {"schema":1,"source_commit":json.loads((ROOT/"native/baseline/project.json").read_text())["commit"],
             "guid_paths":guids,"resources":resources,"assets":assets}
    (output / "unity-assets.json").write_text(json.dumps(index,ensure_ascii=False,allow_nan=False,separators=(",",":"))+"\n")
    report = {"assets":len(assets),"objects":sum(component_counts.values()),"component_counts":dict(component_counts),
              "guid_count":len(guids),"unresolved_guids":dict(unresolved),
              "note":"Unresolved package/builtin references are preserved. Imported does not mean runtime-supported."}
    (output / "import-report.json").write_text(json.dumps(report,ensure_ascii=False,indent=2)+"\n")
    print(json.dumps({k:v for k,v in report.items() if k not in ("unresolved_guids","component_counts")},ensure_ascii=False))
    print(f"Unresolved external GUIDs: {len(unresolved)}; output: {output}")


if __name__ == "__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output",type=Path,default=ROOT/"native/generated")
    args=parser.parse_args()
    compile_assets(args.output)
