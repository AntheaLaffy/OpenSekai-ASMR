use opensekai::animation::{Key, Transform, sample};

fn key(time: f32, value: f32) -> Key {
    Key {
        time,
        value,
        incoming: 0.,
        outgoing: 0.,
        in_weight: 1. / 3.,
        out_weight: 1. / 3.,
        step: false,
        step_in: false,
    }
}
fn close(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-5, "{a} != {b}");
}

#[test]
fn curves_preserve_tangents_weighted_time_and_step_boundaries() {
    let mut keys = [key(2., 0.), key(4., 1.)];
    close(sample(&keys, 2.5), 0.15625); // Hermite smoothstep at 1/4.
    close(sample(&keys, -1.), 0.);
    close(sample(&keys, 8.), 1.);
    keys[0].outgoing = 0.5;
    keys[1].incoming = 0.5;
    close(sample(&keys, 2.5), 0.25); // Straight line with nonzero slopes.
    keys[0].outgoing = 0.;
    keys[1].incoming = 0.;
    keys[0].out_weight = 0.1;
    keys[1].in_weight = 0.5;
    // Bezier parameter 1/2 gives x=.35, y=.5; using x as t is wrong.
    close(sample(&keys, 2.7), 0.5);
    keys[1].step_in = true;
    close(sample(&keys, 3.999), 0.);
    close(sample(&keys, 4.), 1.);
    keys[1].step_in = false;
    keys[0].step = true;
    close(sample(&keys, 3.999), 0.);
}

#[test]
fn hierarchy_applies_parent_rotation_and_nonuniform_scale_to_children() {
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let parent = Transform::trs([10., 20., 30.], [0., 0., h, h], [2., 3., 4.]);
    let child = Transform::trs([1., 2., 3.], [0., 0., 0., 1.], [1.; 3]);
    let point = parent.compose(child).point([2., 0., -1.]);
    for (actual, expected) in point.into_iter().zip([4., 26., 38.]) {
        close(actual, expected);
    }
}
