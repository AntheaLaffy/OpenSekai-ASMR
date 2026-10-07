//! Atlas-backed 2D bone hierarchies and authored animation clips.
//! Pixels remain in the generated asset; joints, curves and expression slots
//! are data, so another character can reuse the same renderer and clock.
use crate::{
    animation::{Key, Transform, sample},
    ffi::AppDraw,
    ui::Painting,
    unity::CANVAS,
};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    ffi::CString,
    fs,
    path::{Component, Path},
};

fn unit_scale() -> [f32; 2] {
    [1., 1.]
}
#[derive(Deserialize)]
struct Bone {
    name: String,
    parent: Option<usize>,
    translation: [f32; 2],
    #[serde(default)]
    rotation: f32,
    #[serde(default = "unit_scale")]
    scale: [f32; 2],
}
#[derive(Deserialize)]
struct Layer {
    name: String,
    bone: usize,
    source: [f32; 4],
    rect: [f32; 4],
    #[serde(default)]
    variant: Option<[String; 2]>,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Property {
    X,
    Y,
    Rotation,
    ScaleX,
    ScaleY,
}
#[derive(Deserialize)]
struct Track {
    bone: usize,
    property: Property,
    keys: Vec<Key>,
}
#[derive(Deserialize)]
struct SlotKey {
    time: f32,
    value: String,
}
#[derive(Deserialize)]
struct Clip {
    duration: f32,
    #[serde(default)]
    repeat: bool,
    tracks: Vec<Track>,
    #[serde(default)]
    slots: BTreeMap<String, Vec<SlotKey>>,
}
#[derive(Deserialize)]
struct Definition {
    schema: u32,
    atlas: String,
    atlas_size: [f32; 2],
    canvas: [f32; 2],
    bones: Vec<Bone>,
    layers: Vec<Layer>,
    clips: BTreeMap<String, Clip>,
}
pub struct Rig {
    definition: Definition,
    texture: CString,
}
impl Rig {
    pub fn load(path: &Path) -> Result<Self, String> {
        let definition: Definition = serde_json::from_slice(
            &fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?,
        )
        .map_err(|e| format!("{}: {e}", path.display()))?;
        if definition.schema != 1
            || definition.bones.is_empty()
            || definition
                .canvas
                .iter()
                .chain(&definition.atlas_size)
                .any(|v| !v.is_finite() || *v <= 0.)
        {
            return Err("Invalid 2D rig dimensions or schema".into());
        }
        if Path::new(&definition.atlas)
            .components()
            .any(|p| !matches!(p, Component::Normal(_)))
        {
            return Err("2D rig atlas must stay inside its asset directory".into());
        }
        let atlas = path
            .parent()
            .ok_or("2D rig has no asset directory")?
            .join(&definition.atlas);
        if !atlas.is_file() {
            return Err(format!("Missing 2D rig atlas: {}", atlas.display()));
        }
        for (index, bone) in definition.bones.iter().enumerate() {
            if bone.parent.is_some_and(|p| p >= index)
                || bone
                    .translation
                    .iter()
                    .chain(&bone.scale)
                    .chain([&bone.rotation])
                    .any(|v| !v.is_finite())
            {
                return Err(format!("Invalid 2D rig bone: {}", bone.name));
            }
        }
        for layer in &definition.layers {
            let [x, y, w, h] = layer.source;
            if layer.bone >= definition.bones.len()
                || layer
                    .rect
                    .iter()
                    .chain(&layer.source)
                    .any(|v| !v.is_finite())
                || x < 0.
                || y < 0.
                || w <= 0.
                || h <= 0.
                || x + w > definition.atlas_size[0]
                || y + h > definition.atlas_size[1]
                || layer.rect[2] <= 0.
                || layer.rect[3] <= 0.
            {
                return Err(format!("Invalid 2D rig layer: {}", layer.name));
            }
        }
        for (name, clip) in &definition.clips {
            if !clip.duration.is_finite() || clip.duration <= 0. {
                return Err(format!("Invalid 2D rig clip: {name}"));
            }
            for track in &clip.tracks {
                if track.bone >= definition.bones.len()
                    || track.keys.is_empty()
                    || track
                        .keys
                        .windows(2)
                        .any(|pair| pair[1].time <= pair[0].time)
                    || track.keys.iter().any(|k| {
                        [
                            k.time,
                            k.value,
                            k.incoming,
                            k.outgoing,
                            k.in_weight,
                            k.out_weight,
                        ]
                        .iter()
                        .any(|v| !v.is_finite())
                            || k.time < 0.
                            || k.time > clip.duration
                    })
                {
                    return Err(format!("Invalid 2D rig track in {name}"));
                }
            }
            for keys in clip.slots.values() {
                if keys.is_empty()
                    || keys.windows(2).any(|pair| pair[1].time <= pair[0].time)
                    || keys
                        .iter()
                        .any(|k| !k.time.is_finite() || k.time < 0. || k.time > clip.duration)
                {
                    return Err(format!("Invalid 2D expression slot in {name}"));
                }
            }
        }
        let texture =
            CString::new(atlas.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
        Ok(Self {
            definition,
            texture,
        })
    }
    pub fn texture(&self) -> &std::ffi::CStr {
        &self.texture
    }
    fn clock(clip: &Clip, time: f32) -> f32 {
        let time = if time.is_finite() { time.max(0.) } else { 0. };
        if clip.repeat {
            time.rem_euclid(clip.duration)
        } else {
            time.min(clip.duration)
        }
    }
    fn transforms(&self, clip: &Clip, time: f32) -> Vec<Transform> {
        let mut pose: Vec<_> = self
            .definition
            .bones
            .iter()
            .map(|b| (b.translation, b.rotation, b.scale))
            .collect();
        for track in &clip.tracks {
            let value = sample(&track.keys, time);
            let (position, rotation, scale) = &mut pose[track.bone];
            match track.property {
                Property::X => position[0] += value,
                Property::Y => position[1] += value,
                Property::Rotation => *rotation += value,
                Property::ScaleX => scale[0] *= value,
                Property::ScaleY => scale[1] *= value,
            }
        }
        let mut world: Vec<Transform> = Vec::with_capacity(pose.len());
        for (index, (position, rotation, scale)) in pose.into_iter().enumerate() {
            let half = rotation.to_radians() * 0.5;
            let local = Transform::trs(
                [position[0], position[1], 0.],
                [0., 0., half.sin(), half.cos()],
                [scale[0], scale[1], 1.],
            );
            world.push(if let Some(parent) = self.definition.bones[index].parent {
                world[parent].compose(local)
            } else {
                local
            });
        }
        world
    }
    pub fn joint(&self, action: &str, time: f32, name: &str) -> Option<[f32; 2]> {
        let clip = self.definition.clips.get(action)?;
        let index = self.definition.bones.iter().position(|b| b.name == name)?;
        let p = self.transforms(clip, Self::clock(clip, time))[index].point([0., 0., 0.]);
        Some([p[0], p[1]])
    }
    pub fn draw(&self, p: &mut Painting, action: &str, time: f32, viewport: [f32; 4], alpha: f32) {
        let Some(clip) = self.definition.clips.get(action) else {
            return;
        };
        if !viewport.iter().all(|v| v.is_finite())
            || viewport[2] <= 0.
            || viewport[3] <= 0.
            || !alpha.is_finite()
        {
            return;
        }
        let time = Self::clock(clip, time);
        let transforms = self.transforms(clip, time);
        let rgba = 0xffffff00 | (alpha.clamp(0., 1.) * 255.).round() as u32;
        if rgba & 255 == 0 {
            return;
        }
        let [sx, sy] = [
            viewport[2] / self.definition.canvas[0],
            viewport[3] / self.definition.canvas[1],
        ];
        for layer in &self.definition.layers {
            if let Some([slot, value]) = &layer.variant {
                let selected = clip.slots.get(slot).and_then(|keys| {
                    let i = keys.partition_point(|k| k.time <= time);
                    keys.get(i.saturating_sub(1))
                });
                if selected.is_none_or(|k| k.value != *value) {
                    continue;
                }
            }
            let [x, y, w, h] = layer.rect;
            let mut xy = [0.; 8];
            for (i, [x, y]) in [[x, y], [x + w, y], [x + w, y + h], [x, y + h]]
                .into_iter()
                .enumerate()
            {
                let q = transforms[layer.bone].point([x, y, 0.]);
                xy[i * 2] = viewport[0] + q[0] * sx;
                xy[i * 2 + 1] = viewport[1] + q[1] * sy;
            }
            let xs = [xy[0], xy[2], xy[4], xy[6]];
            let ys = [xy[1], xy[3], xy[5], xy[7]];
            let left = xs.into_iter().fold(f32::INFINITY, f32::min);
            let top = ys.into_iter().fold(f32::INFINITY, f32::min);
            let [u, v, uw, vh] = layer.source;
            let [tw, th] = self.definition.atlas_size;
            p.draws.push(AppDraw {
                kind: 3,
                rgba,
                xy,
                rect: [
                    left,
                    top,
                    xs.into_iter().fold(f32::NEG_INFINITY, f32::max) - left,
                    ys.into_iter().fold(f32::NEG_INFINITY, f32::max) - top,
                ],
                clip: CANVAS,
                uv: [u / tw, (v + vh) / th, (u + uw) / tw, v / th],
                text: self.texture.as_ptr(),
                ..AppDraw::default()
            });
        }
    }
}
