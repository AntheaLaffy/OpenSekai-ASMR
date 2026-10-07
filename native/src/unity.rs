//! Unity UI data execution. Source fileIDs and RectTransform values are retained.
use crate::{ffi::AppDraw, ui::Painting};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, ffi::CString, path::Path, sync::Arc};

pub const CANVAS: [f32; 4] = [0., 0., 1920., 1080.];
const HORIZONTAL: &str = "30649d3a9faa99c48a7b1166b86bf2a0";
const VERTICAL: &str = "59f8146938fff824cb5fd77236b75775";
const GRID: &str = "8a8695521f0d02e499659fee002a26c2";
const RECT_MASK: &str = "3312d7739989d2b4e91e6319e9a96d76";

pub fn number(v: &Value, key: &str, default: f32) -> f32 {
    v[key].as_f64().map(|v| v as f32).unwrap_or(default)
}
pub fn reference(v: &Value) -> i64 {
    v["fileID"].as_i64().unwrap_or(0)
}
fn vector(v: &Value, default: [f32; 2]) -> [f32; 2] {
    [number(v, "x", default[0]), number(v, "y", default[1])]
}
pub fn color(v: &Value, alpha: f32) -> u32 {
    let channel = |name: &str, multiplier: f32| {
        (number(v, name, 1.) * multiplier * 255.)
            .round()
            .clamp(0., 255.) as u32
    };
    channel("r", 1.) << 24 | channel("g", 1.) << 16 | channel("b", 1.) << 8 | channel("a", alpha)
}
pub fn intersect(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let x = a[0].max(b[0]);
    let y = a[1].max(b[1]);
    [
        x,
        y,
        ((a[0] + a[2]).min(b[0] + b[2]) - x).max(0.),
        ((a[1] + a[3]).min(b[1] + b[3]) - y).max(0.),
    ]
}
pub fn inside(rect: [f32; 4], x: f32, y: f32) -> bool {
    x >= rect[0] && y >= rect[1] && x < rect[0] + rect[2] && y < rect[1] + rect[3]
}

