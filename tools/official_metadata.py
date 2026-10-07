#!/usr/bin/env python3
"""Read the user's OOXML metadata table without editing/recalculating it.

Match by musicId, never worksheet row position. Save provenance and leave a
previously enriched manifest alone so later user edits survive another run.
"""
import hashlib
import json
from pathlib import Path
import re
import shutil
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[1]
NS = {"s": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}


def read_metadata(path):
    with zipfile.ZipFile(path) as archive:
        strings = []
        if "xl/sharedStrings.xml" in archive.namelist():
            strings = ["".join(t.text or "" for t in item.iterfind(".//s:t", NS))
                       for item in ET.fromstring(archive.read("xl/sharedStrings.xml"))]
        sheet = ET.fromstring(archive.read("xl/worksheets/sheet1.xml"))
        rows = []
        for row in sheet.findall("s:sheetData/s:row", NS):
            values = {}
            for cell in row:
                column = re.match(r"[A-Z]+", cell.attrib["r"])[0]
                value = cell.find("s:v", NS)
                if cell.find("s:f", NS) is not None:
                    raise ValueError(f"Metadata formula needs explicit verification: {cell.attrib['r']}")
                text = value.text if value is not None else "".join(t.text or "" for t in cell.iterfind(".//s:t", NS))
                if cell.attrib.get("t") == "s": text = strings[int(text)]
                values[column] = text
            rows.append(values)
    headers = rows[0]
    assert headers.get("A") == "musicId" and "title" in headers.values()
    result = {}
    for row in rows[1:]:
        record = {name: row.get(column, "") for column, name in headers.items()}
        music_id = int(record["musicId"])
        if music_id in result: raise ValueError(f"Duplicate musicId: {music_id}")
        result[music_id] = record
    return result


def main():
    source = ROOT / "官谱id信息对照表.xlsx"
    digest = hashlib.sha256(source.read_bytes()).hexdigest()
    rows = read_metadata(source)
    archive = ROOT / "content/official"
    destination = archive / "source-metadata.xlsx"
    if destination.exists(): assert hashlib.sha256(destination.read_bytes()).hexdigest() == digest
    else: shutil.copyfile(source, destination)
    (archive / "metadata.json").write_text(json.dumps(rows, ensure_ascii=False, indent=2) + "\n")
    updated, unmatched = 0, []
    for path in sorted((ROOT / "content/library").glob("*/manifest.json")):
        data = json.loads(path.read_text())
        if data.get("sourceCollection") != "user-provided-official": continue
        if data.get("sourceMetadataSha256") == digest: continue
        folder = data["sourceSus"].split("/")[0]
        match = re.fullmatch(r"(\d+)_\d+", folder)
        row = rows.get(int(match[1])) if match else None
        if not row:
            unmatched.append(data["id"])
            continue
        for key in ("title", "composer", "lyricist", "arranger", "description"):
            if row[key]: data[key] = row[key]
        data["scoreTitle"] = f'{data["title"]} [{data["musicDifficultyType"].upper()}]'
        level = row[data["musicDifficultyType"] + "Level"]
        count = row[data["musicDifficultyType"] + "NoteCount"]
        if level: data["playLevel"] = int(level)
        if count: data["officialNoteCount"] = int(count)
        if row["fillerSec"]: data["fillerSec"] = float(row["fillerSec"])
        if row["secForMusicScoreMaker"]: data["secForMusicScoreMaker"] = int(row["secForMusicScoreMaker"])
        data["sourceMusicId"] = int(row["musicId"])
        data["sourceMetadataSha256"] = digest
        temporary = path.with_suffix(".metadata.tmp")
        temporary.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")
        temporary.replace(path)
        updated += 1
    report = {"source":source.name,"sha256":digest,"sheet":"original_music_metadata","songs":len(rows),"updated":updated,"unmatched":unmatched}
    (archive / "metadata-report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(report, ensure_ascii=False))


if __name__ == "__main__":
    main()
