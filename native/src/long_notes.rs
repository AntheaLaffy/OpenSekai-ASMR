//! LongNoteLinePreview's mesh and UV subdivision, in canvas coordinates.
use crate::minimap::easing;

#[derive(Debug, Clone)]
pub struct Quad {
    pub xy: [f32; 8],
    pub uv: [f32; 4],
}

/// Edges are [left_x, y, right_x, y]. UVs use the PNG's top-left origin.
pub fn mesh(
    start: [f32; 4],
    end: [f32; 4],
    kind: i32,
    index: usize,
    count: usize,
    uv: [f32; 4],
    clip: [f32; 4],
) -> Vec<Quad> {
    if count == 0 || start[1] == end[1] {
        return Vec::new();
    }
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t.clamp(0., 1.);
    let begin = (count.saturating_sub(index) as f32 / count as f32).clamp(0., 1.);
    let finish = (count.saturating_sub(index + 1) as f32 / count as f32).clamp(0., 1.);
    let start_v = lerp(uv[1], uv[3], begin);
    let length_v = lerp(uv[1], uv[3], finish) - start_v;
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let segments = if kind == 0 {
        1
    } else {
        (((dx * dx + dy * dy).sqrt() / 30.).ceil() as usize).max(1)
    };
    let segment_rate = 1. / segments as f32;
    // Cull only invisible segments; retained vertices use the original full
    // segment count so clipping cannot change the curve or atlas interpolation.
    let a = ((clip[1] - start[1]) / dy).clamp(0., 1.);
    let b = ((clip[1] + clip[3] - start[1]) / dy).clamp(0., 1.);
    let first = ((a.min(b) * segments as f32).floor() as usize).saturating_sub(1);
    let last = ((a.max(b) * segments as f32).ceil() as usize)
        .saturating_add(1)
        .min(segments);
    let point = |raw: f32| {
        let eased = easing(raw, kind).clamp(0., 1.);
        [
            lerp(start[0], end[0], eased),
            lerp(start[1], end[1], raw),
            lerp(start[2], end[2], eased),
            lerp(start[3], end[3], raw),
        ]
    };
    let mut result = Vec::new();
    for i in first..last {
        let t0 = (segment_rate * i as f32).clamp(0., 1.);
        let t1 = (segment_rate * (i + 1) as f32).clamp(0., 1.);
        let s = point(t0);
        let e = point(t1);
        if s[1].max(e[1]) < clip[1] || s[1].min(e[1]) > clip[1] + clip[3] {
            continue;
        }
        // Rotating the source vertex order preserves its start-right/end-left
        // diagonal with the ASM renderer's fixed 0-1-2,0-2-3 indices.
        result.push(Quad {
            xy: [s[2], s[3], s[0], s[1], e[0], e[1], e[2], e[3]],
            uv: [
                uv[2],
                start_v + length_v * t1,
                uv[0],
                start_v + length_v * t0,
            ],
        });
    }
    result
}
