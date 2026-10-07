//! LiveResultView prefab animation and animation-event sound scheduling.
use crate::{
    animation::{self, Key, Transform},
    ffi::AppDraw,
    live::ResultData,
    ui::Painting,
    unity::{Bundle, CANVAS},
};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Default, Deserialize)]
pub struct Library {
    pub results: HashMap<String, Presentation>,
}
#[derive(Deserialize)]
pub struct Presentation {
    pub duration: f32,
    nodes: Vec<Node>,
    sprites: Vec<Sprite>,
    meshes: Vec<Mesh>,
    pub particles: Vec<crate::particles::Emitter>,
    tracks: Vec<Track>,
    pub events: Vec<SoundEvent>,
}
#[derive(Deserialize)]
struct Node {
    parent: Option<usize>,
    position: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
    active: bool,
}
#[derive(Deserialize)]
struct Sprite {
    node: usize,
    guid: String,
    color: [f32; 4],
    size: [f32; 2],
    pivot: [f32; 2],
    enabled: bool,
    additive: bool,
}
#[derive(Deserialize)]
struct Mesh {
    node: usize,
    texture: String,
    enabled: bool,
    tint: [f32; 4],
    texture_transform: [f32; 4],
    quads: Vec<MeshQuad>,
}
#[derive(Deserialize)]
struct MeshQuad {
    vertices: [[f32; 3]; 4],
    uv: [f32; 4],
}
#[derive(Deserialize)]
struct Track {
    node: usize,
    property: String,
    channel: usize,
    keys: Vec<Key>,
}
#[derive(Deserialize)]
pub struct SoundEvent {
    pub time: f32,
    pub cue: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    None,
    Finish,
    Clear,
    FullCombo,
    AllPerfect,
}
impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Finish => "finish",
            Self::Clear => "clear",
            Self::FullCombo => "full_combo",
            Self::AllPerfect => "all_perfect",
        }
    }
    pub fn cue(self) -> Option<&'static str> {
        match self {
            Self::Clear => Some("se_live_clear"),
            Self::FullCombo => Some("se_live_full_combo"),
            Self::AllPerfect => Some("se_live_all_perfect"),
            _ => None,
        }
    }
}
pub fn select(result: &ResultData, auto_mode: u32) -> Kind {
    let c = result.counts;
    if result.total > 0 && c[6] as usize == result.total {
        return if result.autoplay {
            match auto_mode {
                1 => Kind::AllPerfect,
                2 => Kind::FullCombo,
                3 => Kind::Clear,
                4 => Kind::Finish,
                0 => Kind::None,
                _ => Kind::Clear,
            }
        } else {
            Kind::Clear
        };
    }
    if c[0] as usize + c[1] as usize == result.total {
        Kind::AllPerfect
    } else if result.max_combo as usize == result.total {
        Kind::FullCombo
    } else if result.life > 0 {
        Kind::Clear
    } else {
        Kind::Finish
    }
}

