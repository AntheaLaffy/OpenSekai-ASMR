//! Original cue selection and prefab clip bindings; no decoding on the hit path.
use crate::{
    ffi::{AppFrame, AppSound, AppSoundCommand},
    live::{Judge, Session},
    model::{Note, Score},
    timeline::Timeline,
};
use serde::Deserialize;
use std::{collections::HashMap, path::Path};

#[derive(Deserialize)]
struct Definition {
    cues: Vec<CueDefinition>,
}
#[derive(Deserialize)]
struct CueDefinition {
    name: String,
    gain: f32,
    clips: Vec<ClipDefinition>,
}
#[derive(Deserialize)]
struct ClipDefinition {
    path: String,
}
struct Cue {
    clips: Vec<u32>,
    next: usize,
    gain: f32,
}
struct Hold {
    points: Vec<(f32, Note)>,
    voice: u32,
    active: bool,
}

pub struct Sounds {
    // Descriptors borrow these allocations. They are never resized after load.
    _pcm: Vec<Vec<f32>>,
    clips: Vec<AppSound>,
    cues: HashMap<String, Cue>,
    commands: Vec<AppSoundCommand>,
    sequence: u32,
    pairs: HashMap<usize, usize>,
    pair_played: HashMap<usize, &'static str>,
    holds: Vec<Hold>,
    volumes: Vec<(f32, f32)>,
    presentation_cues: Vec<String>,
}

// TapEffectView.SetupTapSE / PlaySe. Long roots use perfect even when critical;
// generated Combo judgements and Miss have no tap sound.
pub fn note_cue(
    category: i32,
    critical: bool,
    judge: Judge,
) -> Option<(&'static str, &'static str)> {
    if judge == Judge::Miss {
        return None;
    }
    if category == 0 && !critical {
        return match judge {
            Judge::Auto | Judge::JustPerfect | Judge::Perfect => {
                Some(("se_live_perfect", "perfect"))
            }
            Judge::Great => Some(("se_live_great", "perfect")),
            Judge::Good => Some(("se_live_good", "good")),
            _ => None,
        };
    }
    let cue = match (category, critical) {
        (0, true) => "se_live_critical",
        (1, _) => "se_live_perfect",
        (2, false) => "se_live_connect",
        (2, true) => "se_live_connect_critical",
        (3 | 8, false) => "se_live_flick",
        (3 | 8, true) => "se_live_flick_critical",
        (4 | 6, false) => "se_live_trace",
        (4 | 6, true) => "se_live_trace_critical",
        _ => return None,
    };
    Some((cue, cue))
}

