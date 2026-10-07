//! Result-effect particles evaluated from the imported prefab modules.
//!
//! Births and random choices are keyed by emitter/particle, so repainting or
//! skipping a render frame cannot reshuffle the effect. Velocity damping uses
//! a 60 Hz visual model; Unity's native integration and PRNG remain comparison
//! targets rather than an asserted identical implementation.
use crate::animation::{self, Key};
use serde::Deserialize;
use std::f32::consts::TAU;

#[derive(Debug, Deserialize)]
pub struct MinMax {
    pub mode: u8,
    pub scalar: f32,
    pub minimum: f32,
    pub lower: Vec<Key>,
    pub upper: Vec<Key>,
}
impl MinMax {
    pub fn evaluate(&self, time: f32, random: f32) -> f32 {
        match self.mode {
            0 => self.scalar,
            1 => animation::sample(&self.upper, time) * self.scalar,
            2 => {
                let lo = animation::sample(&self.lower, time);
                let hi = animation::sample(&self.upper, time);
                (lo + (hi - lo) * random) * self.scalar
            }
            3 => self.minimum + (self.scalar - self.minimum) * random,
            _ => unreachable!("particle compiler validates MinMax modes"),
        }
    }
    fn maximum(&self) -> f32 {
        match self.mode {
            0 => self.scalar,
            3 => self.minimum.max(self.scalar),
            _ => {
                // These imported start lifetimes are constants. Sampling is
                // only a conservative birth-window fallback for other curves.
                (0..=100)
                    .flat_map(|i| {
                        [
                            self.evaluate(i as f32 / 100., 0.),
                            self.evaluate(i as f32 / 100., 1.),
                        ]
                    })
                    .fold(0., f32::max)
                    * 2.
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Gradient {
    pub step: bool,
    pub rgb: Vec<[f32; 4]>,
    pub alpha: Vec<[f32; 2]>,
}
fn gradient_channel<const N: usize>(
    keys: &[[f32; N]],
    time: f32,
    channel: usize,
    step: bool,
) -> f32 {
    let Some(first) = keys.first() else { return 1. };
    if time <= first[0] {
        return first[channel];
    }
    let next = keys.partition_point(|key| key[0] <= time);
    if next == keys.len() {
        return keys[next - 1][channel];
    }
    let (a, b) = (keys[next - 1], keys[next]);
    if step {
        return b[channel];
    }
    let factor = (time - a[0]) / (b[0] - a[0]);
    a[channel] + (b[channel] - a[channel]) * factor
}
impl Gradient {
    pub fn evaluate(&self, time: f32) -> [f32; 4] {
        [
            gradient_channel(&self.rgb, time, 1, self.step),
            gradient_channel(&self.rgb, time, 2, self.step),
            gradient_channel(&self.rgb, time, 3, self.step),
            gradient_channel(&self.alpha, time, 1, self.step),
        ]
    }
}
#[derive(Debug, Deserialize)]
pub struct Color {
    pub mode: u8,
    pub minimum: [f32; 4],
    pub maximum: [f32; 4],
    pub lower: Gradient,
    pub upper: Gradient,
}
impl Color {
    pub fn evaluate(&self, time: f32, random: f32) -> [f32; 4] {
        let (a, b) = match self.mode {
            0 => return self.maximum,
            1 => return self.upper.evaluate(time),
            2 => (self.minimum, self.maximum),
            3 => (self.lower.evaluate(time), self.upper.evaluate(time)),
            4 => return self.upper.evaluate(random),
            _ => unreachable!("particle compiler validates gradient modes"),
        };
        std::array::from_fn(|i| a[i] + (b[i] - a[i]) * random)
    }
}

#[derive(Debug, Deserialize)]
pub struct Burst {
    pub time: f32,
    pub count: MinMax,
    pub cycles: u32,
    pub interval: f32,
    pub probability: f32,
}
#[derive(Debug, Deserialize)]
pub struct Shape {
    pub kind: u8,
    pub position: [f32; 3],
    pub scale: [f32; 3],
    pub radius: f32,
    pub thickness: f32,
    pub arc_mode: u8,
    pub arc_speed: f32,
    pub arc_spread: f32,
    pub random_position: f32,
}
#[derive(Debug, Deserialize)]
pub struct Limit {
    pub magnitude: MinMax,
    pub dampen: f32,
}
#[derive(Debug, Deserialize)]
pub struct Emitter {
    pub name: String,
    pub node: usize,
    pub active_from: Option<f32>,
    pub position: [f32; 3],
    pub duration: f32,
    pub looping: bool,
    pub delay: f32,
    pub capacity: usize,
    pub auto_seed: bool,
    pub seed: u32,
    pub lifetime: MinMax,
    pub speed: MinMax,
    pub size: MinMax,
    pub rotation: MinMax,
    pub start_color: Color,
    pub color: Color,
    pub size_over_life: MinMax,
    pub limit: Option<Limit>,
    pub rate: f32,
    pub bursts: Vec<Burst>,
    pub shape: Shape,
    pub texture: String,
    pub texture_transform: [f32; 4],
    pub enabled: bool,
    pub min_size: f32,
    pub max_size: f32,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    pub birth: f32,
    pub position: [f32; 3],
    pub size: f32,
    pub rotation: f32,
    pub color: [f32; 4],
}

fn mix(mut value: u32) -> u32 {
    value = (value ^ (value >> 16)).wrapping_mul(0x7feb352d);
    value = (value ^ (value >> 15)).wrapping_mul(0x846ca68b);
    value ^ (value >> 16)
}
fn random(seed: u32, stream: u32) -> f32 {
    (mix(seed ^ stream.wrapping_mul(0x9e3779b9)) >> 8) as f32 / 16777216.
}
impl Emitter {
    fn particle(&self, time: f32, birth: f32, id: u32, seed: u32) -> Option<Particle> {
        let seed = mix(seed ^ id);
        let phase = if self.duration > 0. {
            birth.rem_euclid(self.duration) / self.duration
        } else {
            0.
        };
        let lifetime = self.lifetime.evaluate(phase, random(seed, 1));
        let age = time - birth;
        if age < 0. || lifetime <= 0. || age >= lifetime {
            return None;
        }
        let t = age / lifetime;
        let speed = self.speed.evaluate(phase, random(seed, 2));
        let mut distance = speed * age;
        if let Some(limit) = &self.limit {
            let cap = limit.magnitude.evaluate(t, random(seed, 3));
            if speed > cap && limit.dampen > 0. {
                if limit.dampen >= 1. {
                    distance = cap * age;
                } else {
                    let k = -(-limit.dampen).ln_1p() * 60.;
                    distance = cap * age + (speed - cap) * -(-k * age).exp_m1() / k;
                }
            }
        }
        let shape = &self.shape;
        let (mut position, direction) = if shape.kind == 10 {
            let mut arc = if shape.arc_mode == 1 {
                birth * shape.arc_speed
            } else {
                random(seed, 4)
            };
            if shape.arc_spread > 0. {
                arc = (arc / shape.arc_spread).floor() * shape.arc_spread;
            }
            let (sin, cos) = (arc * TAU).sin_cos();
            let inner = 1. - shape.thickness;
            let radius =
                shape.radius * (inner * inner + (1. - inner * inner) * random(seed, 5)).sqrt();
            (
                [
                    cos * radius * shape.scale[0],
                    sin * radius * shape.scale[1],
                    0.,
                ],
                [cos, sin, 0.],
            )
        } else {
            (
                std::array::from_fn(|i| (random(seed, 4 + i as u32) - 0.5) * shape.scale[i]),
                [0., 0., 1.],
            )
        };
        if shape.random_position > 0. {
            let z = random(seed, 8) * 2. - 1.;
            let radius = random(seed, 9).cbrt() * shape.random_position;
            let (sin, cos) = (random(seed, 10) * TAU).sin_cos();
            let xy = (1. - z * z).sqrt();
            let jitter = [cos * xy * radius, sin * xy * radius, z * radius];
            for i in 0..3 {
                position[i] += jitter[i];
            }
        }
        for i in 0..3 {
            position[i] += self.position[i] + shape.position[i] + direction[i] * distance;
        }
        let start = self.start_color.evaluate(phase, random(seed, 11));
        let color = self.color.evaluate(t, random(seed, 12));
        Some(Particle {
            birth,
            position,
            size: self.size.evaluate(phase, random(seed, 13))
                * self.size_over_life.evaluate(t, random(seed, 14)),
            rotation: self.rotation.evaluate(phase, random(seed, 15)),
            color: std::array::from_fn(|i| start[i] * color[i]),
        })
    }
    pub fn sample(&self, clip_time: f32, play_seed: u32) -> Vec<Particle> {
        let Some(activation) = self.active_from else {
            return Vec::new();
        };
        let time = clip_time - activation - self.delay;
        if !self.enabled || !time.is_finite() || time < 0. || self.duration <= 0. {
            return Vec::new();
        }
        let seed = mix(if self.auto_seed { play_seed } else { self.seed } ^ self.node as u32);
        let oldest = (time - self.lifetime.maximum()).max(0.);
        let mut births = Vec::new();
        if self.rate > 0. {
            let last = if self.looping {
                time
            } else {
                time.min(self.duration)
            };
            // A rate emitter's first particle arrives after its first interval.
            for i in ((oldest * self.rate).floor() as u32 + 1)..=(last * self.rate).floor() as u32 {
                births.push((i as f32 / self.rate, i));
            }
        }
        let first_loop = (oldest / self.duration).floor() as u32;
        let last_loop = if self.looping {
            (time / self.duration).floor() as u32
        } else {
            0
        };
        for cycle in first_loop..=last_loop {
            for (burst_id, burst) in self.bursts.iter().enumerate() {
                for repeat in 0..burst.cycles {
                    let birth =
                        cycle as f32 * self.duration + burst.time + repeat as f32 * burst.interval;
                    if birth > time || birth < oldest {
                        continue;
                    }
                    let burst_seed =
                        mix(seed ^ cycle ^ (burst_id as u32).wrapping_mul(256) ^ repeat);
                    if random(burst_seed, 16) >= burst.probability {
                        continue;
                    }
                    let count = burst
                        .count
                        .evaluate(birth / self.duration, random(burst_seed, 17))
                        .max(0.)
                        .round() as u32;
                    for i in 0..count.min(self.capacity as u32) {
                        births.push((birth, mix(burst_seed ^ i ^ 0x10000000)));
                    }
                }
            }
        }
        births.sort_by(|a, b| a.0.total_cmp(&b.0));
        births
            .into_iter()
            .filter_map(|(birth, id)| self.particle(time, birth, id, seed))
            .take(self.capacity)
            .collect()
    }
}
