//! Import a local SUS collection into ordinary, independently editable packages.
use crate::{model::Manifest, storage, sus, timeline::Timeline};
use serde_json::{Value, json};
use std::{fs, path::Path};

pub fn import(source: &Path, destination: &Path, validate_only: bool) -> Result<Value, String> {
    let mut songs = fs::read_dir(source)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter(|d| d.path().is_dir())
        .collect::<Vec<_>>();
    songs.sort_by_key(|d| d.file_name());
    let mut imported = 0;
    let mut existing = 0;
    let mut empty = Vec::new();
    let mut errors = Vec::new();
    let mut notes = 0;
    let mut charts = 0;
    for song in songs {
        let song_id = song.file_name().to_string_lossy().into_owned();
        let info = fs::read_to_string(song.path().join("info.txt")).unwrap_or_default();
        let title = sus::header(&info, "TITLE");
        for difficulty in ["easy", "normal", "hard", "expert", "master", "append"] {
            let path = song.path().join(format!("{difficulty}.txt"));
            if !path.is_file() {
                continue;
            }
            let result = (|| -> Result<(), String> {
                let text = storage::read_text(&path)?;
                let mut score = sus::parse(&text)?;
                if score.note_list.is_empty() {
                    empty.push(format!("{song_id}/{difficulty}"));
                    return Ok(());
                }
                notes += score.note_list.len();
                charts += 1;
                if validate_only {
                    return Ok(());
                }
                let directory = destination.join(format!("{song_id}-{difficulty}"));
                if directory.join("manifest.json").is_file() {
                    existing += 1;
                    return Ok(());
                }
                // Resume only the score written by this exact interrupted import.
                // An unrelated directory must never be overwritten.
                if directory.exists()
                    && directory
                        .read_dir()
                        .map_err(|e| e.to_string())?
                        .any(|item| item.is_err() || item.unwrap().file_name() != "score.json")
                {
                    return Err(format!(
                        "Incomplete package contains other files: {}",
                        directory.display()
                    ));
                }
                let mut manifest = Manifest {
                    id: format!("official-{song_id}-{difficulty}"),
                    title: if title.is_empty() {
                        song_id.clone()
                    } else {
                        title.clone()
                    },
                    music_difficulty_type: difficulty.into(),
                    user_name: sus::header(&text, "DESIGNER"),
                    play_level: sus::header(&text, "PLAYLEVEL").parse().unwrap_or(0),
                    sec_for_music_score_maker: (Timeline::from_events(
                        &score.music_score_event_data_list,
                    )?
                    .time_at(score.music_score_ticks_max)
                        + 3.)
                        .ceil() as i32,
                    ..Manifest::default()
                };
                manifest.score_title =
                    format!("{} [{}]", manifest.title, difficulty.to_uppercase());
                manifest.extra.insert(
                    "sourceSus".into(),
                    json!(format!("{song_id}/{difficulty}.txt")),
                );
                manifest.extra.insert(
                    "sourceWaveOffset".into(),
                    json!(sus::header(&text, "WAVEOFFSET")),
                );
                manifest
                    .extra
                    .insert("sourceCollection".into(), json!("user-provided-official"));
                manifest.normalize_with_id("");
                score.music_id = manifest.music_id();
                let score_bytes = serde_json::to_vec(&score).map_err(|e| e.to_string())?;
                let score_path = directory.join("score.json");
                if score_path.is_file()
                    && fs::read(&score_path).map_err(|e| e.to_string())? != score_bytes
                {
                    return Err(format!(
                        "Incomplete package has a different score: {}",
                        directory.display()
                    ));
                }
                // Publication of manifest is last: interrupted imports never appear playable.
                storage::atomic_write(&score_path, &score_bytes)?;
                storage::atomic_write(
                    &directory.join("manifest.json"),
                    &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
                )?;
                imported += 1;
                Ok(())
            })();
            if let Err(error) = result {
                errors.push(json!({"path":format!("{song_id}/{difficulty}.txt"),"error":error}));
            }
        }
    }
    Ok(
        json!({"charts":charts,"notes":notes,"imported":imported,"existing":existing,"empty":empty,"errors":errors}),
    )
}
