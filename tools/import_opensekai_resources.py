#!/usr/bin/env python3
"""Preserve art/audio from a local OpenSekai Git revision and verify its provenance.

The snapshot has its own Unity GUID namespace, outside the existing Assets
baseline. Only explicit resource roots are imported; no nested Git repository,
Unity cache, C# runtime, DLL or project settings are copied.
"""
import argparse
from collections import Counter
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import tarfile
import wave

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / "resources/opensekai"
RESOURCE_ROOTS = (
    "Assets/Charts", "Assets/Features", "Assets/Fonts", "Assets/Shaders",
    "Assets/TextMesh Pro", "Assets/Scenes", "Assets/Settings",
)
EXTRA_FILES = (
    "Assets/DefaultVolumeProfile.asset", "Assets/DefaultVolumeProfile.asset.meta",
    "Assets/UniversalRenderPipelineGlobalSettings.asset",
    "Assets/UniversalRenderPipelineGlobalSettings.asset.meta", "LICENSE", "README.md",
)
MUSIC = "Assets/Charts/Tell Your World/music.wav"
JACKET = "Assets/Charts/Tell Your World/jacket.png"
MUSIC_BLOB = "c84a4b79df1787c4767517df5028066881548b47"


def git(repo, *args):
    return subprocess.check_output(["git", "-C", str(repo), *args])


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def blob_hash(data):
    return hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()


def json_bytes(value):
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode()


def selected(path):
    return (path in EXTRA_FILES or any(path == r + ".meta" or path.startswith(r + "/")
                                      for r in RESOURCE_ROOTS)) and not path.endswith(
                                          (".scenetemplate", ".scenetemplate.meta"))


def baseline_check():
    """When present, verify the pre-existing reference instead of recapturing it."""
    baseline = ROOT / "native/baseline/files.json"
    if not baseline.is_file():
        return None
    rows = json.loads(baseline.read_text())
    for row in rows:
        path = ROOT / row["path"]
        if not path.is_file() or sha256(path.read_bytes()) != row["sha256"]:
            raise ValueError(f"Existing Unity reference changed: {path}")
    return len(rows)


def put_new(path, data):
    if path.is_symlink():
        raise ValueError(f"Refusing to follow a destination symlink: {path}")
    if path.exists():
        if path.read_bytes() != data:
            raise ValueError(f"Refusing to overwrite different content: {path}")
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("xb") as out:
        out.write(data)