impl Sounds {
    pub fn load(project: &Path, score: &Score) -> Result<Self, String> {
        let root = project.join("resources/opensekai");
        let definition: Definition =
            serde_json::from_str(&crate::storage::read_text(&root.join("soundbank.json"))?)
                .map_err(|e| e.to_string())?;
        let mut pcm = Vec::new();
        let mut cues = HashMap::new();
        for cue in definition.cues {
            let mut clips = Vec::new();
            for clip in cue.clips {
                if Path::new(&clip.path)
                    .components()
                    .any(|c| !matches!(c, std::path::Component::Normal(_)))
                {
                    return Err("Invalid soundbank path".into());
                }
                let decoded = crate::audio::decode(&root.join(&clip.path))?;
                if decoded.len() > 30 * 96000 {
                    return Err("Sound effect exceeds 30 seconds".into());
                }
                clips.push(pcm.len() as u32);
                pcm.push(decoded);
            }
            if clips.is_empty() || !cue.gain.is_finite() || !(0.0..=2.0).contains(&cue.gain) {
                return Err("Invalid sound cue".into());
            }
            cues.insert(
                cue.name,
                Cue {
                    clips,
                    next: 0,
                    gain: cue.gain,
                },
            );
        }
        let clips = pcm
            .iter()
            .map(|v| AppSound {
                pcm: v.as_ptr(),
                frames: (v.len() / 2) as u32,
                reserved: 0,
            })
            .collect();
        let timeline = Timeline::from_events(&score.music_score_event_data_list)?;
        let mut times: HashMap<u32, Vec<usize>> = HashMap::new();
        for (i, n) in score.note_list.iter().enumerate() {
            if !n.is_skip && !n.is_decoration && matches!(n.category, 0..=4 | 6 | 8) {
                times
                    .entry(timeline.time_at(n.ticks).to_bits())
                    .or_default()
                    .push(i);
            }
        }
        let mut pairs = HashMap::new();
        for notes in times.values().filter(|n| n.len() == 2) {
            pairs.insert(notes[0], notes[1]);
            pairs.insert(notes[1], notes[0]);
        }
        let by_id: HashMap<_, _> = score.note_list.iter().map(|n| (n.id, n)).collect();
        let mut holds = Vec::new();
        for root in score.note_list.iter().filter(|n| {
            n.previous_connection_id < 0
                && n.next_connection_id >= 0
                && matches!(n.category, 1 | 6)
                && !n.is_skip
                && !n.is_decoration
        }) {
            let mut points = vec![(timeline.time_at(root.ticks), root.clone())];
            let mut note = root;
            while let Some(next) = by_id.get(&note.next_connection_id) {
                note = next;
                if !note.is_skip {
                    points.push((timeline.time_at(note.ticks), note.clone()));
                }
            }
            if points.len() > 1 {
                holds.push(Hold {
                    points,
                    voice: holds.len() as u32 + 1,
                    active: false,
                });
            }
        }
        let mut volumes: Vec<_> = score
            .music_score_event_data_list
            .iter()
            .filter(|e| e.event_type == 2)
            .map(|e| {
                (
                    timeline.time_at(e.ticks),
                    e.change_value.as_f64().unwrap_or(1.) as f32,
                )
            })
            .collect();
        volumes.sort_by(|a, b| a.0.total_cmp(&b.0));
        Ok(Self {
            _pcm: pcm,
            clips,
            cues,
            commands: Vec::new(),
            sequence: 0,
            pairs,
            pair_played: HashMap::new(),
            holds,
            volumes,
            presentation_cues: Vec::new(),
        })
    }
    fn play(&mut self, name: &str, action: u32, voice: u32, gain: f32) {
        if let Some(cue) = self.cues.get_mut(name) {
            let clip = cue.clips[cue.next];
            cue.next = (cue.next + 1) % cue.clips.len();
            self.commands.push(AppSoundCommand {
                clip,
                voice,
                action,
                gain: (gain * cue.gain).clamp(0., 1.),
            });
        }
    }
    pub fn restart(&mut self) {
        self.commands.clear();
        self.pair_played.clear();
        self.presentation_cues.clear();
        for h in &mut self.holds {
            h.active = false;
        }
        for cue in self.cues.values_mut() {
            cue.next = 0;
        }
    }
    pub fn presentation(&mut self, cues: Vec<String>) {
        self.presentation_cues.extend(cues);
    }
    pub fn frame(
        &mut self,
        score: &Score,
        session: &mut Session,
        preparing: bool,
        frame: &mut AppFrame,
    ) {
        self.commands.clear();
        let gain = self
            .volumes
            .iter()
            .rev()
            .find(|(t, _)| *t <= session.time)
            .map(|(_, v)| *v)
            .unwrap_or(1.)
            .clamp(0., 1.);
        for (index, judge) in session.sound_hits.drain(..) {
            let note = &score.note_list[index];
            if let Some((cue, group)) = note_cue(note.category, note.note_type == 1, judge) {
                let suppress = if let Some(pair) = self.pairs.get(&index) {
                    if let Some(previous) = self.pair_played.remove(pair) {
                        previous == group
                    } else {
                        self.pair_played.insert(index, group);
                        false
                    }
                } else {
                    false
                };
                if !suppress {
                    self.play(cue, 0, 0, gain);
                }
            }
        }
        for _ in 0..std::mem::take(&mut session.unpicked) {
            self.play("se_live_tap", 0, 0, gain);
        }
        let held = session.held_lanes();
        let mut transitions = Vec::new();
        for hold in &mut self.holds {
            let time = session.time;
            let mut active = !preparing
                && !session.paused
                && !session.finished
                && time >= hold.points[0].0
                && time < hold.points.last().unwrap().0;
            if active && !session.result.autoplay {
                let next = hold.points.partition_point(|(t, _)| *t <= time);
                let (ta, a) = &hold.points[next - 1];
                let (tb, b) = &hold.points[next];
                let t =
                    crate::live::ease(((time - ta) / (tb - ta)).clamp(0., 1.), a.note_line_type);
                let start = a.lane_start as f32 + (b.lane_start - a.lane_start) as f32 * t;
                let end = a.lane_end as f32 + (b.lane_end - a.lane_end) as f32 * t;
                active = (0..12).any(|lane| {
                    held & (1 << lane) != 0
                        && lane as f32 >= start - 1.25
                        && lane as f32 <= end + 1.25
                });
            }
            if active != hold.active {
                hold.active = active;
                transitions.push((hold.voice, active, hold.points[0].1.note_type == 1));
            }
        }
        for (voice, active, critical) in transitions {
            if active {
                self.play(
                    if critical {
                        "se_live_long_critical"
                    } else {
                        "se_live_long"
                    },
                    1,
                    voice,
                    gain,
                );
            } else {
                self.commands.push(AppSoundCommand {
                    voice,
                    action: 2,
                    ..AppSoundCommand::default()
                });
            }
        }
        for cue in std::mem::take(&mut self.presentation_cues) {
            self.play(&cue, 0, 0, 1.);
        }
        self.sequence = self.sequence.wrapping_add(1);
        frame.sounds = self.clips.as_ptr();
        frame.sound_count = self.clips.len() as u32;
        frame.sound_commands = self.commands.as_ptr();
        frame.sound_command_count = self.commands.len() as u32;
        frame.sound_sequence = self.sequence;
        frame.sound_state = if session.paused || preparing { 2 } else { 1 };
    }
}
