//! On-disk contracts from MusicScoreMaker, with Newtonsoft reference resolution.
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

fn null_string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}
macro_rules! file_default {
    ($def:ident, $de:ident, $name:literal) => {
        fn $def() -> String {
            $name.into()
        }
        fn $de<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
            Ok(Option::<String>::deserialize(d)?.unwrap_or_else($def))
        }
    };
}
file_default!(audio_default, audio_de, "audio.ogg");
file_default!(jacket_default, jacket_de, "jacket.png");
file_default!(score_default, score_de, "score.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Manifest {
    pub format_version: i32,
    #[serde(deserialize_with = "null_string")]
    pub id: String,
    #[serde(deserialize_with = "null_string")]
    pub title: String,
    #[serde(deserialize_with = "null_string")]
    pub score_title: String,
    #[serde(deserialize_with = "null_string")]
    pub composer: String,
    #[serde(deserialize_with = "null_string")]
    pub lyricist: String,
    #[serde(deserialize_with = "null_string")]
    pub arranger: String,
    #[serde(deserialize_with = "null_string")]
    pub singer: String,
    #[serde(deserialize_with = "null_string")]
    pub collaboration_label: String,
    #[serde(deserialize_with = "null_string")]
    pub description: String,
    #[serde(deserialize_with = "audio_de")]
    pub audio_file_name: String,
    #[serde(deserialize_with = "jacket_de")]
    pub jacket_file_name: String,
    #[serde(deserialize_with = "score_de")]
    pub score_file_name: String,
    #[serde(deserialize_with = "null_string")]
    pub video_file_name: String,
    pub filler_sec: f32,
    pub sec_for_music_score_maker: i32,
    pub preview_start_time_sec: f32,
    #[serde(deserialize_with = "null_string")]
    pub user_name: String,
    #[serde(deserialize_with = "null_string")]
    pub music_difficulty_type: String,
    pub play_level: i32,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Manifest {
    fn default() -> Self {
        // Deserialize defaults through an explicit complete object to keep null
        // and empty filenames distinct, as in CustomMusicScoreManifest.Normalize.
        Self {
            format_version: 1,
            id: String::new(),
            title: String::new(),
            score_title: String::new(),
            composer: String::new(),
            lyricist: String::new(),
            arranger: String::new(),
            singer: String::new(),
            collaboration_label: String::new(),
            description: String::new(),
            audio_file_name: audio_default(),
            jacket_file_name: jacket_default(),
            score_file_name: score_default(),
            video_file_name: String::new(),
            filler_sec: 0.,
            sec_for_music_score_maker: 0,
            preview_start_time_sec: 0.,
            user_name: String::new(),
            music_difficulty_type: String::new(),
            play_level: 0,
            extra: BTreeMap::new(),
        }
    }
}
impl Manifest {
    pub fn normalize_with_id(&mut self, generated_id: &str) {
        if self.format_version <= 0 {
            self.format_version = 1;
        }
        if self.id.trim().is_empty() {
            self.id = generated_id.into();
        }
        if self.title.trim().is_empty() {
            self.title = "Untitled".into();
        }
        if self.score_title.trim().is_empty() {
            self.score_title.clone_from(&self.title);
        }
        self.filler_sec = self.filler_sec.max(0.);
        self.sec_for_music_score_maker = self.sec_for_music_score_maker.max(0);
        self.preview_start_time_sec = self.preview_start_time_sec.max(0.);
        self.play_level = self.play_level.max(0);
        if self.music_difficulty_type.trim().is_empty() {
            self.music_difficulty_type = "master".into();
        }
    }
    pub fn music_id(&self) -> i32 {
        music_id(&self.id)
    }
}

pub fn music_id(id: &str) -> i32 {
    let text = if id.is_empty() {
        "custom".into()
    } else {
        id.to_lowercase()
    };
    // C# chars are UTF-16 code units, not UTF-8 bytes or Unicode scalar values.
    let hash = text
        .encode_utf16()
        .fold(2166136261u32, |h, c| (h ^ c as u32).wrapping_mul(16777619));
    -((hash & 0x7fffffff).max(1) as i32)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Note {
    pub id: i32,
    pub ticks: i64,
    pub lane_start: i32,
    pub lane_end: i32,
    pub category: i32,
    #[serde(rename = "type")]
    pub note_type: i32,
    pub speed_ratio: f32,
    pub note_line_type: i32,
    pub note_base_type: i32,
    pub previous_connection_id: i32,
    pub next_connection_id: i32,
    pub direction: i32,
    pub is_skip: bool,
    pub is_decoration: bool,
    pub guide_color: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Note {
    fn default() -> Self {
        Self {
            id: 0,
            ticks: 0,
            lane_start: 0,
            lane_end: 0,
            category: 0,
            note_type: 0,
            speed_ratio: 1.,
            note_line_type: 0,
            note_base_type: 0,
            previous_connection_id: -1,
            next_connection_id: -1,
            direction: 0,
            is_skip: false,
            is_decoration: false,
            guide_color: None,
            extra: BTreeMap::new(),
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Event {
    pub id: i32,
    pub event_type: i32,
    pub ticks: i64,
    pub change_value: Value,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "PascalCase")]
pub struct Score {
    pub version_code: i32,
    pub music_score_event_data_list: Vec<Event>,
    pub event_array: Vec<Value>,
    pub note_list: Vec<Note>,
    pub music_score_ticks_max: i64,
    pub music_id: i32,
    pub full_combo_data_hash: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Score {
    fn default() -> Self {
        Self {
            version_code: 1,
            music_score_event_data_list: Vec::new(),
            event_array: Vec::new(),
            note_list: Vec::new(),
            music_score_ticks_max: 0,
            music_id: 0,
            full_combo_data_hash: None,
            extra: BTreeMap::new(),
        }
    }
}
impl Score {
    pub fn from_json(text: &str) -> Result<Self, String> {
        let value: Value =
            serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
        let value = resolve_references(value)?;
        let score: Self = serde_json::from_value(value).map_err(|e| e.to_string())?;
        if score.version_code > 1 {
            return Err(format!("Unsupported chart version {}", score.version_code));
        }
        Ok(score)
    }
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();
        let mut ids = HashMap::new();
        for n in &self.note_list {
            if ids.insert(n.id, n).is_some() {
                issues.push(format!("Duplicate note ID {}", n.id));
            }
            // MusicScoreMakerUtility computes width as laneEnd - laneStart + 1.
            // Both ends are inclusive, including one-lane notes at lane 11.
            if n.ticks < 0 || n.lane_start < 0 || n.lane_start > n.lane_end || n.lane_end >= 12 {
                issues.push(format!("Note {} has invalid ticks or lane bounds", n.id));
            }
            if !(0..=15).contains(&n.category)
                || !(0..=1).contains(&n.note_type)
                || !(0..=2).contains(&n.note_line_type)
                || !(0..=2).contains(&n.direction)
                || !(0..=14).contains(&n.note_base_type)
            {
                issues.push(format!("Note {} has an unknown note kind", n.id));
            }
        }
        for n in &self.note_list {
            for (id, forward) in [
                (n.previous_connection_id, false),
                (n.next_connection_id, true),
            ] {
                if id < 0 {
                    continue;
                }
                match ids.get(&id) {
                    None => issues.push(format!("Note {} references missing note {id}", n.id)),
                    Some(other) => {
                        let reciprocal = if forward {
                            other.previous_connection_id
                        } else {
                            other.next_connection_id
                        };
                        if reciprocal != n.id || id == n.id {
                            issues.push(format!(
                                "Note {} has a non-reciprocal connection to {id}",
                                n.id
                            ));
                        }
                        if forward && other.ticks <= n.ticks {
                            issues
                                .push(format!("Note {} connection does not advance in time", n.id));
                        }
                    }
                }
            }
        }
        issues
    }
}

fn resolve_references(root: Value) -> Result<Value, String> {
    fn index(v: &Value, ids: &mut HashMap<String, Value>) -> Result<(), String> {
        match v {
            Value::Object(o) => {
                if let Some(id) = o.get("$id").and_then(Value::as_str)
                    && ids.insert(id.into(), v.clone()).is_some()
                {
                    return Err(format!("Duplicate JSON reference {id}"));
                }
                for v in o.values() {
                    index(v, ids)?;
                }
            }
            Value::Array(a) => {
                for v in a {
                    index(v, ids)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    fn expand(
        v: Value,
        ids: &HashMap<String, Value>,
        active: &mut HashSet<String>,
        depth: usize,
    ) -> Result<Value, String> {
        if depth > 128 {
            return Err("JSON reference nesting limit exceeded".into());
        }
        match v {
            Value::Object(mut o) => {
                if let Some(id) = o.get("$ref").and_then(Value::as_str) {
                    let id = id.to_owned();
                    if !active.insert(id.clone()) {
                        return Err(format!("Cyclic JSON reference {id}"));
                    }
                    let target = ids
                        .get(&id)
                        .ok_or_else(|| format!("Missing JSON reference {id}"))?;
                    let result = expand(target.clone(), ids, active, depth + 1);
                    active.remove(&id);
                    return result;
                }
                o.remove("$id");
                if let Some(values) = o.remove("$values") {
                    return expand(values, ids, active, depth + 1);
                }
                for value in o.values_mut() {
                    *value = expand(value.take(), ids, active, depth + 1)?;
                }
                Ok(Value::Object(o))
            }
            Value::Array(a) => a
                .into_iter()
                .map(|v| expand(v, ids, active, depth + 1))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            v => Ok(v),
        }
    }
    let mut ids = HashMap::new();
    index(&root, &mut ids)?;
    expand(root, &ids, &mut HashSet::new(), 0)
}
