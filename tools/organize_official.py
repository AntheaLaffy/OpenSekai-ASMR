#!/usr/bin/env python3
"""Archive a user-supplied SUS collection and import editable native packages.

Never downloads music, changes the Unity reference Assets, or overwrites an
existing imported chart. The original location remains a relative symlink.
"""
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def inventory(root):
    return [{"path": str(p.relative_to(root)), "bytes": p.stat().st_size,
             "sha256": hashlib.sha256(p.read_bytes()).hexdigest()}
            for p in sorted(root.rglob("*")) if p.is_file()]


def main():
    original = ROOT / "pjsk官谱合集"
    source = ROOT / "content/official/source"
    source.parent.mkdir(parents=True, exist_ok=True)
    record = source.parent / "source-files.json"
    if not source.exists():
        before = inventory(original)
        original.rename(source)
        assert inventory(source) == before, "Relocation integrity check failed"
        record.write_text(json.dumps(before, ensure_ascii=False, indent=2) + "\n")
        original.symlink_to("content/official/source", target_is_directory=True)
    else:
        assert record.is_file(), "Existing archive has no inventory; refusing to replace it"
        assert inventory(source) == json.loads(record.read_text()), "Source archive has changed"
    result = subprocess.run([str(ROOT / "target/release/opensekai"), "import-sus",
                             str(source / "music_score"), str(ROOT / "content/library")],
                            text=True, capture_output=True, check=False)
    report = json.loads(result.stdout)
    report.update(source="content/official/source/music_score", library="content/library",
                  source_files=len(json.loads(record.read_text())), music_files=0)
    (ROOT / "content/official/import-report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(report, ensure_ascii=False))
    if result.returncode:
        raise SystemExit(result.stderr)


if __name__ == "__main__":
    main()