#[derive(Debug, Clone, Deserialize)]
pub struct Sprite {
    pub name: String,
    pub texture: String,
    pub rect: [f32; 4],
    pub border: [f32; 4], // left, bottom, right, top in source pixels
    pub pixels_per_unit: f32,
    pub settings_raw: u32,
}
#[derive(Deserialize)]
pub struct Bundle {
    pub schema: u32,
    pub main: String,
    pub note: String,
    pub prefabs: HashMap<String, Value>,
    pub sprites: HashMap<String, Sprite>,
    pub textures: HashMap<String, [f32; 2]>,
    pub wording_zh: HashMap<String, String>,
    #[serde(default)]
    pub live: Value,
    pub presentations: crate::live_result::Library,
    #[serde(default)]
    pub live_background: crate::live_background::Background,
    #[serde(skip)]
    pub paths: HashMap<String, CString>,
}
impl Bundle {
    pub fn quad(
        &self,
        paint: &mut Painting,
        guid: &str,
        quad: &crate::long_notes::Quad,
        clip: [f32; 4],
        selected: bool,
    ) {
        let Some(sprite) = self.sprites.get(guid) else {
            return;
        };
        let Some(path) = self.paths.get(&sprite.texture) else {
            return;
        };
        let mut bounds = [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        for p in quad.xy.as_chunks::<2>().0 {
            bounds[0] = bounds[0].min(p[0]);
            bounds[1] = bounds[1].min(p[1]);
            bounds[2] = bounds[2].max(p[0]);
            bounds[3] = bounds[3].max(p[1]);
        }
        let rect = [
            bounds[0],
            bounds[1],
            bounds[2] - bounds[0],
            bounds[3] - bounds[1],
        ];
        let visible = intersect(rect, clip);
        if visible[2] <= 0. || visible[3] <= 0. {
            return;
        }
        paint.draws.push(AppDraw {
            kind: 3,
            rgba: 0xffffffff,
            rect,
            clip,
            xy: quad.xy,
            uv: quad.uv,
            text: path.as_ptr(),
            reserved: if selected { 2 } else { 0 },
            ..AppDraw::default()
        });
    }
    pub fn sprite_uv(&self, guid: &str) -> Option<[f32; 4]> {
        let sprite = self.sprites.get(guid)?;
        let [tw, th] = self.textures.get(&sprite.texture)?;
        let [x, y, w, h] = sprite.rect;
        Some([x / tw, 1. - (y + h) / th, (x + w) / tw, 1. - y / th])
    }
    pub fn load(project: &Path) -> Result<Arc<Self>, String> {
        let path = project.join("native/generated/editor-scene.json");
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("{}: {e}; run make opensekai-assets", path.display()))?;
        let mut bundle: Self = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        if bundle.schema != 1 {
            return Err("Unsupported native scene schema".into());
        }
        for path in bundle.textures.keys() {
            let full = project.join(path);
            if !full.is_file() {
                return Err(format!("Original texture missing: {}", full.display()));
            }
            bundle.paths.insert(
                path.clone(),
                CString::new(full.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?,
            );
        }
        Ok(Arc::new(bundle))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sprite(
        &self,
        paint: &mut Painting,
        guid: &str,
        mut rect: [f32; 4],
        clip: [f32; 4],
        rgba: u32,
        sliced: bool,
        preserve_aspect: bool,
        ppu_multiplier: f32,
        rotation: f32,
        flip_y: bool,
    ) {
        let Some(sprite) = self.sprites.get(guid) else {
            return;
        };
        let Some(path) = self.paths.get(&sprite.texture) else {
            return;
        };
        let Some(&[tw, th]) = self.textures.get(&sprite.texture) else {
            return;
        };
        let [sx, sy, sw, sh] = sprite.rect;
        if rect[2] <= 0. || rect[3] <= 0. || sw <= 0. || sh <= 0. {
            return;
        }
        if preserve_aspect && !sliced {
            let scale = (rect[2] / sw).min(rect[3] / sh);
            rect[0] += (rect[2] - sw * scale) * 0.5;
            rect[1] += (rect[3] - sh * scale) * 0.5;
            rect[2] = sw * scale;
            rect[3] = sh * scale;
        }
        let emit = |paint: &mut Painting, local: [f32; 4], mut uv: [f32; 4]| {
            let cx = local[0] + local[2] * 0.5 - rect[2] * 0.5;
            let cy = local[1] + local[3] * 0.5 - rect[3] * 0.5;
            let (sin, cos) = rotation.sin_cos();
            let x = rect[0] + rect[2] * 0.5 + cos * cx + sin * cy - local[2] * 0.5;
            let y = rect[1] + rect[3] * 0.5 - sin * cx + cos * cy - local[3] * 0.5;
            if flip_y {
                uv.swap(1, 3);
            }
            paint.draws.push(AppDraw {
                kind: 2,
                rgba,
                rect: [x, y, local[2], local[3]],
                clip,
                uv,
                rotation,
                text: path.as_ptr(),
                ..AppDraw::default()
            });
        };
        if !sliced || sprite.border == [0.; 4] {
            emit(
                paint,
                [0., 0., rect[2], rect[3]],
                [sx / tw, (th - sy - sh) / th, (sx + sw) / tw, (th - sy) / th],
            );
            return;
        }
        let [left, bottom, right, top] = sprite.border;
        // The original MusicScoreMaker Canvas has referencePixelsPerUnit = 1.
        let ppu = (sprite.pixels_per_unit * ppu_multiplier).max(0.001);
        let mut border = [left / ppu, top / ppu, right / ppu, bottom / ppu];
        for (a, b, size) in [(0, 2, rect[2]), (1, 3, rect[3])] {
            if border[a] + border[b] > size {
                let scale = size / (border[a] + border[b]);
                border[a] *= scale;
                border[b] *= scale;
            }
        }
        let x = [0., border[0], rect[2] - border[2], rect[2]];
        let y = [0., border[1], rect[3] - border[3], rect[3]];
        let u = [
            sx / tw,
            (sx + left) / tw,
            (sx + sw - right) / tw,
            (sx + sw) / tw,
        ];
        let v = [
            (th - sy - sh) / th,
            (th - sy - sh + top) / th,
            (th - sy - bottom) / th,
            (th - sy) / th,
        ];
        for row in 0..3 {
            for col in 0..3 {
                let w = x[col + 1] - x[col];
                let h = y[row + 1] - y[row];
                if w > 0. && h > 0. {
                    emit(
                        paint,
                        [x[col], y[row], w, h],
                        [u[col], v[row], u[col + 1], v[row + 1]],
                    );
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct Component {
    pub id: i64,
    pub kind: String,
    pub guid: String,
    pub fields: Value,
}
#[derive(Clone)]
pub struct SceneNode {
    pub id: i64,
    pub transform_id: i64,
    pub name: String,
    pub active: bool,
    pub transform: Value,
    pub components: Vec<Component>,
    pub children: Vec<usize>,
    pub parent: Option<usize>,
    pub rect: [f32; 4],
    pub size: [f32; 2],
    pub rotation: f32,
    pub flip_y: bool,
    pub visible: bool,
    pub clip: [f32; 4],
    pub alpha: f32,
    matrix: [f32; 6],
}
impl SceneNode {
    pub fn component(&self, kind: &str) -> Option<&Component> {
        self.components.iter().find(|c| c.kind == kind)
    }
    fn layout_component(&self) -> Option<&Component> {
        self.components.iter().find(|c| {
            (c.guid == HORIZONTAL || c.guid == VERTICAL || c.guid == GRID)
                && number(&c.fields, "m_Enabled", 1.) != 0.
        })
    }
}
pub struct Scene {
    pub bundle: Arc<Bundle>,
    pub nodes: Vec<SceneNode>,
    roots: Vec<usize>,
    references: HashMap<i64, usize>,
}
impl Scene {
    pub fn new(bundle: Arc<Bundle>, path: &str) -> Result<Self, String> {
        let objects = bundle
            .prefabs
            .get(path)
            .and_then(|v| v["objects"].as_array())
            .ok_or("Prefab missing")?;
        let by_id: HashMap<_, _> = objects
            .iter()
            .filter_map(|o| Some((o["id"].as_i64()?, o)))
            .collect();
        let mut nodes = Vec::new();
        let mut references = HashMap::new();
        for object in objects.iter().filter(|o| o["kind"] == "RectTransform") {
            let transform_id = object["id"].as_i64().ok_or("Invalid fileID")?;
            let fields = &object["fields"];
            let id = reference(&fields["m_GameObject"]);
            let game = by_id.get(&id).ok_or("Missing GameObject")?;
            let mut components = Vec::new();
            for value in game["fields"]["m_Component"]
                .as_array()
                .ok_or("Missing components")?
            {
                let cid = reference(&value["component"]);
                let Some(component) = by_id.get(&cid) else {
                    continue;
                };
                let kind = component["script"]["types"][0]
                    .as_str()
                    .or_else(|| component["kind"].as_str())
                    .unwrap_or("")
                    .to_owned();
                if matches!(kind.as_str(), "RectTransform" | "CanvasRenderer") {
                    continue;
                }
                references.insert(cid, nodes.len());
                components.push(Component {
                    id: cid,
                    kind,
                    guid: component["fields"]["m_Script"]["guid"]
                        .as_str()
                        .unwrap_or("")
                        .into(),
                    fields: component["fields"].clone(),
                });
            }
            references.insert(id, nodes.len());
            references.insert(transform_id, nodes.len());
            nodes.push(SceneNode {
                id,
                transform_id,
                name: game["fields"]["m_Name"].as_str().unwrap_or("").into(),
                active: game["fields"]["m_IsActive"].as_i64().unwrap_or(1) != 0,
                transform: fields.clone(),
                components,
                children: Vec::new(),
                parent: None,
                rect: [0.; 4],
                size: [0.; 2],
                rotation: 0.,
                flip_y: false,
                visible: false,
                clip: CANVAS,
                alpha: 1.,
                matrix: [1., 0., 0., 1., 0., 0.],
            });
        }
        let mut roots = Vec::new();
        for (i, node) in nodes.iter_mut().enumerate() {
            node.parent = references
                .get(&reference(&node.transform["m_Father"]))
                .copied();
            if node.parent.is_none() {
                roots.push(i);
            }
            node.children = node.transform["m_Children"]
                .as_array()
                .map(|children| {
                    children
                        .iter()
                        .filter_map(|v| references.get(&reference(v)).copied())
                        .collect()
                })
                .unwrap_or_default();
        }
        Ok(Self {
            bundle,
            nodes,
            roots,
            references,
        })
    }
    pub fn find(&self, name: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.name == name)
    }
    pub fn by_reference(&self, id: i64) -> Option<usize> {
        self.references.get(&id).copied()
    }
    pub fn set_reference_active(&mut self, id: i64, active: bool) {
        if let Some(i) = self.by_reference(id) {
            self.nodes[i].active = active;
        }
    }
    pub fn set_text(&mut self, id: i64, text: &str) {
        if let Some(i) = self.by_reference(id) {
            for component in &mut self.nodes[i].components {
                if component.id == id {
                    component.fields["m_text"] = text.into();
                    component.fields["useWordingKey"] = 0.into();
                }
            }
        }
    }
    pub fn set_sprite_name(&mut self, id: i64, name: &str) -> bool {
        let Some(guid) = self
            .bundle
            .sprites
            .iter()
            .find(|(_, s)| s.name == name)
            .map(|(guid, _)| guid.clone())
        else {
            return false;
        };
        if let Some(i) = self.by_reference(id) {
            for c in &mut self.nodes[i].components {
                if c.id == id {
                    c.fields["m_Sprite"] =
                        serde_json::json!({"fileID":21300000,"guid":guid,"type":2});
                    return true;
                }
            }
        }
        false
    }
    pub fn component(&self, kind: &str) -> Option<&Component> {
        self.nodes.iter().find_map(|n| n.component(kind))
    }

    pub fn layout(&mut self) {
        for root in self.roots.clone() {
            self.arrange(
                root,
                [1920., 1080.],
                [1., 0., 0., 1., 0., 0.],
                CANVAS,
                true,
                1.,
                None,
                0,
            );
        }
    }

    fn preferred(&self, index: usize, axis: usize, fallback: f32) -> f32 {
        for c in &self.nodes[index].components {
            let key = if axis == 0 {
                "m_PreferredWidth"
            } else {
                "m_PreferredHeight"
            };
            let min_key = if axis == 0 {
                "m_MinWidth"
            } else {
                "m_MinHeight"
            };
            if c.fields.get(key).is_some() {
                return number(&c.fields, key, -1.)
                    .max(number(&c.fields, min_key, -1.))
                    .max(if number(&c.fields, key, -1.) < 0. {
                        fallback
                    } else {
                        0.
                    });
            }
        }
        fallback
    }
    #[allow(clippy::too_many_arguments)]
    fn arrange(
        &mut self,
        index: usize,
        parent_size: [f32; 2],
        parent_matrix: [f32; 6],
        parent_clip: [f32; 4],
        active: bool,
        parent_alpha: f32,
        override_rect: Option<[f32; 4]>,
        depth: usize,
    ) {
        if depth > 128 {
            return;
        }
        let node = &self.nodes[index];
        let pivot = vector(&node.transform["m_Pivot"], [0.5; 2]);
        let min = vector(&node.transform["m_AnchorMin"], [0.5; 2]);
        let max = vector(&node.transform["m_AnchorMax"], [0.5; 2]);
        let delta = vector(&node.transform["m_SizeDelta"], [0.; 2]);
        let anchored = vector(&node.transform["m_AnchoredPosition"], [0.; 2]);
        let mut local = override_rect.unwrap_or([
            parent_size[0] * min[0] + anchored[0] - delta[0] * pivot[0],
            parent_size[1] * (1. - max[1]) - anchored[1] - delta[1] * (1. - pivot[1]),
            parent_size[0] * (max[0] - min[0]) + delta[0],
            parent_size[1] * (max[1] - min[1]) + delta[1],
        ]);
        let group = node.layout_component().cloned();
        let mut child_rects = HashMap::new();
        if let Some(group) = group.as_ref().filter(|g| g.guid == GRID) {
            let children: Vec<_> = node
                .children
                .iter()
                .copied()
                .filter(|&c| {
                    self.nodes[c].active
                        && !self.nodes[c]
                            .components
                            .iter()
                            .any(|v| number(&v.fields, "m_IgnoreLayout", 0.) != 0.)
                })
                .collect();
            let cell = vector(&group.fields["m_CellSize"], [100.; 2]);
            let spacing = vector(&group.fields["m_Spacing"], [0.; 2]);
            let p = &group.fields["m_Padding"];
            let pad = [
                number(p, "m_Left", 0.),
                number(p, "m_Top", 0.),
                number(p, "m_Right", 0.),
                number(p, "m_Bottom", 0.),
            ];
            let constraint = number(&group.fields, "m_Constraint", 0.) as u32;
            let count = (number(&group.fields, "m_ConstraintCount", 1.) as usize).max(1);
            let cols = match constraint {
                1 => count,
                2 => children.len().div_ceil(count).max(1),
                _ => (((local[2] - pad[0] - pad[2] + spacing[0]) / (cell[0] + spacing[0]).max(1.))
                    .floor() as usize)
                    .max(1),
            };
            let rows = children.len().div_ceil(cols).max(1);
            let preferred = [
                pad[0]
                    + pad[2]
                    + cell[0] * cols as f32
                    + spacing[0] * cols.saturating_sub(1) as f32,
                pad[1]
                    + pad[3]
                    + cell[1] * rows as f32
                    + spacing[1] * rows.saturating_sub(1) as f32,
            ];
            if let Some(fitter) = node
                .components
                .iter()
                .find(|c| c.fields.get("m_HorizontalFit").is_some())
            {
                for axis in 0..2 {
                    if number(
                        &fitter.fields,
                        if axis == 0 {
                            "m_HorizontalFit"
                        } else {
                            "m_VerticalFit"
                        },
                        0.,
                    ) != 0.
                    {
                        local[axis + 2] = preferred[axis];
                    }
                }
            }
            let corner = number(&group.fields, "m_StartCorner", 0.) as usize;
            let start_axis = number(&group.fields, "m_StartAxis", 0.) as usize;
            for (i, &child) in children.iter().enumerate() {
                let (mut col, mut row) = if start_axis == 0 {
                    (i % cols, i / cols)
                } else {
                    (i / rows, i % rows)
                };
                if !corner.is_multiple_of(2) {
                    col = cols - 1 - col;
                }
                if corner / 2 != 0 {
                    row = rows - 1 - row;
                }
                child_rects.insert(
                    child,
                    [
                        pad[0] + col as f32 * (cell[0] + spacing[0]),
                        pad[1] + row as f32 * (cell[1] + spacing[1]),
                        cell[0],
                        cell[1],
                    ],
                );
            }
        } else if let Some(group) = &group {
            let vertical = group.guid == VERTICAL;
            let axis = usize::from(vertical);
            let children: Vec<_> = node
                .children
                .iter()
                .copied()
                .filter(|&c| {
                    self.nodes[c].active
                        && !self.nodes[c]
                            .components
                            .iter()
                            .any(|v| number(&v.fields, "m_IgnoreLayout", 0.) != 0.)
                })
                .collect();
            let padding = &group.fields["m_Padding"];
            let pad = [
                number(padding, "m_Left", 0.),
                number(padding, "m_Top", 0.),
                number(padding, "m_Right", 0.),
                number(padding, "m_Bottom", 0.),
            ];
            let spacing = number(&group.fields, "m_Spacing", 0.);
            let sizes: Vec<_> = children
                .iter()
                .map(|&c| {
                    let s = vector(&self.nodes[c].transform["m_SizeDelta"], [0.; 2]);
                    [self.preferred(c, 0, s[0]), self.preferred(c, 1, s[1])]
                })
                .collect();
            let total = sizes.iter().map(|s| s[axis]).sum::<f32>()
                + spacing * children.len().saturating_sub(1) as f32
                + pad[axis]
                + pad[axis + 2];
            let cross = sizes.iter().map(|s| s[1 - axis]).fold(0., f32::max)
                + pad[1 - axis]
                + pad[3 - axis];
            if let Some(fitter) = node
                .components
                .iter()
                .find(|c| c.fields.get("m_HorizontalFit").is_some())
            {
                for a in 0..2 {
                    let key = if a == 0 {
                        "m_HorizontalFit"
                    } else {
                        "m_VerticalFit"
                    };
                    if number(&fitter.fields, key, 0.) != 0. {
                        local[2 + a] = if a == axis { total } else { cross };
                    }
                }
            }
            let alignment = number(&group.fields, "m_ChildAlignment", 0.) as i32;
            let alignment = [(alignment % 3) as f32 * 0.5, (alignment / 3) as f32 * 0.5];
            let mut cursor = pad[axis] + (local[2 + axis] - total).max(0.) * alignment[axis];
            for (&c, size) in children.iter().zip(sizes) {
                let mut r = [0., 0., size[0], size[1]];
                let other = 1 - axis;
                let force_key = if other == 0 {
                    "m_ChildForceExpandWidth"
                } else {
                    "m_ChildForceExpandHeight"
                };
                let control_key = if other == 0 {
                    "m_ChildControlWidth"
                } else {
                    "m_ChildControlHeight"
                };
                if number(&group.fields, force_key, 0.) != 0.
                    && number(&group.fields, control_key, 0.) != 0.
                {
                    r[2 + other] = (local[2 + other] - pad[other] - pad[other + 2]).max(0.);
                }
                r[axis] = cursor;
                r[other] = pad[other]
                    + (local[2 + other] - pad[other] - pad[other + 2] - r[2 + other])
                        * alignment[other];
                cursor += r[2 + axis] + spacing;
                child_rects.insert(c, r);
            }
        }
        let node = &self.nodes[index];
        let scale = vector(&node.transform["m_LocalScale"], [1.; 2]);
        let q = &node.transform["m_LocalRotation"];
        let qx = number(q, "x", 0.);
        let qy = number(q, "y", 0.);
        let qz = number(q, "z", 0.);
        let qw = number(q, "w", 1.);
        let a = (1. - 2. * (qy * qy + qz * qz)) * scale[0];
        let b = -2. * (qx * qy - qz * qw) * scale[1];
        let c = -2. * (qx * qy + qz * qw) * scale[0];
        let d = (1. - 2. * (qx * qx + qz * qz)) * scale[1];
        let px = local[2] * pivot[0];
        let py = local[3] * (1. - pivot[1]);
        let x = local[0] + px - a * px - b * py;
        let y = local[1] + py - c * px - d * py;
        let [pa, pb, pc, pd, ptx, pty] = parent_matrix;
        let matrix = [
            pa * a + pb * c,
            pa * b + pb * d,
            pc * a + pd * c,
            pc * b + pd * d,
            pa * x + pb * y + ptx,
            pc * x + pd * y + pty,
        ];
        let [a, b, c, d, tx, ty] = matrix;
        let center = [
            tx + a * local[2] * 0.5 + b * local[3] * 0.5,
            ty + c * local[2] * 0.5 + d * local[3] * 0.5,
        ];
        let width = local[2] * a.hypot(c);
        let height = local[3] * b.hypot(d);
        let rect = [
            center[0] - width * 0.5,
            center[1] - height * 0.5,
            width,
            height,
        ];
        let group_alpha = node
            .component("CanvasGroup")
            .map(|c| number(&c.fields, "m_Alpha", 1.))
            .unwrap_or(1.);
        let visible = active && node.active;
        let alpha = parent_alpha * group_alpha;
        let mask = node
            .components
            .iter()
            .any(|c| c.guid == RECT_MASK && number(&c.fields, "m_Enabled", 1.) != 0.);
        let clip = if mask {
            intersect(parent_clip, rect)
        } else {
            parent_clip
        };
        let children = node.children.clone();
        let node = &mut self.nodes[index];
        node.size = [local[2], local[3]];
        node.matrix = matrix;
        node.rect = rect;
        node.rotation = (-c).atan2(a);
        node.flip_y = a * d - b * c < 0.;
        node.visible = visible;
        node.clip = clip;
        node.alpha = alpha;
        for child in children {
            self.arrange(
                child,
                [local[2], local[3]],
                matrix,
                clip,
                visible,
                alpha,
                child_rects.get(&child).copied(),
                depth + 1,
            );
        }
    }

    pub fn draw(&self, paint: &mut Painting, english: bool) {
        self.draw_with(paint, english, &mut |_, _| {});
    }
    pub fn draw_with(
        &self,
        paint: &mut Painting,
        english: bool,
        extra: &mut dyn FnMut(&SceneNode, &mut Painting),
    ) {
        for &root in &self.roots {
            self.draw_node(root, paint, english, extra, 0);
        }
    }
    fn draw_node(
        &self,
        index: usize,
        paint: &mut Painting,
        english: bool,
        extra: &mut dyn FnMut(&SceneNode, &mut Painting),
        depth: usize,
    ) {
        if depth > 128 {
            return;
        }
        let node = &self.nodes[index];
        if !node.visible || node.alpha <= 0. {
            return;
        }
        for component in &node.components {
            let fields = &component.fields;
            if number(fields, "m_Enabled", 1.) == 0. {
                continue;
            }
            if fields.get("m_Sprite").is_some() && fields.get("m_Color").is_some() {
                let rgba = color(&fields["m_Color"], node.alpha);
                if rgba & 255 == 0 {
                    continue;
                }
                if let Some(guid) = fields["m_Sprite"]["guid"].as_str() {
                    self.bundle.sprite(
                        paint,
                        guid,
                        node.rect,
                        node.clip,
                        rgba,
                        matches!(number(fields, "m_Type", 0.) as u32, 1 | 2),
                        number(fields, "m_PreserveAspect", 0.) != 0.,
                        number(fields, "m_PixelsPerUnitMultiplier", 1.),
                        node.rotation,
                        node.flip_y,
                    );
                } else if node.rect[2] > 0. && node.rect[3] > 0. {
                    paint.rect(node.rect, rgba, node.clip);
                    paint.draws.last_mut().unwrap().rotation = node.rotation;
                }
            } else if component.kind == "CustomTextMesh" {
                let key = fields["wordingKey"].as_str().unwrap_or("");
                let authored = fields["m_text"].as_str().unwrap_or("");
                let text = if number(fields, "useWordingKey", 0.) != 0. {
                    if english {
                        wording_en(key)
                            .or_else(|| self.bundle.wording_zh.get(key).map(String::as_str))
                            .unwrap_or(authored)
                    } else {
                        self.bundle
                            .wording_zh
                            .get(key)
                            .map(String::as_str)
                            .unwrap_or(authored)
                    }
                } else {
                    authored
                };
                if text.is_empty() {
                    continue;
                }
                let align = match number(fields, "m_HorizontalAlignment", 1.) as u32 {
                    2 => 1,
                    4 => 2,
                    _ => 0,
                };
                let scale = if node.size[1].abs() > 0.001 {
                    node.rect[3] / node.size[1]
                } else {
                    1.
                };
                let font_size = number(fields, "m_fontSize", 24.) * scale;
                paint.text(
                    text,
                    node.rect,
                    font_size,
                    true,
                    align,
                    color(&fields["m_fontColor"], node.alpha),
                    node.clip,
                );
                let draw = paint.draws.last_mut().unwrap();
                draw.rotation = node.rotation;
                if number(fields, "m_enableAutoSizing", 0.) != 0. {
                    draw.min_font_size = number(fields, "m_fontSizeMin", 6.) * scale;
                    draw.reserved |= 1;
                }
            }
        }
        extra(node, paint);
        for &child in &node.children {
            self.draw_node(child, paint, english, extra, depth + 1);
        }
    }
}

fn wording_en(key: &str) -> Option<&'static str> {
    Some(match key {
        "WORD_MIRROR_FLIP" => "Mirror",
        "WORD_VERTICAL_FLIP" => "Flip vertically",
        "WORD_HOW_TO_USE" => "How to use",
        "WORD_POSTS" => "Publish",
        "WORD_OPTION" => "Options",
        "WORD_CRITICAL" => "Critical",
        "WORD_OFF" => "OFF",
        "WORD_ON" => "ON",
        "WORD_NOTES_COMBO_COUNT" => "Note count",
        "WORD_RESET" => "Reset",
        "WORD_MUSIC_SCORE_MAKER_SAVE_DRAFT" => "Save draft",
        "WORD_CHANGE_2" => "Change",
        "WORD_COPY" => "Copy",
        "WORD_TEST_PLAY" => "Test play",
        "WORD_DELETE" => "Delete",
        "WORD_SAVE" => "Save",
        "WORD_SELECT_LONG_ALL_BUTTON" => "Select connected",
        _ => return None,
    })
}
