//! MusicScoreMakerUtility BPM-only time conversion, in C# single precision.
use crate::model::Event;

pub const TICKS_PER_BEAT: i64 = 480;
pub const TICKS_PER_BAR: i64 = 1920;
/// Original TryParseTimeSignature rounds legacy numeric values before /4.
pub fn time_signature(value: &serde_json::Value) -> (i32, i32) {
    let owned;
    let text = if let Some(text) = value.as_str() {
        text
    } else {
        owned = value.to_string();
        &owned
    };
    if let Some((a, b)) = text.split_once('/')
        && let (Ok(n), Ok(d)) = (a.trim().parse::<i32>(), b.trim().parse::<i32>())
        && n > 0
        && d > 0
    {
        return (n, d);
    }
    if let Ok(value) = text.trim().parse::<f32>()
        && value.is_finite()
        && value > 0.
    {
        let n = value.round_ties_even() as i32;
        if n > 0 {
            return (n, 4);
        }
        for d in [4, 8, 16, 2, 1, 32] {
            let n = d as f32 * value * 0.25;
            let rounded = n.round_ties_even() as i32;
            if rounded >= 1 && (n - rounded as f32).abs() < 0.001 {
                return (rounded, d);
            }
        }
    }
    (4, 4)
}
#[derive(Debug, Clone)]
pub struct Timeline {
    points: Vec<(i64, f32)>,
}
impl Timeline {
    pub fn from_events(events: &[Event]) -> Result<Self, String> {
        let mut sorted: Vec<_> = events.iter().collect();
        sorted.sort_by_key(|e| e.ticks);
        let mut points: Vec<(i64, f32)> = Vec::new();
        let mut bpm = 120.;
        for e in sorted {
            if !(0..=3).contains(&e.event_type) {
                return Err(format!("Unsupported event type {}", e.event_type));
            }
            if e.event_type == 0 {
                let value = e
                    .change_value
                    .as_f64()
                    .map(|v| v as f32)
                    .or_else(|| e.change_value.as_str().and_then(|v| v.parse::<f32>().ok()));
                if let Some(v) = value {
                    if !v.is_finite() {
                        return Err("BPM must be finite".into());
                    }
                    bpm = v;
                }
            }
            let point = (e.ticks, if bpm <= 0. { 120. } else { bpm });
            if points.last().is_some_and(|p| p.0 == e.ticks) {
                *points.last_mut().unwrap() = point;
            } else {
                points.push(point);
            }
        }
        if points.first().is_none_or(|p| p.0 > 0) {
            points.insert(0, (0, 120.));
        }
        Ok(Self { points })
    }
    pub fn from_bpms(points: &[(i64, f32)]) -> Result<Self, String> {
        Self::from_events(
            &points
                .iter()
                .map(|&(ticks, bpm)| Event {
                    ticks,
                    change_value: serde_json::json!(bpm),
                    ..Event::default()
                })
                .collect::<Vec<_>>(),
        )
    }
    pub fn time_at(&self, ticks: i64) -> f32 {
        let mut time = 0.;
        for (i, &(start, bpm)) in self.points.iter().enumerate() {
            let end = self.points.get(i + 1).map_or(ticks, |p| p.0);
            if ticks <= end || i + 1 == self.points.len() {
                return time + (ticks - start) as f32 / TICKS_PER_BEAT as f32 * 60. / bpm;
            }
            time += (end - start) as f32 / TICKS_PER_BEAT as f32 * 60. / bpm;
        }
        time
    }
    pub fn ticks_at(&self, seconds: f32) -> i64 {
        let mut elapsed = 0.;
        for (i, &(start, bpm)) in self.points.iter().enumerate() {
            if let Some(&(end, _)) = self.points.get(i + 1) {
                let segment = (end - start) as f32 / TICKS_PER_BEAT as f32 * 60. / bpm;
                if seconds > elapsed + segment {
                    elapsed += segment;
                    continue;
                }
            }
            // Math.Round uses ties to even; Rust round() would move these notes.
            return start
                + ((seconds - elapsed) * bpm / 60. * TICKS_PER_BEAT as f32).round_ties_even()
                    as i64;
        }
        0
    }
}
