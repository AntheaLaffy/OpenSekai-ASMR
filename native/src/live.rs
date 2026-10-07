//! Native live session. Judge windows come from local LiveConfig; all charts
//! share the native dynamic scoring rule and original note/feedback resources.
use crate::{
    ffi::{AppEvent, AppFrame},
    model::{Note, Score},
    storage::{self, Entry},
    timeline::Timeline,
    ui::Painting,
    unity::{Bundle, CANVAS, inside},
};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};

static EPOCH: AtomicU32 = AtomicU32::new(1);
pub const KEYS: &[u8] = b"asdfghjkl;'\r";
pub const KEY_LABELS: [&str; 12] = [
    "A", "S", "D", "F", "G", "H", "J", "K", "L", ";", "'", "Enter",
];
pub const FLICK_KEYS: &[u8] = b"qwertyuiop[]";
pub const FLICK_LABELS: [&str; 12] = ["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "[", "]"];
// Physical lower row in keyboard order, including both Shift keys.
pub const LOWER_FLICK_KEYS: [u32; 12] = [
    1073742049, 122, 120, 99, 118, 98, 110, 109, 44, 46, 47, 1073742053,
];
pub const LOWER_FLICK_LABELS: [&str; 12] = [
    "LShift", "Z", "X", "C", "V", "B", "N", "M", ",", ".", "/", "RShift",
];
pub const DESKTOP_FLICK_WINDOW: f32 = 0.18;

#[derive(Clone, Copy, Default)]
struct KeyboardContact {
    pressed: Option<f32>,
    released: Option<f32>,
    sustained: bool,
}
#[derive(Clone, Copy, Debug)]
pub enum FlickDirection {
    Any,
    Up,
    Left,
    Right,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum Judge {
    JustPerfect,
    Perfect,
    Great,
    Good,
    Bad,
    Miss,
    Auto,
}
#[derive(Clone, Copy, Debug)]
pub struct HitFeedback {
    pub start: f32,
    pub end: f32,
    pub time: f32,
    pub critical: bool,
    pub flick: bool,
    pub hold: bool,
    pub judge: Judge,
    pub seed: u32,
    pub source_hit: bool,
}
#[derive(Clone, Copy)]
pub struct HoldFeedback {
    pub start: f32,
    pub end: f32,
    pub age: f32,
    pub critical: bool,
    pub seed: u32,
}
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Tap,
    Hold,
    Release,
    Flick,
}
#[derive(Clone)]
struct Target {
    source: Option<usize>,
    visible: bool,
    weight: u16,
    time: f32,
    start: f32,
    end: f32,
    kind: Kind,
    critical: bool,
    direction: i32,
    end_note: bool,
    hold_contact: bool,
    lane_offset: f32,
    result: Option<Judge>,
}
#[derive(Default, Clone, Serialize)]
pub struct ResultData {
    pub counts: [u32; 7],
    pub score: u32,
    pub combo: u32,
    pub max_combo: u32,
    pub life: i32,
    pub total: usize,
    pub autoplay: bool,
    pub practice_without_music: bool,
}
pub struct Session {
    targets: Vec<Target>,
    scoring: crate::scoring::Score,
    cursor: usize,
    pub time: f32,
    pub duration: f32,
    pub result: ResultData,
    pub paused: bool,
    pub finished: bool,
    pub last_judge: Option<(Judge, f32)>,
    pub(crate) combo_time: f32,
    pub(crate) score_gain: u32,
    pub(crate) score_gain_time: Option<f32>,
    pub(crate) lane_touch_times: [Option<f32>; 12],
    note_judgements: Vec<Option<Judge>>,
    pub visual_hits: Vec<HitFeedback>,
    keys: u16,
    flick_keys: [u16; 2],
    keyboard_contacts: [KeyboardContact; 12],
    pointer: u16,
    pub(crate) sound_hits: Vec<(usize, Judge)>,
    pub(crate) unpicked: u32,
}
fn judge_window(target: &Target, late: bool) -> [f32; 4] {
    let v = if target.end_note {
        if late {
            [4., 8., 8.5, 8.5]
        } else {
            [3.5, 6.5, 7.5, 7.5]
        }
    } else if target.kind == Kind::Flick || target.kind == Kind::Hold {
        [
            if target.critical { 3.5 } else { 2.5 },
            if late { 7.5 } else { 6.5 },
            if late { 8. } else { 7. },
            if late { 8.5 } else { 7.5 },
        ]
    } else if target.critical {
        [3.3, 4.5, 6.5, 7.5]
    } else {
        [2.5, 5., 6.5, 7.5]
    };
    v.map(|v| v / 60.)
}
fn judgement(target: &Target, offset: f32, direction: Option<i32>) -> Option<Judge> {
    let windows = judge_window(target, offset > 0.);
    let difference = offset.abs();
    if difference > windows[3] {
        return None;
    }
    if target.kind == Kind::Flick
        && target.direction != 0
        && direction.is_some_and(|direction| target.direction != direction)
        && difference <= windows[0]
    {
        return Some(Judge::Great);
    }
    Some(if difference <= 1. / 120. {
        Judge::JustPerfect
    } else if difference <= windows[0] {
        Judge::Perfect
    } else if difference <= windows[1] {
        Judge::Great
    } else if difference <= windows[2] {
        Judge::Good
    } else {
        Judge::Bad
    })
}
fn lane_match(target: &Target, mask: u16) -> bool {
    (0..12).any(|lane| {
        mask & (1 << lane) != 0
            && lane as f32 >= target.start - target.lane_offset
            && lane as f32 <= target.end + target.lane_offset
    })
}
impl Session {
    pub fn new(score: &Score, autoplay: bool, practice: bool) -> Result<Self, String> {
        let issues = score.validate();
        if !issues.is_empty() {
            return Err(issues.join("; "));
        }
        let timeline = Timeline::from_events(&score.music_score_event_data_list)?;
        let mut targets = Vec::new();
        for (index, note) in score.note_list.iter().enumerate() {
            if note.is_decoration
                || matches!(note.category, 5 | 7 | 9..=11 | 13 | 14)
                || (note.category == 0 && note.is_skip)
            {
                continue;
            }
            let end_note = note.previous_connection_id >= 0 && note.next_connection_id < 0;
            let kind = if matches!(note.category, 3 | 8) {
                Kind::Flick
            } else if end_note && matches!(note.category, 0 | 1) {
                Kind::Release
            } else if note.is_skip || matches!(note.category, 2 | 4 | 6 | 12) {
                Kind::Hold
            } else {
                Kind::Tap
            };
            targets.push(Target {
                source: Some(index),
                visible: note.category != 12,
                weight: crate::scoring::weight(note.category, note.note_type == 1, false),
                time: timeline.time_at(note.ticks),
                start: note.lane_start as f32,
                end: note.lane_end as f32,
                kind,
                critical: note.note_type == 1,
                direction: note.direction,
                end_note,
                hold_contact: note.next_connection_id >= 0 || kind == Kind::Hold,
                lane_offset: if (matches!(note.category, 1 | 6) && !end_note) || kind == Kind::Flick
                {
                    1.5
                } else {
                    1.25
                },
                result: None,
            });
        }
        // LongHoldCombo uses eight subdivisions of each bar in the source.
        let by_id: HashMap<_, _> = score.note_list.iter().map(|n| (n.id, n)).collect();
        for root in score.note_list.iter().filter(|n| {
            n.previous_connection_id < 0 && matches!(n.category, 1 | 6 | 7) && !n.is_decoration
        }) {
            let mut chain = vec![root];
            while let Some(next) = by_id.get(&chain.last().unwrap().next_connection_id) {
                chain.push(*next);
            }
            let end = chain.last().unwrap();
            let mut bar_start = 0i64;
            let mut beats = 4.;
            let changes = score
                .music_score_event_data_list
                .iter()
                .filter(|e| e.event_type == 3)
                .collect::<Vec<_>>();
            while bar_start < end.ticks {
                if let Some(e) = changes
                    .iter()
                    .filter(|e| e.ticks <= bar_start)
                    .max_by_key(|e| e.ticks)
                {
                    beats = e.change_value.as_f64().unwrap_or_else(|| {
                        let (n, d) = crate::timeline::time_signature(&e.change_value);
                        n as f64 * 4. / d as f64
                    });
                }
                let length = (beats * 480.).round() as i64;
                if length <= 0 {
                    break;
                }
                for division in 0..8 {
                    let ticks = bar_start + length * division / 8;
                    if ticks <= root.ticks || ticks >= end.ticks {
                        continue;
                    }
                    let a = chain
                        .iter()
                        .rev()
                        .find(|n| n.ticks <= ticks && !n.is_skip)
                        .unwrap_or(&root);
                    let b = chain
                        .iter()
                        .find(|n| n.ticks > ticks && !n.is_skip)
                        .unwrap_or(end);
                    // LiveConfig.GetNoteLineParentProgress interpolates time,
                    // so a BPM change inside a slide must move its hit lanes too.
                    let mut t = (timeline.time_at(ticks) - timeline.time_at(a.ticks))
                        / (timeline.time_at(b.ticks) - timeline.time_at(a.ticks));
                    t = ease(t, a.note_line_type);
                    targets.push(Target {
                        source: None,
                        visible: false,
                        weight: crate::scoring::weight(12, root.note_type == 1, true),
                        time: timeline.time_at(ticks),
                        start: a.lane_start as f32 + (b.lane_start - a.lane_start) as f32 * t,
                        end: a.lane_end as f32 + (b.lane_end - a.lane_end) as f32 * t,
                        kind: Kind::Hold,
                        critical: root.note_type == 1,
                        direction: 0,
                        end_note: false,
                        hold_contact: true,
                        lane_offset: 1.25,
                        result: None,
                    });
                }
                bar_start += length;
            }
        }
        targets.sort_by(|a, b| a.time.total_cmp(&b.time));
        if targets.is_empty() {
            return Err("Chart has no playable notes".into());
        }
        // Decoration notes do not score, but their last visible event still
        // precedes the ending. Music has its own lifetime in Live.
        let duration = score
            .note_list
            .iter()
            .map(|n| timeline.time_at(n.ticks))
            .fold(0., f32::max);
        Ok(Self {
            scoring: crate::scoring::Score::new(
                &targets
                    .iter()
                    .map(|t| (t.time, t.weight))
                    .collect::<Vec<_>>(),
            ),
            result: ResultData {
                life: 1000,
                total: targets.len(),
                autoplay,
                practice_without_music: practice,
                ..ResultData::default()
            },
            targets,
            cursor: 0,
            time: -2.,
            duration,
            paused: false,
            finished: false,
            last_judge: None,
            combo_time: 0.,
            score_gain: 0,
            score_gain_time: None,
            lane_touch_times: [None; 12],
            note_judgements: vec![None; score.note_list.len()],
            visual_hits: Vec::new(),
            keys: 0,
            flick_keys: [0; 2],
            keyboard_contacts: [KeyboardContact::default(); 12],
            pointer: 0,
            sound_hits: Vec::new(),
            unpicked: 0,
        })
    }
    fn record(&mut self, i: usize, judge: Judge) {
        if self.targets[i].result.is_some() {
            return;
        }
        self.targets[i].result = Some(judge);
        if self.targets[i].hold_contact
            && matches!(judge, Judge::JustPerfect | Judge::Perfect | Judge::Great)
        {
            for lane in 0..12 {
                if self.keys & (1 << lane) != 0 && lane_match(&self.targets[i], 1 << lane) {
                    self.keyboard_contacts[lane].sustained = true;
                }
            }
        }
        if let Some(index) = self.targets[i].source {
            self.note_judgements[index] = Some(judge);
            self.sound_hits.push((index, judge));
        }
        if judge != Judge::Miss {
            self.visual_hits.retain(|hit| self.time - hit.time < 0.6);
            if self.visual_hits.len() == 128 {
                self.visual_hits.remove(0);
            }
            let target = &self.targets[i];
            self.visual_hits.push(HitFeedback {
                start: target.start,
                end: target.end,
                time: self.time,
                critical: target.critical,
                flick: target.kind == Kind::Flick,
                hold: target.hold_contact || target.end_note || target.kind == Kind::Hold,
                judge,
                seed: i as u32,
                source_hit: target.visible,
            });
            if target.visible {
                let lanes = (0..12)
                    .filter(|lane| {
                        (*lane as f32) >= target.start.floor()
                            && (*lane as f32) <= target.end.ceil()
                    })
                    .fold(0, |mask, lane| mask | (1 << lane));
                self.touch_lanes(lanes);
            }
        }
        let accuracy = match judge {
            Judge::JustPerfect | Judge::Auto => 100,
            Judge::Perfect => 90,
            Judge::Great => 70,
            Judge::Good => 50,
            _ => 0,
        };
        let gain = self.scoring.award(i, accuracy, self.result.combo);
        self.result.counts[judge as usize] += 1;
        if matches!(
            judge,
            Judge::JustPerfect | Judge::Perfect | Judge::Great | Judge::Auto
        ) {
            self.result.combo += 1;
            self.combo_time = self.time;
        } else {
            self.result.combo = 0;
        }
        self.result.max_combo = self.result.max_combo.max(self.result.combo);
        self.result.score += gain;
        // Hidden long-body checkpoints must not keep a stale +score label
        // pinned to the HUD. A visible note earns its own transient feedback.
        if gain > 0 && self.targets[i].visible {
            self.score_gain = gain;
            self.score_gain_time = Some(self.time);
        }
        self.result.life = (self.result.life
            - match judge {
                Judge::Bad => 60,
                Judge::Miss => 80,
                _ => 0,
            })
        .max(0);
        self.last_judge = Some((judge, self.time));
    }
    pub fn update(&mut self, dt: f32) {
        if self.paused || self.finished {
            return;
        }
        if dt.is_finite() && dt > 0. {
            self.time += dt;
        }
        for contact in &mut self.keyboard_contacts {
            if contact
                .released
                .is_some_and(|time| self.time - time > DESKTOP_FLICK_WINDOW)
            {
                *contact = KeyboardContact::default();
            }
        }
        let held = self.held_lanes();
        for i in self.cursor..self.targets.len() {
            let target = &self.targets[i];
            if target.time > self.time {
                break;
            }
            if target.result.is_some() {
                continue;
            }
            let delta = self.time - target.time;
            if self.result.autoplay {
                self.record(i, Judge::Auto);
            } else if target.kind == Kind::Hold && lane_match(target, held) {
                self.record(i, Judge::JustPerfect);
            } else if delta > judge_window(target, true)[3] {
                self.record(i, Judge::Miss);
            }
        }
        while self.cursor < self.targets.len() && self.targets[self.cursor].result.is_some() {
            self.cursor += 1;
        }
        // Life is a result/display value in this project; reaching zero must
        // not stop the chart, music, or subsequent input.
        if self.cursor == self.targets.len() && self.time >= self.duration {
            self.finished = true;
            self.keys = 0;
            self.flick_keys = [0; 2];
            self.keyboard_contacts = [KeyboardContact::default(); 12];
            self.pointer = 0;
        }
    }
    fn input(&mut self, mask: u16, kind: Kind, direction: i32) -> bool {
        self.input_with_direction(mask, kind, Some(direction))
    }
    fn input_with_direction(&mut self, mask: u16, kind: Kind, direction: Option<i32>) -> bool {
        if self.paused || self.finished || self.result.autoplay {
            return false;
        }
        let mut nearest = None;
        for i in self.cursor..self.targets.len() {
            let target = &self.targets[i];
            if target.time > self.time + 0.15 {
                break;
            }
            if target.result.is_some() || target.kind != kind || !lane_match(target, mask) {
                continue;
            }
            let offset = self.time - target.time;
            if let Some(judge) = judgement(target, offset, direction)
                && nearest.is_none_or(|(_, _, distance)| offset.abs() < distance)
            {
                nearest = Some((i, judge, offset.abs()));
            }
        }
        if let Some((i, judge, _)) = nearest {
            self.record(i, judge);
            return true;
        }
        false
    }
    pub fn key_lane(&mut self, lane: usize, down: bool) {
        if lane >= 12 || self.paused || self.finished || self.result.autoplay {
            return;
        }
        let mask = 1 << lane;
        if down && self.keys & mask == 0 {
            self.touch_lanes(mask);
            self.keyboard_contacts[lane] = KeyboardContact {
                pressed: Some(self.time),
                ..KeyboardContact::default()
            };
            self.keys |= mask;
            if !self.input(mask, Kind::Tap, 0)
                && !self.targets.iter().skip(self.cursor).any(|target| {
                    target.kind == Kind::Flick
                        && target.result.is_none()
                        && lane_match(target, mask)
                        && (target.time - self.time).abs() <= DESKTOP_FLICK_WINDOW
                })
            {
                self.unpicked += 1;
            }
        } else if !down && self.keys & mask != 0 {
            self.keys &= !mask;
            self.keyboard_contacts[lane].released = Some(self.time);
            if self.held_lanes() & mask == 0 {
                self.input(mask, Kind::Release, 0);
            }
        }
    }
    /// Upper and lower rows finish a released middle-row contact in the same
    /// column. Neither outer row can tap a note or sustain a hold.
    pub fn flick_key_lane(&mut self, lane: usize, down: bool) {
        self.flick_row_lane(lane, false, down);
    }
    pub fn flick_row_lane(&mut self, lane: usize, lower: bool, down: bool) {
        if lane >= 12 || self.paused || self.finished || self.result.autoplay {
            return;
        }
        let mask = 1 << lane;
        let row = usize::from(lower);
        if down && self.flick_keys[row] & mask == 0 {
            self.flick_keys[row] |= mask;
            let contact = std::mem::take(&mut self.keyboard_contacts[lane]);
            if self.keys & mask == 0
                && contact
                    .released
                    .is_some_and(|time| (0. ..=DESKTOP_FLICK_WINDOW).contains(&(self.time - time)))
                && contact.pressed.is_some_and(|time| {
                    contact.sustained || (0. ..=DESKTOP_FLICK_WINDOW).contains(&(self.time - time))
                })
            {
                self.flick_lanes(mask, FlickDirection::Any);
            }
        } else if !down {
            self.flick_keys[row] &= !mask;
        }
    }
    pub fn flick_lanes(&mut self, lanes: u16, direction: FlickDirection) -> bool {
        let direction = match direction {
            FlickDirection::Any => None,
            FlickDirection::Up => Some(0),
            FlickDirection::Left => Some(1),
            FlickDirection::Right => Some(2),
        };
        self.input_with_direction(lanes & 0xfff, Kind::Flick, direction)
    }
    pub fn flick(&mut self, direction: i32) {
        self.flick_lanes(
            self.held_lanes(),
            match direction {
                1 => FlickDirection::Left,
                2 => FlickDirection::Right,
                _ => FlickDirection::Up,
            },
        );
    }
    pub fn pause(&mut self) {
        self.paused = !self.paused;
        self.keys = 0;
        self.flick_keys = [0; 2];
        self.keyboard_contacts = [KeyboardContact::default(); 12];
        self.pointer = 0;
    }
    pub(crate) fn held_lanes(&self) -> u16 {
        self.keys | self.pointer
    }
    fn touch_lanes(&mut self, mask: u16) {
        for lane in 0..12 {
            if mask & (1 << lane) != 0 {
                self.lane_touch_times[lane] = Some(self.time);
            }
        }
    }
    pub fn note_judgement(&self, index: usize) -> Option<Judge> {
        self.note_judgements.get(index).copied().flatten()
    }
}
pub(crate) fn ease(t: f32, kind: i32) -> f32 {
    match kind {
        1 => 1. - (1. - t) * (1. - t),
        2 => t * t,
        _ => t,
    }
}
pub fn display_progress(time: f32, note_time: f32, speed: f32) -> f32 {
    display_progress_at_speed(time, note_time, speed, 6.)
}
pub fn display_offset(flow_speed: f32) -> f32 {
    let flow_speed = if flow_speed.is_finite() {
        flow_speed.clamp(1., 12.)
    } else {
        6.
    };
    (1. - (1. - (flow_speed - 1.) / 11.).powf(1.31)) * -3.65 + 4.
}
pub fn display_progress_at_speed(time: f32, note_time: f32, speed: f32, flow_speed: f32) -> f32 {
    let offset = display_offset(flow_speed);
    let progress = 1. + (time - note_time) * speed.abs() / offset;
    project_progress(progress)
}
pub(crate) fn project_progress(progress: f32) -> f32 {
    let value = 1.06f32.powf((progress.min(2.) - 1.) * 45.);
    // A cutoff at 4% made notes pop 34 pixels into the lane. Let the
    // projected scale tend continuously to the horizon instead.
    if !value.is_finite() { 0. } else { value }
}
pub const HIT_Y: f32 = 859.68;
pub fn note_geometry(
    note: &Note,
    time: f32,
    note_time: f32,
    chart_speed: f32,
    flow_speed: f32,
) -> [f32; 4] {
    let speed = note.speed_ratio * chart_speed;
    let t = display_progress_at_speed(time, note_time, speed, flow_speed);
    let x = (-6.545 + (note.lane_start + note.lane_end) as f32 * 0.595) * t;
    // The note centre stays on the same projected plane as the lane. Earlier
    // code placed spawn above the viewport and abruptly enabled a full sprite.
    let y = if speed < 0. {
        1769.04 - 909.36 * t.min(1.)
    } else {
        HIT_Y * t
    };
    let w = ((note.lane_end - note.lane_start) as f32 * 1.2178 + 2.06) * 108. * t;
    let h = 1.7222 * 108. * t;
    [960. + x * 108. - w / 2., y - h / 2., w, h]
}
/// The marker is a billboard above the note plane, with its authored sprite
/// dimensions. Stretching it to the sliced base made wide flicks enormous.
pub fn flick_marker(note: [f32; 4], size: [f32; 2], time: f32) -> ([f32; 4], f32) {
    let scale = note[3] / (1.7222 * 108.);
    let phase = (time * 2.).rem_euclid(1.);
    let [w, h] = size.map(|v| v * scale);
    let lift = (24. + phase * 216.) * scale;
    let alpha = (phase / 0.08).min(1.) * (1. - ((phase - 0.5) * 2.).clamp(0., 1.));
    (
        [
            note[0] + (note[2] - w) / 2.,
            note[1] + note[3] / 2. - lift - h,
            w,
            h,
        ],
        alpha,
    )
}
pub struct Live {
    sounds: crate::live_sound::Sounds,
    ending: crate::live_result::Sequence,
    pub session: Session,
    score: Score,
    entry: Entry,
    bundle: Arc<Bundle>,
    jacket: Option<std::ffi::CString>,
    result_figure: crate::rig2d::Rig,
    times: Vec<f32>,
    high_speeds: Vec<(f32, f32)>,
    pub note_speed: f32,
    effects: crate::live_effects::Effects,
    hud: crate::live_hud::Hud,
    filler: f32,
    pub audio: Vec<f32>,
    pub epoch: u32,
    audio_ended: bool,
    pub exit: bool,
    saved: bool,
    mouse: [f32; 2],
    pub status: String,
    pub preparing: bool,
}
impl Live {
    pub fn new(project: &Path, entry: Entry, autoplay: bool) -> Result<Self, String> {
        let score = entry.load_score()?;
        let practice = !entry.has_audio();
        let mut session = Session::new(&score, autoplay, practice)?;
        let filler = if practice {
            0.
        } else {
            entry.manifest.filler_sec
        };
        session.time -= filler;
        let timeline = Timeline::from_events(&score.music_score_event_data_list)?;
        let sounds = crate::live_sound::Sounds::load(project, &score)?;
        let times = score
            .note_list
            .iter()
            .map(|n| timeline.time_at(n.ticks))
            .collect();
        let audio = if practice {
            Vec::new()
        } else {
            crate::audio::decode(&entry.asset(&entry.manifest.audio_file_name))?
        };
        let jacket_path = entry.asset(&entry.manifest.jacket_file_name);
        let jacket = if jacket_path.is_file() {
            Some(
                std::ffi::CString::new(jacket_path.to_string_lossy().as_bytes())
                    .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };
        let result_figure =
            crate::rig2d::Rig::load(&project.join("resources/generated/results/rig-v1/rig.json"))?;
        let mut high_speeds: Vec<_> = score
            .music_score_event_data_list
            .iter()
            .filter(|e| e.event_type == 1)
            .map(|e| {
                (
                    timeline.time_at(e.ticks),
                    e.change_value.as_f64().unwrap_or(1.) as f32,
                )
            })
            .collect();
        high_speeds.sort_by(|a, b| a.0.total_cmp(&b.0));
        let bundle = Bundle::load(project)?;
        Ok(Self {
            effects: crate::live_effects::Effects::load(project, &bundle)?,
            hud: crate::live_hud::Hud::load(&bundle)?,
            sounds,
            ending: crate::live_result::Sequence::default(),
            session,
            score,
            entry,
            bundle,
            jacket,
            result_figure,
            times,
            high_speeds,
            note_speed: 6.,
            filler,
            audio,
            epoch: EPOCH.fetch_add(1, Ordering::Relaxed),
            audio_ended: false,
            exit: false,
            saved: false,
            mouse: [0.; 2],
            status: String::new(),
            preparing: true,
        })
    }
    pub fn update(&mut self, dt: f32) {
        if self.preparing {
            return;
        }
        let step = if self.audio.is_empty() || self.session.time < -self.filler || self.audio_ended
        {
            dt
        } else {
            0.
        };
        if self.session.finished && !self.session.paused && step.is_finite() && step > 0. {
            // Keep transient note/judge visuals aging during a silent ending,
            // just as the consumed music clock does when a WAV is present.
            self.session.time += step;
        } else {
            self.session.update(step);
        }
        self.sounds.presentation(self.ending.advance(
            if self.session.paused { 0. } else { dt },
            self.session.finished,
            &self.session.result,
            &self.bundle.presentations,
        ));
        if self.session.finished && !self.saved {
            let record = serde_json::json!({"chartId":self.entry.manifest.id,"scoringRule":crate::scoring::RULE,"result":self.session.result,"elapsed":self.session.time});
            if let Err(e) = storage::atomic_write(
                &self.entry.directory.join("native-last-result.json"),
                &serde_json::to_vec_pretty(&record).unwrap(),
            ) {
                self.status = e;
            }
            self.saved = true;
        }
    }
    pub fn audio_frame(&mut self, frame: &mut AppFrame) {
        frame.audio_epoch = self.epoch;
        self.sounds
            .frame(&self.score, &mut self.session, self.preparing, frame);
        if !self.audio.is_empty() && self.result_age().is_none() && !self.exit {
            frame.audio_pcm = self.audio.as_ptr();
            frame.audio_frames = (self.audio.len() / 2) as u32;
            frame.audio_state = if self.session.paused || self.session.time < -self.filler {
                2
            } else {
                1
            };
        }
    }
    pub fn result_ready(&self) -> bool {
        self.ending
            .elapsed()
            .is_some_and(|t| t >= 7. + crate::live_result_ui::SETTLED)
    }
    pub fn result_age(&self) -> Option<f32> {
        self.ending.elapsed().filter(|t| *t >= 7.).map(|t| t - 7.)
    }
    fn restart(&mut self) {
        if let Ok(session) = Session::new(
            &self.score,
            self.session.result.autoplay,
            self.audio.is_empty(),
        ) {
            self.session = session;
            self.session.time -= self.filler;
            self.epoch = EPOCH.fetch_add(1, Ordering::Relaxed);
            self.saved = false;
            self.audio_ended = false;
            self.sounds.restart();
            self.ending = crate::live_result::Sequence::default();
        }
    }
    pub fn event(&mut self, e: &AppEvent) -> bool {
        if e.kind == 16 {
            self.preparing = false;
            return true;
        }
        if e.kind == 8 {
            if self.session.paused != (e.key == 0) {
                self.session.pause();
            }
            return true;
        }
        if e.kind == 13 {
            if e.key == 2 {
                self.status = "音频输出失败 / Audio output failed".into();
                if !self.session.paused {
                    self.session.pause();
                }
            } else if !self.session.paused && self.session.time >= -self.filler && e.x.is_finite() {
                self.session.time = self.session.time.max(e.x - self.filler);
                self.audio_ended = e.key == 1;
            }
            return true;
        }
        if e.kind == 14 {
            if !self.session.paused && !self.session.finished {
                self.session.pause();
            }
            return true;
        }
        if e.kind == 5 || e.kind == 12 {
            let down = e.kind == 5;
            if self.session.finished && !self.result_ready() {
                return true;
            }
            if down && e.key == 27 && !self.session.finished {
                self.session.pause();
                return true;
            }
            if self.session.paused || self.session.finished {
                if down {
                    match e.key {
                        13 => {
                            if self.session.finished {
                                self.exit = true;
                            } else {
                                self.session.pause();
                            }
                        }
                        114 => self.restart(),
                        8 => self.exit = true,
                        _ => {}
                    }
                }
                return true;
            }
            if let Some(lane) = KEYS.iter().position(|k| *k as u32 == e.key) {
                self.session.key_lane(lane, down);
            }
            if let Some(lane) = FLICK_KEYS.iter().position(|k| *k as u32 == e.key) {
                self.session.flick_key_lane(lane, down);
            }
            if let Some(lane) = LOWER_FLICK_KEYS.iter().position(|k| *k == e.key) {
                self.session.flick_row_lane(lane, true, down);
            }
            return true;
        }
        if (1..=3).contains(&e.kind) {
            if self.session.finished && !self.result_ready() {
                return true;
            }
            if e.kind == 1 && e.key == 1 {
                if self.result_ready() {
                    if inside(crate::live_result_ui::RETRY, e.x, e.y) {
                        self.restart();
                    } else if inside(crate::live_result_ui::RETURN, e.x, e.y) {
                        self.exit = true;
                    }
                    return true;
                }
                if !self.session.finished && inside([1740., 15., 160., 110.], e.x, e.y) {
                    self.session.pause();
                    return true;
                }
                if self.session.paused || self.session.finished {
                    if inside([560., 680., 800., 72.], e.x, e.y) {
                        self.restart();
                    } else if inside([560., 775., 800., 72.], e.x, e.y) {
                        self.exit = true;
                    } else if inside([560., 585., 800., 72.], e.x, e.y) && !self.session.finished {
                        self.session.pause();
                    }
                    return true;
                }
                self.mouse = [e.x, e.y];
                if e.y > 650. {
                    let lane = ((e.x - 960.) / 128.52 + 6.).floor() as i32;
                    if (0..12).contains(&lane) {
                        self.session.pointer = 1 << lane;
                        self.session.touch_lanes(1 << lane);
                        if !self.session.input(1 << lane, Kind::Tap, 0)
                            && !self.session.result.autoplay
                        {
                            self.session.unpicked += 1;
                        }
                    }
                }
            } else if e.kind == 2 && e.key == 1 {
                self.session.input(self.session.pointer, Kind::Release, 0);
                self.session.pointer = 0;
            } else if e.kind == 3 && self.session.pointer != 0 {
                let delta = [e.x - self.mouse[0], e.y - self.mouse[1]];
                let lane = ((e.x - 960.) / 128.52 + 6.).floor() as i32;
                let previous_pointer = self.session.pointer;
                self.session.pointer = if (0..12).contains(&lane) {
                    1 << lane
                } else {
                    0
                };
                self.session
                    .touch_lanes(self.session.pointer & !previous_pointer);
                if delta[0].abs() + delta[1].abs() > 12. {
                    self.session.flick_lanes(
                        self.session.pointer,
                        if delta[0] < -12. {
                            FlickDirection::Left
                        } else if delta[0] > 12. {
                            FlickDirection::Right
                        } else {
                            FlickDirection::Up
                        },
                    );
                    self.mouse = [e.x, e.y];
                }
            }
            return true;
        }
        false
    }
    fn sprite(&self, p: &mut Painting, guid: &str, rect: [f32; 4], sliced: bool, scale: f32) {
        self.bundle.sprite(
            p,
            guid,
            rect,
            CANVAS,
            0xffffffff,
            sliced,
            false,
            1. / scale.max(0.01),
            0.,
            false,
        );
    }
    fn geometry(&self, note: &Note, time: f32) -> [f32; 4] {
        note_geometry(
            note,
            self.session.time,
            time,
            self.chart_speed(),
            self.note_speed,
        )
    }
    fn chart_speed(&self) -> f32 {
        self.high_speeds
            .iter()
            .rev()
            .find(|(time, _)| *time <= self.session.time)
            .map_or(1., |(_, speed)| *speed)
    }
    fn hold_lines(&self, p: &mut Painting) -> (Vec<([f32; 4], bool)>, Vec<HoldFeedback>) {
        use crate::{
            ffi::AppDraw,
            live_lines::{Segment, View},
        };
        let by_id: HashMap<_, _> = self
            .score
            .note_list
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id, i))
            .collect();
        let view = View {
            time: self.session.time,
            chart_speed: self.chart_speed(),
            note_speed: self.note_speed,
        };
        let texture = self.bundle.live["line_texture"].as_str().unwrap_or("");
        let Some(path) = self.bundle.paths.get(texture) else {
            return (Vec::new(), Vec::new());
        };
        let mut anchors = Vec::new();
        let mut holds = Vec::new();
        for (root_index, root) in self.score.note_list.iter().enumerate().filter(|(_, n)| {
            n.previous_connection_id < 0
                && n.next_connection_id >= 0
                && matches!(n.category, 1 | 6 | 7)
        }) {
            let mut chain = vec![root_index];
            let mut current = root_index;
            let mut hops = 0;
            while let Some(&next) = by_id.get(&self.score.note_list[current].next_connection_id) {
                hops += 1;
                if hops >= self.score.note_list.len() {
                    break;
                }
                current = next;
                if !self.score.note_list[next].is_skip {
                    chain.push(next);
                }
            }
            if chain.len() < 2 {
                continue;
            }
            let tail = *chain.last().unwrap();
            if self.times[tail] <= view.time {
                continue;
            }
            let current_pair = chain
                .windows(2)
                .find(|pair| self.times[pair[1]] > view.time)
                .unwrap();
            let current_segment = Segment {
                start: &self.score.note_list[current_pair[0]],
                end: &self.score.note_list[current_pair[1]],
                times: [self.times[current_pair[0]], self.times[current_pair[1]]],
                index: 0,
                count: chain.len() - 1,
                critical: root.note_type == 1,
            };
            let lanes = current_segment.lanes(view.time);
            let mask = (0..12)
                .filter(|lane| {
                    (*lane as f32) >= lanes[0] - 0.35 && (*lane as f32) < lanes[1] + 1.35
                })
                .fold(0u16, |m, lane| m | (1 << lane));
            let contact = self.session.result.autoplay || self.session.held_lanes() & mask != 0;
            let started = self.times[root_index] <= view.time;
            let accepted = self
                .session
                .note_judgement(root_index)
                .is_some_and(|j| j != Judge::Miss);
            let pulse = if started && accepted && contact {
                0.5 - ((view.time - self.times[root_index]) * std::f32::consts::TAU * 2.).cos()
                    * 0.5
            } else {
                0.
            };
            let color = if started && !contact {
                0x99b4a7ff
            } else {
                0xffffffff
            };
            for (index, pair) in chain.windows(2).enumerate() {
                let segment = Segment {
                    start: &self.score.note_list[pair[0]],
                    end: &self.score.note_list[pair[1]],
                    times: [self.times[pair[0]], self.times[pair[1]]],
                    index,
                    count: chain.len() - 1,
                    critical: root.note_type == 1,
                };
                for strip in segment.mesh(&view) {
                    let xs = [strip.xy[0], strip.xy[2], strip.xy[4], strip.xy[6]];
                    let ys = [strip.xy[1], strip.xy[3], strip.xy[5], strip.xy[7]];
                    let x = xs.into_iter().fold(f32::INFINITY, f32::min);
                    let y = ys.into_iter().fold(f32::INFINITY, f32::min);
                    p.draws.push(AppDraw {
                        kind: 3,
                        rgba: color,
                        rect: [
                            x,
                            y,
                            xs.into_iter().fold(f32::NEG_INFINITY, f32::max) - x,
                            ys.into_iter().fold(f32::NEG_INFINITY, f32::max) - y,
                        ],
                        xy: strip.xy,
                        uv: strip.uv,
                        clip: [0., 0., 1920., HIT_Y],
                        text: path.as_ptr(),
                        reserved: 8,
                        font_size: pulse,
                        min_font_size: -0.25,
                        ..AppDraw::default()
                    });
                }
            }
            if started && accepted && contact && root.category == 1 {
                let w = ((lanes[1] - lanes[0]) * 1.2178 + 2.06) * 108.;
                let h = 1.7222 * 108.;
                let x = 960. + (-6.545 + (lanes[0] + lanes[1]) * 0.595) * 108.;
                anchors.push(([x - w / 2., HIT_Y - h / 2., w, h], root.note_type == 1));
            }
            if started && accepted && contact && matches!(root.category, 1 | 6) {
                holds.push(HoldFeedback {
                    start: lanes[0],
                    end: lanes[1],
                    age: view.time - self.times[root_index],
                    critical: root.note_type == 1,
                    seed: root_index as u32,
                });
            }
        }
        (anchors, holds)
    }
    fn plane_sprite(&self, p: &mut Painting, guid: &str, rect: [f32; 4]) {
        let first = p.draws.len();
        self.sprite(p, guid, rect, true, rect[3] / 1.7222);
        let centre_y = rect[1] + rect[3] / 2.;
        for draw in &mut p.draws[first..] {
            let [x, y, w, h] = draw.rect;
            let points = [[x, y + h], [x + w, y + h], [x + w, y], [x, y]];
            for (i, [px, py]) in points.into_iter().enumerate() {
                draw.xy[i * 2] = 960. + (px - 960.) * (py / centre_y.max(1.));
                draw.xy[i * 2 + 1] = py;
            }
            draw.kind = 3;
            let xs = [draw.xy[0], draw.xy[2], draw.xy[4], draw.xy[6]];
            let ys = [draw.xy[1], draw.xy[3], draw.xy[5], draw.xy[7]];
            let left = xs.into_iter().fold(f32::INFINITY, f32::min);
            let top = ys.into_iter().fold(f32::INFINITY, f32::min);
            draw.rect = [
                left,
                top,
                xs.into_iter().fold(f32::NEG_INFINITY, f32::max) - left,
                ys.into_iter().fold(f32::NEG_INFINITY, f32::max) - top,
            ];
        }
    }
    pub fn draw(&self, p: &mut Painting, english: bool) {
        if self.preparing {
            self.effects.warm(p);
            // Emit resource requests before the host starts the gameplay clock.
            // Transparent draws still resolve textures and shape their own text runs.
            let mut textures = std::collections::BTreeSet::new();
            for guid in self.bundle.live["notes"]
                .as_object()
                .into_iter()
                .flat_map(|v| v.values())
                .chain(
                    self.bundle.live["arrows"]
                        .as_object()
                        .into_iter()
                        .flat_map(|v| v.values())
                        .flat_map(|v| v.as_array().into_iter().flatten()),
                )
            {
                if let Some(sprite) = guid.as_str().and_then(|g| self.bundle.sprites.get(g)) {
                    textures.insert(sprite.texture.as_str());
                }
            }
            if let Some(texture) = self.bundle.live["line_texture"].as_str() {
                textures.insert(texture);
            }
            for presentation in self.bundle.presentations.results.values() {
                textures.extend(presentation.texture_paths(&self.bundle));
            }
            textures.extend(self.bundle.live_background.texture_paths(&self.bundle));
            for texture in textures {
                p.draws.push(crate::ffi::AppDraw {
                    kind: 2,
                    rect: [0., 0., 1., 1.],
                    clip: CANVAS,
                    uv: [0., 0., 1., 1.],
                    text: self.bundle.paths[texture].as_ptr(),
                    ..crate::ffi::AppDraw::default()
                });
            }
            if let Some(jacket) = &self.jacket {
                p.draws.push(crate::ffi::AppDraw {
                    kind: 2,
                    rect: [0., 0., 1., 1.],
                    clip: CANVAS,
                    uv: [0., 0., 1., 1.],
                    text: jacket.as_ptr(),
                    ..crate::ffi::AppDraw::default()
                });
            }
            p.draws.push(crate::ffi::AppDraw {
                kind: 2,
                rect: [0., 0., 1., 1.],
                clip: CANVAS,
                uv: [0., 0., 1., 1.],
                text: self.result_figure.texture().as_ptr(),
                ..crate::ffi::AppDraw::default()
            });
            for text in [
                "0123456789",
                "LIFE 0123456789",
                "0123456789 MAX COMBO JUST PERFECT GREAT GOOD BAD MISS AUTO",
                "PAUSED RESULT NO MUSIC · PRACTICE Enter · Continue R · Retry Backspace · Return",
                "无音乐练习 演奏结果 演奏完成 演奏结束 已暂停 继续 重试 退格键 返回选曲 音频输出失败",
            ] {
                for bold in [true, false] {
                    p.text(text, [0., 0., 1920., 1.], 1., bold, 0, 0, CANVAS);
                }
            }
        }
        if let Some(elapsed) = self.result_age() {
            crate::live_result_ui::View {
                bundle: &self.bundle,
                manifest: &self.entry.manifest,
                result: &self.session.result,
                jacket: self.jacket.as_deref(),
                figure: Some(&self.result_figure),
                english,
                elapsed,
            }
            .draw(p);
            return;
        }
        p.rect(CANVAS, 0x182230ff, CANVAS);
        self.bundle
            .live_background
            .draw(&self.bundle, self.jacket.as_deref(), p);
        if let Some(items) = self.bundle.live["static"].as_array() {
            for item in items {
                let rect = std::array::from_fn(|i| item["rect"][i].as_f64().unwrap_or(0.) as f32);
                self.sprite(
                    p,
                    item["guid"].as_str().unwrap_or(""),
                    rect,
                    item["sliced"].as_bool().unwrap_or(false),
                    108.,
                );
            }
        }
        self.effects.draw_lanes(p, &self.session);
        let (anchors, holds) = self.hold_lines(p);
        let mut visible = self
            .score
            .note_list
            .iter()
            .enumerate()
            .filter(|(i, n)| {
                self.times[*i] >= self.session.time - 0.15
                    && self.session.note_judgement(*i).is_none()
                    && !n.is_skip
                    && !matches!(n.category, 5 | 7 | 9..=14)
            })
            .collect::<Vec<_>>();
        visible.sort_by(|a, b| self.times[b.0].total_cmp(&self.times[a.0]));
        for (i, note) in visible {
            let rect = self.geometry(note, self.times[i]);
            if rect[2] < 0.25 || rect[1] > 1080. || rect[1] + rect[3] < 0. {
                continue;
            }
            let kind = match note.category {
                0 if note.previous_connection_id >= 0 => "Long",
                1 | 6 => "Long",
                2 => "Connection",
                3 => "Flick",
                4 => "Friction",
                8 => "FrictionFlick",
                _ => "Normal",
            };
            let name = format!("{kind}{}Note", if note.note_type == 1 { "Crt" } else { "" });
            let guid = self.bundle.live["notes"][&name].as_str().unwrap_or("");
            self.plane_sprite(p, guid, rect);
            if matches!(note.category, 3 | 8) {
                let key = format!(
                    "{name}.{}",
                    if note.direction == 0 {
                        "defaultArrowSprites"
                    } else {
                        "slanArrowSprites"
                    }
                );
                if let Some(arrows) = self.bundle.live["arrows"][&key].as_array()
                    && !arrows.is_empty()
                {
                    let guid = arrows
                        [((note.lane_end - note.lane_start) as usize).min(arrows.len() - 1)]
                    .as_str()
                    .unwrap_or("");
                    let Some(sprite) = self.bundle.sprites.get(guid) else {
                        continue;
                    };
                    let scale = rect[3] / (1.7222 * 108.);
                    let size =
                        [sprite.rect[2], sprite.rect[3]].map(|n| n * 108. / sprite.pixels_per_unit);
                    let (arrow_rect, alpha) = flick_marker(rect, size, self.session.time);
                    let first = p.draws.len();
                    self.bundle.sprite(
                        p,
                        guid,
                        arrow_rect,
                        CANVAS,
                        0xffffff00 | (alpha * 255.).round() as u32,
                        false,
                        false,
                        scale,
                        match note.direction {
                            1 => 15f32.to_radians(),
                            2 => -15f32.to_radians(),
                            _ => 0.,
                        },
                        false,
                    );
                    if note.direction == 2 {
                        for draw in &mut p.draws[first..] {
                            draw.uv.swap(0, 2);
                        }
                    }
                }
            }
        }
        for (rect, critical) in anchors {
            let name = if critical { "LongCrtNote" } else { "LongNote" };
            self.plane_sprite(
                p,
                self.bundle.live["notes"][name].as_str().unwrap_or(""),
                rect,
            );
        }
        self.effects.draw(p, &self.session, &holds);
        self.hud.draw(&self.bundle, p, &self.session);
        let mode = if self.audio.is_empty() {
            if english {
                "NO MUSIC · PRACTICE"
            } else {
                "无音乐练习"
            }
        } else if english {
            "LIVE"
        } else {
            "演奏"
        };
        p.text(
            &format!(
                "{}  [{}]  ·  {} {:.1}",
                self.entry.manifest.title,
                self.entry.manifest.music_difficulty_type.to_uppercase(),
                if english { "SPEED" } else { "流速" },
                self.note_speed
            ),
            [40., 1040., 1840., 34.],
            18.,
            false,
            1,
            0xa1b8d5ff,
            CANVAS,
        );
        for (lane, label) in KEY_LABELS.iter().enumerate() {
            let x = 188.88 + lane as f32 * 128.52;
            for (row, y) in [(0, 775.), (1, 926.)] {
                if self.session.flick_keys[row] & (1 << lane) != 0 {
                    p.rect([x, y, 128.52, 40.], 0xff8bd940, CANVAS);
                }
            }
            p.text(
                FLICK_LABELS[lane],
                [x, 775., 128.52, 40.],
                22.,
                true,
                1,
                0xffb3eaff,
                CANVAS,
            );
            p.text(
                label,
                [x, 838., 128.52, 40.],
                24.,
                false,
                1,
                0xc3d7eaff,
                CANVAS,
            );
            p.text(
                LOWER_FLICK_LABELS[lane],
                [x, 926., 128.52, 40.],
                22.,
                true,
                1,
                0xffb3eaff,
                CANVAS,
            );
        }
        p.text(
            if english {
                "Middle: tap / hold · Flick: press middle, release, then same-column upper / lower · Esc: pause"
            } else {
                "中排普通／长条 · 划动：中排按下、松开，迅速按同列上排或下排 · Esc 暂停"
            },
            [100., 990., 1720., 45.],
            21.,
            false,
            1,
            0xc3d7eaff,
            CANVAS,
        );
        if self.session.time < 0. {
            p.text(
                &format!("{}", (-self.session.time).ceil() as i32),
                [810., 350., 300., 200.],
                120.,
                true,
                1,
                0xffffffff,
                CANVAS,
            );
        }
        self.ending.draw(&self.bundle, p);
        if self.session.paused || self.result_ready() {
            p.rect(CANVAS, 0x000000bd, CANVAS);
            p.rect([480., 150., 960., 740.], 0x263044f5, CANVAS);
            p.text(
                if self.session.finished {
                    if english { "RESULT" } else { "演奏结果" }
                } else {
                    if english { "PAUSED" } else { "已暂停" }
                },
                [540., 175., 840., 90.],
                56.,
                true,
                1,
                0xffffffff,
                CANVAS,
            );
            let report = format!(
                "{}   MAX COMBO {}\nJUST {}  PERFECT {}  GREAT {}\nGOOD {}  BAD {}  MISS {}  AUTO {}",
                self.session.result.score,
                self.session.result.max_combo,
                self.session.result.counts[0],
                self.session.result.counts[1],
                self.session.result.counts[2],
                self.session.result.counts[3],
                self.session.result.counts[4],
                self.session.result.counts[5],
                self.session.result.counts[6]
            );
            for (i, line) in report.lines().enumerate() {
                p.text(
                    line,
                    [540., 290. + i as f32 * 65., 840., 60.],
                    30.,
                    true,
                    1,
                    0xffffffff,
                    CANVAS,
                );
            }
            p.text(
                mode,
                [550., 495., 820., 60.],
                28.,
                true,
                1,
                0xffcf70ff,
                CANVAS,
            );
            for (label, y) in [
                (
                    if english {
                        "Enter · Continue"
                    } else {
                        "Enter · 继续"
                    },
                    585.,
                ),
                (if english { "R · Retry" } else { "R · 重试" }, 680.),
                (
                    if english {
                        "Backspace · Return"
                    } else {
                        "退格键 · 返回选曲"
                    },
                    775.,
                ),
            ] {
                if y == 585. && self.session.finished {
                    continue;
                }
                p.rect([560., y, 800., 72.], 0x385575ff, CANVAS);
                p.text(
                    label,
                    [570., y, 780., 72.],
                    32.,
                    true,
                    1,
                    0xffffffff,
                    CANVAS,
                );
            }
        }
        if !self.status.is_empty() {
            p.text(
                &self.status,
                [20., 205., 1880., 55.],
                24.,
                true,
                0,
                0xff9999ff,
                CANVAS,
            );
        }
    }
}
