//! Scalar Unity animation curves, including weighted tangents and step keys.
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Key {
    pub time: f32,
    pub value: f32,
    pub incoming: f32,
    pub outgoing: f32,
    pub in_weight: f32,
    pub out_weight: f32,
    pub step: bool,
    pub step_in: bool,
}
fn bezier(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    let u = 1. - t;
    u * u * u * a + 3. * u * u * t * b + 3. * u * t * t * c + t * t * t * d
}
pub fn sample(keys: &[Key], time: f32) -> f32 {
    let Some(first) = keys.first() else { return 0. };
    if time <= first.time {
        return first.value;
    }
    let next = keys.partition_point(|k| k.time <= time);
    if next == keys.len() {
        return keys[next - 1].value;
    }
    let (a, b) = (&keys[next - 1], &keys[next]);
    let length = b.time - a.time;
    if a.step || b.step_in || length <= 0. {
        return a.value;
    }
    let x = (time - a.time) / length;
    let wa = a.out_weight.clamp(0., 1.);
    let wb = b.in_weight.clamp(0., 1.);
    let t = if (wa - 1. / 3.).abs() < 1e-6 && (wb - 1. / 3.).abs() < 1e-6 {
        x
    } else {
        // Tangent weights move Bezier control points on the time axis too.
        let (mut lo, mut hi) = (0., 1.);
        for _ in 0..24 {
            let mid = (lo + hi) * 0.5;
            if bezier(0., wa, 1. - wb, 1., mid) < x {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (lo + hi) * 0.5
    };
    bezier(
        a.value,
        a.value + a.outgoing * length * wa,
        b.value - b.incoming * length * wb,
        b.value,
        t,
    )
}

#[derive(Clone, Copy, Debug)]
pub struct Transform(pub [[f32; 4]; 3]);
impl Transform {
    pub const IDENTITY: Self = Self([[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.]]);
    pub fn trs(p: [f32; 3], q: [f32; 4], s: [f32; 3]) -> Self {
        let [x, y, z, w] = q;
        let mut m = [
            [
                1. - 2. * (y * y + z * z),
                2. * (x * y - z * w),
                2. * (x * z + y * w),
                p[0],
            ],
            [
                2. * (x * y + z * w),
                1. - 2. * (x * x + z * z),
                2. * (y * z - x * w),
                p[1],
            ],
            [
                2. * (x * z - y * w),
                2. * (y * z + x * w),
                1. - 2. * (x * x + y * y),
                p[2],
            ],
        ];
        for row in &mut m {
            for (i, scale) in s.iter().enumerate() {
                row[i] *= scale;
            }
        }
        Self(m)
    }
    pub fn point(self, p: [f32; 3]) -> [f32; 3] {
        self.0
            .map(|r| r[0] * p[0] + r[1] * p[1] + r[2] * p[2] + r[3])
    }
    pub fn compose(self, child: Self) -> Self {
        let mut m = [[0.; 4]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, value) in row.iter_mut().enumerate() {
                *value = (0..3).map(|k| self.0[i][k] * child.0[k][j]).sum::<f32>();
                if j == 3 {
                    *value += self.0[i][3];
                }
            }
        }
        Self(m)
    }
}
