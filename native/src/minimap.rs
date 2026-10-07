//! MusicScoreMinimapView's point-sampled 72-pixel score thumbnail.
use crate::{
    model::{Note, Score},
    timeline::time_signature,
    ui::Painting,
};
use std::collections::HashMap;

pub const WIDTH: usize = 72;
const WINDOW: i64 = 30 * 1920;

pub struct Minimap {
    pub start: i64,
    pub end: i64,
    pub range: i64,
    pub height: usize,
    /// Unity Texture2D order: row zero is the bottom row.
    pub pixels: Vec<u32>,
}
impl Minimap {
    pub fn new(score: &Score, focus: i64, max_ticks: i64) -> Self {
        let offset = if max_ticks < 1 {
            0
        } else {
            (focus as f32 / max_ticks as f32 * WINDOW as f32) as i64
        };
        let mut start = (focus - offset).max(0);
        let mut end = start + WINDOW;
        let mut range = WINDOW;
        if max_ticks >= 1 && end > max_ticks {
            start = (max_ticks - WINDOW).max(0);
            end = max_ticks;
        }
        if (1..WINDOW).contains(&max_ticks) {
            start = 0;
            end = max_ticks;
            range = max_ticks;
        }
        let height = (range / 240).clamp(64, 4096) as usize;
        let mut map = Self {
            start,
            end,
            range,
            height,
            pixels: vec![0; WIDTH * height],
        };
        let mut signatures: Vec<_> = score
            .music_score_event_data_list
            .iter()
            .filter(|e| e.event_type == 3)
            .collect();
        signatures.sort_by_key(|e| e.ticks);
        let (mut segment, mut numerator, mut denominator) = (0, 4, 4);
        for event in signatures {
            if event.ticks > segment {
                map.bar_lines(segment, event.ticks, numerator, denominator);
            }
            (numerator, denominator) = time_signature(&event.change_value);
            segment = event.ticks;
        }
        map.bar_lines(segment, end, numerator, denominator);
        let ids: HashMap<_, _> = score.note_list.iter().map(|n| (n.id, n)).collect();
        for note in &score.note_list {
            if let Some(next) = ids.get(&note.next_connection_id) {
                map.band(note, next);
            }
            map.note(note);
        }
        map
    }
    fn y(&self, ticks: i64) -> i64 {
        (ticks - self.start) * self.height as i64 / self.range
    }
    fn ticks(&self, y: i64) -> i64 {
        self.start + self.range * y / self.height as i64
    }
    fn row(&mut self, y: i64, lane_start: i32, lane_end: i32, color: u32) {
        if !(0..self.height as i64).contains(&y) {
            return;
        }
        let start = (lane_start * 6).clamp(0, WIDTH as i32) as usize;
        let end = (lane_end * 6 + 6).clamp(0, WIDTH as i32) as usize;
        if end >= start {
            self.pixels[y as usize * WIDTH + start..y as usize * WIDTH + end].fill(color);
        }
    }
    fn bar_lines(&mut self, start: i64, end: i64, numerator: i32, denominator: i32) {
        if denominator < 1 {
            return;
        }
        let bar = 1920 * numerator as i64 / denominator as i64;
        if bar < 1 {
            return;
        }
        let mut ticks = start;
        if self.start > ticks {
            ticks += (self.start - ticks) / bar * bar;
        }
        while ticks <= end && ticks <= self.end {
            let y = self.y(ticks);
            if (0..self.height as i64).contains(&y) {
                for pixel in &mut self.pixels[y as usize * WIDTH..(y as usize + 1) * WIDTH] {
                    if *pixel & 255 == 0 {
                        *pixel = 0xffffff5a;
                    }
                }
            }
            ticks += bar;
        }
    }
    fn note(&mut self, note: &Note) {
        if note.is_skip
            || matches!(note.category, 2 | 5 | 7 | 11 | 13)
            || note.ticks < self.start
            || note.ticks > self.end
        {
            return;
        }
        let y = self.y(note.ticks).clamp(0, self.height as i64 - 2);
        let color = if note.note_type == 1 {
            0xffe75cff
        } else {
            match note.category {
                1 | 2 | 4..=7 => 0x61e894ff,
                3 | 8 => 0xff77d2ff,
                9..=11 => 0x96aaffff,
                _ => 0x59d6ffff,
            }
        };
        for dy in 0..2 {
            self.row(y + dy, note.lane_start, note.lane_end, color);
        }
    }
    fn band(&mut self, note: &Note, next: &Note) {
        let start = self.start.max(note.ticks);
        let end = self.end.min(next.ticks);
        if end < self.start || start > self.end || end <= start {
            return;
        }
        let y_start = self.y(start).clamp(0, self.height as i64 - 1);
        let y_end = self.y(end).clamp(0, self.height as i64 - 1);
        let color = if note.note_type == 1 {
            0xffd04cb4
        } else {
            0x48b880b4
        };
        for y in y_start.min(y_end)..=y_start.max(y_end) {
            let rate = (self.ticks(y) - note.ticks) as f32 / (next.ticks - note.ticks) as f32;
            let rate = easing(rate, note.note_line_type).clamp(0., 1.);
            let lane = |a: i32, b: i32| (a as f32 + (b - a) as f32 * rate).round_ties_even() as i32;
            self.row(
                y,
                lane(note.lane_start, next.lane_start),
                lane(note.lane_end, next.lane_end),
                color,
            );
        }
    }
    pub fn focus_at(&self, rect: [f32; 4], y: f32, max_ticks: i64) -> i64 {
        let normalized = if rect[3] > 0. {
            (1. - (y - rect[1]) / rect[3]).clamp(0., 1.)
        } else {
            0.
        };
        (self.start + (normalized * self.range as f32) as i64).clamp(0, max_ticks.max(0))
    }
    pub fn draw(&self, paint: &mut Painting, rect: [f32; 4], clip: [f32; 4]) {
        // Equal horizontal pixel runs merge vertically. This retains the source
        // overwrite/alpha rules without adding a texture upload on every edit.
        let mut quads: Vec<([usize; 4], u32)> = Vec::new();
        let mut previous: HashMap<(usize, usize, u32), usize> = HashMap::new();
        for y in 0..self.height {
            let mut current = HashMap::new();
            let mut x = 0;
            while x < WIDTH {
                let color = self.pixels[y * WIDTH + x];
                let start = x;
                while x < WIDTH && self.pixels[y * WIDTH + x] == color {
                    x += 1;
                }
                if color & 255 == 0 {
                    continue;
                }
                let key = (start, x, color);
                let index = if let Some(&index) = previous.get(&key) {
                    quads[index].0[3] += 1;
                    index
                } else {
                    quads.push(([start, y, x - start, 1], color));
                    quads.len() - 1
                };
                current.insert(key, index);
            }
            previous = current;
        }
        let sx = rect[2] / WIDTH as f32;
        let sy = rect[3] / self.height as f32;
        for ([x, y, w, h], color) in quads {
            paint.rect(
                [
                    rect[0] + x as f32 * sx,
                    rect[1] + (self.height - y - h) as f32 * sy,
                    w as f32 * sx,
                    h as f32 * sy,
                ],
                color,
                clip,
            );
        }
    }
}
pub fn easing(t: f32, kind: i32) -> f32 {
    match kind {
        1 => 1. - (1. - t) * (1. - t),
        2 => t * t,
        _ => t,
    }
}
