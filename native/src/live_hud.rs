//! FrontUIView sprites and NumberView spacing, driven by native session data.
use crate::{
    live::Session,
    ui::Painting,
    unity::{Bundle, CANVAS},
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Widget {
    guid: String,
    rect: [f32; 4],
    sliced: bool,
    color: [f32; 4],
}
impl Widget {
    fn paint(
        &self,
        bundle: &Bundle,
        p: &mut Painting,
        offset: [f32; 2],
        scale: f32,
        color: Option<u32>,
    ) {
        let [x, y, w, h] = self.rect;
        let rgba = color.unwrap_or_else(|| {
            self.color.into_iter().fold(0, |bits, c| {
                (bits << 8) | (c.clamp(0., 1.) * 255.).round() as u32
            })
        });
        bundle.sprite(
            p,
            &self.guid,
            [
                offset[0] + x * scale,
                offset[1] + y * scale,
                w * scale,
                h * scale,
            ],
            CANVAS,
            rgba,
            self.sliced,
            false,
            1. / 108.,
            0.,
            false,
        );
    }
    fn paint_gauge(&self, bundle: &Bundle, p: &mut Painting, rate: f32) {
        let mut rect = self.rect;
        rect[2] *= rate.clamp(0., 1.);
        bundle.sprite(
            p,
            &self.guid,
            rect,
            CANVAS,
            0xffffffff,
            true,
            false,
            1. / 108.,
            0.,
            false,
        );
    }
}
#[derive(Deserialize)]
struct Number {
    origin: [f32; 2],
    step: f32,
    count: usize,
    left: bool,
    plus: Option<Widget>,
    digits: Vec<Widget>,
}
impl Number {
    fn paint(&self, bundle: &Bundle, p: &mut Painting, value: u32, normal: u32, surplus: u32) {
        self.paint_at(bundle, p, value, [normal, surplus], [0., 0.]);
    }
    fn paint_at(
        &self,
        bundle: &Bundle,
        p: &mut Painting,
        value: u32,
        colors: [u32; 2],
        offset: [f32; 2],
    ) {
        let significant = if value == 0 {
            1
        } else {
            value.ilog10() as usize + 1
        };
        let icon = usize::from(self.plus.is_some() && value > 0);
        let count = self.count.saturating_sub(icon);
        let colour = |visible: bool| colors[usize::from(!visible)];
        let position = |i: usize| {
            [
                offset[0]
                    + self.origin[0]
                    + self.step * i as f32 * if self.left { 1. } else { -1. },
                offset[1] + self.origin[1],
            ]
        };
        if icon > 0 {
            self.plus.as_ref().unwrap().paint(
                bundle,
                p,
                position(if self.left { 0 } else { significant }),
                1.,
                Some(colors[0]),
            );
        }
        for i in 0..count {
            let exponent = if self.left {
                significant.saturating_sub(i + 1)
            } else {
                i
            };
            let digit = if i < significant {
                (value / 10u32.pow(exponent as u32)) % 10
            } else {
                0
            };
            self.digits[digit as usize].paint(
                bundle,
                p,
                position(i + if self.left { icon } else { 0 }),
                1.,
                Some(colour(i < significant)),
            );
        }
    }
}
#[derive(Deserialize)]
pub struct Hud {
    r#static: Vec<Widget>,
    score_gauge: Widget,
    life_gauge: Widget,
    life_damage_gauge: Widget,
    life_bases: Vec<Widget>,
    rank: Widget,
    rank_text: Widget,
    ranks: Vec<Widget>,
    score_numbers: Vec<Number>,
    add_score_numbers: Vec<Number>,
    life_numbers: Vec<Number>,
    combo_label: Widget,
    combo_ap_label: String,
    combo_origin: [f32; 2],
    combo_step: f32,
    combo_digits: Vec<Widget>,
    combo_ap_digits: Vec<Widget>,
    judges: Vec<Widget>,
}
impl Hud {
    pub fn load(bundle: &Bundle) -> Result<Self, String> {
        serde_json::from_value(bundle.live["hud"].clone())
            .map_err(|e| format!("Original live HUD data: {e}; run make opensekai-assets"))
    }
    pub fn draw(&self, bundle: &Bundle, p: &mut Painting, session: &Session) {
        for widget in &self.r#static {
            widget.paint(bundle, p, [0., 0.], 1., None);
        }
        let score_rate = session.result.score as f32 / 1_000_000.;
        self.score_gauge.paint_gauge(bundle, p, score_rate);
        let rank = if score_rate >= 0.9 {
            0
        } else if score_rate >= 0.75 {
            1
        } else if score_rate >= 0.6 {
            2
        } else if score_rate >= 0.45 {
            3
        } else {
            4
        };
        let [x, y, w, h] = self.rank.rect;
        let glyph = &self.ranks[rank];
        glyph.paint(bundle, p, [x + w / 2., y + h / 2.], 1., None);
        self.rank_text.paint(
            bundle,
            p,
            [0., 0.],
            1.,
            Some([0xff8ec6ff, 0xe18affff, 0x78adffff, 0x55fdf7ff, 0x73ffccff][rank]),
        );
        self.score_numbers[0].paint(bundle, p, session.result.score, 0x444466ff, 0x444466ff);
        self.score_numbers[1].paint(bundle, p, session.result.score, 0xffffffff, 0xcccceeff);
        let life_rate = (session.result.life as f32 / 1000.).clamp(0., 1.);
        self.life_bases[usize::from(life_rate <= 0.2)].paint(bundle, p, [0., 0.], 1., None);
        let gauge = if life_rate > 0.2 {
            &self.life_gauge
        } else {
            &self.life_damage_gauge
        };
        gauge.paint_gauge(bundle, p, life_rate);
        for number in &self.life_numbers {
            number.paint(
                bundle,
                p,
                session.result.life.max(0) as u32,
                0xffffffff,
                0xffffff00,
            );
        }
        if session.result.combo > 0 {
            let all_perfect = session.result.counts[2..].iter().all(|n| *n == 0);
            let label_guid = if all_perfect {
                &self.combo_ap_label
            } else {
                &self.combo_label.guid
            };
            bundle.sprite(
                p,
                label_guid,
                self.combo_label.rect,
                CANVAS,
                0xffffffff,
                false,
                false,
                1.,
                0.,
                false,
            );
            let age = (session.time - session.combo_time).max(0.);
            let scale = 0.6 + (age / 0.1).clamp(0., 1.) * 0.4;
            let digits = if all_perfect {
                &self.combo_ap_digits
            } else {
                &self.combo_digits
            };
            let count = session.result.combo.ilog10() + 1;
            let mut value = session.result.combo;
            for i in 0..count {
                let x = ((count - 1) as f32 / 2. - i as f32) * self.combo_step;
                digits[(value % 10) as usize].paint(
                    bundle,
                    p,
                    [self.combo_origin[0] + x * scale, self.combo_origin[1]],
                    scale,
                    None,
                );
                value /= 10;
            }
        }
        if let Some((judge, time)) = session.last_judge {
            let age = session.time - time;
            if (0. ..=0.3).contains(&age) {
                let inv = 1. - (age * 20.).clamp(0., 1.);
                let scale = 1. - inv * inv * inv;
                let glyph = &self.judges[judge as usize];
                let [x, y, w, h] = glyph.rect;
                // JudgmentView scales the sprite about its authored centre.
                bundle.sprite(
                    p,
                    &glyph.guid,
                    [
                        x + w * (1. - scale) / 2.,
                        y + h * (1. - scale) / 2.,
                        w * scale,
                        h * scale,
                    ],
                    CANVAS,
                    0xffffffff,
                    false,
                    false,
                    1.,
                    0.,
                    false,
                );
            }
        }
        if let Some(time) = session.score_gain_time {
            let age = session.time - time;
            if session.score_gain > 0 && (0. ..0.5).contains(&age) {
                let inv = 1. - (age / 0.2).clamp(0., 1.);
                let progress = 1. - inv * inv;
                let opacity = (age / 0.2).min(1.) * ((0.5 - age) / 0.12).min(1.);
                let alpha = (opacity * 255.).round() as u32;
                let offset = [-54. * (1. - progress), 0.];
                self.add_score_numbers[0].paint_at(
                    bundle,
                    p,
                    session.score_gain,
                    [0x44446600 | alpha, 0x44446600],
                    offset,
                );
                self.add_score_numbers[1].paint_at(
                    bundle,
                    p,
                    session.score_gain,
                    [0xffffff00 | alpha, 0xffffff00],
                    offset,
                );
            }
        }
    }
}
