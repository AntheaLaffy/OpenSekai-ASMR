#!/usr/bin/env python3
"""Make a chart-length 0114 edit from the preserved user MP3.

The recording supplies the musical edit, not its tap sounds. Original samples
are retained on either side of the splice. Existing different audio is never
replaced. Package filler is calibrated for this MP3 rather than the metadata's
different original game audio.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

import numpy as np
from scipy import signal

ROOT = Path(__file__).resolve().parents[1]
RECIPE = ROOT / "resources/audio/0114_01/edit.json"
REPORT = RECIPE.with_name("verification.json")


def sha(data):
    return hashlib.sha256(data).hexdigest()


def put(path, data):
    if path.exists():
        if path.is_symlink() or path.read_bytes() != data:
            raise ValueError(f"Existing different audio left untouched: {path}")
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("xb") as stream:
        stream.write(data)


def decode(path, rate=48000, channels=2):
    data = subprocess.check_output([
        "ffmpeg", "-nostdin", "-v", "error", "-i", str(path.resolve()),
        "-vn", "-f", "f32le", "-acodec", "pcm_f32le", "-ac", str(channels),
        "-ar", str(rate), "pipe:1"])
    audio = np.frombuffer(data, dtype="<f4").reshape(-1, channels)
    if not len(audio) or not np.isfinite(audio).all():
        raise ValueError(f"Invalid decoded audio: {path}")
    return audio


def recipe():
    data = json.loads(RECIPE.read_text())
    for key in ("source", "recording"):
        path = ROOT / data[key]
        if path.is_symlink() or sha(path.read_bytes()) != data[key + "_sha256"]:
            raise ValueError(f"Source changed: {path}")
    if data["sample_rate"] != 48000 or data["channels"] != 2:
        raise ValueError("Recipe must use the host's 48 kHz stereo format")
    return data


def expected_audio(data):
    audio = decode(ROOT / data["source"])
    start, end = data["remove_frames"]
    if not 0 < start < end < len(audio):
        raise ValueError("Invalid splice range")
    return audio, np.concatenate((audio[:start], audio[end:]))


def recording_check(data, game):
    rate = 8000
    # Downsampling only affects this comparison. Runtime WAV samples are exact.
    music = signal.resample_poly(game.mean(axis=1), 1, 6).astype(np.float64)
    recording = decode(ROOT / data["recording"], rate, 1).ravel().astype(np.float64)
    # Independent windows on both sides of the cut. Recorded tap sounds and
    # Opus compression lower correlation; they are not copied into game music.
    calibration = data["calibration"]
    slope = calibration["recording_to_source_slope"]
    offset = calibration["recording_to_source_offset_seconds"]
    full_music = signal.resample_poly(decode(ROOT / data["source"]).mean(axis=1), 1, 6).astype(np.float64)
    def feature(audio):
        _, _, spectrum = signal.stft(audio, fs=rate, nperseg=256,
                                     noverlap=192, boundary=None, padded=False)
        values = np.log(np.maximum(np.abs(spectrum[3:112]), 1e-5))
        values -= values.mean(axis=1, keepdims=True)
        return values.ravel()
    def similarity(a, b):
        return float(np.dot(a, b) / np.sqrt(np.dot(a, a) * np.dot(b, b)))
    windows = []
    for seconds in (25, 40, 60, 80, 100, 110, 120, 125, 128):
        # These are formal gameplay windows. Selection-page previews are never
        # used to find the start. The pre-cut affine map also gives shortened
        # game-audio time after the splice (source time there adds the cut).
        position = slope * seconds + offset
        start = round(position * rate)
        window = recording[round(seconds * rate):round((seconds + 2) * rate)]
        radius = round(.025 * rate)
        candidate = music[start - radius:start + radius + len(window)]
        centered = window - window.mean()
        dots = signal.correlate(candidate, centered, mode="valid", method="fft")
        cumulative = np.r_[0., np.cumsum(candidate)]
        power = np.r_[0., np.cumsum(candidate * candidate)]
        energies = (power[len(window):] - power[:-len(window)]
                    - (cumulative[len(window):] - cumulative[:-len(window)]) ** 2 / len(window))
        denominator = np.sqrt(np.maximum(energies, 1e-20) * np.dot(centered, centered))
        correlations = dots / denominator
        best = int(np.argmax(correlations))
        value = float(correlations[best])
        # Opus, stereo mixing and the game's SE change waveform phase. Compare
        # changing spectral envelopes as well, retaining the raw correlation
        # in the report. Negative controls reject a half-beat shift and, after
        # the cut, the unedited full-song passage.
        target = feature(window)
        spectral, error = -1., 0.
        for delta in np.arange(-.025, .026, .001):
            sample = round((position + delta) * rate)
            candidate_feature = feature(music[sample:sample + len(window)])
            score = similarity(target, candidate_feature)
            if score > spectral:
                spectral, error = score, float(delta)
        controls = []
        for delta in (-.19, .19):
            sample = round((position + delta) * rate)
            controls.append(similarity(target, feature(music[sample:sample + len(window)])))
        if seconds > calibration["recording_splice_seconds"]:
            controls.append(similarity(target, feature(full_music[start:start + len(window)])))
        if spectral < .55 or spectral - max(controls) < .15:
            raise ValueError(f"Recording musical passage mismatch at {seconds}s: {spectral:.4f}, controls={controls}")
        if abs(error) > .012:
            raise ValueError(f"Recording alignment moved at {seconds}s: {error:.4f}s")
        windows.append({"recording_seconds": seconds,
                        "game_audio_seconds": position + error,
                        "waveform_correlation": value,
                        "spectral_envelope_correlation": spectral,
                        "negative_controls": controls,
                        "local_offset_seconds": error})
    return windows


def packages(library, data, attach=False):
    audio_path = ROOT / data["output"]
    audio_bytes = audio_path.read_bytes()
    audio_hash = sha(audio_bytes)
    updates = []
    for manifest_path in sorted(library.glob("0114_01-*/manifest.json")):
        manifest = json.loads(manifest_path.read_text())
        if manifest.get("sourceMusicId") != 114:
            continue
        current = manifest_path.parent / manifest["audioFileName"]
        target = manifest_path.parent / "music-game.wav"
        for path in {current, target}:
            if path.is_file() and (path.is_symlink() or sha(path.read_bytes()) != audio_hash):
                raise ValueError(f"Existing different package music left untouched: {path}")
        # The chart's first playable tick is zero for all five imported charts.
        score = json.loads((manifest_path.parent / manifest["scoreFileName"]).read_text())
        if min(n["ticks"] for n in score["NoteList"]) != 0:
            raise ValueError(f"Chart origin moved: {manifest_path}")
        if attach:
            manifest.setdefault("sourceMetadataFillerSec", manifest["fillerSec"])
            manifest["audioFileName"] = target.name
            manifest["fillerSec"] = data["filler_seconds"]
            manifest["sourceEditedMedia"] = {
                "recipe": str(RECIPE.relative_to(ROOT)),
                "audio": data["output"], "sha256": audio_hash,
                "sourceSha256": data["source_sha256"]}
        elif (manifest["audioFileName"] != target.name
              or abs(manifest["fillerSec"] - data["filler_seconds"]) > 1e-7
              or not target.is_file()
              or manifest.get("sourceEditedMedia", {}).get("sha256") != audio_hash):
            raise ValueError(f"Package edit is not attached: {manifest_path}")
        updates.append((manifest_path, target, manifest))
    if len(updates) != 5:
        raise ValueError(f"Expected all five 0114_01 difficulties, found {len(updates)}")
    if attach:
        for manifest_path, target, manifest in updates:
            put(target, audio_bytes)
            temporary = manifest_path.with_suffix(".audio-edit.tmp")
            with temporary.open("x", encoding="utf-8") as out:
                json.dump(manifest, out, ensure_ascii=False, indent=2)
                out.write("\n")
            temporary.replace(manifest_path)
    return [path.parent.name for path, _, _ in updates]


def run(command, library):
    data = recipe()
    source, expected = expected_audio(data)
    output = ROOT / data["output"]
    if command == "prepare":
        # Float WAV preserves the decoded MP3 waveform without another lossy
        # encoding, automatic gain change, or integer clipping of MP3 peaks.
        with tempfile.TemporaryDirectory(prefix="ojsk-audio-edit-") as temporary:
            encoded = Path(temporary) / "music.wav"
            subprocess.run([
                "ffmpeg", "-nostdin", "-v", "error", "-f", "f32le", "-ac", "2",
                "-ar", "48000", "-i", "pipe:0", "-c:a", "pcm_f32le",
                "-fflags", "+bitexact", "-flags:a", "+bitexact", str(encoded)],
                input=expected.astype("<f4", copy=False).tobytes(), check=True)
            wav = encoded.read_bytes()
        put(output, wav)
    actual = decode(output)
    if not np.array_equal(expected, actual):
        raise ValueError("Game WAV differs from the two retained MP3 sample regions")
    if data["first_note_frame"] != round(data["filler_seconds"] * 48000):
        raise ValueError("Filler does not place tick zero at the calibrated first note")
    windows = recording_check(data, actual)
    attached = packages(library, data, attach=command == "prepare")
    report = {"schema": 1, "source_sha256": data["source_sha256"],
              "output": data["output"], "output_sha256": sha(output.read_bytes()),
              "source_frames": len(source), "game_frames": len(actual),
              "game_seconds": len(actual) / 48000,
              "removed_frames": data["remove_frames"][1] - data["remove_frames"][0],
              "filler_seconds": data["filler_seconds"],
              "samples_equal_to_source_regions": True,
              "selection_preview_excluded": True,
              "recording_windows": windows, "packages": attached,
              "scope": "Waveform edit and chart-origin alignment. Video timing estimates have approximately 40 ms uncertainty; recorded judgement sounds are excluded."}
    if command == "prepare":
        put(REPORT, (json.dumps(report, ensure_ascii=False, indent=2) + "\n").encode())
    else:
        saved = json.loads(REPORT.read_text())
        if saved["output_sha256"] != report["output_sha256"] or saved["game_frames"] != len(actual):
            raise ValueError("Saved verification does not describe the current WAV")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("prepare", "verify"))
    parser.add_argument("--library", type=Path, default=ROOT / "content/library")
    args = parser.parse_args()
    try:
        print(json.dumps(run(args.command, args.library.resolve()), ensure_ascii=False, indent=2))
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"0114 audio edit: {error}\n")


if __name__ == "__main__":
    main()