def import_resources(source, revision):
    baseline_check()
    commit = git(source, "rev-parse", revision + "^{commit}").decode().strip()
    origin = git(source, "remote", "get-url", "origin").decode().strip()
    tree = {}
    for line in git(source, "ls-tree", "-rlz", commit).split(b"\0"):
        if not line:
            continue
        header, name = line.split(b"\t", 1)
        mode, kind, blob, size = header.decode().split()
        name = name.decode()
        if selected(name):
            if kind != "blob" or mode != "100644":
                raise ValueError(f"Unexpected resource type: {name}")
            tree[name] = (blob, int(size))
    if MUSIC not in tree or tree[MUSIC][0] != MUSIC_BLOB:
        raise ValueError("This revision does not contain the confirmed Tell Your World WAV")
    archive = git(source, "archive", "--format=tar", commit, "--", *RESOURCE_ROOTS,
                  *(r + ".meta" for r in RESOURCE_ROOTS), *EXTRA_FILES)
    payloads = {}
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        for member in tar:
            if member.name in tree:
                if not member.isfile():
                    raise ValueError(f"Not a regular file: {member.name}")
                payloads[member.name] = tar.extractfile(member).read()
    if payloads.keys() != tree.keys():
        raise ValueError("Git archive does not match the selected tree")

    existing = {}
    for raw in git(ROOT, "ls-files", "-z", "--", "Assets").split(b"\0"):
        if raw:
            name = raw.decode()
            existing.setdefault(sha256((ROOT / name).read_bytes()), []).append(name)

    rows, audio = [], []
    for name, data in sorted(payloads.items()):
        blob, size = tree[name]
        if size != len(data) or blob != blob_hash(data):
            raise ValueError(f"Git blob mismatch: {name}")
        row = {"path": "upstream/" + name, "source_path": name, "bytes": size,
               "git_blob": blob, "sha256": sha256(data)}
        meta = payloads.get(name + ".meta", b"")
        guid = re.search(rb"(?m)^guid: ([0-9a-f]{32})", meta)
        if guid:
            row["unity_guid"] = guid[1].decode()
        if row["sha256"] in existing:
            row["identical_existing_files"] = existing[row["sha256"]]
        rows.append(row)
        if name.endswith(".wav"):
            with wave.open(io.BytesIO(data), "rb") as wav:
                item = {"path": row["path"], "git_blob": blob, "sha256": row["sha256"],
                        "kind": "music" if name == MUSIC else "sound_effect",
                        "channels": wav.getnchannels(), "sample_rate": wav.getframerate(),
                        "bits_per_sample": wav.getsampwidth() * 8,
                        "frames": wav.getnframes(), "duration_seconds": wav.getnframes() / wav.getframerate()}
            if item["kind"] == "sound_effect":
                item["bank"] = str(Path(name).parent.relative_to("Assets/Features/live"))
                item["cue"] = Path(name).stem if Path(name).stem.lower().startswith("se_") else None
            audio.append(item)
    scene = payloads["Assets/Scenes/LiveScene.unity"].decode()
    filler = float(re.search(r"(?m)^  previewFillerSec: ([0-9.]+)$", scene)[1])
    manifest = {
        "schema": 1,
        "source": {"repository": origin, "commit": commit,
                   "selection": list(RESOURCE_ROOTS) + list(EXTRA_FILES),
                   "excluded": ["C# scripts and plugins", "Unity scene template", "Packages and ProjectSettings", "Git and editor caches"]},
        "summary": {"files": len(rows), "bytes": sum(r["bytes"] for r in rows),
                    "extensions": dict(sorted(Counter(Path(r["source_path"]).suffix for r in rows).items())),
                    "audio_files": len(audio), "sound_effects": len(audio) - 1,
                    "identical_existing_files": sum("identical_existing_files" in r for r in rows)},
        "tell_your_world": {"music": "upstream/" + MUSIC, "jacket": "upstream/" + JACKET,
                            "git_blob": MUSIC_BLOB, "filler_seconds": filler,
                            "filler_source": "upstream/Assets/Scenes/LiveScene.unity:previewFillerSec"},
        "files": rows,
    }
    generated = {"manifest.json": json_bytes(manifest),
                 "audio-catalog.json": json_bytes({"schema": 1, "source_commit": commit, "clips": audio})}
    # Complete preflight before any writes, including an interrupted prior import.
    writes = {"upstream/" + name: data for name, data in payloads.items()} | generated
    for name, data in writes.items():
        target = DEST / name
        if target.is_symlink() or (target.exists() and target.read_bytes() != data):
            raise ValueError(f"Destination has different content: {target}")
    for name, data in writes.items():
        put_new(DEST / name, data)
    return verify(False)