pub struct Sequence {
    elapsed: Option<f32>,
    kind: Option<Kind>,
    particle_seed: u32,
}
impl Default for Sequence {
    fn default() -> Self {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        Self {
            elapsed: None,
            kind: None,
            particle_seed: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        }
    }
}
impl Sequence {
    pub fn advance(
        &mut self,
        dt: f32,
        finished: bool,
        result: &ResultData,
        library: &Library,
    ) -> Vec<String> {
        if !finished {
            return Vec::new();
        }
        let (previous, now) = if let Some(elapsed) = self.elapsed {
            (
                elapsed,
                elapsed + if dt.is_finite() { dt.max(0.) } else { 0. },
            )
        } else {
            self.kind = Some(select(result, 0));
            (-f32::EPSILON, 0.)
        };
        self.elapsed = Some(now);
        let kind = self.kind.unwrap();
        let mut cues = Vec::new();
        // BaseLiveController.PreExit(1,4), LiveResultView.Fade delays the prefab
        // by 0.2 seconds, then SoloLiveController fades out for 2 seconds.
        if previous < 1.
            && now >= 1.
            && let Some(cue) = kind.cue()
        {
            cues.push(cue.into());
        }
        if let Some(presentation) = library.results.get(kind.name()) {
            for event in &presentation.events {
                let at = 1.2 + event.time;
                if previous < at && now >= at {
                    cues.push(event.cue.clone());
                }
            }
        }
        cues
    }
    pub fn ready(&self) -> bool {
        self.elapsed.is_some_and(|t| t >= 7.)
    }
    pub fn elapsed(&self) -> Option<f32> {
        self.elapsed
    }
    pub fn draw(&self, bundle: &Bundle, paint: &mut Painting) {
        let (Some(t), Some(kind)) = (self.elapsed, self.kind) else {
            return;
        };
        if self.ready() {
            return;
        }
        if kind != Kind::None && t >= 1. {
            let fade = ((t - 1.) / 0.2).clamp(0., 1.);
            let alpha = (1. - (fade - 1.).powi(2)).sqrt() * 0.5;
            paint.rect(CANVAS, (alpha * 255.).round() as u32, CANVAS);
            if t >= 1.2
                && let Some(presentation) = bundle.presentations.results.get(kind.name())
            {
                presentation.draw_seeded(bundle, paint, t - 1.2, self.particle_seed);
            }
        }
        if t >= 5. {
            // DOTween's default ease is OutQuad for DOVirtual.Float.
            let f = ((t - 5.) / 2.).clamp(0., 1.);
            let alpha = 1. - (1. - f).powi(2);
            paint.rect(CANVAS, (alpha * 255.).round() as u32, CANVAS);
        }
    }
}
impl Presentation {
    pub fn draw(&self, bundle: &Bundle, paint: &mut Painting, time: f32) {
        self.draw_seeded(bundle, paint, time, 1);
    }
    pub fn draw_seeded(&self, bundle: &Bundle, paint: &mut Painting, time: f32, seed: u32) {
        let mut positions: Vec<_> = self.nodes.iter().map(|n| n.position).collect();
        let mut scales: Vec<_> = self.nodes.iter().map(|n| n.scale).collect();
        let mut active: Vec<_> = self.nodes.iter().map(|n| n.active).collect();
        let mut colors = vec![[1.; 4]; self.nodes.len()];
        let mut tints = vec![[1.; 4]; self.nodes.len()];
        let mut texture_transforms = vec![[1., 1., 0., 0.]; self.nodes.len()];
        for mesh in &self.meshes {
            tints[mesh.node] = mesh.tint;
            texture_transforms[mesh.node] = mesh.texture_transform;
        }
        for sprite in &self.sprites {
            colors[sprite.node] = sprite.color;
        }
        for track in &self.tracks {
            let value = animation::sample(&track.keys, time.min(self.duration));
            match track.property.as_str() {
                "position" => positions[track.node][track.channel] = value,
                "scale" => scales[track.node][track.channel] = value,
                "color" => colors[track.node][track.channel] = value,
                "active" => active[track.node] = value > 0.5,
                "tint" => tints[track.node][track.channel] = value,
                "texture_transform" => texture_transforms[track.node][track.channel] = value,
                _ => {}
            }
        }
        let mut world = vec![Transform::IDENTITY; self.nodes.len()];
        for (i, node) in self.nodes.iter().enumerate() {
            let local = Transform::trs(positions[i], node.rotation, scales[i]);
            world[i] = if let Some(parent) = node.parent {
                active[i] &= active[parent];
                world[parent].compose(local)
            } else {
                local
            };
        }
        for emitter in &self.particles {
            let [sx, sy, ox, oy] = emitter.texture_transform;
            for particle in emitter.sample(time, seed) {
                let rgba = pack(particle.color);
                if rgba & 255 == 0 || particle.size <= 0. {
                    continue;
                }
                let size = particle
                    .size
                    .clamp(emitter.min_size * 10., emitter.max_size * 10.);
                let (sin, cos) = particle.rotation.sin_cos();
                let mut xy = [0.; 8];
                for (i, [x, y]) in [[-0.5, 0.5], [0.5, 0.5], [0.5, -0.5], [-0.5, -0.5]]
                    .into_iter()
                    .enumerate()
                {
                    xy[i * 2] = 960. + (particle.position[0] + (cos * x - sin * y) * size) * 108.;
                    xy[i * 2 + 1] =
                        540. - (particle.position[1] + (sin * x + cos * y) * size) * 108.;
                }
                push_quad(
                    paint,
                    &bundle.paths[&emitter.texture],
                    xy,
                    [ox, 1. - sy - oy, sx + ox, 1. - oy],
                    rgba,
                    4,
                );
            }
        }
        // The source ring has sortingOrder 0 and lies behind the glow sprite
        // at z=-0.2; the remaining glyphs have sortingOrder 4 through 6.
        for mesh in &self.meshes {
            if !mesh.enabled || !active[mesh.node] {
                continue;
            }
            let rgba = pack(tints[mesh.node].map(|v| v * 2.)); // Source vertex shader saturates 2*tint.
            if rgba & 255 == 0 {
                continue;
            }
            let [sx, sy, ox, oy] = texture_transforms[mesh.node];
            for quad in &mesh.quads {
                let mut xy = [0.; 8];
                for (i, corner) in quad.vertices.iter().enumerate() {
                    let p = world[mesh.node].point(*corner);
                    xy[i * 2] = 960. + p[0] * 108.;
                    xy[i * 2 + 1] = 540. - p[1] * 108.;
                }
                let [u0, v0, u1, v1] = quad.uv;
                push_quad(
                    paint,
                    &bundle.paths[&mesh.texture],
                    xy,
                    [
                        u0 * sx + ox,
                        1. - (v1 * sy + oy),
                        u1 * sx + ox,
                        1. - (v0 * sy + oy),
                    ],
                    rgba,
                    4,
                );
            }
        }
        for sprite in &self.sprites {
            if !sprite.enabled || !active[sprite.node] {
                continue;
            }
            let Some(uv) = bundle.sprite_uv(&sprite.guid) else {
                continue;
            };
            let source = &bundle.sprites[&sprite.guid];
            let color = colors[sprite.node];
            let rgba = pack(color);
            if rgba & 255 == 0 {
                continue;
            }
            let [w, h] = sprite.size;
            let [px, py] = sprite.pivot;
            let mut xy = [0.; 8];
            for (i, corner) in [
                [-px * w, (1. - py) * h, 0.],
                [(1. - px) * w, (1. - py) * h, 0.],
                [(1. - px) * w, -py * h, 0.],
                [-px * w, -py * h, 0.],
            ]
            .into_iter()
            .enumerate()
            {
                let p = world[sprite.node].point(corner);
                xy[i * 2] = 960. + p[0] * 108.;
                xy[i * 2 + 1] = 540. - p[1] * 108.;
            }
            push_quad(
                paint,
                &bundle.paths[&source.texture],
                xy,
                [uv[0], uv[3], uv[2], uv[1]],
                rgba,
                if sprite.additive { 4 } else { 0 },
            );
        }
    }
    pub fn texture_paths<'a>(&'a self, bundle: &'a Bundle) -> impl Iterator<Item = &'a str> {
        self.sprites
            .iter()
            .filter_map(|s| bundle.sprites.get(&s.guid).map(|s| s.texture.as_str()))
            .chain(self.meshes.iter().map(|m| m.texture.as_str()))
            .chain(self.particles.iter().map(|p| p.texture.as_str()))
    }
}
fn pack(color: [f32; 4]) -> u32 {
    color.into_iter().fold(0u32, |bits, c| {
        (bits << 8) | (c.clamp(0., 1.) * 255.).round() as u32
    })
}
fn push_quad(
    paint: &mut Painting,
    texture: &std::ffi::CStr,
    xy: [f32; 8],
    uv: [f32; 4],
    rgba: u32,
    reserved: u32,
) {
    let mut bounds = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for [x, y] in xy.as_chunks::<2>().0 {
        bounds[0] = bounds[0].min(*x);
        bounds[1] = bounds[1].min(*y);
        bounds[2] = bounds[2].max(*x);
        bounds[3] = bounds[3].max(*y);
    }
    paint.draws.push(AppDraw {
        kind: 3,
        rgba,
        reserved,
        xy,
        uv,
        rect: [
            bounds[0],
            bounds[1],
            bounds[2] - bounds[0],
            bounds[3] - bounds[1],
        ],
        text: texture.as_ptr(),
        clip: CANVAS,
        ..AppDraw::default()
    });
}
