use opensekai::{
    particles::{Gradient, MinMax},
    ui::Painting,
    unity::Bundle,
};
use std::path::Path;

#[test]
fn random_endpoints_remain_ordered_as_authored() {
    let curve: MinMax = serde_json::from_value(serde_json::json!({
        "mode":3,"scalar":0.05,"minimum":0.15,"lower":[],"upper":[]
    }))
    .unwrap();
    assert!((curve.evaluate(0., 0.) - 0.15).abs() < 1e-6);
    assert!((curve.evaluate(0., 1.) - 0.05).abs() < 1e-6);
    assert!((curve.evaluate(0., 0.25) - 0.125).abs() < 1e-6);
}

#[test]
fn gradients_sample_color_and_alpha_on_separate_key_times() {
    let mut gradient = Gradient {
        step: false,
        rgb: vec![[0., 1., 0., 0.], [1., 0., 0., 1.]],
        alpha: vec![[0., 0.], [0.5, 1.], [1., 0.]],
    };
    assert_eq!(gradient.evaluate(0.25), [0.75, 0., 0.25, 0.5]);
    assert_eq!(gradient.evaluate(0.75), [0.25, 0., 0.75, 0.5]);
    gradient.step = true;
    assert_eq!(gradient.evaluate(0.25), [0., 0., 1., 1.]);
}

#[test]
fn imported_bursts_loops_and_disabled_finish_follow_original_modules() {
    let bundle = Bundle::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let ap = &bundle.presentations.results["all_perfect"];
    assert_eq!(ap.particles.len(), 5);
    let burst = &ap.particles[0];
    assert!(burst.sample(0.1, 42).is_empty());
    assert_eq!(burst.sample(0.17, 42).len(), 150);
    assert!(burst.sample(2., 42).is_empty());
    let fc = &bundle.presentations.results["full_combo"];
    assert_eq!(fc.particles.len(), 4);
    assert_eq!(fc.particles[0].sample(0.34, 42).len(), 150);
    assert_eq!(fc.particles[1].sample(0.34, 42).len(), 150);
    let source_loop = &fc.particles[2];
    assert!(source_loop.sample(0.85, 42).is_empty());
    assert!(!source_loop.sample(1., 42).is_empty());
    let particles = source_loop.sample(4.5, 42);
    assert!(!particles.is_empty()); // Keeps emitting after the animation clip.
    assert_eq!(particles, source_loop.sample(4.5, 42));
    assert!(!source_loop.auto_seed);
    assert_eq!(source_loop.seed, 9);
    assert_eq!(particles, source_loop.sample(4.5, 43));
    assert_ne!(burst.sample(0.17, 42), burst.sample(0.17, 43));
    assert!(particles.len() <= source_loop.capacity);
    let finish = &bundle.presentations.results["finish"];
    assert_eq!(finish.particles.len(), 2);
    for time in [0., 0.5, 1., 3., 5.] {
        assert!(
            finish
                .particles
                .iter()
                .all(|p| p.sample(time, 42).is_empty())
        );
    }
}

#[test]
fn result_particle_textures_reach_additive_native_draws() {
    let bundle = Bundle::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let fc = &bundle.presentations.results["full_combo"];
    let mut paint = Painting::default();
    fc.draw_seeded(&bundle, &mut paint, 0.4, 42);
    let texture = bundle.paths[&fc.particles[0].texture].as_ptr();
    let draws = paint
        .draws
        .iter()
        .filter(|d| d.text == texture)
        .collect::<Vec<_>>();
    assert!(draws.len() > 100);
    assert!(draws.iter().all(|d| d.kind == 3
        && d.reserved & 4 != 0
        && d.rgba & 255 > 0
        && d.xy.iter().all(|v| v.is_finite())));
    assert!(
        fc.texture_paths(&bundle)
            .any(|p| p == fc.particles[0].texture)
    );
}

#[test]
fn original_background_layers_use_the_active_song_cover() {
    use std::ffi::CString;
    let bundle = Bundle::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let cover = CString::new("local-cover.png").unwrap();
    let mut paint = Painting::default();
    bundle
        .live_background
        .draw(&bundle, Some(&cover), &mut paint);
    assert!(paint.draws.len() > 10);
    let covers = paint
        .draws
        .iter()
        .filter(|d| d.text == cover.as_ptr())
        .collect::<Vec<_>>();
    assert_eq!(covers.len(), 6);
    assert!(covers.iter().all(|d| d.reserved & 16 != 0));
    assert!(
        covers
            .iter()
            .all(|d| d.uv == [0., 1., 1., 0.] && d.clip[2] > 0. && d.clip[3] > 0.)
    );
    assert!(
        paint
            .draws
            .iter()
            .all(|d| d.xy.iter().all(|v| v.is_finite()))
    );
}
