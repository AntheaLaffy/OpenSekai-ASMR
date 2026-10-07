use opensekai::{model::Score, timeline::Timeline};
use std::{env, fs, process::ExitCode};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() == 3 && args[1] == "audit-library" {
        let (entries, warnings) = opensekai::storage::scan(std::path::Path::new(&args[2]))?;
        let mut count = 0;
        let mut total = 0;
        let mut mismatches = Vec::new();
        let mut metadata_checked = 0;
        for entry in entries {
            let score = entry.load_score()?;
            let mut session = opensekai::live::Session::new(&score, true, true)?;
            session.update(session.duration + 3.);
            if !session.finished || session.result.counts[6] as usize != session.result.total {
                return Err(format!("Autoplay incomplete: {}", entry.manifest.id).into());
            }
            if session.result.score != opensekai::scoring::MAX_SCORE {
                return Err(format!(
                    "Dynamic score normalization failed: {} = {}",
                    entry.manifest.id, session.result.score
                )
                .into());
            }
            if let Some(expected) = entry
                .manifest
                .extra
                .get("officialNoteCount")
                .and_then(|v| v.as_u64())
            {
                metadata_checked += 1;
                if expected != session.result.total as u64 {
                    mismatches.push(serde_json::json!({"id":entry.manifest.id,"expected":expected,"actual":session.result.total}));
                }
            }
            count += 1;
            total += session.result.total;
        }
        println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"completed":count,"judgements":total,"metadata_checked":metadata_checked,"warnings":warnings,"count_mismatches":mismatches,"scoring_rule":opensekai::scoring::RULE,"perfect_score":opensekai::scoring::MAX_SCORE})
            )?
        );
        return Ok(());
    }
    if args.len() == 4 && matches!(args[1].as_str(), "import-sus" | "check-sus") {
        let report = opensekai::library::import(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
            args[1] == "check-sus",
        )?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        if !report["errors"].as_array().unwrap().is_empty() {
            return Err("Some SUS charts failed; see report".into());
        }
        return Ok(());
    }
    if args.len() != 3 || args[1] != "inspect" {
        return Err("Usage: opensekai inspect SCORE.json | import-sus SOURCE DESTINATION | check-sus SOURCE DESTINATION | audit-library LIBRARY".into());
    }
    let score = Score::from_json(&fs::read_to_string(&args[2])?)?;
    let timeline = Timeline::from_events(&score.music_score_event_data_list)?;
    println!(
        "{}",
        serde_json::json!({
            "version": score.version_code,
            "musicId": score.music_id,
            "notes": score.note_list.len(),
            "events": score.music_score_event_data_list.len(),
            "durationSeconds": timeline.time_at(score.music_score_ticks_max),
            "validation": score.validate(),
        })
    );
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("OpenSekai: {e}");
            ExitCode::FAILURE
        }
    }
}
