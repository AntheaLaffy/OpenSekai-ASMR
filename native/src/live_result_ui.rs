//! Result layout guided by the supplied footage, using native result data.
use crate::{
    ffi::AppDraw,
    live::ResultData,
    live_result::{self, Kind},
    model::Manifest,
    ui::Painting,
    unity::{Bundle, CANVAS},
};
use std::ffi::CStr;

pub const RETRY: [f32; 4] = [1220., 965., 280., 80.];
pub const RETURN: [f32; 4] = [1538., 965., 310., 80.];
pub const SETTLED: f32 = 4.2;
fn enter(time: f32, start: f32, duration: f32) -> f32 {
    let t = ((time - start) / duration).clamp(0., 1.);
    1. - (1. - t).powi(3)
}
fn count(value: u32, time: f32, start: f32, duration: f32) -> u32 {
    let t = enter(time, start, duration);
    if t == 1. {
        value
    } else {
        (f64::from(value) * f64::from(t)).floor() as u32
    }
}
fn animate(p: &mut Painting, first: usize, offset: [f32; 2], alpha: f32) {
    for draw in &mut p.draws[first..] {
        draw.rect[0] += offset[0];
        draw.rect[1] += offset[1];
        if draw.kind == 3 {
            for point in draw.xy.as_chunks_mut::<2>().0 {
                point[0] += offset[0];
                point[1] += offset[1];
            }
        }
        draw.rgba = draw.rgba & 0xffffff00 | (((draw.rgba & 255) as f32 * alpha).round() as u32);
    }
}
fn rank(score: u32) -> &'static str {
    match score {
        0..=399999 => "D",
        400000..=649999 => "C",
        650000..=799999 => "B",
        800000..=989999 => "A",
        _ => "S",
    }
}
pub struct View<'a> {
    pub bundle: &'a Bundle,
    pub manifest: &'a Manifest,
    pub result: &'a ResultData,
    pub jacket: Option<&'a CStr>,
    pub figure: Option<&'a crate::rig2d::Rig>,
    pub english: bool,
    pub elapsed: f32,
}
impl View<'_> {
    pub fn draw(&self, p: &mut Painting) {
        let time = if self.elapsed.is_finite() {
            self.elapsed.max(0.)
        } else {
            0.
        };
        let text = |p: &mut Painting, value: &str, rect, size, color| {
            p.text(value, rect, size, true, 0, color, CANVAS)
        };
        p.rect(CANVAS, 0x171a42ff, CANVAS);
        self.bundle.live_background.draw_base(self.bundle, p);
        p.rect(CANVAS, 0x30305d9c, CANVAS);
        if time < 2.4 {
            let first = p.draws.len();
            let title = if self.result.life == 0 {
                if self.english {
                    "LIVE FINISHED"
                } else {
                    "演奏结束"
                }
            } else if self.english {
                "LIVE CLEAR!"
            } else {
                "演奏完成！"
            };
            let arrival = enter(time, 0., 0.4);
            text(p, title, [225., 335., 1340., 165.], 137., 0xffffffff);
            let first_rank = p.draws.len();
            text(
                p,
                rank(self.result.score),
                [1500., 322., 220., 206.],
                178.,
                0xcc72ffff,
            );
            text(p, "SCORERANK", [1505., 513., 175., 35.], 24., 0xcc72ffff);
            animate(
                p,
                first_rank,
                [0., 35. * (1. - enter(time, 0.55, 0.35))],
                enter(time, 0.55, 0.35),
            );
            self.gauge(p, [230., 529., 1250., 30.], enter(time, 0.2, 0.9));
            for (label, threshold) in [("C", 0.4), ("B", 0.65), ("A", 0.8), ("S", 0.99)] {
                let x = 230. + 1250. * threshold;
                p.rect([x, 515., 2., 50.], 0xffffffff, CANVAS);
                text(p, label, [x - 20., 478., 42., 35.], 24., 0xffffffff);
            }
            animate(
                p,
                first,
                [90. * (1. - arrival), -380. * enter(time, 2., 0.4)],
                arrival * (1. - enter(time, 2., 0.4)),
            );
        }
        let header_first = p.draws.len();
        text(p, "RESULT", [0., -40., 1300., 230.], 190., 0xffffff20);
        p.rect([180., 0., 1560., 176.], 0xaaaabdff, CANVAS);
        p.rect([1565., 0., 128., 176.], 0x26233fff, CANVAS);
        p.rect([220., 20., 135., 138.], 0xc833ffff, CANVAS);
        if let Some(cover) = self.jacket {
            p.draws.push(AppDraw {
                kind: 2,
                rgba: 0xffffffff,
                rect: [229., 29., 117., 120.],
                clip: CANVAS,
                uv: [0., 0., 1., 1.],
                text: cover.as_ptr(),
                ..AppDraw::default()
            });
        }
        text(
            p,
            &self.manifest.title,
            [380., 26., 630., 55.],
            38.,
            0x27253dff,
        );
        p.rect([380., 95., 206., 59.], 0xc832f7ff, CANVAS);
        text(
            p,
            &self.manifest.music_difficulty_type.to_uppercase(),
            [392., 97., 190., 56.],
            31.,
            0xffffffff,
        );
        text(
            p,
            &format!("Lv. {}", self.manifest.play_level),
            [595., 96., 200., 55.],
            32.,
            0x29243eff,
        );
        self.gauge(p, [1015., 83., 525., 24.], 1.);
        for (label, threshold) in [("C", 0.4), ("B", 0.65), ("A", 0.8), ("S", 0.99)] {
            text(
                p,
                label,
                [1015. + 525. * threshold - 29., 32., 58., 47.],
                30.,
                0xffffffff,
            );
        }
        text(
            p,
            rank(self.result.score),
            [1585., 0., 150., 157.],
            140.,
            0xcc72ffff,
        );
        text(p, "SCORERANK", [1582., 147., 125., 25.], 15., 0xcc72ffff);
        let header = enter(time, 2., 0.45);
        animate(p, header_first, [0., -210. * (1. - header)], header);
        let score_first = p.draws.len();
        text(
            p,
            if self.english {
                "RESULT"
            } else {
                "演奏结果"
            },
            [232., 244., 745., 57.],
            34.,
            0xf2b9edff,
        );
        text(
            p,
            if self.english { "SCORE" } else { "分数" },
            [232., 322., 190., 89.],
            51.,
            0xffffffff,
        );
        text(
            p,
            &format!("{:08}", count(self.result.score, time, 2.45, 0.9)),
            [430., 297., 630., 125.],
            94.,
            0xff6fa9ff,
        );
        let mode = if self.result.autoplay {
            if self.english {
                "AUTOPLAY"
            } else {
                "自动演奏"
            }
        } else if self.result.practice_without_music {
            if self.english {
                "NO MUSIC · PRACTICE"
            } else {
                "无音乐练习"
            }
        } else {
            if self.english { "LIVE" } else { "演奏" }
        };
        text(p, mode, [232., 443., 740., 65.], 29., 0xc7c9dcff);
        let score_enter = enter(time, 2.4, 0.35);
        animate(p, score_first, [-75. * (1. - score_enter), 0.], score_enter);
        let combo_first = p.draws.len();
        let kind = live_result::select(self.result, 0);
        let badge = match kind {
            Kind::AllPerfect => "ALL PERFECT!",
            Kind::FullCombo => "FULL COMBO!",
            Kind::Clear => "CLEAR",
            Kind::Finish => "FINISH",
            Kind::None => "AUTO",
        };
        text(p, badge, [786., 584., 530., 47.], 30., 0xf4b5e5ff);
        text(p, "COMBO", [650., 640., 175., 68.], 34., 0xffffffff);
        text(
            p,
            &format!("{}", count(self.result.max_combo, time, 2.9, 0.6)),
            [835., 612., 390., 95.],
            65.,
            0xffffffff,
        );
        let combo_enter = enter(time, 2.85, 0.35);
        animate(p, combo_first, [0., 28. * (1. - combo_enter)], combo_enter);
        let counts = self.result.counts;
        for (i, (label, count, color)) in [
            ("PERFECT", counts[0] + counts[1], 0xaef0ffff),
            ("GREAT", counts[2], 0xd598ffff),
            ("GOOD", counts[3], 0x7fd1ffff),
            ("BAD", counts[4], 0x85f5c2ff),
            ("MISS", counts[5], 0xe4e4eeff),
        ]
        .into_iter()
        .enumerate()
        {
            let row_first = p.draws.len();
            let y = 637. + i as f32 * 66.;
            if i % 2 == 0 {
                p.rect([212., y - 4., 385., 61.], 0x3d3b708a, CANVAS);
            }
            text(p, label, [235., y, 223., 58.], 36., color);
            text(
                p,
                &format!(
                    "{:04}",
                    self::count(count, time, 2.85 + i as f32 * 0.09, 0.5)
                ),
                [465., y, 130., 58.],
                36.,
                0xe2e3f0ff,
            );
            let row = enter(time, 2.85 + i as f32 * 0.09, 0.3);
            animate(p, row_first, [-35. * (1. - row), 0.], row);
        }
        let extra_first = p.draws.len();
        p.rect([645., 755., 375., 174.], 0x39345ca6, CANVAS);
        text(p, "LIFE", [680., 776., 190., 52.], 30., 0x80f1caff);
        text(
            p,
            &format!("{}", count(self.result.life.max(0) as u32, time, 3.2, 0.45)),
            [847., 776., 150., 52.],
            30.,
            0xffffffff,
        );
        text(p, "AUTO", [680., 848., 160., 51.], 30., 0x82d8ffff);
        text(
            p,
            &format!("{}", count(counts[6], time, 3.2, 0.45)),
            [847., 848., 150., 51.],
            30.,
            0xffffffff,
        );
        let extra = enter(time, 3.15, 0.3);
        animate(p, extra_first, [0., 25. * (1. - extra)], extra);
        if let Some(figure) = self.figure {
            let arrival = enter(time, 2.4, 0.5);
            figure.draw(
                p,
                if self.result.life == 0 {
                    "idle"
                } else {
                    "celebrate"
                },
                (time - 2.4).max(0.),
                [1110. + 180. * (1. - arrival), 230., 565., 847.5],
                arrival,
            );
        }
        let buttons_first = p.draws.len();
        for (rect, label, color) in [
            (
                RETRY,
                if self.english {
                    "R · Retry"
                } else {
                    "R · 重试"
                },
                0xf3f4ffff,
            ),
            (
                RETURN,
                if self.english {
                    "Enter · Return"
                } else {
                    "Enter · 返回选曲"
                },
                0x60e5caff,
            ),
        ] {
            p.rect(rect, color, CANVAS);
            p.text(label, rect, 28., true, 1, 0x252640ff, CANVAS);
        }
        let buttons = enter(time, 3.7, 0.5);
        animate(p, buttons_first, [0., 65. * (1. - buttons)], buttons);
        if time < 0.25 {
            p.rect(
                CANVAS,
                ((1. - enter(time, 0., 0.25)) * 255.).round() as u32,
                CANVAS,
            );
        }
    }
    fn gauge(&self, p: &mut Painting, rect: [f32; 4], progress: f32) {
        let [x, y, w, h] = rect;
        p.rect(rect, 0x29253eff, CANVAS);
        let filled = (self.result.score as f32 / crate::scoring::MAX_SCORE as f32).clamp(0., 1.)
            * progress
            * w;
        let boundaries = [0., 0.4, 0.65, 0.8, 0.99, 1.];
        let colors = [0x5af5caff, 0x60d9ffff, 0xbfa0ffff, 0xe075fbff, 0xf383ffff];
        for (i, color) in colors.into_iter().enumerate() {
            let left = boundaries[i] * w;
            let width = (filled - left).clamp(0., (boundaries[i + 1] - boundaries[i]) * w);
            if width > 0. {
                p.rect([x + left, y, width, h], color, CANVAS);
            }
        }
    }
}
