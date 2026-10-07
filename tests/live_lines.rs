use opensekai::{
    live::{HIT_Y, display_offset, note_geometry},
    live_lines::{Segment, View},
    model::Note,
};

#[test]
fn live_body_uses_its_three_texture_bands_and_authored_lane_width() {
    let start = Note {
        lane_end: 5,
        ..Note::default()
    };
    let end = Note {
        ticks: 480,
        lane_end: 5,
        ..Note::default()
    };
    let segment = Segment {
        start: &start,
        end: &end,
        times: [0., 0.4],
        index: 0,
        count: 1,
        critical: false,
    };
    let view = View {
        time: 0.,
        chart_speed: 1.,
        note_speed: 11.3,
    };
    let mesh = segment.mesh(&view);
    assert!(mesh.len() > 3 && mesh.len() <= 300 && mesh.len().is_multiple_of(3));
    let centre = &mesh[1];
    assert!((centre.xy[0] - centre.xy[2] - 760.32).abs() < 0.01);
    assert_eq!(centre.xy[1], HIT_Y);
    assert_eq!(mesh[0].uv[0], 0.125);
    assert_eq!(centre.uv[2], 0.125);
    assert_eq!(centre.uv[0], 0.875);
    assert_eq!(mesh[2].uv[0], 1.);
    assert!((centre.uv[3] - 0.475).abs() < 1e-6);
    let critical = Segment {
        critical: true,
        ..segment
    };
    assert!((critical.mesh(&view)[1].uv[3] - centre.uv[3] - 0.5).abs() < 1e-6);
    assert!(critical.mesh(&View { time: 0.4, ..view }).is_empty());
}

#[test]
fn held_curves_follow_time_easing_before_projection_and_keep_the_head_on_the_line() {
    let start = Note {
        lane_end: 1,
        note_line_type: 2,
        ..Note::default()
    };
    let end = Note {
        lane_start: 8,
        lane_end: 11,
        ..Note::default()
    };
    let segment = Segment {
        start: &start,
        end: &end,
        times: [0., 1.],
        index: 1,
        count: 3,
        critical: false,
    };
    assert_eq!(segment.lanes(0.5), [2., 3.5]);
    let mesh = segment.mesh(&View {
        time: 0.5,
        chart_speed: 1.,
        note_speed: 11.3,
    });
    let centre = &mesh[1];
    let actual_x = (centre.xy[0] + centre.xy[2]) * 0.5;
    let expected_x = 960. + (-6.545 + 5.5 * 0.595) * 108.;
    assert!((actual_x - expected_x).abs() < 0.01);
    assert_eq!(centre.xy[1], HIT_Y);
    assert!(mesh.iter().flat_map(|s| s.xy).all(f32::is_finite));
    assert!(
        mesh.iter()
            .flat_map(|s| [s.xy[1], s.xy[3], s.xy[5], s.xy[7]])
            .all(|y| (0. ..=HIT_Y).contains(&y))
    );
}

#[test]
fn an_entering_tail_stays_attached_to_its_block_across_the_old_spawn_boundary() {
    let start = Note {
        lane_start: 1,
        lane_end: 4,
        note_line_type: 2,
        ..Note::default()
    };
    let end = Note {
        lane_start: 7,
        lane_end: 10,
        ..Note::default()
    };
    let segment = Segment {
        start: &start,
        end: &end,
        times: [0., 4.],
        index: 0,
        count: 1,
        critical: false,
    };
    for speed in [1., 6., 11.3, 12.] {
        let boundary = 4. - display_offset(speed);
        let mut previous: Option<f32> = None;
        for step in -120..=120 {
            let time = boundary + step as f32 * display_offset(speed) / 1200.;
            let body = segment.mesh(&View {
                time,
                chart_speed: 1.,
                note_speed: speed,
            });
            assert!(!body.is_empty());
            let tip = &body[body.len() - 2];
            let note = note_geometry(&end, time, 4., 1., speed);
            let y = note[1] + note[3] / 2.;
            assert!(
                (tip.xy[5] - y).abs() < 0.002,
                "Detached tail at speed {speed}, time {time}"
            );
            assert!(((tip.xy[4] + tip.xy[6]) / 2. - (note[0] + note[2] / 2.)).abs() < 0.002);
            if let Some(previous) = previous {
                assert!((y - previous).abs() < 0.3);
            }
            previous = Some(y);
        }
    }
}

#[test]
fn adjacent_body_bands_share_edges_and_continuous_sections_never_pass_the_hit_line() {
    let start = Note {
        lane_end: 2,
        note_line_type: 1,
        ..Note::default()
    };
    let end = Note {
        lane_start: 8,
        lane_end: 11,
        ..Note::default()
    };
    let segment = Segment {
        start: &start,
        end: &end,
        times: [0., 4.],
        index: 0,
        count: 1,
        critical: false,
    };
    for time in [-0.2, 0., 0.3, 1., 2.5, 3.99] {
        let mesh = segment.mesh(&View {
            time,
            chart_speed: 1.,
            note_speed: 11.3,
        });
        for band in mesh.as_chunks::<3>().0 {
            assert_eq!(band[0].xy[0..2], band[1].xy[2..4]);
            assert_eq!(band[0].xy[6..8], band[1].xy[4..6]);
            assert_eq!(band[1].xy[0..2], band[2].xy[2..4]);
            assert_eq!(band[1].xy[6..8], band[2].xy[4..6]);
        }
        assert!(
            mesh.iter()
                .flat_map(|s| [s.xy[1], s.xy[3], s.xy[5], s.xy[7]])
                .all(|y| (0. ..=HIT_Y).contains(&y))
        );
        if time >= 0. {
            assert_eq!(mesh[1].xy[1], HIT_Y);
        }
    }
}
