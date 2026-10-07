//! Decode local music once. Playback and its sample clock belong to the host.
use std::{path::Path, process::Command};

pub fn decode(path: &Path) -> Result<Vec<f32>, String> {
    let result = Command::new("ffmpeg")
        .args(["-nostdin", "-v", "error", "-i"])
        .arg(path)
        .args([
            "-t",
            "1801",
            "-f",
            "f32le",
            "-acodec",
            "pcm_f32le",
            "-ac",
            "2",
            "-ar",
            "48000",
            "pipe:1",
        ])
        .output()
        .map_err(|e| format!("Audio decoder (ffmpeg): {e}"))?;
    if !result.status.success() || result.stdout.is_empty() {
        return Err(format!(
            "Audio decode failed: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    if result.stdout.len() % 8 != 0 {
        return Err("Incomplete stereo PCM".into());
    }
    if result.stdout.len() > 1800 * 48000 * 8 {
        return Err("Music is longer than the supported 30 minutes".into());
    }
    Ok(result
        .stdout
        .as_chunks::<4>()
        .0
        .iter()
        .map(|v| f32::from_le_bytes(*v))
        .collect())
}
