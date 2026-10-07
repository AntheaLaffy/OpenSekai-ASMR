#!/usr/bin/env python3
"""Preserve user-supplied 0114 media and extract visual reference frames.

The frame labels describe visible states, not a gameplay clock contract. Sources
are copied byte-for-byte; existing different media is never replaced.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / "resources/references/0114_01"
SOURCES = {"recording.mkv": "id:114,title:里表情人,真实参考.mkv",
           "music-full.mp3": "wowaka 初音ミク - 裏表ラバーズ.mp3",
           "jacket.jpg": "里表情人.jpg"}
FRAMES = [(0, "selection"), (22.8, "hold-start"), (30, "flick-pink"),
          (40, "tap-blue"), (60, "curved-hold"), (100, "critical-gold"),
          (110, "simultaneous-flick"), (129.95, "last-notes"),
          (130.5, "ending-background"), (131.35, "fc-enter"),
          (131.5, "fc-ring"), (131.75, "fc-burst"), (132, "fc-flash"),
          (132.5, "fc-particles"), (133.5, "fc-settled"),
          (135.5, "fc-fade"), (153.5, "result")]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def put(path, data):
    if path.exists():
        if path.is_symlink() or path.read_bytes() != data:
            raise ValueError(f"Existing different file left untouched: {path}")
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("xb") as out:
        out.write(data)


def probe(path):
    return json.loads(subprocess.check_output([
        "ffprobe", "-v", "error", "-show_entries",
        "stream=index,codec_name,codec_type,width,height,avg_frame_rate,sample_rate,channels:format=duration,size",
        "-of", "json", str(path.resolve())]))


def importing(source):
    entries = []
    for name, original in SOURCES.items():
        data = (source / original).read_bytes()
        put(DEST / name, data)
        entries.append({"path": name, "original_filename": original,
                        "bytes": len(data), "sha256": digest(data), "media": probe(DEST / name)})
    frames = []
    for seconds, label in FRAMES:
        name = f"frames/{label}.png"
        data = subprocess.check_output([
            "ffmpeg", "-nostdin", "-v", "error", "-ss", str(seconds),
            "-i", str(DEST / "recording.mkv"), "-frames:v", "1",
            "-f", "image2pipe", "-vcodec", "png", "pipe:1"])
        put(DEST / name, data)
        with Image.open(DEST / name) as frame:
            size = list(frame.size)
        frames.append({"path": name, "state": label, "recording_seconds": seconds,
                       "size": size, "sha256": digest(data)})
    reference = {"schema": 1, "music_id": 114, "title_zh": "里表情人",
                 "title": "裏表ラバーズ", "provided_by": "user",
                 "observed_chart": {"difficulty": "MASTER", "combo": 1151, "ending": "FULL COMBO"},
                 "comparison": "Compare equivalent visible states; retain Rust data structures and gameplay logic.",
                 "sources": entries, "frames": frames}
    put(DEST / "reference.json", (json.dumps(reference, ensure_ascii=False, indent=2) + "\n").encode())
    return verify()


def verify():
    reference = json.loads((DEST / "reference.json").read_text())
    for item in reference["sources"] + reference["frames"]:
        path = DEST / item["path"]
        if path.is_symlink() or digest(path.read_bytes()) != item["sha256"]:
            raise ValueError(f"Reference changed: {path}")
    return {"sources_verified": len(reference["sources"]),
            "frames_verified": len(reference["frames"]), "reference": str(DEST / "reference.json")}


def attach_cover(library):
    import io
    verify()
    buffer = io.BytesIO()
    with Image.open(DEST / "jacket.jpg") as image:
        image.convert("RGBA").save(buffer, format="PNG")
    data = buffer.getvalue()
    updates = []
    for path in sorted(library.glob("0114_01-*/manifest.json")):
        manifest = json.loads(path.read_text())
        if manifest.get("sourceMusicId") != 114:
            continue
        cover = path.parent / "jacket.png"
        if cover.exists() and (cover.is_symlink() or cover.read_bytes() != data):
            raise ValueError(f"Existing different cover left untouched: {cover}")
        manifest["jacketFileName"] = "jacket.png"
        manifest["sourceVisualReference"] = "resources/references/0114_01/reference.json"
        updates.append((path, cover, manifest))
    if not updates:
        raise ValueError("No imported 0114_01 packages found")
    for path, cover, manifest in updates:
        put(cover, data)
        temporary = path.with_suffix(".reference.tmp")
        with temporary.open("x", encoding="utf-8") as out:
            json.dump(manifest, out, ensure_ascii=False, indent=2)
            out.write("\n")
        temporary.replace(path)
    return {"cover_attached_packages": [path.parent.name for path, _, _ in updates]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("import", "verify", "attach-cover"))
    parser.add_argument("--source-dir", type=Path, default=ROOT)
    parser.add_argument("--library", type=Path, default=ROOT / "content/library")
    args = parser.parse_args()
    try:
        result = importing(args.source_dir.resolve()) if args.command == "import" else (
            attach_cover(args.library.resolve()) if args.command == "attach-cover" else verify())
        print(json.dumps(result, ensure_ascii=False, indent=2))
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Visual reference: {error}\n")


if __name__ == "__main__":
    main()
