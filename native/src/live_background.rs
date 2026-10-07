//! Original jacket-stage sprites, independent of the native gameplay model.
use crate::{
    ffi::AppDraw,
    ui::Painting,
    unity::{Bundle, CANVAS},
};
use serde::Deserialize;
use std::ffi::CStr;

#[derive(Default, Deserialize)]
pub struct Background {
    pub sprites: Vec<Sprite>,
}
#[derive(Deserialize)]
pub struct Sprite {
    pub guid: String,
    pub jacket: bool,
    pub base: bool,
    pub xy: [f32; 8],
    pub clip: [f32; 4],
    pub color: [f32; 4],
}
impl Background {
    pub fn draw(&self, bundle: &Bundle, jacket: Option<&CStr>, paint: &mut Painting) {
        self.draw_layers(bundle, jacket, paint, false);
    }
    pub fn draw_base(&self, bundle: &Bundle, paint: &mut Painting) {
        self.draw_layers(bundle, None, paint, true);
    }
    fn draw_layers(
        &self,
        bundle: &Bundle,
        jacket: Option<&CStr>,
        paint: &mut Painting,
        base_only: bool,
    ) {
        for sprite in self.sprites.iter().filter(|s| !base_only || s.base) {
            let (texture, uv) = if sprite.jacket {
                let Some(path) = jacket else {
                    continue;
                };
                (path, [0., 1., 1., 0.])
            } else {
                let Some(asset) = bundle.sprites.get(&sprite.guid) else {
                    continue;
                };
                let Some(uv) = bundle.sprite_uv(&sprite.guid) else {
                    continue;
                };
                (
                    bundle.paths[&asset.texture].as_c_str(),
                    [uv[0], uv[3], uv[2], uv[1]],
                )
            };
            let rgba = sprite.color.into_iter().fold(0u32, |bits, c| {
                (bits << 8) | (c.clamp(0., 1.) * 255.).round() as u32
            });
            let xs = [sprite.xy[0], sprite.xy[2], sprite.xy[4], sprite.xy[6]];
            let ys = [sprite.xy[1], sprite.xy[3], sprite.xy[5], sprite.xy[7]];
            let (x, y) = (
                xs.into_iter().fold(f32::INFINITY, f32::min),
                ys.into_iter().fold(f32::INFINITY, f32::min),
            );
            let w = xs.into_iter().fold(f32::NEG_INFINITY, f32::max) - x;
            let h = ys.into_iter().fold(f32::NEG_INFINITY, f32::max) - y;
            paint.draws.push(AppDraw {
                kind: 3,
                reserved: 16, // Source sprites are planar camera projections.
                rgba,
                xy: sprite.xy,
                uv,
                rect: [x, y, w, h],
                text: texture.as_ptr(),
                clip: crate::unity::intersect(sprite.clip, CANVAS),
                ..AppDraw::default()
            });
        }
    }
    pub fn texture_paths<'a>(&'a self, bundle: &'a Bundle) -> impl Iterator<Item = &'a str> {
        self.sprites
            .iter()
            .filter_map(|s| bundle.sprites.get(&s.guid).map(|s| s.texture.as_str()))
    }
}