def verify(check_index):
    manifest = json.loads((DEST / "manifest.json").read_text())
    expected = set()
    for row in manifest["files"]:
        expected.add(row["path"])
        path = DEST / row["path"]
        if path.is_symlink():
            raise ValueError(f"Snapshot must contain regular files: {path}")
        data = path.read_bytes()
        if len(data) != row["bytes"] or sha256(data) != row["sha256"] or blob_hash(data) != row["git_blob"]:
            raise ValueError(f"Resource checksum mismatch: {path}")
    actual = {str(p.relative_to(DEST)) for p in (DEST / "upstream").rglob("*") if p.is_file()}
    if actual != expected:
        raise ValueError(f"Snapshot inventory mismatch: {actual ^ expected}")
    catalog = json.loads((DEST / "audio-catalog.json").read_text())
    audio_paths = {r["path"] for r in manifest["files"] if r["path"].endswith(".wav")}
    if {c["path"] for c in catalog["clips"]} != audio_paths:
        raise ValueError("Audio catalog does not cover the snapshot")
    indexed = 0
    if check_index:
        entries = {}
        for raw in git(ROOT, "ls-files", "--stage", "-z", "--", str(DEST.relative_to(ROOT))).split(b"\0"):
            if raw:
                header, name = raw.split(b"\t", 1)
                mode, blob, stage = header.decode().split()
                if stage != "0" or mode != "100644":
                    raise ValueError(f"Unexpected index entry: {name!r}")
                entries[name.decode()] = blob
        for path in sorted(DEST.rglob("*")):
            if path.is_file():
                name = str(path.relative_to(ROOT))
                if entries.get(name) != blob_hash(path.read_bytes()):
                    raise ValueError(f"File is not fully staged in Git: {name}")
                indexed += 1
    return {"resources": manifest["summary"], "original_reference_files_verified": baseline_check(),
            "git_index_files_verified": indexed}


def attach_song(library):
    verify(False)
    manifest = json.loads((DEST / "manifest.json").read_text())
    media = manifest["tell_your_world"]
    music = (DEST / media["music"]).read_bytes()
    jacket = (DEST / media["jacket"]).read_bytes()
    updates = []
    for path in sorted(library.glob("*/manifest.json")):
        song = json.loads(path.read_text())
        sus = song.get("sourceSus", "")
        if song.get("sourceMusicId") != 1 and not sus.startswith(("0001_01/", "Tell Your World/")):
            continue
        for field, name, data in (("audioFileName", "music.wav", music), ("jacketFileName", "jacket.png", jacket)):
            old = path.parent / Path(song.get(field) or name).name
            new = path.parent / name
            for candidate in (old, new):
                if candidate.is_symlink() or (candidate.exists() and candidate.read_bytes() != data):
                    raise ValueError(f"Existing user media differs; left untouched: {candidate}")
            song[field] = name
        song["fillerSec"] = media["filler_seconds"]
        song["sourceRestoredMedia"] = {"repository": manifest["source"]["repository"],
                                       "commit": manifest["source"]["commit"], "musicGitBlob": MUSIC_BLOB}
        updates.append((path, song))
    if not updates:
        raise ValueError("No Tell Your World packages found; import the bundled SUS charts first (see resource README)")
    for path, song in updates:
        put_new(path.parent / "music.wav", music)
        put_new(path.parent / "jacket.png", jacket)
        data = json_bytes(song)
        if path.read_bytes() != data:
            temporary = path.with_suffix(".media.tmp")
            with temporary.open("xb") as out:
                out.write(data)
            temporary.replace(path)
    return {"attached_packages": [str(path.parent.relative_to(library)) for path, _ in updates],
            "music_git_blob": MUSIC_BLOB, "filler_seconds": media["filler_seconds"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    importing = commands.add_parser("import")
    importing.add_argument("--source", type=Path, required=True)
    importing.add_argument("--revision", default="HEAD")
    checking = commands.add_parser("verify")
    checking.add_argument("--git-index", action="store_true")
    attaching = commands.add_parser("attach-song")
    attaching.add_argument("--library", type=Path, default=ROOT / "content/library")
    args = parser.parse_args()
    try:
        if args.command == "import":
            result = import_resources(args.source.resolve(), args.revision)
        elif args.command == "verify":
            result = verify(args.git_index)
        else:
            result = attach_song(args.library.resolve())
        print(json.dumps(result, ensure_ascii=False, indent=2))
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Resource import: {error}\n")


if __name__ == "__main__":
    main()
