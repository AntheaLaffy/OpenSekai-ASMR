//! SUS import following the local Sekai.SUS.Converter and NoteInfo.Update.
//! Source text stays in the library; native edits are saved as score.json.
use crate::{
    maker::note_base_type,
    model::{Event, Note, Score},
};
use serde_json::json;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone)]
struct Raw {
    bar: usize,
    progress: f32,
    note: Note,
    channel: i32,
}
fn digit(b: u8) -> i32 {
    (b as char).to_digit(36).map_or(-1, |n| n as i32)
}
fn tokens(data: &str) -> Vec<([u8; 2], f32)> {
    let bytes = data.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if !bytes[i].is_ascii_alphanumeric() || !bytes[i + 1].is_ascii_alphanumeric() {
            i += 1;
            continue;
        }
        let value = [
            bytes[i].to_ascii_lowercase(),
            bytes[i + 1].to_ascii_lowercase(),
        ];
        i += 2;
        let mut speed = 1.;
        if bytes.get(i) == Some(&b',') {
            i += 1;
            let start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            speed = data[start..i].parse().unwrap_or(1.);
        }
        out.push((value, speed));
    }
    out
}
fn update(old: &mut Raw, new: &Raw) {
    let (a, b) = (&mut old.note, &new.note);
    if b.category != 0 {
        a.category = match (b.category, a.category) {
            (1, 4) => 6,
            (3, 4) => 8,
            (1, 5) => 7,
            _ => b.category,
        };
    }
    // Width and IsSkip belong to the first row, exactly as NoteInfo.Update.
    if b.note_type != 0 {
        a.note_type = b.note_type;
    }
    if b.direction != 0 {
        a.direction = b.direction;
    }
    if b.note_line_type != 0 {
        a.note_line_type = b.note_line_type;
    }
    if b.speed_ratio != 1. {
        a.speed_ratio = b.speed_ratio;
    }
    if new.channel != -1 {
        old.channel = new.channel;
    }
}
fn add(map: &mut BTreeMap<(u32, i32), Raw>, key: (u32, i32), row: Raw) {
    if let Some(old) = map.get_mut(&key) {
        update(old, &row);
    } else {
        map.insert(key, row);
    }
}
pub fn header(text: &str, name: &str) -> String {
    text.lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix(&format!("#{name} "))
                .map(|v| v.trim().trim_matches('"').to_owned())
        })
        .unwrap_or_default()
}
pub fn parse(text: &str) -> Result<Score, String> {
    let mut normal = BTreeMap::new();
    let mut guides = BTreeMap::new();
    let mut bpms = HashMap::new();
    let mut signatures = BTreeMap::from([(0usize, 4f32)]);
    let mut events: Vec<(usize, f32, i32, f32, bool)> = Vec::new();
    let mut source_events = Vec::new();
    let mut ticks_per_beat = 480f32;
    for (line_no, line) in text.lines().enumerate() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if let Some(value) = line.strip_prefix("#REQUEST \"ticks_per_beat ") {
            ticks_per_beat = value
                .trim_end_matches('"')
                .parse()
                .map_err(|_| "Invalid ticks_per_beat")?;
            if !ticks_per_beat.is_finite() || ticks_per_beat <= 0. {
                return Err("Invalid ticks_per_beat".into());
            }
        }
        if let Some((key, value)) = line.strip_prefix("#BPM").and_then(|s| s.split_once(':')) {
            let value = value.trim().parse::<f32>().map_err(|_| "Invalid BPM")?;
            if !value.is_finite() || value <= 0. {
                return Err("BPM must be positive and finite".into());
            }
            bpms.insert(key.to_lowercase(), value);
            continue;
        }
        if line.starts_with("#TIL00:") || line.starts_with("#VOLUME:") {
            let kind = if line.starts_with("#TIL00:") { 1 } else { 2 };
            let body = line.split_once(':').unwrap().1.trim().trim_matches('"');
            for value in body.split(',').filter(|s| !s.trim().is_empty()) {
                let (position, value) = value
                    .trim()
                    .split_once(':')
                    .ok_or("Invalid speed/volume event")?;
                let (bar, tick) = position.split_once('\'').ok_or("Invalid event position")?;
                events.push((
                    bar.parse().map_err(|_| "Invalid event bar")?,
                    tick.parse().map_err(|_| "Invalid event tick")?,
                    kind,
                    value.parse().map_err(|_| "Invalid event value")?,
                    true,
                ));
            }
            continue;
        }
        let Some((code, data)) = line.strip_prefix('#').and_then(|s| s.split_once(':')) else {
            continue;
        };
        if !matches!(code.len(), 5 | 6) || !code.as_bytes()[..3].iter().all(u8::is_ascii_digit) {
            continue;
        }
        let bar = code[..3].parse::<usize>().map_err(|_| "Invalid bar")?;
        if &code[3..] == "02" {
            let signature = data
                .trim()
                .parse::<f32>()
                .map_err(|_| "Invalid time signature")?;
            if !signature.is_finite() || signature <= 0. {
                return Err("Invalid time signature".into());
            }
            signatures.insert(bar, signature);
            continue;
        }
        let values = tokens(data);
        for (i, &(value, speed)) in values.iter().enumerate() {
            if value == *b"00" {
                continue;
            }
            let progress = i as f32 / values.len() as f32;
            if &code[3..] == "08" {
                let key = String::from_utf8_lossy(&value).into_owned();
                let bpm = *bpms
                    .get(&key)
                    .ok_or_else(|| format!("Line {}: undefined BPM {key}", line_no + 1))?;
                events.push((bar, progress, 0, bpm, false));
                continue;
            }
            let kind = digit(code.as_bytes()[3]);
            let lane = digit(code.as_bytes()[4]) - 2;
            let channel = code.as_bytes().get(5).map_or(-1, |b| digit(*b));
            let n = digit(value[0]);
            let width = digit(value[1]);
            if kind == 1 && ((lane == 13 && matches!(n, 1 | 2)) || n == 4) {
                source_events
                    .push(json!({"susBar":bar,"susProgress":progress,"susLane":lane,"susCode":n}));
                continue;
            }
            if lane < 0 || width < 1 || lane + width > 12 {
                continue;
            }
            let mut note = Note {
                lane_start: lane,
                lane_end: lane + width - 1,
                speed_ratio: speed,
                ..Note::default()
            };
            match (kind, channel, n) {
                (1, -1, 1 | 2) => note.note_type = n - 1,
                (1, -1, 3) => {
                    note.category = 14;
                    note.is_skip = true;
                }
                (1, -1, 5 | 6) => {
                    note.category = 4;
                    note.note_type = n - 5;
                }
                (1, -1, 7 | 8) => {
                    note.category = 5;
                    note.note_type = n - 7;
                }
                (5, -1, 1 | 3 | 4) => {
                    note.category = 3;
                    note.direction = if n == 1 { 0 } else { n - 2 };
                }
                (5, -1, 2) => note.note_line_type = 2,
                (5, -1, 5 | 6) => note.note_line_type = 1,
                (3, 0.., 1) => note.category = 1,
                (3, 0.., 2) => {}
                (3, 0.., 3) => note.category = 2,
                (3, 0.., 5) => note.category = 13,
                (9, 0.., 1) => note.category = 9,
                (9, 0.., 2) => note.category = 10,
                (9, 0.., 3 | 5) => note.category = 11,
                _ => {
                    return Err(format!(
                        "Line {}: unsupported SUS channel/token {code}:{}",
                        line_no + 1,
                        String::from_utf8_lossy(&value)
                    ));
                }
            }
            if !speed.is_finite() {
                return Err("Note speed must be finite".into());
            }
            // The source uses single-precision bar + progress as its merge key.
            let key = ((bar as f32 + progress).to_bits(), lane);
            let mut row = Raw {
                bar,
                progress,
                note,
                channel,
            };
            if kind == 9 {
                if let Some(inherited) = normal.get(&key) {
                    let inherited: &Raw = inherited;
                    row.note.note_type = inherited.note.note_type;
                    row.note.direction = inherited.note.direction;
                    row.note.note_line_type = inherited.note.note_line_type;
                }
                add(&mut guides, key, row);
            } else {
                if let Some(guide) = guides.get_mut(&key) {
                    update(guide, &row);
                }
                add(&mut normal, key, row);
            }
        }
    }
    let max_bar = normal
        .values()
        .chain(guides.values())
        .map(|r| r.bar)
        .chain(events.iter().map(|e| e.0))
        .chain(signatures.keys().copied())
        .max()
        .unwrap_or(0);
    if max_bar > 100_000 {
        return Err("SUS bar limit exceeded".into());
    }
    let mut starts = vec![0f64; max_bar + 2];
    let mut lengths = vec![1920f64; max_bar + 2];
    let mut signature = 4f32;
    for bar in 0..=max_bar {
        if let Some(&v) = signatures.get(&bar) {
            signature = v;
        }
        lengths[bar] = signature as f64 * 480.;
        starts[bar + 1] = starts[bar] + lengths[bar];
    }
    let tick = |bar: usize, fraction: f32| {
        (starts[bar] + lengths[bar] * fraction as f64).round_ties_even() as i64
    };
    let mut score = Score {
        event_array: source_events,
        ..Score::default()
    };
    for (&bar, &sig) in &signatures {
        events.push((bar, 0., 3, sig, false));
    }
    if !events.iter().any(|e| e.0 == 0 && e.1 == 0. && e.2 == 0) {
        events.push((0, 0., 0, 120., false));
    }
    for (bar, offset, kind, value, is_ticks) in events {
        if !value.is_finite() {
            return Err("Event value must be finite".into());
        }
        score.music_score_event_data_list.push(Event {
            id: score.music_score_event_data_list.len() as i32 + 1,
            event_type: kind,
            ticks: if is_ticks {
                (starts[bar] + offset as f64 * 480. / ticks_per_beat as f64).round_ties_even()
                    as i64
            } else {
                tick(bar, offset)
            },
            change_value: json!(value),
            ..Event::default()
        });
    }
    score
        .music_score_event_data_list
        .sort_by_key(|e| (e.ticks, e.id));
    for (is_guide, rows) in [(false, normal), (true, guides)] {
        let mut tails: HashMap<i32, usize> = HashMap::new();
        let mut roots: HashMap<i32, i32> = HashMap::new();
        for row in rows.values() {
            let mut note = row.note.clone();
            note.ticks = tick(row.bar, row.progress);
            note.id = score.note_list.len() as i32 + 1;
            note.note_base_type = note_base_type(note.category);
            if row.channel >= 0 {
                if if is_guide {
                    note.category == 9
                } else {
                    matches!(note.category, 1 | 6 | 7)
                } {
                    roots.insert(row.channel, note.note_type);
                } else if let Some(&previous) = tails.get(&row.channel) {
                    let previous = &mut score.note_list[previous];
                    if previous.ticks >= note.ticks {
                        return Err("Long-note points must advance in time".into());
                    }
                    note.previous_connection_id = previous.id;
                    previous.next_connection_id = note.id;
                    // Live long children inherit the root's critical appearance.
                    note.note_type = note.note_type.max(*roots.get(&row.channel).unwrap_or(&0));
                }
                tails.insert(row.channel, score.note_list.len());
            }
            score.music_score_ticks_max = score.music_score_ticks_max.max(note.ticks);
            score.note_list.push(note);
        }
    }
    let issues = score.validate();
    if !issues.is_empty() {
        return Err(issues.join("; "));
    }
    Ok(score)
}
