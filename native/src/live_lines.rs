//! Live NoteLineView bands, distinct from the chart maker's preview mesh.
use crate::{
    live::{HIT_Y, display_offset, project_progress},
    model::Note,
};

pub struct Segment<'a> {
    pub start: &'a Note,
    pub end: &'a Note,
    pub times: [f32; 2],
    pub index: usize,
    pub count: usize,
    pub critical: bool,
}
pub struct View {
    pub time: f32,
    pub chart_speed: f32,
    pub note_speed: f32,
}
#[derive(Clone, Copy)]
pub struct Point {
    pub centre: f32,
    pub y: f32,
    pub half_width: f32,
    pub border: f32,
    pub line_progress: f32,
}
pub struct Strip {
    pub xy: [f32; 8],
    pub uv: [f32; 4],
}
impl Segment<'_> {
    pub fn lanes(&self, time: f32) -> [f32; 2] {
        let duration = self.times[1] - self.times[0];
        let t = if duration > 0. {
            (time - self.times[0]) / duration
        } else {
            1.
        };
        let t = crate::live::ease(t.clamp(0., 1.), self.start.note_line_type);
        std::array::from_fn(|i| {
            let a = if i == 0 {
                self.start.lane_start
            } else {
                self.start.lane_end
            } as f32;
            let b = if i == 0 {
                self.end.lane_start
            } else {
                self.end.lane_end
            } as f32;
            a + (b - a) * t
        })
    }
    fn point(&self, view: &View, progress: f32, local: f32) -> Point {
        let lanes = self.lanes(self.times[0] + local * (self.times[1] - self.times[0]));
        let negative = self.start.speed_ratio * view.chart_speed < 0.;
        Point {
            centre: 960. + (-6.545 + (lanes[0] + lanes[1]) * 0.595) * 108. * progress,
            y: if negative {
                1769.04 - 909.36 * progress
            } else {
                HIT_Y * progress
            },
            half_width: progress * (((lanes[1] - lanes[0] + 1.) * 1.19) - 0.1) * 54.,
            border: progress * 0.4 * 108.,
            line_progress: (local + self.index as f32) / self.count.max(1) as f32,
        }
    }
    /// The body and its end blocks share one projection. Clip musical time at
    /// the hit line and the actual horizon; clamping raw progress to zero moves
    /// an unseen tail into the lane before its block arrives.
    pub fn mesh(&self, view: &View) -> Vec<Strip> {
        let duration = self.times[1] - self.times[0];
        if duration <= 0. || view.time >= self.times[1] {
            return Vec::new();
        }
        let offset = display_offset(view.note_speed);
        let a_offset = offset / (self.start.speed_ratio * view.chart_speed).abs().max(0.001);
        let b_offset = offset / (self.end.speed_ratio * view.chart_speed).abs().max(0.001);
        let a = 1. + (view.time - self.times[0]) / a_offset;
        let b = 1. + (view.time - self.times[1]) / b_offset;
        let far_progress = 0.25 / HIT_Y;
        let far_raw = 1. + far_progress.ln() / (45. * 1.06f32.ln());
        if a <= b || a <= far_raw || !a.is_finite() || !b.is_finite() {
            return Vec::new();
        }
        let local_start = ((1. - a) / (b - a)).clamp(0., 1.);
        let local_end = ((far_raw - a) / (b - a)).clamp(0., 1.);
        let near = project_progress(a.min(1.));
        let far = project_progress(b.max(far_raw));
        let samples = ((near - far) * 100.).ceil().max(1.) as usize;
        let point = |i: usize| {
            let t = i as f32 / samples as f32;
            let progress = near + (far - near) * t;
            let raw = 1. + progress.ln() / (45. * 1.06f32.ln());
            let local = if i == 0 {
                local_start
            } else if i == samples {
                local_end
            } else {
                ((raw - a) / (b - a)).clamp(local_start, local_end)
            };
            self.point(view, progress, local)
        };
        let bands = [([0., 0.125], 0), ([0.125, 0.875], 1), ([0.875, 1.], 2)];
        let offset_y = if self.critical { 0.025 } else { 0.525 };
        let mut result = Vec::with_capacity(samples * 3);
        for i in 0..samples {
            let start = point(i);
            let end = point(i + 1);
            if start.y == end.y || start.half_width == 0. || end.half_width == 0. {
                continue;
            }
            let edges = |p: Point| {
                [
                    p.centre - p.half_width - p.border,
                    p.centre - p.half_width,
                    p.centre + p.half_width,
                    p.centre + p.half_width + p.border,
                ]
            };
            let s = edges(start);
            let e = edges(end);
            for (uv, band) in bands {
                // Same diagonal as Unity's start-right/end-left triangles.
                result.push(Strip {
                    xy: [
                        s[band + 1],
                        start.y,
                        s[band],
                        start.y,
                        e[band],
                        end.y,
                        e[band + 1],
                        end.y,
                    ],
                    uv: [
                        uv[1],
                        1. - (end.line_progress * 0.2 + offset_y),
                        uv[0],
                        1. - (start.line_progress * 0.2 + offset_y),
                    ],
                });
            }
        }
        result
    }
}
