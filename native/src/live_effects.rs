//! Live effect layers use the source camera, atlas, sizes and gradients.
//! Particle births are deterministic; native cone motion is not Unity's PRNG.
use crate::{
    ffi::AppDraw,
    live::{HIT_Y, HoldFeedback, Judge, Session},
    particles::{Burst, Color, MinMax},
    ui::Painting,
    unity::{Bundle, CANVAS},
};
use serde::Deserialize;
use std::{collections::HashMap, ffi::CString, path::Path};

pub struct Effects {
    textures: HashMap<String, CString>,
    styles: HashMap<String, Style>,
    camera: Camera,
}
#[derive(Deserialize)]
struct Camera {
    position: [f32; 3],
    rotation: [f32; 4],
    fov: f32,
}
impl Camera {
    fn project(&self, point: [f32; 3]) -> Option<[f32; 2]> {
        let [x, y, z, w] = self.rotation;
        let rotation = [
            [
                1. - 2. * (y * y + z * z),
                2. * (x * y - z * w),
                2. * (x * z + y * w),
            ],
            [
                2. * (x * y + z * w),
                1. - 2. * (x * x + z * z),
                2. * (y * z - x * w),
            ],
            [
                2. * (x * z - y * w),
                2. * (y * z + x * w),
                1. - 2. * (x * x + y * y),
            ],
        ];
        let local: [f32; 3] = std::array::from_fn(|i| {
            (0..3)
                .map(|j| rotation[j][i] * (point[j] - self.position[j]))
                .sum()
        });
        if local[2] <= 0.3 {
            return None;
        }
        let focal = 540. / (self.fov.to_radians() / 2.).tan();
        Some([
            960. + local[0] * focal / local[2],
            540. - local[1] * focal / local[2],
        ])
    }
}
#[derive(Deserialize)]
struct Layer {
    per_lane: bool,
    span: bool,
    model: [[f32; 4]; 3],
    lifetime: MinMax,
    speed: MinMax,
    size: MinMax,
    size_y: MinMax,
    size_over_life: Option<MinMax>,
    size_over_life_y: Option<MinMax>,
    rotation: [MinMax; 3],
    rotation3d: bool,
    pivot: [f32; 3],
    start_color: Color,
    blend: MinMax,
    color: Option<Color>,
    bursts: Vec<Burst>,
    rate: MinMax,
    looping: bool,
    duration: f32,
    sheet: [u32; 2],
    frame: MinMax,
    sheet_enabled: bool,
    shape: u8,
    shape_enabled: bool,
    shape_position: [f32; 3],
    shape_scale: [f32; 3],
    radius: f32,
    order: i32,
    min_size: f32,
    render_mode: u8,
    gravity: MinMax,
    limit: Option<MinMax>,
    dampen: f32,
    texture: String,
}
#[derive(Deserialize)]
struct Style {
    layers: Vec<Layer>,
}
#[derive(Clone, Copy)]
struct Playback {
    region: [f32; 2],
    age: f32,
    seed: u32,
    strength: f32,
}
fn random(seed: u32) -> f32 {
    let mut x = seed.wrapping_mul(0x9e3779b9).wrapping_add(0x85ebca6b);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    (x >> 8) as f32 / 16777216.
}
fn bounds(xy: [f32; 8]) -> [f32; 4] {
    let xs = [xy[0], xy[2], xy[4], xy[6]];
    let ys = [xy[1], xy[3], xy[5], xy[7]];
    let x = xs.into_iter().fold(f32::INFINITY, f32::min);
    let y = ys.into_iter().fold(f32::INFINITY, f32::min);
    [
        x,
        y,
        xs.into_iter().fold(f32::NEG_INFINITY, f32::max) - x,
        ys.into_iter().fold(f32::NEG_INFINITY, f32::max) - y,
    ]
}
fn rotate(mut point: [f32; 3], angles: [f32; 3]) -> [f32; 3] {
    // These authored particle rotations use X/Z; retain their plane orientation.
    for axis in [2, 0, 1] {
        let (a, b) = match axis {
            0 => (1, 2),
            1 => (2, 0),
            _ => (0, 1),
        };
        let (sin, cos) = angles[axis].sin_cos();
        let (u, v) = (point[a], point[b]);
        point[a] = u * cos - v * sin;
        point[b] = u * sin + v * cos;
    }
    point
}
impl Layer {
    fn uv(&self, seed: u32) -> [f32; 4] {
        if !self.sheet_enabled {
            return [0., 0., 1., 1.];
        }
        let [columns, rows] = self.sheet;
        let frame =
            (self.frame.evaluate(0., random(seed)) * (columns * rows) as f32).floor() as u32;
        let frame = frame.min(columns * rows - 1);
        let (x, y) = (frame % columns, frame / columns);
        [
            x as f32 / columns as f32,
            y as f32 / rows as f32,
            (x + 1) as f32 / columns as f32,
            (y + 1) as f32 / rows as f32,
        ]
    }
    fn rgba(&self, life: f32, seed: u32, strength: f32) -> u32 {
        let start = self.start_color.evaluate(0., random(seed));
        let over = self
            .color
            .as_ref()
            .map_or([1.; 4], |c| c.evaluate(life, random(seed + 1)));
        (0..4).fold(0, |bits, i| {
            (bits << 8)
                | ((start[i] * over[i] * if i == 3 { strength } else { 1. }).clamp(0., 1.) * 255.)
                    .round() as u32
        })
    }
    fn blend_flag(&self, life: f32, seed: u32) -> u32 {
        // The source shader premultiplies RGB and selects alpha/additive with
        // Custom1.x. ASM's straight-alpha materials produce the same blend.
        if self.blend.evaluate(life, random(seed)) > 0.5 {
            4
        } else {
            0
        }
    }
    fn transform(&self, point: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|i| {
            (0..3).map(|j| self.model[i][j] * point[j]).sum::<f32>() + self.model[i][3]
        })
    }
    fn size(&self, life: f32, seed: u32) -> [f32; 2] {
        let over = |curve: &Option<MinMax>| {
            curve
                .as_ref()
                .map_or(1., |c| c.evaluate(life, random(seed + 1)))
        };
        [
            self.size.evaluate(0., random(seed)) * over(&self.size_over_life),
            self.size_y.evaluate(0., random(seed)) * over(&self.size_over_life_y),
        ]
    }
    fn births(&self, elapsed: f32, seed: u32) -> Vec<(u32, f32)> {
        let lifetime = self
            .lifetime
            .evaluate(0., 0.)
            .max(self.lifetime.evaluate(0., 1.));
        let start = (elapsed - lifetime).max(0.);
        let duration = self.duration.max(0.01);
        let first_loop = if self.looping {
            (start / duration).floor() as u32
        } else {
            0
        };
        let last_loop = if self.looping {
            (elapsed / duration).floor() as u32
        } else {
            0
        };
        let mut births = Vec::new();
        for cycle in first_loop..=last_loop {
            let origin = cycle as f32 * duration;
            for (index, burst) in self.bursts.iter().enumerate() {
                for repetition in 0..burst.cycles.max(1) {
                    let born = origin + burst.time + repetition as f32 * burst.interval;
                    if born < start || born > elapsed {
                        continue;
                    }
                    let salt =
                        seed.wrapping_add(cycle * 997 + index as u32 * 131 + repetition * 29);
                    if random(salt) > burst.probability {
                        continue;
                    }
                    let count = burst.count.evaluate(0., random(salt + 1)).round().max(0.) as u32;
                    births.extend(
                        (0..count.min(256)).map(|i| (salt.wrapping_add(i * 13), elapsed - born)),
                    );
                }
            }
        }
        let rate = self.rate.evaluate(0., random(seed)).max(0.);
        if rate > 0. {
            let end = if self.looping {
                elapsed
            } else {
                elapsed.min(duration)
            };
            let first = (start * rate).ceil().max(1.) as u32;
            let last = (end * rate).floor() as u32;
            births.extend(
                (first..=last)
                    .take(256)
                    .map(|i| (seed.wrapping_add(i * 71), elapsed - i as f32 / rate)),
            );
        }
        births
    }
    fn offset(&self, age: f32, seed: u32) -> [f32; 3] {
        let mut position = if !self.shape_enabled {
            [0.; 3]
        } else if self.shape == 5 {
            std::array::from_fn(|i| {
                self.shape_position[i] + (random(seed + i as u32 + 2) - 0.5) * self.shape_scale[i]
            })
        } else {
            [
                self.shape_position[0] + (random(seed + 2) - 0.5) * self.radius * 2.,
                self.shape_position[1],
                self.shape_position[2] + (random(seed + 3) - 0.5) * self.radius,
            ]
        };
        let speed = self.speed.evaluate(0., random(seed + 4));
        let distance = if let Some(limit) = &self.limit {
            let limit = limit.evaluate(0., random(seed + 5));
            let excess = (speed - limit).max(0.);
            let decay = (1. - self.dampen).clamp(0., 1.);
            if decay < 0.9999 {
                speed.min(limit) * age
                    + excess * (1. - decay.powf(age * 60.)) / ((1. - decay) * 60.)
            } else {
                speed * age
            }
        } else {
            speed * age
        };
        let angle = (random(seed + 6) - 0.5) * 1.6;
        position[0] += angle.sin() * distance;
        position[1] += angle.cos() * distance
            - 4.905 * self.gravity.evaluate(0., random(seed + 7)) * age * age;
        position
    }
}
impl Effects {
    pub fn load(project: &Path, bundle: &Bundle) -> Result<Self, String> {
        let styles: HashMap<String, Style> =
            serde_json::from_value(bundle.live["hit_styles"].clone())
                .map_err(|e| format!("Original hit layers: {e}; run make opensekai-assets"))?;
        let camera = serde_json::from_value(bundle.live["effect_camera"].clone())
            .map_err(|e| format!("Original hit camera: {e}"))?;
        let mut textures = HashMap::new();
        for style in styles.values() {
            for layer in &style.layers {
                if textures.contains_key(&layer.texture) {
                    continue;
                }
                let canonical = project
                    .join("resources/opensekai/upstream")
                    .join(&layer.texture);
                let path = if canonical.is_file() {
                    canonical
                } else {
                    project.join(&layer.texture)
                };
                if !path.is_file() {
                    return Err(format!("Original hit texture missing: {}", path.display()));
                }
                textures.insert(
                    layer.texture.clone(),
                    CString::new(path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?,
                );
            }
        }
        Ok(Self {
            styles,
            camera,
            textures,
        })
    }
    pub fn warm(&self, p: &mut Painting) {
        for path in self.textures.values() {
            p.draws.push(AppDraw {
                kind: 2,
                rect: [0., 0., 1., 1.],
                uv: [0., 0., 1., 1.],
                clip: CANVAS,
                rgba: 0,
                text: path.as_ptr(),
                reserved: 4,
                ..AppDraw::default()
            });
        }
    }
    pub fn draw_lanes(&self, p: &mut Painting, session: &Session) {
        let white = &self.styles["lane"].layers[0];
        for (lane, time) in session.lane_touch_times.iter().enumerate() {
            let Some(time) = time else {
                continue;
            };
            let age = session.time - time;
            if !(0. ..0.18).contains(&age) {
                continue;
            }
            let fade = (1. - age / 0.18).powi(2);
            let left = 188.88 + lane as f32 * 128.52;
            let right = left + 128.52;
            let horizon = |x: f32| 960. + (x - 960.) * 0.002;
            let xy = [
                left,
                HIT_Y,
                right,
                HIT_Y,
                horizon(right),
                0.,
                horizon(left),
                0.,
            ];
            p.draws.push(AppDraw {
                kind: 3,
                xy,
                rect: bounds(xy),
                uv: white.uv(lane as u32),
                clip: [0., 0., 1920., HIT_Y],
                rgba: white.rgba(age / 0.45, lane as u32, fade),
                text: self.textures[&white.texture].as_ptr(),
                reserved: white.blend_flag(age / 0.45, lane as u32),
                ..AppDraw::default()
            });
            p.rect(
                [left + 2., HIT_Y - 3., 124.52, 47.],
                0xffffff00 | (fade * 96.).round() as u32,
                CANVAS,
            );
        }
    }
    fn layer(&self, draws: &mut Vec<(i32, AppDraw)>, layer: &Layer, lane: f32, playback: Playback) {
        let origin = [lane * 0.84 - 4.62, 0., 0.];
        let Some(ground) = self.camera.project(origin) else {
            return;
        };
        let anchor = [960. + (-6.545 + lane * 1.19) * 108., HIT_Y];
        for (salt, age) in layer.births(playback.age, playback.seed) {
            let lifetime = layer.lifetime.evaluate(0., random(salt));
            if lifetime <= 0. || age >= lifetime {
                continue;
            }
            let life = age / lifetime;
            let color = layer.rgba(life, salt, playback.strength);
            if color & 255 == 0 {
                continue;
            }
            let size = layer.size(life, salt + 8);
            let stretch = if layer.span {
                playback.region[1] - playback.region[0] + 1.
            } else {
                1.
            };
            let rotation = std::array::from_fn(|i| {
                if i == 2 || layer.rotation3d {
                    layer.rotation[i].evaluate(0., random(salt + 10 + i as u32))
                } else {
                    0.
                }
            });
            let position = layer.transform(layer.offset(age, salt));
            let scales: [f32; 3] = std::array::from_fn(|i| {
                (0..3)
                    .map(|j| layer.model[j][i].powi(2))
                    .sum::<f32>()
                    .sqrt()
            });
            let mut xy = [0.; 8];
            let mut visible = true;
            for (i, corner) in [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]]
                .into_iter()
                .enumerate()
            {
                let delta = rotate(
                    [
                        (corner[0] + layer.pivot[0]) * size[0],
                        (corner[1] + layer.pivot[1]) * size[1],
                        0.,
                    ],
                    rotation,
                );
                let vertex = if layer.render_mode == 2 {
                    // Mesh quads preserve the authored 90-degree ground rotation.
                    std::array::from_fn(|axis| {
                        (0..3).map(|j| layer.model[axis][j] * delta[j]).sum::<f32>()
                    })
                } else {
                    // Source billboards face the front camera before the custom
                    // tap-effect camera projects them.
                    [
                        delta[0] * scales[0],
                        delta[1] * scales[1],
                        delta[2] * scales[2],
                    ]
                };
                let point = [
                    origin[0] + (position[0] + vertex[0]) * stretch,
                    position[1] + vertex[1],
                    position[2] + vertex[2],
                ];
                if let Some(projected) = self.camera.project(point) {
                    xy[i * 2] = projected[0] - ground[0] + anchor[0];
                    xy[i * 2 + 1] = projected[1] - ground[1] + anchor[1];
                } else {
                    visible = false;
                }
            }
            if !visible {
                continue;
            }
            let mut rect = bounds(xy);
            let minimum = layer.min_size * 1080.;
            if rect[2].max(rect[3]) < minimum && rect[2] > 0. && rect[3] > 0. {
                let factor = minimum / rect[2].max(rect[3]);
                let centre = [rect[0] + rect[2] / 2., rect[1] + rect[3] / 2.];
                for i in 0..4 {
                    xy[i * 2] = centre[0] + (xy[i * 2] - centre[0]) * factor;
                    xy[i * 2 + 1] = centre[1] + (xy[i * 2 + 1] - centre[1]) * factor;
                }
                rect = bounds(xy);
            }
            draws.push((
                layer.order,
                AppDraw {
                    kind: 3,
                    xy,
                    rect,
                    uv: layer.uv(salt + 13),
                    clip: CANVAS,
                    rgba: color,
                    text: self.textures[&layer.texture].as_ptr(),
                    reserved: layer.blend_flag(life, salt),
                    ..AppDraw::default()
                },
            ));
        }
    }
    fn style(&self, draws: &mut Vec<(i32, AppDraw)>, name: &str, playback: Playback) {
        let [start, end] = playback.region;
        for (index, layer) in self.styles[name].layers.iter().enumerate() {
            let instances = if layer.per_lane {
                (end - start + 1.).ceil().max(1.) as u32
            } else {
                1
            };
            for instance in 0..instances {
                let lane = if layer.per_lane {
                    start + instance as f32
                } else {
                    (start + end) / 2.
                };
                let salt = playback
                    .seed
                    .wrapping_mul(4099)
                    .wrapping_add(index as u32 * 193 + instance * 37);
                self.layer(
                    draws,
                    layer,
                    lane,
                    Playback {
                        seed: salt,
                        ..playback
                    },
                );
            }
        }
    }
    pub fn draw(&self, p: &mut Painting, session: &Session, holds: &[HoldFeedback]) {
        let mut draws = Vec::new();
        for hit in &session.visual_hits {
            let age = session.time - hit.time;
            if !(0. ..0.6).contains(&age) {
                continue;
            }
            let strength = match hit.judge {
                Judge::Bad => 0.25,
                Judge::Good => 0.5,
                _ => 1.,
            };
            let name = match (hit.critical, hit.flick, hit.hold) {
                (true, true, _) => "critical_flick",
                (true, _, true) => "critical_long",
                (true, _, _) => "critical_normal",
                (_, true, _) => "flick",
                (_, _, true) => "long",
                _ => "normal",
            };
            // The hidden checkpoints only update score/combo. The hold loop
            // below keeps its aura alive between checkpoints without repeated explosions.
            if !hit.source_hit {
                continue;
            }
            let playback = Playback {
                region: [hit.start, hit.end],
                age,
                seed: hit.seed,
                strength,
            };
            self.style(&mut draws, name, playback);
            if hit.flick {
                self.style(
                    &mut draws,
                    if hit.critical {
                        "critical_flash"
                    } else {
                        "flash"
                    },
                    playback,
                );
            }
        }
        for hold in holds {
            self.style(
                &mut draws,
                if hold.critical {
                    "critical_hold"
                } else {
                    "hold"
                },
                Playback {
                    region: [hold.start, hold.end],
                    age: hold.age,
                    seed: hold.seed,
                    strength: 1.,
                },
            );
        }
        // Background plates precede beams and ripples even when different
        // contacts/hold emitters overlap. Prefab serialization is not draw order.
        draws.sort_by_key(|(order, _)| *order);
        p.draws.extend(draws.into_iter().map(|(_, draw)| draw));
    }
}
